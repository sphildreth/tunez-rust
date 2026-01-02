use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread::{self, JoinHandle},
    time::Duration,
};

use thiserror::Error;

/// Type alias for audio sample callback
pub type SampleCallback = Arc<dyn Fn(&[f32]) + Send + Sync>;

/// Audio playback errors.
#[derive(Debug, Error)]
pub enum AudioError {
    #[error("audio backend unavailable: {0}")]
    Backend(String),
    #[error("unsupported source: {0}")]
    UnsupportedSource(String),
    #[error("io error: {0}")]
    Io(String),
    #[error("{0}")]
    Other(String),
}

pub type AudioResult<T> = Result<T, AudioError>;

/// Abstract audio source.
#[derive(Debug, Clone)]
pub enum AudioSource {
    /// A URL (local file via `file://` or remote). Backends may support a subset.
    Url(String),
    /// A local file path.
    File(PathBuf),
}

/// Runtime playback state for a handle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioState {
    Idle,
    Playing,
    Completed,
    Stopped,
    Error,
}

/// Handle representing an in-flight playback operation.
pub struct AudioHandle {
    state: Arc<Mutex<AudioState>>,
    stop_flag: Arc<AtomicBool>,
    join: Option<JoinHandle<()>>,
    /// Keeps audio resources alive until the handle is dropped.
    /// The stream is kept on the spawning thread (not sent).
    #[allow(dead_code)]
    keepalive: Option<Box<dyn std::any::Any + Send>>,
    /// Non-Send keepalive slot for audio backends (e.g., cpal::Stream).
    /// Must be dropped on the same thread it was created.
    #[allow(dead_code)]
    local_keepalive: Option<Arc<Mutex<Box<dyn std::any::Any>>>>,
    /// Optional callback for streaming audio samples to visualization
    sample_callback: Option<SampleCallback>,
    /// Number of frames (independent samples) played so far
    frames_played: Arc<std::sync::atomic::AtomicU64>,
    /// Sample rate (frames per second)
    sample_rate: u32,
}

impl std::fmt::Debug for AudioHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AudioHandle")
            .field("state", &self.state())
            .finish_non_exhaustive()
    }
}

impl AudioHandle {
    /// Set a callback to receive audio samples for visualization
    pub fn set_sample_callback(&mut self, callback: SampleCallback) {
        self.sample_callback = Some(callback);
    }
}

impl AudioHandle {
    pub(crate) fn spawn_simulated(duration: Duration) -> Self {
        let state = Arc::new(Mutex::new(AudioState::Playing));
        let stop_flag = Arc::new(AtomicBool::new(false));
        let state_clone = state.clone();
        let stop_clone = stop_flag.clone();

        let join = thread::spawn(move || {
            let tick = Duration::from_millis(50);
            let mut elapsed = Duration::ZERO;
            while elapsed < duration && !stop_clone.load(Ordering::SeqCst) {
                thread::sleep(tick);
                elapsed += tick;
            }
            let mut guard = state_clone.lock().unwrap();
            if stop_clone.load(Ordering::SeqCst) {
                *guard = AudioState::Stopped;
            } else {
                *guard = AudioState::Completed;
            }
        });

        Self {
            state,
            stop_flag,
            join: Some(join),
            keepalive: None,
            local_keepalive: None,
            sample_callback: None,
            frames_played: Arc::new(std::sync::atomic::AtomicU64::new(0)), // Default for simulated
            sample_rate: 0,                                                // Default for simulated
        }
    }

    #[allow(dead_code)]
    pub(crate) fn with_keepalive(
        state: Arc<Mutex<AudioState>>,
        stop_flag: Arc<AtomicBool>,
        join: JoinHandle<()>,
        keepalive: Arc<Mutex<Box<dyn std::any::Any>>>,
        frames_played: Arc<std::sync::atomic::AtomicU64>,
        sample_rate: u32,
    ) -> Self {
        Self {
            state,
            stop_flag,
            join: Some(join),
            keepalive: None,
            local_keepalive: Some(keepalive),
            sample_callback: None,
            frames_played,
            sample_rate,
        }
    }

    pub fn state(&self) -> AudioState {
        *self.state.lock().unwrap()
    }

    pub fn stop(mut self) {
        self.stop_flag.store(true, Ordering::SeqCst);
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
        // drop keepalive to release backend resources
        let _ = self.keepalive.take();
        let _ = self.local_keepalive.take();
    }

    /// Call the sample callback if set
    pub fn send_samples(&self, samples: &[f32]) {
        if let Some(callback) = &self.sample_callback {
            callback(samples);
        }
    }

    /// Get current playback position
    pub fn position(&self) -> Duration {
        let frames = self.frames_played.load(Ordering::SeqCst);
        if self.sample_rate > 0 {
            Duration::from_secs_f64(frames as f64 / self.sample_rate as f64)
        } else {
            Duration::ZERO
        }
    }
}

/// Audio backend interface.
pub trait AudioEngine: Send + Sync {
    fn play(&self, source: AudioSource) -> AudioResult<AudioHandle>;
}

/// No-op audio engine used for tests and headless environments.
#[derive(Debug, Default, Clone)]
pub struct NullAudioEngine;

impl AudioEngine for NullAudioEngine {
    fn play(&self, _source: AudioSource) -> AudioResult<AudioHandle> {
        // Simulate ~1 second of playback.
        Ok(AudioHandle::spawn_simulated(Duration::from_millis(1000)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn null_engine_completes() {
        let engine = NullAudioEngine;
        let handle = engine
            .play(AudioSource::Url("test".into()))
            .expect("null engine should succeed");
        thread::sleep(Duration::from_millis(1100));
        assert_eq!(handle.state(), AudioState::Completed);
    }

    #[test]
    fn handle_can_stop_early() {
        let engine = NullAudioEngine;
        let handle = engine
            .play(AudioSource::Url("test".into()))
            .expect("null engine should succeed");
        handle.stop();
    }
}
