//! Plugin protocol types for Tunez external plugins.
//!
//! This module defines the JSON-based request/response protocol used for
//! communication with external plugin processes.

use serde::{Deserialize, Serialize};
use tunez_core::models::{
    Album, AlbumId, Page, PageRequest, Playlist, PlaylistId, StreamUrl, Track, TrackId,
};
use tunez_core::provider::{BrowseKind, CollectionItem, ProviderCapabilities, TrackSearchFilters};

/// Protocol version for compatibility checking.
pub const PROTOCOL_VERSION: u32 = 1;

/// Request sent from Tunez to a plugin process.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginRequest {
    /// Unique request ID for correlation.
    pub id: u64,
    /// The method to invoke on the plugin.
    pub method: PluginMethod,
}

/// Response from a plugin process to Tunez.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginResponse {
    /// Request ID this response correlates to.
    pub id: u64,
    /// The result of the method invocation.
    pub result: PluginResult,
}

/// Methods that can be invoked on a plugin.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "params")]
pub enum PluginMethod {
    /// Initialize the plugin and return its metadata.
    Initialize,
    /// Get provider capabilities.
    Capabilities,
    /// Search for tracks.
    SearchTracks {
        query: String,
        filters: TrackSearchFilters,
        paging: PageRequest,
    },
    /// Browse library items.
    Browse {
        kind: BrowseKind,
        paging: PageRequest,
    },
    /// List playlists.
    ListPlaylists { paging: PageRequest },
    /// Search playlists.
    SearchPlaylists { query: String, paging: PageRequest },
    /// Get a specific playlist.
    GetPlaylist { playlist_id: PlaylistId },
    /// List tracks in a playlist.
    ListPlaylistTracks {
        playlist_id: PlaylistId,
        paging: PageRequest,
    },
    /// Get a specific album.
    GetAlbum { album_id: AlbumId },
    /// List tracks in an album.
    ListAlbumTracks {
        album_id: AlbumId,
        paging: PageRequest,
    },
    /// Get a specific track.
    GetTrack { track_id: TrackId },
    /// Get the stream URL for a track.
    GetStreamUrl { track_id: TrackId },
    /// Shutdown the plugin gracefully.
    Shutdown,
}

/// Result of a plugin method invocation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status")]
pub enum PluginResult {
    /// Successful initialization.
    Initialized(PluginInfo),
    /// Capabilities response.
    Capabilities(ProviderCapabilities),
    /// Track search results.
    Tracks(Page<Track>),
    /// Browse results (collection items).
    CollectionItems(Page<CollectionItem>),
    /// Playlist list results.
    Playlists(Page<Playlist>),
    /// Single playlist.
    Playlist(Playlist),
    /// Single album.
    Album(Album),
    /// Single track.
    Track(Track),
    /// Stream URL.
    StreamUrl(StreamUrl),
    /// Shutdown acknowledged.
    ShutdownAck,
    /// Error response.
    Error(PluginError),
}

/// Plugin initialization info returned after Initialize.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginInfo {
    /// Plugin's unique identifier.
    pub id: String,
    /// Human-friendly name.
    pub name: String,
    /// Plugin version (semantic versioning).
    pub version: String,
    /// Protocol version the plugin supports.
    pub protocol_version: u32,
}

/// Error returned by a plugin.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginError {
    /// Error category for consistent handling.
    pub kind: PluginErrorKind,
    /// Human-readable error message.
    pub message: String,
}

/// Categories of plugin errors, matching ProviderError categories.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginErrorKind {
    /// Network/connectivity error.
    Network,
    /// Authentication/authorization error.
    Authentication,
    /// Entity not found.
    NotFound,
    /// Operation not supported by this plugin.
    NotSupported,
    /// Protocol version mismatch.
    ProtocolMismatch,
    /// Internal plugin error.
    Internal,
}

impl From<PluginError> for tunez_core::provider::ProviderError {
    fn from(err: PluginError) -> Self {
        match err.kind {
            PluginErrorKind::Network => Self::NetworkError {
                message: err.message,
            },
            PluginErrorKind::Authentication => Self::AuthenticationError {
                message: err.message,
            },
            PluginErrorKind::NotFound => Self::NotFound {
                entity: err.message,
            },
            PluginErrorKind::NotSupported => Self::NotSupported {
                operation: err.message,
            },
            PluginErrorKind::ProtocolMismatch | PluginErrorKind::Internal => Self::Other {
                message: err.message,
            },
        }
    }
}

impl From<tunez_core::provider::ProviderError> for PluginError {
    fn from(err: tunez_core::provider::ProviderError) -> Self {
        use tunez_core::provider::ProviderError;
        match err {
            ProviderError::NetworkError { message } => Self {
                kind: PluginErrorKind::Network,
                message,
            },
            ProviderError::AuthenticationError { message } => Self {
                kind: PluginErrorKind::Authentication,
                message,
            },
            ProviderError::NotFound { entity } => Self {
                kind: PluginErrorKind::NotFound,
                message: entity,
            },
            ProviderError::NotSupported { operation } => Self {
                kind: PluginErrorKind::NotSupported,
                message: operation,
            },
            ProviderError::Other { message } => Self {
                kind: PluginErrorKind::Internal,
                message,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_serializes_correctly() {
        let req = PluginRequest {
            id: 1,
            method: PluginMethod::Initialize,
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("\"id\":1"));
        assert!(json.contains("\"Initialize\""));
    }

    #[test]
    fn response_deserializes_correctly() {
        let json = r#"{"id":1,"result":{"status":"Initialized","id":"test","name":"Test Plugin","version":"1.0.0","protocol_version":1}}"#;
        let resp: PluginResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.id, 1);
        match resp.result {
            PluginResult::Initialized(info) => {
                assert_eq!(info.id, "test");
                assert_eq!(info.name, "Test Plugin");
            }
            _ => panic!("expected Initialized result"),
        }
    }

    #[test]
    fn error_converts_to_provider_error() {
        let err = PluginError {
            kind: PluginErrorKind::NotFound,
            message: "track-123".to_string(),
        };
        let provider_err: tunez_core::provider::ProviderError = err.into();
        match provider_err {
            tunez_core::provider::ProviderError::NotFound { entity } => {
                assert_eq!(entity, "track-123");
            }
            _ => panic!("expected NotFound"),
        }
    }
}
