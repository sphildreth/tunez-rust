//! Plugin provider adapter that implements the Provider trait for external plugins.

use crate::host::{ExecPluginHost, PluginConfig, PluginHostError};
use crate::protocol::{PluginMethod, PluginResult};
use std::sync::RwLock;
use tunez_core::models::{
    Album, AlbumId, Page, PageRequest, Playlist, PlaylistId, StreamUrl, Track, TrackId,
};
use tunez_core::provider::{
    BrowseKind, CollectionItem, Provider, ProviderCapabilities, ProviderError, ProviderResult,
    TrackSearchFilters,
};

/// A provider backed by an external plugin process.
///
/// This adapter wraps an `ExecPluginHost` and implements the `Provider` trait,
/// translating method calls to plugin requests and responses.
pub struct ExecPluginProvider {
    host: ExecPluginHost,
    id: String,
    name: String,
    capabilities: RwLock<Option<ProviderCapabilities>>,
}

impl ExecPluginProvider {
    /// Create a new plugin provider from configuration.
    pub fn new(config: PluginConfig) -> Result<Self, PluginHostError> {
        let host = ExecPluginHost::new(config);
        let info = host.start()?;

        Ok(Self {
            host,
            id: info.id,
            name: info.name,
            capabilities: RwLock::new(None),
        })
    }

    /// Create a plugin provider with a custom ID (useful for configuration-driven naming).
    pub fn with_id(config: PluginConfig, id: String) -> Result<Self, PluginHostError> {
        let host = ExecPluginHost::new(config);
        let info = host.start()?;

        Ok(Self {
            host,
            id,
            name: info.name,
            capabilities: RwLock::new(None),
        })
    }

    /// Stop the underlying plugin process.
    pub fn stop(&self) -> Result<(), PluginHostError> {
        self.host.stop()
    }

    /// Check if the plugin is running.
    pub fn is_running(&self) -> bool {
        self.host.is_running()
    }

    fn map_host_error(err: PluginHostError) -> ProviderError {
        match err {
            PluginHostError::PluginError(msg) => ProviderError::Other { message: msg },
            PluginHostError::ProcessTerminated => ProviderError::NetworkError {
                message: "plugin process terminated".to_string(),
            },
            other => ProviderError::Other {
                message: other.to_string(),
            },
        }
    }
}

