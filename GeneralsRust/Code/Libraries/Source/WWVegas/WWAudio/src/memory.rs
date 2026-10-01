//! Memory management for audio buffers and resources.

use crate::error::Result;
use parking_lot::Mutex;

/// Memory allocation strategy
#[derive(Debug, Clone, Copy)]
pub enum AllocationStrategy {
    /// Standard system allocator
    System,
    /// Custom pool allocator
    Pool,
    /// Stack-based allocator for small allocations
    Stack,
}

/// Memory pool for audio buffers
pub struct MemoryPool {
    _strategy: AllocationStrategy,
    pool_size: usize,
    _block_size: usize,
}

/// Audio memory manager
pub struct AudioMemoryManager {
    pools: Vec<MemoryPool>,
    /// Allocation counters guarded by a single lock
    state: Mutex<CombinedState>,
}

/// Total/peak allocation tracking (previously three locks)
struct CombinedState {
    total_allocated: usize,
    peak_usage: usize,
    active_allocations: usize,
}

impl AudioMemoryManager {
    /// Create new memory manager
    pub fn new() -> Self {
        Self {
            pools: Vec::new(),
            state: Mutex::new(CombinedState {
                total_allocated: 0,
                peak_usage: 0,
                active_allocations: 0,
            }),
        }
    }

    /// Allocate audio buffer
    pub fn allocate(&self, size: usize) -> Result<Vec<u8>> {
        if size == 0 {
            return Ok(Vec::new());
        }

        {
            let mut state = self.state.lock();
            state.total_allocated = state.total_allocated.saturating_add(size);
            if state.total_allocated > state.peak_usage {
                state.peak_usage = state.total_allocated;
            }
            state.active_allocations = state.active_allocations.saturating_add(1);
        }

        Ok(vec![0u8; size])
    }

    /// Deallocate audio buffer
    pub fn deallocate(&self, buffer: Vec<u8>) -> Result<()> {
        let size = buffer.len();
        drop(buffer);

        {
            let mut state = self.state.lock();
            state.total_allocated = state.total_allocated.saturating_sub(size);
            state.active_allocations = state.active_allocations.saturating_sub(1);
        }

        Ok(())
    }

    /// Get memory statistics
    pub fn stats(&self) -> MemoryStats {
        let state = self.state.lock();

        let pool_capacity: usize = self.pools.iter().map(|pool| pool.pool_size).sum();
        let pool_utilization = if pool_capacity == 0 {
            0.0
        } else {
            (state.total_allocated.min(pool_capacity) as f32) / (pool_capacity as f32)
        };

        MemoryStats {
            total_allocated: state.total_allocated,
            peak_usage: state.peak_usage,
            active_allocations: state.active_allocations,
            pool_utilization,
        }
    }
}

/// Memory usage statistics
#[derive(Debug, Clone)]
pub struct MemoryStats {
    pub total_allocated: usize,
    pub peak_usage: usize,
    pub active_allocations: usize,
    pub pool_utilization: f32,
}

impl MemoryPool {
    /// Create new memory pool
    pub fn new(strategy: AllocationStrategy, pool_size: usize, block_size: usize) -> Self {
        Self {
            _strategy: strategy,
            pool_size,
            _block_size: block_size,
        }
    }
}

impl Default for AudioMemoryManager {
    fn default() -> Self {
        Self::new()
    }
}

// Safety: MemoryPool only carries plain configuration values, so moving it
// across threads and sharing `&MemoryPool` is sound.
unsafe impl Send for MemoryPool {}
unsafe impl Sync for MemoryPool {}
