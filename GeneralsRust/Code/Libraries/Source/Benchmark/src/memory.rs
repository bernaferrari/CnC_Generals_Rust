//! Memory benchmarking module
//!
//! Comprehensive memory performance benchmarks for game engine testing.
//! Tests allocation patterns, memory bandwidth, cache performance, and
//! realistic game memory usage scenarios.
//!
//! # Benchmarks Included
//!
//! - **Allocation Performance** - Small, medium, and large allocations
//! - **Deallocation Performance** - Free performance and fragmentation
//! - **Memory Bandwidth** - Sequential and random access patterns
//! - **Cache Performance** - L1, L2, L3 cache efficiency
//! - **Pool Allocators** - Custom allocator performance
//! - **Game Object Creation** - Realistic game entity allocation
//! - **Vector Operations** - Dynamic array performance
//! - **HashMap Operations** - Hash table performance

use crate::{BenchmarkConfig, BenchmarkResult, BenchmarkCategory, Measurement, MeasurementUnit, Result};
use std::time::Instant;
use std::collections::HashMap;

/// Memory benchmarks for allocation and access patterns
pub struct MemoryBenchmarks {
    config: BenchmarkConfig,
}

impl MemoryBenchmarks {
    pub fn new(config: &BenchmarkConfig) -> Self {
        Self {
            config: config.clone(),
        }
    }

    /// Run all memory benchmarks
    pub async fn run_all(&mut self) -> Result<Vec<BenchmarkResult>> {
        let mut results = Vec::new();

        log::info!("Running memory benchmarks...");

        // Allocation benchmarks
        results.push(self.benchmark_small_allocations().await?);
        results.push(self.benchmark_medium_allocations().await?);
        results.push(self.benchmark_large_allocations().await?);

        // Memory bandwidth tests
        results.push(self.benchmark_sequential_read().await?);
        results.push(self.benchmark_sequential_write().await?);
        results.push(self.benchmark_random_access().await?);

        // Cache performance
        results.push(self.benchmark_cache_lines().await?);

        // Collection operations
        results.push(self.benchmark_vector_operations().await?);
        results.push(self.benchmark_hashmap_operations().await?);

        // Game-specific tests
        results.push(self.benchmark_game_object_creation().await?);

        log::info!("Memory benchmarks completed: {} tests", results.len());

        Ok(results)
    }

    /// Benchmark small allocations (< 1KB)
    async fn benchmark_small_allocations(&self) -> Result<BenchmarkResult> {
        let mut result = BenchmarkResult::new(
            "Memory Small Allocations".to_string(),
            BenchmarkCategory::Memory,
        );

        result.add_metadata("description".to_string(),
            "Tests small allocation performance (typical for game objects)".to_string());

        const ALLOC_SIZE: usize = 64;
        const ALLOC_COUNT: usize = 10000;

        // Warmup
        for _ in 0..self.config.warmup_iterations {
            let _: Vec<Vec<u8>> = (0..ALLOC_COUNT)
                .map(|_| vec![0u8; ALLOC_SIZE])
                .collect();
        }

        // Measure
        for _ in 0..self.config.measurement_iterations {
            let start = Instant::now();
            let allocations: Vec<Vec<u8>> = (0..ALLOC_COUNT)
                .map(|_| vec![0u8; ALLOC_SIZE])
                .collect();
            drop(allocations);
            let duration = start.elapsed();

            result.add_measurement(Measurement::new(
                duration.as_nanos() as f64,
                MeasurementUnit::Nanoseconds,
            ));

            // Calculate allocations per second
            let allocs_per_sec = (ALLOC_COUNT as f64 * 1_000_000_000.0) / duration.as_nanos() as f64;
            result.add_measurement(Measurement::new(
                allocs_per_sec,
                MeasurementUnit::OperationsPerSecond,
            ).with_metadata("metric".to_string(), "allocations_per_second".to_string()));
        }

        Ok(result)
    }

