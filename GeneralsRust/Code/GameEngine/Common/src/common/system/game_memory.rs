////////////////////////////////////////////////////////////////////////////////
//                                                                            //
//  (c) 2001-2003 Electronic Arts Inc.                                       //
//                                                                            //
////////////////////////////////////////////////////////////////////////////////

//! GameMemory - Memory management system for C&C Generals
//!
//! This module provides comprehensive memory management functionality with
//! Rust safety features, including:
//! - Memory pool allocation
//! - Custom allocators
//! - Memory tracking and debugging
//! - Thread-safe memory operations
//! - RAII memory management
//!
//! Converted from C++ GameMemory.h/cpp to modern Rust

use bumpalo::Bump;
use parking_lot::{Mutex as ParkingMutex, RwLock as ParkingRwLock};
use std::alloc::{alloc, dealloc, Layout};
use std::collections::{HashMap, VecDeque};
use std::ptr::{null_mut, NonNull};
use std::sync::{
    atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
    Arc,
};
use std::thread;
use thiserror::Error;

/// Thread-safe wrapper for memory pointers
#[derive(Debug)]
pub struct SafeMemoryPtr {
    ptr: usize,
}

unsafe impl Send for SafeMemoryPtr {}
unsafe impl Sync for SafeMemoryPtr {}

impl SafeMemoryPtr {
    pub fn new(ptr: NonNull<u8>) -> Self {
        Self {
            ptr: ptr.as_ptr() as usize,
        }
    }

    pub fn as_non_null(&self) -> NonNull<u8> {
        unsafe { NonNull::new_unchecked(self.ptr as *mut u8) }
    }

    pub fn as_ptr(&self) -> *mut u8 {
        self.ptr as *mut u8
    }
}

/// Memory allocation alignment
pub const MEMORY_ALIGNMENT: usize = 16;

/// Memory pool block size
pub const MEMORY_POOL_BLOCK_SIZE: usize = 4096;

/// Maximum number of memory pools
pub const MAX_MEMORY_POOLS: usize = 256;

/// Memory debug bounding wall pattern
pub const MEMORY_BOUNDING_WALL_PATTERN: u32 = 0xDEADBEEF;

/// Memory allocation errors
#[derive(Debug, Error)]
pub enum MemoryError {
    #[error("Out of memory")]
    OutOfMemory,
    #[error("Invalid memory allocation size: {size}")]
    InvalidSize { size: usize },
    #[error("Memory corruption detected at address: {address:p}")]
    MemoryCorruption { address: *const u8 },
    #[error("Double free detected at address: {address:p}")]
    DoubleFree { address: *const u8 },
    #[error("Pool allocation failed for pool: {pool_name}")]
    PoolAllocationFailed { pool_name: String },
    #[error("Invalid memory pool: {pool_name}")]
    InvalidPool { pool_name: String },
    #[error("Memory alignment error: required {required}, got {actual}")]
    AlignmentError { required: usize, actual: usize },
}

/// Memory allocation statistics
#[derive(Debug, Default)]
pub struct MemoryStats {
    pub total_allocated: AtomicUsize,
    pub total_freed: AtomicUsize,
    pub current_allocated: AtomicUsize,
    pub peak_allocated: AtomicUsize,
    pub allocation_count: AtomicUsize,
    pub free_count: AtomicUsize,
    pub pool_allocations: AtomicUsize,
    pub heap_allocations: AtomicUsize,
}

impl MemoryStats {
    /// Record a new allocation
    pub fn record_allocation(&self, size: usize) {
        self.total_allocated.fetch_add(size, Ordering::Relaxed);
        let current = self.current_allocated.fetch_add(size, Ordering::Relaxed) + size;
        self.allocation_count.fetch_add(1, Ordering::Relaxed);

        // Update peak if necessary
        let peak = self.peak_allocated.load(Ordering::Relaxed);
        if current > peak {
            self.peak_allocated.store(current, Ordering::Relaxed);
        }
    }

