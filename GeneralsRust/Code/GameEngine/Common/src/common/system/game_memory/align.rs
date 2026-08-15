//! Alignment and pool init records from C++ `GameMemory.h`.

/// Alignment for user pointers.
///
/// C++ used `MEM_BOUND_ALIGNMENT = 4` on Win32. Headers embed pointer fields,
/// so 64-bit Rust uses `max(align_of::<usize>(), 8)`. User-visible pool sizes
/// may round up slightly vs C++.
pub const MEM_BOUND_ALIGNMENT: usize = {
    let ptr_align = std::mem::align_of::<usize>();
    if ptr_align > 8 { ptr_align } else { 8 }
};

/// Max sub-pools per DynamicMemoryAllocator.
pub const MAX_DMA_SUBPOOLS: usize = 8;

/// Slop vs `sizeof(T)` (C++ `MEMORY_POOL_OBJECT_ALLOCATION_SLOP`).
pub const MEMORY_POOL_OBJECT_ALLOCATION_SLOP: usize = 16;

#[cfg(debug_assertions)]
pub(crate) mod debug_consts {
    pub const SINGLEBLOCK_MAGIC_COOKIE: u16 = 12345;
    pub const GARBAGE_FILL_VALUE: u32 = 0xDEAD_BEEF;
    pub const WALLCOUNT: usize = 2;
    pub const WALLSIZE: usize = WALLCOUNT * std::mem::size_of::<u32>();
    pub const INIT_FILLER_VALUE: u32 = 0xF00D_CAFE;
}

#[cfg(not(debug_assertions))]
pub(crate) mod debug_consts {
    pub const WALLCOUNT: usize = 0;
    pub const WALLSIZE: usize = 0;
}

/// Initialization record for a pool or DMA sub-pool.
#[derive(Debug, Clone)]
pub struct PoolInitRec {
    pub pool_name: &'static str,
    pub allocation_size: usize,
    pub initial_allocation_count: usize,
    pub overflow_allocation_count: usize,
}

impl PoolInitRec {
    pub const fn new(
        pool_name: &'static str,
        allocation_size: usize,
        initial_allocation_count: usize,
        overflow_allocation_count: usize,
    ) -> Self {
        Self {
            pool_name,
            allocation_size,
            initial_allocation_count,
            overflow_allocation_count,
        }
    }
}

#[inline]
pub(crate) fn round_up_mem_bound(i: usize) -> usize {
    (i + (MEM_BOUND_ALIGNMENT - 1)) & !(MEM_BOUND_ALIGNMENT - 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_up_matches_alignment() {
        assert_eq!(MEM_BOUND_ALIGNMENT, std::mem::align_of::<usize>().max(8));
        assert_eq!(round_up_mem_bound(0), 0);
        assert_eq!(round_up_mem_bound(1), MEM_BOUND_ALIGNMENT);
        assert_eq!(round_up_mem_bound(8), 8);
        assert_eq!(round_up_mem_bound(9), 16);
    }
}
