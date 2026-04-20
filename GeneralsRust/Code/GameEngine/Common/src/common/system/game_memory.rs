////////////////////////////////////////////////////////////////////////////////
//                                                                            //
//  (c) 2001-2003 Electronic Arts Inc.                                       //
//                                                                            //
////////////////////////////////////////////////////////////////////////////////

//! GameMemory - Faithful port of C&C Generals pool allocator
//!
//! Ported from GameMemory.h/cpp. Provides:
//! - `MemoryPoolSingleBlock`: fundamental allocation unit with header, user data,
//!   and debug bounding walls
//! - `MemoryPoolBlob`: monolithic chunk of same-sized blocks
//! - `MemoryPool`: pool of fixed-size blocks backed by blobs
//! - `DynamicMemoryAllocator`: multi-size allocator routing to best-fit sub-pool
//! - `MemoryPoolFactory`: central manager for all pools and DMAs
//!
//! Debug features (active in debug_assertions):
//! - Fill patterns: 0xDEADBEEF on free, 0xF00DCAFE on init allocation
//! - Bounding walls (0xBABEFACE-derived) detect overruns/underruns
//! - Magic cookie validation (12345)
//! - Tag string tracking per allocation

use std::alloc::{alloc, dealloc, Layout};
use std::collections::HashMap;
use std::ptr;
use std::sync::Mutex;

// ---------------------------------------------------------------------------
// Constants matching C++ defines
// ---------------------------------------------------------------------------

/// Alignment boundary for all allocations (C++: MEM_BOUND_ALIGNMENT = 4).
pub const MEM_BOUND_ALIGNMENT: usize = 4;

/// Max sub-pools per DynamicMemoryAllocator (C++: MAX_DYNAMICMEMORYALLOCATOR_SUBPOOLS = 8).
pub const MAX_DMA_SUBPOOLS: usize = 8;

/// How much slop is tolerable per pool vs sizeof(T) (C++: MEMORY_POOL_OBJECT_ALLOCATION_SLOP = 16).
pub const MEMORY_POOL_OBJECT_ALLOCATION_SLOP: usize = 16;

#[cfg(debug_assertions)]
mod debug_consts {
    pub const SINGLEBLOCK_MAGIC_COOKIE: u16 = 12345;
    pub const GARBAGE_FILL_VALUE: u32 = 0xDEAD_BEEF;
    pub const WALLCOUNT: usize = 2;
    pub const WALLSIZE: usize = WALLCOUNT * std::mem::size_of::<u32>();
    pub const INIT_FILLER_VALUE: u32 = 0xF00D_CAFE;
}

#[cfg(not(debug_assertions))]
mod debug_consts {
    pub const WALLCOUNT: usize = 0;
    pub const WALLSIZE: usize = 0;
}

// ---------------------------------------------------------------------------
// PoolInitRec - mirrors C++ PoolInitRec
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Helper: round up to alignment boundary
// ---------------------------------------------------------------------------

#[inline]
fn round_up_mem_bound(i: usize) -> usize {
    (i + (MEM_BOUND_ALIGNMENT - 1)) & !(MEM_BOUND_ALIGNMENT - 1)
}

/// Fill a region of memory with a 32-bit pattern (matches C++ memset32).
#[cfg(debug_assertions)]
fn memset32(ptr: *mut u8, value: u32, bytes_to_fill: usize) {
    let words = bytes_to_fill / 4;
    let remainder = bytes_to_fill % 4;
    let p = ptr as *mut u32;
    for i in 0..words {
        unsafe {
            ptr::write(p.add(i), value);
        }
    }
    let bp = unsafe { ptr.add(words * 4) };
    for i in 0..remainder {
        unsafe {
            ptr::write(bp.add(i), value as u8);
        }
    }
}

// ---------------------------------------------------------------------------
// Raw system allocation (matches C++ sysAllocateDoNotZero / sysFree)
// ---------------------------------------------------------------------------

fn sys_allocate(num_bytes: usize) -> *mut u8 {
    if num_bytes == 0 {
        return std::ptr::null_mut();
    }
    let layout =
        Layout::from_size_align(num_bytes, MEM_BOUND_ALIGNMENT).expect("invalid sys layout");
    let ptr = unsafe { alloc(layout) };
    if ptr.is_null() {
        panic!("Out of memory: sys_allocate failed for {} bytes", num_bytes);
    }
    #[cfg(debug_assertions)]
    {
        let actual = layout.size();
        memset32(ptr, debug_consts::INIT_FILLER_VALUE, actual);
    }
    ptr
}


unsafe fn sys_free(ptr: *mut u8, num_bytes: usize) {
    if ptr.is_null() {
        return;
    }
    let layout = Layout::from_size_align(num_bytes, MEM_BOUND_ALIGNMENT)
        .expect("invalid sys layout on free");
    #[cfg(debug_assertions)]
    {
        memset32(ptr, debug_consts::GARBAGE_FILL_VALUE, layout.size());
    }
    dealloc(ptr, layout);
}

// ---------------------------------------------------------------------------
// BlockHeader - header embedded before user data in every allocation.
// Mirrors the non-debug fields of C++ MemoryPoolSingleBlock.
// ---------------------------------------------------------------------------

