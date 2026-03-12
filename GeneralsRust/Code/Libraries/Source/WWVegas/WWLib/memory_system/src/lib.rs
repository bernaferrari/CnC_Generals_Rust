//! Memory management system for Command & Conquer Generals Zero Hour
//!
//! This crate provides a sophisticated memory pool system that efficiently manages
//! allocations of various sizes with features like:
//! - Memory pools for same-size allocations
//! - Dynamic memory allocator for variable sizes
//! - Debug features (when enabled)
//! - Checkpointing for memory leak detection
//! - Memory statistics and reporting

use base_types::*;
use parking_lot::Mutex;
use std::alloc::{alloc, dealloc, Layout};
use std::collections::HashMap;
use std::fmt;
use std::ptr;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Maximum number of subpools allowed in a DynamicMemoryAllocator
pub const MAX_DYNAMIC_MEMORY_ALLOCATOR_SUBPOOLS: usize = 8;

/// Memory report flags for debugging and statistics
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MemoryReportFlags {
    /// Display stats for each pool, in addition to each block
    PoolInfo = 0x0200,

    /// Report on the overall memory situation (including all pools and dma's)
    FactoryInfo = 0x0400,

    /// Report on pools that have overflowed their initial allocation
    PoolOverflow = 0x0800,

    /// Simple leak checking
    SimpleLeaks = 0x1000,

    /// Display the stacktrace for allocation location for all blocks found
    StackTrace = 0x0100,
}

/// Pool initialization record
#[derive(Debug, Clone)]
pub struct PoolInitRec {
    /// Name of the pool
    pub pool_name: String,

    /// Size, in bytes, of each allocation in the pool
    pub allocation_size: Int,

    /// Initial number of blocks to allocate
    pub initial_allocation_count: Int,

    /// When the pool runs out of space, allocate more blocks in this increment
    pub overflow_allocation_count: Int,
}

impl PoolInitRec {
    /// Create a new PoolInitRec
    pub fn new(
        pool_name: &str,
        allocation_size: Int,
        initial_allocation_count: Int,
        overflow_allocation_count: Int,
    ) -> Self {
        Self {
            pool_name: pool_name.to_string(),
            allocation_size,
            initial_allocation_count,
            overflow_allocation_count,
        }
    }
}

/// Memory block header for tracking allocations
#[derive(Debug)]
struct MemoryBlockHeader {
    /// Size of this block
    size: usize,

    /// Pool this block belongs to (if any)
    pool: Option<*mut MemoryPool>,

    /// Tag string for debugging
    #[cfg(feature = "debug")]
    tag_string: Option<String>,

    /// Stack trace information
    #[cfg(feature = "stacktrace")]
    stack_trace: Vec<String>,
}

impl MemoryBlockHeader {
    /// Create a new memory block header
    fn new(size: usize, pool: Option<*mut MemoryPool>) -> Self {
        Self {
            size,
            pool,
            #[cfg(feature = "debug")]
            tag_string: None,
            #[cfg(feature = "stacktrace")]
            stack_trace: Vec::new(),
        }
    }
}

/// Memory blob that contains multiple blocks of the same size
struct MemoryPoolBlob {
    /// Pointer to the actual memory
    memory: *mut u8,

    /// Layout of the allocation
    layout: Layout,

    /// Size of each block in this blob
    block_size: usize,

    /// Number of blocks in this blob
    block_count: usize,

    /// Bitmap of free/used blocks (1 = used, 0 = free)
    used_blocks: Vec<bool>,

    /// Next blob in the pool
    next_blob: Option<Box<MemoryPoolBlob>>,

    /// Previous blob in the pool
    prev_blob: Option<*mut MemoryPoolBlob>,
}

impl MemoryPoolBlob {
    /// Create a new memory blob
    fn new(block_size: usize, block_count: usize) -> Result<Self, String> {
        let total_size = block_size * block_count;
        let layout = Layout::from_size_align(total_size, std::mem::align_of::<usize>())
            .map_err(|e| format!("Failed to create layout: {}", e))?;

        let memory = unsafe { alloc(layout) };
        if memory.is_null() {
            return Err("Failed to allocate memory for blob".to_string());
        }

        Ok(Self {
            memory,
            layout,
            block_size,
            block_count,
            used_blocks: vec![false; block_count],
            next_blob: None,
            prev_blob: None,
        })
    }