    /// Record a free operation
    pub fn record_free(&self, size: usize) {
        self.total_freed.fetch_add(size, Ordering::Relaxed);
        self.current_allocated.fetch_sub(size, Ordering::Relaxed);
        self.free_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Record pool allocation
    pub fn record_pool_allocation(&self) {
        self.pool_allocations.fetch_add(1, Ordering::Relaxed);
    }

    /// Record heap allocation
    pub fn record_heap_allocation(&self) {
        self.heap_allocations.fetch_add(1, Ordering::Relaxed);
    }

    /// Get current memory usage summary
    pub fn get_summary(&self) -> MemorySummary {
        MemorySummary {
            total_allocated: self.total_allocated.load(Ordering::Relaxed),
            total_freed: self.total_freed.load(Ordering::Relaxed),
            current_allocated: self.current_allocated.load(Ordering::Relaxed),
            peak_allocated: self.peak_allocated.load(Ordering::Relaxed),
            allocation_count: self.allocation_count.load(Ordering::Relaxed),
            free_count: self.free_count.load(Ordering::Relaxed),
            pool_allocations: self.pool_allocations.load(Ordering::Relaxed),
            heap_allocations: self.heap_allocations.load(Ordering::Relaxed),
        }
    }
}

/// Memory usage summary snapshot
#[derive(Debug, Clone)]
pub struct MemorySummary {
    pub total_allocated: usize,
    pub total_freed: usize,
    pub current_allocated: usize,
    pub peak_allocated: usize,
    pub allocation_count: usize,
    pub free_count: usize,
    pub pool_allocations: usize,
    pub heap_allocations: usize,
}

/// Memory block header for debugging and tracking
#[repr(C, align(16))]
#[derive(Debug)]
pub struct MemoryBlockHeader {
    pub size: usize,
    pub pool_id: u32,
    pub allocation_id: u64,
    pub thread_id: std::thread::ThreadId,
    pub timestamp: std::time::Instant,
    pub file: Option<&'static str>,
    pub line: Option<u32>,
    pub bounding_wall_start: u32,
}

impl MemoryBlockHeader {
    /// Create a new memory block header
    pub fn new(size: usize, pool_id: u32, allocation_id: u64) -> Self {
        Self {
            size,
            pool_id,
            allocation_id,
            thread_id: thread::current().id(),
            timestamp: std::time::Instant::now(),
            file: None,
            line: None,
            bounding_wall_start: MEMORY_BOUNDING_WALL_PATTERN,
        }
    }

    /// Set debug location information
    pub fn set_location(&mut self, file: &'static str, line: u32) {
        self.file = Some(file);
        self.line = Some(line);
    }

    /// Verify memory block integrity
    pub fn verify_integrity(&self) -> Result<(), MemoryError> {
        if self.bounding_wall_start != MEMORY_BOUNDING_WALL_PATTERN {
            return Err(MemoryError::MemoryCorruption {
                address: self as *const _ as *const u8,
            });
        }
        Ok(())
    }
}

/// Memory pool for fixed-size allocations
pub struct MemoryPool {
    pub name: String,
    pub block_size: usize,
    pub alignment: usize,
    pub initial_capacity: usize,
    pub growth_factor: f32,

    // Pool storage
    chunks: ParkingMutex<VecDeque<Chunk>>,
    free_blocks: ParkingMutex<Vec<SafeMemoryPtr>>,

    // Statistics
    total_allocated: AtomicUsize,
    total_freed: AtomicUsize,
    current_usage: AtomicUsize,
    peak_usage: AtomicUsize,

