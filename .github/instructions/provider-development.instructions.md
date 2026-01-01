---
description: 'Guidelines for implementing music providers in Tunez, including trait implementation, capability declarations, and testing strategies'
applyTo: '**/providers/**/*.rs,**/*_provider.rs'
---

# Provider Development Guidelines for Tunez

## Overview

Providers are the core abstraction in Tunez for accessing music from different sources (local files, streaming services, APIs). In Phase 1, providers are **built-in Rust crates** compiled into the `tunez` binary.

**Phase 1 Stream Contract:** Providers return a **stream URL only**. The audio subsystem handles decoding and playback.

Canonical references:
- `docs/tunez-requirements.md` (Section 4.1: Provider architecture)
- `docs/filesystem-provider-prd.md` (Example provider implementation)
- `docs/melodee-provider-prd.md` (Example remote provider)

## Provider Trait

All providers must implement the `Provider` trait from `tunez-core`:

```rust
use async_trait::async_trait;

#[async_trait]
pub trait Provider: Send + Sync {
    /// Provider name (e.g., "filesystem", "melodee")
    fn name(&self) -> &str;
    
    /// Provider display name for UI
    fn display_name(&self) -> &str;
    
    /// Declare provider capabilities
    fn capabilities(&self) -> ProviderCapabilities;
    
    /// Search for tracks by query
    async fn search_tracks(&self, query: &str, limit: usize) -> Result<Vec<Track>, ProviderError>;
    
    /// Get track metadata by ID
    async fn get_track(&self, id: &TrackId) -> Result<Track, ProviderError>;
    
    /// Get stream URL for playback (Phase 1 contract)
    async fn get_stream_url(&self, track_id: &TrackId) -> Result<String, ProviderError>;
    
    /// List playlists (if supported by capabilities)
    async fn list_playlists(&self) -> Result<Vec<Playlist>, ProviderError> {
        Err(ProviderError::NotSupported("playlists"))
    }
    
    /// Search playlists (if supported)
    async fn search_playlists(&self, query: &str) -> Result<Vec<Playlist>, ProviderError> {
        Err(ProviderError::NotSupported("playlist_search"))
    }
    
    /// Get tracks in a playlist (if supported)
    async fn get_playlist_tracks(&self, playlist_id: &PlaylistId) -> Result<Vec<Track>, ProviderError> {
        Err(ProviderError::NotSupported("playlists"))
    }
}
```

## Capability Declaration

Providers must declare their capabilities upfront so the UI can adapt gracefully:

```rust
#[derive(Debug, Clone)]
pub struct ProviderCapabilities {
    pub supports_playlists: bool,
    pub supports_lyrics: bool,
    pub supports_offline_download: bool,
    pub supports_search: bool,
    pub supports_albums: bool,
    pub requires_auth: bool,
}

impl ProviderCapabilities {
    pub fn basic() -> Self {
        Self {
            supports_playlists: false,
            supports_lyrics: false,
            supports_offline_download: false,
            supports_search: true,
            supports_albums: false,
            requires_auth: false,
        }
    }
}
```

**Guidelines:**
- Return `ProviderError::NotSupported` for operations the provider cannot perform
- Never panic or return generic errors for missing capabilities
- UI must check capabilities before showing/enabling features
- Document capabilities clearly in provider README or module docs

## Error Handling

Use the `ProviderError` enum for all provider-specific errors:

```rust
#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    #[error("Operation not supported: {0}")]
    NotSupported(&'static str),
    
    #[error("Authentication failed: {0}")]
    AuthError(String),
    
    #[error("Network error: {0}")]
    NetworkError(#[from] reqwest::Error),
    
    #[error("Track not found: {0}")]
    TrackNotFound(String),
    
    #[error("Invalid track ID: {0}")]
    InvalidTrackId(String),
    
    #[error("Provider configuration error: {0}")]
    ConfigError(String),
    
    #[error("Rate limit exceeded; retry after {0}s")]
    RateLimited(u64),
    
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}
```

**Error Handling Principles:**
- Never panic on network failures, decode errors, or missing files
- Return clear, actionable error messages
- For invalid/unreadable tracks: log the error, return an error result, let the player skip to the next track
- For transient errors (rate limits, network): consider retry logic with exponential backoff
- For auth errors: provide clear instructions for user action

## Authentication and Secrets

**Never store plaintext passwords or API keys in config files.**

```rust
// ❌ BAD: Hardcoded or plaintext secret
let api_key = "sk_live_abc123"; // NO!
let password = config.password; // NO! (if in plaintext TOML)

// ✅ GOOD: Use OS keyring via keyring crate
use keyring::Entry;

let entry = Entry::new("tunez", "melodee_api_key")?;
let api_key = entry.get_password()?;

// ✅ GOOD: OAuth tokens via secure storage
let entry = Entry::new("tunez", &format!("{}_refresh_token", provider_name))?;
let refresh_token = entry.get_password()?;
```

