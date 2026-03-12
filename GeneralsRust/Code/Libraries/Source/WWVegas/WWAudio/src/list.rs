//! Audio-specific list and collection utilities.

use crate::error::Result;
use parking_lot::RwLock;
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

/// Thread-safe audio list with priority ordering
pub struct AudioList<T> {
    items: Arc<RwLock<Vec<AudioListItem<T>>>>,
    capacity: usize,
    auto_sort: bool,
}

/// Audio list item with priority
#[derive(Debug, Clone)]
pub struct AudioListItem<T> {
    pub data: T,
    pub priority: crate::Priority,
    pub timestamp: std::time::Instant,
}

/// Audio queue for FIFO operations
pub struct AudioQueue<T> {
    queue: Arc<RwLock<VecDeque<T>>>,
    max_size: usize,
}

/// Audio hash map with priority-based eviction
pub struct AudioHashMap<K, V> {
    map: Arc<RwLock<HashMap<K, AudioMapEntry<V>>>>,
    max_entries: usize,
}

/// Audio map entry with metadata
#[derive(Debug, Clone)]
pub struct AudioMapEntry<V> {
    pub value: V,
    pub priority: crate::Priority,
    pub access_count: u64,
    pub last_accessed: std::time::Instant,
}

impl<T> AudioList<T> {
    /// Create new audio list
    pub fn new(capacity: usize, auto_sort: bool) -> Self {
        Self {
            items: Arc::new(RwLock::new(Vec::with_capacity(capacity))),
            capacity,
            auto_sort,
        }
    }

    /// Add item with priority
    pub fn add(&self, data: T, priority: crate::Priority) -> Result<()> {
        let mut items = self.items.write();

        if items.len() >= self.capacity {
            // Remove lowest priority item
            if let Some(min_idx) = items
                .iter()
                .enumerate()
                .min_by_key(|(_, item)| item.priority)
                .map(|(idx, _)| idx)
            {
                items.remove(min_idx);
            }
        }

        let item = AudioListItem {
            data,
            priority,
            timestamp: std::time::Instant::now(),
        };

        items.push(item);

        if self.auto_sort {
            items.sort_by(|a, b| b.priority.cmp(&a.priority));
        }

        Ok(())
    }

    /// Remove item by index
    pub fn remove(&self, index: usize) -> Option<T> {
        let mut items = self.items.write();
        if index < items.len() {
            Some(items.remove(index).data)
        } else {
            None
        }
    }

    /// Get item by index (read-only)
    pub fn get(&self, index: usize) -> Option<AudioListItem<T>>
    where
        T: Clone,
    {
        let items = self.items.read();
        items.get(index).cloned()
    }

    /// Get all items with minimum priority
    pub fn get_by_priority(&self, min_priority: crate::Priority) -> Vec<AudioListItem<T>>
    where
        T: Clone,
    {
        let items = self.items.read();
        items
            .iter()
            .filter(|item| item.priority >= min_priority)
            .cloned()
            .collect()
    }

    /// Clear all items
    pub fn clear(&self) {
        let mut items = self.items.write();
        items.clear();
    }

    /// Get current size
    pub fn len(&self) -> usize {
        self.items.read().len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.items.read().is_empty()
    }

    /// Manually sort by priority
    pub fn sort(&self) {
        let mut items = self.items.write();
        items.sort_by(|a, b| b.priority.cmp(&a.priority));
    }

    /// Remove items older than duration
    pub fn cleanup_old(&self, max_age: std::time::Duration) -> usize {
        let mut items = self.items.write();
        let now = std::time::Instant::now();
        let initial_len = items.len();

        items.retain(|item| now.duration_since(item.timestamp) <= max_age);

        initial_len - items.len()
    }
}

impl<T> AudioQueue<T> {
    /// Create new audio queue
    pub fn new(max_size: usize) -> Self {
        Self {
            queue: Arc::new(RwLock::new(VecDeque::with_capacity(max_size))),
            max_size,
        }
    }

    /// Push item to back of queue
    pub fn push(&self, item: T) -> Result<()> {
        let mut queue = self.queue.write();

        if queue.len() >= self.max_size {
            queue.pop_front(); // Remove oldest
        }

        queue.push_back(item);
        Ok(())
    }