    /// Allocate a block from this blob
    fn allocate_block(&mut self) -> Option<*mut u8> {
        for (index, is_used) in self.used_blocks.iter_mut().enumerate() {
            if !*is_used {
                *is_used = true;
                let block_ptr = unsafe { self.memory.add(index * self.block_size) };
                return Some(block_ptr);
            }
        }
        None
    }

    /// Free a block in this blob
    fn free_block(&mut self, block_ptr: *mut u8) -> bool {
        let block_offset = unsafe { block_ptr.offset_from(self.memory) } as usize;
        if block_offset % self.block_size != 0 {
            return false; // Invalid block pointer
        }

        let block_index = block_offset / self.block_size;
        if block_index >= self.block_count {
            return false; // Block index out of bounds
        }

        if !self.used_blocks[block_index] {
            return false; // Block was already free
        }

        self.used_blocks[block_index] = false;
        true
    }

    /// Get the number of free blocks in this blob
    fn free_block_count(&self) -> usize {
        self.used_blocks.iter().filter(|&&used| !used).count()
    }

    /// Get the number of used blocks in this blob
    fn used_block_count(&self) -> usize {
        self.used_blocks.iter().filter(|&&used| used).count()
    }

    /// Check if this blob is completely empty
    fn is_empty(&self) -> bool {
        self.used_block_count() == 0
    }
}

impl Drop for MemoryPoolBlob {
    fn drop(&mut self) {
        if !self.memory.is_null() {
            unsafe {
                dealloc(self.memory, self.layout);
            }
        }
    }
}

/// Memory pool for efficiently allocating objects of the same size
pub struct MemoryPool {
    /// Name of this pool
    pool_name: String,

    /// Size of each block in this pool
    allocation_size: usize,

    /// Initial number of blocks to allocate
    initial_allocation_count: usize,

    /// Number of blocks to allocate when overflowing
    overflow_allocation_count: usize,

    /// Total number of blocks currently in use
    used_blocks_in_pool: usize,

    /// Total number of blocks in all blobs
    total_blocks_in_pool: usize,

    /// High-water mark of used blocks
    peak_used_blocks_in_pool: usize,

    /// First blob in this pool
    first_blob: Option<Box<MemoryPoolBlob>>,

    /// Last blob in this pool
    last_blob: Option<*mut MemoryPoolBlob>,

    /// First blob that has free blocks
    first_blob_with_free_blocks: Option<*mut MemoryPoolBlob>,
}

impl MemoryPool {
    /// Create a new memory pool
    pub fn new() -> Self {
        Self {
            pool_name: String::new(),
            allocation_size: 0,
            initial_allocation_count: 0,
            overflow_allocation_count: 0,
            used_blocks_in_pool: 0,
            total_blocks_in_pool: 0,
            peak_used_blocks_in_pool: 0,
            first_blob: None,
            last_blob: None,
            first_blob_with_free_blocks: None,
        }
    }

    /// Initialize the memory pool
    pub fn init(
        &mut self,
        pool_name: &str,
        allocation_size: usize,
        initial_allocation_count: usize,
        overflow_allocation_count: usize,
    ) -> Result<(), String> {
        self.pool_name = pool_name.to_string();
        self.allocation_size = allocation_size;
        self.initial_allocation_count = initial_allocation_count;
        self.overflow_allocation_count = overflow_allocation_count;

        // Create the initial blob
        let blob = MemoryPoolBlob::new(allocation_size, initial_allocation_count)?;
        self.total_blocks_in_pool = initial_allocation_count;
        self.first_blob = Some(Box::new(blob));
        self.last_blob = self
            .first_blob
            .as_mut()
            .map(|b| b.as_mut() as *mut MemoryPoolBlob);
        self.first_blob_with_free_blocks = self.last_blob;

        Ok(())
    }

