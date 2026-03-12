//! Rust-idiomatic Memory Pooling System for Game Objects
//!
//! This module provides efficient pool-based memory allocation for game objects,
//! modules, and other frequently allocated types. It replaces C++'s MEMORY_POOL_GLUE
//! macros with a safe, ergonomic Rust API that:
//!
//! - Uses generational indices for stable object IDs
//! - Provides thread-safe pool access via Arc/RwLock
//! - Implements cache-friendly memory layouts
//! - Tracks detailed allocation statistics
//! - Prevents use-after-free via compile-time guarantees
//! - Integrates seamlessly with the Object and Module factories
//!
//! # Architecture
//!
//! The system consists of several layers:
//!
//! 1. **ObjectPool<T>**: Generic typed pool with generational indices
//! 2. **PoolAllocator**: Low-level slab allocator for typed arenas
//! 3. **PoolRegistry**: Global registry for managing multiple pools
//! 4. **PoolHandle**: Smart handle with automatic cleanup
//! 5. **PoolStats**: Comprehensive statistics and monitoring
//!
//! # Usage Example
//!
//! ```rust,ignore
//! use memory::{ObjectPool, PoolConfig, PoolHandle};
//!
//! // Create a pool for a specific type
//! let pool = ObjectPool::<GameObject>::new(PoolConfig {
//!     name: "GameObjects",
//!     initial_capacity: 1024,
//!     grow_by: 256,
//!     cache_line_aligned: true,
//! });
//!
//! // Allocate an object (returns generational handle)
//! let handle: PoolHandle<GameObject> = pool.alloc(GameObject::new());
//!
//! // Access the object
//! handle.with(|obj| {
//!     obj.update();
//! });
//!
//! // Object is automatically freed when handle drops
//! ```

pub mod allocator;
pub mod config;
pub mod generation;
pub mod handle;
pub mod pool;
pub mod registry;
pub mod stats;

// Re-export main types
pub use allocator::PoolAllocator;
pub use config::{PoolConfig, PoolConfigBuilder};
pub use generation::{Generation, GenerationalIndex};
pub use handle::{PoolHandle, WeakPoolHandle};
pub use pool::{ObjectPool, PoolFactory};
pub use registry::{PoolRegistry, POOL_REGISTRY};
pub use stats::{AllocationStats, MemoryStats, PoolStats};

// Re-export debug tracking (conditional compilation)
#[cfg(debug_assertions)]
pub use pool::{DebugTracker, LeakInfo, PoolDebugExt};

// Re-export specialized pools
pub mod specialized;
pub use specialized::{ModulePoolRegistry, ObjectPoolRegistry};

// Benchmark utilities
#[cfg(test)]
pub mod benchmarks;

#[cfg(test)]
mod tests;