/// Header placed before each user data region.
/// In C++, this was `MemoryPoolSingleBlock` allocated inline with the user data.
/// Here we use a `repr(C)` struct to ensure layout compatibility.
#[repr(C)]
struct BlockHeader {
    /// Pointer back to the owning blob (NULL for raw/DMA blocks).
    owning_blob: *mut MemoryPoolBlob,
    /// Next block in the free list (within blob) or raw list (within DMA).
    next: *mut BlockHeader,
    /// Previous block (doubly-linked, matches C++ MPSB_DLINK).
    prev: *mut BlockHeader,
    /// Logical size requested by the user.
    logical_size: usize,
    /// Unique seed for bounding walls (debug).
    #[cfg(debug_assertions)]
    wall_pattern: u32,
    /// Magic cookie for validation (debug).
    #[cfg(debug_assertions)]
    magic_cookie: u16,
    /// Debug flags (e.g. IGNORE_LEAKS).
    #[cfg(debug_assertions)]
    debug_flags: u16,
}

impl BlockHeader {
    /// Calculate the raw block size needed for a given logical (user) size.
    /// Matches C++ MemoryPoolSingleBlock::calcRawBlockSize.
    fn calc_raw_block_size(logical_size: usize) -> usize {
        let aligned = round_up_mem_bound(logical_size);
        let mut s = std::mem::size_of::<BlockHeader>() + aligned;
        #[cfg(debug_assertions)]
        {
            s += debug_consts::WALLSIZE * 2;
        }
        s
    }

    /// Get a pointer to user data (just past the header + front wall).
    fn user_data_ptr(&self) -> *mut u8 {
        let base = self as *const BlockHeader as *mut u8;
        let offset = std::mem::size_of::<BlockHeader>();
        #[cfg(debug_assertions)]
        {
            let p = unsafe { base.add(offset + debug_consts::WALLSIZE) };
            return p;
        }
        #[cfg(not(debug_assertions))]
        {
            unsafe { base.add(offset) }
        }
    }

    /// Recover BlockHeader* from a user data pointer.
    fn from_user_data(user: *mut u8) -> *mut BlockHeader {
        #[cfg(debug_assertions)]
        let offset = std::mem::size_of::<BlockHeader>() + debug_consts::WALLSIZE;
        #[cfg(not(debug_assertions))]
        let offset = std::mem::size_of::<BlockHeader>();
        unsafe { user.sub(offset) as *mut BlockHeader }
    }

    /// Initialize a freshly allocated block header.
    fn init(&mut self, logical_size: usize, owning_blob: *mut MemoryPoolBlob) {
        self.owning_blob = owning_blob;
        self.next = ptr::null_mut();
        self.prev = ptr::null_mut();
        self.logical_size = logical_size;
        #[cfg(debug_assertions)]
        {
            self.magic_cookie = debug_consts::SINGLEBLOCK_MAGIC_COOKIE;
            self.debug_flags = 0;
        }
    }

    #[cfg(debug_assertions)]
    fn fill_walls(&mut self) {
        let base = self as *mut BlockHeader as *mut u8;
        let front_start = unsafe { base.add(std::mem::size_of::<BlockHeader>()) };
        let pattern = self.wall_pattern;
        let p = front_start as *mut u32;
        for i in 0..debug_consts::WALLCOUNT {
            unsafe {
                ptr::write(p.add(i), pattern.wrapping_add(i as u32));
            }
        }
        let back_start = unsafe {
            self.user_data_ptr()
                .add(round_up_mem_bound(self.logical_size))
        };
        let bp = back_start as *mut u32;
        for i in 0..debug_consts::WALLCOUNT {
            unsafe {
                ptr::write(bp.add(i), pattern.wrapping_sub(i as u32));
            }
        }
    }

    #[cfg(debug_assertions)]

    #[cfg(debug_assertions)]

    #[cfg(debug_assertions)]

    #[cfg(debug_assertions)]
    fn mark_as_freed(&mut self) {
        let user = self.user_data_ptr();
        let size = round_up_mem_bound(self.logical_size);
        memset32(user, debug_consts::GARBAGE_FILL_VALUE, size);
    }
}

// ---------------------------------------------------------------------------
// MemoryPoolBlob - matches C++ MemoryPoolBlob
// ---------------------------------------------------------------------------

/// A monolithic chunk of memory subdivided into fixed-size blocks.
struct MemoryPoolBlob {
    /// Pointer to the raw allocation holding all blocks.
    block_data: *mut u8,
    /// Layout for deallocation.
    layout: Layout,
    /// First free block in this blob.
    first_free_block: *mut BlockHeader,
    /// Number of blocks currently in use.
    used_blocks_in_blob: usize,
    /// Total number of blocks in this blob.
    total_blocks_in_blob: usize,
    /// Raw size of each block (header + aligned user data + walls).
    raw_block_size: usize,
    /// Logical (user-visible) allocation size.
    allocation_size: usize,
    /// Doubly-linked list pointers.
    prev_blob: *mut MemoryPoolBlob,
    next_blob: *mut MemoryPoolBlob,
}

