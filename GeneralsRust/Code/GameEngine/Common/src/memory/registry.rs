//! Global Pool Registry
//!
//! Manages all pools in the system and provides centralized
//! statistics and monitoring.

use super::pool::ObjectPool;
use super::stats::{AllocationStats, MemoryStats};
use once_cell::sync::Lazy;
use parking_lot::RwLock;
use std::any::TypeId;
use std::collections::HashMap;
use std::sync::Arc;

/// Global registry of all pools.
pub struct PoolRegistry {
    /// Map from (TypeId, name) to pool.
    pools: RwLock<HashMap<(TypeId, String), Arc<dyn PoolHandle>>>,
}

impl PoolRegistry {
    /// Create a new registry.
    pub fn new() -> Self {
        Self {
            pools: RwLock::new(HashMap::new()),
        }
    }

    /// Register a pool.
    pub fn register<T: 'static + Send + Sync>(&self, name: String, pool: Arc<ObjectPool<T>>) {
        let key = (TypeId::of::<T>(), name);
        self.pools
            .write()
            .insert(key, Arc::new(TypedPoolHandle { pool }));
    }

    /// Get a pool by type and name.
    pub fn get<T: 'static + Send + Sync>(&self, name: &str) -> Option<Arc<ObjectPool<T>>> {
        let key = (TypeId::of::<T>(), name.to_string());
        self.pools.read().get(&key).and_then(|handle| {
            handle
                .as_any()
                .downcast_ref::<TypedPoolHandle<T>>()
                .map(|h| Arc::clone(&h.pool))
        })
    }

    /// Get global memory statistics.
    pub fn memory_stats(&self) -> MemoryStats {
        let pools = self.pools.read();
        let mut total_allocations = 0;
        let mut total_bytes_allocated = 0;
        let mut total_bytes_in_use = 0;
        let mut pool_stats = Vec::new();

        for handle in pools.values() {
            let stats = handle.get_stats();
            total_allocations += stats.total_allocations;
            total_bytes_allocated += stats.bytes_allocated;
            total_bytes_in_use += stats.bytes_in_use;
            pool_stats.push(stats);
        }

        let overall_utilization = if total_bytes_allocated > 0 {
            total_bytes_in_use as f64 / total_bytes_allocated as f64
        } else {
            0.0
        };

        MemoryStats {
            total_pools: pools.len(),
            total_allocations,
            total_bytes_allocated,
            total_bytes_in_use,
            overall_utilization,
            pools: pool_stats,
        }
    }

    /// Print a report of all pools.
    pub fn print_report(&self) {
        let stats = self.memory_stats();
        println!("{}", stats.report());
    }

    /// Get list of all pool names.
    pub fn pool_names(&self) -> Vec<String> {
        self.pools
            .read()
            .keys()
            .map(|(_, name)| name.clone())
            .collect()
    }

    /// Clear all pools (dangerous!).
    pub fn clear_all(&self) {
        for handle in self.pools.read().values() {
            handle.clear();
        }
    }
}

impl Default for PoolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Trait for type-erased pool handles.
trait PoolHandle: Send + Sync {
    fn as_any(&self) -> &dyn std::any::Any;
    fn get_stats(&self) -> AllocationStats;
    fn clear(&self);
}

/// Typed wrapper for pools.
struct TypedPoolHandle<T> {
    pool: Arc<ObjectPool<T>>,
}

impl<T: 'static + Send + Sync> PoolHandle for TypedPoolHandle<T> {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn get_stats(&self) -> AllocationStats {
        self.pool.stats().snapshot()
    }

    fn clear(&self) {
        self.pool.clear();
    }
}

/// Global pool registry singleton.
pub static POOL_REGISTRY: Lazy<PoolRegistry> = Lazy::new(PoolRegistry::new);

/// Convenience macro for registering a pool.
#[macro_export]
macro_rules! register_pool {
    ($name:expr, $pool:expr) => {
        $crate::memory::POOL_REGISTRY.register($name.to_string(), $pool)
    };
}

/// Convenience macro for getting a pool.
#[macro_export]
macro_rules! get_pool {
    ($ty:ty, $name:expr) => {
        $crate::memory::POOL_REGISTRY.get::<$ty>($name)
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::PoolConfig;

    #[test]
    fn test_registry() {
        let registry = PoolRegistry::new();
        let pool = ObjectPool::<u64>::new(PoolConfig::new("Test")).unwrap();

        registry.register("test".to_string(), pool.clone());
        let retrieved = registry.get::<u64>("test").unwrap();

        assert!(Arc::ptr_eq(&pool, &retrieved));
    }

    #[test]
    fn test_memory_stats() {
        let registry = PoolRegistry::new();
        let pool = ObjectPool::<u64>::new(PoolConfig::new("Test")).unwrap();
        registry.register("test".to_string(), pool.clone());

        let _handle = pool.alloc(42).unwrap();

        let stats = registry.memory_stats();
        assert_eq!(stats.total_pools, 1);
        assert!(stats.total_allocations > 0);
    }
}
