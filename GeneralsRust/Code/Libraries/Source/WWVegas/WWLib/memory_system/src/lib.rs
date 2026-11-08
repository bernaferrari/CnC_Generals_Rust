//! Owner-scoped memory pools for Command & Conquer Generals Zero Hour.
//!
//! [`with_memory_manager`], [`with_memory_pool`], and
//! [`with_dynamic_memory_allocator`] create independent allocation scopes.
//! Handles cannot outlive their scope, and freed or foreign handles are rejected.
//! Access to initialized bytes is borrowed from the owner; no pointer may be
//! retained across an allocation, free, or reinitialization.

use base_types::Int;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::fmt;
use std::marker::PhantomData;

pub const MAX_DYNAMIC_MEMORY_ALLOCATOR_SUBPOOLS: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MemoryReportFlags {
    PoolInfo = 0x0200,
    FactoryInfo = 0x0400,
    PoolOverflow = 0x0800,
    SimpleLeaks = 0x1000,
    StackTrace = 0x0100,
}

#[derive(Debug, Clone)]
pub struct PoolInitRec {
    pub pool_name: String,
    pub allocation_size: Int,
    pub initial_allocation_count: Int,
    pub overflow_allocation_count: Int,
}

impl PoolInitRec {
    pub fn new(pool_name: &str, allocation_size: Int, initial_allocation_count: Int, overflow_allocation_count: Int) -> Self {
        Self { pool_name: pool_name.to_string(), allocation_size, initial_allocation_count, overflow_allocation_count }
    }
}

// Invariance prevents handles from distinct nested scopes from being mixed.
type Brand<'id> = PhantomData<fn(&'id ()) -> &'id ()>;

/// A fixed-size allocation, valid only in its originating scope and pool.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PoolBlock<'id> {
    owner: usize,
    blob: usize,
    slot: usize,
    generation: u64,
    brand: Brand<'id>,
}

/// A dynamic allocation, valid only in its originating scope and allocator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemoryBlock<'id> {
    owner: usize,
    slot: usize,
    generation: u64,
    brand: Brand<'id>,
}

// Vec<usize> supplies the alignment previously requested from the system
// allocator. bytemuck exposes the initialized words as bytes without casts in
// this crate. Each blob uses a word-aligned stride, including odd block sizes.
struct AlignedBytes {
    words: Vec<usize>,
    len: usize,
}

impl AlignedBytes {
    fn new(len: usize) -> Result<Self, String> {
        let word = std::mem::size_of::<usize>();
        let count = len.checked_add(word - 1).ok_or("Allocation size overflow")? / word;
        let mut words = Vec::new();
        words.try_reserve_exact(count).map_err(|e| format!("Failed to allocate memory: {e}"))?;
        words.resize(count, 0);
        Ok(Self { words, len })
    }
    fn as_bytes(&self) -> &[u8] {
        &bytemuck::cast_slice::<usize, u8>(&self.words)[..self.len]
    }
    fn as_bytes_mut(&mut self) -> &mut [u8] {
        &mut bytemuck::cast_slice_mut::<usize, u8>(&mut self.words)[..self.len]
    }
}

struct MemoryPoolBlob {
    bytes: AlignedBytes,
    stride: usize,
    slots: Vec<Option<u64>>,
    free_slots: Vec<usize>,
}

impl MemoryPoolBlob {
    fn new(block_size: usize, count: usize) -> Result<Self, String> {
        let word = std::mem::size_of::<usize>();
        let stride = block_size.checked_add(word - 1).ok_or("Block size overflow")? / word * word;
        let len = stride.checked_mul(count).ok_or("Blob size overflow")?;
        let bytes = AlignedBytes::new(len)?;
        let mut slots = Vec::new();
        slots.try_reserve_exact(count).map_err(|e| format!("Failed to allocate slots: {e}"))?;
        slots.resize(count, None);
        let mut free_slots = Vec::new();
        free_slots.try_reserve_exact(count).map_err(|e| format!("Failed to allocate free list: {e}"))?;
        free_slots.extend((0..count).rev());
        Ok(Self { bytes, stride, slots, free_slots })
    }
}

