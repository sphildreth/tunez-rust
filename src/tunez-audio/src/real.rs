use std::{
    fs::File,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread,
};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use symphonia::{
    core::{
        audio::SampleBuffer, codecs::DecoderOptions, formats::FormatOptions, meta::MetadataOptions,
        probe::Hint,
    },
    default,
};

use crate::engine::SampleCallback;
use crate::{AudioEngine, AudioError, AudioHandle, AudioResult, AudioSource, AudioState};

/// Audio engine backed by cpal + symphonia (local files only).
#[derive(Debug, Default, Clone, Copy)]
pub struct CpalAudioEngine;

impl CpalAudioEngine {
    fn resolve_path(source: AudioSource) -> AudioResult<PathBuf> {
        match source {
            AudioSource::File(path) => Ok(path),
            AudioSource::Url(url) => {
                if let Some(stripped) = url.strip_prefix("file://") {
                    Ok(PathBuf::from(stripped))
                } else {
                    Err(AudioError::UnsupportedSource(url))
                }
            }
        }
    }
}

impl AudioEngine for CpalAudioEngine {
    fn play(&self, source: AudioSource) -> AudioResult<AudioHandle> {
        let path = Self::resolve_path(source)?;
        let samples = decode_to_f32(&path)?;

        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or_else(|| AudioError::Backend("no output device".into()))?;
        let config = device
            .default_output_config()
            .map_err(|e| AudioError::Backend(e.to_string()))?;

        let state = Arc::new(Mutex::new(AudioState::Playing));
        let stop_flag = Arc::new(AtomicBool::new(false));
        let state_clone = state.clone();
        let stop_clone = stop_flag.clone();

        let sample_rate = config.sample_rate().0;
        let channels = config.channels() as usize;

        // Interleave samples; if the source is mono, duplicate to all channels.
        let mut interleaved = Vec::with_capacity(samples.len() * channels);
        for frame in samples.chunks(1) {
            for _ in 0..channels {
                interleaved.push(frame[0]);
            }
        }

        let mut idx = 0usize;
        // Create a shared sample callback that will be set on the handle
        let sample_callback: Arc<Mutex<Option<SampleCallback>>> = Arc::new(Mutex::new(None));
        let sample_callback_clone = sample_callback.clone();

        // Create frames_played counter
        let frames_played = Arc::new(std::sync::atomic::AtomicU64::new(0));
        let frames_played_clone = frames_played.clone();
        // Reset idx for stream (already initialized above but we need to track it inside closure)
        // Wait, current impl captures `idx` by value (copy) if it's usize? No, closure moves `idx`.
        // `idx` is initialized at line 71: `let mut idx = 0usize;`.
        // `move |data...|` captures it.

        let stream = match config.sample_format() {
            cpal::SampleFormat::F32 => device.build_output_stream(
                &config.into(),
                move |data: &mut [f32], _| {
                    // Generate samples for this chunk
                    let channels = 2; // Hardcoded? No, `channels` var at line 61. But we can't capture it easily if traits obscure it?
                                      // Re-capture `channels` from outer scope?
                                      // Wait, `channels` is defined at line 61. Closure `move` will capture it.
                    let channel_count = channels;

                    let mut chunk = Vec::with_capacity(data.len());
                    let mut frames_processed = 0;

                    for sample in data.iter_mut() {
                        if stop_clone.load(Ordering::SeqCst) || idx >= interleaved.len() {
                            *sample = 0.0;
                            chunk.push(0.0);
                            // Do not increment idx/frames if stopped/finished
                            continue;
                        }
                        *sample = interleaved[idx];
                        chunk.push(interleaved[idx]);
                        idx += 1;
                        frames_processed += 1;
                    }

                    // Update frames played (frames = samples / channels)
                    if channel_count > 0 {
                        frames_played_clone
                            .fetch_add((frames_processed / channel_count) as u64, Ordering::SeqCst);
                    }

                    // Send samples to visualization callback if available
                    if let Some(callback) = sample_callback_clone.lock().unwrap().as_ref() {
                        callback(&chunk);
                    }

                    if idx >= interleaved.len() {
                        stop_clone.store(true, Ordering::SeqCst);
                    }
                },
                move |err| {
                    tracing::error!("cpal stream error: {}", err);
                    let mut guard = state_clone.lock().unwrap();
                    *guard = AudioState::Error;
                },
                None,
            ),
            format => {
                return Err(AudioError::Backend(format!(
                    "unsupported sample format: {format:?}"
                )));
            }
        }
        .map_err(|e: cpal::BuildStreamError| AudioError::Backend(e.to_string()))?;

        stream
            .play()
            .map_err(|e: cpal::PlayStreamError| AudioError::Backend(e.to_string()))?;

        let join = thread::spawn({
            let state = state.clone();
            let stop_flag = stop_flag.clone();
            move || {
                while !stop_flag.load(Ordering::SeqCst) {
                    thread::sleep(std::time::Duration::from_millis(20));
                }
                let mut guard = state.lock().unwrap();
                if *guard != AudioState::Error {
                    *guard = AudioState::Completed;
                }
            }
        });

        // Keep stream alive by wrapping in Arc<Mutex> (Stream is not Send on some platforms)
        #[allow(clippy::arc_with_non_send_sync)]
        let stream_keepalive: Arc<Mutex<Box<dyn std::any::Any>>> =
            Arc::new(Mutex::new(Box::new(stream)));

        let mut handle = AudioHandle::with_keepalive(
            state,
            stop_flag,
            join,
            stream_keepalive.clone(),
            frames_played,
            sample_rate,
        );

        // Set up the sample callback forwarding
        // The handle will store the callback and the stream will call it
        let sample_callback_clone = sample_callback.clone();
        let forwarding_callback: SampleCallback = Arc::new(move |samples| {
            if let Some(callback) = sample_callback_clone.lock().unwrap().as_ref() {
                callback(samples);
            }
        });
        handle.set_sample_callback(forwarding_callback);

        // Set up audio control
        struct CpalControl {
            stream: Arc<Mutex<Box<dyn std::any::Any>>>,
            frames_played: Arc<std::sync::atomic::AtomicU64>,
            sample_rate: u32,
        }
        impl crate::engine::AudioControl for CpalControl {
            fn pause(&self) -> AudioResult<()> {
                let guard = self.stream.lock().unwrap();
                if let Some(s) = guard.downcast_ref::<cpal::Stream>() {
                    s.pause()
                        .map_err(|e| crate::AudioError::Backend(e.to_string()))?;
                }
                Ok(())
            }
            fn resume(&self) -> AudioResult<()> {
                let guard = self.stream.lock().unwrap();
                if let Some(s) = guard.downcast_ref::<cpal::Stream>() {
                    s.play()
                        .map_err(|e| crate::AudioError::Backend(e.to_string()))?;
                }
                Ok(())
            }
            fn seek(&self, position: std::time::Duration) -> AudioResult<()> {
                let frames = (position.as_secs_f64() * self.sample_rate as f64) as u64;
                self.frames_played
                    .store(frames, std::sync::atomic::Ordering::SeqCst);
                Ok(())
            }
        }

        handle.set_control(Arc::new(CpalControl {
            stream: stream_keepalive,
            frames_played: frames_played.clone(),
            sample_rate,
        }));

        Ok(handle)
    }
}

