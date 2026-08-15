//! One blob of fixed-size blocks. Free list is indices, storage is [`AlignedBuf`].

use super::ptr_identity::{self, AlignedBuf, BlockHeader};

pub(crate) struct MemoryPoolBlob {
    storage: AlignedBuf,
    free: Vec<usize>,
    used_blocks: usize,
    total_blocks: usize,
    raw_block_size: usize,
    pub(crate) allocation_size: usize,
}

impl MemoryPoolBlob {
    pub(crate) fn new(allocation_size: usize, count: usize, self_id: *mut u8) -> Self {
        let raw_block_size = BlockHeader::calc_raw_block_size(allocation_size);
        let mut storage = AlignedBuf::with_bytes(raw_block_size * count);
        let mut free = Vec::with_capacity(count);
        for i in 0..count {
            ptr_identity::init_slot(&mut storage, i, raw_block_size, allocation_size, self_id);
            free.push(count - 1 - i);
        }
        Self {
            storage,
            free,
            used_blocks: 0,
            total_blocks: count,
            raw_block_size,
            allocation_size,
        }
    }

    pub(crate) fn has_free_blocks(&self) -> bool {
        !self.free.is_empty()
    }

    pub(crate) fn used_blocks(&self) -> usize {
        self.used_blocks
    }

    pub(crate) fn total_blocks(&self) -> usize {
        self.total_blocks
    }

    pub(crate) fn allocate(&mut self, self_id: *mut u8) -> *mut u8 {
        let index = self.free.pop().expect("no free blocks in blob");
        self.used_blocks += 1;
        ptr_identity::prepare_alloc(
            &mut self.storage,
            index,
            self.raw_block_size,
            self.allocation_size,
            self_id,
        )
    }

    pub(crate) fn free_user(&mut self, user: *mut u8) {
        let index = ptr_identity::slot_index(&self.storage, user, self.raw_block_size)
            .expect("user pointer not in this blob");
        ptr_identity::mark_freed(user);
        self.free.push(index);
        self.used_blocks = self.used_blocks.saturating_sub(1);
    }

    pub(crate) fn contains_user(&self, user: *mut u8) -> bool {
        ptr_identity::slot_index(&self.storage, user, self.raw_block_size)
            .is_some_and(|i| i < self.total_blocks)
    }
}
