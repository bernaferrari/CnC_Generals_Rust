use memory_system::{with_memory_manager, PoolInitRec};

#[test]
fn scoped_session_allocations_are_isolated_and_reused() {
    let configs = [PoolInitRec::new("small", 16, 1, 2)];
    with_memory_manager(&configs, |session| {
        let first = session.allocate_memory(8).unwrap();
        assert_eq!(session.with_memory(first, |bytes| bytes.to_vec()), Some(vec![0; 16]));
        session.with_memory_mut(first, |bytes| bytes[0] = 42).unwrap();
        assert_eq!(session.with_memory(first, |bytes| bytes[0]), Some(42));
        let second = session.allocate_memory(8).unwrap();
        {
            let factory = session.factory().lock();
            let report = factory.get_memory_report();
            assert_eq!(report["all_dmas_used_blocks"], 2);
        }
        assert!(session.free_memory(first));
        assert!(!session.free_memory(first));
        assert!(session.with_memory(first, |_| ()).is_none());
        let reused = session.allocate_memory(8).unwrap();
        assert_ne!(first, reused);
        assert_eq!(session.with_memory(reused, |bytes| bytes[0]), Some(0));
        assert!(!session.free_memory(first));
        assert!(session.free_memory(second));
        assert!(session.free_memory(reused));
    }).unwrap();
}