**Authentication Guidelines:**
- Use OS keyring (`keyring` crate) for tokens and API keys
- Support OAuth flow for services that require it
- Store only refresh tokens, not access tokens (regenerate on startup)
- Provide clear error messages when auth fails
- Never log secrets (tokens, API keys, passwords, auth headers)

## Stream URL Contract (Phase 1)

Providers return a **stream URL string** that the audio subsystem can fetch and decode:

```rust
async fn get_stream_url(&self, track_id: &TrackId) -> Result<String, ProviderError> {
    match self {
        // Local file provider
        FilesystemProvider => {
            let path = self.resolve_path(track_id)?;
            Ok(format!("file://{}", path.display()))
        }
        
        // Remote streaming provider
        MelodeeProvider => {
            let response = self.client
                .get(&format!("{}/tracks/{}/stream", self.base_url, track_id))
                .bearer_auth(&self.get_token()?)
                .send()
                .await?;
            
            let stream_url = response.json::<StreamResponse>().await?.url;
            Ok(stream_url)
        }
    }
}
```

**Stream URL Guidelines:**
- Return `file://` URLs for local files
- Return `http://` or `https://` URLs for remote streams
- Ensure URLs are valid and accessible (pre-validate if possible)
- Handle signed URLs and expiration (refresh if needed)
- Return URLs that point to actual audio data, not HTML pages

## Async and Concurrency

All provider operations are `async` because they may involve network I/O or disk I/O:

```rust
// ✅ GOOD: Non-blocking async implementation
async fn search_tracks(&self, query: &str, limit: usize) -> Result<Vec<Track>, ProviderError> {
    let response = self.client
        .get(&format!("{}/search", self.base_url))
        .query(&[("q", query), ("limit", &limit.to_string())])
        .send()
        .await?
        .error_for_status()?;
    
    let results = response.json::<SearchResponse>().await?;
    Ok(results.tracks)
}
```

**Concurrency Guidelines:**
- Use `tokio` for async runtime (consistent with Tunez core)
- Prefer `async/await` over manual futures
- Use `tokio::spawn` for background tasks (e.g., refreshing auth tokens)
- Avoid blocking calls; use `tokio::task::spawn_blocking` if necessary
- Respect API rate limits; implement backoff and retry logic

## Configuration

Providers should be configurable via `config.toml`:

```toml
[providers.filesystem]
enabled = true
music_dir = "~/Music"
watch_for_changes = true

[providers.melodee]
enabled = true
base_url = "https://melodee.example.com/api"
profile = "home"
# API key stored in OS keyring, not here
```

**Configuration Guidelines:**
- Use `serde` for config deserialization
- Validate config on load; return `ConfigError` for invalid settings
- Provide sensible defaults for optional fields
- Document all config options in provider README
- Never store secrets in config; use keyring

## Testing Providers

### Unit Tests
Test provider logic without real network/filesystem:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use mockito::Server;
    
    #[tokio::test]
    async fn test_search_tracks() {
        let mut server = Server::new_async().await;
        let mock = server.mock("GET", "/search")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"tracks": [{"id": "1", "title": "Test"}]}"#)
            .create();
        
        let provider = MelodeeProvider::new(server.url());
        let results = provider.search_tracks("test", 10).await.unwrap();
        
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Test");
        mock.assert();
    }
    
    #[tokio::test]
    async fn test_unsupported_operation() {
        let provider = FilesystemProvider::new("/music");
        let result = provider.list_playlists().await;
        
        assert!(matches!(result, Err(ProviderError::NotSupported(_))));
    }
}
```

### Contract Tests
Verify all providers implement the `Provider` trait correctly:

```rust
#[cfg(test)]
mod contract_tests {
    use super::*;
    
    async fn test_provider_contract<P: Provider>(provider: P) {
        // Verify capabilities are declared
        let caps = provider.capabilities();
        assert!(!provider.name().is_empty());
        
        // If search is supported, verify it works
        if caps.supports_search {
            let results = provider.search_tracks("test", 5).await;
            assert!(results.is_ok() || matches!(results, Err(ProviderError::NotSupported(_))));
        }
        
        // Verify unsupported operations return NotSupported
        if !caps.supports_playlists {
            assert!(matches!(
                provider.list_playlists().await,
                Err(ProviderError::NotSupported(_))
            ));
        }
    }
    
    #[tokio::test]
    async fn filesystem_provider_contract() {
        let provider = FilesystemProvider::new("/tmp/music");
        test_provider_contract(provider).await;
    }
}
```

### Integration Tests
Test against real (or staging) services when possible:

```rust
#[cfg(test)]
#[ignore] // Only run with --ignored flag
mod integration_tests {
    use super::*;
    
