//! CPU benchmarking module
//!
//! Comprehensive CPU performance benchmarks for game engine performance testing.
//! Tests single-threaded and multi-threaded performance, cache efficiency,
//! and CPU-intensive game operations like physics calculations and AI pathfinding.
//!
//! # Benchmarks Included
//!
//! - **Single-threaded Performance** - Tests CPU speed for sequential operations
//! - **Multi-threaded Performance** - Tests parallel processing capabilities
//! - **Cache Efficiency** - Memory access patterns and cache hit rates
//! - **Integer Operations** - Game logic calculations (coordinates, IDs, etc.)
//! - **Floating Point Operations** - Physics and graphics math
//! - **Vector Math** - SIMD operations for 3D transformations
//! - **Matrix Operations** - Camera and model transformations
//! - **Game Physics Simulation** - Particle systems, collisions
//! - **Pathfinding** - A* algorithm performance
//! - **Unit AI Logic** - Decision trees and state machines

use crate::{BenchmarkConfig, BenchmarkResult, BenchmarkCategory, Measurement, MeasurementUnit, Result};
use std::time::Instant;

#[cfg(feature = "cpu")]
use rayon::prelude::*;

/// CPU benchmarks for game engine performance
pub struct CpuBenchmarks {
    config: BenchmarkConfig,
}

impl CpuBenchmarks {
    pub fn new(config: &BenchmarkConfig) -> Self {
        Self {
            config: config.clone(),
        }
    }

    /// Run all CPU benchmarks
    pub async fn run_all(&mut self) -> Result<Vec<BenchmarkResult>> {
        let mut results = Vec::new();

        log::info!("Running CPU benchmarks...");

        // Single-threaded performance tests
        results.push(self.benchmark_integer_operations().await?);
        results.push(self.benchmark_floating_point_operations().await?);
        results.push(self.benchmark_vector_math().await?);
        results.push(self.benchmark_matrix_operations().await?);

        // Multi-threaded performance tests
        #[cfg(feature = "cpu")]
        {
            results.push(self.benchmark_parallel_processing().await?);
            results.push(self.benchmark_thread_scaling().await?);
        }

        // Cache and memory access patterns
        results.push(self.benchmark_cache_efficiency().await?);
        results.push(self.benchmark_memory_access_patterns().await?);

        // Game-specific benchmarks
        results.push(self.benchmark_physics_simulation().await?);
        results.push(self.benchmark_pathfinding().await?);
        results.push(self.benchmark_unit_ai_logic().await?);

        log::info!("CPU benchmarks completed: {} tests", results.len());

        Ok(results)
    }

    /// Benchmark integer arithmetic operations (common in game logic)
    async fn benchmark_integer_operations(&self) -> Result<BenchmarkResult> {
        let mut result = BenchmarkResult::new(
            "CPU Integer Operations".to_string(),
            BenchmarkCategory::Cpu,
        );

        result.add_metadata("description".to_string(),
            "Tests integer arithmetic used in game logic (coordinates, unit IDs, counters)".to_string());

        // Warmup
        for _ in 0..self.config.warmup_iterations {
            let _ = Self::integer_workload(1000);
        }

        // Measure
        for _ in 0..self.config.measurement_iterations {
            let start = Instant::now();
            let _ = Self::integer_workload(10000);
            let duration = start.elapsed();

            result.add_measurement(Measurement::new(
                duration.as_nanos() as f64,
                MeasurementUnit::Nanoseconds,
            ));
        }

        // Calculate operations per second
        let stats = result.statistics();
        let ops_per_sec = (10000.0 * 1_000_000_000.0) / stats.mean;
        result.add_measurement(Measurement::new(
            ops_per_sec,
            MeasurementUnit::OperationsPerSecond,
        ).with_metadata("metric".to_string(), "throughput".to_string()));

        Ok(result)
    }

    /// Integer arithmetic workload
    fn integer_workload(iterations: u32) -> i64 {
        let mut sum: i64 = 0;
        let mut a: i64 = 1;
        let mut b: i64 = 2;

        for i in 0..iterations {
            a = a.wrapping_add(b);
            b = a.wrapping_sub(b);
            sum = sum.wrapping_add(a ^ b);
            sum = sum.wrapping_mul(i as i64);
            sum = sum.wrapping_rem(1_000_000);
        }

        sum
    }

