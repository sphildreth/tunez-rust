use path_clean::PathClean;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use tunez_core::models::{Album, AlbumId, Track, TrackId};
use tunez_core::provider::{ProviderError, ProviderResult};
use walkdir::WalkDir;

#[derive(Debug, Clone, Default)]
pub struct LibraryIndex {
    pub tracks: Vec<Track>,
    pub albums: BTreeMap<AlbumId, Album>,
    pub artists: BTreeSet<String>,
}

pub fn scan_library(roots: Vec<String>) -> ProviderResult<LibraryIndex> {
    let mut index = LibraryIndex::default();
    for root in roots {
        let root_path = PathBuf::from(root.clone());
        for entry in WalkDir::new(&root_path).follow_links(false) {
            let entry = entry.map_err(|e| ProviderError::Other {
                message: e.to_string(),
            })?;
            if !entry.file_type().is_file() {
                continue;
            }
            let path = entry.path();
            if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                if is_supported_extension(ext) {
                    if let Some(track) = parse_track(path, &root_path)? {
                        index.artists.insert(track.artist.clone());
                        if let Some(album_title) = &track.album {
                            let album_id = AlbumId::new(album_title.clone());
                            index.albums.entry(album_id.clone()).or_insert(Album {
                                id: album_id.clone(),
                                provider_id: "filesystem".into(),
                                title: album_title.clone(),
                                artist: track.artist.clone(),
                                track_count: None,
                                duration_seconds: None,
                            });
                        }
                        index.tracks.push(track);
                    }
                }
            }
        }
    }
    Ok(index)
}

fn is_supported_extension(ext: &str) -> bool {
    matches!(
        ext.to_ascii_lowercase().as_str(),
        "mp3" | "m4a" | "flac" | "wav" | "ogg"
    )
}

fn parse_track(path: &Path, root: &Path) -> ProviderResult<Option<Track>> {
    let canonical = path
        .canonicalize()
        .map_err(|e| ProviderError::Other {
            message: e.to_string(),
        })?
        .clean();
    if !canonical.starts_with(root) {
        return Ok(None);
    }
    let id = TrackId::new(canonical.to_string_lossy().to_string());
    let title = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Unknown")
        .to_string();
    let track = Track {
        id,
        provider_id: "filesystem".into(),
        title,
        artist: "Unknown Artist".into(),
        album: None,
        duration_seconds: None,
    };
    Ok(Some(track))
}