/// A fixed-size pool. Obtain one via [`with_memory_pool`] or a factory.
pub struct MemoryPool<'id> {
    owner: usize,
    brand: Brand<'id>,
    pool_name: String,
    allocation_size: usize,
    overflow_allocation_count: usize,
    blobs: Vec<MemoryPoolBlob>,
    first_free_blob: Option<usize>,
    used: usize,
    total: usize,
    peak: usize,
    bytes_allocated: usize,
    bytes_freed: usize,
    next_generation: u64,
}

impl<'id> MemoryPool<'id> {
    fn new(owner: usize) -> Self {
        Self { owner, brand: PhantomData, pool_name: String::new(), allocation_size: 0, overflow_allocation_count: 0, blobs: Vec::new(), first_free_blob: None, used: 0, total: 0, peak: 0, bytes_allocated: 0, bytes_freed: 0, next_generation: 0 }
    }

    /// Initialize or replace this pool, invalidating its previous handles.
    pub fn init(&mut self, name: &str, size: usize, initial: usize, overflow: usize) -> Result<(), String> {
        // C++ rounds pool allocation sizes to its four-byte memory boundary.
        let size = size.checked_add(3).ok_or("Block size overflow")? & !3;
        let blob = MemoryPoolBlob::new(size, initial)?;
        self.bytes_freed = self.bytes_freed.saturating_add(self.used.saturating_mul(self.allocation_size));
        self.pool_name = name.to_string();
        self.allocation_size = size;
        self.overflow_allocation_count = overflow;
        self.blobs = vec![blob];
        self.first_free_blob = Some(0);
        self.used = 0;
        self.total = initial;
        self.peak = 0;
        Ok(())
    }

    pub fn allocate_block(&mut self) -> Result<PoolBlock<'id>, String> {
        let generation = self.next_generation.checked_add(1).ok_or("Allocation generation exhausted")?;
        let current = self.first_free_blob.filter(|&index| !self.blobs[index].free_slots.is_empty());
        let found = current.or_else(|| self.blobs.iter().position(|blob| !blob.free_slots.is_empty()));
        let blob = match found {
            Some(index) => index,
            None => {
                self.first_free_blob = None;
                if self.overflow_allocation_count == 0 { return Err("Pool cannot grow".into()); }
                let total = self.total.checked_add(self.overflow_allocation_count).ok_or("Pool size overflow")?;
                let blob = MemoryPoolBlob::new(self.allocation_size, self.overflow_allocation_count)?;
                self.blobs.try_reserve(1).map_err(|e| format!("Failed to grow pool: {e}"))?;
                self.blobs.push(blob);
                self.total = total;
                self.blobs.len() - 1
            }
        };
        self.first_free_blob = Some(blob);
        let entry = &mut self.blobs[blob];
        let slot = entry.free_slots.pop().expect("selected blob has a free slot");
        entry.slots[slot] = Some(generation);
        let start = slot * entry.stride;
        entry.bytes.as_bytes_mut()[start..start + self.allocation_size].fill(0);
        self.next_generation = generation;
        self.used += 1;
        self.bytes_allocated = self.bytes_allocated.saturating_add(self.allocation_size);
        self.peak = self.peak.max(self.used);
        Ok(PoolBlock { owner: self.owner, blob, slot, generation, brand: PhantomData })
    }

    fn valid(&self, block: PoolBlock<'id>) -> bool {
        block.owner == self.owner && self.blobs.get(block.blob).and_then(|b| b.slots.get(block.slot)).copied().flatten() == Some(block.generation)
    }

    pub fn get_block(&self, block: PoolBlock<'id>) -> Option<&[u8]> {
        if !self.valid(block) { return None; }
        let blob = &self.blobs[block.blob];
        let start = block.slot * blob.stride;
        Some(&blob.bytes.as_bytes()[start..start + self.allocation_size])
    }

    pub fn get_block_mut(&mut self, block: PoolBlock<'id>) -> Option<&mut [u8]> {
        if !self.valid(block) { return None; }
        let blob = &mut self.blobs[block.blob];
        let start = block.slot * blob.stride;
        Some(&mut blob.bytes.as_bytes_mut()[start..start + self.allocation_size])
    }

    pub fn free_block(&mut self, block: PoolBlock<'id>) -> bool {
        if !self.valid(block) { return false; }
        self.blobs[block.blob].slots[block.slot] = None;
        self.blobs[block.blob].free_slots.push(block.slot);
        if self.first_free_blob.is_none() { self.first_free_blob = Some(block.blob); }
        self.used -= 1;
        self.bytes_freed = self.bytes_freed.saturating_add(self.allocation_size);
        true
    }
    pub fn get_free_block_count(&self) -> usize { self.total - self.used }
    pub fn get_used_block_count(&self) -> usize { self.used }
    pub fn get_total_block_count(&self) -> usize { self.total }
    pub fn get_peak_block_count(&self) -> usize { self.peak }
    pub fn get_pool_name(&self) -> &str { &self.pool_name }
    pub fn get_allocation_size(&self) -> usize { self.allocation_size }
}

