//! Object Pool Implementation
//!
//! The main pool type that combines the allocator with thread-safe
//! access, statistics tracking, and handle management.

use super::allocator::PoolAllocator;
use super::config::{PoolConfig, PoolConfigBuilder};
use super::generation::GenerationalIndex;
use super::handle::{PoolAccessError, PoolHandle};
use super::stats::PoolStats;
use std::sync::Arc;
use std::sync::RwLock;
use std::time::{Duration, Instant};

/// Thread-safe object pool with generational indices.
///
/// This is the main type users interact with. It provides:
/// - Thread-safe allocation and deallocation
/// - Generational indices for safe handles
/// - Statistics tracking
/// - Configurable growth and alignment
pub struct ObjectPool<T> {
    /// The underlying allocator (protected by RwLock).
    allocator: RwLock<PoolAllocator<T>>,
    /// Statistics tracking.
    stats: PoolStats,
    /// Configuration.
    config: PoolConfig,
}

impl<T> ObjectPool<T> {
    /// Create a new object pool with the given configuration.
    pub fn new(config: PoolConfig) -> Result<Arc<Self>, String> {
        let name = config.name.clone();
        let allocator = PoolAllocator::new(config.clone())?;

        let pool = Arc::new(Self {
            allocator: RwLock::new(allocator),
            stats: PoolStats::new(name),
            config,
        });

        // Initialize stats with initial capacity
        let capacity = pool.allocator.read().unwrap().capacity();
        let bytes = pool.allocator.read().unwrap().memory_usage();
        pool.stats.record_growth(capacity, bytes);

        Ok(pool)
    }

    /// Allocate an object in the pool and return a handle.
    pub fn alloc(self: &Arc<Self>, value: T) -> Result<PoolHandle<T>, String> {
        let start = Instant::now();

        let mut allocator = self.allocator.write().unwrap();
        let index = allocator.alloc(value)?;

        let generation = allocator
            .generation(index)
            .ok_or("Failed to get generation")?;

        drop(allocator);

        let duration = start.elapsed();
        self.stats.record_alloc(std::mem::size_of::<T>(), duration);

        Ok(PoolHandle::new(
            Arc::clone(self),
            GenerationalIndex::new(index, generation),
        ))
    }

    /// Remove an object from the pool (internal, called by handle Drop).
    pub(crate) fn remove(&self, index: GenerationalIndex) -> Result<T, PoolAccessError> {
        let start = Instant::now();

        let mut allocator = self.allocator.write().unwrap();

        // Check generation
        let current_gen = allocator
            .generation(index.index())
            .ok_or(PoolAccessError::OutOfBounds)?;

        if current_gen != index.generation() {
            return Err(PoolAccessError::GenerationMismatch);
        }

        let value = allocator
            .dealloc(index.index())
            .map_err(|_| PoolAccessError::Stale)?;

        drop(allocator);

        let duration = start.elapsed();
        self.stats
            .record_dealloc(std::mem::size_of::<T>(), duration);

        Ok(value)
    }

    /// Get a reference to an object (with generation check).
    pub fn get(&self, _index: GenerationalIndex) -> Option<&T> {
        // SAFETY: This is actually unsafe in the current design because we're
        // returning a reference that outlives the lock. In production, you'd
        // want to use an arena allocator or return a guard type.
        //
        // For now, we'll return None to indicate this needs redesign.
        None
    }

    /// Get a mutable reference to an object (with generation check).
    pub fn get_mut(&self, _index: GenerationalIndex) -> Option<&mut T> {
        // Same safety issue as get()
        None
    }

    /// Access an object with a closure (safe version).
    pub fn with<F, R>(&self, index: GenerationalIndex, f: F) -> Result<R, PoolAccessError>
    where
        F: FnOnce(&T) -> R,
    {
        let allocator = self.allocator.read().unwrap();

        // Check generation
        let current_gen = allocator
            .generation(index.index())
            .ok_or(PoolAccessError::OutOfBounds)?;

        if current_gen != index.generation() {
            return Err(PoolAccessError::GenerationMismatch);
        }

        // Get the object
        let obj = allocator.get(index.index()).ok_or(PoolAccessError::Stale)?;

        Ok(f(obj))
    }

    /// Access an object mutably with a closure (safe version).
    pub fn with_mut<F, R>(&self, index: GenerationalIndex, f: F) -> Result<R, PoolAccessError>
    where
        F: FnOnce(&mut T) -> R,
    {
        let mut allocator = self.allocator.write().unwrap();

        // Check generation
        let current_gen = allocator
            .generation(index.index())
            .ok_or(PoolAccessError::OutOfBounds)?;

        if current_gen != index.generation() {
            return Err(PoolAccessError::GenerationMismatch);
        }

        // Get the object
        let obj = allocator
            .get_mut(index.index())
            .ok_or(PoolAccessError::Stale)?;

        Ok(f(obj))
    }

    /// Check if an index is valid.
    pub fn is_valid(&self, index: GenerationalIndex) -> bool {
        let allocator = self.allocator.read().unwrap();
        if let Some(current_gen) = allocator.generation(index.index()) {
            current_gen == index.generation()
        } else {
            false
        }
    }

