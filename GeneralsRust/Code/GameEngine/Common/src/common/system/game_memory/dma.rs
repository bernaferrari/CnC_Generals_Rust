//! Multi-size allocator routing to best-fit sub-pools.

use super::align::{MAX_DMA_SUBPOOLS, PoolInitRec};
use super::pool::MemoryPool;
use super::ptr_identity::{self, AlignedBuf, BlockHeader};
use std::collections::HashMap;

/// Oversized allocation stored as an owned aligned buffer.
struct RawBlock {
    storage: AlignedBuf,
}

/// Multi-size allocator that routes to the best-fit sub-pool.
pub struct DynamicMemoryAllocator {
    pools: Vec<Box<MemoryPool>>,
    used_blocks_in_dma: usize,
    raw_blocks: HashMap<usize, RawBlock>,
}

impl DynamicMemoryAllocator {
    pub(crate) fn new() -> Self {
        Self {
            pools: Vec::new(),
            used_blocks_in_dma: 0,
            raw_blocks: HashMap::new(),
        }
    }

    pub(crate) fn init(&mut self, sub_pools: &[PoolInitRec]) {
        const DEFAULT_DMA: [PoolInitRec; 7] = [
            PoolInitRec::new("dmaPool_16", 16, 64, 64),
            PoolInitRec::new("dmaPool_32", 32, 64, 64),
            PoolInitRec::new("dmaPool_64", 64, 64, 64),
            PoolInitRec::new("dmaPool_128", 128, 64, 64),
            PoolInitRec::new("dmaPool_256", 256, 64, 64),
            PoolInitRec::new("dmaPool_512", 512, 64, 64),
            PoolInitRec::new("dmaPool_1024", 1024, 64, 64),
        ];
        let params = if sub_pools.is_empty() {
            &DEFAULT_DMA[..]
        } else {
            sub_pools
        };
        self.pools.clear();
        for parm in params.iter().take(MAX_DMA_SUBPOOLS) {
            let mut pool = Box::new(MemoryPool::new());
            pool.init(
                parm.pool_name,
                parm.allocation_size,
                parm.initial_allocation_count,
                parm.overflow_allocation_count,
            );
            self.pools.push(pool);
        }
        self.used_blocks_in_dma = 0;
    }

    fn find_pool_index(&self, alloc_size: usize) -> Option<usize> {
        self.pools
            .iter()
            .position(|p| p.get_allocation_size() >= alloc_size)
    }

    pub fn allocate_bytes(&mut self, num_bytes: usize) -> *mut u8 {
        let ptr = self.allocate_bytes_do_not_zero(num_bytes);
        if !ptr.is_null() {
            ptr_identity::zero_user(ptr, num_bytes);
        }
        ptr
    }

    pub fn allocate_bytes_do_not_zero(&mut self, num_bytes: usize) -> *mut u8 {
        if let Some(i) = self.find_pool_index(num_bytes) {
            let user = self.pools[i].allocate_block_do_not_zero();
            self.used_blocks_in_dma += 1;
            return user;
        }
        let raw_size = BlockHeader::calc_raw_block_size(num_bytes);
        let mut storage = AlignedBuf::with_bytes(raw_size);
        let user =
            ptr_identity::prepare_alloc(&mut storage, 0, raw_size, num_bytes, std::ptr::null_mut());
        self.raw_blocks.insert(user as usize, RawBlock { storage });
        self.used_blocks_in_dma += 1;
        user
    }

    pub fn free_bytes(&mut self, user_ptr: *mut u8) {
        if user_ptr.is_null() {
            return;
        }
        if self.raw_blocks.remove(&(user_ptr as usize)).is_some() {
            self.used_blocks_in_dma = self.used_blocks_in_dma.saturating_sub(1);
            return;
        }
        if let Some(i) = self.pools.iter().position(|p| p.contains_user(user_ptr)) {
            self.pools[i].free_block(user_ptr);
            self.used_blocks_in_dma = self.used_blocks_in_dma.saturating_sub(1);
        }
    }

    pub fn get_actual_allocation_size(&self, num_bytes: usize) -> usize {
        self.find_pool_index(num_bytes)
            .map(|i| self.pools[i].get_allocation_size())
            .unwrap_or(num_bytes)
    }

    pub fn reset(&mut self) {
        for pool in &mut self.pools {
            pool.reset();
        }
        self.raw_blocks.clear();
        self.used_blocks_in_dma = 0;
    }
}