    /// Benchmark floating-point operations (physics, graphics math)
    async fn benchmark_floating_point_operations(&self) -> Result<BenchmarkResult> {
        let mut result = BenchmarkResult::new(
            "CPU Floating-Point Operations".to_string(),
            BenchmarkCategory::Cpu,
        );

        result.add_metadata("description".to_string(),
            "Tests floating-point math used in physics and graphics calculations".to_string());

        // Warmup
        for _ in 0..self.config.warmup_iterations {
            let _ = Self::floating_point_workload(1000);
        }

        // Measure
        for _ in 0..self.config.measurement_iterations {
            let start = Instant::now();
            let _ = Self::floating_point_workload(10000);
            let duration = start.elapsed();

            result.add_measurement(Measurement::new(
                duration.as_nanos() as f64,
                MeasurementUnit::Nanoseconds,
            ));
        }

        let stats = result.statistics();
        let ops_per_sec = (10000.0 * 1_000_000_000.0) / stats.mean;
        result.add_measurement(Measurement::new(
            ops_per_sec,
            MeasurementUnit::OperationsPerSecond,
        ).with_metadata("metric".to_string(), "throughput".to_string()));

        Ok(result)
    }

    /// Floating-point arithmetic workload
    fn floating_point_workload(iterations: u32) -> f64 {
        let mut sum = 0.0;
        let mut a = 1.0;
        let mut b = 2.0;

        for i in 0..iterations {
            a = a + b;
            b = a - b;
            sum += (a * b).sqrt();
            sum += (i as f64).sin() * (i as f64).cos();
            sum = sum.abs().min(1_000_000.0);
        }

        sum
    }

    /// Benchmark vector math operations (3D transformations)
    async fn benchmark_vector_math(&self) -> Result<BenchmarkResult> {
        let mut result = BenchmarkResult::new(
            "CPU Vector Math (3D)".to_string(),
            BenchmarkCategory::Cpu,
        );

        result.add_metadata("description".to_string(),
            "Tests 3D vector operations for unit positions and transformations".to_string());

        use glam::Vec3;

        // Warmup
        for _ in 0..self.config.warmup_iterations {
            let _ = Self::vector_workload(1000);
        }

        // Measure
        for _ in 0..self.config.measurement_iterations {
            let start = Instant::now();
            let _ = Self::vector_workload(10000);
            let duration = start.elapsed();

            result.add_measurement(Measurement::new(
                duration.as_nanos() as f64,
                MeasurementUnit::Nanoseconds,
            ));
        }

        Ok(result)
    }

    /// Vector math workload
    fn vector_workload(iterations: u32) -> f32 {
        use glam::Vec3;

        let mut result = Vec3::ZERO;
        let a = Vec3::new(1.0, 2.0, 3.0);
        let b = Vec3::new(4.0, 5.0, 6.0);

        for i in 0..iterations {
            let t = (i % 100) as f32 / 100.0;
            result += a.lerp(b, t);
            result = result.normalize_or_zero();
            result = result.cross(a).dot(b) * Vec3::ONE;
        }

        result.length()
    }

    /// Benchmark matrix operations (camera and model transformations)
    async fn benchmark_matrix_operations(&self) -> Result<BenchmarkResult> {
        let mut result = BenchmarkResult::new(
            "CPU Matrix Operations (4x4)".to_string(),
            BenchmarkCategory::Cpu,
        );

        result.add_metadata("description".to_string(),
            "Tests 4x4 matrix operations for camera and model transformations".to_string());

        // Warmup
        for _ in 0..self.config.warmup_iterations {
            let _ = Self::matrix_workload(1000);
        }

        // Measure
        for _ in 0..self.config.measurement_iterations {
            let start = Instant::now();
            let _ = Self::matrix_workload(5000);
            let duration = start.elapsed();

            result.add_measurement(Measurement::new(
                duration.as_nanos() as f64,
                MeasurementUnit::Nanoseconds,
            ));
        }

        Ok(result)
    }

