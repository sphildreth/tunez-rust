//! Caching functionality for the filesystem provider.
//!
//! Provides metadata caching to speed up library browsing and offline mode support.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};
use serde::{Deserialize, Serialize};
use tunez_core::models::{Album, Playlist, Track};

/// Cache entry with timestamp for expiration
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CacheEntry<T> {
    data: T,
    timestamp: SystemTime,
}

/// Cache configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    /// Maximum cache size in bytes
    pub max_size_bytes: u64,
    /// Maximum age of cache entries in seconds
    pub max_age_seconds: u64,
    /// Whether to cache metadata
    pub cache_metadata: bool,
    /// Whether to cache artwork
    pub cache_artwork: bool,
    /// Whether to cache lyrics
    pub cache_lyrics: bool,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            max_size_bytes: 100 * 1024 * 1024, // 100 MB
            max_age_seconds: 7 * 24 * 60 * 60, // 7 days
            cache_metadata: true,
            cache_artwork: false,
            cache_lyrics: false,
        }
    }
}

/// Metadata cache for filesystem provider
#[derive(Debug, Clone)]
pub struct MetadataCache {
    /// Track cache: file path -> track metadata
    tracks: HashMap<PathBuf, CacheEntry<Track>>,
    /// Album cache: album id -> album metadata
    albums: HashMap<String, CacheEntry<Album>>,
    /// Playlist cache: playlist id -> playlist metadata
    playlists: HashMap<String, CacheEntry<Playlist>>,
    /// Configuration
    config: CacheConfig,
    /// Current size in bytes
    current_size: u64,
}

impl MetadataCache {
    pub fn new(config: CacheConfig) -> Self {
        Self {
            tracks: HashMap::new(),
            albums: HashMap::new(),
            playlists: HashMap::new(),
            config,
            current_size: 0,
        }
    }

    /// Add a track to the cache
    pub fn add_track(&mut self, path: PathBuf, track: Track) {
        if !self.config.cache_metadata {
            return;
        }

        // Calculate approximate size
        let size = std::mem::size_of_val(&track) as u64;
        
        if self.current_size + size > self.config.max_size_bytes {
            self.evict_old_entries();
        }

        let entry = CacheEntry {
            data: track,
            timestamp: SystemTime::now(),
        };

        self.current_size += size;
        self.tracks.insert(path, entry);
    }

    /// Get a track from the cache if it's still valid
    pub fn get_track(&self, path: &PathBuf) -> Option<&Track> {
        if let Some(entry) = self.tracks.get(path) {
            if self.is_entry_valid(entry) {
                return Some(&entry.data);
            }
        }
        None
    }

    /// Add an album to the cache
    pub fn add_album(&mut self, id: String, album: Album) {
        if !self.config.cache_metadata {
            return;
        }

        // Calculate approximate size
        let size = std::mem::size_of_val(&album) as u64;
        
        if self.current_size + size > self.config.max_size_bytes {
            self.evict_old_entries();
        }

        let entry = CacheEntry {
            data: album,
            timestamp: SystemTime::now(),
        };

        self.current_size += size;
        self.albums.insert(id, entry);
    }

    /// Get an album from the cache if it's still valid
    pub fn get_album(&self, id: &str) -> Option<&Album> {
        if let Some(entry) = self.albums.get(id) {
            if self.is_entry_valid(entry) {
                return Some(&entry.data);
            }
        }
        None
    }

    /// Add a playlist to the cache
    pub fn add_playlist(&mut self, id: String, playlist: Playlist) {
        if !self.config.cache_metadata {
            return;
        }

        // Calculate approximate size
        let size = std::mem::size_of_val(&playlist) as u64;
        
        if self.current_size + size > self.config.max_size_bytes {
            self.evict_old_entries();
        }

        let entry = CacheEntry {
            data: playlist,
            timestamp: SystemTime::now(),
        };

        self.current_size += size;
        self.playlists.insert(id, entry);
    }

