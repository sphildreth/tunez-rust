use crate::models::{
    Album, AlbumId, Page, PageCursor, PageRequest, Playlist, PlaylistId, StreamUrl, Track, TrackId,
};
use crate::provider::{
    BrowseKind, CollectionItem, Provider, ProviderCapabilities, ProviderError, TrackSearchFilters,
};
use thiserror::Error;

/// Expectations supplied by a provider implementation to run the shared contract suite.
#[derive(Debug, Clone)]
pub struct ProviderContractExpectations {
    /// The provider id that should be returned by all tracks/playlists.
    pub provider_id: String,
    /// Required search expectation; validates stable ids and metadata.
    pub search: SearchExpectation,
    /// Track id to validate stream URL resolution.
    pub stream_track_id: TrackId,
    /// Playlist expectations (only required if playlists capability is advertised).
    pub playlist: Option<PlaylistExpectation>,
}

/// Search expectation used to validate provider search behavior.
#[derive(Debug, Clone)]
pub struct SearchExpectation {
    /// Query text to send to the provider.
    pub query: String,
    /// Optional filters to apply (artist/album/year).
    pub filters: TrackSearchFilters,
    /// The first track id expected for the search query (deterministic ordering).
    pub expected_first_track_id: TrackId,
}

/// Playlist expectation used when the provider advertises playlist support.
#[derive(Debug, Clone)]
pub struct PlaylistExpectation {
    /// A known playlist id that should be returned by playlist listing/search.
    pub playlist_id: PlaylistId,
    /// Optional query that should find the playlist via search.
    pub search_query: Option<String>,
}

/// Errors surfaced by the provider contract test harness.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ProviderContractError {
    #[error("search returned no tracks for query: {query}")]
    EmptySearch { query: String },
    #[error("search returned wrong first track id: expected {expected:?}, got {actual:?}")]
    SearchWrongFirstTrack { expected: TrackId, actual: TrackId },
    #[error("search returned track from different provider: {actual}")]
    SearchProviderMismatch { actual: String },
    #[error("get_track returned mismatched id: expected {expected:?}, got {actual:?}")]
    TrackLookupMismatch { expected: TrackId, actual: TrackId },
    #[error("stream URL was empty for track {track_id:?}")]
    EmptyStreamUrl { track_id: TrackId },
    #[error("provider advertises playlists capability but no playlist expectation supplied")]
    MissingPlaylistExpectation,
    #[error("provider claims playlists support but did not return a playlist")]
    PlaylistEmpty,
    #[error(
        "provider claims playlists support but did not return expected playlist id {expected:?}"
    )]
    PlaylistMissingExpected { expected: PlaylistId },
    #[error("provider claims playlists support but search did not return expected playlist id {expected:?}")]
    PlaylistSearchMissing { expected: PlaylistId },
    #[error(
        "provider does not advertise playlists but list_playlists did not return NotSupported"
    )]
    PlaylistsNotSupportedExpected,
    #[error(
        "provider does not advertise playlists but search_playlists did not return NotSupported"
    )]
    PlaylistSearchNotSupportedExpected,
    #[error("provider error while running contract: {0}")]
    ProviderFailure(String),
}

/// Run the shared provider contract suite against a provider implementation.
///
/// Providers should call this from their crate-level tests with known fixtures that
/// exist in their test setup.
pub fn run_provider_contract<P: Provider>(
    provider: &P,
    expectations: &ProviderContractExpectations,
) -> Result<(), ProviderContractError> {
    verify_search(provider, expectations)?;
    verify_stream(provider, expectations)?;
    verify_playlists(provider, expectations)?;
    Ok(())
}