    /// Matrix math workload
    fn matrix_workload(iterations: u32) -> f32 {
        use glam::{Mat4, Vec3};

        let mut transform = Mat4::IDENTITY;

        for i in 0..iterations {
            let angle = (i as f32) * 0.01;
            let rotation = Mat4::from_rotation_y(angle);
            let translation = Mat4::from_translation(Vec3::new(
                (i as f32).sin(),
                (i as f32).cos(),
                i as f32 * 0.1,
            ));
            let scale = Mat4::from_scale(Vec3::splat(1.0 + (i as f32) * 0.001));

            transform = transform * rotation * translation * scale;
        }

        transform.determinant()
    }

    /// Benchmark parallel processing performance
    #[cfg(feature = "cpu")]
    async fn benchmark_parallel_processing(&self) -> Result<BenchmarkResult> {
        let mut result = BenchmarkResult::new(
            "CPU Parallel Processing".to_string(),
            BenchmarkCategory::Cpu,
        );

        result.add_metadata("description".to_string(),
            "Tests multi-threaded performance using Rayon".to_string());
        result.add_metadata("threads".to_string(), num_cpus::get().to_string());

        let data: Vec<u32> = (0..100000).collect();

        // Warmup
        for _ in 0..self.config.warmup_iterations {
            let _: u64 = data.par_iter()
                .map(|&x| (x as u64).pow(2))
                .sum();
        }

        // Measure
        for _ in 0..self.config.measurement_iterations {
            let start = Instant::now();
            let _: u64 = data.par_iter()
                .map(|&x| (x as u64).pow(2))
                .sum();
            let duration = start.elapsed();

            result.add_measurement(Measurement::new(
                duration.as_nanos() as f64,
                MeasurementUnit::Nanoseconds,
            ));
        }

        Ok(result)
    }

    /// Benchmark thread scaling efficiency
    #[cfg(feature = "cpu")]
    async fn benchmark_thread_scaling(&self) -> Result<BenchmarkResult> {
        let mut result = BenchmarkResult::new(
            "CPU Thread Scaling".to_string(),
            BenchmarkCategory::Cpu,
        );

        result.add_metadata("description".to_string(),
            "Tests how performance scales with thread count".to_string());

        let data: Vec<u32> = (0..1000000).collect();
        let num_cores = num_cpus::get();

        // Test with different thread counts
        for thread_count in [1, 2, 4, num_cores.min(8), num_cores] {
            let pool = rayon::ThreadPoolBuilder::new()
                .num_threads(thread_count)
                .build()
                .unwrap();

            let start = Instant::now();
            pool.install(|| {
                let _: u64 = data.par_iter()
                    .map(|&x| Self::cpu_intensive_work(x))
                    .sum();
            });
            let duration = start.elapsed();

            result.add_measurement(Measurement::new(
                duration.as_millis() as f64,
                MeasurementUnit::Milliseconds,
            ).with_metadata("threads".to_string(), thread_count.to_string()));
        }

        Ok(result)
    }

    /// CPU-intensive work for thread scaling test
    #[cfg(feature = "cpu")]
    fn cpu_intensive_work(x: u32) -> u64 {
        let mut result = x as u64;
        for _ in 0..100 {
            result = result.wrapping_mul(31).wrapping_add(17);
            result ^= result >> 13;
        }
        result
    }

    /// Benchmark cache efficiency with different access patterns
    async fn benchmark_cache_efficiency(&self) -> Result<BenchmarkResult> {
        let mut result = BenchmarkResult::new(
            "CPU Cache Efficiency".to_string(),
            BenchmarkCategory::Cpu,
        );

        result.add_metadata("description".to_string(),
            "Tests cache performance with sequential vs random access".to_string());

        const SIZE: usize = 1024 * 1024; // 1M elements
        let data: Vec<u32> = (0..SIZE as u32).collect();

        // Sequential access (cache-friendly)
        let start = Instant::now();
        let mut sum: u64 = 0;
        for _ in 0..10 {
            for &val in &data {
                sum = sum.wrapping_add(val as u64);
            }
        }
        let sequential_time = start.elapsed();

        result.add_measurement(Measurement::new(
            sequential_time.as_micros() as f64,
            MeasurementUnit::Microseconds,
        ).with_metadata("pattern".to_string(), "sequential".to_string()));

        // Random access (cache-unfriendly)
        let indices: Vec<usize> = (0..SIZE).map(|i| (i * 31337) % SIZE).collect();
        let start = Instant::now();
        let mut sum: u64 = 0;
        for _ in 0..10 {
            for &idx in &indices {
                sum = sum.wrapping_add(data[idx] as u64);
            }
        }
        let random_time = start.elapsed();

        result.add_measurement(Measurement::new(
            random_time.as_micros() as f64,
            MeasurementUnit::Microseconds,
        ).with_metadata("pattern".to_string(), "random".to_string()));

        // Calculate cache efficiency ratio
        let ratio = random_time.as_nanos() as f64 / sequential_time.as_nanos() as f64;
        result.add_measurement(Measurement::new(
            ratio,
            MeasurementUnit::Ratio,
        ).with_metadata("metric".to_string(), "random_vs_sequential".to_string()));

        Ok(result)
    }

