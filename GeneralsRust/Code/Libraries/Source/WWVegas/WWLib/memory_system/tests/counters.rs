use memory_system::{with_memory_manager, PoolInitRec};

#[test]
fn reports_include_requested_bytes_and_implicit_frees() {
    let configs = [PoolInitRec::new("dma", 16, 2, 1)];
    with_memory_manager(&configs, |session| {
        let mut factory = session.factory().lock();
        let pool = factory.create_memory_pool("fixed", 5, 2, 1).unwrap();
        let fixed = pool.allocate_block().unwrap(); // C++ rounds to 8
        let dma = factory.dynamic_memory_allocator_mut(0).unwrap();
        let pooled = dma.allocate_bytes(3).unwrap();
        let raw = dma.allocate_bytes(20).unwrap();
        let report = factory.get_memory_report();
        assert_eq!((report["total_allocated"], report["total_freed"]), (31, 0));
        assert!(factory.dynamic_memory_allocator_mut(0).unwrap().free_bytes(pooled));
        assert!(!factory.dynamic_memory_allocator_mut(0).unwrap().free_bytes(pooled));
        assert_eq!(factory.get_memory_report()["total_freed"], 3);
        factory.dynamic_memory_allocator_mut(0).unwrap().init(&configs).unwrap();
        assert!(!factory.dynamic_memory_allocator_mut(0).unwrap().free_bytes(raw));
        assert_eq!(factory.get_memory_report()["total_freed"], 23);
        factory.reset_all_pools();
        assert!(!factory.memory_pool_mut(0).unwrap().free_block(fixed));
        assert_eq!(factory.get_memory_report()["total_freed"], 31);
        let pool = factory.memory_pool_mut(0).unwrap();
        let old = pool.allocate_block().unwrap();
        pool.init("new", 4, 1, 0).unwrap();
        assert!(!pool.free_block(old));
        let report = factory.get_memory_report();
        assert_eq!((report["total_allocated"], report["total_freed"]), (39, 39));
    }).unwrap();
}