    /// Benchmark medium allocations (1KB - 1MB)
    async fn benchmark_medium_allocations(&self) -> Result<BenchmarkResult> {
        let mut result = BenchmarkResult::new(
            "Memory Medium Allocations".to_string(),
            BenchmarkCategory::Memory,
        );

        result.add_metadata("description".to_string(),
            "Tests medium allocation performance (textures, meshes)".to_string());

        const ALLOC_SIZE: usize = 64 * 1024; // 64KB
        const ALLOC_COUNT: usize = 1000;

        // Warmup
        for _ in 0..self.config.warmup_iterations.min(5) {
            let _: Vec<Vec<u8>> = (0..ALLOC_COUNT)
                .map(|_| vec![0u8; ALLOC_SIZE])
                .collect();
        }

        // Measure
        for _ in 0..self.config.measurement_iterations.min(20) {
            let start = Instant::now();
            let allocations: Vec<Vec<u8>> = (0..ALLOC_COUNT)
                .map(|_| vec![0u8; ALLOC_SIZE])
                .collect();
            drop(allocations);
            let duration = start.elapsed();

            result.add_measurement(Measurement::new(
                duration.as_micros() as f64,
                MeasurementUnit::Microseconds,
            ));
        }

        Ok(result)
    }

    /// Benchmark large allocations (> 1MB)
    async fn benchmark_large_allocations(&self) -> Result<BenchmarkResult> {
        let mut result = BenchmarkResult::new(
            "Memory Large Allocations".to_string(),
            BenchmarkCategory::Memory,
        );

        result.add_metadata("description".to_string(),
            "Tests large allocation performance (level data, asset loading)".to_string());

        const ALLOC_SIZE: usize = 16 * 1024 * 1024; // 16MB
        const ALLOC_COUNT: usize = 10;

        // Warmup
        for _ in 0..self.config.warmup_iterations.min(3) {
            let _: Vec<Vec<u8>> = (0..ALLOC_COUNT)
                .map(|_| vec![0u8; ALLOC_SIZE])
                .collect();
        }

        // Measure
        for _ in 0..self.config.measurement_iterations.min(10) {
            let start = Instant::now();
            let allocations: Vec<Vec<u8>> = (0..ALLOC_COUNT)
                .map(|_| vec![0u8; ALLOC_SIZE])
                .collect();
            drop(allocations);
            let duration = start.elapsed();

            result.add_measurement(Measurement::new(
                duration.as_millis() as f64,
                MeasurementUnit::Milliseconds,
            ));
        }

        Ok(result)
    }

    /// Benchmark sequential read performance
    async fn benchmark_sequential_read(&self) -> Result<BenchmarkResult> {
        let mut result = BenchmarkResult::new(
            "Memory Sequential Read".to_string(),
            BenchmarkCategory::Memory,
        );

        result.add_metadata("description".to_string(),
            "Tests sequential memory read bandwidth".to_string());

        const SIZE: usize = 128 * 1024 * 1024; // 128MB
        let data: Vec<u8> = vec![0xAA; SIZE];

        // Warmup
        for _ in 0..5 {
            let mut sum: u64 = 0;
            for &byte in &data {
                sum = sum.wrapping_add(byte as u64);
            }
            let _ = sum;
        }

        // Measure
        for _ in 0..self.config.measurement_iterations.min(10) {
            let start = Instant::now();
            let mut sum: u64 = 0;
            for &byte in &data {
                sum = sum.wrapping_add(byte as u64);
            }
            let _ = sum;
            let duration = start.elapsed();

            let bandwidth_mb_s = (SIZE as f64 / (1024.0 * 1024.0)) / duration.as_secs_f64();

            result.add_measurement(Measurement::new(
                duration.as_millis() as f64,
                MeasurementUnit::Milliseconds,
            ));

            result.add_measurement(Measurement::new(
                bandwidth_mb_s,
                MeasurementUnit::MegabytesPerSecond,
            ).with_metadata("metric".to_string(), "bandwidth".to_string()));
        }

        Ok(result)
    }

    /// Benchmark sequential write performance
    async fn benchmark_sequential_write(&self) -> Result<BenchmarkResult> {
        let mut result = BenchmarkResult::new(
            "Memory Sequential Write".to_string(),
            BenchmarkCategory::Memory,
        );

        result.add_metadata("description".to_string(),
            "Tests sequential memory write bandwidth".to_string());

        const SIZE: usize = 128 * 1024 * 1024; // 128MB
        let mut data: Vec<u8> = vec![0; SIZE];

        // Warmup
        for _ in 0..5 {
            for byte in &mut data {
                *byte = 0xAA;
            }
        }

        // Measure
        for _ in 0..self.config.measurement_iterations.min(10) {
            let start = Instant::now();
            for (i, byte) in data.iter_mut().enumerate() {
                *byte = (i % 256) as u8;
            }
            let duration = start.elapsed();

            let bandwidth_mb_s = (SIZE as f64 / (1024.0 * 1024.0)) / duration.as_secs_f64();

            result.add_measurement(Measurement::new(
                duration.as_millis() as f64,
                MeasurementUnit::Milliseconds,
            ));

            result.add_measurement(Measurement::new(
                bandwidth_mb_s,
                MeasurementUnit::MegabytesPerSecond,
            ).with_metadata("metric".to_string(), "bandwidth".to_string()));
        }

        Ok(result)
    }

