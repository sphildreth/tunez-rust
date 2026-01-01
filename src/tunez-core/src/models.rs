use serde::{Deserialize, Serialize};

/// A provider-scoped track identifier.
///
/// Providers MUST treat this as an opaque, case-sensitive identifier that is
/// stable across runs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub struct TrackId(pub String);

impl TrackId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }
}

impl AsRef<str> for TrackId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl From<&str> for TrackId {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl From<String> for TrackId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

/// A provider-scoped album identifier.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Hash)]
pub struct AlbumId(pub String);

impl AlbumId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }
}

impl From<&str> for AlbumId {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl From<String> for AlbumId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

/// A provider-scoped playlist identifier.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub struct PlaylistId(pub String);

impl PlaylistId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }
}

impl From<&str> for PlaylistId {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl From<String> for PlaylistId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

/// The minimal track metadata required by Tunez for UI + scrobbling.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Track {
    pub id: TrackId,
    pub provider_id: String,
    pub title: String,
    pub artist: String,
    pub album: Option<String>,
    /// Duration in seconds when known.
    pub duration_seconds: Option<u32>,
    /// Track number within album when known.
    pub track_number: Option<u32>,
}

/// Minimal album metadata to support browse/detail views.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Album {
    pub id: AlbumId,
    pub provider_id: String,
    pub title: String,
    pub artist: String,
    pub track_count: Option<u32>,
    pub duration_seconds: Option<u32>,
}

/// Minimal playlist metadata to support browse/detail views.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Playlist {
    pub id: PlaylistId,
    pub provider_id: String,
    pub name: String,
    pub description: Option<String>,
    pub track_count: Option<u32>,
}

/// Stream URL returned by a provider. Providers MUST return a URL/handle; Tunez
/// is responsible for reading/decoding the stream.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StreamUrl(pub String);

impl StreamUrl {
    pub fn new(url: impl Into<String>) -> Self {
        Self(url.into())
    }
}

impl AsRef<str> for StreamUrl {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl From<&str> for StreamUrl {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl From<String> for StreamUrl {
    fn from(value: String) -> Self {
        Self(value)
    }
}

/// Paging request represented as offset/limit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PageRequest {
    pub offset: u32,
    pub limit: u32,
}

impl PageRequest {
    pub fn new(offset: u32, limit: u32) -> Self {
        Self { offset, limit }
    }

    pub fn first_page(limit: u32) -> Self {
        Self { offset: 0, limit }
    }
}

/// Cursor returned from a paged provider call.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PageCursor(pub String);

/// A single page of items plus an optional cursor for continuation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Page<T> {
    pub items: Vec<T>,
    pub next: Option<PageCursor>,
}

impl<T> Page<T> {
    pub fn single_page(items: Vec<T>) -> Self {
        Self { items, next: None }
    }
}
