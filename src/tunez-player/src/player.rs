use crate::{Queue, QueueId, QueueItem};
use std::sync::Arc;
use tunez_audio::{AudioEngine, AudioHandle, AudioSource};

/// Type alias for player sample callback
pub type PlayerSampleCallback = Box<dyn Fn(&[f32]) + Send + Sync>;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum PlayerState {
    #[default]
    Stopped,
    Buffering {
        id: QueueId,
    },
    Playing {
        id: QueueId,
    },
    Paused {
        id: QueueId,
    },
    Error {
        id: Option<QueueId>,
        message: String,
    },
}

#[derive(Default)]
pub struct Player {
    queue: Queue,
    state: PlayerState,
    audio: Option<AudioHandle>,
    sample_callback: Option<PlayerSampleCallback>,
}

impl std::fmt::Debug for Player {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Player")
            .field("queue", &self.queue)
            .field("state", &self.state)
            .field("audio", &self.audio)
            .finish_non_exhaustive()
    }
}

impl Player {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn queue(&self) -> &Queue {
        &self.queue
    }

    pub fn queue_mut(&mut self) -> &mut Queue {
        &mut self.queue
    }

    pub fn state(&self) -> &PlayerState {
        &self.state
    }

    pub fn current(&self) -> Option<&QueueItem> {
        self.queue.current()
    }

    /// Get current playback position
    pub fn position(&self) -> std::time::Duration {
        if let Some(audio) = &self.audio {
            audio.position()
        } else {
            std::time::Duration::ZERO
        }
    }

    /// Set a callback to receive audio samples for visualization
    pub fn set_sample_callback<F>(&mut self, callback: F)
    where
        F: Fn(&[f32]) + Send + Sync + 'static,
    {
        self.sample_callback = Some(Box::new(callback));
    }

    pub fn play(&mut self) -> Option<&QueueItem> {
        if self.queue.current().is_none() {
            self.queue.select_first()?;
        }
        let current = self.queue.current()?;
        self.state = PlayerState::Playing { id: current.id };
        self.stop_audio();
        self.queue.current()
    }

    pub fn play_with_audio<E: AudioEngine>(
        &mut self,
        engine: &E,
        source: AudioSource,
    ) -> Option<&QueueItem> {
        self.play()?;
        let current_id = self.queue.current().map(|c| c.id)?;
        match engine.play(source) {
            Ok(mut handle) => {
                // Set up sample callback if one has been registered
                if let Some(callback) = self.sample_callback.take() {
                    // Convert Box to Arc for the AudioHandle
                    let arc_callback: Arc<dyn Fn(&[f32]) + Send + Sync> = Arc::new(callback);
                    handle.set_sample_callback(arc_callback);
                }
                self.audio = Some(handle);
                self.queue.current()
            }
            Err(err) => {
                self.state = PlayerState::Error {
                    id: Some(current_id),
                    message: err.to_string(),
                };
                None
            }
        }
    }

    /// Get mutable access to audio handle for setting up callbacks
    pub fn audio_mut(&mut self) -> Option<&mut AudioHandle> {
        self.audio.as_mut()
    }

    pub fn pause(&mut self) -> bool {
        if let PlayerState::Playing { id } = self.state {
            self.state = PlayerState::Paused { id };
            self.stop_audio();
            return true;
        }
        false
    }

    pub fn resume(&mut self) -> bool {
        if let PlayerState::Paused { id } = self.state {
            self.state = PlayerState::Playing { id };
            return true;
        }
        false
    }

    pub fn stop(&mut self) {
        self.stop_audio();
        self.queue.reset_current();
        self.state = PlayerState::Stopped;
    }

    pub fn skip_next(&mut self) -> Option<&QueueItem> {
        self.queue.advance()?;
        let next_id = self.queue.current().map(|c| c.id)?;
        self.state = PlayerState::Buffering { id: next_id };
        self.state = PlayerState::Playing { id: next_id };
        self.stop_audio();
        self.queue.current()
    }

    pub fn set_error(&mut self, message: impl Into<String>) {
        let id = self.queue.current().map(|item| item.id);
        self.state = PlayerState::Error {
            id,
            message: message.into(),
        };
        self.stop_audio();
    }

    /// Handle a track error by logging, notifying, and skipping to next track.
    ///
    /// This implements the PRD §4.9 requirement: "log the error, show a user-visible
    /// message, and skip the track."
    ///
    /// Returns the next track if available, or None if queue is exhausted.
    /// The `on_error` callback is invoked with the error message for UI display.
    pub fn handle_track_error<F>(
        &mut self,
        error: impl Into<String>,
        mut on_error: F,
    ) -> Option<&QueueItem>
    where
        F: FnMut(&str),
    {
        let message = error.into();
        let track_info = self
            .queue
            .current()
            .map(|item| format!("{} - {}", item.track.artist, item.track.title))
            .unwrap_or_else(|| "unknown track".into());

        // Log the error
        tracing::warn!(
            track = %track_info,
            error = %message,
            "track playback failed; skipping to next"
        );

        // Notify UI via callback
        let user_message = format!("Error playing '{}': {}", track_info, message);
        on_error(&user_message);

        // Stop current audio and skip to next
        self.stop_audio();

        // Try to advance to next track
        if let Some(next) = self.queue.advance() {
            let next_id = next.id;
            self.state = PlayerState::Buffering { id: next_id };
            self.queue.current()
        } else {
            // No more tracks; go to stopped state
            self.state = PlayerState::Stopped;
            None
        }
    }

