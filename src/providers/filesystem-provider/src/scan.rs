use crate::tags::parse_tags;
use path_clean::PathClean;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use tunez_core::models::{Album, AlbumId, Playlist, PlaylistId, Track, TrackId};
use tunez_core::provider::{ProviderError, ProviderResult};
use walkdir::WalkDir;

#[derive(Debug, Clone, Default)]
pub struct LibraryIndex {
    pub tracks: Vec<Track>,
    pub albums: BTreeMap<AlbumId, Album>,
    pub artists: BTreeSet<String>,
    pub playlists: BTreeMap<PlaylistId, PlaylistEntry>,
}

#[derive(Debug, Clone, Default)]
pub struct PlaylistEntry {
    pub playlist: Playlist,
    pub track_ids: Vec<TrackId>,
}

#[derive(Debug, Clone)]
pub struct ScanOptions {
    pub follow_symlinks: bool,
    pub excluded_paths: Vec<PathBuf>,
    pub extensions_allowlist: Vec<String>,
}

impl Default for ScanOptions {
    fn default() -> Self {
        Self {
            follow_symlinks: false,
            excluded_paths: Vec::new(),
            extensions_allowlist: vec![
                "mp3".into(),
                "m4a".into(),
                "flac".into(),
                "wav".into(),
                "ogg".into(),
            ],
        }
    }
}

pub fn album_id_for(artist: &str, album: &str) -> AlbumId {
    AlbumId::new(format!("{}::{}", artist, album))
}

fn canonicalize_within_root(path: &Path, root: &Path) -> Option<PathBuf> {
    let Ok(canon) = path.canonicalize() else {
        return None;
    };
    let cleaned = canon.clean();
    if cleaned.starts_with(root) {
        Some(cleaned)
    } else {
        None
    }
}

#[allow(dead_code)]
pub fn scan_library(roots: Vec<String>) -> ProviderResult<LibraryIndex> {
    scan_library_with_options(roots, ScanOptions::default())
}

pub fn scan_library_with_options(
    roots: Vec<String>,
    opts: ScanOptions,
) -> ProviderResult<LibraryIndex> {
    let mut index = LibraryIndex::default();
    for root in roots {
        let root_path = PathBuf::from(root.clone());
        for entry in WalkDir::new(&root_path).follow_links(opts.follow_symlinks) {
            let entry = match entry {
                Ok(e) => e,
                Err(_e) => {
                    continue;
                }
            };
            if !entry.file_type().is_file() {
                continue;
            }
            let path = entry.path();

            if opts.excluded_paths.iter().any(|p| path.starts_with(p)) {
                continue;
            }

            if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                if is_supported_extension(ext, &opts.extensions_allowlist) {
                    if let Some(track) = parse_track(path, &root_path)? {
                        index.artists.insert(track.artist.clone());
                        if let Some(album_title) = &track.album {
                            let album_id = album_id_for(&track.artist, album_title);
                            let album_entry =
                                index.albums.entry(album_id.clone()).or_insert(Album {
                                    id: album_id.clone(),
                                    provider_id: "filesystem".into(),
                                    title: album_title.clone(),
                                    artist: track.artist.clone(),
                                    track_count: Some(0),
                                    duration_seconds: None,
                                });
                            album_entry.track_count =
                                Some(album_entry.track_count.unwrap_or(0) + 1);
                        }
                        index.tracks.push(track);
                    }
                } else if is_playlist_extension(ext) {
                    if let Some(rel) = path.strip_prefix(&root_path).ok().and_then(|p| p.to_str()) {
                        load_m3u_playlist(&mut index, path, rel, &root_path, &opts)?;
                    }
                }
            }
        }
    }
    index
        .tracks
        .sort_by(|a, b| a.title.cmp(&b.title).then_with(|| a.id.0.cmp(&b.id.0)));
    Ok(index)
}

fn is_supported_extension(ext: &str, allowlist: &[String]) -> bool {
    let lowered = ext.to_ascii_lowercase();
    allowlist.iter().any(|allowed| allowed == &lowered)
}

fn is_playlist_extension(ext: &str) -> bool {
    matches!(ext.to_ascii_lowercase().as_str(), "m3u" | "m3u8")
}

fn parse_track(path: &Path, root: &Path) -> ProviderResult<Option<Track>> {
    let Some(canonical) = canonicalize_within_root(path, root) else {
        return Ok(None);
    };
    let id = TrackId::new(canonical.to_string_lossy().to_string());

    let relative = canonical
        .strip_prefix(root)
        .map_err(|e| ProviderError::Other {
            message: e.to_string(),
        })?;
    let mut components = relative.components().collect::<Vec<_>>();
    let _ = components.pop();
    let (inferred_artist, inferred_album) = if components.len() >= 2 {
        let album_component = components
            .pop()
            .and_then(|c| c.as_os_str().to_str())
            .unwrap_or("Unknown Album");
        let artist_component = components
            .pop()
            .and_then(|c| c.as_os_str().to_str())
            .unwrap_or("Unknown Artist");
        (
            artist_component.to_string(),
            Some(album_component.to_string()),
        )
    } else if components.len() == 1 {
        let artist_component = components
            .pop()
            .and_then(|c| c.as_os_str().to_str())
            .unwrap_or("Unknown Artist");
        (artist_component.to_string(), None)
    } else {
        ("Unknown Artist".into(), None)
    };

    let file_stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Unknown");

    let tags = parse_tags(path)?;
    let artist = tags.artist.unwrap_or(inferred_artist);
    let album = tags.album.or(inferred_album);
    let title = tags.title.unwrap_or_else(|| file_stem.to_string());

    let track = Track {
        id,
        provider_id: "filesystem".into(),
        title,
        artist,
        album,
        duration_seconds: tags.duration_seconds,
        track_number: tags.track_number,
    };
    Ok(Some(track))
}

fn load_m3u_playlist(
    index: &mut LibraryIndex,
    path: &Path,
    rel_path: &str,
    root: &Path,
    opts: &ScanOptions,
) -> ProviderResult<()> {
    let playlist_id = PlaylistId::new(rel_path);
    let playlist = Playlist {
        id: playlist_id.clone(),
        provider_id: "filesystem".into(),
        name: rel_path.to_string(),
        description: None,
        track_count: None,
    };

    let contents = fs::read_to_string(path).map_err(|e| ProviderError::Other {
        message: e.to_string(),
    })?;
    let mut track_ids = Vec::new();
    for line in contents.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let candidate = if Path::new(trimmed).is_absolute() {
            PathBuf::from(trimmed)
        } else {
            root.join(trimmed)
        };
        if let Some(canonical) = canonicalize_within_root(&candidate, root) {
            if opts.extensions_allowlist.iter().any(|ext| {
                candidate
                    .extension()
                    .and_then(|s| s.to_str())
                    .map(|e| e.eq_ignore_ascii_case(ext))
                    .unwrap_or(false)
            }) {
                track_ids.push(TrackId::new(canonical.to_string_lossy().to_string()));
            }
        }
    }

    index.playlists.insert(
        playlist_id,
        PlaylistEntry {
            playlist,
            track_ids,
        },
    );

    Ok(())
}