    /// Benchmark memory access patterns
    async fn benchmark_memory_access_patterns(&self) -> Result<BenchmarkResult> {
        let mut result = BenchmarkResult::new(
            "CPU Memory Access Patterns".to_string(),
            BenchmarkCategory::Cpu,
        );

        result.add_metadata("description".to_string(),
            "Tests different memory access patterns and stride sizes".to_string());

        const SIZE: usize = 8 * 1024 * 1024; // 8M elements (32MB)
        let data: Vec<u32> = (0..SIZE as u32).collect();

        // Test different stride sizes
        for stride in &[1, 2, 4, 8, 16, 64, 256] {
            let start = Instant::now();
            let mut sum: u64 = 0;
            let mut i = 0;
            while i < SIZE {
                sum = sum.wrapping_add(data[i] as u64);
                i += stride;
            }
            let duration = start.elapsed();

            result.add_measurement(Measurement::new(
                duration.as_micros() as f64,
                MeasurementUnit::Microseconds,
            ).with_metadata("stride".to_string(), stride.to_string()));
        }

        Ok(result)
    }

    /// Benchmark physics simulation (particle system)
    async fn benchmark_physics_simulation(&self) -> Result<BenchmarkResult> {
        let mut result = BenchmarkResult::new(
            "Game Physics Simulation".to_string(),
            BenchmarkCategory::Cpu,
        );

        result.add_metadata("description".to_string(),
            "Simulates particle physics with collisions and forces".to_string());

        use glam::Vec3;

        #[derive(Clone)]
        struct Particle {
            position: Vec3,
            velocity: Vec3,
            mass: f32,
        }

        let mut particles: Vec<Particle> = (0..1000)
            .map(|i| Particle {
                position: Vec3::new(
                    (i as f32 % 10.0) * 10.0,
                    (i as f32 / 10.0) * 10.0,
                    (i as f32 / 100.0) * 10.0,
                ),
                velocity: Vec3::new(
                    ((i * 17) % 100) as f32 - 50.0,
                    ((i * 31) % 100) as f32 - 50.0,
                    ((i * 47) % 100) as f32 - 50.0,
                ),
                mass: 1.0 + (i % 10) as f32,
            })
            .collect();

        // Warmup
        for _ in 0..5 {
            Self::simulate_physics_step(&mut particles, 0.016);
        }

        // Measure
        for _ in 0..self.config.measurement_iterations.min(50) {
            let start = Instant::now();
            Self::simulate_physics_step(&mut particles, 0.016);
            let duration = start.elapsed();

            result.add_measurement(Measurement::new(
                duration.as_micros() as f64,
                MeasurementUnit::Microseconds,
            ));
        }

        Ok(result)
    }

    /// Simulate one physics step
    fn simulate_physics_step(particles: &mut [Particle], dt: f32) {
        use glam::Vec3;

        let gravity = Vec3::new(0.0, -9.81, 0.0);

        // Apply forces and update positions
        for particle in particles.iter_mut() {
            let force = gravity * particle.mass;
            let acceleration = force / particle.mass;
            particle.velocity += acceleration * dt;
            particle.position += particle.velocity * dt;

            // Simple boundary collisions
            if particle.position.y < 0.0 {
                particle.position.y = 0.0;
                particle.velocity.y = -particle.velocity.y * 0.8; // Bounce with energy loss
            }
        }
    }