    /// Handle a track error and automatically start playing the next track.
    ///
    /// This variant also attempts to play the next track using the audio engine.
    pub fn handle_track_error_and_play<E, F>(
        &mut self,
        engine: &E,
        error: impl Into<String>,
        source_fn: impl Fn(&QueueItem) -> AudioSource,
        mut on_error: F,
    ) -> Option<&QueueItem>
    where
        E: AudioEngine,
        F: FnMut(&str),
    {
        let message = error.into();
        let track_info = self
            .queue
            .current()
            .map(|item| format!("{} - {}", item.track.artist, item.track.title))
            .unwrap_or_else(|| "unknown track".into());

        // Log the error
        tracing::warn!(
            track = %track_info,
            error = %message,
            "track playback failed; skipping to next"
        );

        // Notify UI via callback
        let user_message = format!("Error playing '{}': {}", track_info, message);
        on_error(&user_message);

        // Stop current audio
        self.stop_audio();

        // Try to advance to next track and play it
        if self.queue.advance().is_some() {
            let current = self.queue.current()?;
            let source = source_fn(current);
            self.play_with_audio(engine, source)
        } else {
            // No more tracks; go to stopped state
            self.state = PlayerState::Stopped;
            None
        }
    }

    fn stop_audio(&mut self) {
        if let Some(handle) = self.audio.take() {
            handle.stop();
        }
    }
}

#[cfg(test)]
mod tests {
    use tunez_core::{Track, TrackId};

    use super::*;

    fn track(title: &str) -> Track {
        Track {
            id: TrackId::new(title),
            provider_id: "test".into(),
            title: title.to_string(),
            artist: "artist".into(),
            album: None,
            duration_seconds: None,
            track_number: None,
        }
    }

    #[test]
    fn play_starts_first_track() {
        let mut player = Player::new();
        player.queue_mut().enqueue_back(track("one"));

        let current = player.play().expect("should play first track");
        assert_eq!(current.track.title, "one");
        assert!(matches!(player.state(), PlayerState::Playing { .. }));
    }

    #[test]
    fn pause_and_resume_transitions() {
        let mut player = Player::new();
        player.queue_mut().enqueue_back(track("one"));
        player.play();

        assert!(player.pause());
        assert!(matches!(player.state(), PlayerState::Paused { .. }));
        assert!(player.resume());
        assert!(matches!(player.state(), PlayerState::Playing { .. }));
    }

    #[test]
    fn skip_advances_queue_and_state() {
        let mut player = Player::new();
        player.queue_mut().enqueue_back(track("one"));
        player.queue_mut().enqueue_back(track("two"));
        player.play();

        let next = player.skip_next().expect("should move to next track");
        assert_eq!(next.track.title, "two");
        assert!(matches!(player.state(), PlayerState::Playing { .. }));
    }

    #[test]
    fn stop_clears_current_selection() {
        let mut player = Player::new();
        player.queue_mut().enqueue_back(track("one"));
        player.play();

        player.stop();
        assert!(player.current().is_none());
        assert!(matches!(player.state(), PlayerState::Stopped));
    }

    #[test]
    fn error_state_captures_current() {
        let mut player = Player::new();
        player.queue_mut().enqueue_back(track("one"));
        player.play();

        player.set_error("failed to decode");
        match player.state() {
            PlayerState::Error { id, message } => {
                assert!(id.is_some());
                assert_eq!(message, "failed to decode");
            }
            _ => panic!("expected error state"),
        }
    }

    #[test]
    fn play_with_audio_uses_engine() {
        let mut player = Player::new();
        player.queue_mut().enqueue_back(track("one"));
        let engine = tunez_audio::NullAudioEngine;
        let current = player
            .play_with_audio(&engine, AudioSource::Url("test".into()))
            .expect("should start with audio");
        assert_eq!(current.track.title, "one");
        assert!(matches!(player.state(), PlayerState::Playing { .. }));
    }

    #[test]
    fn handle_track_error_skips_to_next() {
        let mut player = Player::new();
        player.queue_mut().enqueue_back(track("one"));
        player.queue_mut().enqueue_back(track("two"));
        player.play();

        let mut error_messages = Vec::new();
        let next = player.handle_track_error("decode failed", |msg| {
            error_messages.push(msg.to_string());
        });

        assert!(next.is_some());
        assert_eq!(next.unwrap().track.title, "two");
        assert_eq!(error_messages.len(), 1);
        assert!(error_messages[0].contains("decode failed"));
        assert!(error_messages[0].contains("one")); // track title
    }

    #[test]
    fn handle_track_error_stops_at_end_of_queue() {
        let mut player = Player::new();
        player.queue_mut().enqueue_back(track("only"));
        player.play();

        let mut error_count = 0;
        let next = player.handle_track_error("error", |_| {
            error_count += 1;
        });

        assert!(next.is_none());
        assert_eq!(error_count, 1);
        assert!(matches!(player.state(), PlayerState::Stopped));
    }

    #[test]
    fn handle_track_error_does_not_panic_on_empty_queue() {
        let mut player = Player::new();

        let mut error_count = 0;
        let next = player.handle_track_error("error", |_| {
            error_count += 1;
        });

        assert!(next.is_none());
        // Callback should still be called even for unknown track
        assert_eq!(error_count, 1);
    }
}
