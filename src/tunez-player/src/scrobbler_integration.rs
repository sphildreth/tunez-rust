//! Scrobbler integration for the player.
//!
//! Provides a wrapper that integrates scrobbling with playback state,
//! ensuring scrobbler failures never interrupt playback.

use crate::{Player, PlayerState, QueueItem};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tunez_core::{
    PlaybackProgress, PlaybackState as ScrobblePlaybackState, ScrobbleEvent, Scrobbler,
};

/// Type alias for error callbacks.
pub type ErrorCallback = Box<dyn Fn(&str) + Send + Sync>;

/// Manages scrobbling for a player, ensuring failures don't interrupt playback.
pub struct ScrobblerManager {
    scrobbler: Option<Arc<dyn Scrobbler>>,
    player_name: String,
    device_id: Option<String>,
    tick_interval: Duration,
    last_tick: Option<Instant>,
    last_position: u64,
    /// Whether scrobbling is enabled for the current session
    enabled: bool,
    /// Callback for error notifications
    error_callback: Option<ErrorCallback>,
}

impl std::fmt::Debug for ScrobblerManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ScrobblerManager")
            .field("player_name", &self.player_name)
            .field("device_id", &self.device_id)
            .field("tick_interval", &self.tick_interval)
            .field("enabled", &self.enabled)
            .field(
                "scrobbler",
                &self.scrobbler.as_ref().map(|s| s.id().to_string()),
            )
            .finish()
    }
}

impl ScrobblerManager {
    /// Create a new scrobbler manager.
    ///
    /// # Arguments
    /// * `scrobbler` - Optional scrobbler implementation (None = disabled)
    /// * `player_name` - Name of the player (e.g., "Tunez")
    /// * `device_id` - Optional device identifier
    pub fn new(
        scrobbler: Option<Arc<dyn Scrobbler>>,
        player_name: impl Into<String>,
        device_id: Option<String>,
    ) -> Self {
        Self {
            scrobbler,
            player_name: player_name.into(),
            device_id,
            tick_interval: Duration::from_secs(1),
            last_tick: None,
            last_position: 0,
            enabled: true,
            error_callback: None,
        }
    }

    /// Set a callback for error notifications.
    pub fn set_error_callback<F>(&mut self, callback: F)
    where
        F: Fn(&str) + Send + Sync + 'static,
    {
        self.error_callback = Some(Box::new(callback));
    }

    /// Enable or disable scrobbling.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Check if scrobbling is enabled and configured.
    pub fn is_active(&self) -> bool {
        self.enabled && self.scrobbler.is_some()
    }

    /// Notify the scrobbler of a playback state transition.
    ///
    /// This should be called when:
    /// - Playback starts (Started)
    /// - Playback resumes after pause (Resumed)
    /// - Playback is paused (Paused)
    /// - Playback is stopped (Stopped)
    /// - Track ends naturally (Ended)
    pub fn on_state_change(&mut self, player: &Player, state: ScrobblePlaybackState) {
        if !self.is_active() {
            return;
        }

        if let Some(current) = player.current() {
            self.submit_event(current, state, self.last_position);
        }

        // Reset tick tracking on state changes
        if matches!(state, ScrobblePlaybackState::Started) {
            self.last_tick = Some(Instant::now());
            self.last_position = 0;
        }
    }

    /// Process a playback tick (called at ~1 second intervals during playback).
    ///
    /// This method:
    /// 1. Checks if enough time has passed since the last scrobble update
    /// 2. If so, submits a progress update to the scrobbler
    ///
    /// Returns true if a scrobble was submitted (or attempted).
    pub fn tick(&mut self, player: &Player, position_seconds: u64) -> bool {
        if !self.is_active() {
            return false;
        }

        // Only scrobble during active playback
        if !matches!(player.state(), PlayerState::Playing { .. }) {
            return false;
        }

        // Check if we should submit based on tick interval
        let now = Instant::now();
        let should_tick = match self.last_tick {
            Some(last) => now.duration_since(last) >= self.tick_interval,
            None => true,
        };

        if !should_tick {
            return false;
        }

        self.last_tick = Some(now);
        self.last_position = position_seconds;

        // Submit progress update (the scrobbler decides what to do with it)
        if let Some(current) = player.current() {
            // For periodic ticks during playback, we don't change state
            // The scrobbler will receive position updates to track progress
            self.submit_event(current, ScrobblePlaybackState::Started, position_seconds);
            return true;
        }

        false
    }

    /// Notify the scrobbler that a track has ended (reached its natural end).
    pub fn on_track_ended(&mut self, player: &Player) {
        if !self.is_active() {
            return;
        }

        if let Some(current) = player.current() {
            let duration = current.track.duration_seconds.unwrap_or(0) as u64;
            self.submit_event(current, ScrobblePlaybackState::Ended, duration);
        }
    }

    /// Submit a scrobble event, handling errors gracefully.
    fn submit_event(&self, item: &QueueItem, state: ScrobblePlaybackState, position: u64) {
        let Some(scrobbler) = &self.scrobbler else {
            return;
        };

        let event = ScrobbleEvent {
            track: item.track.clone(),
            progress: PlaybackProgress {
                position_seconds: position,
                duration_seconds: item.track.duration_seconds.map(|d| d as u64),
            },
            state,
            player_name: self.player_name.clone(),
            device_id: self.device_id.clone(),
        };

        // Submit, but never interrupt playback on failure
        if let Err(e) = scrobbler.submit(&event) {
            tracing::warn!(
                scrobbler_id = scrobbler.id(),
                error = %e,
                track = %item.track.title,
                "scrobble submission failed"
            );

            // Notify via callback (for UI indicator)
            if let Some(callback) = &self.error_callback {
                callback(&format!("Scrobble failed: {}", e));
            }
        }
    }