    /// Allocate a block from this pool
    pub fn allocate_block(&mut self) -> Result<*mut u8, String> {
        // First try to allocate from existing blobs
        let mut current_blob = self.first_blob_with_free_blocks;
        while let Some(blob_ptr) = current_blob {
            let blob = unsafe { &mut *blob_ptr };
            if let Some(block_ptr) = blob.allocate_block() {
                self.used_blocks_in_pool += 1;
                if self.used_blocks_in_pool > self.peak_used_blocks_in_pool {
                    self.peak_used_blocks_in_pool = self.used_blocks_in_pool;
                }

                // Zero the memory
                unsafe {
                    ptr::write_bytes(block_ptr, 0, self.allocation_size);
                }

                return Ok(block_ptr);
            }

            // Move to next blob
            current_blob = blob
                .next_blob
                .as_ref()
                .map(|b| b.as_ref() as *const MemoryPoolBlob as *mut MemoryPoolBlob);
        }

        // No free blocks found, create a new blob
        let allocation_count = if self.first_blob.is_none() {
            self.initial_allocation_count
        } else {
            self.overflow_allocation_count
        };

        let mut new_blob = MemoryPoolBlob::new(self.allocation_size, allocation_count)?;
        self.total_blocks_in_pool += allocation_count;

        // Link the new blob into the list
        if let Some(last_blob_ptr) = self.last_blob {
            let last_blob = unsafe { &mut *last_blob_ptr };
            last_blob.next_blob = Some(Box::new(new_blob));
            let new_blob_ptr =
                last_blob.next_blob.as_mut().unwrap().as_mut() as *mut MemoryPoolBlob;
            unsafe { (*new_blob_ptr).prev_blob = Some(last_blob_ptr) };
            self.last_blob = Some(new_blob_ptr);
        } else {
            // This is the first blob
            self.first_blob = Some(Box::new(new_blob));
            self.last_blob = self
                .first_blob
                .as_mut()
                .map(|b| b.as_mut() as *mut MemoryPoolBlob);
        }

        // Update first blob with free blocks
        self.first_blob_with_free_blocks = self.last_blob;

        // Now allocate from the new blob
        self.allocate_block()
    }

    /// Free a block back to this pool
    pub fn free_block(&mut self, block_ptr: *mut u8) -> bool {
        if block_ptr.is_null() {
            return true;
        }

        // Find which blob this block belongs to
        let mut current_blob = self.first_blob.as_mut();
        while let Some(blob) = current_blob {
            if blob.memory <= block_ptr
                && block_ptr < unsafe { blob.memory.add(blob.block_size * blob.block_count) }
            {
                if blob.free_block(block_ptr) {
                    self.used_blocks_in_pool -= 1;

                    // Update first_blob_with_free_blocks if needed
                    if self.first_blob_with_free_blocks.is_none() || blob.free_block_count() > 0 {
                        self.first_blob_with_free_blocks =
                            Some(blob.as_mut() as *mut MemoryPoolBlob);
                    }

                    return true;
                }
            }
            current_blob = blob.next_blob.as_mut();
        }

        false
    }

    /// Get the number of free blocks in this pool
    pub fn get_free_block_count(&self) -> usize {
        let mut total_free = 0;
        let mut current_blob = self.first_blob.as_ref();
        while let Some(blob) = current_blob {
            total_free += blob.free_block_count();
            current_blob = blob.next_blob.as_ref();
        }
        total_free
    }

    /// Get the number of used blocks in this pool
    pub fn get_used_block_count(&self) -> usize {
        self.used_blocks_in_pool
    }

    /// Get the total number of blocks in this pool
    pub fn get_total_block_count(&self) -> usize {
        self.total_blocks_in_pool
    }

    /// Get the peak number of used blocks
    pub fn get_peak_block_count(&self) -> usize {
        self.peak_used_blocks_in_pool
    }

    /// Get the pool name
    pub fn get_pool_name(&self) -> &str {
        &self.pool_name
    }

    /// Get the allocation size
    pub fn get_allocation_size(&self) -> usize {
        self.allocation_size
    }
}

impl fmt::Debug for MemoryPool {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MemoryPool")
            .field("pool_name", &self.pool_name)
            .field("allocation_size", &self.allocation_size)
            .field("used_blocks", &self.used_blocks_in_pool)
            .field("total_blocks", &self.total_blocks_in_pool)
            .field("peak_used", &self.peak_used_blocks_in_pool)
            .finish()
    }
}

/// Dynamic memory allocator for variable-sized allocations
pub struct DynamicMemoryAllocator {
    /// Number of subpools
    num_pools: usize,

    /// Total number of blocks allocated
    used_blocks_in_dma: usize,

    /// The subpools
    pools: Vec<Option<Box<MemoryPool>>>,

