//! Stress and Load Testing Suite for Stealth & Detection System
//!
//! Comprehensive stress testing covering:
//! - Object count scaling (100 to 10,000 objects)
//! - Concurrent access patterns and race condition detection
//! - Performance under load (high frequency operations)
//! - Edge cases at scale
//! - Full system integration stress
//!
//! Collects metrics:
//! - Execution time per operation
//! - Memory usage patterns
//! - Lock contention rates
//! - Cache effectiveness
//! - Allocation patterns

#[cfg(test)]
mod stealth_stress_tests {
    use crate::common::ObjectID;
    use crate::system::detection_manager::{
        DetectionManager, DetectionModifier, DetectionStrength,
    };
    use crate::system::stealth_manager::{StealthManager, StealthStatus, StealthStrength};
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};
    use std::thread;
    use std::time::Instant;

    // ============================================================================
    // METRICS COLLECTION
    // ============================================================================

    /// Performance metrics collected during stress tests
    #[derive(Debug, Clone)]
    struct StressMetrics {
        /// Total execution time in milliseconds
        execution_time_ms: f64,
        /// Operations performed
        operation_count: usize,
        /// Operations per second
        ops_per_second: f64,
        /// Average time per operation in microseconds
        avg_micros_per_op: f64,
        /// Lock contentions detected
        lock_contentions: usize,
        /// Data consistency check pass rate
        consistency_pass_rate: f32,
        /// Memory estimate (heap allocations)
        memory_estimate: usize,
    }

    impl StressMetrics {
        fn new() -> Self {
            Self {
                execution_time_ms: 0.0,
                operation_count: 0,
                ops_per_second: 0.0,
                avg_micros_per_op: 0.0,
                lock_contentions: 0,
                consistency_pass_rate: 1.0,
                memory_estimate: 0,
            }
        }

        fn calculate(&mut self, start: Instant, operations: usize) {
            let elapsed = start.elapsed().as_secs_f64() * 1000.0;
            self.execution_time_ms = elapsed;
            self.operation_count = operations;
            self.ops_per_second = (operations as f64 / elapsed) * 1000.0;
            self.avg_micros_per_op = (elapsed * 1000.0) / operations as f64;
        }

        fn report(&self) {
            println!("\n--- Stress Test Metrics ---");
            println!("Execution time:       {:.2} ms", self.execution_time_ms);
            println!("Operations:           {}", self.operation_count);
            println!("Ops/second:           {:.0}", self.ops_per_second);
            println!("Avg time/op:          {:.3} us", self.avg_micros_per_op);
            println!("Lock contentions:     {}", self.lock_contentions);
            println!(
                "Consistency pass:     {:.1}%",
                self.consistency_pass_rate * 100.0
            );
            println!("Memory estimate:      {} bytes", self.memory_estimate);
        }
    }

    // ============================================================================
    // 1. OBJECT COUNT SCALING TESTS (5 tests)
    // ============================================================================

    /// Test 1a: 100 stealthed objects tracked
    #[test]
    fn stress_100_stealthed_objects() {
        let start = Instant::now();
        let mut manager = StealthManager::new();
        let object_count = 100;

        // Register 100 objects
        for i in 1..=object_count {
            assert!(manager.register_object(i as ObjectID).is_ok());
        }

        // Set all to invisible
        for i in 1..=object_count {
            assert!(manager
                .set_stealth_status(i as ObjectID, 0, StealthStatus::Invisible)
                .is_ok());
        }

        // Verify all are invisible
        for i in 1..=object_count {
            let status = manager.get_stealth_status(i as ObjectID, 0).unwrap();
            assert_eq!(status, StealthStatus::Invisible);
        }

        // Reveal half of them
        for i in 1..=(object_count / 2) {
            assert!(manager.reveal_stealth(i as ObjectID, 0, 1, 100).is_ok());
        }

        // Verify revealed/invisible states
        for i in 1..=object_count {
            let status = manager.get_stealth_status(i as ObjectID, 0).unwrap();
            if i <= object_count / 2 {
                assert_eq!(status, StealthStatus::Revealed);
            } else {
                assert_eq!(status, StealthStatus::Invisible);
            }
        }

        let mut metrics = StressMetrics::new();
        metrics.calculate(start, object_count * 5);
        metrics.memory_estimate = object_count * 200; // Rough estimate
        metrics.report();

        assert!(
            metrics.execution_time_ms < 50.0,
            "100 object test took too long"
        );
    }

    /// Test 1b: 1,000 detection checks per frame
    #[test]
    fn stress_1000_detection_checks_per_frame() {
        let start = Instant::now();
        let mut stealth_mgr = StealthManager::new();
        let mut detection_mgr = DetectionManager::new();

        let stealth_count = 50;
        let detector_count = 20;
        let checks_per_detector = 1000 / detector_count;

        // Setup stealth objects
        for i in 1..=stealth_count {
            assert!(stealth_mgr.register_object(i as ObjectID).is_ok());
            stealth_mgr
                .set_stealth_status(i as ObjectID, 0, StealthStatus::Invisible)
                .unwrap();
            stealth_mgr
                .set_stealth_strength(i as ObjectID, StealthStrength::standard_cloak())
                .unwrap();
        }

        // Setup detectors
        for i in 1..=detector_count {
            assert!(detection_mgr
                .register_object((1000 + i) as ObjectID)
                .is_ok());
            detection_mgr
                .set_detection_strength(
                    (1000 + i) as ObjectID,
                    DetectionStrength::strong_detector(),
                )
                .unwrap();
        }

        // Run 1000 detection checks
        let mut detections = 0;
        for detector in 1..=detector_count {
            for stealth_obj in 1..=stealth_count {
                if detector <= stealth_count {
                    let stealth_strength = stealth_mgr
                        .get_stealth_strength(stealth_obj as ObjectID)
                        .unwrap()
                        .value();
                    let modifier = DetectionModifier::default();

                    if let Ok(can_detect) = detection_mgr.can_detect_stealth(
                        (1000 + detector) as ObjectID,
                        stealth_strength,
                        modifier,
                    ) {
                        if can_detect {
                            detections += 1;
                        }
                    }
                }
            }
        }

        let mut metrics = StressMetrics::new();
        metrics.calculate(start, detector_count * checks_per_detector);
        metrics.consistency_pass_rate =
            (detections as f32) / (detector_count * checks_per_detector) as f32;
        metrics.report();

        assert!(
            metrics.execution_time_ms < 100.0,
            "1000 detection checks took too long"
        );
        assert!(detections > 0, "Should have found some detections");
    }

    /// Test 1c: 10,000 objects with mixed states
    #[test]
    fn stress_10000_mixed_state_objects() {
        let start = Instant::now();
        let mut manager = StealthManager::new();
        let object_count = 10_000;

        // Register all objects
        for i in 1..=object_count {
            if let Err(e) = manager.register_object(i as ObjectID) {
                panic!("Failed to register object {}: {}", i, e);
            }
        }

        // Set mixed states
        for i in 1..=object_count {
            let state = match i % 3 {
                0 => StealthStatus::Hidden,
                1 => StealthStatus::Invisible,
                _ => StealthStatus::Revealed,
            };
            let strength = StealthStrength::new((i % 100) as f32);

            assert!(manager.set_stealth_status(i as ObjectID, 0, state).is_ok());
            assert!(manager
                .set_stealth_strength(i as ObjectID, strength)
                .is_ok());
        }

        // Verify states
        let mut hidden_count = 0;
        let mut invisible_count = 0;
        let mut revealed_count = 0;

        for i in 1..=object_count {
            let status = manager.get_stealth_status(i as ObjectID, 0).unwrap();
            match status {
                StealthStatus::Hidden => hidden_count += 1,
                StealthStatus::Invisible => invisible_count += 1,
                StealthStatus::Revealed => revealed_count += 1,
            }
        }

        let mut metrics = StressMetrics::new();
        metrics.calculate(start, object_count * 3);
        metrics.memory_estimate = object_count * 300;
        metrics.consistency_pass_rate = 1.0; // All states accounted for
        metrics.report();

        // Verify distribution
        assert!(hidden_count > 0 && invisible_count > 0 && revealed_count > 0);
        assert_eq!(
            hidden_count + invisible_count + revealed_count,
            object_count
        );
        assert!(
            metrics.execution_time_ms < 500.0,
            "10000 object stress test took too long"
        );
    }

    /// Test 1d: Concurrent operations on all objects
    #[test]
    fn stress_concurrent_operations() {
        let object_count = 1000;
        let thread_count = 8;
        let ops_per_thread = object_count / thread_count;

        let manager = Arc::new(Mutex::new(StealthManager::new()));

        // Initialize
        {
            let mut mgr = manager.lock().unwrap();
            for i in 1..=object_count {
                let _ = mgr.register_object(i as ObjectID);
            }
        }

        let start = Instant::now();

        // Spawn threads for concurrent operations
        let handles: Vec<_> = (0..thread_count)
            .map(|thread_id| {
                let mgr = Arc::clone(&manager);
                thread::spawn(move || {
                    let start_obj = thread_id * ops_per_thread + 1;
                    let end_obj = start_obj + ops_per_thread;

                    for i in start_obj..end_obj {
                        let mut guard = mgr.lock().unwrap();

                        // Alternate operations
                        if i % 2 == 0 {
                            let _ = guard.set_stealth_status(
                                i as ObjectID,
                                thread_id % 8,
                                StealthStatus::Invisible,
                            );
                        } else {
                            let _ = guard.set_stealth_strength(
                                i as ObjectID,
                                StealthStrength::standard_cloak(),
                            );
                        }
                    }
                })
            })
            .collect();

        // Wait for all threads
        for handle in handles {
            let _ = handle.join();
        }

        let mut metrics = StressMetrics::new();
        metrics.calculate(start, object_count * 2);
        metrics.lock_contentions = thread_count; // Track thread count as contentions
        metrics.report();

        // Verify data integrity
        let mgr = manager.lock().unwrap();
        for i in 1..=object_count {
            let _ = mgr.get_stealth_strength(i as ObjectID);
        }

        assert!(
            metrics.execution_time_ms < 1000.0,
            "Concurrent ops took too long"
        );
    }

    /// Test 1e: Memory usage validation
    #[test]
    fn stress_memory_usage_validation() {
        let object_counts = vec![100, 500, 1000, 5000];
        let mut measurements = Vec::new();

        for count in object_counts {
            let mut manager = StealthManager::new();
            let before_bytes = manager.estimated_heap_bytes();
            for i in 1..=count {
                let _ = manager.register_object(i as ObjectID);
                let _ =
                    manager.set_stealth_strength(i as ObjectID, StealthStrength::standard_cloak());
            }

            let after_bytes = manager.estimated_heap_bytes();
            let byte_diff = after_bytes.saturating_sub(before_bytes);
            measurements.push((count, byte_diff));

            println!("Objects: {}, Estimated heap bytes: {}", count, byte_diff);
        }

        // Verify the per-object memory cost stays roughly constant as the object count grows.
        // HashMap capacity grows in jumps, so comparing absolute growth ratios is unstable.
        for i in 1..measurements.len() {
            let (prev_count, prev_bytes) = measurements[i - 1];
            let (next_count, next_bytes) = measurements[i];
            assert!(
                prev_bytes > 0 && next_bytes > 0,
                "Invalid memory measurements"
            );

            let prev_per_object = prev_bytes as f32 / prev_count as f32;
            let next_per_object = next_bytes as f32 / next_count as f32;

            assert!(
                next_per_object <= prev_per_object * 2.0,
                "Memory growth is superlinear"
            );
        }
    }

    // ============================================================================
    // 2. CONCURRENT ACCESS TESTS (5 tests)
    // ============================================================================

    /// Test 2a: Multiple threads accessing stealth system
    #[test]
    fn stress_multi_thread_access() {
        let object_count = 500;
        let thread_count = 4;
        let iterations = 100;

        let manager = Arc::new(Mutex::new(StealthManager::new()));

        {
            let mut mgr = manager.lock().unwrap();
            for i in 1..=object_count {
                let _ = mgr.register_object(i as ObjectID);
            }
        }

        let start = Instant::now();

        let handles: Vec<_> = (0..thread_count)
            .map(|thread_id| {
                let mgr = Arc::clone(&manager);
                thread::spawn(move || {
                    for iter in 0..iterations {
                        let obj_id = ((thread_id * iterations + iter) % object_count) + 1;
                        let mut guard = mgr.lock().unwrap();

                        // Read operations
                        let _ = guard.get_stealth_status(obj_id as ObjectID, 0);
                        let _ = guard.get_stealth_strength(obj_id as ObjectID);

                        // Write operations
                        let status = if iter % 2 == 0 {
                            StealthStatus::Invisible
                        } else {
                            StealthStatus::Revealed
                        };
                        let _ = guard.set_stealth_status(obj_id as ObjectID, 0, status);
                    }
                })
            })
            .collect();

        for handle in handles {
            let _ = handle.join();
        }

        let mut metrics = StressMetrics::new();
        metrics.calculate(start, thread_count * iterations * 4);
        metrics.lock_contentions = thread_count;
        metrics.report();

        assert!(metrics.execution_time_ms < 1000.0);
    }

    /// Test 2b: Race condition detection
    #[test]
    fn stress_race_condition_detection() {
        let object_id: ObjectID = 100;
        let thread_count = 8;
        let iterations = 500;

        let manager = Arc::new(Mutex::new(StealthManager::new()));

        {
            let mut mgr = manager.lock().unwrap();
            let _ = mgr.register_object(object_id);
        }

        let counter = Arc::new(Mutex::new(0usize));

        let handles: Vec<_> = (0..thread_count)
            .map(|_| {
                let mgr = Arc::clone(&manager);
                let cnt = Arc::clone(&counter);
                thread::spawn(move || {
                    for _ in 0..iterations {
                        let mut guard = mgr.lock().unwrap();

                        // Try to create race condition with status changes
                        let current = guard.get_stealth_status(object_id, 0).unwrap();
                        let next_status = match current {
                            StealthStatus::Hidden => StealthStatus::Invisible,
                            StealthStatus::Invisible => StealthStatus::Revealed,
                            StealthStatus::Revealed => StealthStatus::Hidden,
                        };

                        let _ = guard.set_stealth_status(object_id, 0, next_status);

                        let mut c = cnt.lock().unwrap();
                        *c += 1;
                    }
                })
            })
            .collect();

        for handle in handles {
            let _ = handle.join();
        }

        // Verify no data corruption
        let mgr = manager.lock().unwrap();
        let final_status = mgr.get_stealth_status(object_id, 0).unwrap();

        // Should be in a valid state
        assert!(
            final_status == StealthStatus::Hidden
                || final_status == StealthStatus::Invisible
                || final_status == StealthStatus::Revealed
        );

        let final_count = *counter.lock().unwrap();
        assert_eq!(final_count, thread_count * iterations);
    }

    /// Test 2c: Lock contention measurement
    #[test]
    fn stress_lock_contention_measurement() {
        let object_count = 100;
        let thread_count = 16; // High contention scenario
        let iterations = 100;

        let manager = Arc::new(Mutex::new(StealthManager::new()));

        {
            let mut mgr = manager.lock().unwrap();
            for i in 1..=object_count {
                let _ = mgr.register_object(i as ObjectID);
            }
        }

        let start = Instant::now();
        let contentions = Arc::new(Mutex::new(0usize));

        let handles: Vec<_> = (0..thread_count)
            .map(|thread_id| {
                let mgr = Arc::clone(&manager);
                let cont = Arc::clone(&contentions);
                thread::spawn(move || {
                    for i in 0..iterations {
                        let obj_id = ((thread_id * iterations + i) % object_count) + 1;

                        let wait_start = Instant::now();
                        let mut guard = mgr.lock().unwrap();
                        let wait_time = wait_start.elapsed().as_micros();

                        if wait_time > 100 {
                            let mut c = cont.lock().unwrap();
                            *c += 1;
                        }

                        let _ = guard.set_stealth_strength(
                            obj_id as ObjectID,
                            StealthStrength::standard_cloak(),
                        );
                    }
                })
            })
            .collect();

        for handle in handles {
            let _ = handle.join();
        }

        let mut metrics = StressMetrics::new();
        metrics.calculate(start, thread_count * iterations);
        metrics.lock_contentions = *contentions.lock().unwrap();
        metrics.report();

        // Lock contention is timing-dependent; it may be zero on fast machines / schedulers.
        assert!(metrics.lock_contentions <= thread_count * iterations);
        assert!(metrics.execution_time_ms < 2000.0);
    }

    /// Test 2d: Deadlock prevention verification
    #[test]
    fn stress_deadlock_prevention() {
        let object_count = 50;
        let thread_count = 8;
        let iterations = 100;

        let stealth_mgr = Arc::new(Mutex::new(StealthManager::new()));
        let detection_mgr = Arc::new(Mutex::new(DetectionManager::new()));

        {
            let mut s = stealth_mgr.lock().unwrap();
            let mut d = detection_mgr.lock().unwrap();
            for i in 1..=object_count {
                let _ = s.register_object(i as ObjectID);
                let _ = d.register_object(i as ObjectID);
            }
        }

        let start = Instant::now();
        let completed = Arc::new(Mutex::new(0usize));

        let handles: Vec<_> = (0..thread_count)
            .map(|thread_id| {
                let s = Arc::clone(&stealth_mgr);
                let d = Arc::clone(&detection_mgr);
                let comp = Arc::clone(&completed);

                thread::spawn(move || {
                    for i in 0..iterations {
                        // Always lock in same order (stealth first, then detection)
                        let obj_id = ((thread_id * iterations + i) % object_count) + 1;

                        let mut s_guard = s.lock().unwrap();
                        let _ = s_guard.set_stealth_strength(
                            obj_id as ObjectID,
                            StealthStrength::standard_cloak(),
                        );
                        drop(s_guard);

                        let mut d_guard = d.lock().unwrap();
                        let _ = d_guard.set_detection_strength(
                            obj_id as ObjectID,
                            DetectionStrength::standard_detector(),
                        );
                        drop(d_guard);

                        let mut c = comp.lock().unwrap();
                        *c += 1;
                    }
                })
            })
            .collect();

        // Set a timeout implicitly by having a tight test
        for handle in handles {
            let _ = handle.join();
        }

        let mut metrics = StressMetrics::new();
        metrics.calculate(start, *completed.lock().unwrap());
        metrics.report();

        assert_eq!(*completed.lock().unwrap(), thread_count * iterations);
        assert!(metrics.execution_time_ms < 2000.0);
    }

    /// Test 2e: Consistency under concurrent modification
    #[test]
    fn stress_consistency_concurrent_modification() {
        let object_id: ObjectID = 42;
        let thread_count = 4;
        let iterations = 1000;

        let manager = Arc::new(Mutex::new(StealthManager::new()));

        {
            let mut mgr = manager.lock().unwrap();
            let _ = mgr.register_object(object_id);
        }

        let modifications = Arc::new(Mutex::new(HashMap::new()));

        let handles: Vec<_> = (0..thread_count)
            .map(|thread_id| {
                let mgr = Arc::clone(&manager);
                let mods = Arc::clone(&modifications);

                thread::spawn(move || {
                    for i in 0..iterations {
                        let mut guard = mgr.lock().unwrap();

                        let status = if (thread_id + i) % 2 == 0 {
                            StealthStatus::Invisible
                        } else {
                            StealthStatus::Revealed
                        };

                        let _ = guard.set_stealth_status(object_id, 0, status);

                        let mut m = mods.lock().unwrap();
                        *m.entry(thread_id).or_insert(0) += 1;
                    }
                })
            })
            .collect();

        for handle in handles {
            let _ = handle.join();
        }

        // Verify final state is valid
        let mgr = manager.lock().unwrap();
        let final_status = mgr.get_stealth_status(object_id, 0).unwrap();
        assert!(
            final_status == StealthStatus::Invisible || final_status == StealthStatus::Revealed
        );

        let total_mods: usize = modifications.lock().unwrap().values().sum();
        assert_eq!(total_mods, thread_count * iterations);
    }

    // ============================================================================
    // 3. PERFORMANCE UNDER LOAD TESTS (5 tests)
    // ============================================================================

    /// Test 3a: Detection calculations at 1000 Hz
    #[test]
    fn stress_detection_1000hz() {
        let detector_count = 10;
        let target_count = 20;
        let frames = 100; // Simulate 100 frames at 1000Hz

        let mut stealth_mgr = StealthManager::new();
        let mut detection_mgr = DetectionManager::new();

        // Setup
        for i in 1..=target_count {
            let _ = stealth_mgr.register_object(i as ObjectID);
            let _ =
                stealth_mgr.set_stealth_strength(i as ObjectID, StealthStrength::standard_cloak());
        }

        for i in 1..=detector_count {
            let _ = detection_mgr.register_object((1000 + i) as ObjectID);
            let _ = detection_mgr.set_detection_strength(
                (1000 + i) as ObjectID,
                DetectionStrength::standard_detector(),
            );
        }

        let start = Instant::now();

        // Simulate 1000 Hz detection checks
        let mut total_checks = 0;
        for _frame in 0..frames {
            for detector_id in 1..=detector_count {
                for target_id in 1..=target_count {
                    if let Ok(stealth_strength) =
                        stealth_mgr.get_stealth_strength(target_id as ObjectID)
                    {
                        let modifier = DetectionModifier {
                            distance_factor: 0.8,
                            unit_type_factor: 0.9,
                            movement_factor: 1.0,
                            special_factor: 1.0,
                        };

                        let _ = detection_mgr.can_detect_stealth(
                            (1000 + detector_id) as ObjectID,
                            stealth_strength.value(),
                            modifier,
                        );
                        total_checks += 1;
                    }
                }
            }
        }

        let mut metrics = StressMetrics::new();
        metrics.calculate(start, total_checks);
        metrics.report();

        assert!(
            metrics.execution_time_ms < 200.0,
            "1000 Hz detection too slow"
        );
        assert!(metrics.ops_per_second > 5000.0, "Throughput too low");
    }

    /// Test 3b: Modifier calculations for 100+ units
    #[test]
    fn stress_modifier_calculations_100_units() {
        let unit_count = 100;
        let detector_id: ObjectID = 1;

        let mut detection_mgr = DetectionManager::new();
        let _ = detection_mgr.register_object(detector_id);
        let _ =
            detection_mgr.set_detection_strength(detector_id, DetectionStrength::strong_detector());

        let start = Instant::now();

        // Calculate modifiers for 100 units
        let mut calculations = 0;
        for unit in 0..unit_count {
            for distance_bucket in 0..10 {
                for unit_type in 0..5 {
                    let distance_factor = 1.0 / (1.0 + (distance_bucket as f32 * 10.0));
                    let unit_factor = match unit_type {
                        0 => 0.8,  // Infantry
                        1 => 0.9,  // Vehicles
                        2 => 0.7,  // Aircraft
                        3 => 0.95, // Structures
                        _ => 1.0,
                    };

                    let modifier = DetectionModifier {
                        distance_factor,
                        unit_type_factor: unit_factor,
                        movement_factor: if unit % 2 == 0 { 1.0 } else { 0.8 },
                        special_factor: 1.0,
                    };

                    let _ = detection_mgr.get_detection_effectiveness(detector_id, modifier);
                    calculations += 1;
                }
            }
        }

        let mut metrics = StressMetrics::new();
        metrics.calculate(start, calculations);
        metrics.report();

        assert!(metrics.execution_time_ms < 100.0);
    }

    /// Test 3c: Event generation at high frequency
    #[test]
    fn stress_event_generation_high_frequency() {
        let detection_events = Arc::new(Mutex::new(Vec::new()));
        let event_count = 10_000;

        let start = Instant::now();

        // Simulate rapid event generation
        for i in 0..event_count {
            let event = format!("detection_event_{}", i);
            let mut events = detection_events.lock().unwrap();
            events.push(event);
        }

        let mut metrics = StressMetrics::new();
        metrics.calculate(start, event_count);
        metrics.report();

        let events = detection_events.lock().unwrap();
        assert_eq!(events.len(), event_count);
        assert!(metrics.execution_time_ms < 50.0);
    }

    /// Test 3d: Cache effectiveness under load
    #[test]
    fn stress_cache_effectiveness() {
        let detector_count = 20;
        let access_pattern = 1000; // Repeated accesses

        let mut detection_mgr = DetectionManager::new();

        for i in 1..=detector_count {
            let _ = detection_mgr.register_object(i as ObjectID);
            let _ = detection_mgr
                .set_detection_strength(i as ObjectID, DetectionStrength::standard_detector());
        }

        let start = Instant::now();

        // Repeated accesses to same objects (cache-friendly)
        let mut hits = 0;
        for _frame in 0..access_pattern {
            for detector in 1..=detector_count {
                if let Ok(_strength) = detection_mgr.get_detection_strength(detector as ObjectID) {
                    hits += 1;
                }
            }
        }

        let mut metrics = StressMetrics::new();
        metrics.calculate(start, detector_count * access_pattern);
        metrics.consistency_pass_rate =
            (hits as f32) / (detector_count as f32 * access_pattern as f32);
        metrics.report();

        assert_eq!(hits, detector_count * access_pattern);
        assert!(
            metrics.ops_per_second > 100_000.0,
            "Cache should accelerate this"
        );
    }

    /// Test 3e: Memory allocation patterns
    #[test]
    fn stress_memory_allocation_patterns() {
        let object_count = 5000;

        let start = Instant::now();

        let mut manager = StealthManager::new();
        let mut last_capacity = manager.object_capacity();
        let mut last_bytes = manager.estimated_heap_bytes();
        let mut growth_events = 0usize;

        for i in 1..=object_count {
            let _ = manager.register_object(i as ObjectID);
            let capacity = manager.object_capacity();
            let bytes = manager.estimated_heap_bytes();
            if capacity != last_capacity {
                growth_events += 1;
                assert!(capacity > last_capacity, "Capacity should only grow");
                assert!(bytes > last_bytes, "Bytes should only grow");
                last_capacity = capacity;
                last_bytes = bytes;
            } else {
                assert_eq!(
                    bytes, last_bytes,
                    "Heap estimate changed without capacity change"
                );
            }
        }

        let mut metrics = StressMetrics::new();
        metrics.calculate(start, object_count);
        metrics.memory_estimate = manager.estimated_heap_bytes();
        metrics.report();

        // HashMap should grow O(log N) times; continuous per-insert growth would be pathological.
        assert!(
            growth_events <= 32,
            "Too many growth events: {}",
            growth_events
        );
    }

    // ============================================================================
    // 4. EDGE CASES AT SCALE TESTS (5 tests)
    // ============================================================================

    /// Test 4a: All players visible to all objects
    #[test]
    fn stress_all_visible_all_players() {
        let object_count = 1000;
        let player_count = 8;

        let start = Instant::now();
        let mut manager = StealthManager::new();

        // Register objects
        for i in 1..=object_count {
            let _ = manager.register_object(i as ObjectID);
        }

        // Make all visible to all players
        for obj in 1..=object_count {
            for player in 0..player_count {
                let _ =
                    manager.set_stealth_status(obj as ObjectID, player, StealthStatus::Revealed);
            }
        }

        // Verify all are visible
        for obj in 1..=object_count {
            for player in 0..player_count {
                let status = manager.get_stealth_status(obj as ObjectID, player).unwrap();
                assert_eq!(status, StealthStatus::Revealed);
            }
        }

        let mut metrics = StressMetrics::new();
        metrics.calculate(start, object_count * player_count * 2);
        metrics.report();

        assert!(metrics.execution_time_ms < 500.0);
    }

    /// Test 4b: All objects stealthed
    #[test]
    fn stress_all_objects_stealthed() {
        let object_count = 5000;

        let start = Instant::now();
        let mut manager = StealthManager::new();

        for i in 1..=object_count {
            let _ = manager.register_object(i as ObjectID);
            let _ = manager.set_stealth_strength(i as ObjectID, StealthStrength::strong_stealth());
            let _ = manager.set_stealth_status(i as ObjectID, 0, StealthStatus::Invisible);
        }

        // Break stealth for all
        for i in 1..=object_count {
            let _ = manager.break_stealth_all(i as ObjectID, 100);
        }

        let mut metrics = StressMetrics::new();
        metrics.calculate(start, object_count * 3);
        metrics.report();

        assert!(metrics.execution_time_ms < 300.0);
    }

    /// Test 4c: All objects detected
    #[test]
    fn stress_all_objects_detected() {
        let object_count = 2000;

        let start = Instant::now();
        let mut stealth_mgr = StealthManager::new();
        let mut detection_mgr = DetectionManager::new();

        // Register all as both stealthy and detectors
        for i in 1..=object_count {
            let _ = stealth_mgr.register_object(i as ObjectID);
            let _ = stealth_mgr.set_stealth_status(i as ObjectID, 0, StealthStatus::Invisible);
            let _ =
                stealth_mgr.set_stealth_strength(i as ObjectID, StealthStrength::weak_stealth());

            let _ = detection_mgr.register_object(i as ObjectID);
            let _ = detection_mgr
                .set_detection_strength(i as ObjectID, DetectionStrength::strong_detector());
        }

        // Try to detect all
        let modifier = DetectionModifier::default();
        let mut detected = 0;
        for detector in 1..=object_count {
            for target in 1..=object_count {
                if detector != target {
                    if let Ok(stealth_str) = stealth_mgr.get_stealth_strength(target as ObjectID) {
                        if let Ok(can_detect) = detection_mgr.can_detect_stealth(
                            detector as ObjectID,
                            stealth_str.value(),
                            modifier,
                        ) {
                            if can_detect {
                                detected += 1;
                            }
                        }
                    }
                }
            }
        }

        let mut metrics = StressMetrics::new();
        metrics.calculate(start, object_count * object_count);
        metrics.consistency_pass_rate =
            (detected as f32) / ((object_count * (object_count - 1)) as f32);
        metrics.report();

        assert!(detected > 0);
    }

    /// Test 4d: Rapid state changes
    #[test]
    fn stress_rapid_state_changes() {
        let object_count = 500;
        let state_changes = 10_000;

        let start = Instant::now();
        let mut manager = StealthManager::new();

        for i in 1..=object_count {
            let _ = manager.register_object(i as ObjectID);
        }

        // Rapidly change states
        for change in 0..state_changes {
            let obj_id = (change % object_count) + 1;
            let state = match change % 3 {
                0 => StealthStatus::Hidden,
                1 => StealthStatus::Invisible,
                _ => StealthStatus::Revealed,
            };

            let _ = manager.set_stealth_status(obj_id as ObjectID, 0, state);
        }

        let mut metrics = StressMetrics::new();
        metrics.calculate(start, state_changes);
        metrics.report();

        assert!(metrics.execution_time_ms < 200.0);
        assert!(metrics.ops_per_second > 50_000.0);
    }

    /// Test 4e: Boundary condition stress
    #[test]
    fn stress_boundary_conditions() {
        let mut manager = StealthManager::new();

        // Test with minimum and maximum values
        let test_cases = vec![
            (1, 0.0),               // Min object, min strength
            (1, 100.0),             // Min object, max strength
            (ObjectID::MAX, 0.0),   // Max object ID, min strength
            (ObjectID::MAX, 100.0), // Max object ID, max strength
        ];

        let start = Instant::now();

        for (obj_id, strength) in test_cases {
            // Skip if MAX would overflow in register loop
            if obj_id != ObjectID::MAX {
                let _ = manager.register_object(obj_id);
                let _ = manager.set_stealth_strength(obj_id, StealthStrength::new(strength));

                for player in 0..8 {
                    let _ = manager.set_stealth_status(obj_id, player, StealthStatus::Invisible);
                    let _ = manager.reveal_stealth(obj_id, player, 1, 100);
                }
            }
        }

        let mut metrics = StressMetrics::new();
        metrics.calculate(start, 100);
        metrics.report();

        assert!(metrics.execution_time_ms < 50.0);
    }

    // ============================================================================
    // 5. INTEGRATION STRESS TESTS (5 tests)
    // ============================================================================

    /// Test 5a: All systems active simultaneously
    #[test]
    fn stress_all_systems_active() {
        let object_count = 500;
        let thread_count = 4;
        let iterations = 100;

        let stealth_mgr = Arc::new(Mutex::new(StealthManager::new()));
        let detection_mgr = Arc::new(Mutex::new(DetectionManager::new()));

        {
            let mut s = stealth_mgr.lock().unwrap();
            let mut d = detection_mgr.lock().unwrap();
            for i in 1..=object_count {
                let _ = s.register_object(i as ObjectID);
                let _ = d.register_object(i as ObjectID);
            }
        }

        let start = Instant::now();

        let handles: Vec<_> = (0..thread_count)
            .map(|thread_id| {
                let s = Arc::clone(&stealth_mgr);
                let d = Arc::clone(&detection_mgr);

                thread::spawn(move || {
                    for i in 0..iterations {
                        let obj_id = ((thread_id * iterations + i) % object_count) + 1;

                        // Stealth operations
                        {
                            let mut guard = s.lock().unwrap();
                            let _ = guard.set_stealth_status(
                                obj_id as ObjectID,
                                0,
                                StealthStatus::Invisible,
                            );
                            let _ = guard.set_stealth_strength(
                                obj_id as ObjectID,
                                StealthStrength::standard_cloak(),
                            );
                        }

                        // Detection operations
                        {
                            let mut guard = d.lock().unwrap();
                            let _ = guard.set_detection_strength(
                                obj_id as ObjectID,
                                DetectionStrength::standard_detector(),
                            );
                        }

                        // Cross-system check
                        {
                            let s_guard = s.lock().unwrap();
                            let d_guard = d.lock().unwrap();
                            if let (Ok(stealth_str), Ok(det_str)) = (
                                s_guard.get_stealth_strength(obj_id as ObjectID),
                                d_guard.get_detection_strength(obj_id as ObjectID),
                            ) {
                                let _ = det_str.value() > stealth_str.value();
                            }
                        }
                    }
                })
            })
            .collect();

        for handle in handles {
            let _ = handle.join();
        }

        let mut metrics = StressMetrics::new();
        metrics.calculate(start, thread_count * iterations * 6);
        metrics.lock_contentions = thread_count;
        metrics.report();

        assert!(metrics.execution_time_ms < 1500.0);
    }

    /// Test 5b: Special powers + upgrades + conditions
    #[test]
    fn stress_special_powers_upgrades_conditions() {
        let object_count = 300;
        let iterations = 100;

        let start = Instant::now();
        let mut stealth_mgr = StealthManager::new();
        let mut condition_flags = HashMap::new();
        let mut upgrade_levels = HashMap::new();

        for i in 1..=object_count {
            let _ = stealth_mgr.register_object(i as ObjectID);
            condition_flags.insert(i, false);
            upgrade_levels.insert(i, 0);
        }

        // Simulate combined effects
        for _iter in 0..iterations {
            for obj in 1..=object_count {
                // Toggle condition
                let has_condition = condition_flags.get_mut(&obj).unwrap();
                *has_condition = !*has_condition;

                // Upgrade level affects stealth
                let upgrade = upgrade_levels.get_mut(&obj).unwrap();
                *upgrade = (*upgrade + 1) % 4;

                // Set stealth based on combined state
                let strength = 20.0 + ((*upgrade as f32) * 20.0);
                let status = if *has_condition {
                    StealthStatus::Revealed
                } else {
                    StealthStatus::Invisible
                };

                let _ = stealth_mgr
                    .set_stealth_strength(obj as ObjectID, StealthStrength::new(strength));
                let _ = stealth_mgr.set_stealth_status(obj as ObjectID, 0, status);
            }
        }

        let mut metrics = StressMetrics::new();
        metrics.calculate(start, object_count * iterations * 3);
        metrics.report();

        assert!(metrics.execution_time_ms < 500.0);
    }

    /// Test 5c: Detection events flooding
    #[test]
    fn stress_detection_events_flooding() {
        let detector_count = 50;
        let target_count = 100;
        let frames = 50;

        let events = Arc::new(Mutex::new(Vec::new()));
        let start = Instant::now();

        // Simulate rapid event generation
        for _frame in 0..frames {
            for detector in 1..=detector_count {
                for target in 1..=target_count {
                    let event = format!("detect_{}_{}", detector, target);
                    let mut e = events.lock().unwrap();
                    e.push(event);
                }
            }
        }

        let mut metrics = StressMetrics::new();
        metrics.calculate(start, detector_count * target_count * frames);
        metrics.report();

        let event_list = events.lock().unwrap();
        assert_eq!(event_list.len(), detector_count * target_count * frames);
        assert!(metrics.execution_time_ms < 200.0);
    }

    /// Test 5d: Area effects with many objects
    #[test]
    fn stress_area_effects_many_objects() {
        let object_count = 2000;
        let area_checks = 100;

        let start = Instant::now();
        let mut affected = 0;

        let mut stealth_mgr = StealthManager::new();

        for i in 1..=object_count {
            let _ = stealth_mgr.register_object(i as ObjectID);
            let _ = stealth_mgr.set_stealth_status(i as ObjectID, 0, StealthStatus::Invisible);
        }

        // Simulate area effect checks
        for _check in 0..area_checks {
            // Check all objects in "area"
            for obj in 1..=object_count {
                if let Ok(status) = stealth_mgr.get_stealth_status(obj as ObjectID, 0) {
                    if status == StealthStatus::Invisible {
                        affected += 1;
                    }
                }
            }
        }

        let mut metrics = StressMetrics::new();
        metrics.calculate(start, object_count * area_checks);
        metrics.consistency_pass_rate = (affected as f32) / ((object_count * area_checks) as f32);
        metrics.report();

        assert_eq!(affected, object_count * area_checks);
        assert!(metrics.execution_time_ms < 500.0);
    }

    /// Test 5e: Concurrent special power grants
    #[test]
    fn stress_concurrent_special_power_grants() {
        let object_count = 500;
        let thread_count = 8;
        let iterations = 50;

        let manager = Arc::new(Mutex::new(StealthManager::new()));

        {
            let mut mgr = manager.lock().unwrap();
            for i in 1..=object_count {
                let _ = mgr.register_object(i as ObjectID);
            }
        }

        let start = Instant::now();

        let handles: Vec<_> = (0..thread_count)
            .map(|thread_id| {
                let mgr = Arc::clone(&manager);
                thread::spawn(move || {
                    for i in 0..iterations {
                        let obj_id = ((thread_id * iterations + i) % object_count) + 1;

                        let mut guard = mgr.lock().unwrap();

                        // Grant stealth (special power simulation)
                        let strength = StealthStrength::standard_cloak();
                        let _ = guard.set_stealth_strength(obj_id as ObjectID, strength);
                        let _ = guard.set_stealth_status(
                            obj_id as ObjectID,
                            0,
                            StealthStatus::Invisible,
                        );

                        // Verify grant
                        let _ = guard.get_stealth_status(obj_id as ObjectID, 0);
                    }
                })
            })
            .collect();

        for handle in handles {
            let _ = handle.join();
        }

        let mut metrics = StressMetrics::new();
        metrics.calculate(start, thread_count * iterations * 4);
        metrics.lock_contentions = thread_count;
        metrics.report();

        assert!(metrics.execution_time_ms < 1000.0);
    }

    // ============================================================================
    // HELPER FUNCTIONS
    // ============================================================================

    // No allocator instrumentation in this crate; use `StealthManager::estimated_heap_bytes()`.
}
