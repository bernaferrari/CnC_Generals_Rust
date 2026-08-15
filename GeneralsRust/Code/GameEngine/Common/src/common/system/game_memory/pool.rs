//! Fixed-size [`MemoryPool`] backed by a `Vec` of blobs.

use super::align::{MEM_BOUND_ALIGNMENT, round_up_mem_bound};
use super::blob::MemoryPoolBlob;
use super::ptr_identity;

/// A pool of fixed-size blocks, backed by one or more blobs.
pub struct MemoryPool {
    pool_name: String,
    allocation_size: usize,
    initial_allocation_count: usize,
    overflow_allocation_count: usize,
    used_blocks_in_pool: usize,
    total_blocks_in_pool: usize,
    peak_used_blocks_in_pool: usize,
    blobs: Vec<Box<MemoryPoolBlob>>,
    first_free_blob: Option<usize>,
}

impl MemoryPool {
    pub(crate) fn new() -> Self {
        Self {
            pool_name: String::new(),
            allocation_size: 0,
            initial_allocation_count: 0,
            overflow_allocation_count: 0,
            used_blocks_in_pool: 0,
            total_blocks_in_pool: 0,
            peak_used_blocks_in_pool: 0,
            blobs: Vec::new(),
            first_free_blob: None,
        }
    }

    pub(crate) fn init(
        &mut self,
        pool_name: &str,
        allocation_size: usize,
        initial_allocation_count: usize,
        overflow_allocation_count: usize,
    ) {
        self.pool_name = pool_name.to_string();
        self.allocation_size = round_up_mem_bound(allocation_size);
        self.initial_allocation_count = initial_allocation_count;
        self.overflow_allocation_count = overflow_allocation_count;
        self.used_blocks_in_pool = 0;
        self.total_blocks_in_pool = 0;
        self.peak_used_blocks_in_pool = 0;
        self.create_blob(initial_allocation_count);
    }

    fn create_blob(&mut self, allocation_count: usize) {
        assert!(
            allocation_count > 0 && allocation_count % MEM_BOUND_ALIGNMENT == 0,
            "bad allocationCount ({allocation_count})"
        );
        // owning_blob is ABI decoration only; free-lists use Vec indices.
        let blob = Box::new(MemoryPoolBlob::new(
            self.allocation_size,
            allocation_count,
            std::ptr::null_mut(),
        ));
        self.blobs.push(blob);
        self.first_free_blob = Some(self.blobs.len() - 1);
        self.total_blocks_in_pool += allocation_count;
    }

    fn refresh_free_blob(&mut self) {
        if let Some(i) = self.first_free_blob {
            if self.blobs.get(i).is_some_and(|b| b.has_free_blocks()) {
                return;
            }
        }
        self.first_free_blob = self.blobs.iter().position(|b| b.has_free_blocks());
    }

    pub fn allocate_block(&mut self) -> *mut u8 {
        let ptr = self.allocate_block_do_not_zero();
        if !ptr.is_null() {
            ptr_identity::zero_user(ptr, self.allocation_size);
        }
        ptr
    }

    pub fn allocate_block_do_not_zero(&mut self) -> *mut u8 {
        self.refresh_free_blob();
        if self.first_free_blob.is_none() {
            if self.overflow_allocation_count == 0 {
                panic!("Pool '{}' is full and cannot grow", self.pool_name);
            }
            self.create_blob(self.overflow_allocation_count);
        }
        let idx = self.first_free_blob.expect("blob after grow");
        let blob = &mut self.blobs[idx];
        let id = blob.as_mut() as *mut MemoryPoolBlob as *mut u8;
        let user = blob.allocate(id);
        self.used_blocks_in_pool += 1;
        if self.used_blocks_in_pool > self.peak_used_blocks_in_pool {
            self.peak_used_blocks_in_pool = self.used_blocks_in_pool;
        }
        user
    }

    pub fn free_block(&mut self, user_ptr: *mut u8) {
        if user_ptr.is_null() {
            return;
        }
        let idx = self
            .blobs
            .iter()
            .position(|b| b.contains_user(user_ptr))
            .expect("block does not belong to this pool");
        self.blobs[idx].free_user(user_ptr);
        self.first_free_blob = Some(idx);
        self.used_blocks_in_pool = self.used_blocks_in_pool.saturating_sub(1);
    }

    pub fn get_pool_name(&self) -> &str {
        &self.pool_name
    }
    pub fn get_allocation_size(&self) -> usize {
        self.allocation_size
    }
    pub fn get_used_block_count(&self) -> usize {
        self.used_blocks_in_pool
    }
    pub fn get_total_block_count(&self) -> usize {
        self.total_blocks_in_pool
    }
    pub fn get_free_block_count(&self) -> usize {
        self.total_blocks_in_pool
            .saturating_sub(self.used_blocks_in_pool)
    }
    pub fn get_peak_block_count(&self) -> usize {
        self.peak_used_blocks_in_pool
    }
    pub fn get_initial_block_count(&self) -> usize {
        self.initial_allocation_count
    }

    pub fn release_empties(&mut self) -> usize {
        let mut released = 0usize;
        let size = self.allocation_size;
        self.blobs.retain(|b| {
            if b.used_blocks() == 0 {
                released += b.total_blocks() * size;
                false
            } else {
                true
            }
        });
        self.first_free_blob = self.blobs.iter().position(|b| b.has_free_blocks());
        released
    }

    pub fn reset(&mut self) {
        self.blobs.clear();
        self.used_blocks_in_pool = 0;
        self.total_blocks_in_pool = 0;
        self.first_free_blob = None;
        self.create_blob(self.initial_allocation_count);
    }

    pub(crate) fn contains_user(&self, user: *mut u8) -> bool {
        self.blobs.iter().any(|b| b.contains_user(user))
    }
}