fn verify_search<P: Provider>(
    provider: &P,
    expectations: &ProviderContractExpectations,
) -> Result<(), ProviderContractError> {
    let caps = provider.capabilities();
    let page = provider
        .search_tracks(
            &expectations.search.query,
            expectations.search.filters.clone(),
            PageRequest::first_page(10),
        )
        .map_err(|e| ProviderContractError::ProviderFailure(e.to_string()))?;

    if page.items.is_empty() {
        return Err(ProviderContractError::EmptySearch {
            query: expectations.search.query.clone(),
        });
    }

    let first = &page.items[0];
    if first.id != expectations.search.expected_first_track_id {
        return Err(ProviderContractError::SearchWrongFirstTrack {
            expected: expectations.search.expected_first_track_id.clone(),
            actual: first.id.clone(),
        });
    }

    if first.provider_id != expectations.provider_id {
        return Err(ProviderContractError::SearchProviderMismatch {
            actual: first.provider_id.clone(),
        });
    }

    if caps.supports_offline_download() && first.title.trim().is_empty() {
        // Offline download implies local metadata; require a non-empty title.
        return Err(ProviderContractError::ProviderFailure(
            "empty track title returned".to_string(),
        ));
    }

    let track = provider
        .get_track(&first.id)
        .map_err(|e| ProviderContractError::ProviderFailure(e.to_string()))?;
    if track.id != first.id {
        return Err(ProviderContractError::TrackLookupMismatch {
            expected: first.id.clone(),
            actual: track.id.clone(),
        });
    }

    if track.provider_id != expectations.provider_id {
        return Err(ProviderContractError::SearchProviderMismatch {
            actual: track.provider_id,
        });
    }

    Ok(())
}

fn verify_stream<P: Provider>(
    provider: &P,
    expectations: &ProviderContractExpectations,
) -> Result<(), ProviderContractError> {
    let url = provider
        .get_stream_url(&expectations.stream_track_id)
        .map_err(|e| ProviderContractError::ProviderFailure(e.to_string()))?;
    if url.as_ref().is_empty() {
        return Err(ProviderContractError::EmptyStreamUrl {
            track_id: expectations.stream_track_id.clone(),
        });
    }
    Ok(())
}

