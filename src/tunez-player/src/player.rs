use crate::{Queue, QueueId, QueueItem};
use tunez_audio::{AudioEngine, AudioHandle, AudioSource};

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

#[derive(Debug, Default)]
pub struct Player {
    queue: Queue,
    state: PlayerState,
    audio: Option<AudioHandle>,
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
            Ok(handle) => {
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
}