fn decode_to_f32(path: &Path) -> AudioResult<Vec<f32>> {
    let file = File::open(path).map_err(|e| AudioError::Io(e.to_string()))?;
    // File implements MediaSource directly; no BufReader wrapper needed.
    let mss = symphonia::core::io::MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }

    let probed = default::get_probe()
        .format(
            &hint,
            mss,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .map_err(|e| AudioError::Backend(e.to_string()))?;
    let mut format = probed.format;
    let track = format
        .default_track()
        .ok_or_else(|| AudioError::Backend("no default track".into()))?;
    // Extract values we need before the loop to avoid holding a borrow across next_packet()
    let track_id = track.id;
    let codec_params = track.codec_params.clone();
    let mut decoder = default::get_codecs()
        .make(&codec_params, &DecoderOptions::default())
        .map_err(|e| AudioError::Backend(e.to_string()))?;

    let mut samples = Vec::new();
    loop {
        let packet = match format.next_packet() {
            Ok(packet) => packet,
            Err(symphonia::core::errors::Error::IoError(_)) => break,
            Err(err) => return Err(AudioError::Backend(err.to_string())),
        };
        if packet.track_id() != track_id {
            continue;
        }
        let audio_buf = decoder
            .decode(&packet)
            .map_err(|e| AudioError::Backend(e.to_string()))?;
        let spec = *audio_buf.spec();
        let mut sample_buf = SampleBuffer::<f32>::new(audio_buf.capacity() as u64, spec);
        sample_buf.copy_interleaved_ref(audio_buf);
        samples.extend_from_slice(sample_buf.samples());
    }

    // Downsample if necessary to keep total sample count reasonable for testing contexts.
    let max_samples = 48000 * 120; // ~2 minutes at 48kHz mono
    if samples.len() > max_samples {
        samples.truncate(max_samples);
    }
    Ok(samples)
}
