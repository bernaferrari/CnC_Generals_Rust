//! Isolated `unsafe` for C++ GameMemory pointer identity.
//!
//! Save/load and the historic `void*` pool API identify blocks by address.
//! Headers sit immediately before user bytes (`MemoryPoolSingleBlock` in
//! GameMemory.cpp). Free lists, blob lists, and factory tables are **not**
//! in this module — those use `Vec` / `HashMap`.
//!
//! # Safety contract
//! - [`AlignedBuf`] heap is 8-aligned (`Vec<u64>`) and never reallocates
//!   after [`AlignedBuf::with_bytes`].
//! - [`BlockHeader`] is `repr(C, align(8))` so pointer fields are aligned.
//! - [`from_user_data`] only runs on pointers produced by [`user_ptr`].

use super::align::{MEM_BOUND_ALIGNMENT, debug_consts, round_up_mem_bound};
use std::ptr;

/// 8-aligned byte buffer. Capacity is fixed at construction so user pointers stay valid.
pub(crate) struct AlignedBuf {
    words: Vec<u64>,
    bytes: usize,
}

impl AlignedBuf {
    pub(crate) fn with_bytes(bytes: usize) -> Self {
        let bytes = round_up_mem_bound(bytes.max(1));
        let words = bytes / 8;
        Self {
            words: vec![0u64; words],
            bytes,
        }
    }

    pub(crate) fn len(&self) -> usize {
        self.bytes
    }

    pub(crate) fn as_mut_ptr(&mut self) -> *mut u8 {
        self.words.as_mut_ptr() as *mut u8
    }

    pub(crate) fn as_ptr(&self) -> *const u8 {
        self.words.as_ptr() as *const u8
    }
}

/// Header placed before each user region (C++ `MemoryPoolSingleBlock` fields).
#[repr(C, align(8))]
pub(crate) struct BlockHeader {
    pub owning_blob: *mut u8,
    pub next: *mut BlockHeader,
    pub prev: *mut BlockHeader,
    pub logical_size: usize,
    #[cfg(debug_assertions)]
    pub wall_pattern: u32,
    #[cfg(debug_assertions)]
    pub magic_cookie: u16,
    #[cfg(debug_assertions)]
    pub debug_flags: u16,
}

const _: () = {
    assert!(MEM_BOUND_ALIGNMENT >= 8);
    assert!(std::mem::align_of::<BlockHeader>() >= MEM_BOUND_ALIGNMENT);
    assert!(std::mem::size_of::<BlockHeader>() % MEM_BOUND_ALIGNMENT == 0);
};

impl BlockHeader {
    pub(crate) fn calc_raw_block_size(logical_size: usize) -> usize {
        let aligned = round_up_mem_bound(logical_size);
        let mut s = std::mem::size_of::<BlockHeader>() + aligned;
        #[cfg(debug_assertions)]
        {
            s += debug_consts::WALLSIZE * 2;
        }
        round_up_mem_bound(s)
    }

    pub(crate) fn header_to_user_offset() -> usize {
        let offset = std::mem::size_of::<BlockHeader>();
        #[cfg(debug_assertions)]
        {
            return offset + debug_consts::WALLSIZE;
        }
        #[cfg(not(debug_assertions))]
        {
            offset
        }
    }
}

