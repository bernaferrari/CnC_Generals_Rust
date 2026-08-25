//! Allocator alignment tests for the GameMemory port.
//!
//! `game_engine` disables lib tests (`[lib] test = false`), so the unit tests
//! in `game_memory.rs` do not run via `cargo test -p game_engine`. These
//! integration tests cover the same invariants through the public API.
//!
//! C++ used `MEM_BOUND_ALIGNMENT = 4` on 32-bit; the 64-bit Rust port must
//! honor pointer alignment (`max(align_of::<usize>(), 8)`). Pool allocation
//! sizes may round up slightly vs C++ — that is OK.

use game_engine::common::system::game_memory::{
    MEM_BOUND_ALIGNMENT, MemoryPoolFactory, debug_block_header_from_user,
    debug_write_header_pointer_fields,
};
use std::ptr;

#[test]
fn game_memory_mem_bound_alignment_is_pointer_safe() {
    assert_eq!(MEM_BOUND_ALIGNMENT, std::mem::align_of::<usize>().max(8));
    assert!(MEM_BOUND_ALIGNMENT >= 8);
    assert!(MEM_BOUND_ALIGNMENT.is_power_of_two());
}

#[test]
fn game_memory_pool_size_rounding_still_works() {
    let mut factory = MemoryPoolFactory::new();
    unsafe {
        let p1 = factory.create_memory_pool("Round1", 1, 8, 8);
        assert_eq!((*p1).get_allocation_size(), MEM_BOUND_ALIGNMENT);

        let p4 = factory.create_memory_pool("Round4", 4, 8, 8);
        assert_eq!((*p4).get_allocation_size(), MEM_BOUND_ALIGNMENT);

        let p5 = factory.create_memory_pool("Round5", 5, 8, 8);
        assert_eq!((*p5).get_allocation_size(), MEM_BOUND_ALIGNMENT);

        let p8 = factory.create_memory_pool("Round8", 8, 8, 8);
        assert_eq!((*p8).get_allocation_size(), 8);

        let p12 = factory.create_memory_pool("Round12", 12, 8, 8);
        assert_eq!((*p12).get_allocation_size(), 16);

        let p16 = factory.create_memory_pool("Round16", 16, 8, 8);
        assert_eq!((*p16).get_allocation_size(), 16);

        let p64 = factory.create_memory_pool("Round64", 64, 8, 8);
        assert_eq!((*p64).get_allocation_size(), 64);
    }
}

#[test]
fn game_memory_allocated_user_pointers_are_aligned() {
    let mut factory = MemoryPoolFactory::new();
    unsafe {
        let pool = factory.create_memory_pool("UserAlign", 1, 8, 8);
        let mut ptrs = Vec::new();
        for _ in 0..8 {
            let p = (*pool).allocate_block();
            assert!(!p.is_null());
            assert_eq!(
                p as usize % MEM_BOUND_ALIGNMENT,
                0,
                "user pointer {:p} not aligned to {}",
                p,
                MEM_BOUND_ALIGNMENT
            );
            // User region itself is safe for a pointer-sized store.
            ptr::write(p as *mut usize, 0xA5A5_A5A5_A5A5_A5A5);
            assert_eq!(ptr::read(p as *mut usize), 0xA5A5_A5A5_A5A5_A5A5);
            ptrs.push(p);
        }
        for p in ptrs {
            (*pool).free_block(p);
        }
    }
}

#[test]
fn game_memory_header_pointer_fields_can_be_written_without_misaligned_ptr() {
    let mut factory = MemoryPoolFactory::new();
    unsafe {
        let pool = factory.create_memory_pool("HdrAlign", 16, 8, 8);
        let p = (*pool).allocate_block();
        let hdr = debug_block_header_from_user(p);
        assert_eq!(
            hdr as usize % MEM_BOUND_ALIGNMENT,
            0,
            "header {:p} not aligned to {}",
            hdr,
            MEM_BOUND_ALIGNMENT
        );
        assert_eq!(hdr as usize % std::mem::align_of::<*mut u8>(), 0);
        assert!(
            debug_write_header_pointer_fields(p),
            "header pointer-field write failed (misaligned?)"
        );
        (*pool).free_block(p);
    }
}

#[test]
fn game_memory_dma_user_and_header_alignment() {
    let mut factory = MemoryPoolFactory::new();
    let dma = factory.create_dynamic_memory_allocator(&[]);
    unsafe {
        // Sub-pool path (fits in default 16..1024 pools).
        let small = (*dma).allocate_bytes(3);
        assert_eq!(small as usize % MEM_BOUND_ALIGNMENT, 0);
        assert_eq!(
            debug_block_header_from_user(small) as usize % MEM_BOUND_ALIGNMENT,
            0
        );
        assert!(debug_write_header_pointer_fields(small));
        assert_eq!((*dma).get_actual_allocation_size(3), 16);
        (*dma).free_bytes(small);

        // Raw-block path (larger than any default sub-pool).
        let large = (*dma).allocate_bytes(2048);
        assert_eq!(large as usize % MEM_BOUND_ALIGNMENT, 0);
        assert_eq!(
            debug_block_header_from_user(large) as usize % MEM_BOUND_ALIGNMENT,
            0
        );
        assert!(debug_write_header_pointer_fields(large));
        assert_eq!((*dma).get_actual_allocation_size(2048), 2048);
        (*dma).free_bytes(large);
    }
}

#[test]
fn game_memory_debug_fill_and_walls_still_work() {
    let mut factory = MemoryPoolFactory::new();
    unsafe {
        let pool = factory.create_memory_pool("FillWalls", 64, 8, 8);
        let p = (*pool).allocate_block();
        ptr::write_bytes(p, 0xAA, 64);
        (*pool).free_block(p);
        // allocate_block zeros after the free-fill; walls are re-armed.
        let p2 = (*pool).allocate_block();
        let slice = std::slice::from_raw_parts(p2, 64);
        assert!(slice.iter().all(|&b| b == 0));
        assert!(debug_write_header_pointer_fields(p2));
        (*pool).free_block(p2);
    }
}
