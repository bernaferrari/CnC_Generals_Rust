//! Audio caching system for efficient memory management.

use crate::{AudioSource, Priority, error::Result};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;

/// Audio cache configuration
#[derive(Debug, Clone)]
pub struct CacheConfig {
    pub max_size_bytes: usize,
    pub max_items: usize,
    pub block_size: usize,
    pub preload_threshold: usize,
}

/// Cache statistics
#[derive(Debug, Clone)]
pub struct CacheStats {
    pub total_size: usize,
    pub used_size: usize,
    pub item_count: usize,
    pub hit_rate: f32,
    pub miss_count: u64,
    pub hit_count: u64,
}

/// Cached audio item
pub struct CacheItem {
    pub key: String,
    pub source: Arc<AudioSource>,
    pub priority: Priority,
    pub access_count: u64,
    pub last_accessed: std::time::Instant,
}

/// Audio cache manager
pub struct AudioCache {
    config: CacheConfig,
    /// Items and stats are always updated together, so they share one lock
    state: RwLock<CombinedCacheState>,
}

/// Cached items plus their statistics (previously two separate locks)
struct CombinedCacheState {
    items: HashMap<String, CacheItem>,
    stats: CacheStats,
}

impl AudioCache {
    /// Create new audio cache with configuration
    pub fn new(config: CacheConfig) -> Self {
        Self {
            config,
            state: RwLock::new(CombinedCacheState {
                items: HashMap::new(),
                stats: CacheStats {
                    total_size: 0,
                    used_size: 0,
                    item_count: 0,
                    hit_rate: 0.0,
                    miss_count: 0,
                    hit_count: 0,
                },
            }),
        }
    }

    /// Get cached item
    pub async fn get(&self, key: &str) -> Result<Option<Arc<AudioSource>>> {
        let mut state = self.state.write();

        if let Some(item) = state.items.get_mut(key) {
            item.access_count = item.access_count.saturating_add(1);
            item.last_accessed = std::time::Instant::now();
            let data = std::sync::Arc::clone(&item.source);
            let stats = &mut state.stats;
            stats.hit_count = stats.hit_count.saturating_add(1);
            stats.hit_rate = compute_hit_rate(stats.hit_count, stats.miss_count);
            Ok(Some(data))
        } else {
            let stats = &mut state.stats;
            stats.miss_count = stats.miss_count.saturating_add(1);
            stats.hit_rate = compute_hit_rate(stats.hit_count, stats.miss_count);
            Ok(None)
        }
    }

    /// Store item in cache
    pub async fn put(
        &self,
        key: String,
        source: Arc<AudioSource>,
        priority: Priority,
    ) -> Result<()> {
        let data_len = source.metadata().file_size;
        let mut state = self.state.write();
        let CombinedCacheState {
            items,
            stats,
        } = &mut *state;

        if let Some(existing) = items.get(&key) {
            stats.used_size = stats
                .used_size
                .saturating_sub(existing.source.metadata().file_size)
                .saturating_add(data_len);
        } else {
            stats.used_size = stats.used_size.saturating_add(data_len);
            stats.item_count = stats.item_count.saturating_add(1);
        }

        while (stats.used_size > self.config.max_size_bytes)
            || (items.len() > self.config.max_items)
        {
            if let Some(evict_key) = select_eviction_candidate(items) {
                if let Some(entry) = items.remove(&evict_key) {
                    stats.used_size = stats
                        .used_size
                        .saturating_sub(entry.source.metadata().file_size);
                    stats.item_count = stats.item_count.saturating_sub(1);
                } else {
                    break;
                }
            } else {
                break;
            }
        }

        let map_key = key.clone();
        items.insert(
            map_key,
            CacheItem {
                key,
                source,
                priority,
                access_count: 1,
                last_accessed: std::time::Instant::now(),
            },
        );

        stats.total_size = stats.total_size.max(stats.used_size);
        stats.hit_rate = compute_hit_rate(stats.hit_count, stats.miss_count);

        Ok(())
    }

    /// Remove item from cache
    pub async fn remove(&self, key: &str) -> Result<bool> {
        let mut state = self.state.write();
        if let Some(entry) = state.items.remove(key) {
            let stats = &mut state.stats;
            stats.used_size = stats
                .used_size
                .saturating_sub(entry.source.metadata().file_size);
            stats.item_count = stats.item_count.saturating_sub(1);
            stats.hit_rate = compute_hit_rate(stats.hit_count, stats.miss_count);
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Get cache statistics
    pub fn stats(&self) -> CacheStats {
        self.state.read().stats.clone()
    }

    /// Clear all cached items
    pub async fn clear(&self) -> Result<()> {
        let mut state = self.state.write();
        state.items.clear();

        let stats = &mut state.stats;
        stats.used_size = 0;
        stats.item_count = 0;
        stats.hit_rate = compute_hit_rate(stats.hit_count, stats.miss_count);
        Ok(())
    }
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            max_size_bytes: 32 * 1024 * 1024, // 32MB
            max_items: 1000,
            block_size: 8 * 1024,           // 8KB
            preload_threshold: 1024 * 1024, // 1MB
        }
    }
}

fn compute_hit_rate(hit: u64, miss: u64) -> f32 {
    let total = hit.saturating_add(miss);
    if total == 0 {
        0.0
    } else {
        hit as f32 / total as f32
    }
}

fn select_eviction_candidate(items: &HashMap<String, CacheItem>) -> Option<String> {
    items
        .iter()
        .min_by(|(_, a), (_, b)| {
            a.priority
                .cmp(&b.priority)
                .then_with(|| a.last_accessed.cmp(&b.last_accessed))
        })
        .map(|(key, _)| key.clone())
}
