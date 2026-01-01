mod mapping;
pub mod models;

use futures::executor::block_on;
use mapping::{map_album, map_playlist, map_track};
use reqwest::Client;
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use tokio::sync::RwLock;
use tunez_core::models::{
    Album, AlbumId, Page, PageRequest, Playlist, PlaylistId, StreamUrl, Track, TrackId,
};
use tunez_core::provider::{
    BrowseKind, CollectionItem, Provider, ProviderCapabilities, ProviderError, ProviderResult,
    TrackSearchFilters,
};
use url::Url;

#[derive(Clone)]
pub struct MelodeeConfig {
    pub base_url: String,
    pub access_token: Option<String>,
}

#[derive(Debug, Error)]
pub enum MelodeeAuthError {
    #[error("authentication error: {0}")]
    Auth(String),
}

#[derive(Clone)]
pub struct MelodeeProvider {
    id: String,
    name: String,
    client: Client,
    base_url: Url,
    access_token: Arc<RwLock<Option<String>>>,
}

impl MelodeeProvider {
    pub fn new(config: MelodeeConfig) -> Result<Self, ProviderError> {
        let base_url = Url::parse(&config.base_url).map_err(|e| ProviderError::Other {
            message: format!("invalid base_url: {e}"),
        })?;
        let client = Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(20))
            .build()
            .map_err(|e| ProviderError::Other {
                message: e.to_string(),
            })?;
        Ok(Self {
            id: "melodee".into(),
            name: "Melodee".into(),
            client,
            base_url,
            access_token: Arc::new(RwLock::new(config.access_token)),
        })
    }

    fn auth_header(&self) -> Option<String> {
        block_on(async { self.access_token.read().await.clone() })
    }

    fn capabilities() -> ProviderCapabilities {
        ProviderCapabilities {
            playlists: true,
            lyrics: true,
            artwork: true,
            favorites: false,
            recently_played: false,
            offline_download: false,
        }
    }
}