    /// Get a playlist from the cache if it's still valid
    pub fn get_playlist(&self, id: &str) -> Option<&Playlist> {
        if let Some(entry) = self.playlists.get(id) {
            if self.is_entry_valid(entry) {
                return Some(&entry.data);
            }
        }
        None
    }

    /// Check if a cache entry is still valid (not expired)
    fn is_entry_valid<T>(&self, entry: &CacheEntry<T>) -> bool {
        match entry.timestamp.elapsed() {
            Ok(duration) => duration < Duration::from_secs(self.config.max_age_seconds),
            Err(_) => false, // Clock went backwards
        }
    }

    /// Evict old entries to make space
    fn evict_old_entries(&mut self) {
        let now = SystemTime::now();
        let max_age = Duration::from_secs(self.config.max_age_seconds);

        // Remove old entries
        self.tracks.retain(|_, entry| {
            if let Ok(duration) = entry.timestamp.elapsed() {
                duration < max_age
            } else {
                false // Remove if clock went backwards
            }
        });

        self.albums.retain(|_, entry| {
            if let Ok(duration) = entry.timestamp.elapsed() {
                duration < max_age
            } else {
                false
            }
        });

        self.playlists.retain(|_, entry| {
            if let Ok(duration) = entry.timestamp.elapsed() {
                duration < max_age
            } else {
                false
            }
        });

        // Recalculate size (approximate)
        self.current_size = self.tracks.len() as u64 * 1024 + // Approximate size per entry
                            self.albums.len() as u64 * 512 +
                            self.playlists.len() as u64 * 512;
    }

    /// Clear the entire cache
    pub fn clear(&mut self) {
        self.tracks.clear();
        self.albums.clear();
        self.playlists.clear();
        self.current_size = 0;
    }

    /// Get cache statistics
    pub fn stats(&self) -> CacheStats {
        CacheStats {
            track_count: self.tracks.len(),
            album_count: self.albums.len(),
            playlist_count: self.playlists.len(),
            estimated_size_bytes: self.current_size,
        }
    }
}

/// Cache statistics
#[derive(Debug, Clone)]
pub struct CacheStats {
    pub track_count: usize,
    pub album_count: usize,
    pub playlist_count: usize,
    pub estimated_size_bytes: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use tunez_core::models::{Track, TrackId};

    #[test]
    fn cache_creation() {
        let config = CacheConfig::default();
        let cache = MetadataCache::new(config);
        assert_eq!(cache.stats().track_count, 0);
    }

    #[test]
    fn add_and_get_track() {
        let mut cache = MetadataCache::new(CacheConfig::default());
        let path = PathBuf::from("/test/song.mp3");
        let track = Track {
            id: TrackId::new("test-id"),
            provider_id: "filesystem".into(),
            title: "Test Song".into(),
            artist: "Test Artist".into(),
            album: Some("Test Album".into()),
            duration_seconds: Some(180),
            track_number: Some(1),
        };

        cache.add_track(path.clone(), track.clone());
        let retrieved = cache.get_track(&path);
        assert_eq!(retrieved, Some(&track));
    }

    #[test]
    fn expired_entry_not_returned() {
        use std::thread;
        use std::time::Duration as StdDuration;

        let mut config = CacheConfig::default();
        config.max_age_seconds = 1; // 1 second for testing
        let mut cache = MetadataCache::new(config);
        
        let path = PathBuf::from("/test/song.mp3");
        let track = Track {
            id: TrackId::new("test-id"),
            provider_id: "filesystem".into(),
            title: "Test Song".into(),
            artist: "Test Artist".into(),
            album: Some("Test Album".into()),
            duration_seconds: Some(180),
            track_number: Some(1),
        };

        cache.add_track(path.clone(), track);
        thread::sleep(StdDuration::from_secs(2)); // Sleep longer than max age
        
        let retrieved = cache.get_track(&path);
        assert_eq!(retrieved, None);
    }
}