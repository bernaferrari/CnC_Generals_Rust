//! Generic object pooling for efficient memory management
//!
//! This module provides object pools to minimize allocation overhead for frequently
//! created/destroyed objects. Matches C++ RenderObjectRecycler patterns.

use std::sync::{Arc, Mutex};

/// Handle to a pooled object that returns to pool on drop
pub struct PoolHandle<T> {
    object: Option<T>,
    pool: Arc<Mutex<ObjectPool<T>>>,
}

impl<T> PoolHandle<T> {
    /// Get a reference to the pooled object
    pub fn get(&self) -> &T {
        self.object.as_ref().unwrap()
    }

    /// Get a mutable reference to the pooled object
    pub fn get_mut(&mut self) -> &mut T {
        self.object.as_mut().unwrap()
    }
}

impl<T> Drop for PoolHandle<T> {
    fn drop(&mut self) {
        if let Some(obj) = self.object.take() {
            if let Ok(mut pool) = self.pool.lock() {
                pool.return_to_pool(obj);
            }
        }
    }
}

impl<T> std::ops::Deref for PoolHandle<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.get()
    }
}

impl<T> std::ops::DerefMut for PoolHandle<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.get_mut()
    }
}

/// Generic object pool for efficient reuse
pub struct ObjectPool<T> {
    available: Vec<T>,
    in_use: usize,
    max_size: usize,
    total_allocated: usize,
    total_reused: usize,
}

impl<T> ObjectPool<T> {
    /// Create a new object pool with specified capacity
    pub fn new(initial_capacity: usize, max_size: usize) -> Self {
        Self {
            available: Vec::with_capacity(initial_capacity),
            in_use: 0,
            max_size,
            total_allocated: 0,
            total_reused: 0,
        }
    }

    /// Create a new pool with default settings
    pub fn with_capacity(capacity: usize) -> Self {
        Self::new(capacity, capacity * 4)
    }

    /// Get current pool statistics
    pub fn stats(&self) -> PoolStats {
        PoolStats {
            available: self.available.len(),
            in_use: self.in_use,
            max_size: self.max_size,
            total_allocated: self.total_allocated,
            total_reused: self.total_reused,
            reuse_rate: if self.total_allocated > 0 {
                self.total_reused as f32 / self.total_allocated as f32
            } else {
                0.0
            },
        }
    }

    /// Return an object to the pool
    fn return_to_pool(&mut self, obj: T) {
        self.in_use = self.in_use.saturating_sub(1);

        // Only keep if under max size
        if self.available.len() < self.max_size {
            self.available.push(obj);
        }
    }

    /// Clear all available objects
    pub fn clear(&mut self) {
        self.available.clear();
    }

    /// Preallocate objects in the pool
    pub fn preallocate(&mut self, count: usize)
    where
        T: Default,
    {
        self.available.reserve(count);
        for _ in 0..count {
            self.available.push(T::default());
        }
    }
}

impl<T: Default> ObjectPool<T> {
    /// Acquire an object from the pool
    pub fn acquire(&mut self) -> T {
        self.in_use += 1;
        self.total_allocated += 1;

        if let Some(obj) = self.available.pop() {
            self.total_reused += 1;
            obj
        } else {
            T::default()
        }
    }

    /// Release an object back to the pool
    pub fn release(&mut self, obj: T) {
        self.return_to_pool(obj);
    }
}

/// Thread-safe object pool wrapper
pub struct ThreadSafePool<T> {
    pool: Arc<Mutex<ObjectPool<T>>>,
}

impl<T> Clone for ThreadSafePool<T> {
    fn clone(&self) -> Self {
        Self {
            pool: Arc::clone(&self.pool),
        }
    }
}

impl<T> ThreadSafePool<T> {
    /// Create a new thread-safe pool
    pub fn new(initial_capacity: usize, max_size: usize) -> Self {
        Self {
            pool: Arc::new(Mutex::new(ObjectPool::new(initial_capacity, max_size))),
        }
    }

    /// Get pool statistics
    pub fn stats(&self) -> PoolStats {
        self.pool.lock().unwrap().stats()
    }

    /// Clear the pool
    pub fn clear(&self) {
        self.pool.lock().unwrap().clear();
    }
}

impl<T: Default> ThreadSafePool<T> {
    /// Acquire an object with automatic return on drop
    pub fn acquire(&self) -> PoolHandle<T> {
        let obj = self.pool.lock().unwrap().acquire();
        PoolHandle {
            object: Some(obj),
            pool: Arc::clone(&self.pool),
        }
    }

    /// Preallocate objects
    pub fn preallocate(&self, count: usize) {
        self.pool.lock().unwrap().preallocate(count);
    }
}

/// Pool statistics
#[derive(Debug, Clone, Copy)]
pub struct PoolStats {
    pub available: usize,
    pub in_use: usize,
    pub max_size: usize,
    pub total_allocated: usize,
    pub total_reused: usize,
    pub reuse_rate: f32,
}