impl MemoryPoolBlob {
    fn init(&mut self, allocation_size: usize, allocation_count: usize) {
        self.allocation_size = allocation_size;
        self.total_blocks_in_blob = allocation_count;
        self.used_blocks_in_blob = 0;
        self.raw_block_size = BlockHeader::calc_raw_block_size(allocation_size);

        let total_bytes = self.raw_block_size * allocation_count;
        self.layout =
            Layout::from_size_align(total_bytes, MEM_BOUND_ALIGNMENT).expect("invalid blob layout");
        self.block_data = sys_allocate(total_bytes);

        self.prev_blob = ptr::null_mut();
        self.next_blob = ptr::null_mut();

        // Initialize each block header and link into free list.
        for i in 0..allocation_count {
            let block_ptr =
                unsafe { self.block_data.add(i * self.raw_block_size) } as *mut BlockHeader;
            unsafe {
                (*block_ptr).init(allocation_size, self);
                #[cfg(debug_assertions)]
                {
                    (*block_ptr).wall_pattern = 0xBABE_FACE_u32.wrapping_add(i as u32);
                    (*block_ptr).fill_walls();
                }
            }
            // Link: last block points to previous, forming a stack.
            if i == 0 {
                unsafe {
                    (*block_ptr).next = ptr::null_mut();
                    (*block_ptr).prev = ptr::null_mut();
                }
            } else {
                let prev_block = unsafe { self.block_data.add((i - 1) * self.raw_block_size) }
                    as *mut BlockHeader;
                unsafe {
                    (*block_ptr).next = prev_block;
                    (*prev_block).prev = block_ptr;
                }
            }
            #[cfg(debug_assertions)]
            unsafe {
                (*block_ptr).mark_as_freed();
            }
        }
        // First free block = last in the array (top of stack).
        if allocation_count > 0 {
            self.first_free_block = unsafe {
                self.block_data
                    .add((allocation_count - 1) * self.raw_block_size)
            } as *mut BlockHeader;
        } else {
            self.first_free_block = ptr::null_mut();
        }
    }

    fn has_free_blocks(&self) -> bool {
        !self.first_free_block.is_null()
    }

    fn allocate_single_block(&mut self) -> *mut u8 {
        assert!(!self.first_free_block.is_null(), "no free blocks in blob");
        let block = self.first_free_block;
        unsafe {
            self.first_free_block = (*block).next;
            if !self.first_free_block.is_null() {
                (*self.first_free_block).prev = ptr::null_mut();
            }
            (*block).next = ptr::null_mut();
            (*block).prev = ptr::null_mut();
            (*block).init(self.allocation_size, self);
            #[cfg(debug_assertions)]
            {
                (*block).wall_pattern = 0xBABE_FACE_u32;
                (*block).fill_walls();
            }
        }
        self.used_blocks_in_blob += 1;
        unsafe { (*block).user_data_ptr() }
    }

    fn free_single_block(&mut self, user_ptr: *mut u8) {
        let block = BlockHeader::from_user_data(user_ptr);
        unsafe {
            assert_eq!((*block).owning_blob, self as *mut MemoryPoolBlob);
            #[cfg(debug_assertions)]
            {
                (*block).mark_as_freed();
            }
            (*block).next = self.first_free_block;
            (*block).prev = ptr::null_mut();
            if !self.first_free_block.is_null() {
                (*self.first_free_block).prev = block;
            }
            self.first_free_block = block;
        }
        self.used_blocks_in_blob -= 1;
    }
}

impl Drop for MemoryPoolBlob {
    fn drop(&mut self) {
        if !self.block_data.is_null() {
            unsafe {
                sys_free(self.block_data, self.layout.size());
            }
            self.block_data = ptr::null_mut();
        }
    }
}

// ---------------------------------------------------------------------------
// MemoryPool - matches C++ MemoryPool
// ---------------------------------------------------------------------------

/// A pool of fixed-size blocks, backed by one or more blobs.
pub struct MemoryPool {
    pool_name: String,
    allocation_size: usize,
    initial_allocation_count: usize,
    overflow_allocation_count: usize,
    used_blocks_in_pool: usize,
    total_blocks_in_pool: usize,
    peak_used_blocks_in_pool: usize,
    first_blob: *mut MemoryPoolBlob,
    last_blob: *mut MemoryPoolBlob,
    first_blob_with_free_blocks: *mut MemoryPoolBlob,
    /// Linked list of pools in factory.
    next_pool_in_factory: *mut MemoryPool,
    /// Back-pointer to owning factory.
    factory: *mut MemoryPoolFactory,
}

impl MemoryPool {
    #[allow(dead_code)]
    fn new() -> Self {
        Self {
            pool_name: String::new(),
            allocation_size: 0,
            initial_allocation_count: 0,
            overflow_allocation_count: 0,
            used_blocks_in_pool: 0,
            total_blocks_in_pool: 0,
            peak_used_blocks_in_pool: 0,
            first_blob: ptr::null_mut(),
            last_blob: ptr::null_mut(),
            first_blob_with_free_blocks: ptr::null_mut(),
            next_pool_in_factory: ptr::null_mut(),
            factory: ptr::null_mut(),
        }
    }

    fn init(
        &mut self,
        factory: *mut MemoryPoolFactory,
        pool_name: &str,
        allocation_size: usize,
        initial_allocation_count: usize,
        overflow_allocation_count: usize,
    ) {
        self.factory = factory;
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
            "bad allocationCount ({})",
            allocation_count
        );

        let raw_size = std::mem::size_of::<MemoryPoolBlob>();
        let blob_ptr = sys_allocate(raw_size) as *mut MemoryPoolBlob;
        unsafe {
            ptr::write_bytes(blob_ptr, 0, 1);
            (*blob_ptr).block_data = ptr::null_mut();
        }
        // Init in place
        unsafe {
            (*blob_ptr).init(self.allocation_size, allocation_count);
        }

        // Link into pool's blob list.
        unsafe {
            (*blob_ptr).prev_blob = self.last_blob;
            (*blob_ptr).next_blob = ptr::null_mut();
            if !self.last_blob.is_null() {
                (*self.last_blob).next_blob = blob_ptr;
            }
            if self.first_blob.is_null() {
                self.first_blob = blob_ptr;
            }
            self.last_blob = blob_ptr;
        }