/// Write a fresh header at slot `index` and return the user pointer.
pub(crate) fn init_slot(
    buf: &mut AlignedBuf,
    index: usize,
    raw_block_size: usize,
    logical_size: usize,
    owning_blob: *mut u8,
) -> *mut u8 {
    let base = buf.as_mut_ptr().wrapping_add(index * raw_block_size);
    // SAFETY: [Category 6 — misaligned / Category 4 — uninit]
    // `base` is `Vec<u64>` heap + `index * raw_block_size`. `raw_block_size`
    // and `AlignedBuf` are multiples of 8, matching `BlockHeader` align.
    // The slot is inside `buf.len()` (caller constructs `count * raw_block_size`).
    unsafe {
        let header = base as *mut BlockHeader;
        ptr::write(
            header,
            BlockHeader {
                owning_blob,
                next: ptr::null_mut(),
                prev: ptr::null_mut(),
                logical_size,
                #[cfg(debug_assertions)]
                wall_pattern: 0xBABE_FACE_u32.wrapping_add(index as u32),
                #[cfg(debug_assertions)]
                magic_cookie: debug_consts::SINGLEBLOCK_MAGIC_COOKIE,
                #[cfg(debug_assertions)]
                debug_flags: 0,
            },
        );
        #[cfg(debug_assertions)]
        fill_walls(header);
        #[cfg(debug_assertions)]
        fill_user(header, debug_consts::GARBAGE_FILL_VALUE);
    }
    user_ptr(base)
}

pub(crate) fn prepare_alloc(
    buf: &mut AlignedBuf,
    index: usize,
    raw_block_size: usize,
    logical_size: usize,
    owning_blob: *mut u8,
) -> *mut u8 {
    let user = init_slot(buf, index, raw_block_size, logical_size, owning_blob);
    #[cfg(debug_assertions)]
    {
        let header = from_user_data(user);
        // SAFETY: [Category 4 — uninit]
        // `user` came from `init_slot` on this buffer; header is initialized.
        unsafe {
            fill_user(header, debug_consts::INIT_FILLER_VALUE);
        }
    }
    user
}

pub(crate) fn mark_freed(user: *mut u8) {
    #[cfg(debug_assertions)]
    {
        let header = from_user_data(user);
        // SAFETY: [Category 3 — dangling]
        // `user` is a live pool pointer (caller still owns the blob storage).
        unsafe {
            fill_user(header, debug_consts::GARBAGE_FILL_VALUE);
        }
    }
}

pub(crate) fn user_ptr(header_base: *mut u8) -> *mut u8 {
    header_base.wrapping_add(BlockHeader::header_to_user_offset())
}

pub(crate) fn from_user_data(user: *mut u8) -> *mut BlockHeader {
    user.wrapping_sub(BlockHeader::header_to_user_offset()) as *mut BlockHeader
}

pub(crate) fn slot_index(buf: &AlignedBuf, user: *mut u8, raw_block_size: usize) -> Option<usize> {
    let base = buf.as_ptr() as usize;
    let addr = user as usize;
    if addr < base {
        return None;
    }
    let off = addr - base;
    if off < BlockHeader::header_to_user_offset() {
        return None;
    }
    let slot_off = off - BlockHeader::header_to_user_offset();
    if raw_block_size == 0 || slot_off % raw_block_size != 0 {
        return None;
    }
    Some(slot_off / raw_block_size)
}

pub(crate) fn zero_user(user: *mut u8, bytes: usize) {
    if user.is_null() || bytes == 0 {
        return;
    }
    // SAFETY: [Category 10 — OOB]
    // `user` is a pool/DMA user pointer; `bytes` is the pool allocation size
    // or the DMA request size, both stored with the block.
    unsafe {
        ptr::write_bytes(user, 0, bytes);
    }
}

#[cfg(debug_assertions)]
// SAFETY: caller initialized `header` inside a live pool slot; walls
// stay within that slot's padded bounds.
unsafe fn fill_walls(header: *mut BlockHeader) {
    // SAFETY: [Category 6 — misaligned] caller initialized `header` in this buffer.
    unsafe {
        let base = header as *mut u8;
        let pattern = (*header).wall_pattern;
        let front = base.add(std::mem::size_of::<BlockHeader>()) as *mut u32;
        for i in 0..debug_consts::WALLCOUNT {
            ptr::write(front.add(i), pattern.wrapping_add(i as u32));
        }
        let back = user_ptr(base).add(round_up_mem_bound((*header).logical_size)) as *mut u32;
        for i in 0..debug_consts::WALLCOUNT {
            ptr::write(back.add(i), pattern.wrapping_sub(i as u32));
        }
    }
}

