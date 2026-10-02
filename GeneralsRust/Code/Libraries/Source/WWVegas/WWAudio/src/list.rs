//! Audio-specific list and collection utilities.

use crate::error::Result;
use std::collections::{HashMap, VecDeque};

/// Audio list with priority ordering, owned by its creator.
pub struct AudioList<T> {
    items: Vec<AudioListItem<T>>,
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

/// Audio queue for FIFO operations, owned by its creator.
pub struct AudioQueue<T> {
    queue: VecDeque<T>,
    max_size: usize,
}

/// Audio hash map with priority-based eviction, owned by its creator.
pub struct AudioHashMap<K, V> {
    map: HashMap<K, AudioMapEntry<V>>,
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
            items: Vec::with_capacity(capacity),
            capacity,
            auto_sort,
        }
    }

    /// Add item with priority
    pub fn add(&mut self, data: T, priority: crate::Priority) -> Result<()> {
        let items = &mut self.items;

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
    pub fn remove(&mut self, index: usize) -> Option<T> {
        let items = &mut self.items;
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
        self.items.get(index).cloned()
    }

    /// Get all items with minimum priority
    pub fn get_by_priority(&self, min_priority: crate::Priority) -> Vec<AudioListItem<T>>
    where
        T: Clone,
    {
        self.items
            .iter()
            .filter(|item| item.priority >= min_priority)
            .cloned()
            .collect()
    }

    /// Clear all items
    pub fn clear(&mut self) {
        self.items.clear();
    }

    /// Get current size
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Manually sort by priority
    pub fn sort(&mut self) {
        self.items.sort_by(|a, b| b.priority.cmp(&a.priority));
    }

    /// Remove items older than duration
    pub fn cleanup_old(&mut self, max_age: std::time::Duration) -> usize {
        let now = std::time::Instant::now();
        let initial_len = self.items.len();

        self.items
            .retain(|item| now.duration_since(item.timestamp) <= max_age);

        initial_len - self.items.len()
    }
}

impl<T> AudioQueue<T> {
    /// Create new audio queue
    pub fn new(max_size: usize) -> Self {
        Self {
            queue: VecDeque::with_capacity(max_size),
            max_size,
        }
    }

    /// Push item to back of queue
    pub fn push(&mut self, item: T) -> Result<()> {
        let queue = &mut self.queue;

        if queue.len() >= self.max_size {
            queue.pop_front(); // Remove oldest
        }

        queue.push_back(item);
        Ok(())
    }

    /// Pop item from front of queue
    pub fn pop(&mut self) -> Option<T> {
        self.queue.pop_front()
    }

    /// Peek at front item without removing
    pub fn peek(&self) -> Option<T>
    where
        T: Clone,
    {
        self.queue.front().cloned()
    }

    /// Get queue size
    pub fn len(&self) -> usize {
        self.queue.len()
    }

    /// Check if queue is empty
    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    /// Clear all items
    pub fn clear(&mut self) {
        self.queue.clear();
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
            map: HashMap::with_capacity(max_entries),
            max_entries,
        }
    }

    /// Insert item with priority
    pub fn insert(&mut self, key: K, value: V, priority: crate::Priority) -> Result<()> {
        let map = &mut self.map;

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
    pub fn get(&mut self, key: &K) -> Option<V> {
        if let Some(entry) = self.map.get_mut(key) {
            entry.access_count += 1;
            entry.last_accessed = std::time::Instant::now();
            Some(entry.value.clone())
        } else {
            None
        }
    }

    /// Remove item
    pub fn remove(&mut self, key: &K) -> Option<V> {
        self.map.remove(key).map(|entry| entry.value)
    }

    /// Check if key exists
    pub fn contains_key(&self, key: &K) -> bool {
        self.map.contains_key(key)
    }

    /// Get map size
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// Check if map is empty
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// Clear all items
    pub fn clear(&mut self) {
        self.map.clear();
    }

    /// Get all keys
    pub fn keys(&self) -> Vec<K> {
        self.map.keys().cloned().collect()
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