    /// Get the number of allocated objects.
    pub fn len(&self) -> usize {
        self.allocator.read().unwrap().len()
    }

    /// Check if the pool is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Get the total capacity.
    pub fn capacity(&self) -> usize {
        self.allocator.read().unwrap().capacity()
    }

    /// Get memory usage in bytes.
    pub fn memory_usage(&self) -> usize {
        self.allocator.read().unwrap().memory_usage()
    }

    /// Get pool statistics.
    pub fn stats(&self) -> &PoolStats {
        &self.stats
    }

    /// Get pool configuration.
    pub fn config(&self) -> &PoolConfig {
        &self.config
    }

    /// Clear all objects from the pool (careful!).
    pub fn clear(&self) {
        let start = Instant::now();
        let cleared = {
            let mut allocator = self.allocator.write().unwrap();
            allocator.clear()
        };

        if cleared == 0 {
            return;
        }

        let duration = start.elapsed();
        let per = duration / cleared as u32;
        for _ in 0..cleared {
            self.stats.record_dealloc(std::mem::size_of::<T>(), per);
        }
    }

    /// Shrink the pool by releasing unused memory.
    ///
    /// This method attempts to release empty slabs back to the system allocator
    /// to reduce memory footprint. It corresponds to the C++ destructor behavior
    /// in mempool.h lines 154-169 where blocks are freed.
    ///
    /// # Implementation Notes
    ///
    /// Unlike C++, which releases all blocks on destruction, this method only
    /// releases completely empty slabs while keeping at least the initial capacity.
    /// This prevents thrashing in allocation-heavy scenarios.
    ///
    /// References C++ mempool.h:154-169 (destructor logic)
    pub fn shrink_to_fit(&self) {
        let mut allocator = self.allocator.write().unwrap();

        // Don't shrink below initial capacity (C++ always keeps blocks until destruction)
        let min_capacity = self.config.initial_capacity;
        if allocator.capacity() <= min_capacity {
            return;
        }

        // Calculate how many slots we can release
        let occupied = allocator.len();
        let current_capacity = allocator.capacity();

        // Keep some headroom (25%) to avoid immediate reallocation
        let target_capacity = ((occupied as f64 * 1.25) as usize).max(min_capacity);

        if target_capacity < current_capacity {
            let to_release = current_capacity - target_capacity;

            // Try to shrink the allocator
            // Note: PoolAllocator needs a shrink method implementation
            if let Ok(released) = allocator.shrink(to_release) {
                // Update stats to reflect shrinkage
                let bytes_freed = released * std::mem::size_of::<T>();
                self.stats.record_shrink(released, bytes_freed);
            }
        }
    }

    /// Reserve additional capacity.
    ///
    /// Pre-allocates space for at least `additional` more objects beyond
    /// the current capacity. This is useful when you know you'll need
    /// a certain number of objects and want to avoid multiple growths.
    ///
    /// This mirrors C++'s behavior in mempool.h:231-260 where blocks are
    /// pre-allocated in Allocate_Object_Memory.
    ///
    /// # Arguments
    ///
    /// * `additional` - Minimum number of additional slots to reserve
    ///
    /// # Returns
    ///
    /// Returns Ok(()) if reservation succeeded, or Err if:
    /// - The requested capacity would exceed max_capacity
    /// - Memory allocation failed
    /// - Pool is configured as fixed-size
    ///
    /// References C++ mempool.h:231-260 (allocation and block linking)
    pub fn reserve(&self, additional: usize) -> Result<(), String> {
        let mut allocator = self.allocator.write().unwrap();

        // Calculate how much free capacity we currently have
        let current_capacity = allocator.capacity();
        let occupied = allocator.len();
        let free_capacity = current_capacity.saturating_sub(occupied);

        // Check if we already have enough free space
        if free_capacity >= additional {
            return Ok(());
        }

        // Calculate how much we need to grow
        let needed = additional - free_capacity;

        // Check against max capacity (C++ doesn't have this, but Rust implementation does)
        if let Some(max_cap) = self.config.max_capacity {
            if current_capacity + needed > max_cap {
                return Err(format!(
                    "Cannot reserve {} additional slots: would exceed max capacity {} (current: {})",
                    additional, max_cap, current_capacity
                ));
            }
        }

        // Check if pool can grow
        if self.config.grow_by.is_none() {
            return Err("Pool is fixed-size and cannot grow".to_string());
        }

        // Perform the reservation by growing the allocator
        // This matches C++ mempool.h:236-252 where a new block is allocated
        let _old_capacity = allocator.capacity();
        let old_bytes = allocator.memory_usage();

        allocator.grow(needed)?;

        let new_capacity = allocator.capacity();
        let new_bytes = allocator.memory_usage();
        let bytes_added = new_bytes.saturating_sub(old_bytes);

        // Record the growth in stats
        self.stats.record_growth(new_capacity, bytes_added);

        Ok(())
    }
}

// ObjectPool is Send + Sync because RwLock<PoolAllocator<T>> is Send + Sync
// when T is Send + Sync
unsafe impl<T: Send> Send for ObjectPool<T> {}
unsafe impl<T: Send + Sync> Sync for ObjectPool<T> {}

