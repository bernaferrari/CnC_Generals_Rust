//! Memory management for audio buffers and resources.

use crate::error::Result;

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
    _allocated_blocks: Vec<*mut u8>,
}

/// Audio memory manager
///
/// Plain owned accounting state; the manager is driven from a single owner
/// (the audio load path on the game thread), so no lock is needed.
pub struct AudioMemoryManager {
    pools: Vec<MemoryPool>,
    total_allocated: usize,
    peak_usage: usize,
    active_allocations: usize,
}

impl AudioMemoryManager {
    /// Create new memory manager
    pub fn new() -> Self {
        Self {
            pools: Vec::new(),
            total_allocated: 0,
            peak_usage: 0,
            active_allocations: 0,
        }
    }

    /// Allocate audio buffer
    pub fn allocate(&mut self, size: usize) -> Result<Vec<u8>> {
        if size == 0 {
            return Ok(Vec::new());
        }

        self.total_allocated = self.total_allocated.saturating_add(size);
        if self.total_allocated > self.peak_usage {
            self.peak_usage = self.total_allocated;
        }
        self.active_allocations = self.active_allocations.saturating_add(1);

        Ok(vec![0u8; size])
    }

    /// Deallocate audio buffer
    pub fn deallocate(&mut self, buffer: Vec<u8>) -> Result<()> {
        let size = buffer.len();
        drop(buffer);

        self.total_allocated = self.total_allocated.saturating_sub(size);
        self.active_allocations = self.active_allocations.saturating_sub(1);

        Ok(())
    }

    /// Get memory statistics
    pub fn stats(&self) -> MemoryStats {
        let total_allocated = self.total_allocated;
        let peak_usage = self.peak_usage;
        let active_allocations = self.active_allocations;

        let pool_capacity: usize = self.pools.iter().map(|pool| pool.pool_size).sum();
        let pool_utilization = if pool_capacity == 0 {
            0.0
        } else {
            (total_allocated.min(pool_capacity) as f32) / (pool_capacity as f32)
        };

        MemoryStats {
            total_allocated,
            peak_usage,
            active_allocations,
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
            _allocated_blocks: Vec::new(),
        }
    }
}

impl Default for AudioMemoryManager {
    fn default() -> Self {
        Self::new()
    }
}

// SAFETY: The only non-`Copy` field, `_allocated_blocks`, is an owned
// `Vec<*mut u8>` private to this type; the stored raw pointers are never
// dereferenced through this type (the pool performs no allocation itself),
// so moving a MemoryPool across threads is sound.
unsafe impl Send for MemoryPool {}
// SAFETY: Same reasoning as the Send impl: the raw pointers inside the
// owned `_allocated_blocks` Vec are never dereferenced through this type, so
// sharing `&MemoryPool` across threads is sound.
unsafe impl Sync for MemoryPool {}
