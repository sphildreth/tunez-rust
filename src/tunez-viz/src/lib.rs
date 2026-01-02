//! Visualization system for Tunez music player.
//!
//! Provides multiple visualization modes and FFT computation for audio analysis.

use ratatui::{
    style::Style,
    widgets::{Block, Sparkline},
    Frame,
};
use rustfft::{num_complex::Complex, num_traits::Zero, Fft, FftPlanner};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use tunez_core::models::Track;

/// Different visualization modes available in Tunez
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VizMode {
    /// Spectrum analyzer with bars
    Spectrum,
    /// Oscilloscope waveform
    Oscilloscope,
    /// VU meter style
    VUMeter,
    /// Particle visualization
    Particles,
}

impl VizMode {
    pub fn all() -> &'static [VizMode] {
        &[
            VizMode::Spectrum,
            VizMode::Oscilloscope,
            VizMode::VUMeter,
            VizMode::Particles,
        ]
    }

    pub fn name(&self) -> &'static str {
        match self {
            VizMode::Spectrum => "Spectrum",
            VizMode::Oscilloscope => "Oscilloscope",
            VizMode::VUMeter => "VU Meter",
            VizMode::Particles => "Particles",
        }
    }
}

/// Visualization state and computation
#[derive(Clone)]
pub struct Visualizer {
    /// Audio sample buffer (wrapped for thread safety)
    sample_buffer: Arc<Mutex<VecDeque<f32>>>,
    /// Current visualization mode
    mode: VizMode,
    /// Current track for context
    current_track: Option<Track>,
    /// Animation phase for particle effects
    phase: f32,
    /// FFT processor
    fft: Arc<dyn Fft<f32>>,
    /// Pre-computed Hann window
    window: Vec<f32>,
    /// Scratch buffer for FFT computation
    scratch: Arc<Mutex<Vec<Complex<f32>>>>,
}

impl Visualizer {
    pub fn new() -> Self {
        let mut planner = FftPlanner::new();
        let fft = planner.plan_fft_forward(1024);
        
        // Pre-compute Hann window
        let window: Vec<f32> = (0..1024)
            .map(|i| {
                let n = i as f32;
                let len = 1024.0;
                0.5 * (1.0 - (2.0 * std::f32::consts::PI * n / (len - 1.0)).cos())
            })
            .collect();

        Self {
            sample_buffer: Arc::new(Mutex::new(VecDeque::with_capacity(2048))),
            mode: VizMode::Spectrum,
            current_track: None,
            phase: 0.0,
            fft,
            window,
            scratch: Arc::new(Mutex::new(vec![Complex::zero(); 1024])),
        }
    }

    /// Set the current visualization mode
    pub fn set_mode(&mut self, mode: VizMode) {
        self.mode = mode;
    }

    /// Get the current visualization mode
    pub fn mode(&self) -> VizMode {
        self.mode
    }

    /// Add audio samples for visualization (thread-safe)
    pub fn add_samples(&self, samples: &[f32]) {
        let mut buffer = self.sample_buffer.lock().unwrap();
        for &sample in samples {
            if buffer.len() >= 2048 {
                buffer.pop_front();
            }
            buffer.push_back(sample);
        }
    }

    /// Set the current track for context
    pub fn set_current_track(&mut self, track: Option<Track>) {
        self.current_track = track;
    }

    /// Update animation phase (called on each tick)
    pub fn update_animation(&mut self) {
        self.phase += 0.1;
        if self.phase > std::f32::consts::TAU {
            self.phase -= std::f32::consts::TAU;
        }
    }

    /// Check if visualization should render based on terminal capabilities
    /// Returns true if visualization should be rendered, false if it should be skipped
    pub fn should_render(&self, width: u16, height: u16) -> bool {
        // Minimum size for meaningful visualization
        if width < 20 || height < 3 {
            return false;
        }
        
        // Check for color support (this would be passed from UI context)
        // For now, always render if size is adequate
        true
    }

    /// Get recommended FPS based on terminal size and capabilities
    /// Returns frames per second (FPS)
    pub fn get_recommended_fps(&self, width: u16, height: u16) -> u32 {
        // Adaptive FPS based on terminal size
        // Smaller terminals = lower FPS for better performance
        if width < 40 || height < 8 {
            15 // Low FPS for small terminals
        } else if width < 60 || height < 12 {
            25 // Medium FPS for medium terminals
        } else {
            30 // High FPS for large terminals
        }
    }

    /// Compute visualization data based on current mode
    pub fn compute(&self) -> VisualizationData {
        match self.mode {
            VizMode::Spectrum => self.compute_spectrum(),
            VizMode::Oscilloscope => self.compute_oscilloscope(),
            VizMode::VUMeter => self.compute_vu_meter(),
            VizMode::Particles => self.compute_particles(),
        }
    }