    // Configuration
    enabled: AtomicBool,
    debug_enabled: AtomicBool,
}

/// Memory chunk in a pool
pub struct Chunk {
    pub memory: SafeMemoryPtr,
    pub size: usize,
    pub layout: Layout,
}

impl Drop for Chunk {
    fn drop(&mut self) {
        unsafe {
            dealloc(self.memory.as_ptr(), self.layout);
        }
    }
}

impl MemoryPool {
    /// Create a new memory pool
    pub fn new(
        name: &str,
        block_size: usize,
        initial_capacity: usize,
    ) -> Result<Self, MemoryError> {
        let aligned_size = Self::align_size(block_size, MEMORY_ALIGNMENT);

        let pool = Self {
            name: name.to_string(),
            block_size: aligned_size,
            alignment: MEMORY_ALIGNMENT,
            initial_capacity,
            growth_factor: 1.5,

            chunks: ParkingMutex::new(VecDeque::new()),
            free_blocks: ParkingMutex::new(Vec::new()),

            total_allocated: AtomicUsize::new(0),
            total_freed: AtomicUsize::new(0),
            current_usage: AtomicUsize::new(0),
            peak_usage: AtomicUsize::new(0),

            enabled: AtomicBool::new(true),
            debug_enabled: AtomicBool::new(cfg!(debug_assertions)),
        };

        Ok(pool)
    }

    /// Initialize the pool with initial capacity
    pub fn init(&self) -> Result<(), MemoryError> {
        if self.initial_capacity > 0 {
            self.grow_pool(self.initial_capacity)?;
        }
        Ok(())
    }

    /// Allocate a block from the pool
    pub fn allocate(&self) -> Result<NonNull<u8>, MemoryError> {
        if !self.enabled.load(Ordering::Relaxed) {
            return Err(MemoryError::InvalidPool {
                pool_name: self.name.clone(),
            });
        }

        // Try to get a free block
        if let Some(block) = self.free_blocks.lock().pop() {
            self.record_allocation();
            return Ok(block.as_non_null());
        }

        // Need to grow the pool
        let growth_size = ((self.current_usage.load(Ordering::Relaxed) as f32 * self.growth_factor)
            as usize)
            .max(16);
        self.grow_pool(growth_size)?;

        // Try again after growing
        if let Some(block) = self.free_blocks.lock().pop() {
            self.record_allocation();
            Ok(block.as_non_null())
        } else {
            Err(MemoryError::PoolAllocationFailed {
                pool_name: self.name.clone(),
            })
        }
    }

    /// Free a block back to the pool
    pub fn deallocate(&self, ptr: NonNull<u8>) -> Result<(), MemoryError> {
        // Verify the block belongs to this pool
        if !self.owns_pointer(ptr) {
            return Err(MemoryError::InvalidPool {
                pool_name: self.name.clone(),
            });
        }

        // Add to free list
        self.free_blocks.lock().push(SafeMemoryPtr::new(ptr));
        self.record_free();

        Ok(())
    }

    /// Check if a pointer belongs to this pool
    pub fn owns_pointer(&self, ptr: NonNull<u8>) -> bool {
        let chunks = self.chunks.lock();
        let ptr_addr = ptr.as_ptr() as usize;

        for chunk in chunks.iter() {
            let start = chunk.memory.as_ptr() as usize;
            let end = start + chunk.size;

            if ptr_addr >= start && ptr_addr < end {
                return true;
            }
        }

        false
    }

    /// Grow the pool by allocating more chunks
    fn grow_pool(&self, num_blocks: usize) -> Result<(), MemoryError> {
        let total_size = self.block_size * num_blocks;
        let layout = Layout::from_size_align(total_size, self.alignment)
            .map_err(|_| MemoryError::InvalidSize { size: total_size })?;

        unsafe {
            let memory = alloc(layout);
            if memory.is_null() {
                return Err(MemoryError::OutOfMemory);
            }

            let memory = NonNull::new_unchecked(memory);

            // Create the chunk
            let chunk = Chunk {
                memory: SafeMemoryPtr::new(memory),
                size: total_size,
                layout,
            };

            // Add individual blocks to free list
            let mut free_blocks = self.free_blocks.lock();
            for i in 0..num_blocks {
                let block_ptr = memory.as_ptr().add(i * self.block_size);
                free_blocks.push(SafeMemoryPtr::new(NonNull::new_unchecked(block_ptr)));
            }

            // Add chunk to chunk list
            self.chunks.lock().push_back(chunk);
        }

        Ok(())
    }