impl fmt::Debug for MemoryPool<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MemoryPool").field("pool_name", &self.pool_name).field("allocation_size", &self.allocation_size).field("used_blocks", &self.used).field("total_blocks", &self.total).field("peak_used", &self.peak).finish()
    }
}

enum Allocation<'id> {
    Pool { index: usize, block: PoolBlock<'id> },
    Raw(AlignedBytes),
}
struct AllocationEntry<'id> {
    generation: u64,
    requested_size: usize,
    allocation: Option<Allocation<'id>>,
}

/// Variable-sized allocator using the first fitting pool, then raw storage.
pub struct DynamicMemoryAllocator<'id> {
    owner: usize,
    brand: Brand<'id>,
    pools: Vec<MemoryPool<'id>>,
    slots: Vec<AllocationEntry<'id>>,
    free_slots: Vec<usize>,
    next_generation: u64,
    used: usize,
    raw_count: usize,
    bytes_allocated: usize,
    bytes_freed: usize,
}

impl<'id> DynamicMemoryAllocator<'id> {
    fn new(owner: usize) -> Self {
        Self { owner, brand: PhantomData, pools: Vec::new(), slots: Vec::new(), free_slots: Vec::new(), next_generation: 0, used: 0, raw_count: 0, bytes_allocated: 0, bytes_freed: 0 }
    }

    pub fn init(&mut self, configs: &[PoolInitRec]) -> Result<(), String> {
        let defaults;
        let configs = if configs.is_empty() {
            defaults = [16, 32, 64, 128, 256, 512, 1024].map(|size| PoolInitRec::new(&format!("dmaPool_{size}"), size, 64, 64));
            &defaults[..]
        } else { configs };
        if configs.len() > MAX_DYNAMIC_MEMORY_ALLOCATOR_SUBPOOLS { return Err(format!("Too many pools requested: {} (max is {})", configs.len(), MAX_DYNAMIC_MEMORY_ALLOCATOR_SUBPOOLS)); }
        let mut pools = Vec::with_capacity(configs.len());
        let mut previous = None;
        for (index, config) in configs.iter().enumerate() {
            let size = usize::try_from(config.allocation_size).map_err(|_| "Negative allocation size")?;
            let initial = usize::try_from(config.initial_allocation_count).map_err(|_| "Negative initial count")?;
            let overflow = usize::try_from(config.overflow_allocation_count).map_err(|_| "Negative overflow count")?;
            if previous.is_some_and(|last| size <= last) { return Err("Pool sizes must increase".into()); }
            previous = Some(size);
            let mut pool = MemoryPool::new(index);
            pool.init(&config.pool_name, size, initial, overflow)?;
            pools.push(pool);
        }
        for slot in &self.slots {
            if slot.allocation.is_some() {
                self.bytes_freed = self.bytes_freed.saturating_add(slot.requested_size);
            }
        }
        self.pools = pools;
        self.slots.clear();
        self.free_slots.clear();
        self.used = 0;
        self.raw_count = 0;
        Ok(())
    }

