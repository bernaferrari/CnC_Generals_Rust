//! Comprehensive Tests for Memory Pooling System

#[cfg(test)]
mod tests {
    use crate::memory::*;
    use std::sync::{Arc, Barrier};
    use std::thread;

    #[derive(Debug, Clone, PartialEq)]
    struct GameObject {
        id: u32,
        position: [f32; 3],
        velocity: [f32; 3],
        health: f32,
    }

    impl GameObject {
        fn new(id: u32) -> Self {
            Self {
                id,
                position: [0.0, 0.0, 0.0],
                velocity: [0.0, 0.0, 0.0],
                health: 100.0,
            }
        }
    }

    #[test]
    fn test_basic_alloc_dealloc() {
        let config = PoolConfig::for_game_objects("TestPool");
        let pool = ObjectPool::<GameObject>::new(config).unwrap();

        let handle = pool.alloc(GameObject::new(1)).unwrap();
        assert_eq!(pool.len(), 1);

        let result = handle.with(|obj| obj.id).unwrap();
        assert_eq!(result, 1);

        drop(handle);
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn test_many_allocations() {
        let config = PoolConfig::for_game_objects("TestPool");
        let pool = ObjectPool::<GameObject>::new(config).unwrap();

        let handles: Vec<_> = (0..1000)
            .map(|i| pool.alloc(GameObject::new(i)).unwrap())
            .collect();

        assert_eq!(pool.len(), 1000);

        for (i, handle) in handles.iter().enumerate() {
            let id = handle.with(|obj| obj.id).unwrap();
            assert_eq!(id, i as u32);
        }
    }

    #[test]
    fn test_generation_tracking() {
        let config = PoolConfig::for_game_objects("TestPool");
        let pool = ObjectPool::<GameObject>::new(config).unwrap();

        // Allocate object
        let handle1 = pool.alloc(GameObject::new(1)).unwrap();
        let index1 = handle1.index();

        // Free it
        handle1.free().unwrap();

        // Allocate another in the same slot
        let handle2 = pool.alloc(GameObject::new(2)).unwrap();
        let index2 = handle2.index();

        // Indices should have same index but different generation
        assert_eq!(index1.index(), index2.index());
        assert_ne!(index1.generation(), index2.generation());

        // Old index should be invalid
        assert!(!pool.is_valid(index1));
        assert!(pool.is_valid(index2));
    }

    #[test]
    fn test_handle_mutation() {
        let config = PoolConfig::for_game_objects("TestPool");
        let pool = ObjectPool::<GameObject>::new(config).unwrap();

        let handle = pool.alloc(GameObject::new(1)).unwrap();

        handle
            .with_mut(|obj| {
                obj.health = 50.0;
                obj.position = [1.0, 2.0, 3.0];
            })
            .unwrap();

        let health = handle.with(|obj| obj.health).unwrap();
        assert_eq!(health, 50.0);
    }

    #[test]
    fn test_weak_handles() {
        let config = PoolConfig::for_game_objects("TestPool");
        let pool = ObjectPool::<GameObject>::new(config).unwrap();

        let handle = pool.alloc(GameObject::new(1)).unwrap();
        let weak = handle.downgrade();

        assert!(weak.is_valid());

        let upgraded = weak.upgrade().unwrap();
        let id = upgraded.with(|obj| obj.id).unwrap();
        assert_eq!(id, 1);

        drop(handle);
        drop(upgraded);

        assert!(!weak.is_valid());
        assert!(weak.upgrade().is_none());
    }

    #[test]
    fn test_concurrent_allocations() {
        let config = PoolConfig::for_game_objects("TestPool");
        let pool = ObjectPool::<GameObject>::new(config).unwrap();
        let barrier = Arc::new(Barrier::new(10));

        let handles: Vec<_> = (0..10)
            .map(|i| {
                let pool = Arc::clone(&pool);
                let barrier = Arc::clone(&barrier);
                thread::spawn(move || {
                    barrier.wait();
                    let mut handles = Vec::new();
                    for j in 0..100 {
                        let id = i * 100 + j;
                        handles.push(pool.alloc(GameObject::new(id)).unwrap());
                    }
                    handles
                })
            })
            .collect();

        let all_handles: Vec<_> = handles
            .into_iter()
            .flat_map(|h| h.join().unwrap())
            .collect();

        assert_eq!(all_handles.len(), 1000);
        assert_eq!(pool.len(), 1000);
    }

    #[test]
    fn test_pool_growth() {
        let config = PoolConfigBuilder::new("TestPool")
            .with_initial_capacity(10)
            .with_grow_by(10)
            .build();

        let pool = ObjectPool::<GameObject>::new(config).unwrap();
        assert!(pool.capacity() >= 10);

        // Allocate beyond initial capacity
        let handles: Vec<_> = (0..50)
            .map(|i| pool.alloc(GameObject::new(i)).unwrap())
            .collect();

        assert_eq!(handles.len(), 50);
        assert!(pool.capacity() >= 50);
    }

    #[test]
    fn test_max_capacity() {
        let config = PoolConfigBuilder::new("TestPool")
            .with_initial_capacity(10)
            .with_grow_by(10)
            .with_max_capacity(20)
            .build();

        let pool = ObjectPool::<GameObject>::new(config).unwrap();

        // Allocate up to max
        let mut handles = Vec::new();
        for i in 0..20 {
            handles.push(pool.alloc(GameObject::new(i)).unwrap());
        }

        // Next allocation should fail
        let result = pool.alloc(GameObject::new(999));
        assert!(result.is_err());
    }

    #[test]
    fn test_pool_statistics() {
        let config = PoolConfig::for_game_objects("TestPool");
        let pool = ObjectPool::<GameObject>::new(config).unwrap();

        for i in 0..10 {
            let _handle = pool.alloc(GameObject::new(i)).unwrap();
        }

        let stats = pool.stats().snapshot();
        assert_eq!(stats.total_allocations, 10);
        assert_eq!(stats.active_allocations, 10);
        assert!(stats.bytes_in_use > 0);
        assert!(stats.utilization > 0.0);
    }

    #[test]
    fn test_cache_line_alignment() {
        let config = PoolConfigBuilder::new("TestPool")
            .with_initial_capacity(10)
            .cache_line_aligned()
            .build();

        assert_eq!(config.alignment(), 64);

        let pool = ObjectPool::<GameObject>::new(config).unwrap();
        let handle = pool.alloc(GameObject::new(1)).unwrap();
        assert!(handle.is_valid());
    }

    #[test]
    fn test_custom_alignment() {
        let config = PoolConfigBuilder::new("TestPool")
            .with_initial_capacity(10)
            .with_alignment(128)
            .build();

        assert_eq!(config.alignment(), 128);

        let pool = ObjectPool::<GameObject>::new(config).unwrap();
        let handle = pool.alloc(GameObject::new(1)).unwrap();
        assert!(handle.is_valid());
    }

    #[test]
    fn test_zero_on_alloc() {
        let config = PoolConfigBuilder::new("TestPool")
            .with_initial_capacity(10)
            .zero_on_alloc()
            .build();

        let pool = ObjectPool::<[u8; 64]>::new(config).unwrap();
        let handle = pool.alloc([0xFF; 64]).unwrap();

        // Memory should have been zeroed before value was written
        // (This is hard to test directly, but we can verify config)
        assert!(pool.config().zero_on_alloc);
    }

    #[test]
    fn test_specialized_pools() {
        use crate::memory::specialized::*;

        let obj_pool = create_game_object_pool::<GameObject>("Objects").unwrap();
        let proj_pool = create_projectile_pool::<GameObject>("Projectiles").unwrap();
        let small_pool = create_small_object_pool::<u32>("SmallObjs").unwrap();

        assert_eq!(obj_pool.config().name, "Objects");
        assert_eq!(proj_pool.config().name, "Projectiles");
        assert_eq!(small_pool.config().name, "SmallObjs");
    }

    #[test]
    fn test_memory_usage_tracking() {
        let config = PoolConfig::for_game_objects("TestPool");
        let pool = ObjectPool::<GameObject>::new(config).unwrap();

        let initial_usage = pool.memory_usage();
        assert!(initial_usage > 0);

        let _handles: Vec<_> = (0..100)
            .map(|i| pool.alloc(GameObject::new(i)).unwrap())
            .collect();

        let usage_after = pool.memory_usage();
        assert!(usage_after >= initial_usage);
    }

    #[test]
    fn test_stats_recommendations() {
        let config = PoolConfigBuilder::new("TestPool")
            .with_initial_capacity(10)
            .with_grow_by(5)
            .build();

        let pool = ObjectPool::<GameObject>::new(config).unwrap();

        // Cause multiple growth events
        let _handles: Vec<_> = (0..100)
            .map(|i| pool.alloc(GameObject::new(i)).unwrap())
            .collect();

        let stats = pool.stats().snapshot();
        let recommendations = stats.needs_optimization();

        // Should recommend increasing initial capacity
        assert!(!recommendations.is_empty());
    }

    #[test]
    fn test_drop_cleanup() {
        let config = PoolConfig::for_game_objects("TestPool");
        let pool = ObjectPool::<GameObject>::new(config).unwrap();

        let handles: Vec<_> = (0..10)
            .map(|i| pool.alloc(GameObject::new(i)).unwrap())
            .collect();

        assert_eq!(pool.len(), 10);

        drop(handles);
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn test_preset_configs() {
        let game_cfg = PoolConfig::for_game_objects("Objects");
        assert!(game_cfg.cache_line_aligned);
        assert_eq!(game_cfg.initial_capacity, 512);

        let proj_cfg = PoolConfig::for_projectiles("Projectiles");
        assert_eq!(proj_cfg.initial_capacity, 256);
        assert_eq!(proj_cfg.max_capacity, Some(2048));

        let module_cfg = PoolConfig::for_modules("Modules");
        assert_eq!(module_cfg.initial_capacity, 1024);

        let debug_cfg = PoolConfig::for_debug("Debug");
        assert!(debug_cfg.track_allocations);
        assert!(debug_cfg.zero_on_alloc);
        assert!(debug_cfg.zero_on_free);
    }
}