    /// Record an allocation
    fn record_allocation(&self) {
        let current = self.current_usage.fetch_add(1, Ordering::Relaxed) + 1;
        self.total_allocated.fetch_add(1, Ordering::Relaxed);

        // Update peak usage
        let peak = self.peak_usage.load(Ordering::Relaxed);
        if current > peak {
            self.peak_usage.store(current, Ordering::Relaxed);
        }
    }

    /// Record a free operation
    fn record_free(&self) {
        self.current_usage.fetch_sub(1, Ordering::Relaxed);
        self.total_freed.fetch_add(1, Ordering::Relaxed);
    }

    /// Align size to boundary
    fn align_size(size: usize, alignment: usize) -> usize {
        (size + alignment - 1) & !(alignment - 1)
    }

    /// Get pool statistics
    pub fn get_stats(&self) -> PoolStats {
        PoolStats {
            name: self.name.clone(),
            block_size: self.block_size,
            total_allocated: self.total_allocated.load(Ordering::Relaxed),
            total_freed: self.total_freed.load(Ordering::Relaxed),
            current_usage: self.current_usage.load(Ordering::Relaxed),
            peak_usage: self.peak_usage.load(Ordering::Relaxed),
            free_blocks: self.free_blocks.lock().len(),
            total_chunks: self.chunks.lock().len(),
            enabled: self.enabled.load(Ordering::Relaxed),
        }
    }

    /// Enable or disable the pool
    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::Relaxed);
    }

    /// Clear all allocations (dangerous - only use in shutdown)
    pub unsafe fn clear(&self) {
        let mut chunks = self.chunks.lock();
        chunks.clear(); // Drop will handle deallocation

        self.free_blocks.lock().clear();
        self.current_usage.store(0, Ordering::Relaxed);
    }
}

/// Pool statistics
#[derive(Debug, Clone)]
pub struct PoolStats {
    pub name: String,
    pub block_size: usize,
    pub total_allocated: usize,
    pub total_freed: usize,
    pub current_usage: usize,
    pub peak_usage: usize,
    pub free_blocks: usize,
    pub total_chunks: usize,
    pub enabled: bool,
}

/// Game memory allocator
pub struct GameMemoryAllocator {
    // Memory pools indexed by size
    pools: ParkingRwLock<HashMap<usize, Arc<MemoryPool>>>,

    // Bump allocator for temporary allocations
    bump_allocator: ParkingMutex<Bump>,

    // Global statistics
    stats: MemoryStats,

    // Configuration
    pool_enabled: AtomicBool,
    debug_enabled: AtomicBool,
    allocation_counter: AtomicU64,
}

impl GameMemoryAllocator {
    /// Create a new game memory allocator
    pub fn new() -> Self {
        Self {
            pools: ParkingRwLock::new(HashMap::new()),
            bump_allocator: ParkingMutex::new(Bump::new()),
            stats: MemoryStats::default(),
            pool_enabled: AtomicBool::new(true),
            debug_enabled: AtomicBool::new(cfg!(debug_assertions)),
            allocation_counter: AtomicU64::new(1),
        }
    }

    /// Initialize the allocator with default pools
    pub fn init(&self) -> Result<(), MemoryError> {
        use crate::common::system::memory_init::{get_default_dma_params, init_memory_pools};

        // Initialize memory pool manager first
        init_memory_pools();

        // Create default DMA pools
        let dma_params = get_default_dma_params();
        for param in dma_params {
            let pool = Arc::new(MemoryPool::new(
                &param.name,
                param.alloc_size,
                param.initial_count,
            )?);
            pool.init()?;
            self.pools.write().insert(param.alloc_size, pool);
        }

        Ok(())
    }

