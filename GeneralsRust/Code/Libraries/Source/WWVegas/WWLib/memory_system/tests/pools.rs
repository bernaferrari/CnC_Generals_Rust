use memory_system::{with_dynamic_memory_allocator, with_memory_manager, with_memory_pool, PoolInitRec};

#[test]
fn pool_growth_reuse_zeroing_alignment_and_counters() {
    with_memory_pool(|pool| {
        pool.init("odd", 5, 2, 3).unwrap();
        assert_eq!(pool.get_allocation_size(), 8);
        let a = pool.allocate_block().unwrap();
        let b = pool.allocate_block().unwrap();
        for handle in [a, b] {
            let bytes = pool.get_block_mut(handle).unwrap();
            assert_eq!(&*bytes, &[0; 8]);
            assert_eq!(bytes.as_ptr().align_offset(std::mem::align_of::<usize>()), 0);
            bytes.fill(0xaa);
        }
        let c = pool.allocate_block().unwrap();
        assert_eq!((pool.get_used_block_count(), pool.get_total_block_count(), pool.get_free_block_count(), pool.get_peak_block_count()), (3, 5, 2, 3));
        assert!(pool.free_block(a));
        assert!(!pool.free_block(a));
        let replacement = pool.allocate_block().unwrap();
        assert_ne!(a, replacement);
        assert_eq!(pool.get_block(replacement), Some(&[0; 8][..]));
        assert!(pool.get_block(a).is_none());
        assert!(!pool.free_block(a));
        assert!(pool.free_block(b));
        assert!(pool.free_block(c));
        assert!(pool.free_block(replacement));
        assert_eq!((pool.get_used_block_count(), pool.get_free_block_count(), pool.get_peak_block_count()), (0, 5, 3));
    });
}

#[test]
fn reinitialization_invalidates_pool_and_dma_handles() {
    with_memory_pool(|pool| {
        pool.init("a", 4, 1, 0).unwrap();
        let old = pool.allocate_block().unwrap();
        assert!(pool.allocate_block().is_err());
        pool.init("b", 4, 1, 0).unwrap();
        let current = pool.allocate_block().unwrap();
        assert!(pool.get_block(old).is_none());
        assert!(!pool.free_block(old));
        assert!(pool.free_block(current));
    });
    with_dynamic_memory_allocator(|dma| {
        let configs = [PoolInitRec::new("a", 8, 1, 1)];
        dma.init(&configs).unwrap();
        let old = dma.allocate_bytes(4).unwrap();
        dma.init(&configs).unwrap();
        let current = dma.allocate_bytes(4).unwrap();
        assert!(dma.get_bytes(old).is_none());
        assert!(!dma.free_bytes(old));
        assert!(dma.free_bytes(current));
    });
}

#[test]
fn dynamic_selection_raw_storage_and_failed_growth() {
    with_dynamic_memory_allocator(|dma| {
        dma.init(&[PoolInitRec::new("small", 8, 1, 0), PoolInitRec::new("large", 32, 1, 1)]).unwrap();
        let small = dma.allocate_bytes(3).unwrap();
        // C++ chooses the first fitting pool and does not fall back when it is full.
        assert!(dma.allocate_bytes(3).is_err());
        let large = dma.allocate_bytes(9).unwrap();
        let raw = dma.allocate_bytes(33).unwrap();
        assert_eq!(dma.get_bytes(small).unwrap().len(), 8);
        assert_eq!(dma.get_bytes(large).unwrap().len(), 32);
        assert_eq!(dma.get_bytes(raw).unwrap(), &[0; 33]);
        assert_eq!(dma.get_bytes(raw).unwrap().as_ptr().align_offset(std::mem::align_of::<usize>()), 0);
        dma.get_bytes_mut(raw).unwrap()[32] = 7;
        assert_eq!(dma.get_bytes(raw).unwrap()[32], 7);
        let stats = dma.get_memory_stats();
        assert_eq!((stats["pool_0_used_blocks"], stats["pool_1_used_blocks"], stats["raw_blocks_count"], stats["used_blocks_in_dma"]), (1, 1, 1, 3));
        for block in [small, large, raw] { assert!(dma.free_bytes(block)); }
        assert_eq!(dma.get_memory_stats()["raw_blocks_count"], 0);
        assert_eq!(dma.get_used_block_count(), 0);
    });
}

#[test]
fn same_scope_foreign_pools_and_allocators_are_rejected() {
    with_memory_manager(&[], |session| {
        let mut factory = session.factory().lock();
        let first = factory.create_memory_pool("a", 4, 1, 1).unwrap().allocate_block().unwrap();
        let second = factory.create_memory_pool("b", 4, 1, 1).unwrap().allocate_block().unwrap();
        let pool = factory.memory_pool_mut(1).unwrap();
        assert!(pool.get_block(first).is_none());
        assert!(!pool.free_block(first));
        assert!(pool.free_block(second));
        assert!(factory.memory_pool_mut(0).unwrap().free_block(first));
        let a = factory.dynamic_memory_allocator_mut(0).unwrap().allocate_bytes(1).unwrap();
        let b = factory.create_dynamic_memory_allocator(&[]).unwrap().allocate_bytes(1).unwrap();
        let dma = factory.dynamic_memory_allocator_mut(1).unwrap();
        assert!(dma.get_bytes(a).is_none());
        assert!(!dma.free_bytes(a));
        assert!(dma.free_bytes(b));
        assert!(factory.dynamic_memory_allocator_mut(0).unwrap().free_bytes(a));
    }).unwrap();
}

#[test]
fn defaults_invalid_configs_and_reset() {
    with_dynamic_memory_allocator(|dma| {
        dma.init(&[]).unwrap();
        assert_eq!(dma.get_memory_stats()["num_pools"], 7);
        assert!(dma.init(&[PoolInitRec::new("bad", -1, 1, 1)]).is_err());
        assert_eq!(dma.get_memory_stats()["num_pools"], 7);
    });
    with_memory_manager(&[], |session| {
        let mut factory = session.factory().lock();
        let block = factory.create_memory_pool("reset", 4, 1, 0).unwrap().allocate_block().unwrap();
        factory.reset_all_pools();
        let pool = factory.memory_pool_mut(0).unwrap();
        assert!(!pool.free_block(block));
        assert_eq!(pool.get_used_block_count(), 0);
        let new_block = pool.allocate_block().unwrap();
        assert_ne!(block, new_block);
    }).unwrap();
}
