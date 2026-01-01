//! Queue persistence to disk.
//!
//! Handles saving and loading the playback queue state to survive restarts.
//! Includes corruption handling and last-known-good backups.

use crate::queue::{Queue, QueueId, QueueItem};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{self, BufReader, BufWriter};
use std::path::{Path, PathBuf};
use thiserror::Error;
use tunez_core::Track;

/// Version of the queue persistence format.
const PERSISTENCE_VERSION: u32 = 1;

/// Maximum number of items allowed in a persisted queue.
/// Prevents memory exhaustion from maliciously crafted files.
const MAX_QUEUE_ITEMS: usize = 10_000;

/// Maximum allowed file size for queue persistence (10 MB).
/// Prevents loading extremely large files that could exhaust memory.
const MAX_QUEUE_FILE_SIZE: u64 = 10 * 1024 * 1024;

/// Queue persistence errors.
#[derive(Debug, Error)]
pub enum QueuePersistenceError {
    #[error("failed to create queue directory {path}: {source}")]
    CreateDir { path: PathBuf, source: io::Error },

    #[error("failed to write queue file {path}: {source}")]
    Write { path: PathBuf, source: io::Error },

    #[error("failed to read queue file {path}: {source}")]
    Read { path: PathBuf, source: io::Error },

    #[error("corrupt queue file {path}: {reason}")]
    Corrupt { path: PathBuf, reason: String },

    #[error("queue format version {found} is not supported (expected {expected})")]
    UnsupportedVersion { found: u32, expected: u32 },

    #[error("queue file too large ({size} bytes, max {max} bytes)")]
    FileTooLarge { size: u64, max: u64 },

    #[error("queue has too many items ({count}, max {max})")]
    TooManyItems { count: usize, max: usize },
}

pub type QueuePersistenceResult<T> = Result<T, QueuePersistenceError>;

/// Serialized representation of the queue.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct PersistedQueue {
    version: u32,
    items: Vec<PersistedQueueItem>,
    current_index: Option<usize>,
    next_id: u64,
}

/// Serialized queue item.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct PersistedQueueItem {
    id: u64,
    track: Track,
}

/// Queue persistence manager.
///
/// Handles saving and loading queue state with backup support.
#[derive(Debug, Clone)]
pub struct QueuePersistence {
    /// Path to the primary queue file.
    queue_path: PathBuf,
    /// Path to the backup file.
    backup_path: PathBuf,
    /// Path to keep corrupt files for debugging.
    corrupt_path: PathBuf,
}

impl QueuePersistence {
    /// Create a new persistence manager for the given data directory.
    pub fn new(data_dir: &Path) -> Self {
        Self {
            queue_path: data_dir.join("queue.json"),
            backup_path: data_dir.join("queue.backup.json"),
            corrupt_path: data_dir.join("queue.corrupt.json"),
        }
    }

    /// Save the queue state to disk.
    ///
    /// Creates a backup of the previous state before writing.
    pub fn save(&self, queue: &Queue) -> QueuePersistenceResult<()> {
        // Ensure directory exists
        if let Some(parent) = self.queue_path.parent() {
            fs::create_dir_all(parent).map_err(|source| QueuePersistenceError::CreateDir {
                path: parent.to_path_buf(),
                source,
            })?;
        }

        // Create backup of existing file before overwriting
        if self.queue_path.exists() {
            if let Err(e) = fs::copy(&self.queue_path, &self.backup_path) {
                tracing::warn!(
                    error = %e,
                    "failed to create queue backup; continuing anyway"
                );
            }
        }

        // Serialize and write
        let persisted = self.queue_to_persisted(queue);
        let file =
            fs::File::create(&self.queue_path).map_err(|source| QueuePersistenceError::Write {
                path: self.queue_path.clone(),
                source,
            })?;
        let writer = BufWriter::new(file);
        serde_json::to_writer_pretty(writer, &persisted).map_err(|e| {
            QueuePersistenceError::Write {
                path: self.queue_path.clone(),
                source: io::Error::other(e),
            }
        })?;

        tracing::debug!(
            items = queue.len(),
            path = %self.queue_path.display(),
            "saved queue to disk"
        );

        Ok(())
    }