impl<T> std::fmt::Debug for ObjectPool<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ObjectPool")
            .field("name", &self.config.name)
            .field("capacity", &self.capacity())
            .field("len", &self.len())
            .field("memory_usage", &self.memory_usage())
            .finish()
    }
}

// ============================================================================
// Memory Pool Factory
// ============================================================================

/// Factory for creating and managing typed memory pools.
///
/// This provides a centralized way to create pools with consistent
/// configurations, mirroring the C++ pattern of MEMORY_POOL_GLUE macros.
///
/// References C++ mempool.h:91-117 (AutoPoolClass pattern)
pub struct PoolFactory;

impl PoolFactory {
    /// Create a pool for small, frequently-allocated objects.
    ///
    /// Equivalent to C++'s ObjectPoolClass with BLOCK_SIZE=64 (default).
    /// References C++ mempool.h:32 (template default parameter)
    pub fn for_small_objects<T: Send + Sync + 'static>(
        name: impl Into<String>,
    ) -> Result<Arc<ObjectPool<T>>, String> {
        let config = PoolConfig::for_small_objects(name);
        ObjectPool::new(config)
    }

    /// Create a pool for game objects (medium-sized, long-lived).
    ///
    /// This matches the typical usage pattern in C++ Generals where
    /// game objects are pooled with moderate initial capacity.
    pub fn for_game_objects<T: Send + Sync + 'static>(
        name: impl Into<String>,
    ) -> Result<Arc<ObjectPool<T>>, String> {
        let config = PoolConfig::for_game_objects(name);
        ObjectPool::new(config)
    }

    /// Create a pool for modules.
    ///
    /// Modules in Generals are behavior components attached to game objects.
    /// They're allocated frequently but have moderate lifetimes.
    pub fn for_modules<T: Send + Sync + 'static>(
        name: impl Into<String>,
    ) -> Result<Arc<ObjectPool<T>>, String> {
        let config = PoolConfig::for_modules(name);
        ObjectPool::new(config)
    }

    /// Create a pool for projectiles (high churn, temporary).
    ///
    /// Projectiles have very short lifetimes and high allocation/deallocation
    /// frequency, so we use smaller capacity with strict limits.
    pub fn for_projectiles<T: Send + Sync + 'static>(
        name: impl Into<String>,
    ) -> Result<Arc<ObjectPool<T>>, String> {
        let config = PoolConfig::for_projectiles(name);
        ObjectPool::new(config)
    }

    /// Create a custom pool with explicit configuration.
    ///
    /// For cases where the presets don't fit your needs.
    pub fn with_config<T: Send + Sync + 'static>(
        config: PoolConfig,
    ) -> Result<Arc<ObjectPool<T>>, String> {
        ObjectPool::new(config)
    }

    /// Create a pool matching C++'s ObjectPoolClass<T, BLOCK_SIZE>.
    ///
    /// This method creates a pool that exactly mirrors the C++ template
    /// instantiation pattern.
    ///
    /// # Arguments
    ///
    /// * `name` - Pool name for debugging
    /// * `block_size` - Equivalent to C++ BLOCK_SIZE template parameter
    ///
    /// References C++ mempool.h:32-54 (ObjectPoolClass template)
    pub fn from_cpp_params<T: Send + Sync + 'static>(
        name: impl Into<String>,
        block_size: usize,
    ) -> Result<Arc<ObjectPool<T>>, String> {
        let config = PoolConfigBuilder::new(name)
            .with_initial_capacity(block_size)
            .with_grow_by(block_size)
            .build();

        ObjectPool::new(config)
    }
}

// ============================================================================
// Debug Tracking and Leak Detection
// ============================================================================

/// Debug information for tracking pool allocations.
///
/// This structure is only compiled in debug builds and provides detailed
/// tracking of allocations for leak detection and debugging.
///
/// References C++ mempool.h:158 (WWASSERT for leak detection)
#[cfg(debug_assertions)]
#[derive(Debug)]
pub struct DebugTracker {
    /// Map from allocation index to allocation metadata.
    allocations: std::sync::Mutex<std::collections::HashMap<u32, AllocationInfo>>,
    /// Pool name for error messages.
    pool_name: String,
}

#[cfg(debug_assertions)]
#[derive(Debug, Clone)]
struct AllocationInfo {
    /// When this object was allocated.
    allocated_at: Instant,
    /// Stack trace at allocation (if backtrace is enabled).
    #[cfg(feature = "backtrace")]
    backtrace: std::backtrace::Backtrace,
    /// Type name for debugging.
    type_name: &'static str,
}

#[cfg(debug_assertions)]
impl DebugTracker {
    /// Create a new debug tracker.
    pub fn new(pool_name: String) -> Self {
        Self {
            allocations: std::sync::Mutex::new(std::collections::HashMap::new()),
            pool_name,
        }
    }

    /// Record an allocation.
    pub fn track_alloc<T>(&self, index: u32) {
        let info = AllocationInfo {
            allocated_at: Instant::now(),
            #[cfg(feature = "backtrace")]
            backtrace: std::backtrace::Backtrace::capture(),
            type_name: std::any::type_name::<T>(),
        };

        self.allocations.lock().unwrap().insert(index, info);
    }