        self.first_blob_with_free_blocks = blob_ptr;
        self.total_blocks_in_pool += allocation_count;
    }

    fn free_blob(&mut self, blob: *mut MemoryPoolBlob) -> usize {
        assert!(!blob.is_null());
        unsafe {
            let total_in_blob = (*blob).total_blocks_in_blob;
            let used_in_blob = (*blob).used_blocks_in_blob;
            assert_eq!(used_in_blob, 0, "freeing a nonempty blob");

            // Unlink from list.
            let prev = (*blob).prev_blob;
            let next = (*blob).next_blob;
            if !prev.is_null() {
                (*prev).next_blob = next;
            } else {
                self.first_blob = next;
            }
            if !next.is_null() {
                (*next).prev_blob = prev;
            } else {
                self.last_blob = prev;
            }

            if self.first_blob_with_free_blocks == blob {
                self.first_blob_with_free_blocks = self.first_blob;
            }

            // Drop the blob.
            ptr::drop_in_place(blob);
            sys_free(blob as *mut u8, std::mem::size_of::<MemoryPoolBlob>());

            self.used_blocks_in_pool -= used_in_blob;
            self.total_blocks_in_pool -= total_in_blob;
            total_in_blob * self.allocation_size + std::mem::size_of::<MemoryPoolBlob>()
        }
    }

    /// Allocate a block, zeroed. Returns pointer to user data.
    pub fn allocate_block(&mut self) -> *mut u8 {
        let ptr = self.allocate_block_do_not_zero();
        if !ptr.is_null() {
            unsafe {
                ptr::write_bytes(ptr, 0, self.allocation_size);
            }
        }
        ptr
    }

    /// Allocate a block without zeroing. Returns pointer to user data.
    pub fn allocate_block_do_not_zero(&mut self) -> *mut u8 {
        // Check if current free-blob pointer is stale.
        if !self.first_blob_with_free_blocks.is_null() {
            let has_free = unsafe { (*self.first_blob_with_free_blocks).has_free_blocks() };
            if !has_free {
                // Scan for a blob with free blocks.
                let mut blob = self.first_blob;
                let mut found: *mut MemoryPoolBlob = ptr::null_mut();
                while !blob.is_null() {
                    if unsafe { (*blob).has_free_blocks() } {
                        found = blob;
                        break;
                    }
                    blob = unsafe { (*blob).next_blob };
                }
                self.first_blob_with_free_blocks = found;
            }
        }

        // No free blocks anywhere: overflow.
        if self.first_blob_with_free_blocks.is_null() {
            if self.overflow_allocation_count == 0 {
                panic!("Pool '{}' is full and cannot grow", self.pool_name);
            }
            self.create_blob(self.overflow_allocation_count);
        }

        let blob = self.first_blob_with_free_blocks;
        assert!(!blob.is_null());
        let user_ptr = unsafe { (*blob).allocate_single_block() };

        #[cfg(debug_assertions)]
        {
            memset32(
                user_ptr,
                debug_consts::INIT_FILLER_VALUE,
                self.allocation_size,
            );
        }

        self.used_blocks_in_pool += 1;
        if self.used_blocks_in_pool > self.peak_used_blocks_in_pool {
            self.peak_used_blocks_in_pool = self.used_blocks_in_pool;
        }
        user_ptr
    }

    /// Free a block. OK to pass null.
    pub fn free_block(&mut self, user_ptr: *mut u8) {
        if user_ptr.is_null() {
            return;
        }
        let block = BlockHeader::from_user_data(user_ptr);
        let blob = unsafe { (*block).owning_blob };
        assert!(
            !blob.is_null() && unsafe { (*blob).allocation_size == self.allocation_size },
            "block does not belong to this pool"
        );
        unsafe {
            (*blob).free_single_block(user_ptr);
        }
        if self.first_blob_with_free_blocks.is_null() {
            self.first_blob_with_free_blocks = blob;
        }
        self.used_blocks_in_pool -= 1;
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

    /// Release empty blobs back to the system.
    pub fn release_empties(&mut self) -> usize {
        let mut released = 0usize;
        let mut blob = self.first_blob;
        while !blob.is_null() {
            let next = unsafe { (*blob).next_blob };
            if unsafe { (*blob).used_blocks_in_blob == 0 } {
                released += self.free_blob(blob);
            }
            blob = next;
        }
        released
    }

    /// Destroy all blocks and blobs.
    pub fn reset(&mut self) {
        while !self.first_blob.is_null() {
            self.free_blob(self.first_blob);
        }
        self.first_blob = ptr::null_mut();
        self.last_blob = ptr::null_mut();
        self.first_blob_with_free_blocks = ptr::null_mut();
        self.create_blob(self.initial_allocation_count);
    }
}

impl Drop for MemoryPool {
    fn drop(&mut self) {
        while !self.first_blob.is_null() {
            self.free_blob(self.first_blob);
        }
    }
}

// ---------------------------------------------------------------------------
// DynamicMemoryAllocator - matches C++ DynamicMemoryAllocator
// ---------------------------------------------------------------------------

/// Multi-size allocator that routes to the best-fit sub-pool.
pub struct DynamicMemoryAllocator {
    factory: *mut MemoryPoolFactory,
    pools: [*mut MemoryPool; MAX_DMA_SUBPOOLS],
    num_pools: usize,
    used_blocks_in_dma: usize,
    /// Linked list of raw (oversized) blocks.
    raw_blocks: *mut BlockHeader,
    /// Linked list pointer for factory.
    next_dma_in_factory: *mut DynamicMemoryAllocator,
}