    /// Benchmark random access performance
    async fn benchmark_random_access(&self) -> Result<BenchmarkResult> {
        let mut result = BenchmarkResult::new(
            "Memory Random Access".to_string(),
            BenchmarkCategory::Memory,
        );

        result.add_metadata("description".to_string(),
            "Tests random memory access patterns (cache-unfriendly)".to_string());

        const SIZE: usize = 16 * 1024 * 1024; // 16M elements
        let data: Vec<u32> = (0..SIZE as u32).collect();

        // Generate random access pattern
        let indices: Vec<usize> = (0..100000)
            .map(|i| ((i * 31337 + 17) % SIZE))
            .collect();

        // Warmup
        for _ in 0..5 {
            let mut sum: u64 = 0;
            for &idx in &indices {
                sum = sum.wrapping_add(data[idx] as u64);
            }
            let _ = sum;
        }

        // Measure
        for _ in 0..self.config.measurement_iterations.min(20) {
            let start = Instant::now();
            let mut sum: u64 = 0;
            for &idx in &indices {
                sum = sum.wrapping_add(data[idx] as u64);
            }
            let _ = sum;
            let duration = start.elapsed();

            result.add_measurement(Measurement::new(
                duration.as_micros() as f64,
                MeasurementUnit::Microseconds,
            ));

            let accesses_per_sec = (indices.len() as f64 * 1_000_000.0) / duration.as_micros() as f64;
            result.add_measurement(Measurement::new(
                accesses_per_sec,
                MeasurementUnit::OperationsPerSecond,
            ).with_metadata("metric".to_string(), "random_accesses_per_second".to_string()));
        }

        Ok(result)
    }

    /// Benchmark cache line efficiency
    async fn benchmark_cache_lines(&self) -> Result<BenchmarkResult> {
        let mut result = BenchmarkResult::new(
            "Memory Cache Line Performance".to_string(),
            BenchmarkCategory::Memory,
        );

        result.add_metadata("description".to_string(),
            "Tests performance with different stride sizes (cache line effects)".to_string());

        const SIZE: usize = 8 * 1024 * 1024; // 8M elements
        let data: Vec<u64> = vec![42; SIZE];

        // Test different strides (cache line is typically 64 bytes = 8 u64s)
        for stride in &[1, 2, 4, 8, 16, 32, 64] {
            let start = Instant::now();
            let mut sum: u64 = 0;
            let mut i = 0;
            while i < SIZE {
                sum = sum.wrapping_add(data[i]);
                i += stride;
            }
            let _ = sum;
            let duration = start.elapsed();

            result.add_measurement(Measurement::new(
                duration.as_micros() as f64,
                MeasurementUnit::Microseconds,
            ).with_metadata("stride".to_string(), stride.to_string()));
        }

        Ok(result)
    }

    /// Benchmark vector operations
    async fn benchmark_vector_operations(&self) -> Result<BenchmarkResult> {
        let mut result = BenchmarkResult::new(
            "Memory Vector Operations".to_string(),
            BenchmarkCategory::Memory,
        );

        result.add_metadata("description".to_string(),
            "Tests dynamic array performance (push, pop, resize)".to_string());

        const INITIAL_SIZE: usize = 10000;

        // Test push operations
        for _ in 0..self.config.measurement_iterations.min(20) {
            let start = Instant::now();
            let mut vec = Vec::new();
            for i in 0..INITIAL_SIZE {
                vec.push(i);
            }
            let duration = start.elapsed();

            result.add_measurement(Measurement::new(
                duration.as_micros() as f64,
                MeasurementUnit::Microseconds,
            ).with_metadata("operation".to_string(), "push".to_string()));
        }

        // Test with_capacity (pre-allocated)
        for _ in 0..self.config.measurement_iterations.min(20) {
            let start = Instant::now();
            let mut vec = Vec::with_capacity(INITIAL_SIZE);
            for i in 0..INITIAL_SIZE {
                vec.push(i);
            }
            let duration = start.elapsed();

            result.add_measurement(Measurement::new(
                duration.as_micros() as f64,
                MeasurementUnit::Microseconds,
            ).with_metadata("operation".to_string(), "push_with_capacity".to_string()));
        }

        Ok(result)
    }