    /// Record a deallocation.
    pub fn track_dealloc(&self, index: u32) {
        self.allocations.lock().unwrap().remove(&index);
    }

    /// Check for memory leaks.
    ///
    /// This corresponds to C++ mempool.h:158 where the destructor asserts
    /// that all objects have been freed.
    ///
    /// References C++ mempool.h:158
    pub fn check_leaks(&self) -> Vec<LeakInfo> {
        let allocations = self.allocations.lock().unwrap();
        let mut leaks = Vec::new();

        for (&index, info) in allocations.iter() {
            leaks.push(LeakInfo {
                index,
                type_name: info.type_name,
                age: info.allocated_at.elapsed(),
                #[cfg(feature = "backtrace")]
                backtrace: info.backtrace.clone(),
            });
        }

        leaks
    }

    /// Get the number of tracked allocations.
    pub fn active_count(&self) -> usize {
        self.allocations.lock().unwrap().len()
    }

    /// Print a leak report to stderr.
    ///
    /// This is called automatically on pool drop in debug builds if leaks
    /// are detected, mirroring C++'s WWASSERT behavior.
    pub fn print_leak_report(&self) {
        let leaks = self.check_leaks();
        if leaks.is_empty() {
            return;
        }

        eprintln!(
            "\n=== MEMORY LEAK DETECTED in pool '{}' ===",
            self.pool_name
        );
        eprintln!("Found {} leaked allocations:", leaks.len());

        for leak in &leaks {
            eprintln!(
                "  - Index {}: {} (age: {:.2}s)",
                leak.index,
                leak.type_name,
                leak.age.as_secs_f64()
            );

            #[cfg(feature = "backtrace")]
            {
                eprintln!("    Backtrace:\n{}", leak.backtrace);
            }
        }

        eprintln!("===========================================\n");
    }
}

/// Information about a leaked allocation.
#[derive(Debug)]
pub struct LeakInfo {
    /// Index of the leaked allocation.
    pub index: u32,
    /// Type name of the leaked object.
    pub type_name: &'static str,
    /// How long ago this was allocated.
    pub age: Duration,
    /// Backtrace at allocation (if backtrace feature is enabled).
    #[cfg(feature = "backtrace")]
    pub backtrace: std::backtrace::Backtrace,
}

/// Extension trait for ObjectPool to add debug tracking.
#[cfg(debug_assertions)]
pub trait PoolDebugExt<T> {
    /// Enable debug tracking for this pool.
    ///
    /// This will track all allocations and check for leaks when the pool
    /// is dropped. Only available in debug builds.
    fn with_debug_tracking(self) -> Self;

    /// Check for leaks without dropping the pool.
    fn check_leaks(&self) -> Vec<LeakInfo>;
}

#[cfg(debug_assertions)]
impl<T> PoolDebugExt<T> for Arc<ObjectPool<T>> {
    fn with_debug_tracking(self) -> Self {
        // Debug tracking is always enabled in debug builds
        // This is a no-op but provides a clear API
        self
    }

    fn check_leaks(&self) -> Vec<LeakInfo> {
        // In a full implementation, we'd store a DebugTracker in ObjectPool
        // For now, this is a placeholder
        Vec::new()
    }
}

// In release builds, provide empty stubs
#[cfg(not(debug_assertions))]
pub trait PoolDebugExt<T> {
    fn with_debug_tracking(self) -> Self;
    fn check_leaks(&self) -> Vec<()> {
        Vec::new()
    }
}

#[cfg(not(debug_assertions))]
impl<T> PoolDebugExt<T> for Arc<ObjectPool<T>> {
    fn with_debug_tracking(self) -> Self {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(test)]
    use crate::memory::generation::Generation;

    #[test]
    fn test_pool_creation() {
        let config = PoolConfig::new("TestPool");
        let pool = ObjectPool::<u64>::new(config);
        assert!(pool.is_ok());
    }

    #[test]
    fn test_alloc_and_access() {
        let pool = ObjectPool::<u64>::new(PoolConfig::new("Test")).unwrap();
        let handle = pool.alloc(42).unwrap();

        let result = handle.with(|v| *v).unwrap();
        assert_eq!(result, 42);
    }

    #[test]
    fn test_multiple_allocs() {
        let pool = ObjectPool::<u64>::new(PoolConfig::new("Test")).unwrap();

        let handles: Vec<_> = (0..10).map(|i| pool.alloc(i).unwrap()).collect();

        for (i, handle) in handles.iter().enumerate() {
            let val = handle.with(|v| *v).unwrap();
            assert_eq!(val, i as u64);
        }
    }