    pub fn allocate_bytes(&mut self, size: usize) -> Result<MemoryBlock<'id>, String> {
        let generation = self.next_generation.checked_add(1).ok_or("Allocation generation exhausted")?;
        if self.free_slots.is_empty() {
            self.slots.try_reserve(1).map_err(|e| format!("Failed to allocate slot: {e}"))?;
            // Every live slot may eventually be freed; reserve that space now.
            self.free_slots.try_reserve(self.slots.len() + 1).map_err(|e| format!("Failed to allocate free list: {e}"))?;
            self.free_slots.push(self.slots.len());
            self.slots.push(AllocationEntry { generation: 0, requested_size: 0, allocation: None });
        }
        let slot = *self.free_slots.last().unwrap();
        let allocation = if let Some(index) = self.pools.iter().position(|p| size <= p.allocation_size) {
            Allocation::Pool { index, block: self.pools[index].allocate_block()? }
        } else {
            let bytes = AlignedBytes::new(size)?;
            self.raw_count += 1;
            Allocation::Raw(bytes)
        };
        self.free_slots.pop();
        self.slots[slot] = AllocationEntry { generation, requested_size: size, allocation: Some(allocation) };
        self.next_generation = generation;
        self.used += 1;
        self.bytes_allocated = self.bytes_allocated.saturating_add(size);
        Ok(MemoryBlock { owner: self.owner, slot, generation, brand: PhantomData })
    }

    fn allocation(&self, block: MemoryBlock<'id>) -> Option<&Allocation<'id>> {
        if block.owner != self.owner { return None; }
        let slot = self.slots.get(block.slot)?;
        if slot.generation != block.generation { return None; }
        slot.allocation.as_ref()
    }
    pub fn get_bytes(&self, block: MemoryBlock<'id>) -> Option<&[u8]> {
        match self.allocation(block)? {
            Allocation::Pool { index, block } => self.pools[*index].get_block(*block),
            Allocation::Raw(bytes) => Some(bytes.as_bytes()),
        }
    }
    pub fn get_bytes_mut(&mut self, block: MemoryBlock<'id>) -> Option<&mut [u8]> {
        if block.owner != self.owner { return None; }
        let slot = self.slots.get_mut(block.slot)?;
        if slot.generation != block.generation { return None; }
        match slot.allocation.as_mut()? {
            Allocation::Pool { index, block } => self.pools[*index].get_block_mut(*block),
            Allocation::Raw(bytes) => Some(bytes.as_bytes_mut()),
        }
    }
    pub fn free_bytes(&mut self, block: MemoryBlock<'id>) -> bool {
        if block.owner != self.owner { return false; }
        let Some(slot) = self.slots.get_mut(block.slot) else { return false; };
        if slot.generation != block.generation { return false; }
        let Some(allocation) = slot.allocation.take() else { return false; };
        self.bytes_freed = self.bytes_freed.saturating_add(slot.requested_size);
        match allocation {
            Allocation::Pool { index, block } => { assert!(self.pools[index].free_block(block)); }
            Allocation::Raw(_) => self.raw_count -= 1,
        }
        self.free_slots.push(block.slot);
        self.used -= 1;
        true
    }
    pub fn get_used_block_count(&self) -> usize { self.used }
    pub fn get_memory_stats(&self) -> HashMap<String, usize> {
        let mut stats = HashMap::new();
        stats.insert("raw_blocks_count".into(), self.raw_count);
        stats.insert("total_allocated".into(), self.bytes_allocated);
        stats.insert("total_freed".into(), self.bytes_freed);
        stats.insert("used_blocks_in_dma".into(), self.used);
        stats.insert("num_pools".into(), self.pools.len());
        for (i, pool) in self.pools.iter().enumerate() {
            stats.insert(format!("pool_{i}_used_blocks"), pool.used);
            stats.insert(format!("pool_{i}_total_blocks"), pool.total);
            stats.insert(format!("pool_{i}_peak_blocks"), pool.peak);
        }
        stats
    }
}
impl fmt::Debug for DynamicMemoryAllocator<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DynamicMemoryAllocator").field("num_pools", &self.pools.len()).field("used_blocks", &self.used).field("raw_blocks_count", &self.raw_count).finish()
    }
}