#[cfg(debug_assertions)]
// SAFETY: caller guarantees header/user region was initialized by
// init_slot and logical_size bytes are writable.
unsafe fn fill_user(header: *mut BlockHeader, value: u32) {
    // SAFETY: [Category 4 — uninit] header/user region were written by `init_slot`.
    unsafe {
        let user = user_ptr(header as *mut u8);
        let size = round_up_mem_bound((*header).logical_size);
        let words = size / 4;
        let p = user as *mut u32;
        for i in 0..words {
            ptr::write(p.add(i), value);
        }
        let rem = size % 4;
        let bp = user.add(words * 4);
        for i in 0..rem {
            ptr::write(bp.add(i), value as u8);
        }
    }
}

/// Recover the block-header address sitting before `user`.
#[doc(hidden)]
pub fn debug_block_header_from_user(user: *mut u8) -> *mut u8 {
    from_user_data(user) as *mut u8
}

/// Write (and restore) header pointer fields recovered from `user`.
///
/// # Safety
/// `user` must be a live allocation from this pool/DMA.
#[doc(hidden)]
// SAFETY: contract documented above — `user` must be a live allocation
// from this pool/DMA; all derefs below are alignment-checked first.
pub unsafe fn debug_write_header_pointer_fields(user: *mut u8) -> bool {
    if user.is_null() {
        return false;
    }
    let block = from_user_data(user);
    let ptr_align = std::mem::align_of::<*mut u8>();
    if (block as usize) % MEM_BOUND_ALIGNMENT != 0 {
        return false;
    }
    if (block as usize) % std::mem::align_of::<BlockHeader>() != 0 {
        return false;
    }
    // SAFETY: [Category 6 — misaligned]
    // Alignment checked above. `user` is a live allocation (function contract).
    unsafe {
        let next_field = ptr::addr_of_mut!((*block).next);
        let prev_field = ptr::addr_of_mut!((*block).prev);
        let blob_field = ptr::addr_of_mut!((*block).owning_blob);
        if (next_field as usize) % ptr_align != 0
            || (prev_field as usize) % ptr_align != 0
            || (blob_field as usize) % ptr_align != 0
        {
            return false;
        }
        let saved_next = ptr::read(next_field);
        let saved_prev = ptr::read(prev_field);
        let saved_blob = ptr::read(blob_field);
        ptr::write(next_field, block);
        ptr::write(prev_field, block);
        ptr::write(blob_field, saved_blob);
        let ok = ptr::read(next_field) == block && ptr::read(prev_field) == block;
        ptr::write(next_field, saved_next);
        ptr::write(prev_field, saved_prev);
        ptr::write(blob_field, saved_blob);
        ok
    }
}

#[cfg(all(test, debug_assertions))]
// SAFETY: caller passes a live user pointer whose header magic was
// verified before use; reads stay within the slot's wall region.
pub(crate) unsafe fn header_walls_ok(user: *mut u8) -> bool {
    let header = from_user_data(user);
    if (*header).magic_cookie != debug_consts::SINGLEBLOCK_MAGIC_COOKIE {
        return false;
    }
    let base = header as *const u8;
    let front = base.add(std::mem::size_of::<BlockHeader>()) as *const u32;
    for i in 0..debug_consts::WALLCOUNT {
        if ptr::read(front.add(i)) != (*header).wall_pattern.wrapping_add(i as u32) {
            return false;
        }
    }
    let back =
        user_ptr(header as *mut u8).add(round_up_mem_bound((*header).logical_size)) as *const u32;
    for i in 0..debug_consts::WALLCOUNT {
        if ptr::read(back.add(i)) != (*header).wall_pattern.wrapping_sub(i as u32) {
            return false;
        }
    }
    true
}