impl Provider for MelodeeProvider {
    fn id(&self) -> &str {
        &self.id
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn capabilities(&self) -> ProviderCapabilities {
        Self::capabilities()
    }

    fn search_tracks(
        &self,
        query: &str,
        _filters: TrackSearchFilters,
        paging: PageRequest,
    ) -> ProviderResult<Page<Track>> {
        let url = self
            .base_url
            .join(&format!("api/v1/search/songs?q={}", query))
            .map_err(|e| ProviderError::Other {
                message: e.to_string(),
            })?;
        let resp = futures::executor::block_on(async {
            self.client
                .get(url)
                .query(&[
                    ("page", paging.offset / paging.limit),
                    ("pageSize", paging.limit),
                ])
                .bearer_auth(self.auth_header().unwrap_or_default())
                .send()
                .await
        })
        .map_err(|e| ProviderError::NetworkError {
            message: e.to_string(),
        })?;
        if resp.status() == reqwest::StatusCode::UNAUTHORIZED {
            return Err(ProviderError::AuthenticationError {
                message: "unauthorized".into(),
            });
        }
        let body: models::SongPagedResponse = futures::executor::block_on(async {
            resp.json().await
        })
        .map_err(|e| ProviderError::Other {
            message: e.to_string(),
        })?;
        let items: Vec<Track> = body
            .data
            .into_iter()
            .map(|song| map_track(&song, &self.id))
            .collect();
        Ok(Page { items, next: None })
    }

    fn browse(
        &self,
        kind: BrowseKind,
        paging: PageRequest,
    ) -> ProviderResult<Page<CollectionItem>> {
        match kind {
            BrowseKind::Artists | BrowseKind::Genres => Err(ProviderError::NotSupported {
                operation: "browse".into(),
            }),
            BrowseKind::Albums => {
                let url =
                    self.base_url
                        .join("api/v1/albums")
                        .map_err(|e| ProviderError::Other {
                            message: e.to_string(),
                        })?;
                let resp = futures::executor::block_on(async {
                    self.client
                        .get(url)
                        .query(&[
                            ("page", paging.offset / paging.limit),
                            ("pageSize", paging.limit),
                        ])
                        .bearer_auth(self.auth_header().unwrap_or_default())
                        .send()
                        .await
                })
                .map_err(|e| ProviderError::NetworkError {
                    message: e.to_string(),
                })?;
                if resp.status() == reqwest::StatusCode::UNAUTHORIZED {
                    return Err(ProviderError::AuthenticationError {
                        message: "unauthorized".into(),
                    });
                }
                let body: models::AlbumPagedResponse = futures::executor::block_on(async {
                    resp.json().await
                })
                .map_err(|e| ProviderError::Other {
                    message: e.to_string(),
                })?;
                let items = body
                    .data
                    .into_iter()
                    .map(|a| CollectionItem::Album(map_album(&a, &self.id)))
                    .collect();
                Ok(Page { items, next: None })
            }
            BrowseKind::Playlists => {
                let url = self.base_url.join("api/v1/user/playlists").map_err(|e| {
                    ProviderError::Other {
                        message: e.to_string(),
                    }
                })?;
                let resp = futures::executor::block_on(async {
                    self.client
                        .get(url)
                        .query(&[
                            ("page", paging.offset / paging.limit),
                            ("limit", paging.limit),
                        ])
                        .bearer_auth(self.auth_header().unwrap_or_default())
                        .send()
                        .await
                })
                .map_err(|e| ProviderError::NetworkError {
                    message: e.to_string(),
                })?;
                if resp.status() == reqwest::StatusCode::UNAUTHORIZED {
                    return Err(ProviderError::AuthenticationError {
                        message: "unauthorized".into(),
                    });
                }
                let body: models::PlaylistPagedResponse = futures::executor::block_on(async {
                    resp.json().await
                })
                .map_err(|e| ProviderError::Other {
                    message: e.to_string(),
                })?;
                let items = body
                    .data
                    .into_iter()
                    .map(|p| CollectionItem::Playlist(map_playlist(&p, &self.id)))
                    .collect();
                Ok(Page { items, next: None })
            }
        }
    }

    fn list_playlists(&self, paging: PageRequest) -> ProviderResult<Page<Playlist>> {
        let url =
            self.base_url
                .join("api/v1/user/playlists")
                .map_err(|e| ProviderError::Other {
                    message: e.to_string(),
                })?;
        let resp = futures::executor::block_on(async {
            self.client
                .get(url)
                .query(&[
                    ("page", paging.offset / paging.limit),
                    ("limit", paging.limit),
                ])
                .bearer_auth(self.auth_header().unwrap_or_default())
                .send()
                .await
        })
        .map_err(|e| ProviderError::NetworkError {
            message: e.to_string(),
        })?;
        if resp.status() == reqwest::StatusCode::UNAUTHORIZED {
            return Err(ProviderError::AuthenticationError {
                message: "unauthorized".into(),
            });
        }
        let body: models::PlaylistPagedResponse = futures::executor::block_on(async {
            resp.json().await
        })
        .map_err(|e| ProviderError::Other {
            message: e.to_string(),
        })?;
        let items = body
            .data
            .into_iter()
            .map(|p| map_playlist(&p, &self.id))
            .collect();
        Ok(Page { items, next: None })
    }

    fn search_playlists(&self, query: &str, paging: PageRequest) -> ProviderResult<Page<Playlist>> {
        let page = self.list_playlists(paging)?;
        let filtered = page
            .items
            .into_iter()
            .filter(|p| {
                p.name
                    .to_ascii_lowercase()
                    .contains(&query.to_ascii_lowercase())
            })
            .collect();
        Ok(Page {
            items: filtered,
            next: None,
        })
    }

    fn get_playlist(&self, playlist_id: &PlaylistId) -> ProviderResult<Playlist> {
        let url = self
            .base_url
            .join(&format!("api/v1/playlists/{}", playlist_id.0))
            .map_err(|e| ProviderError::Other {
                message: e.to_string(),
            })?;
        let resp = futures::executor::block_on(async {
            self.client
                .get(url)
                .bearer_auth(self.auth_header().unwrap_or_default())
                .send()
                .await
        })
        .map_err(|e| ProviderError::NetworkError {
            message: e.to_string(),
        })?;
        match resp.status() {
            reqwest::StatusCode::UNAUTHORIZED => {
                return Err(ProviderError::AuthenticationError {
                    message: "unauthorized".into(),
                })
            }
            reqwest::StatusCode::NOT_FOUND => {
                return Err(ProviderError::NotFound {
                    entity: playlist_id.0.clone(),
                })
            }
            _ => {}
        }
        let playlist: models::Playlist = futures::executor::block_on(async { resp.json().await })
            .map_err(|e| ProviderError::Other {
            message: e.to_string(),
        })?;
        Ok(map_playlist(&playlist, &self.id))
    }

    fn list_playlist_tracks(
        &self,
        playlist_id: &PlaylistId,
        paging: PageRequest,
    ) -> ProviderResult<Page<Track>> {
        let url = self
            .base_url
            .join(&format!("api/v1/playlists/{}/songs", playlist_id.0))
            .map_err(|e| ProviderError::Other {
                message: e.to_string(),
            })?;
        let resp = futures::executor::block_on(async {
            self.client
                .get(url)
                .query(&[
                    ("page", paging.offset / paging.limit),
                    ("pageSize", paging.limit),
                ])
                .bearer_auth(self.auth_header().unwrap_or_default())
                .send()
                .await
        })
        .map_err(|e| ProviderError::NetworkError {
            message: e.to_string(),
        })?;
        match resp.status() {
            reqwest::StatusCode::UNAUTHORIZED => {
                return Err(ProviderError::AuthenticationError {
                    message: "unauthorized".into(),
                })
            }
            reqwest::StatusCode::NOT_FOUND => {
                return Err(ProviderError::NotFound {
                    entity: playlist_id.0.clone(),
                })
            }
            _ => {}
        }
        let body: models::SongPagedResponse = futures::executor::block_on(async {
            resp.json().await
        })
        .map_err(|e| ProviderError::Other {
            message: e.to_string(),
        })?;
        let items = body
            .data
            .into_iter()
            .map(|s| map_track(&s, &self.id))
            .collect();
        Ok(Page { items, next: None })
    }

    fn get_album(&self, album_id: &AlbumId) -> ProviderResult<Album> {
        let url = self
            .base_url
            .join(&format!("api/v1/albums/{}", album_id.0))
            .map_err(|e| ProviderError::Other {
                message: e.to_string(),
            })?;
        let resp = futures::executor::block_on(async {
            self.client
                .get(url)
                .bearer_auth(self.auth_header().unwrap_or_default())
                .send()
                .await
        })
        .map_err(|e| ProviderError::NetworkError {
            message: e.to_string(),
        })?;
        match resp.status() {
            reqwest::StatusCode::UNAUTHORIZED => {
                return Err(ProviderError::AuthenticationError {
                    message: "unauthorized".into(),
                })
            }
            reqwest::StatusCode::NOT_FOUND => {
                return Err(ProviderError::NotFound {
                    entity: album_id.0.clone(),
                })
            }
            _ => {}
        }
        let album: models::Album = futures::executor::block_on(async { resp.json().await })
            .map_err(|e| ProviderError::Other {
                message: e.to_string(),
            })?;
        Ok(map_album(&album, &self.id))
    }

    fn list_album_tracks(
        &self,
        album_id: &AlbumId,
        paging: PageRequest,
    ) -> ProviderResult<Page<Track>> {
        let url = self
            .base_url
            .join(&format!("api/v1/albums/{}/songs", album_id.0))
            .map_err(|e| ProviderError::Other {
                message: e.to_string(),
            })?;
        let resp = futures::executor::block_on(async {
            self.client
                .get(url)
                .query(&[
                    ("page", paging.offset / paging.limit),
                    ("pageSize", paging.limit),
                ])
                .bearer_auth(self.auth_header().unwrap_or_default())
                .send()
                .await
        })
        .map_err(|e| ProviderError::NetworkError {
            message: e.to_string(),
        })?;
        match resp.status() {
            reqwest::StatusCode::UNAUTHORIZED => {
                return Err(ProviderError::AuthenticationError {
                    message: "unauthorized".into(),
                })
            }
            reqwest::StatusCode::NOT_FOUND => {
                return Err(ProviderError::NotFound {
                    entity: album_id.0.clone(),
                })
            }
            _ => {}
        }
        let body: models::SongPagedResponse = futures::executor::block_on(async {
            resp.json().await
        })
        .map_err(|e| ProviderError::Other {
            message: e.to_string(),
        })?;
        let items = body
            .data
            .into_iter()
            .map(|s| map_track(&s, &self.id))
            .collect();
        Ok(Page { items, next: None })
    }

    fn get_track(&self, track_id: &TrackId) -> ProviderResult<Track> {
        let url = self
            .base_url
            .join(&format!("api/v1/songs/{}", track_id.0))
            .map_err(|e| ProviderError::Other {
                message: e.to_string(),
            })?;
        let resp = futures::executor::block_on(async {
            self.client
                .get(url)
                .bearer_auth(self.auth_header().unwrap_or_default())
                .send()
                .await
        })
        .map_err(|e| ProviderError::NetworkError {
            message: e.to_string(),
        })?;
        match resp.status() {
            reqwest::StatusCode::UNAUTHORIZED => {
                return Err(ProviderError::AuthenticationError {
                    message: "unauthorized".into(),
                })
            }
            reqwest::StatusCode::NOT_FOUND => {
                return Err(ProviderError::NotFound {
                    entity: track_id.0.clone(),
                })
            }
            _ => {}
        }
        let song: models::Song =
            futures::executor::block_on(async { resp.json().await }).map_err(|e| {
                ProviderError::Other {
                    message: e.to_string(),
                }
            })?;
        Ok(map_track(&song, &self.id))
    }

    fn get_stream_url(&self, track_id: &TrackId) -> ProviderResult<StreamUrl> {
        let song = self.get_track(track_id).map_err(|e| ProviderError::Other {
            message: e.to_string(),
        })?;
        if song.id != *track_id {
            return Err(ProviderError::Other {
                message: "track id mismatch".into(),
            });
        }
        Ok(StreamUrl(song.id.0.clone()))
    }
}