    /// Allocate memory with alignment
    pub fn allocate_aligned(
        &self,
        size: usize,
        alignment: usize,
    ) -> Result<NonNull<u8>, MemoryError> {
        if size == 0 {
            return Err(MemoryError::InvalidSize { size });
        }

        let aligned_size = Self::align_size(size, alignment.max(MEMORY_ALIGNMENT));

        // Try pool allocation first if enabled
        if self.pool_enabled.load(Ordering::Relaxed) {
            if let Some(pool) = self.find_suitable_pool(aligned_size) {
                match pool.allocate() {
                    Ok(ptr) => {
                        self.stats.record_allocation(aligned_size);
                        self.stats.record_pool_allocation();
                        return Ok(ptr);
                    }
                    Err(_) => {
                        // Fall through to heap allocation
                    }
                }
            }
        }

        // Fall back to heap allocation
        self.heap_allocate(aligned_size, alignment)
    }

    /// Allocate memory (default alignment)
    pub fn allocate(&self, size: usize) -> Result<NonNull<u8>, MemoryError> {
        self.allocate_aligned(size, MEMORY_ALIGNMENT)
    }

    /// Allocate zeroed memory
    pub fn allocate_zeroed(&self, size: usize) -> Result<NonNull<u8>, MemoryError> {
        let ptr = self.allocate(size)?;
        unsafe {
            std::ptr::write_bytes(ptr.as_ptr(), 0, size);
        }
        Ok(ptr)
    }

    /// Deallocate memory
    pub fn deallocate(&self, ptr: NonNull<u8>, size: usize) -> Result<(), MemoryError> {
        // Try pool deallocation first
        if self.pool_enabled.load(Ordering::Relaxed) {
            let pools = self.pools.read();
            for pool in pools.values() {
                if pool.owns_pointer(ptr) {
                    pool.deallocate(ptr)?;
                    self.stats.record_free(size);
                    return Ok(());
                }
            }
        }

        // Fall back to heap deallocation
        self.heap_deallocate(ptr, size)
    }

    /// Reallocate memory
    pub fn reallocate(
        &self,
        ptr: NonNull<u8>,
        old_size: usize,
        new_size: usize,
    ) -> Result<NonNull<u8>, MemoryError> {
        if new_size == 0 {
            self.deallocate(ptr, old_size)?;
            return Err(MemoryError::InvalidSize { size: new_size });
        }

        if old_size == new_size {
            return Ok(ptr);
        }

        // Allocate new block
        let new_ptr = self.allocate(new_size)?;

        // Copy existing data
        unsafe {
            let copy_size = old_size.min(new_size);
            std::ptr::copy_nonoverlapping(ptr.as_ptr(), new_ptr.as_ptr(), copy_size);
        }

        // Free old block
        self.deallocate(ptr, old_size)?;

        Ok(new_ptr)
    }

    /// Allocate from bump allocator (temporary allocations)
    pub fn bump_allocate(&self, size: usize, alignment: usize) -> Result<NonNull<u8>, MemoryError> {
        let bump = self.bump_allocator.lock();
        let layout = Layout::from_size_align(size, alignment)
            .map_err(|_| MemoryError::InvalidSize { size })?;

        match bump.try_alloc_layout(layout) {
            Ok(ptr) => {
                self.stats.record_allocation(size);
                Ok(ptr)
            }
            Err(_) => Err(MemoryError::OutOfMemory),
        }
    }

    /// Clear bump allocator
    pub fn clear_bump_allocator(&self) {
        let mut bump = self.bump_allocator.lock();
        bump.reset();
    }

    /// Create or get a memory pool for a specific size
    pub fn create_pool(
        &self,
        name: &str,
        block_size: usize,
        initial_capacity: usize,
    ) -> Result<Arc<MemoryPool>, MemoryError> {
        let pool = Arc::new(MemoryPool::new(name, block_size, initial_capacity)?);
        pool.init()?;

        self.pools.write().insert(block_size, pool.clone());
        Ok(pool)
    }

    /// Get memory statistics
    pub fn get_stats(&self) -> MemorySummary {
        self.stats.get_summary()
    }

    /// Get all pool statistics
    pub fn get_pool_stats(&self) -> Vec<PoolStats> {
        let pools = self.pools.read();
        pools.values().map(|pool| pool.get_stats()).collect()
    }

