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
    #[serde(rename = "totalCount")]
    pub total_count: Option<u32>,
    #[serde(rename = "pageSize")]
    pub page_size: Option<u32>,
    #[serde(rename = "currentPage")]
    pub current_page: Option<u32>,
    #[serde(rename = "totalPages")]
    pub total_pages: Option<u32>,
    #[serde(rename = "hasPrevious")]
    pub has_previous: Option<bool>,
    #[serde(rename = "hasNext")]
    pub has_next: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct Song {
    pub id: String,
    pub title: String,
    #[serde(rename = "durationMs", default)]
    pub duration_ms: Option<u64>,
    #[serde(rename = "streamUrl", default)]
    pub stream_url: Option<String>,
    #[serde(default)]
    pub artist: Option<ArtistRef>,
    #[serde(default)]
    pub album: Option<AlbumRef>,
    #[serde(rename = "thumbnailUrl", default)]
    pub thumbnail_url: Option<String>,
    #[serde(rename = "imageUrl", default)]
    pub image_url: Option<String>,
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
    #[serde(rename = "apiKey")]
    pub api_key: String,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(rename = "songsCount", default)]
    pub songs_count: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct Album {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub artist: Option<ArtistRef>,
    #[serde(rename = "songsCount", default)]
    pub songs_count: Option<u32>,
}
#[derive(Debug, Deserialize)]
pub struct Lyrics {
    #[serde(rename = "plainText")]
    pub plain_text: String,
}
