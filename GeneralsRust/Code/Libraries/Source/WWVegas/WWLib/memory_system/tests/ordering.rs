use memory_system::with_memory_pool;

#[test]
fn free_slots_are_reused_lifo_without_abandoning_current_blob() {
    with_memory_pool(|pool| {
        pool.init("ordering", 8, 2, 2).unwrap();
        let first = pool.allocate_block().unwrap();
        let second = pool.allocate_block().unwrap();
        let first_address = pool.get_block(first).unwrap().as_ptr();
        let second_address = pool.get_block(second).unwrap().as_ptr();
        assert!(pool.free_block(first));
        assert!(pool.free_block(second));
        let reused = pool.allocate_block().unwrap();
        assert_eq!(pool.get_block(reused).unwrap().as_ptr(), second_address);
        let reused_first = pool.allocate_block().unwrap();
        assert_eq!(pool.get_block(reused_first).unwrap().as_ptr(), first_address);
        let overflow = pool.allocate_block().unwrap();
        assert!(pool.free_block(reused_first));
        let next = pool.allocate_block().unwrap();
        assert_ne!(pool.get_block(next).unwrap().as_ptr(), first_address);
        assert!(pool.free_block(reused));
        assert!(pool.free_block(overflow));
        assert!(pool.free_block(next));
    });
}