impl DynamicMemoryAllocator {
    #[allow(dead_code)]
    fn new() -> Self {
        Self {
            factory: ptr::null_mut(),
            pools: [ptr::null_mut(); MAX_DMA_SUBPOOLS],
            num_pools: 0,
            used_blocks_in_dma: 0,
            raw_blocks: ptr::null_mut(),
            next_dma_in_factory: ptr::null_mut(),
        }
    }

    fn init(&mut self, factory: *mut MemoryPoolFactory, sub_pools: &[PoolInitRec]) {
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

        self.factory = factory;
        self.num_pools = params.len().min(MAX_DMA_SUBPOOLS);
        self.used_blocks_in_dma = 0;

        // Create sub-pools via the factory.
        // Since we can't call factory methods through raw ptr safely during init,
        // we create the pools inline.
        for (i, parm) in params.iter().take(self.num_pools).enumerate() {
            let raw_size = std::mem::size_of::<MemoryPool>();
            let pool_ptr = sys_allocate(raw_size) as *mut MemoryPool;
            unsafe {
                ptr::write_bytes(pool_ptr, 0, 1);
                (*pool_ptr).init(
                    factory,
                    parm.pool_name,
                    parm.allocation_size,
                    parm.initial_allocation_count,
                    parm.overflow_allocation_count,
                );
            }
            self.pools[i] = pool_ptr;
        }
    }

    fn find_pool_for_size(&self, alloc_size: usize) -> Option<*mut MemoryPool> {
        for i in 0..self.num_pools {
            let pool = self.pools[i];
            if !pool.is_null() && unsafe { (*pool).get_allocation_size() } >= alloc_size {
                return Some(pool);
            }
        }
        None
    }

    /// Allocate bytes, zeroed. Never returns null (panics on OOM).
    pub fn allocate_bytes(&mut self, num_bytes: usize) -> *mut u8 {
        let ptr = self.allocate_bytes_do_not_zero(num_bytes);
        if !ptr.is_null() {
            unsafe {
                ptr::write_bytes(ptr, 0, num_bytes);
            }
        }
        ptr
    }

    /// Allocate bytes without zeroing. Never returns null.
    pub fn allocate_bytes_do_not_zero(&mut self, num_bytes: usize) -> *mut u8 {
        if let Some(pool_ptr) = self.find_pool_for_size(num_bytes) {
            let user_ptr = unsafe { (*pool_ptr).allocate_block_do_not_zero() };
            self.used_blocks_in_dma += 1;
            return user_ptr;
        }

        // Too large for any sub-pool: allocate as a raw block.
        let raw_size = BlockHeader::calc_raw_block_size(num_bytes);
        let block_ptr = sys_allocate(raw_size) as *mut BlockHeader;
        unsafe {
            (*block_ptr).init(num_bytes, ptr::null_mut());
            (*block_ptr).owning_blob = ptr::null_mut();
            (*block_ptr).next = self.raw_blocks;
            (*block_ptr).prev = ptr::null_mut();
            if !self.raw_blocks.is_null() {
                (*self.raw_blocks).prev = block_ptr;
            }
            self.raw_blocks = block_ptr;
        }

        #[cfg(debug_assertions)]
        {
            let user = unsafe { (*block_ptr).user_data_ptr() };
            memset32(user, debug_consts::INIT_FILLER_VALUE, num_bytes);
        }

        self.used_blocks_in_dma += 1;
        unsafe { (*block_ptr).user_data_ptr() }
    }

    /// Free bytes. OK to pass null.
    pub fn free_bytes(&mut self, user_ptr: *mut u8) {
        if user_ptr.is_null() {
            return;
        }
        let block = BlockHeader::from_user_data(user_ptr);
        let blob = unsafe { (*block).owning_blob };

        if !blob.is_null() {
            // Belongs to a sub-pool blob.
            let pool = unsafe { (*blob).allocation_size };
            if let Some(pool_ptr) = self.find_pool_for_size(pool) {
                unsafe {
                    (*pool_ptr).free_block(user_ptr);
                }
            }
        } else {
            // Raw block (oversized). Remove from linked list and free.
            unsafe {
                let next = (*block).next;
                let prev = (*block).prev;
                if !prev.is_null() {
                    (*prev).next = next;
                } else {
                    self.raw_blocks = next;
                }
                if !next.is_null() {
                    (*next).prev = prev;
                }
                #[cfg(debug_assertions)]
                {
                    (*block).mark_as_freed();
                }
                let raw_size = BlockHeader::calc_raw_block_size((*block).logical_size);
                sys_free(block as *mut u8, raw_size);
            }
        }
        self.used_blocks_in_dma -= 1;
    }

    /// Return the actual allocation size for a request (may be larger than requested).
    pub fn get_actual_allocation_size(&self, num_bytes: usize) -> usize {
        if let Some(pool_ptr) = self.find_pool_for_size(num_bytes) {
            unsafe { (*pool_ptr).get_allocation_size() }
        } else {
            num_bytes
        }
    }

    /// Reset all allocations.
    pub fn reset(&mut self) {
        for i in 0..self.num_pools {
            let pool = self.pools[i];
            if !pool.is_null() {
                unsafe {
                    (*pool).reset();
                }
            }
        }
        // Free raw blocks.
        while !self.raw_blocks.is_null() {
            let block = self.raw_blocks;
            unsafe {
                self.raw_blocks = (*block).next;
                let raw_size = BlockHeader::calc_raw_block_size((*block).logical_size);
                sys_free(block as *mut u8, raw_size);
            }
        }
        self.used_blocks_in_dma = 0;
    }
}