/// Specialized pools for common WW3D objects
pub mod specialized_pools {
    use super::*;
    use glam::{Mat4, Vec3};

    /// Transform data for caching
    #[derive(Clone, Default)]
    pub struct TransformData {
        pub world_matrix: Mat4,
        pub dirty: bool,
        pub parent_index: Option<usize>,
    }

    /// Particle data
    #[derive(Clone)]
    pub struct ParticleData {
        pub position: Vec3,
        pub velocity: Vec3,
        pub color: [f32; 4],
        pub size: f32,
        pub lifetime: f32,
        pub age: f32,
    }

    impl Default for ParticleData {
        fn default() -> Self {
            Self {
                position: Vec3::ZERO,
                velocity: Vec3::ZERO,
                color: [1.0, 1.0, 1.0, 1.0],
                size: 1.0,
                lifetime: 1.0,
                age: 0.0,
            }
        }
    }

    /// Collision contact data
    #[derive(Clone, Default)]
    pub struct ContactData {
        pub point: Vec3,
        pub normal: Vec3,
        pub penetration: f32,
        pub body_a: usize,
        pub body_b: usize,
    }

    /// Pre-configured pools for common object types
    pub struct EnginePoolCollection {
        pub transforms: ThreadSafePool<TransformData>,
        pub particles: ThreadSafePool<ParticleData>,
        pub contacts: ThreadSafePool<ContactData>,
    }

    impl Default for EnginePoolCollection {
        fn default() -> Self {
            Self::new()
        }
    }

    impl EnginePoolCollection {
        /// Create pools with sensible defaults for a game engine
        pub fn new() -> Self {
            Self {
                // Transform pool: 1000 initial, up to 10000
                transforms: ThreadSafePool::new(1000, 10000),
                // Particle pool: 5000 initial, up to 50000
                particles: ThreadSafePool::new(5000, 50000),
                // Contact pool: 500 initial, up to 5000
                contacts: ThreadSafePool::new(500, 5000),
            }
        }

        /// Preallocate all pools
        pub fn preallocate(&self) {
            self.transforms.preallocate(1000);
            self.particles.preallocate(5000);
            self.contacts.preallocate(500);
        }

        /// Get statistics for all pools
        pub fn stats(&self) -> PoolCollectionStats {
            PoolCollectionStats {
                transforms: self.transforms.stats(),
                particles: self.particles.stats(),
                contacts: self.contacts.stats(),
            }
        }
    }

    /// Statistics for the entire pool collection
    #[derive(Debug)]
    pub struct PoolCollectionStats {
        pub transforms: PoolStats,
        pub particles: PoolStats,
        pub contacts: PoolStats,
    }

    impl std::fmt::Display for PoolCollectionStats {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            writeln!(f, "Pool Statistics:")?;
            writeln!(
                f,
                "  Transforms: {}/{} in use, {:.1}% reuse rate",
                self.transforms.in_use,
                self.transforms.max_size,
                self.transforms.reuse_rate * 100.0
            )?;
            writeln!(
                f,
                "  Particles:  {}/{} in use, {:.1}% reuse rate",
                self.particles.in_use,
                self.particles.max_size,
                self.particles.reuse_rate * 100.0
            )?;
            writeln!(
                f,
                "  Contacts:   {}/{} in use, {:.1}% reuse rate",
                self.contacts.in_use,
                self.contacts.max_size,
                self.contacts.reuse_rate * 100.0
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_object_pool_basic() {
        let mut pool: ObjectPool<Vec<i32>> = ObjectPool::new(10, 100);

        // Acquire and release
        let mut obj = pool.acquire();
        obj.push(42);
        pool.release(obj);

        // Should reuse the same object
        let obj2 = pool.acquire();
        assert_eq!(obj2.len(), 1); // Not cleared by default
    }

    #[test]
    fn test_thread_safe_pool() {
        let pool = ThreadSafePool::<Vec<i32>>::new(10, 100);

        {
            let mut handle = pool.acquire();
            handle.push(42);
        } // Automatically returned to pool

        let handle2 = pool.acquire();
        // Pool doesn't clear by default, so length is preserved
        assert_eq!(handle2.len(), 1);
    }

    #[test]
    fn test_pool_stats() {
        let mut pool: ObjectPool<Vec<i32>> = ObjectPool::new(5, 20);

        let obj1 = pool.acquire();
        let stats = pool.stats();
        assert_eq!(stats.in_use, 1);
        assert_eq!(stats.total_allocated, 1);

        pool.release(obj1);
        let _obj2 = pool.acquire();
        let stats = pool.stats();
        assert_eq!(stats.total_reused, 1);
        assert!(stats.reuse_rate > 0.0);
    }

    #[test]
    fn test_specialized_pools() {
        use specialized_pools::*;

        let pools = EnginePoolCollection::new();
        pools.preallocate();

        // Test particle pool
        {
            let mut particle = pools.particles.acquire();
            particle.position = glam::Vec3::new(1.0, 2.0, 3.0);
        }

        let stats = pools.stats();
        assert!(stats.particles.total_allocated > 0);
    }
}