    /// Load the queue from disk.
    ///
    /// On corruption:
    /// - Moves the corrupt file for debugging
    /// - Shows a warning
    /// - Returns an empty queue
    pub fn load(&self) -> QueuePersistenceResult<Queue> {
        if !self.queue_path.exists() {
            return Ok(Queue::new());
        }

        match self.try_load(&self.queue_path) {
            Ok(queue) => Ok(queue),
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    path = %self.queue_path.display(),
                    "queue file is corrupt or unreadable; starting with empty queue"
                );

                // Preserve corrupt file for debugging
                if let Err(move_err) = fs::rename(&self.queue_path, &self.corrupt_path) {
                    tracing::warn!(
                        error = %move_err,
                        "failed to preserve corrupt queue file"
                    );
                }

                // Try to recover from backup
                if self.backup_path.exists() {
                    tracing::info!(
                        path = %self.backup_path.display(),
                        "attempting to recover from backup"
                    );
                    match self.try_load(&self.backup_path) {
                        Ok(queue) => {
                            tracing::info!(items = queue.len(), "recovered queue from backup");
                            return Ok(queue);
                        }
                        Err(backup_err) => {
                            tracing::warn!(
                                error = %backup_err,
                                "backup also corrupt; starting fresh"
                            );
                        }
                    }
                }

                // Start with empty queue
                Ok(Queue::new())
            }
        }
    }

    /// Attempt to load a queue from a specific file path.
    fn try_load(&self, path: &Path) -> QueuePersistenceResult<Queue> {
        // Check file size before loading to prevent memory exhaustion
        let metadata = fs::metadata(path).map_err(|source| QueuePersistenceError::Read {
            path: path.to_path_buf(),
            source,
        })?;
        let file_size = metadata.len();
        if file_size > MAX_QUEUE_FILE_SIZE {
            return Err(QueuePersistenceError::FileTooLarge {
                size: file_size,
                max: MAX_QUEUE_FILE_SIZE,
            });
        }

        let file = fs::File::open(path).map_err(|source| QueuePersistenceError::Read {
            path: path.to_path_buf(),
            source,
        })?;
        let reader = BufReader::new(file);
        let persisted: PersistedQueue =
            serde_json::from_reader(reader).map_err(|e| QueuePersistenceError::Corrupt {
                path: path.to_path_buf(),
                reason: e.to_string(),
            })?;

        // Version check
        if persisted.version != PERSISTENCE_VERSION {
            return Err(QueuePersistenceError::UnsupportedVersion {
                found: persisted.version,
                expected: PERSISTENCE_VERSION,
            });
        }

        // Bounds check on item count
        if persisted.items.len() > MAX_QUEUE_ITEMS {
            return Err(QueuePersistenceError::TooManyItems {
                count: persisted.items.len(),
                max: MAX_QUEUE_ITEMS,
            });
        }

        let queue = self.persisted_to_queue(persisted);

        tracing::debug!(
            items = queue.len(),
            path = %path.display(),
            "loaded queue from disk"
        );

        Ok(queue)
    }

    /// Convert a Queue to its persisted representation.
    fn queue_to_persisted(&self, queue: &Queue) -> PersistedQueue {
        let items = queue
            .items()
            .iter()
            .map(|item| PersistedQueueItem {
                id: item.id.0,
                track: item.track.clone(),
            })
            .collect();

        let current_index = queue
            .current()
            .and_then(|current| queue.items().iter().position(|item| item.id == current.id));

        PersistedQueue {
            version: PERSISTENCE_VERSION,
            items,
            current_index,
            next_id: queue.next_id(),
        }
    }

    /// Convert a persisted representation back to a Queue.
    fn persisted_to_queue(&self, persisted: PersistedQueue) -> Queue {
        let items: Vec<QueueItem> = persisted
            .items
            .into_iter()
            .map(|item| QueueItem {
                id: QueueId::new(item.id),
                track: item.track,
            })
            .collect();

        Queue::from_persisted(items, persisted.current_index, persisted.next_id)
    }

    /// Check if a persisted queue exists.
    pub fn exists(&self) -> bool {
        self.queue_path.exists()
    }

    /// Delete the persisted queue (and backup).
    pub fn clear(&self) -> QueuePersistenceResult<()> {
        if self.queue_path.exists() {
            fs::remove_file(&self.queue_path).map_err(|source| QueuePersistenceError::Write {
                path: self.queue_path.clone(),
                source,
            })?;
        }
        if self.backup_path.exists() {
            let _ = fs::remove_file(&self.backup_path);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use tunez_core::TrackId;

    fn test_track(id: &str) -> Track {
        Track {
            id: TrackId::new(id),
            provider_id: "test".into(),
            title: format!("Track {}", id),
            artist: "Test Artist".into(),
            album: Some("Test Album".into()),
            duration_seconds: Some(180),
            track_number: Some(1),
        }
    }

    #[test]
    fn save_and_load_roundtrip() {
        let dir = tempdir().unwrap();
        let persistence = QueuePersistence::new(dir.path());

        let mut queue = Queue::new();
        queue.enqueue_back(test_track("1"));
        queue.enqueue_back(test_track("2"));
        queue.enqueue_back(test_track("3"));
        queue.select_first();
        queue.advance(); // current is now track 2

        persistence.save(&queue).unwrap();

        let loaded = persistence.load().unwrap();
        assert_eq!(loaded.len(), 3);
        assert_eq!(loaded.current().unwrap().track.id.0, "2");
    }

    #[test]
    fn load_empty_on_no_file() {
        let dir = tempdir().unwrap();
        let persistence = QueuePersistence::new(dir.path());

        let loaded = persistence.load().unwrap();
        assert!(loaded.is_empty());
    }

    #[test]
    fn corrupt_file_returns_empty_queue() {
        let dir = tempdir().unwrap();
        let persistence = QueuePersistence::new(dir.path());

        // Write garbage to the queue file
        fs::write(&persistence.queue_path, "{ invalid json }").unwrap();

        let loaded = persistence.load().unwrap();
        assert!(loaded.is_empty());

        // Corrupt file should be preserved
        assert!(persistence.corrupt_path.exists());
    }

    #[test]
    fn backup_is_created_on_save() {
        let dir = tempdir().unwrap();
        let persistence = QueuePersistence::new(dir.path());

        let mut queue = Queue::new();
        queue.enqueue_back(test_track("1"));
        persistence.save(&queue).unwrap();

        // Save again to trigger backup
        queue.enqueue_back(test_track("2"));
        persistence.save(&queue).unwrap();

        assert!(persistence.backup_path.exists());
    }

    #[test]
    fn recovery_from_backup() {
        let dir = tempdir().unwrap();
        let persistence = QueuePersistence::new(dir.path());

        // Create a valid queue and save it
        let mut queue = Queue::new();
        queue.enqueue_back(test_track("original"));
        persistence.save(&queue).unwrap();

        // Create another save (this will backup the first)
        queue.enqueue_back(test_track("updated"));
        persistence.save(&queue).unwrap();

        // Corrupt the main file
        fs::write(&persistence.queue_path, "corrupt").unwrap();

        // Load should recover from backup (which has the "updated" version)
        let loaded = persistence.load().unwrap();
        // The backup was created before the second save, so it has only "original"
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded.items()[0].track.title, "Track original");
    }

    #[test]
    fn rejects_oversized_file() {
        let dir = tempdir().unwrap();
        let persistence = QueuePersistence::new(dir.path());

        // Create a file larger than MAX_QUEUE_FILE_SIZE
        // We'll use a smaller test limit by writing a large JSON array
        let large_data = "x".repeat((MAX_QUEUE_FILE_SIZE + 1) as usize);
        fs::write(&persistence.queue_path, large_data).unwrap();

        // Should fall back to empty queue (file too large is treated as corruption)
        let loaded = persistence.load().unwrap();
        assert!(loaded.is_empty());
    }

    #[test]
    fn rejects_too_many_items() {
        let dir = tempdir().unwrap();
        let persistence = QueuePersistence::new(dir.path());

        // Create a valid JSON with too many items
        let items: Vec<_> = (0..MAX_QUEUE_ITEMS + 1)
            .map(|i| PersistedQueueItem {
                id: i as u64,
                track: test_track(&i.to_string()),
            })
            .collect();
        let persisted = PersistedQueue {
            version: PERSISTENCE_VERSION,
            items,
            current_index: None,
            next_id: (MAX_QUEUE_ITEMS + 1) as u64,
        };
        let json = serde_json::to_string(&persisted).unwrap();
        fs::write(&persistence.queue_path, json).unwrap();

        // Should fall back to empty queue
        let loaded = persistence.load().unwrap();
        assert!(loaded.is_empty());
    }
}