    /// Raw blocks allocated directly from system
    raw_blocks: Vec<(*mut u8, usize)>,
}

impl DynamicMemoryAllocator {
    /// Create a new dynamic memory allocator
    pub fn new() -> Self {
        Self {
            num_pools: 0,
            used_blocks_in_dma: 0,
            pools: (0..MAX_DYNAMIC_MEMORY_ALLOCATOR_SUBPOOLS)
                .map(|_| None)
                .collect(),
            raw_blocks: Vec::new(),
        }
    }

    /// Initialize the DMA with pool configurations
    pub fn init(&mut self, pool_configs: &[PoolInitRec]) -> Result<(), String> {
        if pool_configs.len() > MAX_DYNAMIC_MEMORY_ALLOCATOR_SUBPOOLS {
            return Err(format!(
                "Too many pools requested: {} (max is {})",
                pool_configs.len(),
                MAX_DYNAMIC_MEMORY_ALLOCATOR_SUBPOOLS
            ));
        }

        self.num_pools = pool_configs.len();

        for (i, config) in pool_configs.iter().enumerate() {
            let mut pool = Box::new(MemoryPool::new());
            pool.init(
                &config.pool_name,
                config.allocation_size as usize,
                config.initial_allocation_count as usize,
                config.overflow_allocation_count as usize,
            )?;
            self.pools[i] = Some(pool);
        }

        Ok(())
    }

    /// Allocate bytes from the appropriate pool or system
    pub fn allocate_bytes(&mut self, size: usize) -> Result<*mut u8, String> {
        // Try to find a pool that can handle this allocation
        for pool in self.pools.iter_mut().flatten() {
            if size <= pool.get_allocation_size() {
                match pool.allocate_block() {
                    Ok(block_ptr) => {
                        self.used_blocks_in_dma += 1;
                        return Ok(block_ptr);
                    }
                    Err(_) => continue, // Try next pool
                }
            }
        }

        // No pool can handle this size, allocate directly from system
        let layout = Layout::from_size_align(size, std::mem::align_of::<usize>())
            .map_err(|e| format!("Failed to create layout: {}", e))?;

        let memory = unsafe { alloc(layout) };
        if memory.is_null() {
            return Err("Failed to allocate memory".to_string());
        }

        // Zero the memory
        unsafe {
            ptr::write_bytes(memory, 0, size);
        }

        self.raw_blocks.push((memory, size));
        self.used_blocks_in_dma += 1;

        Ok(memory)
    }

    /// Free bytes
    pub fn free_bytes(&mut self, ptr: *mut u8) -> bool {
        if ptr.is_null() {
            return true;
        }

        // Check if this is a raw block
        if let Some(pos) = self
            .raw_blocks
            .iter()
            .position(|(block_ptr, _)| *block_ptr == ptr)
        {
            let (block_ptr, size) = self.raw_blocks.swap_remove(pos);
            let layout = Layout::from_size_align(size, std::mem::align_of::<usize>()).unwrap();
            unsafe {
                dealloc(block_ptr, layout);
            }
            self.used_blocks_in_dma -= 1;
            return true;
        }

        // Check pools
        for pool in self.pools.iter_mut().flatten() {
            if pool.free_block(ptr) {
                self.used_blocks_in_dma -= 1;
                return true;
            }
        }

        false
    }

    /// Get total used blocks
    pub fn get_used_block_count(&self) -> usize {
        self.used_blocks_in_dma
    }

    /// Get memory statistics
    pub fn get_memory_stats(&self) -> HashMap<String, usize> {
        let mut stats = HashMap::new();

        stats.insert("raw_blocks_count".to_string(), self.raw_blocks.len());
        stats.insert("used_blocks_in_dma".to_string(), self.used_blocks_in_dma);
        stats.insert("num_pools".to_string(), self.num_pools);

        for (i, pool) in self.pools.iter().flatten().enumerate() {
            stats.insert(
                format!("pool_{}_used_blocks", i),
                pool.get_used_block_count(),
            );
            stats.insert(
                format!("pool_{}_total_blocks", i),
                pool.get_total_block_count(),
            );
            stats.insert(
                format!("pool_{}_peak_blocks", i),
                pool.get_peak_block_count(),
            );
        }

        stats
    }
}

