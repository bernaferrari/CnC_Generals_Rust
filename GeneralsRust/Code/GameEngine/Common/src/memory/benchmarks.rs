//! Benchmarks for Memory Pooling System
//!
//! Compares pool allocator performance against:
//! - Standard Rust heap allocation (Box)
//! - Vec with pre-allocation
//! - typed-arena crate
//!
//! Run with: `cargo bench --features benchmark`

#[cfg(test)]
pub mod benchmarks {
    use crate::memory::*;
    use std::time::{Duration, Instant};

    #[derive(Clone)]
    struct BenchObject {
        id: u64,
        position: [f32; 3],
        velocity: [f32; 3],
        health: f32,
        data: [u8; 32],
    }

    impl BenchObject {
        fn new(id: u64) -> Self {
            Self {
                id,
                position: [0.0, 0.0, 0.0],
                velocity: [1.0, 1.0, 1.0],
                health: 100.0,
                data: [0; 32],
            }
        }
    }

    /// Benchmark pool allocation vs Box allocation.
    pub fn bench_allocation_comparison(iterations: usize) -> BenchmarkResults {
        println!("\n=== Allocation Benchmark ({} iterations) ===", iterations);

        // Pool allocation
        let pool_time = {
            let config = PoolConfig::for_game_objects("BenchPool");
            let pool = ObjectPool::<BenchObject>::new(config).unwrap();

            let start = Instant::now();
            let handles: Vec<_> = (0..iterations)
                .map(|i| pool.alloc(BenchObject::new(i as u64)).unwrap())
                .collect();
            let elapsed = start.elapsed();

            // Keep handles alive
            std::hint::black_box(handles);

            elapsed
        };

        // Box allocation
        let box_time = {
            let start = Instant::now();
            let boxes: Vec<_> = (0..iterations)
                .map(|i| Box::new(BenchObject::new(i as u64)))
                .collect();
            let elapsed = start.elapsed();

            std::hint::black_box(boxes);

            elapsed
        };

        // Vec with capacity
        let vec_time = {
            let mut vec = Vec::with_capacity(iterations);
            let start = Instant::now();
            for i in 0..iterations {
                vec.push(BenchObject::new(i as u64));
            }
            let elapsed = start.elapsed();

            std::hint::black_box(vec);

            elapsed
        };

        BenchmarkResults {
            pool_time,
            box_time,
            vec_time,
            iterations,
        }
    }

    /// Benchmark mixed allocation/deallocation patterns.
    pub fn bench_mixed_operations(iterations: usize) -> Duration {
        let config = PoolConfig::for_game_objects("BenchPool");
        let pool = ObjectPool::<BenchObject>::new(config).unwrap();

        let start = Instant::now();

        let mut handles = Vec::new();

        // Alternate between allocating and freeing
        for i in 0..iterations {
            handles.push(pool.alloc(BenchObject::new(i as u64)).unwrap());

            if i % 10 == 0 && !handles.is_empty() {
                handles.remove(0);
            }
        }

        let elapsed = start.elapsed();
        std::hint::black_box(handles);
        elapsed
    }

    /// Benchmark concurrent allocations.
    pub fn bench_concurrent_allocations(threads: usize, allocs_per_thread: usize) -> Duration {
        use std::sync::Arc;
        use std::thread;

        let config = PoolConfig::for_game_objects("BenchPool");
        let pool = ObjectPool::<BenchObject>::new(config).unwrap();

        let start = Instant::now();

        let handles: Vec<_> = (0..threads)
            .map(|t| {
                let pool = Arc::clone(&pool);
                thread::spawn(move || {
                    let mut handles = Vec::new();
                    for i in 0..allocs_per_thread {
                        let id = (t * allocs_per_thread + i) as u64;
                        handles.push(pool.alloc(BenchObject::new(id)).unwrap());
                    }
                    handles
                })
            })
            .collect();

        let all_handles: Vec<_> = handles
            .into_iter()
            .flat_map(|h| h.join().unwrap())
            .collect();

        let elapsed = start.elapsed();
        std::hint::black_box(all_handles);
        elapsed
    }

