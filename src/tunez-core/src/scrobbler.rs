use crate::models::{PageRequest, Track};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Playback states surfaced to Scrobblers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlaybackState {
    Started,
    Resumed,
    Paused,
    Stopped,
    Ended,
}

/// Per-second (or similar cadence) playback telemetry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlaybackProgress {
    pub position_seconds: u64,
    pub duration_seconds: Option<u64>,
}

/// Scrobbler-facing event payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScrobbleEvent {
    pub track: Track,
    pub progress: PlaybackProgress,
    pub state: PlaybackState,
    /// Stable player identifier (e.g., "Tunez").
    pub player_name: String,
    /// Optional device identifier for the current host.
    pub device_id: Option<String>,
}

#[derive(Debug, Error)]
pub enum ScrobblerError {
    #[error("scrobbling is not configured")]
    NotConfigured,
    #[error("network error: {message}")]
    Network { message: String },
    #[error("authentication error: {message}")]
    Authentication { message: String },
    #[error("rate limited: {message}")]
    RateLimited { message: String },
    #[error("{message}")]
    Other { message: String },
}

pub type ScrobblerResult<T> = Result<T, ScrobblerError>;

/// Scrobbler interface (Phase 1).
pub trait Scrobbler: Send + Sync {
    /// Stable scrobbler identifier (e.g., "listenbrainz").
    fn id(&self) -> &str;

    /// Advertised default polling cadence (seconds). Core MAY use this to
    /// adjust tick frequency; it defaults to 1s in Phase 1.
    fn desired_tick(&self) -> std::time::Duration {
        std::time::Duration::from_secs(1)
    }

    /// Called when playback state/progress changes.
    fn submit(&self, event: &ScrobbleEvent) -> ScrobblerResult<()>;

    /// Optional hook to allow the scrobbler to request historical replay or
    /// catch-up after reconnect. Core may ignore if unsupported.
    fn backfill(
        &self,
        _since_seconds: Option<u64>,
        _page: Option<PageRequest>,
    ) -> ScrobblerResult<()> {
        Ok(())
    }
}