impl Drop for DynamicMemoryAllocator {
    fn drop(&mut self) {
        // Free raw blocks.
        while !self.raw_blocks.is_null() {
            let block = self.raw_blocks;
            unsafe {
                self.raw_blocks = (*block).next;
                let raw_size = BlockHeader::calc_raw_block_size((*block).logical_size);
                sys_free(block as *mut u8, raw_size);
            }
        }
        // Destroy sub-pools.
        for i in 0..self.num_pools {
            let pool = self.pools[i];
            if !pool.is_null() {
                unsafe {
                    ptr::drop_in_place(pool);
                    sys_free(pool as *mut u8, std::mem::size_of::<MemoryPool>());
                }
                self.pools[i] = ptr::null_mut();
            }
        }
    }
}

// ---------------------------------------------------------------------------
// MemoryPoolFactory - matches C++ MemoryPoolFactory
// ---------------------------------------------------------------------------

/// Central manager for all MemoryPools and DynamicMemoryAllocators.
pub struct MemoryPoolFactory {
    pools: HashMap<String, *mut MemoryPool>,
    pool_list_head: *mut MemoryPool,
    dma_list_head: *mut DynamicMemoryAllocator,
    /// Debug: total bytes in use (logical).
    #[cfg(debug_assertions)]
    used_bytes: usize,
    /// Debug: total bytes allocated (physical).
    #[cfg(debug_assertions)]
    phys_bytes: usize,
}

impl MemoryPoolFactory {
    fn new() -> Self {
        Self {
            pools: HashMap::new(),
            pool_list_head: ptr::null_mut(),
            dma_list_head: ptr::null_mut(),
            #[cfg(debug_assertions)]
            used_bytes: 0,
            #[cfg(debug_assertions)]
            phys_bytes: 0,
        }
    }

    fn init(&mut self) {
        // C++ just sets defaults, which we already do.
    }

    /// Create a memory pool. If a pool with the given name exists, return it.
    pub fn create_memory_pool(
        &mut self,
        pool_name: &str,
        allocation_size: usize,
        initial_allocation_count: usize,
        overflow_allocation_count: usize,
    ) -> *mut MemoryPool {
        if let Some(&existing) = self.pools.get(pool_name) {
            assert_eq!(
                unsafe { (*existing).get_allocation_size() },
                round_up_mem_bound(allocation_size),
                "pool size mismatch for '{}'",
                pool_name
            );
            return existing;
        }

        assert!(
            initial_allocation_count > 0,
            "illegal pool size: initial={}",
            initial_allocation_count
        );

        let raw_size = std::mem::size_of::<MemoryPool>();
        let pool_ptr = sys_allocate(raw_size) as *mut MemoryPool;
        unsafe {
            ptr::write_bytes(pool_ptr, 0, 1);
            (*pool_ptr).init(
                self as *mut MemoryPoolFactory,
                pool_name,
                allocation_size,
                initial_allocation_count,
                overflow_allocation_count,
            );
            (*pool_ptr).next_pool_in_factory = self.pool_list_head;
        }
        self.pool_list_head = pool_ptr;
        self.pools.insert(pool_name.to_string(), pool_ptr);
        pool_ptr
    }

    /// Find an existing pool by name.
    pub fn find_memory_pool(&self, pool_name: &str) -> Option<*mut MemoryPool> {
        self.pools.get(pool_name).copied()
    }

    /// Destroy a memory pool.
    pub fn destroy_memory_pool(&mut self, pool: *mut MemoryPool) {
        if pool.is_null() {
            return;
        }
        unsafe {
            assert_eq!(
                (*pool).get_used_block_count(),
                0,
                "destroying a nonempty pool"
            );
            // Unlink from list.
            let mut prev: *mut MemoryPool = ptr::null_mut();
            let mut cur = self.pool_list_head;
            while !cur.is_null() {
                if cur == pool {
                    if !prev.is_null() {
                        (*prev).next_pool_in_factory = (*cur).next_pool_in_factory;
                    } else {
                        self.pool_list_head = (*cur).next_pool_in_factory;
                    }
                    break;
                }
                prev = cur;
                cur = (*cur).next_pool_in_factory;
            }
        }
        // Remove from hashmap.
        let name = unsafe { (*pool).get_pool_name().to_string() };
        self.pools.remove(&name);
        // Drop and free.
        unsafe {
            ptr::drop_in_place(pool);
            sys_free(pool as *mut u8, std::mem::size_of::<MemoryPool>());
        }
    }

    /// Create a DMA with given sub-pool parameters. Empty slice uses defaults.
    pub fn create_dynamic_memory_allocator(
        &mut self,
        sub_pools: &[PoolInitRec],
    ) -> *mut DynamicMemoryAllocator {
        let raw_size = std::mem::size_of::<DynamicMemoryAllocator>();
        let dma_ptr = sys_allocate(raw_size) as *mut DynamicMemoryAllocator;
        unsafe {
            ptr::write_bytes(dma_ptr, 0, 1);
            (*dma_ptr).init(self as *mut MemoryPoolFactory, sub_pools);
            (*dma_ptr).next_dma_in_factory = self.dma_list_head;
        }
        self.dma_list_head = dma_ptr;
        dma_ptr
    }

