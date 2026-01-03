use crate::models::{
    Album, AlbumId, Page, PageRequest, Playlist, PlaylistId, StreamUrl, Track, TrackId,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Capability flags describing optional provider features.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ProviderCapabilities {
    pub playlists: bool,
    pub lyrics: bool,
    pub artwork: bool,
    pub favorites: bool,
    pub recently_played: bool,
    pub offline_download: bool,
}

impl ProviderCapabilities {
    pub fn supports_playlists(&self) -> bool {
        self.playlists
    }

    pub fn supports_lyrics(&self) -> bool {
        self.lyrics
    }

    pub fn supports_offline_download(&self) -> bool {
        self.offline_download
    }
}

/// Common categories of provider failures surfaced to the core/UI.
#[derive(Debug, Error)]
pub enum ProviderError {
    #[error("network error: {message}")]
    NetworkError { message: String },
    #[error("authentication error: {message}")]
    AuthenticationError { message: String },
    #[error("entity not found: {entity}")]
    NotFound { entity: String },
    #[error("operation not supported: {operation}")]
    NotSupported { operation: String },
    #[error("{message}")]
    Other { message: String },
}

pub type ProviderResult<T> = Result<T, ProviderError>;

/// Track search filters (optional).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrackSearchFilters {
    pub artist: Option<String>,
    pub album: Option<String>,
    pub year: Option<u32>,
}

/// Provider interface (Phase 1).
///
/// Providers return **stream URLs only**; playback is handled by the Tunez core.
pub trait Provider: Send + Sync {
    /// Stable provider identifier (e.g., "filesystem" or "melodee").
    fn id(&self) -> &str;

    /// Human-friendly provider name.
    fn name(&self) -> &str;

    /// Advertised capabilities.
    fn capabilities(&self) -> ProviderCapabilities;

    fn search_tracks(
        &self,
        query: &str,
        filters: TrackSearchFilters,
        paging: PageRequest,
    ) -> ProviderResult<Page<Track>>;

    fn browse(&self, kind: BrowseKind, paging: PageRequest)
        -> ProviderResult<Page<CollectionItem>>;

    fn list_playlists(&self, paging: PageRequest) -> ProviderResult<Page<Playlist>>;

    fn search_playlists(&self, query: &str, paging: PageRequest) -> ProviderResult<Page<Playlist>>;

    fn get_playlist(&self, playlist_id: &PlaylistId) -> ProviderResult<Playlist>;

    fn list_playlist_tracks(
        &self,
        playlist_id: &PlaylistId,
        paging: PageRequest,
    ) -> ProviderResult<Page<Track>>;

    fn get_album(&self, album_id: &AlbumId) -> ProviderResult<Album>;

    fn list_album_tracks(
        &self,
        album_id: &AlbumId,
        paging: PageRequest,
    ) -> ProviderResult<Page<Track>>;

    fn get_track(&self, track_id: &TrackId) -> ProviderResult<Track>;

    /// Returns a playable stream URL for the given track.
    fn get_stream_url(&self, track_id: &TrackId) -> ProviderResult<StreamUrl>;

    /// Returns the lyrics for the given track.
    fn get_lyrics(&self, _track_id: &TrackId) -> ProviderResult<String> {
        Err(ProviderError::NotSupported {
            operation: "get_lyrics".into(),
        })
    }
}


/// Browse kinds supported by the core UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BrowseKind {
    Artists,
    Albums,
    Playlists,
    Genres,
}

/// Items returned from browse views.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CollectionItem {
    Album(Album),
    Playlist(Playlist),
    /// Artist name only; provider can lazily fetch albums/tracks.
    Artist {
        name: String,
        provider_id: String,
    },
    Genre {
        name: String,
        provider_id: String,
    },
}