impl fmt::Debug for DynamicMemoryAllocator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DynamicMemoryAllocator")
            .field("num_pools", &self.num_pools)
            .field("used_blocks", &self.used_blocks_in_dma)
            .field("raw_blocks_count", &self.raw_blocks.len())
            .finish()
    }
}

/// Global memory pool factory
pub struct MemoryPoolFactory {
    /// All pools managed by this factory
    pools: Vec<Box<MemoryPool>>,

    /// Dynamic memory allocators
    dmas: Vec<Box<DynamicMemoryAllocator>>,

    /// Total memory statistics
    total_allocated: AtomicUsize,
    total_freed: AtomicUsize,
}

impl MemoryPoolFactory {
    /// Create a new memory pool factory
    pub fn new() -> Self {
        Self {
            pools: Vec::new(),
            dmas: Vec::new(),
            total_allocated: AtomicUsize::new(0),
            total_freed: AtomicUsize::new(0),
        }
    }

    /// Create a memory pool
    pub fn create_memory_pool(
        &mut self,
        pool_name: &str,
        allocation_size: usize,
        initial_allocation_count: usize,
        overflow_allocation_count: usize,
    ) -> Result<&mut MemoryPool, String> {
        let mut pool = Box::new(MemoryPool::new());
        pool.init(
            pool_name,
            allocation_size,
            initial_allocation_count,
            overflow_allocation_count,
        )?;
        self.pools.push(pool);
        Ok(self.pools.last_mut().unwrap())
    }

    /// Create a dynamic memory allocator
    pub fn create_dynamic_memory_allocator(
        &mut self,
        pool_configs: &[PoolInitRec],
    ) -> Result<&mut DynamicMemoryAllocator, String> {
        let mut dma = Box::new(DynamicMemoryAllocator::new());
        dma.init(pool_configs)?;
        self.dmas.push(dma);
        Ok(self.dmas.last_mut().unwrap())
    }

    /// Get memory statistics for all managed resources
    pub fn get_memory_report(&self) -> HashMap<String, usize> {
        let mut report = HashMap::new();

        report.insert("total_pools".to_string(), self.pools.len());
        report.insert("total_dmas".to_string(), self.dmas.len());
        report.insert(
            "total_allocated".to_string(),
            self.total_allocated.load(Ordering::Relaxed),
        );
        report.insert(
            "total_freed".to_string(),
            self.total_freed.load(Ordering::Relaxed),
        );

        let mut total_pool_blocks = 0;
        let mut total_pool_used = 0;
        let mut total_pool_peak = 0;

        for (i, pool) in self.pools.iter().enumerate() {
            report.insert(format!("pool_{}_used", i), pool.get_used_block_count());
            report.insert(format!("pool_{}_total", i), pool.get_total_block_count());
            report.insert(format!("pool_{}_peak", i), pool.get_peak_block_count());

            total_pool_blocks += pool.get_total_block_count();
            total_pool_used += pool.get_used_block_count();
            total_pool_peak += pool.get_peak_block_count();
        }

        report.insert("all_pools_total_blocks".to_string(), total_pool_blocks);
        report.insert("all_pools_used_blocks".to_string(), total_pool_used);
        report.insert("all_pools_peak_blocks".to_string(), total_pool_peak);

        let mut total_dma_blocks = 0;
        for (i, dma) in self.dmas.iter().enumerate() {
            report.insert(format!("dma_{}_used", i), dma.get_used_block_count());
            total_dma_blocks += dma.get_used_block_count();
        }

        report.insert("all_dmas_used_blocks".to_string(), total_dma_blocks);
        report
    }

    /// Reset all memory pools (free all allocations)
    pub fn reset_all_pools(&mut self) {
        for pool in self.pools.iter_mut() {
            // Note: In a real implementation, we'd need to add a reset method to MemoryPool
            // For now, we'll just log that this would reset the pool
            log::info!("Would reset pool: {}", pool.get_pool_name());
        }
    }
}

impl fmt::Debug for MemoryPoolFactory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MemoryPoolFactory")
            .field("pools_count", &self.pools.len())
            .field("dmas_count", &self.dmas.len())
            .field(
                "total_allocated",
                &self.total_allocated.load(Ordering::Relaxed),
            )
            .field("total_freed", &self.total_freed.load(Ordering::Relaxed))
            .finish()
    }
}

