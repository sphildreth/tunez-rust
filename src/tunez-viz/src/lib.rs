//! Visualization system for Tunez music player.
//!
//! Provides multiple visualization modes and FFT computation for audio analysis.

use ratatui::{
    style::Style,
    widgets::{Block, Sparkline},
    Frame,
};
use rustfft::{FftPlanner, num_complex::Complex};
use std::collections::VecDeque;
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
    /// FFT size
    fft_size: usize,
    /// Audio sample buffer
    sample_buffer: VecDeque<f32>,
    /// Current visualization mode
    mode: VizMode,
    /// Current track for context
    current_track: Option<Track>,
    /// Animation phase for particle effects
    phase: f32,
}

impl Visualizer {
    pub fn new() -> Self {
        Self {
            fft_size: 1024,
            sample_buffer: VecDeque::with_capacity(2048),
            mode: VizMode::Spectrum,
            current_track: None,
            phase: 0.0,
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

    /// Add audio samples for visualization
    pub fn add_samples(&mut self, samples: &[f32]) {
        for &sample in samples {
            if self.sample_buffer.len() >= 2048 {
                self.sample_buffer.pop_front();
            }
            self.sample_buffer.push_back(sample);
        }
    }

    /// Set the current track for context
    pub fn set_current_track(&mut self, track: Option<Track>) {
        self.current_track = track;
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
        // For now, return a simple visualization based on sample activity
        // In a real implementation, we would perform FFT analysis
        let activity: f32 = self
            .sample_buffer
            .iter()
            .take(512)
            .map(|&s| s.abs())
            .sum::<f32>();

        let magnitudes: Vec<u64> = (0..64)
            .map(|i| {
                let base = (activity * 10.0) as u64;
                let variation = (i as u64 * 5) % 20;
                base.saturating_sub(variation)
            })
            .collect();

        VisualizationData::Spectrum(magnitudes)
    }

    fn compute_oscilloscope(&self) -> VisualizationData {
        let samples: Vec<u64> = self
            .sample_buffer
            .iter()
            .take(256) // Take a reasonable number of samples for waveform
            .map(|&s| {
                // Scale to 0-100 range for visualization
                let scaled = (s + 1.0) * 50.0; // From [-1,1] to [0,100]
                scaled.max(0.0).min(100.0) as u64
            })
            .collect();

        VisualizationData::Waveform(samples)
    }

    fn compute_vu_meter(&self) -> VisualizationData {
        // Calculate RMS of recent samples
        let rms: f32 = self
            .sample_buffer
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
        let activity: f32 = self
            .sample_buffer
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
        let data = self.compute();

        match data {
            VisualizationData::Spectrum(magnitudes) => {
                let sparkline = Sparkline::default()
                    .block(Block::default().title(self.mode.name()))
                    .data(&magnitudes);
                frame.render_widget(sparkline, area);
            }
            VisualizationData::Waveform(samples) => {
                let sparkline = Sparkline::default()
                    .block(Block::default().title(self.mode.name()))
                    .data(&samples);
                frame.render_widget(sparkline, area);
            }
            VisualizationData::VUMeter(level) => {
                // Create a simple bar representation
                let bar_data: Vec<u64> = vec![0; 10].into_iter()
                    .enumerate()
                    .map(|(i, _)| if (i + 1) as u64 * 10 <= level { 100 } else { 0 })
                    .collect();

                let sparkline = Sparkline::default()
                    .block(Block::default().title(self.mode.name()))
                    .data(&bar_data);
                frame.render_widget(sparkline, area);
            }
            VisualizationData::Particles(_) => {
                // For particles, we'll just show a placeholder since ratatui Sparkline doesn't support particle systems
                let sparkline = Sparkline::default()
                    .block(Block::default().title(self.mode.name()))
                    .data(&[50, 60, 70, 80, 90, 80, 70, 60, 50]);
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
        let mut viz = Visualizer::new();
        let samples = vec![0.5, -0.3, 0.8, -0.1];
        viz.add_samples(&samples);
        
        let data = viz.compute();
        match data {
            VisualizationData::Spectrum(_) => {} // Expected
            _ => panic!("Expected spectrum data"),
        }
    }
}