    /// Pop item from front of queue
    pub fn pop(&self) -> Option<T> {
        let mut queue = self.queue.write();
        queue.pop_front()
    }

    /// Peek at front item without removing
    pub fn peek(&self) -> Option<T>
    where
        T: Clone,
    {
        let queue = self.queue.read();
        queue.front().cloned()
    }

    /// Get queue size
    pub fn len(&self) -> usize {
        self.queue.read().len()
    }

    /// Check if queue is empty
    pub fn is_empty(&self) -> bool {
        self.queue.read().is_empty()
    }

    /// Clear all items
    pub fn clear(&self) {
        let mut queue = self.queue.write();
        queue.clear();
    }
}

impl<K, V> AudioHashMap<K, V>
where
    K: Eq + std::hash::Hash + Clone,
    V: Clone,
{
    /// Create new audio hash map
    pub fn new(max_entries: usize) -> Self {
        Self {
            map: Arc::new(RwLock::new(HashMap::with_capacity(max_entries))),
            max_entries,
        }
    }

    /// Insert item with priority
    pub fn insert(&self, key: K, value: V, priority: crate::Priority) -> Result<()> {
        let mut map = self.map.write();

        // If at capacity, evict lowest priority item
        if map.len() >= self.max_entries && !map.contains_key(&key) {
            if let Some((evict_key, _)) = map
                .iter()
                .min_by_key(|(_, entry)| (entry.priority, entry.access_count))
                .map(|(k, v)| (k.clone(), v.clone()))
            {
                map.remove(&evict_key);
            }
        }

        let entry = AudioMapEntry {
            value,
            priority,
            access_count: 1,
            last_accessed: std::time::Instant::now(),
        };

        map.insert(key, entry);
        Ok(())
    }

    /// Get item and update access statistics
    pub fn get(&self, key: &K) -> Option<V> {
        let mut map = self.map.write();
        if let Some(entry) = map.get_mut(key) {
            entry.access_count += 1;
            entry.last_accessed = std::time::Instant::now();
            Some(entry.value.clone())
        } else {
            None
        }
    }

    /// Remove item
    pub fn remove(&self, key: &K) -> Option<V> {
        let mut map = self.map.write();
        map.remove(key).map(|entry| entry.value)
    }

    /// Check if key exists
    pub fn contains_key(&self, key: &K) -> bool {
        let map = self.map.read();
        map.contains_key(key)
    }

    /// Get map size
    pub fn len(&self) -> usize {
        self.map.read().len()
    }

    /// Check if map is empty
    pub fn is_empty(&self) -> bool {
        self.map.read().is_empty()
    }

    /// Clear all items
    pub fn clear(&self) {
        let mut map = self.map.write();
        map.clear();
    }

    /// Get all keys
    pub fn keys(&self) -> Vec<K> {
        let map = self.map.read();
        map.keys().cloned().collect()
    }
}

impl<T> Default for AudioList<T> {
    fn default() -> Self {
        Self::new(1000, true)
    }
}

impl<T> Default for AudioQueue<T> {
    fn default() -> Self {
        Self::new(1000)
    }
}

impl<K, V> Default for AudioHashMap<K, V>
where
    K: Eq + std::hash::Hash + Clone,
    V: Clone,
{
    fn default() -> Self {
        Self::new(1000)
    }
}

impl<T> Clone for AudioList<T> {
    fn clone(&self) -> Self {
        Self {
            items: self.items.clone(),
            capacity: self.capacity,
            auto_sort: self.auto_sort,
        }
    }
}

impl<T> Clone for AudioQueue<T> {
    fn clone(&self) -> Self {
        Self {
            queue: self.queue.clone(),
            max_size: self.max_size,
        }
    }
}

impl<K, V> Clone for AudioHashMap<K, V>
where
    K: Eq + std::hash::Hash + Clone,
    V: Clone,
{
    fn clone(&self) -> Self {
        Self {
            map: self.map.clone(),
            max_entries: self.max_entries,
        }
    }
}