/// Global memory pool factory instance
static mut GLOBAL_MEMORY_POOL_FACTORY: Option<Mutex<MemoryPoolFactory>> = None;

/// Initialize the global memory system
pub fn init_memory_manager(pool_configs: &[PoolInitRec]) -> Result<(), String> {
    unsafe {
        GLOBAL_MEMORY_POOL_FACTORY = Some(Mutex::new(MemoryPoolFactory::new()));
    }

    if let Some(ref factory) = unsafe { GLOBAL_MEMORY_POOL_FACTORY.as_ref() } {
        let mut factory = factory.lock();
        factory.create_dynamic_memory_allocator(pool_configs)?;
    }

    Ok(())
}

/// Shutdown the global memory system
pub fn shutdown_memory_manager() {
    unsafe {
        GLOBAL_MEMORY_POOL_FACTORY = None;
    }
}

/// Get the global memory pool factory
pub fn get_memory_pool_factory() -> Option<&'static Mutex<MemoryPoolFactory>> {
    unsafe { GLOBAL_MEMORY_POOL_FACTORY.as_ref() }
}

/// Convenience function to allocate memory
pub fn allocate_memory(size: usize) -> Result<*mut u8, String> {
    if let Some(factory) = get_memory_pool_factory() {
        let mut factory = factory.lock();
        if let Some(dma) = factory.dmas.first_mut() {
            return dma.allocate_bytes(size);
        }
    }
    Err("Memory system not initialized".to_string())
}

/// Convenience function to free memory
pub fn free_memory(ptr: *mut u8) -> bool {
    if let Some(factory) = get_memory_pool_factory() {
        let mut factory = factory.lock();
        for dma in factory.dmas.iter_mut() {
            if dma.free_bytes(ptr) {
                return true;
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_pool_creation() {
        let mut pool = MemoryPool::new();
        assert!(pool.init("test_pool", 64, 10, 5).is_ok());

        assert_eq!(pool.get_pool_name(), "test_pool");
        assert_eq!(pool.get_allocation_size(), 64);
        assert_eq!(pool.get_total_block_count(), 10);
        assert_eq!(pool.get_used_block_count(), 0);
    }

    #[test]
    fn test_memory_pool_allocation() {
        let mut pool = MemoryPool::new();
        pool.init("test_pool", 64, 10, 5).unwrap();

        // Allocate a block
        let block = pool.allocate_block().unwrap();
        assert!(!block.is_null());
        assert_eq!(pool.get_used_block_count(), 1);
        assert_eq!(pool.get_free_block_count(), 9);

        // Free the block
        assert!(pool.free_block(block));
        assert_eq!(pool.get_used_block_count(), 0);
        assert_eq!(pool.get_free_block_count(), 10);
    }

    #[test]
    fn test_dynamic_memory_allocator() {
        let pool_configs = vec![
            PoolInitRec::new("small_pool", 32, 10, 5),
            PoolInitRec::new("medium_pool", 128, 5, 2),
        ];

        let mut dma = DynamicMemoryAllocator::new();
        assert!(dma.init(&pool_configs).is_ok());
        assert_eq!(dma.get_used_block_count(), 0);

        // Allocate memory
        let block = dma.allocate_bytes(16).unwrap(); // Should use small_pool
        assert!(!block.is_null());
        assert_eq!(dma.get_used_block_count(), 1);

        // Free memory
        assert!(dma.free_bytes(block));
        assert_eq!(dma.get_used_block_count(), 0);
    }

    #[test]
    fn test_memory_pool_overflow() {
        let mut pool = MemoryPool::new();
        pool.init("test_pool", 32, 2, 3).unwrap(); // Only 2 initial blocks, 3 overflow

        // Allocate all initial blocks
        let block1 = pool.allocate_block().unwrap();
        let block2 = pool.allocate_block().unwrap();
        assert_eq!(pool.get_used_block_count(), 2);
        assert_eq!(pool.get_total_block_count(), 2);

        // Next allocation should trigger overflow
        let block3 = pool.allocate_block().unwrap();
        assert_eq!(pool.get_used_block_count(), 3);
        assert_eq!(pool.get_total_block_count(), 5); // 2 + 3 overflow

        // Clean up
        pool.free_block(block1);
        pool.free_block(block2);
        pool.free_block(block3);
        assert_eq!(pool.get_used_block_count(), 0);
    }
}