    /// Enable or disable pool allocation
    pub fn set_pool_enabled(&self, enabled: bool) {
        self.pool_enabled.store(enabled, Ordering::Relaxed);
    }

    /// Enable or disable debug mode
    pub fn set_debug_enabled(&self, enabled: bool) {
        self.debug_enabled.store(enabled, Ordering::Relaxed);
    }

    /// Heap allocation fallback
    fn heap_allocate(&self, size: usize, alignment: usize) -> Result<NonNull<u8>, MemoryError> {
        let layout = Layout::from_size_align(size, alignment)
            .map_err(|_| MemoryError::InvalidSize { size })?;

        unsafe {
            let ptr = alloc(layout);
            if ptr.is_null() {
                return Err(MemoryError::OutOfMemory);
            }

            self.stats.record_allocation(size);
            self.stats.record_heap_allocation();
            Ok(NonNull::new_unchecked(ptr))
        }
    }

    /// Heap deallocation
    fn heap_deallocate(&self, ptr: NonNull<u8>, size: usize) -> Result<(), MemoryError> {
        let layout = Layout::from_size_align(size, MEMORY_ALIGNMENT)
            .map_err(|_| MemoryError::InvalidSize { size })?;

        unsafe {
            dealloc(ptr.as_ptr(), layout);
        }

        self.stats.record_free(size);
        Ok(())
    }

    /// Find a suitable pool for allocation size
    fn find_suitable_pool(&self, size: usize) -> Option<Arc<MemoryPool>> {
        let pools = self.pools.read();

        // Find exact match first
        if let Some(pool) = pools.get(&size) {
            return Some(pool.clone());
        }

        // Find smallest pool that can accommodate the size
        pools
            .iter()
            .filter(|(pool_size, _)| **pool_size >= size)
            .min_by_key(|(pool_size, _)| *pool_size)
            .map(|(_, pool)| pool.clone())
    }

    /// Align size to boundary
    fn align_size(size: usize, alignment: usize) -> usize {
        (size + alignment - 1) & !(alignment - 1)
    }

    /// Get next allocation ID
    fn next_allocation_id(&self) -> u64 {
        self.allocation_counter.fetch_add(1, Ordering::Relaxed)
    }

    /// Shutdown the allocator
    pub unsafe fn shutdown(&self) {
        // Clear all pools
        let mut pools = self.pools.write();
        for pool in pools.values() {
            pool.clear();
        }
        pools.clear();

        // Clear bump allocator
        self.clear_bump_allocator();
    }
}

impl Default for GameMemoryAllocator {
    fn default() -> Self {
        Self::new()
    }
}

// Global allocator instance
lazy_static::lazy_static! {
    pub static ref GAME_MEMORY_ALLOCATOR: Arc<GameMemoryAllocator> = Arc::new(GameMemoryAllocator::new());
}

/// Initialize the game memory system
pub fn init_game_memory() -> Result<(), MemoryError> {
    GAME_MEMORY_ALLOCATOR.init()
}

/// Get the global game memory allocator
pub fn get_game_memory_allocator() -> Arc<GameMemoryAllocator> {
    GAME_MEMORY_ALLOCATOR.clone()
}

/// Allocate game memory
pub fn game_allocate(size: usize) -> Result<NonNull<u8>, MemoryError> {
    GAME_MEMORY_ALLOCATOR.allocate(size)
}

/// Allocate aligned game memory
pub fn game_allocate_aligned(size: usize, alignment: usize) -> Result<NonNull<u8>, MemoryError> {
    GAME_MEMORY_ALLOCATOR.allocate_aligned(size, alignment)
}

/// Allocate zeroed game memory
pub fn game_allocate_zeroed(size: usize) -> Result<NonNull<u8>, MemoryError> {
    GAME_MEMORY_ALLOCATOR.allocate_zeroed(size)
}

/// Deallocate game memory
pub fn game_deallocate(ptr: NonNull<u8>, size: usize) -> Result<(), MemoryError> {
    GAME_MEMORY_ALLOCATOR.deallocate(ptr, size)
}

