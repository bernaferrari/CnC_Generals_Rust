//! Global Pool Registry
//!
//! Manages all pools in the system and provides centralized
//! statistics and monitoring.

use super::pool::ObjectPool;
use super::stats::{AllocationStats, MemoryStats};
use std::any::TypeId;
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

/// Global registry of all pools.
///
/// Single-owner like the pools it tracks: the C++ original is a set of
/// plain per-type statics (`AutoPoolClass::Allocator`) driven by one thread.
/// THREAD: the process-wide instance lives in a `thread_local!` below —
/// no cross-thread boundary, so no lock.
pub struct PoolRegistry {
    /// Map from (TypeId, name) to pool.
    pools: RefCell<HashMap<(TypeId, String), Arc<dyn PoolHandle>>>,
}

impl PoolRegistry {
    /// Create a new registry.
    pub fn new() -> Self {
        Self {
            pools: RefCell::new(HashMap::new()),
        }
    }

    /// Register a pool.
    pub fn register<T: 'static + Send>(&self, name: String, pool: Arc<ObjectPool<T>>) {
        let key = (TypeId::of::<T>(), name);
        self.pools
            .borrow_mut()
            .insert(key, Arc::new(TypedPoolHandle { pool }));
    }

    /// Get a pool by type and name.
    pub fn get<T: 'static + Send>(&self, name: &str) -> Option<Arc<ObjectPool<T>>> {
        let key = (TypeId::of::<T>(), name.to_string());
        self.pools.borrow().get(&key).and_then(|handle| {
            handle
                .as_any()
                .downcast_ref::<TypedPoolHandle<T>>()
                .map(|h| Arc::clone(&h.pool))
        })
    }

    /// Get global memory statistics.
    pub fn memory_stats(&self) -> MemoryStats {
        let pools = self.pools.borrow();
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
            .borrow()
            .keys()
            .map(|(_, name)| name.clone())
            .collect()
    }

    /// Clear all pools (dangerous!).
    pub fn clear_all(&self) {
        for handle in self.pools.borrow().values() {
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
///
/// The registry is thread-local (single-owner), so no Send/Sync bound:
/// handles never cross a thread boundary.
trait PoolHandle {
    fn as_any(&self) -> &dyn std::any::Any;
    fn get_stats(&self) -> AllocationStats;
    fn clear(&self);
}

/// Typed wrapper for pools.
struct TypedPoolHandle<T> {
    pool: Arc<ObjectPool<T>>,
}

impl<T: 'static + Send> PoolHandle for TypedPoolHandle<T> {
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
///
/// Thread-local to match the C++ thread model: the registry is a plain
/// static driven by one thread, so each thread gets its own instance
/// instead of sharing a lock.
thread_local! {
    pub static POOL_REGISTRY: PoolRegistry = PoolRegistry::new();
}

/// Convenience macro for registering a pool.
#[macro_export]
macro_rules! register_pool {
    ($name:expr, $pool:expr) => {
        $crate::memory::POOL_REGISTRY.with(|registry| {
            registry.register($name.to_string(), $pool)
        })
    };
}

/// Convenience macro for getting a pool.
#[macro_export]
macro_rules! get_pool {
    ($ty:ty, $name:expr) => {
        $crate::memory::POOL_REGISTRY.with(|registry| registry.get::<$ty>($name))
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