/// Factory whose pools and allocators share one generative owner scope.
pub struct MemoryPoolFactory<'id> {
    pools: Vec<MemoryPool<'id>>,
    dmas: Vec<DynamicMemoryAllocator<'id>>,
    next_owner: usize,
}
impl<'id> MemoryPoolFactory<'id> {
    fn new() -> Self { Self { pools: Vec::new(), dmas: Vec::new(), next_owner: 0 } }
    fn owner(&mut self) -> Result<usize, String> {
        let id = self.next_owner;
        self.next_owner = id.checked_add(1).ok_or("Owner identity exhausted")?;
        Ok(id)
    }
    pub fn create_memory_pool(&mut self, name: &str, size: usize, initial: usize, overflow: usize) -> Result<&mut MemoryPool<'id>, String> {
        let mut pool = MemoryPool::new(self.owner()?);
        pool.init(name, size, initial, overflow)?;
        self.pools.push(pool);
        Ok(self.pools.last_mut().unwrap())
    }
    pub fn create_dynamic_memory_allocator(&mut self, configs: &[PoolInitRec]) -> Result<&mut DynamicMemoryAllocator<'id>, String> {
        let mut dma = DynamicMemoryAllocator::new(self.owner()?);
        dma.init(configs)?;
        self.dmas.push(dma);
        Ok(self.dmas.last_mut().unwrap())
    }
    /// Borrow a factory-owned pool by its creation-order index.
    pub fn memory_pool_mut(&mut self, index: usize) -> Option<&mut MemoryPool<'id>> {
        self.pools.get_mut(index)
    }
    /// Borrow a factory-owned dynamic allocator by its creation-order index.
    pub fn dynamic_memory_allocator_mut(&mut self, index: usize) -> Option<&mut DynamicMemoryAllocator<'id>> {
        self.dmas.get_mut(index)
    }
    /// Report cumulative requested bytes allocated and freed as well as live
    /// and peak block counts. Byte totals saturate at `usize::MAX`.
    pub fn get_memory_report(&self) -> HashMap<String, usize> {
        let mut report = HashMap::new();
        report.insert("total_pools".into(), self.pools.len());
        report.insert("total_dmas".into(), self.dmas.len());
        let allocated = self.pools.iter().map(|p| p.bytes_allocated)
            .chain(self.dmas.iter().map(|d| d.bytes_allocated))
            .fold(0usize, usize::saturating_add);
        let freed = self.pools.iter().map(|p| p.bytes_freed)
            .chain(self.dmas.iter().map(|d| d.bytes_freed))
            .fold(0usize, usize::saturating_add);
        report.insert("total_allocated".into(), allocated);
        report.insert("total_freed".into(), freed);
        let (mut total, mut used, mut peak) = (0, 0, 0);
        for (i, pool) in self.pools.iter().enumerate() {
            report.insert(format!("pool_{i}_used"), pool.used);
            report.insert(format!("pool_{i}_total"), pool.total);
            report.insert(format!("pool_{i}_peak"), pool.peak);
            total += pool.total; used += pool.used; peak += pool.peak;
        }
        report.insert("all_pools_total_blocks".into(), total);
        report.insert("all_pools_used_blocks".into(), used);
        report.insert("all_pools_peak_blocks".into(), peak);
        let mut dma_used = 0;
        for (i, dma) in self.dmas.iter().enumerate() {
            report.insert(format!("dma_{i}_used"), dma.used);
            dma_used += dma.used;
        }
        report.insert("all_dmas_used_blocks".into(), dma_used);
        report
    }
    /// Free all allocations in factory-owned fixed-size pools.
    pub fn reset_all_pools(&mut self) {
        for pool in &mut self.pools {
            pool.bytes_freed = pool.bytes_freed.saturating_add(pool.used.saturating_mul(pool.allocation_size));
            for blob in &mut pool.blobs {
                blob.slots.fill(None);
                blob.free_slots.clear();
                blob.free_slots.extend((0..blob.slots.len()).rev());
            }
            pool.first_free_blob = (!pool.blobs.is_empty()).then_some(0);
            pool.used = 0;
        }
    }
}
impl fmt::Debug for MemoryPoolFactory<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MemoryPoolFactory").field("pools_count", &self.pools.len()).field("dmas_count", &self.dmas.len()).finish()
    }
}

