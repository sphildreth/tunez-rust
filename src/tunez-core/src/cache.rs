//! Cache management and eviction for Tunez.
//!
//! Handles offline download storage and automatic cleanup based on size/age policies.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CacheError {
    #[error("failed to read cache directory: {0}")]
    ReadDir(std::io::Error),
    #[error("failed to get file metadata: {0}")]
    Metadata(std::io::Error),
    #[error("failed to remove file {path}: {error}")]
    RemoveFile {
        path: PathBuf,
        error: std::io::Error,
    },
    #[error("cache directory not found")]
    NotFound,
}

pub type CacheResult<T> = Result<T, CacheError>;

/// Cache eviction policy
#[derive(Debug, Clone)]
pub struct CachePolicy {
    /// Maximum total size in bytes (0 = no limit)
    pub max_size_bytes: u64,
    /// Maximum age of files in seconds (0 = no limit)
    pub max_age_seconds: u64,
    /// Whether to enforce the policy
    pub enabled: bool,
}

impl Default for CachePolicy {
    fn default() -> Self {
        Self {
            max_size_bytes: 10 * 1024 * 1024 * 1024, // 10 GB
            max_age_seconds: 30 * 24 * 60 * 60,      // 30 days
            enabled: true,
        }
    }
}

/// Cache manager for offline downloads
pub struct CacheManager {
    download_dir: PathBuf,
    policy: CachePolicy,
}

impl CacheManager {
    pub fn new(download_dir: PathBuf, policy: CachePolicy) -> Self {
        Self {
            download_dir,
            policy,
        }
    }

    /// Enforce cache eviction policy
    pub fn enforce_policy(&self) -> CacheResult<Vec<PathBuf>> {
        if !self.policy.enabled {
            return Ok(Vec::new());
        }

        if !self.download_dir.exists() {
            return Err(CacheError::NotFound);
        }

        let mut removed = Vec::new();

        // Get all files in download directory
        let entries: Vec<_> = fs::read_dir(&self.download_dir)
            .map_err(CacheError::ReadDir)?
            .filter_map(|entry| entry.ok())
            .filter(|entry| {
                // Only consider files, not directories
                entry.path().is_file()
            })
            .filter_map(|entry| {
                let path = entry.path();
                let metadata = entry.metadata().ok()?;
                let modified = metadata.modified().ok()?;
                let size = metadata.len();
                Some((path, modified, size))
            })
            .collect();

        // First, remove files older than max_age
        if self.policy.max_age_seconds > 0 {
            let now = SystemTime::now();
            let max_age = Duration::from_secs(self.policy.max_age_seconds);

            for (path, modified, _) in &entries {
                if let Ok(age) = now.duration_since(*modified) {
                    if age > max_age {
                        if let Err(e) = fs::remove_file(path) {
                            tracing::warn!("Failed to remove old cache file: {}", e);
                        } else {
                            removed.push(path.clone());
                            tracing::info!("Removed old cache file: {}", path.display());
                        }
                    }
                }
            }
        }

        // Then, if still over size limit, remove oldest files
        if self.policy.max_size_bytes > 0 {
            let total_size: u64 = entries
                .iter()
                .filter(|(path, _, _)| !removed.contains(path))
                .map(|(_, _, size)| size)
                .sum();

            if total_size > self.policy.max_size_bytes {
                // Sort by modification time (oldest first)
                let mut to_remove: Vec<_> = entries
                    .into_iter()
                    .filter(|(path, _, _)| !removed.contains(path))
                    .collect();
                to_remove.sort_by(|a, b| a.1.cmp(&b.1));

                let size_to_free = total_size - self.policy.max_size_bytes;
                let mut freed = 0u64;

                for (path, _, size) in to_remove {
                    if freed >= size_to_free {
                        break;
                    }

                    if let Err(e) = fs::remove_file(&path) {
                        tracing::warn!("Failed to remove cache file: {}", e);
                    } else {
                        removed.push(path.clone());
                        freed += size;
                        tracing::info!("Removed cache file to free space: {}", path.display());
                    }
                }
            }
        }

        Ok(removed)
    }

    /// Get current cache usage statistics
    pub fn get_stats(&self) -> CacheResult<CacheStats> {
        if !self.download_dir.exists() {
            return Ok(CacheStats::default());
        }

        let entries: Vec<_> = fs::read_dir(&self.download_dir)
            .map_err(CacheError::ReadDir)?
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.path().is_file())
            .filter_map(|entry| {
                let metadata = entry.metadata().ok()?;
                let modified = metadata.modified().ok()?;
                let size = metadata.len();
                Some((modified, size))
            })
            .collect();

        let total_size = entries.iter().map(|(_, size)| size).sum();
        let file_count = entries.len() as u64;
        let oldest = entries.iter().map(|(modified, _)| modified).min().cloned();
        let newest = entries.iter().map(|(modified, _)| modified).max().cloned();

        Ok(CacheStats {
            total_size,
            file_count,
            oldest_file: oldest,
            newest_file: newest,
        })
    }

    /// Get the download directory path
    pub fn download_dir(&self) -> &Path {
        &self.download_dir
    }
}

#[derive(Debug, Default)]
pub struct CacheStats {
    pub total_size: u64,
    pub file_count: u64,
    pub oldest_file: Option<SystemTime>,
    pub newest_file: Option<SystemTime>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn test_cache_stats() {
        let dir = tempdir().unwrap();
        let policy = CachePolicy {
            max_size_bytes: 1000,
            max_age_seconds: 60,
            enabled: true,
        };
        let manager = CacheManager::new(dir.path().to_path_buf(), policy);

        // Create some test files
        File::create(dir.path().join("file1.txt"))
            .unwrap()
            .write_all(b"test")
            .unwrap();
        File::create(dir.path().join("file2.txt"))
            .unwrap()
            .write_all(b"test data")
            .unwrap();

        let stats = manager.get_stats().unwrap();
        assert_eq!(stats.file_count, 2);
        assert!(stats.total_size > 0);
    }

    #[test]
    fn test_enforce_policy_removes_old_files() {
        let dir = tempdir().unwrap();
        let policy = CachePolicy {
            max_size_bytes: 1000,
            max_age_seconds: 1, // 1 second
            enabled: true,
        };
        let manager = CacheManager::new(dir.path().to_path_buf(), policy);

        // Create a file
        let file_path = dir.path().join("old.txt");
        File::create(&file_path)
            .unwrap()
            .write_all(b"test")
            .unwrap();

        // Wait a bit
        std::thread::sleep(Duration::from_secs(2));

        // Enforce policy
        let removed = manager.enforce_policy().unwrap();
        assert_eq!(removed.len(), 1);
        assert!(!file_path.exists());
    }
}
