mod scan;

use scan::{scan_library, LibraryIndex};
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
}

impl FilesystemProvider {
    pub fn new(roots: Vec<String>) -> Result<Self, ProviderError> {
        let index = scan_library(roots)?;
        Ok(Self {
            id: "filesystem".into(),
            name: "Filesystem".into(),
            index: Arc::new(RwLock::new(index)),
        })
    }

    fn caps() -> ProviderCapabilities {
        ProviderCapabilities {
            playlists: false,
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
        Self::caps()
    }

    fn search_tracks(
        &self,
        query: &str,
        _filters: TrackSearchFilters,
        paging: PageRequest,
    ) -> ProviderResult<Page<Track>> {
        let index = self.index.read().expect("index poisoned");
        let mut items: Vec<Track> = index
            .tracks
            .iter()
            .filter(|t| {
                let q = query.to_ascii_lowercase();
                t.title.to_ascii_lowercase().contains(&q)
                    || t.artist.to_ascii_lowercase().contains(&q)
                    || t.album
                        .as_ref()
                        .map(|a| a.to_ascii_lowercase().contains(&q))
                        .unwrap_or(false)
            })
            .cloned()
            .collect();
        items.sort_by(|a, b| a.title.cmp(&b.title));
        let start = paging.offset as usize;
        let end = start.saturating_add(paging.limit as usize);
        let slice = items
            .into_iter()
            .skip(start)
            .take(paging.limit as usize)
            .collect::<Vec<_>>();
        let next = if end < index.tracks.len() {
            Some(PageCursor(end.to_string()))
        } else {
            None
        };
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
                albums.sort_by(|a, b| a.title.cmp(&b.title));
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

    fn list_playlists(&self, _paging: PageRequest) -> ProviderResult<Page<Playlist>> {
        Err(ProviderError::NotSupported {
            operation: "list_playlists".into(),
        })
    }

    fn search_playlists(
        &self,
        _query: &str,
        _paging: PageRequest,
    ) -> ProviderResult<Page<Playlist>> {
        Err(ProviderError::NotSupported {
            operation: "search_playlists".into(),
        })
    }

    fn get_playlist(&self, _playlist_id: &PlaylistId) -> ProviderResult<Playlist> {
        Err(ProviderError::NotSupported {
            operation: "get_playlist".into(),
        })
    }

    fn list_playlist_tracks(
        &self,
        _playlist_id: &PlaylistId,
        _paging: PageRequest,
    ) -> ProviderResult<Page<Track>> {
        Err(ProviderError::NotSupported {
            operation: "list_playlist_tracks".into(),
        })
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
        let tracks = index
            .tracks
            .iter()
            .filter(|t| t.album.as_ref() == Some(&album_id.0))
            .cloned()
            .collect::<Vec<_>>();
        let start = paging.offset as usize;
        let end = start.saturating_add(paging.limit as usize);
        let slice = tracks
            .into_iter()
            .skip(start)
            .take(paging.limit as usize)
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
        let track = self.get_track(track_id)?;
        Ok(StreamUrl(format!("file://{}", track.id.0)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::tempdir;

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
}