fn verify_playlists<P: Provider>(
    provider: &P,
    expectations: &ProviderContractExpectations,
) -> Result<(), ProviderContractError> {
    let caps = provider.capabilities();
    if caps.supports_playlists() {
        let playlist_expectation = expectations
            .playlist
            .as_ref()
            .ok_or(ProviderContractError::MissingPlaylistExpectation)?;

        let listed = provider
            .list_playlists(PageRequest::first_page(25))
            .map_err(|e| ProviderContractError::ProviderFailure(e.to_string()))?;
        if listed.items.is_empty() {
            return Err(ProviderContractError::PlaylistEmpty);
        }
        let expected_playlist = listed
            .items
            .iter()
            .find(|p| p.id == playlist_expectation.playlist_id);
        if expected_playlist.is_none() {
            return Err(ProviderContractError::PlaylistMissingExpected {
                expected: playlist_expectation.playlist_id.clone(),
            });
        }

        if let Some(query) = &playlist_expectation.search_query {
            let searched = provider
                .search_playlists(query, PageRequest::first_page(25))
                .map_err(|e| ProviderContractError::ProviderFailure(e.to_string()))?;
            let found = searched
                .items
                .iter()
                .any(|p| p.id == playlist_expectation.playlist_id);
            if !found {
                return Err(ProviderContractError::PlaylistSearchMissing {
                    expected: playlist_expectation.playlist_id.clone(),
                });
            }
        }
    } else {
        match provider.list_playlists(PageRequest::first_page(1)) {
            Err(ProviderError::NotSupported { .. }) => {}
            _ => return Err(ProviderContractError::PlaylistsNotSupportedExpected),
        }

        match provider.search_playlists("irrelevant", PageRequest::first_page(1)) {
            Err(ProviderError::NotSupported { .. }) => {}
            _ => return Err(ProviderContractError::PlaylistSearchNotSupportedExpected),
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone)]
    struct FakeProvider {
        id: String,
        name: String,
        capabilities: ProviderCapabilities,
        tracks: Vec<Track>,
        playlists: Vec<Playlist>,
        playlist_tracks: Vec<Track>,
        stream_prefix: String,
    }

    impl FakeProvider {
        fn with_playlists() -> Self {
            let track = Track {
                id: TrackId::new("track-1"),
                provider_id: "fake".into(),
                title: "Track One".into(),
                artist: "Artist".into(),
                album: Some("Album".into()),
                duration_seconds: Some(180),
            };
            let playlist = Playlist {
                id: PlaylistId::new("pl-1"),
                provider_id: "fake".into(),
                name: "Favorites".into(),
                description: Some("Favs".into()),
                track_count: Some(1),
            };
            Self {
                id: "fake".into(),
                name: "Fake Provider".into(),
                capabilities: ProviderCapabilities {
                    playlists: true,
                    lyrics: false,
                    artwork: false,
                    favorites: false,
                    recently_played: false,
                    offline_download: true,
                },
                tracks: vec![track.clone()],
                playlists: vec![playlist],
                playlist_tracks: vec![track],
                stream_prefix: "file:///music/".into(),
            }
        }

        fn without_playlists() -> Self {
            let track = Track {
                id: TrackId::new("track-1"),
                provider_id: "fake".into(),
                title: "Track One".into(),
                artist: "Artist".into(),
                album: Some("Album".into()),
                duration_seconds: Some(180),
            };
            Self {
                id: "fake".into(),
                name: "Fake Provider".into(),
                capabilities: ProviderCapabilities {
                    playlists: false,
                    lyrics: false,
                    artwork: false,
                    favorites: false,
                    recently_played: false,
                    offline_download: true,
                },
                tracks: vec![track.clone()],
                playlists: Vec::new(),
                playlist_tracks: vec![track],
                stream_prefix: "file:///music/".into(),
            }
        }
    }

    impl Provider for FakeProvider {
        fn id(&self) -> &str {
            &self.id
        }

        fn name(&self) -> &str {
            &self.name
        }

        fn capabilities(&self) -> ProviderCapabilities {
            self.capabilities
        }

        fn search_tracks(
            &self,
            query: &str,
            _filters: TrackSearchFilters,
            _paging: PageRequest,
        ) -> Result<Page<Track>, ProviderError> {
            let lower = query.to_ascii_lowercase();
            let mut items: Vec<Track> = self
                .tracks
                .iter()
                .cloned()
                .filter(|t| t.title.to_ascii_lowercase().contains(&lower))
                .collect();
            if items.is_empty() {
                items = self.tracks.clone();
            }
            Ok(Page {
                items,
                next: Some(PageCursor("end".into())),
            })
        }

        fn browse(
            &self,
            _kind: BrowseKind,
            _paging: PageRequest,
        ) -> Result<Page<CollectionItem>, ProviderError> {
            Err(ProviderError::NotSupported {
                operation: "browse".into(),
            })
        }

        fn list_playlists(&self, _paging: PageRequest) -> Result<Page<Playlist>, ProviderError> {
            if self.capabilities.playlists {
                Ok(Page {
                    items: self.playlists.clone(),
                    next: None,
                })
            } else {
                Err(ProviderError::NotSupported {
                    operation: "list_playlists".into(),
                })
            }
        }

        fn search_playlists(
            &self,
            query: &str,
            _paging: PageRequest,
        ) -> Result<Page<Playlist>, ProviderError> {
            if !self.capabilities.playlists {
                return Err(ProviderError::NotSupported {
                    operation: "search_playlists".into(),
                });
            }
            let lower = query.to_ascii_lowercase();
            let items: Vec<Playlist> = self
                .playlists
                .iter()
                .cloned()
                .filter(|p| p.name.to_ascii_lowercase().contains(&lower))
                .collect();
            Ok(Page { items, next: None })
        }

        fn get_playlist(&self, playlist_id: &PlaylistId) -> Result<Playlist, ProviderError> {
            self.playlists
                .iter()
                .find(|p| &p.id == playlist_id)
                .cloned()
                .ok_or_else(|| ProviderError::NotFound {
                    entity: playlist_id.0.clone(),
                })
        }

        fn list_playlist_tracks(
            &self,
            playlist_id: &PlaylistId,
            _paging: PageRequest,
        ) -> Result<Page<Track>, ProviderError> {
            if self.playlists.iter().any(|p| &p.id == playlist_id) {
                Ok(Page {
                    items: self.playlist_tracks.clone(),
                    next: None,
                })
            } else {
                Err(ProviderError::NotFound {
                    entity: playlist_id.0.clone(),
                })
            }
        }

        fn get_album(&self, _album_id: &AlbumId) -> Result<Album, ProviderError> {
            Err(ProviderError::NotSupported {
                operation: "get_album".into(),
            })
        }

        fn list_album_tracks(
            &self,
            _album_id: &AlbumId,
            _paging: PageRequest,
        ) -> Result<Page<Track>, ProviderError> {
            Err(ProviderError::NotSupported {
                operation: "list_album_tracks".into(),
            })
        }

        fn get_track(&self, track_id: &TrackId) -> Result<Track, ProviderError> {
            self.tracks
                .iter()
                .find(|t| &t.id == track_id)
                .cloned()
                .ok_or_else(|| ProviderError::NotFound {
                    entity: track_id.0.clone(),
                })
        }

        fn get_stream_url(&self, track_id: &TrackId) -> Result<StreamUrl, ProviderError> {
            self.tracks
                .iter()
                .find(|t| &t.id == track_id)
                .map(|_| StreamUrl::new(format!("{}{}", self.stream_prefix, track_id.0)))
                .ok_or_else(|| ProviderError::NotFound {
                    entity: track_id.0.clone(),
                })
        }
    }

    #[test]
    fn contract_passes_with_playlists() {
        let provider = FakeProvider::with_playlists();
        let expectations = ProviderContractExpectations {
            provider_id: "fake".into(),
            search: SearchExpectation {
                query: "track".into(),
                filters: TrackSearchFilters::default(),
                expected_first_track_id: TrackId::new("track-1"),
            },
            stream_track_id: TrackId::new("track-1"),
            playlist: Some(PlaylistExpectation {
                playlist_id: PlaylistId::new("pl-1"),
                search_query: Some("fav".into()),
            }),
        };

        let result = run_provider_contract(&provider, &expectations);
        assert!(result.is_ok(), "expected contract to pass: {result:?}");
    }

    #[test]
    fn contract_validates_not_supported_when_playlists_disabled() {
        let provider = FakeProvider::without_playlists();
        let expectations = ProviderContractExpectations {
            provider_id: "fake".into(),
            search: SearchExpectation {
                query: "track".into(),
                filters: TrackSearchFilters::default(),
                expected_first_track_id: TrackId::new("track-1"),
            },
            stream_track_id: TrackId::new("track-1"),
            playlist: None,
        };

        let result = run_provider_contract(&provider, &expectations);
        assert!(result.is_ok(), "expected contract to pass: {result:?}");
    }

    #[test]
    fn contract_fails_when_stream_url_empty() {
        let mut provider = FakeProvider::with_playlists();
        provider.stream_prefix.clear();
        let expectations = ProviderContractExpectations {
            provider_id: "fake".into(),
            search: SearchExpectation {
                query: "track".into(),
                filters: TrackSearchFilters::default(),
                expected_first_track_id: TrackId::new("track-1"),
            },
            stream_track_id: TrackId::new("track-1"),
            playlist: Some(PlaylistExpectation {
                playlist_id: PlaylistId::new("pl-1"),
                search_query: Some("fav".into()),
            }),
        };

        let result = run_provider_contract(&provider, &expectations);
        assert!(matches!(
            result,
            Err(ProviderContractError::EmptyStreamUrl { .. })
        ));
    }
}