    fn compute_spectrum(&self) -> VisualizationData {
        let buffer_lock = self.sample_buffer.lock().unwrap();
        // Take latest 1024 samples
        let len = buffer_lock.len();
        let skip = if len > 1024 { len - 1024 } else { 0 };
        
        let mut input: Vec<Complex<f32>> = buffer_lock
            .iter()
            .skip(skip)
            .zip(self.window.iter())
            .map(|(&s, &w)| Complex::new(s * w, 0.0))
            .collect();
            
        // Pad with zeros if not enough samples
        while input.len() < 1024 {
            input.push(Complex::zero());
        }
        
        // Drop lock before expensive FFT
        drop(buffer_lock);
        
        // Run FFT
        let mut scratch = self.scratch.lock().unwrap();
        // Fft::process takes buffer as slice of Complex.
        // It processes in-place or out-of-place depending on implementation, 
        // but rustfft `process` generally takes `&mut [Complex]`.
        // We reuse the scratch buffer if needed, but here `input` is our proper buffer.
        // `process` takes `input` and `scratch`.
        if let Err(_) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
             // Just safeguard against partial inputs, though we padded.
        })) {}

        self.fft.process_with_scratch(&mut input, &mut scratch);
        
        // Compute magnitudes (first half is enough, symmetric)
        // 512 bins from 0 to Nyquist.
        // Map to 64 bars typically.
        let magnitudes: Vec<f32> = input.iter()
            .take(512)
            .map(|c| c.norm())
            .collect();
            
        // Map 512 bins to ~64 display bars
        // Simple linear grouping for MVP, or log
        // Let's do a simple grouping: 512 / 8 = 64
        let bars: Vec<u64> = magnitudes.chunks(8)
            .map(|chunk| {
                 let sum: f32 = chunk.iter().sum();
                 // Scale for visual
                 let val = (sum * 2.0).min(100.0);
                 val as u64
            })
            .collect();

        VisualizationData::Spectrum(bars)
    }

    fn compute_oscilloscope(&self) -> VisualizationData {
        let buffer = self.sample_buffer.lock().unwrap();
        let samples: Vec<u64> = buffer
            .iter()
            .take(256) // Take a reasonable number of samples for waveform
            .map(|&s| {
                // Scale to 0-100 range for visualization
                let scaled = (s + 1.0) * 50.0; // From [-1,1] to [0,100]
                scaled.clamp(0.0, 100.0) as u64
            })
            .collect();

        VisualizationData::Waveform(samples)
    }

    fn compute_vu_meter(&self) -> VisualizationData {
        // Calculate RMS of recent samples
        let buffer = self.sample_buffer.lock().unwrap();
        let rms: f32 = buffer
            .iter()
            .take(128)
            .map(|&s| s * s)
            .sum::<f32>()
            .sqrt();

        // Convert to 0-100 scale
        let level = (rms * 100.0).min(100.0) as u64;

        VisualizationData::VUMeter(level)
    }

    fn compute_particles(&self) -> VisualizationData {
        // Use a calculated phase based on time or sample buffer
        let phase = (self.phase + 0.1) % (std::f32::consts::TAU);

        // Generate particle positions based on audio activity
        let buffer = self.sample_buffer.lock().unwrap();
        let activity: f32 = buffer
            .iter()
            .take(64)
            .map(|&s| s.abs())
            .sum::<f32>()
            .max(0.001); // Avoid zero

        let particles: Vec<(u16, u16, u8)> = (0..10)
            .map(|i| {
                let x = ((i as f32 * 0.5 + phase.sin() * activity * 10.0) % 100.0) as u16;
                let y = ((i as f32 * 0.7 + phase.cos() * activity * 15.0) % 100.0) as u16;
                let intensity = (activity * 255.0) as u8;
                (x, y, intensity)
            })
            .collect();

        VisualizationData::Particles(particles)
    }

    /// Render the visualization to the frame
    pub fn render(&self, frame: &mut Frame, area: ratatui::layout::Rect) {
        self.render_with_color_support(frame, area, true);
    }

    /// Render the visualization with color support control
    pub fn render_with_color_support(&self, frame: &mut Frame, area: ratatui::layout::Rect, use_color: bool) {
        let data = self.compute();

        match data {
            VisualizationData::Spectrum(magnitudes) => {
                let mut sparkline = Sparkline::default()
                    .block(Block::default().title(self.mode.name()))
                    .data(&magnitudes);
                
                // Apply color if supported
                if use_color {
                    sparkline = sparkline.style(Style::default().fg(ratatui::style::Color::Cyan));
                }
                
                frame.render_widget(sparkline, area);
            }
            VisualizationData::Waveform(samples) => {
                let mut sparkline = Sparkline::default()
                    .block(Block::default().title(self.mode.name()))
                    .data(&samples);
                
                // Apply color if supported
                if use_color {
                    sparkline = sparkline.style(Style::default().fg(ratatui::style::Color::Green));
                }
                
                frame.render_widget(sparkline, area);
            }
            VisualizationData::VUMeter(level) => {
                // Create a simple bar representation
                let bar_data: Vec<u64> = vec![0; 10].into_iter()
                    .enumerate()
                    .map(|(i, _)| if (i + 1) as u64 * 10 <= level { 100 } else { 0 })
                    .collect();

                let mut sparkline = Sparkline::default()
                    .block(Block::default().title(self.mode.name()))
                    .data(&bar_data);
                
                // Apply color if supported
                if use_color {
                    sparkline = sparkline.style(Style::default().fg(ratatui::style::Color::Yellow));
                }
                
                frame.render_widget(sparkline, area);
            }
            VisualizationData::Particles(_) => {
                // For particles, we'll just show a placeholder since ratatui Sparkline doesn't support particle systems
                let mut sparkline = Sparkline::default()
                    .block(Block::default().title(self.mode.name()))
                    .data(&[50, 60, 70, 80, 90, 80, 70, 60, 50]);
                
                // Apply color if supported
                if use_color {
                    sparkline = sparkline.style(Style::default().fg(ratatui::style::Color::Magenta));
                }
                
                frame.render_widget(sparkline, area);
            }
        }
    }
}