    /// Benchmark pathfinding (A* algorithm)
    async fn benchmark_pathfinding(&self) -> Result<BenchmarkResult> {
        let mut result = BenchmarkResult::new(
            "Game Pathfinding (A*)".to_string(),
            BenchmarkCategory::Cpu,
        );

        result.add_metadata("description".to_string(),
            "Tests A* pathfinding on a grid map (typical RTS scenario)".to_string());

        // Create a 100x100 grid with obstacles
        const SIZE: usize = 100;
        let mut grid = vec![vec![true; SIZE]; SIZE]; // true = walkable

        // Add some obstacles
        for i in 0..SIZE {
            for j in 0..SIZE {
                if (i + j) % 7 == 0 {
                    grid[i][j] = false; // obstacle
                }
            }
        }

        // Warmup
        for _ in 0..5 {
            let _ = Self::find_path(&grid, (0, 0), (SIZE - 1, SIZE - 1));
        }

        // Measure multiple pathfinding operations
        for _ in 0..self.config.measurement_iterations.min(20) {
            let start = Instant::now();
            let _ = Self::find_path(&grid, (0, 0), (SIZE - 1, SIZE - 1));
            let duration = start.elapsed();

            result.add_measurement(Measurement::new(
                duration.as_micros() as f64,
                MeasurementUnit::Microseconds,
            ));
        }

        Ok(result)
    }

    /// Simple A* pathfinding implementation
    fn find_path(grid: &[Vec<bool>], start: (usize, usize), goal: (usize, usize)) -> Option<Vec<(usize, usize)>> {
        use std::collections::{BinaryHeap, HashMap};
        use std::cmp::Ordering;

        #[derive(Eq, PartialEq)]
        struct Node {
            pos: (usize, usize),
            f_score: i32,
        }

        impl Ord for Node {
            fn cmp(&self, other: &Self) -> Ordering {
                other.f_score.cmp(&self.f_score) // Reverse for min-heap
            }
        }

        impl PartialOrd for Node {
            fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
                Some(self.cmp(other))
            }
        }

        let heuristic = |a: (usize, usize), b: (usize, usize)| -> i32 {
            ((a.0 as i32 - b.0 as i32).abs() + (a.1 as i32 - b.1 as i32).abs()) * 10
        };

        let mut open_set = BinaryHeap::new();
        let mut came_from: HashMap<(usize, usize), (usize, usize)> = HashMap::new();
        let mut g_score: HashMap<(usize, usize), i32> = HashMap::new();

        g_score.insert(start, 0);
        open_set.push(Node {
            pos: start,
            f_score: heuristic(start, goal),
        });

        while let Some(Node { pos: current, .. }) = open_set.pop() {
            if current == goal {
                // Reconstruct path
                let mut path = vec![current];
                let mut current = current;
                while let Some(&prev) = came_from.get(&current) {
                    path.push(prev);
                    current = prev;
                }
                path.reverse();
                return Some(path);
            }

            // Check neighbors
            let neighbors = [
                (current.0.wrapping_sub(1), current.1),
                (current.0 + 1, current.1),
                (current.0, current.1.wrapping_sub(1)),
                (current.0, current.1 + 1),
            ];

            for neighbor in neighbors {
                if neighbor.0 >= grid.len() || neighbor.1 >= grid[0].len() {
                    continue;
                }
                if !grid[neighbor.0][neighbor.1] {
                    continue; // obstacle
                }

                let tentative_g_score = g_score.get(&current).unwrap_or(&i32::MAX) + 10;

                if tentative_g_score < *g_score.get(&neighbor).unwrap_or(&i32::MAX) {
                    came_from.insert(neighbor, current);
                    g_score.insert(neighbor, tentative_g_score);
                    open_set.push(Node {
                        pos: neighbor,
                        f_score: tentative_g_score + heuristic(neighbor, goal),
                    });
                }
            }
        }