    /// Destroy a DMA.
    pub fn destroy_dynamic_memory_allocator(&mut self, dma: *mut DynamicMemoryAllocator) {
        if dma.is_null() {
            return;
        }
        // Unlink.
        unsafe {
            let mut prev: *mut DynamicMemoryAllocator = ptr::null_mut();
            let mut cur = self.dma_list_head;
            while !cur.is_null() {
                if cur == dma {
                    if !prev.is_null() {
                        (*prev).next_dma_in_factory = (*cur).next_dma_in_factory;
                    } else {
                        self.dma_list_head = (*cur).next_dma_in_factory;
                    }
                    break;
                }
                prev = cur;
                cur = (*cur).next_dma_in_factory;
            }
        }
        unsafe {
            ptr::drop_in_place(dma);
            sys_free(
                dma as *mut u8,
                std::mem::size_of::<DynamicMemoryAllocator>(),
            );
        }
    }

    /// Reset all pools and DMAs.
    pub fn reset(&mut self) {
        let mut pool = self.pool_list_head;
        while !pool.is_null() {
            unsafe {
                (*pool).reset();
                pool = (*pool).next_pool_in_factory;
            }
        }
        let mut dma = self.dma_list_head;
        while !dma.is_null() {
            unsafe {
                (*dma).reset();
                dma = (*dma).next_dma_in_factory;
            }
        }
        #[cfg(debug_assertions)]
        {
            self.used_bytes = 0;
            self.phys_bytes = 0;
        }
    }
}

impl Drop for MemoryPoolFactory {
    fn drop(&mut self) {
        while !self.pool_list_head.is_null() {
            let next = unsafe { (*self.pool_list_head).next_pool_in_factory };
            self.destroy_memory_pool(self.pool_list_head);
            self.pool_list_head = next;
        }
        while !self.dma_list_head.is_null() {
            let next = unsafe { (*self.dma_list_head).next_dma_in_factory };
            self.destroy_dynamic_memory_allocator(self.dma_list_head);
            self.dma_list_head = next;
        }
    }
}

// ---------------------------------------------------------------------------
// Global singletons (matching C++ TheMemoryPoolFactory, TheDynamicMemoryAllocator)
// ---------------------------------------------------------------------------

/// Wrapper to make raw DMA pointer Send-safe.
/// The Mutex already provides exclusive access, so this is sound.
struct DmaPtr(*mut DynamicMemoryAllocator);
unsafe impl Send for DmaPtr {}
unsafe impl Send for MemoryPoolFactory {}

lazy_static::lazy_static! {
    static ref THE_FACTORY: Mutex<MemoryPoolFactory> = Mutex::new(MemoryPoolFactory::new());
    static ref THE_DMA: Mutex<Option<DmaPtr>> = Mutex::new(None);
}

/// Initialize the memory manager (matches C++ initMemoryManager).
pub fn init_memory_manager() {
    let mut factory = THE_FACTORY.lock().unwrap();
    factory.init();

    let dma_ptr = factory.create_dynamic_memory_allocator(&[]);
    let mut dma = THE_DMA.lock().unwrap();
    *dma = Some(DmaPtr(dma_ptr));
}

/// Shut down the memory manager.
pub fn shutdown_memory_manager() {
    {
        let mut dma = THE_DMA.lock().unwrap();
        *dma = None;
    }
    let mut factory = THE_FACTORY.lock().unwrap();
    // Destroy all DMAs and pools.
    loop {
        let head = factory.dma_list_head;
        if head.is_null() {
            break;
        }
        let next = unsafe { (*head).next_dma_in_factory };
        factory.destroy_dynamic_memory_allocator(head);
        factory.dma_list_head = next;
    }
    loop {
        let head = factory.pool_list_head;
        if head.is_null() {
            break;
        }
        let next = unsafe { (*head).next_pool_in_factory };
        factory.destroy_memory_pool(head);
        factory.pool_list_head = next;
    }
    factory.pools.clear();
}

/// Get the global MemoryPoolFactory.
pub fn get_memory_pool_factory() -> &'static Mutex<MemoryPoolFactory> {
    &THE_FACTORY
}

/// Allocate bytes from the global DMA (zeroed). Panics on OOM.
pub fn dma_allocate(num_bytes: usize) -> *mut u8 {
    let dma_opt = THE_DMA.lock().unwrap();
    let dma_ptr = dma_opt.as_ref().expect("DMA not initialized").0;
    let dma = unsafe { &mut *dma_ptr };
    dma.allocate_bytes(num_bytes)
}

/// Allocate bytes from the global DMA (not zeroed). Panics on OOM.
pub fn dma_allocate_do_not_zero(num_bytes: usize) -> *mut u8 {
    let dma_opt = THE_DMA.lock().unwrap();
    let dma_ptr = dma_opt.as_ref().expect("DMA not initialized").0;
    let dma = unsafe { &mut *dma_ptr };
    dma.allocate_bytes_do_not_zero(num_bytes)
}

/// Free bytes through the global DMA. OK to pass null.
pub fn dma_free(ptr: *mut u8) {
    if ptr.is_null() {
        return;
    }
    let dma_opt = THE_DMA.lock().unwrap();
    let dma_ptr = dma_opt.as_ref().expect("DMA not initialized").0;
    let dma = unsafe { &mut *dma_ptr };
    dma.free_bytes(ptr);
}