    /// Benchmark cache locality effects.
    pub fn bench_cache_locality(iterations: usize) -> CacheLocalityResults {
        // Non-aligned pool
        let config_unaligned = PoolConfigBuilder::new("Unaligned")
            .with_initial_capacity(iterations)
            .build();
        let pool_unaligned = ObjectPool::<BenchObject>::new(config_unaligned).unwrap();

        // Cache-line aligned pool
        let config_aligned = PoolConfigBuilder::new("Aligned")
            .with_initial_capacity(iterations)
            .cache_line_aligned()
            .build();
        let pool_aligned = ObjectPool::<BenchObject>::new(config_aligned).unwrap();

        // Allocate objects
        let handles_unaligned: Vec<_> = (0..iterations)
            .map(|i| pool_unaligned.alloc(BenchObject::new(i as u64)).unwrap())
            .collect();

        let handles_aligned: Vec<_> = (0..iterations)
            .map(|i| pool_aligned.alloc(BenchObject::new(i as u64)).unwrap())
            .collect();

        // Sequential access benchmark
        let unaligned_time = {
            let start = Instant::now();
            for handle in &handles_unaligned {
                handle
                    .with(|obj| {
                        std::hint::black_box(obj.id);
                    })
                    .unwrap();
            }
            start.elapsed()
        };

        let aligned_time = {
            let start = Instant::now();
            for handle in &handles_aligned {
                handle
                    .with(|obj| {
                        std::hint::black_box(obj.id);
                    })
                    .unwrap();
            }
            start.elapsed()
        };

        CacheLocalityResults {
            unaligned_time,
            aligned_time,
            iterations,
        }
    }

    /// Benchmark pool growth overhead.
    pub fn bench_pool_growth() -> GrowthBenchmarkResults {
        // Small initial capacity to force growth
        let config = PoolConfigBuilder::new("GrowthBench")
            .with_initial_capacity(10)
            .with_grow_by(10)
            .build();

        let pool = ObjectPool::<BenchObject>::new(config).unwrap();

        let mut growth_times = Vec::new();
        let mut handles = Vec::new();

        for i in 0..100 {
            let start = Instant::now();
            handles.push(pool.alloc(BenchObject::new(i)).unwrap());
            let elapsed = start.elapsed();

            // Growth happens at 10, 20, 30, etc.
            if (i + 1) % 10 == 0 {
                growth_times.push(elapsed);
            }
        }

        std::hint::black_box(handles);

        GrowthBenchmarkResults {
            growth_times,
            total_allocations: 100,
        }
    }

    /// Benchmark memory usage.
    pub fn bench_memory_usage(object_count: usize) -> MemoryUsageResults {
        let config = PoolConfig::for_game_objects("MemoryBench");
        let pool = ObjectPool::<BenchObject>::new(config).unwrap();

        let initial_usage = pool.memory_usage();

        let handles: Vec<_> = (0..object_count)
            .map(|i| pool.alloc(BenchObject::new(i as u64)).unwrap())
            .collect();

        let final_usage = pool.memory_usage();

        let stats = pool.stats().snapshot();

        std::hint::black_box(handles);

        MemoryUsageResults {
            object_count,
            object_size: std::mem::size_of::<BenchObject>(),
            initial_pool_bytes: initial_usage,
            final_pool_bytes: final_usage,
            effective_bytes: stats.bytes_in_use,
            overhead_bytes: final_usage - stats.bytes_in_use,
            fragmentation: stats.fragmentation,
        }
    }

    /// Run all benchmarks and print results.
    pub fn run_all_benchmarks() {
        println!("\n╔═══════════════════════════════════════════════════════╗");
        println!("║     Memory Pool Benchmark Suite                      ║");
        println!("╚═══════════════════════════════════════════════════════╝");

        // Allocation comparison
        let alloc_results = bench_allocation_comparison(10000);
        alloc_results.print();

        // Mixed operations
        println!("\n=== Mixed Operations Benchmark ===");
        let mixed_time = bench_mixed_operations(10000);
        println!("Mixed ops (10,000 iterations): {:?}", mixed_time);
        println!(
            "Avg per operation: {:.2} μs",
            mixed_time.as_micros() as f64 / 10000.0
        );

        // Concurrent allocations
        println!("\n=== Concurrent Allocations Benchmark ===");
        let concurrent_time = bench_concurrent_allocations(8, 1000);
        println!("8 threads × 1,000 allocs: {:?}", concurrent_time);
        println!(
            "Throughput: {:.0} allocs/sec",
            8000.0 / concurrent_time.as_secs_f64()
        );

        // Cache locality
        let cache_results = bench_cache_locality(1000);
        cache_results.print();

        // Pool growth
        let growth_results = bench_pool_growth();
        growth_results.print();

        // Memory usage
        let memory_results = bench_memory_usage(1000);
        memory_results.print();
    }