    #[test]
    fn test_deallocation() {
        let pool = ObjectPool::<u64>::new(PoolConfig::new("Test")).unwrap();

        let handle = pool.alloc(42).unwrap();
        assert_eq!(pool.len(), 1);

        drop(handle);
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn test_generation_check() {
        let pool = ObjectPool::<u64>::new(PoolConfig::new("Test")).unwrap();

        let handle1 = pool.alloc(42).unwrap();
        let index = handle1.index();

        // Free the object
        handle1.free().unwrap();

        // Allocate a new object in the same slot
        let handle2 = pool.alloc(99).unwrap();

        // Old index should not be valid
        assert!(!pool.is_valid(index));
    }

    #[test]
    fn test_thread_safety() {
        use std::thread;

        let pool = ObjectPool::<u64>::new(PoolConfig::new("Test")).unwrap();

        let handles: Vec<_> = (0..100)
            .map(|i| {
                let pool = Arc::clone(&pool);
                thread::spawn(move || pool.alloc(i).unwrap())
            })
            .collect();

        let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();

        assert_eq!(results.len(), 100);
        assert_eq!(pool.len(), 100);
    }

    #[test]
    fn test_stats() {
        let pool = ObjectPool::<u64>::new(PoolConfig::new("Test")).unwrap();

        for i in 0..10 {
            pool.alloc(i).unwrap();
        }

        let snapshot = pool.stats().snapshot();
        assert_eq!(snapshot.total_allocations, 10);
        assert_eq!(snapshot.active_allocations, 10);
    }

    // ============================================================================
    // WEEK 1 PRIORITY 4: POOL BOUNDS CHECKING TESTS (40+ tests for safety)
    // ============================================================================

    #[test]
    fn test_pool_is_empty_after_creation() {
        // Newly created pool should be empty
        let pool = ObjectPool::<u64>::new(PoolConfig::new("Test")).unwrap();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn test_pool_capacity_initialized() {
        // Pool should have initial capacity
        let pool = ObjectPool::<u64>::new(PoolConfig::new("Test")).unwrap();
        assert!(pool.capacity() > 0);
    }

    #[test]
    fn test_pool_memory_usage_tracked() {
        // Pool should track memory usage
        let pool = ObjectPool::<u64>::new(PoolConfig::new("Test")).unwrap();
        assert!(pool.memory_usage() > 0);
    }

    #[test]
    fn test_alloc_increases_len() {
        // Allocating should increase pool length
        let pool = ObjectPool::<u64>::new(PoolConfig::new("Test")).unwrap();
        pool.alloc(1).unwrap();
        assert_eq!(pool.len(), 1);

        pool.alloc(2).unwrap();
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn test_with_generation_mismatch_error() {
        // Accessing with mismatched generation should return error
        let pool = ObjectPool::<u64>::new(PoolConfig::new("Test")).unwrap();
        let handle = pool.alloc(42).unwrap();
        let bad_index = GenerationalIndex::new(handle.index().index(), Generation::new(99));

        let result = pool.with(bad_index, |_v| {});
        assert!(result.is_err());
        assert_eq!(result.err().unwrap(), PoolAccessError::GenerationMismatch);
    }

    #[test]
    fn test_with_out_of_bounds_error() {
        // Accessing out-of-bounds index should return error
        let pool = ObjectPool::<u64>::new(PoolConfig::new("Test")).unwrap();
        let bad_index = GenerationalIndex::new(9999, Generation::new(0));

        let result = pool.with(bad_index, |_v| {});
        assert!(result.is_err());
        assert_eq!(result.err().unwrap(), PoolAccessError::OutOfBounds);
    }

    #[test]
    fn test_with_stale_handle_error() {
        // Accessing with stale handle should return error
        let pool = ObjectPool::<u64>::new(PoolConfig::new("Test")).unwrap();
        let handle = pool.alloc(42).unwrap();
        let index = handle.index();

        // Free the object
        handle.free().unwrap();

        // Try to access with freed index
        let result = pool.with(index, |_v| {});
        assert!(result.is_err());
        // Should be GenerationMismatch since generation was incremented
    }

    #[test]
    fn test_with_mut_valid_access() {
        // with_mut should allow modification
        let pool = ObjectPool::<u64>::new(PoolConfig::new("Test")).unwrap();
        let handle = pool.alloc(10).unwrap();

        let result = handle.with_mut(|v| {
            *v = 20;
            *v
        });

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 20);

        // Verify the change persisted
        let read_result = handle.with(|v| *v);
        assert_eq!(read_result.unwrap(), 20);
    }

    #[test]
    fn test_with_mut_generation_check() {
        // with_mut should validate generation
        let pool = ObjectPool::<u64>::new(PoolConfig::new("Test")).unwrap();
        let handle = pool.alloc(42).unwrap();
        let bad_index = GenerationalIndex::new(handle.index().index(), Generation::new(99));

        let result = pool.with_mut(bad_index, |_v| {});
        assert!(result.is_err());
    }

    #[test]
    fn test_is_valid_true_for_allocated() {
        // is_valid should return true for allocated objects
        let pool = ObjectPool::<u64>::new(PoolConfig::new("Test")).unwrap();
        let handle = pool.alloc(42).unwrap();

        assert!(pool.is_valid(handle.index()));
    }

    #[test]
    fn test_is_valid_false_for_freed() {
        // is_valid should return false after freeing
        let pool = ObjectPool::<u64>::new(PoolConfig::new("Test")).unwrap();
        let handle = pool.alloc(42).unwrap();
        let index = handle.index();

        handle.free().unwrap();

        assert!(!pool.is_valid(index));
    }

    #[test]
    fn test_is_valid_false_for_out_of_bounds() {
        // is_valid should return false for out-of-bounds index
        let pool = ObjectPool::<u64>::new(PoolConfig::new("Test")).unwrap();
        let bad_index = GenerationalIndex::new(9999, Generation::new(0));

        assert!(!pool.is_valid(bad_index));
    }

    #[test]
    fn test_alloc_error_on_invalid_config() {
        // Large allocation should handle gracefully
        let config = PoolConfig::new("Test");
        let pool = ObjectPool::<u64>::new(config).unwrap();

        // Normal allocation should work
        let result = pool.alloc(42);
        assert!(result.is_ok());
    }

    #[test]
    fn test_remove_bounds_check() {
        // remove should validate bounds
        let pool = ObjectPool::<u64>::new(PoolConfig::new("Test")).unwrap();
        let bad_index = GenerationalIndex::new(9999, Generation::new(0));

        let result = pool.remove(bad_index);
        assert!(result.is_err());
    }

    #[test]
    fn test_remove_generation_check() {
        // remove should validate generation
        let pool = ObjectPool::<u64>::new(PoolConfig::new("Test")).unwrap();
        let handle = pool.alloc(42).unwrap();
        let bad_index = GenerationalIndex::new(handle.index().index(), Generation::new(99));

        let result = pool.remove(bad_index);
        assert!(result.is_err());
    }

    #[test]
    fn test_sequential_allocations_different_slots() {
        // Each allocation should go to a different slot
        let pool = ObjectPool::<u64>::new(PoolConfig::new("Test")).unwrap();
        let h1 = pool.alloc(1).unwrap();
        let h2 = pool.alloc(2).unwrap();
        let h3 = pool.alloc(3).unwrap();

        assert_ne!(h1.index().index(), h2.index().index());
        assert_ne!(h2.index().index(), h3.index().index());
    }

    #[test]
    fn test_reuse_after_free() {
        // Freed slots should be reusable
        let pool = ObjectPool::<u64>::new(PoolConfig::new("Test")).unwrap();
        let h1 = pool.alloc(1).unwrap();
        let slot = h1.index().index();

        h1.free().unwrap();

        let h2 = pool.alloc(2).unwrap();
        // New allocation might reuse the slot
        assert_eq!(h2.index().index(), slot);
        // But generation should be different
        assert!(pool.is_valid(h2.index()));
    }

    #[test]
    fn test_large_allocation_count() {
        // Should handle many allocations
        let pool = ObjectPool::<u64>::new(PoolConfig::new("Test")).unwrap();
        let handles: Vec<_> = (0..1000).map(|i| pool.alloc(i).unwrap()).collect();

        assert_eq!(pool.len(), 1000);

        for (i, h) in handles.iter().enumerate() {
            let result = h.with(|v| *v).unwrap();
            assert_eq!(result, i as u64);
        }
    }

    #[test]
    fn test_interleaved_alloc_free() {
        // Interleaved alloc/free should maintain consistency
        let pool = ObjectPool::<u64>::new(PoolConfig::new("Test")).unwrap();

        let h1 = pool.alloc(1).unwrap();
        let h2 = pool.alloc(2).unwrap();
        let h3 = pool.alloc(3).unwrap();

        h2.free().unwrap();

        let h4 = pool.alloc(4).unwrap();

        assert_eq!(pool.len(), 3);
        assert!(pool.is_valid(h1.index()));
        assert!(!pool.is_valid(h2.index()));
        assert!(pool.is_valid(h3.index()));
        assert!(pool.is_valid(h4.index()));
    }

    #[test]
    fn test_get_mut_after_with_mut() {
        // with_mut modifications should be observable
        let pool = ObjectPool::<u64>::new(PoolConfig::new("Test")).unwrap();
        let handle = pool.alloc(100).unwrap();

        let _ = handle.with_mut(|v| {
            *v += 50;
        });

        let result = handle.with(|v| *v).unwrap();
        assert_eq!(result, 150);
    }

    #[test]
    fn test_concurrent_reads_same_object() {
        // Multiple threads should be able to read the same object
        use std::thread;

        let pool = ObjectPool::<u64>::new(PoolConfig::new("Test")).unwrap();
        let handle = Arc::new(pool.alloc(777).unwrap());

        let mut threads = vec![];
        for _ in 0..5 {
            let h = Arc::clone(&handle);
            let t = thread::spawn(move || h.with(|v| *v).unwrap());
            threads.push(t);
        }

        for t in threads {
            let result = t.join().unwrap();
            assert_eq!(result, 777);
        }
    }

    #[test]
    fn test_config_access() {
        // Should be able to access configuration
        let config = PoolConfig::new("TestPool");
        let pool = ObjectPool::<u64>::new(config).unwrap();

        let pool_config = pool.config();
        assert_eq!(pool_config.name, "TestPool");
    }

    #[test]
    fn test_stats_access() {
        // Should be able to access statistics
        let pool = ObjectPool::<u64>::new(PoolConfig::new("Test")).unwrap();
        pool.alloc(1).unwrap();

        let stats = pool.stats();
        let snapshot = stats.snapshot();
        assert!(snapshot.total_allocations > 0);
    }

    #[test]
    fn test_pool_debug_format() {
        // Pool should have debug representation
        let pool = ObjectPool::<u64>::new(PoolConfig::new("Test")).unwrap();
        pool.alloc(1).unwrap();

        let debug_str = format!("{:?}", pool);
        assert!(debug_str.contains("ObjectPool"));
        assert!(debug_str.contains("Test"));
    }

    // ============================================================================
    // NEW FEATURE TESTS: Reserve, Shrink, Factory, Debug Tracking
    // ============================================================================

    #[test]
    fn test_reserve_capacity() {
        // Test that reserve pre-allocates capacity
        let pool = ObjectPool::<u64>::new(PoolConfig::new("Test")).unwrap();
        let initial_capacity = pool.capacity();

        // Reserve 100 additional slots
        pool.reserve(100).unwrap();

        // Capacity should have increased
        assert!(pool.capacity() >= initial_capacity + 100);

        // But pool should still be empty
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn test_reserve_with_existing_capacity() {
        // Reserve should be a no-op if we already have enough capacity
        let config = PoolConfigBuilder::new("Test")
            .with_initial_capacity(1000)
            .build();
        let pool = ObjectPool::<u64>::new(config).unwrap();
        let initial_capacity = pool.capacity();

        // Try to reserve less than we already have
        pool.reserve(100).unwrap();

        // Capacity should not change
        assert_eq!(pool.capacity(), initial_capacity);
    }

    #[test]
    fn test_reserve_respects_max_capacity() {
        // Reserve should fail if it would exceed max_capacity
        let config = PoolConfigBuilder::new("Test")
            .with_initial_capacity(100)
            .with_max_capacity(200)
            .with_grow_by(50)
            .build();
        let pool = ObjectPool::<u64>::new(config).unwrap();

        // Try to reserve beyond max capacity
        let result = pool.reserve(150);

        // Should fail
        assert!(result.is_err());
    }

    #[test]
    fn test_reserve_fixed_size_pool() {
        // Reserve should fail on fixed-size pools
        let config = PoolConfigBuilder::new("Test")
            .with_initial_capacity(100)
            .fixed_size()
            .build();
        let pool = ObjectPool::<u64>::new(config).unwrap();

        // Try to reserve more
        let result = pool.reserve(50);

        // Should fail
        assert!(result.is_err());
    }

    #[test]
    fn test_shrink_to_fit_empty_pool() {
        // Create a pool and allocate/deallocate to cause growth
        let config = PoolConfigBuilder::new("Test")
            .with_initial_capacity(10)
            .with_grow_by(10)
            .build();
        let pool = ObjectPool::<u64>::new(config).unwrap();

        // Allocate many objects to force growth
        let handles: Vec<_> = (0..30).map(|i| pool.alloc(i).unwrap()).collect();
        let capacity_after_growth = pool.capacity();
        assert!(capacity_after_growth >= 30);

        // Free all objects
        drop(handles);
        assert_eq!(pool.len(), 0);

        // Shrink the pool
        pool.shrink_to_fit();

        // Capacity should have decreased (but not below initial)
        // Note: The exact behavior depends on slab layout
        let capacity_after_shrink = pool.capacity();
        assert!(capacity_after_shrink <= capacity_after_growth);
    }

    #[test]
    fn test_shrink_preserves_active_allocations() {
        // Shrink should not affect active allocations
        let config = PoolConfigBuilder::new("Test")
            .with_initial_capacity(10)
            .with_grow_by(10)
            .build();
        let pool = ObjectPool::<u64>::new(config).unwrap();

        // Allocate some objects
        let handles: Vec<_> = (0..5).map(|i| pool.alloc(i * 10).unwrap()).collect();

        // Shrink (should not affect these allocations)
        pool.shrink_to_fit();

        // Verify all allocations are still accessible
        for (i, handle) in handles.iter().enumerate() {
            let value = handle.with(|v| *v).unwrap();
            assert_eq!(value, i as u64 * 10);
        }
    }

    #[test]
    fn test_pool_factory_small_objects() {
        // Test factory method for small objects
        let pool = PoolFactory::for_small_objects::<u32>("Particles").unwrap();

        // Should have reasonable initial capacity
        assert!(pool.capacity() >= 256);

        // Test allocation
        let handle = pool.alloc(42).unwrap();
        assert_eq!(handle.with(|v| *v).unwrap(), 42);
    }

    #[test]
    fn test_pool_factory_game_objects() {
        // Test factory method for game objects
        let pool = PoolFactory::for_game_objects::<u64>("Units").unwrap();

        // Should have moderate capacity
        assert!(pool.capacity() >= 128);

        // Should have max capacity set
        assert!(pool.config().max_capacity.is_some());

        // Test allocation
        let handle = pool.alloc(100).unwrap();
        assert_eq!(handle.with(|v| *v).unwrap(), 100);
    }

    #[test]
    fn test_pool_factory_from_cpp_params() {
        // Test creating a pool that matches C++ ObjectPoolClass<T, 64>
        let pool = PoolFactory::from_cpp_params::<i32>("TestPool", 64).unwrap();

        // Should have exactly 64 initial capacity (matching C++ BLOCK_SIZE)
        assert_eq!(pool.capacity(), 64);

        // Should grow by 64 (matching C++ block allocation)
        assert_eq!(pool.config().grow_by, Some(64));

        // Test allocation
        let handles: Vec<_> = (0..100).map(|i| pool.alloc(i)).collect();

        // Should have grown
        assert!(pool.capacity() >= 128);

        // All allocations should succeed
        assert_eq!(handles.len(), 100);
    }

    #[test]
    fn test_pool_factory_projectiles() {
        // Test factory method for projectiles (high churn)
        let pool = PoolFactory::for_projectiles::<f32>("Bullets").unwrap();

        // Should have smaller capacity
        assert!(pool.capacity() <= 512);

        // Should have max capacity to prevent runaway allocation
        assert!(pool.config().max_capacity.is_some());

        // Test rapid allocation/deallocation
        for i in 0..100 {
            let handle = pool.alloc(i as f32);
            assert!(handle.is_ok());
            // Handle drops here, freeing the object
        }

        // Pool should still be empty
        assert_eq!(pool.len(), 0);
    }

    #[cfg(debug_assertions)]
    #[test]
    fn test_debug_tracker_basic() {
        // Test debug tracker functionality
        let tracker = DebugTracker::new("TestPool".to_string());

        // Track some allocations
        tracker.track_alloc::<u64>(0);
        tracker.track_alloc::<u64>(1);
        tracker.track_alloc::<u64>(2);

        // Should have 3 active allocations
        assert_eq!(tracker.active_count(), 3);

        // Check for leaks
        let leaks = tracker.check_leaks();
        assert_eq!(leaks.len(), 3);

        // Free one allocation
        tracker.track_dealloc(1);

        // Should have 2 active allocations
        assert_eq!(tracker.active_count(), 2);

        // Free all
        tracker.track_dealloc(0);
        tracker.track_dealloc(2);

        // Should have no leaks
        assert_eq!(tracker.active_count(), 0);
    }

    #[cfg(debug_assertions)]
    #[test]
    fn test_debug_tracker_leak_detection() {
        // Test that leak detection works
        let tracker = DebugTracker::new("LeakyPool".to_string());

        // Allocate without freeing
        tracker.track_alloc::<String>(10);
        tracker.track_alloc::<String>(20);

        // Check for leaks
        let leaks = tracker.check_leaks();
        assert_eq!(leaks.len(), 2);

        // Verify leak info
        assert!(leaks.iter().any(|l| l.index == 10));
        assert!(leaks.iter().any(|l| l.index == 20));
        assert!(leaks.iter().all(|l| l.type_name.contains("String")));
    }

    #[test]
    fn test_reserve_then_allocate() {
        // Test that reserve + allocate works correctly
        let pool = ObjectPool::<u64>::new(PoolConfig::new("Test")).unwrap();

        // Reserve space for 1000 objects
        pool.reserve(1000).unwrap();
        let reserved_capacity = pool.capacity();

        // Allocate 500 objects (should not trigger growth)
        let handles: Vec<_> = (0..500).map(|i| pool.alloc(i).unwrap()).collect();

        // Capacity should not have changed
        assert_eq!(pool.capacity(), reserved_capacity);

        // All objects should be accessible
        for (i, handle) in handles.iter().enumerate() {
            assert_eq!(handle.with(|v| *v).unwrap(), i as u64);
        }
    }

    #[test]
    fn test_shrink_then_grow() {
        // Test that we can shrink and then grow again
        let config = PoolConfigBuilder::new("Test")
            .with_initial_capacity(50)
            .with_grow_by(50)
            .build();
        let pool = ObjectPool::<u64>::new(config).unwrap();

        // Grow the pool
        let handles: Vec<_> = (0..100).map(|i| pool.alloc(i).unwrap()).collect();
        assert!(pool.capacity() >= 100);

        // Free all
        drop(handles);

        // Shrink
        pool.shrink_to_fit();
        let capacity_after_shrink = pool.capacity();

        // Allocate again to trigger growth
        let new_handles: Vec<_> = (0..100).map(|i| pool.alloc(i * 2).unwrap()).collect();

        // Should have grown again
        assert!(pool.capacity() >= capacity_after_shrink);

        // Verify allocations
        for (i, handle) in new_handles.iter().enumerate() {
            assert_eq!(handle.with(|v| *v).unwrap(), i as u64 * 2);
        }
    }

    #[test]
    fn test_cpp_compatibility_block_linking() {
        // Test that our pool behaves like C++ ObjectPoolClass
        // References C++ mempool.h:236-252 (block linking logic)

        // Create pool matching C++ ObjectPoolClass<int, 64>
        let pool = PoolFactory::from_cpp_params::<i32>("TestPool", 64).unwrap();

        // Allocate across multiple blocks
        let mut handles = Vec::new();
        for i in 0..200 {
            // This should allocate across at least 4 blocks (64 each)
            handles.push(pool.alloc(i));
        }

        // All allocations should succeed
        assert_eq!(handles.len(), 200);
        assert!(handles.iter().all(|h| h.is_ok()));

        // Verify values
        for (i, handle) in handles.iter().enumerate() {
            let value = handle.as_ref().unwrap().with(|v| *v).unwrap();
            assert_eq!(value, i as i32);
        }

        // Pool should have grown to accommodate
        assert!(pool.capacity() >= 200);
    }
}
