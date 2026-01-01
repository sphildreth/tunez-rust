use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct SongPagedResponse {
    pub data: Vec<Song>,
    pub meta: PaginationMetadata,
}

#[derive(Debug, Deserialize)]
pub struct AlbumPagedResponse {
    pub data: Vec<Album>,
    pub meta: PaginationMetadata,
}

#[derive(Debug, Deserialize)]
pub struct PlaylistPagedResponse {
    pub data: Vec<Playlist>,
    pub meta: PaginationMetadata,
}

#[derive(Debug, Deserialize)]
pub struct PaginationMetadata {
    pub totalCount: Option<u32>,
    pub pageSize: Option<u32>,
    pub currentPage: Option<u32>,
    pub totalPages: Option<u32>,
    pub hasPrevious: Option<bool>,
    pub hasNext: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct Song {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub durationMs: Option<u64>,
    #[serde(default)]
    pub streamUrl: Option<String>,
    #[serde(default)]
    pub artist: Option<ArtistRef>,
    #[serde(default)]
    pub album: Option<AlbumRef>,
    #[serde(default)]
    pub thumbnailUrl: Option<String>,
    #[serde(default)]
    pub imageUrl: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ArtistRef {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct AlbumRef {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct Playlist {
    pub apiKey: String,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub songsCount: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct Album {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub artist: Option<ArtistRef>,
    #[serde(default)]
    pub songsCount: Option<u32>,
}