/// Host-owned manager. The enclosing scope ensures allocation identities cannot
/// be confused with another manager, even after it has been dropped.
pub struct MemoryPoolSession<'id> { factory: Mutex<MemoryPoolFactory<'id>> }
impl<'id> MemoryPoolSession<'id> {
    pub fn init(&self, configs: &[PoolInitRec]) -> Result<(), String> {
        self.factory.lock().create_dynamic_memory_allocator(configs)?;
        Ok(())
    }
    pub fn factory(&self) -> &Mutex<MemoryPoolFactory<'id>> { &self.factory }
    pub fn allocate_memory(&self, size: usize) -> Result<MemoryBlock<'id>, String> {
        self.factory.lock().dmas.first_mut().ok_or("Memory system not initialized")?.allocate_bytes(size)
    }
    pub fn free_memory(&self, block: MemoryBlock<'id>) -> bool {
        self.factory.lock().dmas.iter_mut().any(|dma| dma.free_bytes(block))
    }
    /// The callback runs under the factory lock and must not reenter the session.
    pub fn with_memory<R>(&self, block: MemoryBlock<'id>, f: impl FnOnce(&[u8]) -> R) -> Option<R> {
        let factory = self.factory.lock();
        factory.dmas.iter().find_map(|dma| dma.get_bytes(block)).map(f)
    }
    /// The callback runs under the factory lock and must not reenter the session.
    pub fn with_memory_mut<R>(&self, block: MemoryBlock<'id>, f: impl FnOnce(&mut [u8]) -> R) -> Option<R> {
        let mut factory = self.factory.lock();
        factory.dmas.iter_mut().find_map(|dma| dma.get_bytes_mut(block)).map(f)
    }
}

/// Create a manager with an identity that cannot escape this call.
///
/// Allocations cannot outlive their manager:
/// ```compile_fail
/// use memory_system::with_memory_manager;
/// let block = with_memory_manager(&[], |manager| {
///     manager.allocate_memory(16).unwrap()
/// }).unwrap();
/// ```
pub fn with_memory_manager<R>(configs: &[PoolInitRec], f: impl for<'id> FnOnce(&MemoryPoolSession<'id>) -> R) -> Result<R, String> {
    fn run<'id, R>(_: &'id mut (), configs: &[PoolInitRec], f: impl FnOnce(&MemoryPoolSession<'id>) -> R) -> Result<R, String> {
        let session = MemoryPoolSession { factory: Mutex::new(MemoryPoolFactory::new()) };
        session.init(configs)?;
        Ok(f(&session))
    }
    run(&mut (), configs, f)
}

/// Run with an independently branded fixed-size pool.
///
/// Handles from another pool scope cannot be passed to this pool:
/// ```compile_fail
/// use memory_system::with_memory_pool;
/// with_memory_pool(|first| {
///     first.init("first", 4, 1, 1).unwrap();
///     let block = first.allocate_block().unwrap();
///     with_memory_pool(|second| {
///         second.init("second", 4, 1, 1).unwrap();
///         second.free_block(block);
///     });
/// });
/// ```
pub fn with_memory_pool<R>(f: impl for<'id> FnOnce(&mut MemoryPool<'id>) -> R) -> R {
    fn run<'id, R>(_: &'id mut (), f: impl FnOnce(&mut MemoryPool<'id>) -> R) -> R { f(&mut MemoryPool::new(0)) }
    run(&mut (), f)
}

/// Run with an independently branded dynamic allocator.
pub fn with_dynamic_memory_allocator<R>(f: impl for<'id> FnOnce(&mut DynamicMemoryAllocator<'id>) -> R) -> R {
    fn run<'id, R>(_: &'id mut (), f: impl FnOnce(&mut DynamicMemoryAllocator<'id>) -> R) -> R { f(&mut DynamicMemoryAllocator::new(0)) }
    run(&mut (), f)
}
