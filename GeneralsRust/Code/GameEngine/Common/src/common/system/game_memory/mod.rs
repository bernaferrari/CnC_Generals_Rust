//! GameMemory — C++ pool allocator with Vec-owned tables.
//!
//! Blob/pool/DMA linkage is `Vec`/`HashMap`. The only remaining `unsafe` lives
//! in [`ptr_identity`]: C++ save/ABI still identifies user blocks by address.

mod align;
mod blob;
mod dma;
mod pool;
mod ptr_identity;

pub use align::{
    MAX_DMA_SUBPOOLS, MEM_BOUND_ALIGNMENT, MEMORY_POOL_OBJECT_ALLOCATION_SLOP, PoolInitRec,
};
pub use dma::DynamicMemoryAllocator;
pub use pool::MemoryPool;
pub use ptr_identity::{debug_block_header_from_user, debug_write_header_pointer_fields};

use std::collections::HashMap;
use std::sync::Mutex;

/// Central manager for all MemoryPools and DynamicMemoryAllocators.
pub struct MemoryPoolFactory {
    pools: HashMap<String, Box<MemoryPool>>,
    dmas: Vec<Box<DynamicMemoryAllocator>>,
}

impl MemoryPoolFactory {
    pub fn new() -> Self {
        Self {
            pools: HashMap::new(),
            dmas: Vec::new(),
        }
    }

    pub fn init(&mut self) {}

    pub fn create_memory_pool(
        &mut self,
        pool_name: &str,
        allocation_size: usize,
        initial_allocation_count: usize,
        overflow_allocation_count: usize,
    ) -> *mut MemoryPool {
        if let Some(existing) = self.pools.get_mut(pool_name) {
            assert_eq!(
                existing.get_allocation_size(),
                align::round_up_mem_bound(allocation_size),
                "pool size mismatch for '{pool_name}'"
            );
            return existing.as_mut();
        }
        assert!(
            initial_allocation_count > 0,
            "illegal pool size: initial={initial_allocation_count}"
        );
        let mut pool = Box::new(MemoryPool::new());
        pool.init(
            pool_name,
            allocation_size,
            initial_allocation_count,
            overflow_allocation_count,
        );
        let ptr = pool.as_mut() as *mut MemoryPool;
        self.pools.insert(pool_name.to_string(), pool);
        ptr
    }

    pub fn find_memory_pool(&self, pool_name: &str) -> Option<*mut MemoryPool> {
        self.pools
            .get(pool_name)
            .map(|p| p.as_ref() as *const MemoryPool as *mut MemoryPool)
    }

    pub fn destroy_memory_pool(&mut self, pool: *mut MemoryPool) {
        if pool.is_null() {
            return;
        }
        self.pools
            .retain(|_, boxed| !std::ptr::eq(boxed.as_ref(), pool));
    }

    pub fn create_dynamic_memory_allocator(
        &mut self,
        sub_pools: &[PoolInitRec],
    ) -> *mut DynamicMemoryAllocator {
        let mut dma = Box::new(DynamicMemoryAllocator::new());
        dma.init(sub_pools);
        let ptr = dma.as_mut() as *mut DynamicMemoryAllocator;
        self.dmas.push(dma);
        ptr
    }

    pub fn destroy_dynamic_memory_allocator(&mut self, dma: *mut DynamicMemoryAllocator) {
        if dma.is_null() {
            return;
        }
        self.dmas.retain(|boxed| !std::ptr::eq(boxed.as_ref(), dma));
    }

    pub fn reset(&mut self) {
        for pool in self.pools.values_mut() {
            pool.reset();
        }
        for dma in &mut self.dmas {
            dma.reset();
        }
    }

    fn primary_dma_mut(&mut self) -> &mut DynamicMemoryAllocator {
        self.dmas.first_mut().expect("DMA not initialized").as_mut()
    }
}

impl Default for MemoryPoolFactory {
    fn default() -> Self {
        Self::new()
    }
}

lazy_static::lazy_static! {
    static ref THE_FACTORY: Mutex<MemoryPoolFactory> = Mutex::new(MemoryPoolFactory::new());
}

pub fn init_memory_manager() {
    let mut factory = THE_FACTORY.lock().unwrap();
    factory.dmas.clear();
    factory.create_dynamic_memory_allocator(&[]);
}

pub fn shutdown_memory_manager() {
    let mut factory = THE_FACTORY.lock().unwrap();
    factory.dmas.clear();
    factory.pools.clear();
}

pub fn get_memory_pool_factory() -> &'static Mutex<MemoryPoolFactory> {
    &THE_FACTORY
}

pub fn dma_allocate(num_bytes: usize) -> *mut u8 {
    THE_FACTORY
        .lock()
        .unwrap()
        .primary_dma_mut()
        .allocate_bytes(num_bytes)
}

pub fn dma_allocate_do_not_zero(num_bytes: usize) -> *mut u8 {
    THE_FACTORY
        .lock()
        .unwrap()
        .primary_dma_mut()
        .allocate_bytes_do_not_zero(num_bytes)
}

pub fn dma_free(ptr: *mut u8) {
    if ptr.is_null() {
        return;
    }
    THE_FACTORY
        .lock()
        .unwrap()
        .primary_dma_mut()
        .free_bytes(ptr);
}

pub fn create_pool(
    pool_name: &str,
    allocation_size: usize,
    initial_count: usize,
    overflow_count: usize,
) -> *mut MemoryPool {
    THE_FACTORY.lock().unwrap().create_memory_pool(
        pool_name,
        allocation_size,
        initial_count,
        overflow_count,
    )
}

pub fn find_pool(pool_name: &str) -> Option<*mut MemoryPool> {
    THE_FACTORY.lock().unwrap().find_memory_pool(pool_name)
}