/// Reallocate game memory
pub fn game_reallocate(
    ptr: NonNull<u8>,
    old_size: usize,
    new_size: usize,
) -> Result<NonNull<u8>, MemoryError> {
    GAME_MEMORY_ALLOCATOR.reallocate(ptr, old_size, new_size)
}

/// Custom allocator for specific types
pub struct GameAllocator<T> {
    _phantom: std::marker::PhantomData<T>,
}

impl<T> GameAllocator<T> {
    pub const fn new() -> Self {
        Self {
            _phantom: std::marker::PhantomData,
        }
    }

    pub fn allocate(&self, count: usize) -> Result<NonNull<T>, MemoryError> {
        let size = std::mem::size_of::<T>() * count;
        let alignment = std::mem::align_of::<T>();
        let ptr = game_allocate_aligned(size, alignment)?;
        Ok(ptr.cast())
    }

    pub fn deallocate(&self, ptr: NonNull<T>, count: usize) -> Result<(), MemoryError> {
        let size = std::mem::size_of::<T>() * count;
        game_deallocate(ptr.cast(), size)
    }
}

/// RAII memory guard
pub struct MemoryGuard {
    ptr: Option<NonNull<u8>>,
    size: usize,
}

impl MemoryGuard {
    /// Create a new memory guard
    pub fn new(size: usize) -> Result<Self, MemoryError> {
        let ptr = game_allocate(size)?;
        Ok(Self {
            ptr: Some(ptr),
            size,
        })
    }

    /// Get the managed pointer
    pub fn as_ptr(&self) -> *mut u8 {
        self.ptr.map(|p| p.as_ptr()).unwrap_or(null_mut())
    }

    /// Get the managed pointer as NonNull
    pub fn as_non_null(&self) -> Option<NonNull<u8>> {
        self.ptr
    }

    /// Release the memory guard (caller takes ownership)
    pub fn release(mut self) -> Option<NonNull<u8>> {
        self.ptr.take()
    }
}

impl Drop for MemoryGuard {
    fn drop(&mut self) {
        if let Some(ptr) = self.ptr.take() {
            let _ = game_deallocate(ptr, self.size);
        }
    }
}

/// Memory debugging utilities
pub mod debug {
    use super::*;

    /// Check for memory leaks
    pub fn check_memory_leaks() -> bool {
        let stats = GAME_MEMORY_ALLOCATOR.get_stats();
        stats.current_allocated > 0
    }

    /// Print memory statistics
    pub fn print_memory_stats() {
        let stats = GAME_MEMORY_ALLOCATOR.get_stats();
        println!("Memory Statistics:");
        println!("  Total Allocated: {} bytes", stats.total_allocated);
        println!("  Total Freed: {} bytes", stats.total_freed);
        println!("  Current Allocated: {} bytes", stats.current_allocated);
        println!("  Peak Allocated: {} bytes", stats.peak_allocated);
        println!("  Allocation Count: {}", stats.allocation_count);
        println!("  Free Count: {}", stats.free_count);
        println!("  Pool Allocations: {}", stats.pool_allocations);
        println!("  Heap Allocations: {}", stats.heap_allocations);
    }

