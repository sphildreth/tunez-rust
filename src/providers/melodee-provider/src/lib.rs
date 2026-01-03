mod mapping;
pub mod models;

use mapping::{map_album, map_playlist, map_track};
use reqwest::blocking::{Client, Response};
use reqwest::StatusCode;
use serde::de::DeserializeOwned;
use std::sync::{Arc, RwLock};
use std::time::Duration;
use tunez_core::models::{
    Album, AlbumId, Page, PageRequest, Playlist, PlaylistId, StreamUrl, Track, TrackId,
};
use tunez_core::provider::{
    BrowseKind, CollectionItem, Provider, ProviderCapabilities, ProviderError, ProviderResult,
    TrackSearchFilters,
};
use url::Url;

use tunez_core::secrets::CredentialStore;

#[derive(Clone)]
pub struct MelodeeConfig {
    pub base_url: String,
    pub profile: Option<String>,
}

#[derive(Clone)]
pub struct MelodeeProvider {
    id: String,
    name: String,
    client: Client,
    base_url: Url,
    profile: Option<String>,
    creds: CredentialStore,
    // Cache the token in memory to avoid hitting keyring on every request
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
            profile: config.profile,
            creds: CredentialStore::new(),
            access_token: Arc::new(RwLock::new(None)),
        })
    }

    fn auth_header(&self) -> Option<String> {
        // First check memory cache
        if let Ok(guard) = self.access_token.read() {
            if let Some(token) = guard.as_ref() {
                return Some(token.clone());
            }
        }

        // Then try keyring
        if let Ok(token) = self
            .creds
            .get_access_token(&self.id, self.profile.as_deref())
        {
            // Cache it
            if let Ok(mut guard) = self.access_token.write() {
                *guard = Some(token.clone());
            }
            return Some(token);
        }

        None
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

    fn paging_query(&self, paging: PageRequest) -> Vec<(&str, String)> {
        vec![
            ("page", (paging.offset / paging.limit).to_string()),
            ("pageSize", paging.limit.to_string()),
        ]
    }

    fn send_get<T: DeserializeOwned>(
        &self,
        path: &str,
        query: Vec<(&str, String)>,
        not_found_entity: Option<String>,
    ) -> ProviderResult<T> {
        let url = self.base_url.join(path).map_err(|e| ProviderError::Other {
            message: e.to_string(),
        })?;
        let mut request = self.client.get(url.clone()).query(&query);
        if let Some(token) = self.auth_header() {
            request = request.bearer_auth(token);
        }
        let response = request.send().map_err(|e| ProviderError::NetworkError {
            message: e.to_string(),
        })?;
        let response = Self::map_response(response, path, not_found_entity)?;
        response.json::<T>().map_err(|e| ProviderError::Other {
            message: e.to_string(),
        })
    }

    fn map_response(
        response: Response,
        path: &str,
        not_found_entity: Option<String>,
    ) -> ProviderResult<Response> {
        match response.status() {
            StatusCode::UNAUTHORIZED => Err(ProviderError::AuthenticationError {
                message: "unauthorized".into(),
            }),
            StatusCode::NOT_FOUND => Err(ProviderError::NotFound {
                entity: not_found_entity.unwrap_or_else(|| path.to_string()),
            }),
            status if status.is_client_error() || status.is_server_error() => {
                Err(ProviderError::Other {
                    message: format!("http {} from {}", status, path),
                })
            }
            _ => Ok(response),
        }
    }

    fn fetch_song(&self, track_id: &TrackId) -> ProviderResult<models::Song> {
        self.send_get(
            &format!("api/v1/songs/{}", track_id.0),
            Vec::new(),
            Some(track_id.0.clone()),
        )
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
        filters: TrackSearchFilters,
        paging: PageRequest,
    ) -> ProviderResult<Page<Track>> {
        let mut query_params = vec![
            ("q", query.to_string()),
            ("page", (paging.offset / paging.limit).to_string()),
            ("pageSize", paging.limit.to_string()),
        ];
        if let Some(artist) = filters.artist {
            query_params.push(("filterByArtistApiKey", artist));
        }
        let body: models::SongPagedResponse =
            self.send_get("api/v1/search/songs", query_params, None)?;
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
                let body: models::AlbumPagedResponse =
                    self.send_get("api/v1/albums", self.paging_query(paging), None)?;
                let items = body
                    .data
                    .into_iter()
                    .map(|a| CollectionItem::Album(map_album(&a, &self.id)))
                    .collect();
                Ok(Page { items, next: None })
            }
            BrowseKind::Playlists => {
                let body: models::PlaylistPagedResponse = self.send_get(
                    "api/v1/user/playlists",
                    vec![
                        ("page", (paging.offset / paging.limit).to_string()),
                        ("limit", paging.limit.to_string()),
                    ],
                    None,
                )?;
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
        let body: models::PlaylistPagedResponse = self.send_get(
            "api/v1/user/playlists",
            vec![
                ("page", (paging.offset / paging.limit).to_string()),
                ("limit", paging.limit.to_string()),
            ],
            None,
        )?;
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
        let playlist: models::Playlist = self.send_get(
            &format!("api/v1/playlists/{}", playlist_id.0),
            Vec::new(),
            Some(playlist_id.0.clone()),
        )?;
        Ok(map_playlist(&playlist, &self.id))
    }

    fn list_playlist_tracks(
        &self,
        playlist_id: &PlaylistId,
        paging: PageRequest,
    ) -> ProviderResult<Page<Track>> {
        let body: models::SongPagedResponse = self.send_get(
            &format!("api/v1/playlists/{}/songs", playlist_id.0),
            self.paging_query(paging),
            Some(playlist_id.0.clone()),
        )?;
        let items = body
            .data
            .into_iter()
            .map(|s| map_track(&s, &self.id))
            .collect();
        Ok(Page { items, next: None })
    }

    fn get_album(&self, album_id: &AlbumId) -> ProviderResult<Album> {
        let album: models::Album = self.send_get(
            &format!("api/v1/albums/{}", album_id.0),
            Vec::new(),
            Some(album_id.0.clone()),
        )?;
        Ok(map_album(&album, &self.id))
    }

    fn list_album_tracks(
        &self,
        album_id: &AlbumId,
        paging: PageRequest,
    ) -> ProviderResult<Page<Track>> {
        let body: models::SongPagedResponse = self.send_get(
            &format!("api/v1/albums/{}/songs", album_id.0),
            self.paging_query(paging),
            Some(album_id.0.clone()),
        )?;
        let items = body
            .data
            .into_iter()
            .map(|s| map_track(&s, &self.id))
            .collect();
        Ok(Page { items, next: None })
    }

    fn get_track(&self, track_id: &TrackId) -> ProviderResult<Track> {
        let song = self.fetch_song(track_id)?;
        Ok(map_track(&song, &self.id))
    }

    fn get_stream_url(&self, track_id: &TrackId) -> ProviderResult<StreamUrl> {
        let song = self.fetch_song(track_id)?;
        if song.id != track_id.0 {
            return Err(ProviderError::Other {
                message: "track id mismatch".into(),
            });
        }
        let raw_url = song
            .stream_url
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| ProviderError::Other {
                message: format!("missing stream url for track {}", track_id.0),
            })?;
        let resolved = Url::parse(raw_url)
            .or_else(|_| self.base_url.join(raw_url))
            .map_err(|e| ProviderError::Other {
                message: format!("invalid stream url: {e}"),
            })?;
        Ok(StreamUrl::new(resolved.to_string()))
    }

    fn get_lyrics(&self, track_id: &TrackId) -> ProviderResult<String> {
        let lyrics: models::Lyrics = self.send_get(
            &format!("api/v1/songs/{}/lyrics", track_id.0),
            Vec::new(),
            Some(track_id.0.clone()),
        )?;
        Ok(lyrics.plain_text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tunez_core::provider_contract::{
        run_provider_contract, PlaylistExpectation, ProviderContractExpectations, SearchExpectation,
    };
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[test]
    fn provider_contract_passes_against_mock() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let server = rt.block_on(MockServer::start());
        let base_url = format!("{}/", server.uri());

        rt.block_on(
            Mock::given(method("GET"))
                .and(path("/api/v1/search/songs"))
                .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                    "data": [
                        {
                            "id": "song-1",
                            "title": "Test Song",
                            "durationMs": 180000,
                            "streamUrl": "/stream/song-1",
                            "artist": { "id": "artist-1", "name": "Artist" },
                            "album": { "id": "album-1", "name": "Album" }
                        }
                    ],
                    "meta": { "totalCount": 1, "pageSize": 1, "currentPage": 1 }
                })))
                .mount(&server),
        );

        rt.block_on(
            Mock::given(method("GET"))
                .and(path("/api/v1/songs/song-1"))
                .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                    "id": "song-1",
                    "title": "Test Song",
                    "durationMs": 180000,
                    "streamUrl": "/stream/song-1",
                    "artist": { "id": "artist-1", "name": "Artist" },
                    "album": { "id": "album-1", "name": "Album" }
                })))
                .mount(&server),
        );

        rt.block_on(
            Mock::given(method("GET"))
                .and(path("/api/v1/user/playlists"))
                .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                    "data": [
                        {
                            "apiKey": "playlist-1",
                            "name": "Morning Mix",
                            "description": "Desc",
                            "songsCount": 1
                        }
                    ],
                    "meta": { "totalCount": 1, "pageSize": 1, "currentPage": 1 }
                })))
                .mount(&server),
        );

        let provider = MelodeeProvider::new(MelodeeConfig {
            base_url,
            profile: None,
        })
        .expect("provider constructed");

        let track_id = TrackId::new("song-1");
        let expectations = ProviderContractExpectations {
            provider_id: "melodee".into(),
            search: SearchExpectation {
                query: "test".into(),
                filters: TrackSearchFilters::default(),
                expected_first_track_id: track_id.clone(),
            },
            stream_track_id: track_id.clone(),
            playlist: Some(PlaylistExpectation {
                playlist_id: PlaylistId::new("playlist-1"),
                search_query: Some("Mix".into()),
            }),
        };

        run_provider_contract(&provider, &expectations).unwrap();
    }
}