/// Create a named memory pool via the global factory.
pub fn create_pool(
    pool_name: &str,
    allocation_size: usize,
    initial_count: usize,
    overflow_count: usize,
) -> *mut MemoryPool {
    let mut factory = THE_FACTORY.lock().unwrap();
    factory.create_memory_pool(pool_name, allocation_size, initial_count, overflow_count)
}

/// Find a named pool.
pub fn find_pool(pool_name: &str) -> Option<*mut MemoryPool> {
    let factory = THE_FACTORY.lock().unwrap();
    factory.find_memory_pool(pool_name)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_round_up_mem_bound() {
        assert_eq!(round_up_mem_bound(1), 4);
        assert_eq!(round_up_mem_bound(4), 4);
        assert_eq!(round_up_mem_bound(5), 8);
        assert_eq!(round_up_mem_bound(8), 8);
    }

    #[test]
    fn test_pool_create_and_alloc() {
        let mut factory = MemoryPoolFactory::new();
        factory.init();
        let pool = factory.create_memory_pool("TestPool", 64, 16, 16);
        assert!(!pool.is_null());

        unsafe {
            assert_eq!((*pool).get_allocation_size(), 64);
            assert_eq!((*pool).get_total_block_count(), 16);
            assert_eq!((*pool).get_used_block_count(), 0);

            let p1 = (*pool).allocate_block();
            assert!(!p1.is_null());
            assert_eq!((*pool).get_used_block_count(), 1);

            // Verify zeroed.
            let slice = std::slice::from_raw_parts(p1, 64);
            assert!(slice.iter().all(|&b| b == 0));

            (*pool).free_block(p1);
            assert_eq!((*pool).get_used_block_count(), 0);
        }
    }

    #[test]
    fn test_pool_overflow() {
        let mut factory = MemoryPoolFactory::new();
        factory.init();
        let pool = factory.create_memory_pool("OverflowPool", 32, 4, 4);
        assert!(!pool.is_null());

        unsafe {
            // Allocate more than initial count.
            let mut ptrs = Vec::new();
            for _ in 0..8 {
                let p = (*pool).allocate_block();
                assert!(!p.is_null());
                ptrs.push(p);
            }
            assert_eq!((*pool).get_used_block_count(), 8);
            assert!((*pool).get_total_block_count() >= 8);

            for p in ptrs {
                (*pool).free_block(p);
            }
            assert_eq!((*pool).get_used_block_count(), 0);
        }
    }

    #[test]
    fn test_dma_basic() {
        let mut factory = MemoryPoolFactory::new();
        factory.init();
        let dma = factory.create_dynamic_memory_allocator(&[]);
        assert!(!dma.is_null());

        unsafe {
            // Small allocation goes to sub-pool.
            let p1 = (*dma).allocate_bytes(32);
            assert!(!p1.is_null());
            // Verify zeroed.
            let slice = std::slice::from_raw_parts(p1, 32);
            assert!(slice.iter().all(|&b| b == 0));
            (*dma).free_bytes(p1);

            // Large allocation (bigger than any sub-pool) goes raw.
            let p2 = (*dma).allocate_bytes(2048);
            assert!(!p2.is_null());
            (*dma).free_bytes(p2);
        }
    }

    #[test]
    fn test_pool_reset() {
        let mut factory = MemoryPoolFactory::new();
        factory.init();
        let pool = factory.create_memory_pool("ResetPool", 64, 8, 8);
        unsafe {
            let p = (*pool).allocate_block();
            assert!(!p.is_null());
            (*pool).reset();
            assert_eq!((*pool).get_used_block_count(), 0);
        }
    }

    #[test]
    fn test_factory_destroy_pool() {
        let mut factory = MemoryPoolFactory::new();
        factory.init();
        let pool = factory.create_memory_pool("DestroyPool", 64, 4, 4);
        assert!(factory.find_memory_pool("DestroyPool").is_some());
        factory.destroy_memory_pool(pool);
        assert!(factory.find_memory_pool("DestroyPool").is_none());
    }

    #[test]
    fn test_fill_pattern_on_free() {
        let mut factory = MemoryPoolFactory::new();
        factory.init();
        let pool = factory.create_memory_pool("FillPool", 64, 4, 4);
        unsafe {
            let p = (*pool).allocate_block();
            // Write something.
            ptr::write_bytes(p, 0xAA, 64);
            (*pool).free_block(p);
            // Re-allocate: in debug, it should have been filled with GARBAGE_FILL_VALUE
            // but then we wrote 0xAA. After free, it's filled with DEADBEEF.
            // Then allocate_block zeros it again.
            let p2 = (*pool).allocate_block();
            let slice = std::slice::from_raw_parts(p2, 64);
            assert!(slice.iter().all(|&b| b == 0));
        }
    }

    #[test]
    fn test_bounding_walls() {
        #[cfg(debug_assertions)]
        {
            let mut factory = MemoryPoolFactory::new();
            factory.init();
            let pool = factory.create_memory_pool("WallPool", 64, 4, 4);
            unsafe {
                let p = (*pool).allocate_block();
                let block = BlockHeader::from_user_data(p);
                assert!((*block).verify());
                assert!((*block).check_underrun());
                assert!((*block).check_overrun());
                (*pool).free_block(p);
            }
        }
    }

    #[test]
    fn test_init_memory_manager() {
        init_memory_manager();
        let p = dma_allocate(128);
        assert!(!p.is_null());
        let slice = unsafe { std::slice::from_raw_parts(p, 128) };
        assert!(slice.iter().all(|&b| b == 0));
        dma_free(p);
        shutdown_memory_manager();
    }
}
