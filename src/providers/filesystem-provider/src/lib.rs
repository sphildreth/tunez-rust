mod scan;
mod tags;

use scan::{scan_library_with_options, LibraryIndex, ScanOptions};
use std::sync::{Arc, RwLock};
use tunez_core::models::{
    Album, AlbumId, Page, PageCursor, PageRequest, Playlist, PlaylistId, StreamUrl, Track, TrackId,
};
use tunez_core::provider::{
    BrowseKind, CollectionItem, Provider, ProviderCapabilities, ProviderError, ProviderResult,
    TrackSearchFilters,
};

#[derive(Clone, Debug)]
pub struct FilesystemProvider {
    id: String,
    name: String,
    index: Arc<RwLock<LibraryIndex>>,
    capabilities: Arc<RwLock<ProviderCapabilities>>,
    roots: Vec<String>,
    options: ScanOptions,
}

impl FilesystemProvider {
    pub fn new(roots: Vec<String>) -> Result<Self, ProviderError> {
        Self::with_options(roots, ScanOptions::default())
    }

    pub fn with_options(roots: Vec<String>, options: ScanOptions) -> Result<Self, ProviderError> {
        let index = scan_library_with_options(roots.clone(), options.clone())?;
        let caps = Self::capabilities_from_index(&index);
        Ok(Self {
            id: "filesystem".into(),
            name: "Filesystem".into(),
            index: Arc::new(RwLock::new(index)),
            capabilities: Arc::new(RwLock::new(caps)),
            roots,
            options,
        })
    }

    pub fn rescan(&self) -> Result<(), ProviderError> {
        let new_index = scan_library_with_options(self.roots.clone(), self.options.clone())?;
        let mut guard = self.index.write().expect("index poisoned");
        *guard = new_index;
        let caps = Self::capabilities_from_index(&guard);
        let mut caps_guard = self.capabilities.write().expect("capabilities poisoned");
        *caps_guard = caps;
        Ok(())
    }

    fn capabilities_from_index(index: &LibraryIndex) -> ProviderCapabilities {
        ProviderCapabilities {
            playlists: !index.playlists.is_empty(),
            lyrics: false,
            artwork: false,
            favorites: false,
            recently_played: false,
            offline_download: true,
        }
    }
}

