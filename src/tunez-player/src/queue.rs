use rand::{seq::SliceRandom, thread_rng};
use serde::{Deserialize, Serialize};
use tunez_core::Track;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct QueueId(pub u64);

impl QueueId {
    pub(crate) fn new(id: u64) -> Self {
        QueueId(id)
    }

    fn next(seed: &mut u64) -> Self {
        let id = *seed;
        *seed = seed.saturating_add(1);
        QueueId(id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueueItem {
    pub id: QueueId,
    pub track: Track,
}

#[derive(Debug, Default, Clone)]
pub struct Queue {
    items: Vec<QueueItem>,
    current: Option<usize>,
    next_id: u64,
}

impl Queue {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn current(&self) -> Option<&QueueItem> {
        self.current.and_then(|idx| self.items.get(idx))
    }

    pub fn items(&self) -> &[QueueItem] {
        &self.items
    }

    pub fn enqueue_back(&mut self, track: Track) -> QueueId {
        let id = QueueId::next(&mut self.next_id);
        self.items.push(QueueItem { id, track });
        id
    }

    pub fn enqueue_next(&mut self, track: Track) -> QueueId {
        let id = QueueId::next(&mut self.next_id);
        let insert_at = self.current.map(|idx| idx + 1).unwrap_or(0);
        self.items.insert(insert_at, QueueItem { id, track });
        if let Some(current) = self.current.as_mut() {
            if insert_at <= *current {
                *current += 1;
            }
        }
        id
    }

    pub fn remove(&mut self, id: QueueId) -> Option<QueueItem> {
        let idx = self.items.iter().position(|item| item.id == id)?;
        let removed = self.items.remove(idx);
        match self.current {
            Some(current_idx) if idx < current_idx => self.current = Some(current_idx - 1),
            Some(current_idx) if idx == current_idx => {
                if self.items.is_empty() {
                    self.current = None;
                } else if idx < self.items.len() {
                    self.current = Some(idx);
                } else {
                    self.current = Some(self.items.len() - 1);
                }
            }
            _ => {}
        }
        Some(removed)
    }

    pub fn clear(&mut self) {
        self.items.clear();
        self.current = None;
    }

    pub fn select_first(&mut self) -> Option<&QueueItem> {
        self.select_index(0)
    }

    pub fn select_index(&mut self, index: usize) -> Option<&QueueItem> {
        if index < self.items.len() {
            self.current = Some(index);
            self.current()
        } else {
            None
        }
    }

    pub fn advance(&mut self) -> Option<&QueueItem> {
        match self.current {
            Some(idx) if idx + 1 < self.items.len() => {
                self.current = Some(idx + 1);
                self.current()
            }
            _ => {
                self.current = None;
                None
            }
        }
    }

    pub fn previous(&mut self) -> Option<&QueueItem> {
        match self.current {
            Some(idx) if idx > 0 => {
                self.current = Some(idx - 1);
                self.current()
            }
            _ => None,
        }
    }

    pub fn reset_current(&mut self) {
        self.current = None;
    }

    pub fn shuffle_preserve_current(&mut self) {
        if self.items.len() <= 1 {
            return;
        }

        if let Some(current_idx) = self.current {
            let current = self.items.remove(current_idx);
            let mut rng = thread_rng();
            self.items.shuffle(&mut rng);
            self.items.insert(0, current);
            self.current = Some(0);
        } else {
            let mut rng = thread_rng();
            self.items.shuffle(&mut rng);
        }
    }

    /// Get the next_id value (for persistence).
    pub(crate) fn next_id(&self) -> u64 {
        self.next_id
    }

    /// Reconstruct a Queue from persisted state.
    pub(crate) fn from_persisted(
        items: Vec<QueueItem>,
        current_index: Option<usize>,
        next_id: u64,
    ) -> Self {
        // Validate current_index
        let current = match current_index {
            Some(idx) if idx < items.len() => Some(idx),
            _ => None,
        };

        Self {
            items,
            current,
            next_id,
        }
    }
}

#[cfg(test)]
mod tests {
    use tunez_core::{Track, TrackId};

    use super::*;

    fn track(id: &str) -> Track {
        Track {
            id: TrackId::new(id),
            provider_id: "test".into(),
            title: id.to_string(),
            artist: "artist".into(),
            album: None,
            duration_seconds: None,
            track_number: None,
        }
    }

    #[test]
    fn enqueue_and_select_first() {
        let mut queue = Queue::new();
        let first = queue.enqueue_back(track("one"));
        queue.enqueue_back(track("two"));

        let selected = queue.select_first().unwrap();
        assert_eq!(selected.id, first);
        assert_eq!(queue.len(), 2);
    }

    #[test]
    fn enqueue_next_inserts_after_current() {
        let mut queue = Queue::new();
        queue.enqueue_back(track("one"));
        queue.enqueue_back(track("two"));
        queue.select_first();
        let next = queue.enqueue_next(track("inserted"));

        assert_eq!(queue.items()[1].id, next);
        assert_eq!(queue.items()[1].track.title, "inserted");
    }

    #[test]
    fn remove_updates_current_pointer() {
        let mut queue = Queue::new();
        let first = queue.enqueue_back(track("one"));
        let second = queue.enqueue_back(track("two"));
        queue.select_first();

        queue.remove(first);
        let current = queue.current().unwrap();
        assert_eq!(current.id, second);
    }

    #[test]
    fn shuffle_keeps_current_at_front() {
        let mut queue = Queue::new();
        let first = queue.enqueue_back(track("one"));
        queue.enqueue_back(track("two"));
        queue.enqueue_back(track("three"));
        queue.select_first();

        queue.shuffle_preserve_current();
        let current = queue.current().unwrap();
        assert_eq!(current.id, first);
        assert_eq!(queue.items()[0].id, first);
        assert_eq!(queue.len(), 3);
    }

    #[test]
    fn advance_clears_current_at_end() {
        let mut queue = Queue::new();
        queue.enqueue_back(track("one"));
        queue.enqueue_back(track("two"));
        queue.select_first();

        let second = queue.advance().unwrap();
        assert_eq!(second.track.title, "two");
        assert!(queue.advance().is_none());
        assert!(queue.current().is_none());
    }
}