    /// Print pool statistics
    pub fn print_pool_stats() {
        let pool_stats = GAME_MEMORY_ALLOCATOR.get_pool_stats();
        println!("Pool Statistics:");
        for stat in pool_stats {
            println!("  Pool '{}' (block size: {}):", stat.name, stat.block_size);
            println!("    Total Allocated: {}", stat.total_allocated);
            println!("    Current Usage: {}", stat.current_usage);
            println!("    Peak Usage: {}", stat.peak_usage);
            println!("    Free Blocks: {}", stat.free_blocks);
            println!("    Enabled: {}", stat.enabled);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn test_memory_allocation() {
        let allocator = GameMemoryAllocator::new();
        allocator.init().unwrap();

        let ptr = allocator.allocate(1024).unwrap();
        assert!(!ptr.as_ptr().is_null());

        allocator.deallocate(ptr, 1024).unwrap();
    }

    #[test]
    fn test_aligned_allocation() {
        let allocator = GameMemoryAllocator::new();
        allocator.init().unwrap();

        let ptr = allocator.allocate_aligned(1024, 32).unwrap();
        assert_eq!(ptr.as_ptr() as usize % 32, 0);

        allocator.deallocate(ptr, 1024).unwrap();
    }

    #[test]
    fn test_zeroed_allocation() {
        let allocator = GameMemoryAllocator::new();
        allocator.init().unwrap();

        let ptr = allocator.allocate_zeroed(1024).unwrap();
        unsafe {
            let slice = std::slice::from_raw_parts(ptr.as_ptr(), 1024);
            assert!(slice.iter().all(|&b| b == 0));
        }

        allocator.deallocate(ptr, 1024).unwrap();
    }

    #[test]
    fn test_reallocation() {
        let allocator = GameMemoryAllocator::new();
        allocator.init().unwrap();

        let ptr1 = allocator.allocate(512).unwrap();
        let ptr2 = allocator.reallocate(ptr1, 512, 1024).unwrap();

        assert!(!ptr2.as_ptr().is_null());
        allocator.deallocate(ptr2, 1024).unwrap();
    }

    #[test]
    fn test_memory_pool() {
        let pool = MemoryPool::new("TestPool", 64, 10).unwrap();
        pool.init().unwrap();

        let ptr1 = pool.allocate().unwrap();
        let ptr2 = pool.allocate().unwrap();

        assert_ne!(ptr1, ptr2);

        pool.deallocate(ptr1).unwrap();
        pool.deallocate(ptr2).unwrap();

        let stats = pool.get_stats();
        assert_eq!(stats.total_allocated, 2);
        assert_eq!(stats.total_freed, 2);
    }

    #[test]
    fn test_memory_guard() {
        let guard = MemoryGuard::new(1024).unwrap();
        assert!(!guard.as_ptr().is_null());

        let ptr = guard.release().unwrap();
        game_deallocate(ptr, 1024).unwrap();
    }

    #[test]
    fn test_bump_allocator() {
        let allocator = GameMemoryAllocator::new();

        let ptr1 = allocator.bump_allocate(64, 8).unwrap();
        let ptr2 = allocator.bump_allocate(32, 4).unwrap();

        assert_ne!(ptr1, ptr2);

        allocator.clear_bump_allocator();
    }

    #[test]
    fn test_custom_allocator() {
        let allocator = GameAllocator::<u32>::new();

        let ptr = allocator.allocate(10).unwrap();
        allocator.deallocate(ptr, 10).unwrap();
    }

    #[test]
    fn test_multithreaded_allocation() {
        let allocator = Arc::new(GameMemoryAllocator::new());
        allocator.init().unwrap();

        let handles: Vec<_> = (0..4)
            .map(|_| {
                let allocator = allocator.clone();
                thread::spawn(move || {
                    for _ in 0..100 {
                        let ptr = allocator.allocate(1024).unwrap();
                        allocator.deallocate(ptr, 1024).unwrap();
                    }
                })
            })
            .collect();

        for handle in handles {
            handle.join().unwrap();
        }

        let stats = allocator.get_stats();
        assert_eq!(stats.allocation_count, stats.free_count);
    }

    #[test]
    fn test_memory_stats() {
        let allocator = GameMemoryAllocator::new();
        allocator.init().unwrap();

        let ptr1 = allocator.allocate(512).unwrap();
        let ptr2 = allocator.allocate(256).unwrap();

        let stats = allocator.get_stats();
        assert!(stats.current_allocated >= 768);
        assert!(stats.allocation_count >= 2);

        allocator.deallocate(ptr1, 512).unwrap();
        allocator.deallocate(ptr2, 256).unwrap();

        let final_stats = allocator.get_stats();
        assert_eq!(final_stats.allocation_count, final_stats.free_count);
    }
}