    #[tokio::test]
    async fn test_melodee_real_search() {
        let provider = MelodeeProvider::from_env(); // Load from env vars
        let results = provider.search_tracks("beethoven", 5).await.unwrap();
        assert!(!results.is_empty());
    }
}
```

## Performance Considerations

### Caching
Implement caching for expensive operations:

```rust
use std::sync::Arc;
use tokio::sync::RwLock;
use std::collections::HashMap;

pub struct CachedProvider {
    inner: Arc<dyn Provider>,
    cache: Arc<RwLock<HashMap<String, Vec<Track>>>>,
}

impl CachedProvider {
    async fn search_tracks_cached(&self, query: &str, limit: usize) -> Result<Vec<Track>, ProviderError> {
        let cache_key = format!("{}:{}", query, limit);
        
        // Check cache
        {
            let cache = self.cache.read().await;
            if let Some(cached) = cache.get(&cache_key) {
                return Ok(cached.clone());
            }
        }
        
        // Fetch and cache
        let results = self.inner.search_tracks(query, limit).await?;
        {
            let mut cache = self.cache.write().await;
            cache.insert(cache_key, results.clone());
        }
        
        Ok(results)
    }
}
```

### Pagination
For large result sets, implement pagination:

```rust
async fn search_tracks_paginated(
    &self,
    query: &str,
    page: usize,
    page_size: usize,
) -> Result<PaginatedResult<Track>, ProviderError> {
    let offset = page * page_size;
    let response = self.client
        .get(&format!("{}/search", self.base_url))
        .query(&[
            ("q", query),
            ("limit", &page_size.to_string()),
            ("offset", &offset.to_string()),
        ])
        .send()
        .await?;
    
    // ... parse and return paginated results
}
```

## Security Considerations

- **Input validation:** Sanitize all user input before using in queries or file paths
- **Path traversal:** Validate file paths; reject `..` or absolute paths outside allowed directories
- **SSRF prevention:** If provider allows custom URLs, validate against allow-list
- **Rate limiting:** Respect API rate limits; implement exponential backoff
- **Token refresh:** Refresh OAuth tokens before expiry; handle 401 errors gracefully
- **Audit logging:** Log all authentication attempts and failures (without logging secrets)

## Common Patterns

### Pattern: Filesystem Provider with Metadata Parsing
```rust
pub struct FilesystemProvider {
    music_dir: PathBuf,
}

impl FilesystemProvider {
    async fn scan_directory(&self) -> Result<Vec<Track>, ProviderError> {
        let mut tracks = Vec::new();
        
        let entries = tokio::fs::read_dir(&self.music_dir).await?;
        // ... iterate and parse metadata with lofty or similar
        
        Ok(tracks)
    }
}
```

### Pattern: Remote Provider with OAuth
```rust
pub struct OAuthProvider {
    client: reqwest::Client,
    base_url: String,
    access_token: Arc<RwLock<Option<String>>>,
}

impl OAuthProvider {
    async fn ensure_authenticated(&self) -> Result<(), ProviderError> {
        let token = self.access_token.read().await;
        if token.is_some() {
            return Ok(());
        }
        drop(token);
        
        // Refresh token flow
        let refresh_token = self.get_refresh_token_from_keyring()?;
        let new_token = self.exchange_refresh_token(&refresh_token).await?;
        
        let mut token = self.access_token.write().await;
        *token = Some(new_token);
        Ok(())
    }
}
```

## Anti-Patterns (Avoid These)

❌ **Blocking I/O in async functions:**
```rust
async fn search_tracks(&self, query: &str) -> Result<Vec<Track>, ProviderError> {
    let data = std::fs::read_to_string("cache.json")?; // BLOCKS!
    // Use tokio::fs::read_to_string instead
}
```

❌ **Panicking on errors:**
```rust
async fn get_track(&self, id: &TrackId) -> Result<Track, ProviderError> {
    let track = self.find_track(id).expect("track not found"); // PANIC!
    // Return Err(ProviderError::TrackNotFound(...)) instead
}
```

❌ **Hardcoded secrets:**
```rust
let api_key = "sk_live_abc123"; // NO! Use keyring
```

❌ **Ignoring capabilities:**
```rust
// BAD: Implementing unsupported feature
async fn list_playlists(&self) -> Result<Vec<Playlist>, ProviderError> {
    Ok(Vec::new()) // Should return NotSupported error
}
```

## References

- Tunez PRD: `docs/tunez-requirements.md` (Section 4.1)
- Filesystem Provider PRD: `docs/filesystem-provider-prd.md`
- Melodee Provider PRD: `docs/melodee-provider-prd.md`
- [`async-trait` crate](https://docs.rs/async-trait/)
- [`keyring` crate](https://docs.rs/keyring/)
- [`reqwest` crate](https://docs.rs/reqwest/)