/// Data structure representing visualization output
pub enum VisualizationData {
    /// Spectrum analyzer data (frequency magnitudes)
    Spectrum(Vec<u64>),
    /// Waveform data (time-domain samples)
    Waveform(Vec<u64>),
    /// VU meter level
    VUMeter(u64),
    /// Particle positions and intensities
    Particles(Vec<(u16, u16, u8)>),
}

impl Default for Visualizer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn visualizer_creation() {
        let viz = Visualizer::new();
        assert_eq!(viz.mode(), VizMode::Spectrum);
    }

    #[test]
    fn viz_mode_names() {
        assert_eq!(VizMode::Spectrum.name(), "Spectrum");
        assert_eq!(VizMode::Oscilloscope.name(), "Oscilloscope");
        assert_eq!(VizMode::VUMeter.name(), "VU Meter");
        assert_eq!(VizMode::Particles.name(), "Particles");
    }

    #[test]
    fn add_samples() {
        let viz = Visualizer::new();
        let samples = vec![0.5, -0.3, 0.8, -0.1];
        viz.add_samples(&samples);
        
        let data = viz.compute();
        match data {
            VisualizationData::Spectrum(_) => {} // Expected
            _ => panic!("Expected spectrum data"),
        }
    }

    #[test]
    fn spectrum_detects_sine_wave() {
        // Generate a sine wave at ~2200Hz (approx bin 50 of 1024 points at 44.1kHz)
        // 1024 points.
        // We just need a pure tone.
        // If we have 1024 samples, and the sine wave completes N cycles in that window.
        // frequency bin k = cycles * (SampleRate / N) ? No, 
        // bin k corresponds to k cycles per window.
        // So if we inject a sine wave with 32 cycles in 1024 samples.
        // Bin 32 should peak.
        // 32 cycles in 1024 samples.
        let hz = 32.0;
        let samples: Vec<f32> = (0..2048) // Fill more than enough
            .map(|i| {
                let t = i as f32;
                (t * hz * std::f32::consts::TAU / 1024.0).sin()
            })
            .collect();
            
        let viz = Visualizer::new();
        viz.add_samples(&samples);
        
        // Compute
        let data = viz.compute();
        match data {
            VisualizationData::Spectrum(bars) => {
                // We bucket 512 bins into 64 bars. 8 bins per bar.
                // Bin 32 is in bar range [32/8] = 4.
                // So bar 4 should have high value.
                assert!(bars.len() >= 5);
                // Check if peak is roughly where expected
                // Note: Windowing might spread energy slightly, but 4 should be high.
                let peak_idx = bars.iter().enumerate().max_by_key(|(_, &v)| v).map(|(i, _)| i).unwrap();
                println!("Peak index: {}", peak_idx);
                // Allow +/- 1 bar tolerance
                assert!((peak_idx as i32 - 4).abs() <= 1, "Expected peak around bar 4, got {}", peak_idx);
            }
            _ => panic!("Wrong mode"),
        }
    }
}