impl Provider for FilesystemProvider {
    fn id(&self) -> &str {
        &self.id
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn capabilities(&self) -> ProviderCapabilities {
        *self.capabilities.read().expect("capabilities poisoned")
    }

    fn search_tracks(
        &self,
        query: &str,
        _filters: TrackSearchFilters,
        paging: PageRequest,
    ) -> ProviderResult<Page<Track>> {
        let index = self.index.read().expect("index poisoned");
        let q = query.to_ascii_lowercase();
        let mut items: Vec<Track> = index
            .tracks
            .iter()
            .filter(|t| {
                t.title.to_ascii_lowercase().contains(&q)
                    || t.artist.to_ascii_lowercase().contains(&q)
                    || t.album
                        .as_ref()
                        .map(|a| a.to_ascii_lowercase().contains(&q))
                        .unwrap_or(false)
            })
            .cloned()
            .collect();
        items.sort_by(|a, b| a.title.cmp(&b.title).then_with(|| a.id.0.cmp(&b.id.0)));
        let start = paging.offset as usize;
        let end = start.saturating_add(paging.limit as usize);
        let next = if end < items.len() {
            Some(PageCursor(end.to_string()))
        } else {
            None
        };
        let slice = items
            .into_iter()
            .skip(start)
            .take(paging.limit as usize)
            .collect::<Vec<_>>();
        Ok(Page { items: slice, next })
    }

    fn browse(
        &self,
        kind: BrowseKind,
        paging: PageRequest,
    ) -> ProviderResult<Page<CollectionItem>> {
        let index = self.index.read().expect("index poisoned");
        match kind {
            BrowseKind::Artists => {
                let mut artists: Vec<_> = index
                    .artists
                    .iter()
                    .cloned()
                    .map(|name| CollectionItem::Artist {
                        name,
                        provider_id: self.id.clone(),
                    })
                    .collect();
                artists.sort_by(|a, b| match (a, b) {
                    (
                        CollectionItem::Artist { name: a, .. },
                        CollectionItem::Artist { name: b, .. },
                    ) => a.cmp(b),
                    _ => std::cmp::Ordering::Equal,
                });
                let start = paging.offset as usize;
                let end = start.saturating_add(paging.limit as usize);
                let slice = artists
                    .into_iter()
                    .skip(start)
                    .take(paging.limit as usize)
                    .collect::<Vec<_>>();
                let next = if end < index.artists.len() {
                    Some(PageCursor(end.to_string()))
                } else {
                    None
                };
                Ok(Page { items: slice, next })
            }
            BrowseKind::Albums => {
                let mut albums: Vec<Album> = index.albums.values().cloned().collect();
                albums.sort_by(|a, b| a.title.cmp(&b.title).then_with(|| a.id.0.cmp(&b.id.0)));
                let start = paging.offset as usize;
                let end = start.saturating_add(paging.limit as usize);
                let slice = albums
                    .into_iter()
                    .skip(start)
                    .take(paging.limit as usize)
                    .map(CollectionItem::Album)
                    .collect();
                let next = if end < index.albums.len() {
                    Some(PageCursor(end.to_string()))
                } else {
                    None
                };
                Ok(Page { items: slice, next })
            }
            BrowseKind::Playlists | BrowseKind::Genres => Err(ProviderError::NotSupported {
                operation: "browse".into(),
            }),
        }
    }

    fn list_playlists(&self, paging: PageRequest) -> ProviderResult<Page<Playlist>> {
        if !self.capabilities().supports_playlists() {
            return Err(ProviderError::NotSupported {
                operation: "list_playlists".into(),
            });
        }
        let index = self.index.read().expect("index poisoned");
        let mut items: Vec<Playlist> = index
            .playlists
            .values()
            .map(|p| p.playlist.clone())
            .collect();
        items.sort_by(|a, b| a.name.cmp(&b.name).then_with(|| a.id.0.cmp(&b.id.0)));
        let start = paging.offset as usize;
        let end = start.saturating_add(paging.limit as usize);
        let next = if end < items.len() {
            Some(PageCursor(end.to_string()))
        } else {
            None
        };
        let slice = items
            .into_iter()
            .skip(start)
            .take(paging.limit as usize)
            .collect();
        Ok(Page { items: slice, next })
    }

    fn search_playlists(&self, query: &str, paging: PageRequest) -> ProviderResult<Page<Playlist>> {
        if !self.capabilities().supports_playlists() {
            return Err(ProviderError::NotSupported {
                operation: "search_playlists".into(),
            });
        }
        let index = self.index.read().expect("index poisoned");
        let q = query.to_ascii_lowercase();
        let mut items: Vec<Playlist> = index
            .playlists
            .values()
            .filter(|p| p.playlist.name.to_ascii_lowercase().contains(&q))
            .map(|p| p.playlist.clone())
            .collect();
        items.sort_by(|a, b| a.name.cmp(&b.name).then_with(|| a.id.0.cmp(&b.id.0)));
        let start = paging.offset as usize;
        let end = start.saturating_add(paging.limit as usize);
        let next = if end < items.len() {
            Some(PageCursor(end.to_string()))
        } else {
            None
        };
        let slice = items
            .into_iter()
            .skip(start)
            .take(paging.limit as usize)
            .collect();
        Ok(Page { items: slice, next })
    }

    fn get_playlist(&self, playlist_id: &PlaylistId) -> ProviderResult<Playlist> {
        if !self.capabilities().supports_playlists() {
            return Err(ProviderError::NotSupported {
                operation: "get_playlist".into(),
            });
        }
        let index = self.index.read().expect("index poisoned");
        let entry = index
            .playlists
            .get(playlist_id)
            .ok_or(ProviderError::NotFound {
                entity: playlist_id.0.clone(),
            })?;
        Ok(entry.playlist.clone())
    }

    fn list_playlist_tracks(
        &self,
        playlist_id: &PlaylistId,
        paging: PageRequest,
    ) -> ProviderResult<Page<Track>> {
        if !self.capabilities().supports_playlists() {
            return Err(ProviderError::NotSupported {
                operation: "list_playlist_tracks".into(),
            });
        }
        let index = self.index.read().expect("index poisoned");
        let entry = index
            .playlists
            .get(playlist_id)
            .ok_or(ProviderError::NotFound {
                entity: playlist_id.0.clone(),
            })?;
        let mut tracks: Vec<Track> = entry
            .track_ids
            .iter()
            .filter_map(|id| index.tracks.iter().find(|t| &t.id == id))
            .cloned()
            .collect();
        tracks.sort_by(|a, b| a.title.cmp(&b.title).then_with(|| a.id.0.cmp(&b.id.0)));
        let start = paging.offset as usize;
        let end = start.saturating_add(paging.limit as usize);
        let next = if end < tracks.len() {
            Some(PageCursor(end.to_string()))
        } else {
            None
        };
        let slice = tracks
            .into_iter()
            .skip(start)
            .take(paging.limit as usize)
            .collect();
        Ok(Page { items: slice, next })
    }

    fn get_album(&self, album_id: &AlbumId) -> ProviderResult<Album> {
        let index = self.index.read().expect("index poisoned");
        index
            .albums
            .get(album_id)
            .cloned()
            .ok_or_else(|| ProviderError::NotFound {
                entity: album_id.0.clone(),
            })
    }

    fn list_album_tracks(
        &self,
        album_id: &AlbumId,
        paging: PageRequest,
    ) -> ProviderResult<Page<Track>> {
        let index = self.index.read().expect("index poisoned");
        let mut tracks = index
            .tracks
            .iter()
            .filter(|t| t.album.as_ref() == Some(&album_id.0))
            .cloned()
            .collect::<Vec<_>>();
        tracks.sort_by(|a, b| match (a.track_number, b.track_number) {
            (Some(na), Some(nb)) => na.cmp(&nb).then_with(|| a.title.cmp(&b.title)),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => a.title.cmp(&b.title),
        });
        let start = paging.offset as usize;
        let end = start.saturating_add(paging.limit as usize);
        let slice = tracks
            .iter()
            .skip(start)
            .take(paging.limit as usize)
            .cloned()
            .collect::<Vec<_>>();
        let next = if end < tracks.len() {
            Some(PageCursor(end.to_string()))
        } else {
            None
        };
        Ok(Page { items: slice, next })
    }

    fn get_track(&self, track_id: &TrackId) -> ProviderResult<Track> {
        let index = self.index.read().expect("index poisoned");
        index
            .tracks
            .iter()
            .find(|t| &t.id == track_id)
            .cloned()
            .ok_or_else(|| ProviderError::NotFound {
                entity: track_id.0.clone(),
            })
    }

    fn get_stream_url(&self, track_id: &TrackId) -> ProviderResult<StreamUrl> {
        // Validate the file still exists before returning the URL.
        let track = self.get_track(track_id)?;
        let path = std::path::Path::new(&track.id.0);
        if !path.exists() {
            return Err(ProviderError::NotFound {
                entity: track.id.0.clone(),
            });
        }
        Ok(StreamUrl(format!("file://{}", track.id.0)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::tempdir;
    use tunez_core::models::TrackId;
    use tunez_core::provider_contract::{
        run_provider_contract, ProviderContractExpectations, SearchExpectation,
    };

    #[test]
    fn search_returns_tracks() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("song.mp3");
        let mut f = File::create(&file_path).unwrap();
        writeln!(f, "fake").unwrap();

        let provider =
            FilesystemProvider::new(vec![dir.path().to_string_lossy().to_string()]).unwrap();
        let page = provider
            .search_tracks(
                "song",
                TrackSearchFilters::default(),
                PageRequest::first_page(10),
            )
            .unwrap();
        assert!(!page.items.is_empty());
    }

    #[test]
    fn provider_contract_passes() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("contract.mp3");
        let mut f = File::create(&file_path).unwrap();
        writeln!(f, "fake").unwrap();
        let provider =
            FilesystemProvider::new(vec![dir.path().to_string_lossy().to_string()]).unwrap();
        let track_id = TrackId::new(
            file_path
                .canonicalize()
                .unwrap()
                .to_string_lossy()
                .to_string(),
        );

        let expectations = ProviderContractExpectations {
            provider_id: "filesystem".into(),
            search: SearchExpectation {
                query: "contract".into(),
                filters: TrackSearchFilters::default(),
                expected_first_track_id: track_id.clone(),
            },
            stream_track_id: track_id,
            playlist: None,
        };

        run_provider_contract(&provider, &expectations).unwrap();
    }
}