impl Provider for ExecPluginProvider {
    fn id(&self) -> &str {
        &self.id
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn capabilities(&self) -> ProviderCapabilities {
        // Return cached capabilities if available
        {
            let guard = self.capabilities.read().unwrap();
            if let Some(caps) = *guard {
                return caps;
            }
        }

        // Fetch from plugin
        match self.host.send_request(PluginMethod::Capabilities) {
            Ok(PluginResult::Capabilities(caps)) => {
                let mut guard = self.capabilities.write().unwrap();
                *guard = Some(caps);
                caps
            }
            _ => {
                // Return conservative defaults on failure
                tracing::warn!(
                    provider_id = %self.id,
                    "Failed to fetch capabilities from plugin, using defaults"
                );
                ProviderCapabilities::default()
            }
        }
    }

    fn search_tracks(
        &self,
        query: &str,
        filters: TrackSearchFilters,
        paging: PageRequest,
    ) -> ProviderResult<Page<Track>> {
        let result = self
            .host
            .send_request(PluginMethod::SearchTracks {
                query: query.to_string(),
                filters,
                paging,
            })
            .map_err(Self::map_host_error)?;

        match result {
            PluginResult::Tracks(page) => Ok(page),
            PluginResult::Error(err) => Err(err.into()),
            _ => Err(ProviderError::Other {
                message: "unexpected response type".to_string(),
            }),
        }
    }

    fn browse(
        &self,
        kind: BrowseKind,
        paging: PageRequest,
    ) -> ProviderResult<Page<CollectionItem>> {
        let result = self
            .host
            .send_request(PluginMethod::Browse { kind, paging })
            .map_err(Self::map_host_error)?;

        match result {
            PluginResult::CollectionItems(page) => Ok(page),
            PluginResult::Error(err) => Err(err.into()),
            _ => Err(ProviderError::Other {
                message: "unexpected response type".to_string(),
            }),
        }
    }

    fn list_playlists(&self, paging: PageRequest) -> ProviderResult<Page<Playlist>> {
        let result = self
            .host
            .send_request(PluginMethod::ListPlaylists { paging })
            .map_err(Self::map_host_error)?;

        match result {
            PluginResult::Playlists(page) => Ok(page),
            PluginResult::Error(err) => Err(err.into()),
            _ => Err(ProviderError::Other {
                message: "unexpected response type".to_string(),
            }),
        }
    }

    fn search_playlists(&self, query: &str, paging: PageRequest) -> ProviderResult<Page<Playlist>> {
        let result = self
            .host
            .send_request(PluginMethod::SearchPlaylists {
                query: query.to_string(),
                paging,
            })
            .map_err(Self::map_host_error)?;

        match result {
            PluginResult::Playlists(page) => Ok(page),
            PluginResult::Error(err) => Err(err.into()),
            _ => Err(ProviderError::Other {
                message: "unexpected response type".to_string(),
            }),
        }
    }

    fn get_playlist(&self, playlist_id: &PlaylistId) -> ProviderResult<Playlist> {
        let result = self
            .host
            .send_request(PluginMethod::GetPlaylist {
                playlist_id: playlist_id.clone(),
            })
            .map_err(Self::map_host_error)?;

        match result {
            PluginResult::Playlist(playlist) => Ok(playlist),
            PluginResult::Error(err) => Err(err.into()),
            _ => Err(ProviderError::Other {
                message: "unexpected response type".to_string(),
            }),
        }
    }

    fn list_playlist_tracks(
        &self,
        playlist_id: &PlaylistId,
        paging: PageRequest,
    ) -> ProviderResult<Page<Track>> {
        let result = self
            .host
            .send_request(PluginMethod::ListPlaylistTracks {
                playlist_id: playlist_id.clone(),
                paging,
            })
            .map_err(Self::map_host_error)?;

        match result {
            PluginResult::Tracks(page) => Ok(page),
            PluginResult::Error(err) => Err(err.into()),
            _ => Err(ProviderError::Other {
                message: "unexpected response type".to_string(),
            }),
        }
    }

    fn get_album(&self, album_id: &AlbumId) -> ProviderResult<Album> {
        let result = self
            .host
            .send_request(PluginMethod::GetAlbum {
                album_id: album_id.clone(),
            })
            .map_err(Self::map_host_error)?;

        match result {
            PluginResult::Album(album) => Ok(album),
            PluginResult::Error(err) => Err(err.into()),
            _ => Err(ProviderError::Other {
                message: "unexpected response type".to_string(),
            }),
        }
    }

    fn list_album_tracks(
        &self,
        album_id: &AlbumId,
        paging: PageRequest,
    ) -> ProviderResult<Page<Track>> {
        let result = self
            .host
            .send_request(PluginMethod::ListAlbumTracks {
                album_id: album_id.clone(),
                paging,
            })
            .map_err(Self::map_host_error)?;

        match result {
            PluginResult::Tracks(page) => Ok(page),
            PluginResult::Error(err) => Err(err.into()),
            _ => Err(ProviderError::Other {
                message: "unexpected response type".to_string(),
            }),
        }
    }

    fn get_track(&self, track_id: &TrackId) -> ProviderResult<Track> {
        let result = self
            .host
            .send_request(PluginMethod::GetTrack {
                track_id: track_id.clone(),
            })
            .map_err(Self::map_host_error)?;

        match result {
            PluginResult::Track(track) => Ok(track),
            PluginResult::Error(err) => Err(err.into()),
            _ => Err(ProviderError::Other {
                message: "unexpected response type".to_string(),
            }),
        }
    }

    fn get_stream_url(&self, track_id: &TrackId) -> ProviderResult<StreamUrl> {
        let result = self
            .host
            .send_request(PluginMethod::GetStreamUrl {
                track_id: track_id.clone(),
            })
            .map_err(Self::map_host_error)?;

        match result {
            PluginResult::StreamUrl(url) => Ok(url),
            PluginResult::Error(err) => Err(err.into()),
            _ => Err(ProviderError::Other {
                message: "unexpected response type".to_string(),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_host_error_converts_correctly() {
        let err = PluginHostError::PluginError("test error".to_string());
        let provider_err = ExecPluginProvider::map_host_error(err);
        match provider_err {
            ProviderError::Other { message } => assert_eq!(message, "test error"),
            _ => panic!("expected Other error"),
        }
    }

    #[test]
    fn map_terminated_to_network_error() {
        let err = PluginHostError::ProcessTerminated;
        let provider_err = ExecPluginProvider::map_host_error(err);
        match provider_err {
            ProviderError::NetworkError { message } => {
                assert!(message.contains("terminated"));
            }
            _ => panic!("expected NetworkError"),
        }
    }
}