    // Result types

    pub struct BenchmarkResults {
        pub pool_time: Duration,
        pub box_time: Duration,
        pub vec_time: Duration,
        pub iterations: usize,
    }

    impl BenchmarkResults {
        pub fn print(&self) {
            println!(
                "Pool allocator:  {:?} ({:.2} μs/alloc)",
                self.pool_time,
                self.pool_time.as_micros() as f64 / self.iterations as f64
            );
            println!(
                "Box allocator:   {:?} ({:.2} μs/alloc)",
                self.box_time,
                self.box_time.as_micros() as f64 / self.iterations as f64
            );
            println!(
                "Vec (pre-sized): {:?} ({:.2} μs/alloc)",
                self.vec_time,
                self.vec_time.as_micros() as f64 / self.iterations as f64
            );

            let speedup_vs_box = self.box_time.as_secs_f64() / self.pool_time.as_secs_f64();
            let speedup_vs_vec = self.vec_time.as_secs_f64() / self.pool_time.as_secs_f64();

            println!("Speedup vs Box: {:.2}x", speedup_vs_box);
            println!("Speedup vs Vec: {:.2}x", speedup_vs_vec);
        }
    }

    pub struct CacheLocalityResults {
        pub unaligned_time: Duration,
        pub aligned_time: Duration,
        pub iterations: usize,
    }

    impl CacheLocalityResults {
        pub fn print(&self) {
            println!("\n=== Cache Locality Benchmark ===");
            println!(
                "Unaligned: {:?} ({:.2} ns/access)",
                self.unaligned_time,
                self.unaligned_time.as_nanos() as f64 / self.iterations as f64
            );
            println!(
                "Aligned:   {:?} ({:.2} ns/access)",
                self.aligned_time,
                self.aligned_time.as_nanos() as f64 / self.iterations as f64
            );

            let improvement = (self.unaligned_time.as_secs_f64() - self.aligned_time.as_secs_f64())
                / self.unaligned_time.as_secs_f64()
                * 100.0;
            println!("Improvement: {:.1}%", improvement);
        }
    }

    pub struct GrowthBenchmarkResults {
        pub growth_times: Vec<Duration>,
        pub total_allocations: usize,
    }

    impl GrowthBenchmarkResults {
        pub fn print(&self) {
            println!("\n=== Pool Growth Benchmark ===");
            println!("Total allocations: {}", self.total_allocations);
            println!("Growth events: {}", self.growth_times.len());

            if !self.growth_times.is_empty() {
                let avg_growth_time: Duration =
                    self.growth_times.iter().sum::<Duration>() / self.growth_times.len() as u32;
                let max_growth_time = self.growth_times.iter().max().unwrap();

                println!("Avg growth time: {:?}", avg_growth_time);
                println!("Max growth time: {:?}", max_growth_time);
            }
        }
    }

    pub struct MemoryUsageResults {
        pub object_count: usize,
        pub object_size: usize,
        pub initial_pool_bytes: usize,
        pub final_pool_bytes: usize,
        pub effective_bytes: usize,
        pub overhead_bytes: usize,
        pub fragmentation: f64,
    }

    impl MemoryUsageResults {
        pub fn print(&self) {
            println!("\n=== Memory Usage Benchmark ===");
            println!("Objects:        {}", self.object_count);
            println!("Object size:    {} bytes", self.object_size);
            println!(
                "Theoretical:    {} bytes",
                self.object_count * self.object_size
            );
            println!(
                "Effective:      {} bytes ({:.2} MB)",
                self.effective_bytes,
                self.effective_bytes as f64 / 1_048_576.0
            );
            println!(
                "Pool total:     {} bytes ({:.2} MB)",
                self.final_pool_bytes,
                self.final_pool_bytes as f64 / 1_048_576.0
            );
            println!(
                "Overhead:       {} bytes ({:.1}%)",
                self.overhead_bytes,
                self.overhead_bytes as f64 / self.final_pool_bytes as f64 * 100.0
            );
            println!("Fragmentation:  {:.1}%", self.fragmentation * 100.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::benchmarks::*;

    #[test]
    #[ignore] // Run manually with: cargo test --features benchmark -- --ignored
    fn run_benchmarks() {
        run_all_benchmarks();
    }

    #[test]
    fn quick_allocation_bench() {
        let results = bench_allocation_comparison(1000);
        // Just ensure it runs without panic
        assert!(results.pool_time.as_nanos() > 0);
    }
}
