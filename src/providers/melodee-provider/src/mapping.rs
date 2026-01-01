use crate::models::{Album, Playlist, Song};
use tunez_core::models::{
    Album as CoreAlbum, AlbumId, Playlist as CorePlaylist, PlaylistId, Track, TrackId,
};

pub fn map_track(song: &Song, provider_id: &str) -> Track {
    Track {
        id: TrackId::new(song.id.clone()),
        provider_id: provider_id.to_string(),
        title: song.title.clone(),
        artist: song
            .artist
            .as_ref()
            .map(|a| a.name.clone())
            .unwrap_or_else(|| "Unknown Artist".into()),
        album: song.album.as_ref().map(|a| a.name.clone()),
        duration_seconds: song.duration_ms.map(|d| (d / 1000) as u32),
        track_number: None,
    }
}

pub fn map_album(album: &Album, provider_id: &str) -> CoreAlbum {
    CoreAlbum {
        id: AlbumId::new(album.id.clone()),
        provider_id: provider_id.to_string(),
        title: album.name.clone(),
        artist: album
            .artist
            .as_ref()
            .map(|a| a.name.clone())
            .unwrap_or_else(|| "Unknown Artist".into()),
        track_count: album.songs_count,
        duration_seconds: None,
    }
}

pub fn map_playlist(playlist: &Playlist, provider_id: &str) -> CorePlaylist {
    CorePlaylist {
        id: PlaylistId::new(playlist.api_key.clone()),
        provider_id: provider_id.to_string(),
        name: playlist.name.clone(),
        description: playlist.description.clone(),
        track_count: playlist.songs_count,
    }
}

#[derive(Debug, Default)]
#[allow(dead_code)]
pub struct MelodeePaging {
    pub current_page: Option<u32>,
    pub page_size: Option<u32>,
}