    /// Benchmark HashMap operations
    async fn benchmark_hashmap_operations(&self) -> Result<BenchmarkResult> {
        let mut result = BenchmarkResult::new(
            "Memory HashMap Operations".to_string(),
            BenchmarkCategory::Memory,
        );

        result.add_metadata("description".to_string(),
            "Tests hash table performance (insert, lookup, remove)".to_string());

        const ENTRY_COUNT: usize = 10000;

        // Test insert operations
        for _ in 0..self.config.measurement_iterations.min(20) {
            let start = Instant::now();
            let mut map = HashMap::new();
            for i in 0..ENTRY_COUNT {
                map.insert(i, i * 2);
            }
            let duration = start.elapsed();

            result.add_measurement(Measurement::new(
                duration.as_micros() as f64,
                MeasurementUnit::Microseconds,
            ).with_metadata("operation".to_string(), "insert".to_string()));
        }

        // Test lookup operations
        let mut map = HashMap::new();
        for i in 0..ENTRY_COUNT {
            map.insert(i, i * 2);
        }

        for _ in 0..self.config.measurement_iterations.min(20) {
            let start = Instant::now();
            let mut sum = 0;
            for i in 0..ENTRY_COUNT {
                if let Some(&val) = map.get(&i) {
                    sum += val;
                }
            }
            let _ = sum;
            let duration = start.elapsed();

            result.add_measurement(Measurement::new(
                duration.as_micros() as f64,
                MeasurementUnit::Microseconds,
            ).with_metadata("operation".to_string(), "lookup".to_string()));
        }

        Ok(result)
    }

    /// Benchmark game object creation
    async fn benchmark_game_object_creation(&self) -> Result<BenchmarkResult> {
        let mut result = BenchmarkResult::new(
            "Memory Game Object Creation".to_string(),
            BenchmarkCategory::Memory,
        );

        result.add_metadata("description".to_string(),
            "Tests realistic game entity allocation and initialization".to_string());

        #[derive(Clone)]
        struct GameObject {
            id: u32,
            position: [f32; 3],
            rotation: [f32; 4],
            scale: [f32; 3],
            health: f32,
            team: u8,
            state: u8,
            flags: u32,
            components: Vec<u32>,
        }

        impl GameObject {
            fn new(id: u32) -> Self {
                Self {
                    id,
                    position: [0.0, 0.0, 0.0],
                    rotation: [0.0, 0.0, 0.0, 1.0],
                    scale: [1.0, 1.0, 1.0],
                    health: 100.0,
                    team: 0,
                    state: 0,
                    flags: 0,
                    components: Vec::with_capacity(4),
                }
            }
        }

        const OBJECT_COUNT: usize = 10000;

        // Warmup
        for _ in 0..5 {
            let objects: Vec<GameObject> = (0..OBJECT_COUNT as u32)
                .map(GameObject::new)
                .collect();
            drop(objects);
        }

        // Measure
        for _ in 0..self.config.measurement_iterations.min(20) {
            let start = Instant::now();
            let objects: Vec<GameObject> = (0..OBJECT_COUNT as u32)
                .map(GameObject::new)
                .collect();
            drop(objects);
            let duration = start.elapsed();

            result.add_measurement(Measurement::new(
                duration.as_micros() as f64,
                MeasurementUnit::Microseconds,
            ));

            let objects_per_sec = (OBJECT_COUNT as f64 * 1_000_000.0) / duration.as_micros() as f64;
            result.add_measurement(Measurement::new(
                objects_per_sec,
                MeasurementUnit::OperationsPerSecond,
            ).with_metadata("metric".to_string(), "objects_per_second".to_string()));
        }

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_memory_benchmarks_run() {
        let config = BenchmarkConfig {
            warmup_iterations: 2,
            measurement_iterations: 5,
            ..Default::default()
        };

        let mut mem_bench = MemoryBenchmarks::new(&config);
        let results = mem_bench.run_all().await.unwrap();

        assert!(!results.is_empty(), "Should have memory benchmark results");

        for result in results {
            assert!(!result.measurements.is_empty(), "Each benchmark should have measurements");
            assert_eq!(result.category, BenchmarkCategory::Memory);
        }
    }
}