        None // No path found
    }

    /// Benchmark unit AI logic (decision tree)
    async fn benchmark_unit_ai_logic(&self) -> Result<BenchmarkResult> {
        let mut result = BenchmarkResult::new(
            "Game Unit AI Logic".to_string(),
            BenchmarkCategory::Cpu,
        );

        result.add_metadata("description".to_string(),
            "Tests AI decision making for game units (state machines, targeting)".to_string());

        use glam::Vec3;

        #[derive(Clone)]
        struct Unit {
            position: Vec3,
            health: f32,
            ammo: i32,
            state: UnitState,
        }

        #[derive(Clone, PartialEq)]
        enum UnitState {
            Idle,
            Moving,
            Attacking,
            Retreating,
        }

        let mut units: Vec<Unit> = (0..500)
            .map(|i| Unit {
                position: Vec3::new(
                    (i % 25) as f32 * 10.0,
                    0.0,
                    (i / 25) as f32 * 10.0,
                ),
                health: 100.0,
                ammo: 30,
                state: UnitState::Idle,
            })
            .collect();

        let enemies: Vec<Unit> = (0..500)
            .map(|i| Unit {
                position: Vec3::new(
                    500.0 + (i % 25) as f32 * 10.0,
                    0.0,
                    (i / 25) as f32 * 10.0,
                ),
                health: 100.0,
                ammo: 30,
                state: UnitState::Idle,
            })
            .collect();

        // Warmup
        for _ in 0..5 {
            Self::update_unit_ai(&mut units.clone(), &enemies);
        }

        // Measure
        for _ in 0..self.config.measurement_iterations.min(50) {
            let start = Instant::now();
            Self::update_unit_ai(&mut units.clone(), &enemies);
            let duration = start.elapsed();

            result.add_measurement(Measurement::new(
                duration.as_micros() as f64,
                MeasurementUnit::Microseconds,
            ));
        }

        Ok(result)
    }

    /// Update AI for all units
    fn update_unit_ai(units: &mut [Unit], enemies: &[Unit]) {
        for unit in units.iter_mut() {
            // Find closest enemy
            let mut closest_enemy_dist = f32::MAX;
            let mut closest_enemy_pos = glam::Vec3::ZERO;

            for enemy in enemies.iter() {
                let dist = unit.position.distance(enemy.position);
                if dist < closest_enemy_dist {
                    closest_enemy_dist = dist;
                    closest_enemy_pos = enemy.position;
                }
            }

            // Decision tree for unit behavior
            if unit.health < 30.0 {
                unit.state = UnitState::Retreating;
            } else if unit.ammo == 0 {
                unit.state = UnitState::Retreating;
            } else if closest_enemy_dist < 50.0 {
                unit.state = UnitState::Attacking;
                // Simulate targeting calculations
                let _ = (closest_enemy_pos - unit.position).normalize();
            } else if closest_enemy_dist < 200.0 {
                unit.state = UnitState::Moving;
            } else {
                unit.state = UnitState::Idle;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_cpu_benchmarks_run() {
        let config = BenchmarkConfig {
            warmup_iterations: 2,
            measurement_iterations: 5,
            ..Default::default()
        };

        let mut cpu_bench = CpuBenchmarks::new(&config);
        let results = cpu_bench.run_all().await.unwrap();

        assert!(!results.is_empty(), "Should have CPU benchmark results");

        for result in results {
            assert!(!result.measurements.is_empty(), "Each benchmark should have measurements");
            assert_eq!(result.category, BenchmarkCategory::Cpu);
        }
    }

    #[test]
    fn test_integer_workload() {
        let result = CpuBenchmarks::integer_workload(1000);
        assert_ne!(result, 0, "Workload should produce non-zero result");
    }

    #[test]
    fn test_floating_point_workload() {
        let result = CpuBenchmarks::floating_point_workload(1000);
        assert!(result.is_finite(), "Result should be finite");
    }

    #[test]
    fn test_pathfinding() {
        const SIZE: usize = 20;
        let grid = vec![vec![true; SIZE]; SIZE];

        let path = CpuBenchmarks::find_path(&grid, (0, 0), (SIZE - 1, SIZE - 1));
        assert!(path.is_some(), "Should find a path in open grid");

        let path = path.unwrap();
        assert_eq!(path.first(), Some(&(0, 0)));
        assert_eq!(path.last(), Some(&(SIZE - 1, SIZE - 1)));
    }
}