    /// Get the configured tick interval.
    pub fn tick_interval(&self) -> Duration {
        self.tick_interval
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Mutex;
    use tunez_core::{ScrobbleEvent, Scrobbler, ScrobblerError, ScrobblerResult, Track, TrackId};

    /// Mock scrobbler that records submissions
    struct MockScrobbler {
        submissions: Mutex<Vec<ScrobbleEvent>>,
        fail_count: AtomicUsize,
    }

    impl MockScrobbler {
        fn new() -> Self {
            Self {
                submissions: Mutex::new(Vec::new()),
                fail_count: AtomicUsize::new(0),
            }
        }

        fn set_fail_next(&self, count: usize) {
            self.fail_count.store(count, Ordering::SeqCst);
        }

        fn submissions(&self) -> Vec<ScrobbleEvent> {
            self.submissions.lock().unwrap().clone()
        }
    }

    impl Scrobbler for MockScrobbler {
        fn id(&self) -> &str {
            "mock"
        }

        fn submit(&self, event: &ScrobbleEvent) -> ScrobblerResult<()> {
            let fail = self.fail_count.load(Ordering::SeqCst);
            if fail > 0 {
                self.fail_count.fetch_sub(1, Ordering::SeqCst);
                return Err(ScrobblerError::Network {
                    message: "simulated failure".into(),
                });
            }
            self.submissions.lock().unwrap().push(event.clone());
            Ok(())
        }
    }

    fn test_track(title: &str) -> Track {
        Track {
            id: TrackId::new(title),
            provider_id: "test".into(),
            title: title.into(),
            artist: "Test Artist".into(),
            album: None,
            duration_seconds: Some(180),
            track_number: None,
        }
    }

    #[test]
    fn scrobbles_on_state_change() {
        let scrobbler = Arc::new(MockScrobbler::new());
        let mut manager =
            ScrobblerManager::new(Some(scrobbler.clone()), "Tunez", Some("test-device".into()));

        let mut player = Player::new();
        player.queue_mut().enqueue_back(test_track("Test Song"));
        player.play();

        manager.on_state_change(&player, ScrobblePlaybackState::Started);

        let submissions = scrobbler.submissions();
        assert_eq!(submissions.len(), 1);
        assert_eq!(submissions[0].state, ScrobblePlaybackState::Started);
    }

    #[test]
    fn scrobbler_failure_does_not_panic() {
        let scrobbler = Arc::new(MockScrobbler::new());
        scrobbler.set_fail_next(5);

        let mut manager =
            ScrobblerManager::new(Some(scrobbler.clone()), "Tunez", Some("test-device".into()));

        let mut player = Player::new();
        player.queue_mut().enqueue_back(test_track("Test Song"));
        player.play();

        // These should not panic even though scrobbler fails
        // on_state_change calls submit once
        manager.on_state_change(&player, ScrobblePlaybackState::Started);
        // After initial tick is tracked, these will be skipped due to tick interval
        // But tick immediately after on_state_change will call once more
        let _ticked = manager.tick(&player, 10);

        // We used at least 1 failure (from on_state_change)
        // tick may or may not trigger based on interval
        let remaining = scrobbler.fail_count.load(Ordering::SeqCst);
        // Should have called at least on_state_change (uses 1 failure)
        assert!(remaining < 5, "at least one call should have been made");
        // No panic is the main success criterion
    }

    #[test]
    fn disabled_scrobbler_does_not_submit() {
        let scrobbler = Arc::new(MockScrobbler::new());
        let mut manager =
            ScrobblerManager::new(Some(scrobbler.clone()), "Tunez", Some("test-device".into()));
        manager.set_enabled(false);

        let mut player = Player::new();
        player.queue_mut().enqueue_back(test_track("Test Song"));
        player.play();

        manager.on_state_change(&player, ScrobblePlaybackState::Started);

        let submissions = scrobbler.submissions();
        assert!(submissions.is_empty());
    }

    #[test]
    fn no_scrobbler_configured_is_safe() {
        let mut manager = ScrobblerManager::new(None, "Tunez", None);

        let mut player = Player::new();
        player.queue_mut().enqueue_back(test_track("Test Song"));
        player.play();

        // These should not panic with no scrobbler configured
        manager.on_state_change(&player, ScrobblePlaybackState::Started);
        manager.tick(&player, 10);
        manager.on_track_ended(&player);

        assert!(!manager.is_active());
    }

    #[test]
    fn error_callback_is_invoked_on_failure() {
        let scrobbler = Arc::new(MockScrobbler::new());
        scrobbler.set_fail_next(1);

        let error_count = Arc::new(AtomicUsize::new(0));
        let error_count_clone = error_count.clone();

        let mut manager =
            ScrobblerManager::new(Some(scrobbler.clone()), "Tunez", Some("test-device".into()));
        manager.set_error_callback(move |_msg| {
            error_count_clone.fetch_add(1, Ordering::SeqCst);
        });

        let mut player = Player::new();
        player.queue_mut().enqueue_back(test_track("Test Song"));
        player.play();

        manager.on_state_change(&player, ScrobblePlaybackState::Started);

        assert_eq!(error_count.load(Ordering::SeqCst), 1);
    }
}
