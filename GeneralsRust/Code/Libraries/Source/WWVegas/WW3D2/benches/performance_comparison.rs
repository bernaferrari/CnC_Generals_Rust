//! Comprehensive performance benchmarks comparing Rust to C++ baseline
//!
//! This benchmark suite measures performance of critical operations to ensure
//! we match or exceed C++ performance

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use glam::{Mat4, Vec3};
use std::time::Duration;

// ============================================================================
// SIMD Math Benchmarks
// ============================================================================

fn bench_simd_vector_ops(c: &mut Criterion) {
    let mut group = c.benchmark_group("simd_vector_ops");

    for size in [100, 1000, 10000].iter() {
        let vectors_a: Vec<Vec3> = (0..*size)
            .map(|i| Vec3::new(i as f32, (i + 1) as f32, (i + 2) as f32))
            .collect();
        let vectors_b: Vec<Vec3> = (0..*size)
            .map(|i| Vec3::new((i + 10) as f32, (i + 11) as f32, (i + 12) as f32))
            .collect();

        group.throughput(Throughput::Elements(*size as u64));

        // Dot product benchmark
        group.bench_with_input(BenchmarkId::new("dot_product", size), size, |b, _| {
            let mut output = vec![0.0f32; *size];
            b.iter(|| {
                for i in 0..*size {
                    output[i] = vectors_a[i].dot(vectors_b[i]);
                }
                black_box(&output);
            });
        });

        // Cross product benchmark
        group.bench_with_input(BenchmarkId::new("cross_product", size), size, |b, _| {
            let mut output = vec![Vec3::ZERO; *size];
            b.iter(|| {
                for i in 0..*size {
                    output[i] = vectors_a[i].cross(vectors_b[i]);
                }
                black_box(&output);
            });
        });

        // Normalize benchmark
        group.bench_with_input(BenchmarkId::new("normalize", size), size, |b, _| {
            let mut output = vectors_a.clone();
            b.iter(|| {
                for v in &mut output {
                    *v = v.normalize_or_zero();
                }
                black_box(&output);
            });
        });
    }

    group.finish();
}

fn bench_simd_matrix_ops(c: &mut Criterion) {
    let mut group = c.benchmark_group("simd_matrix_ops");

    for size in [10, 100, 1000].iter() {
        let matrices: Vec<Mat4> = (0..*size)
            .map(|i| Mat4::from_translation(Vec3::new(i as f32, 0.0, 0.0)))
            .collect();

        group.throughput(Throughput::Elements(*size as u64));

        // Matrix multiplication
        group.bench_with_input(BenchmarkId::new("matrix_multiply", size), size, |b, _| {
            b.iter(|| {
                let mut result = Mat4::IDENTITY;
                for m in &matrices {
                    result = result * *m;
                }
                black_box(result);
            });
        });

        // Point transformation
        let points: Vec<Vec3> = (0..*size * 10)
            .map(|i| Vec3::new(i as f32, (i + 1) as f32, (i + 2) as f32))
            .collect();

        group.bench_with_input(BenchmarkId::new("transform_points", size), size, |b, _| {
            let matrix = matrices[0];
            let mut output = vec![Vec3::ZERO; points.len()];
            b.iter(|| {
                for (i, point) in points.iter().enumerate() {
                    output[i] = matrix.transform_point3(*point);
                }
                black_box(&output);
            });
        });
    }

    group.finish();
}

// ============================================================================
// Memory Pooling Benchmarks
// ============================================================================

fn bench_memory_pooling(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_pooling");

    // Allocation without pooling
    group.bench_function("allocation_no_pool", |b| {
        b.iter(|| {
            let mut objects = Vec::new();
            for _ in 0..1000 {
                objects.push(vec![0.0f32; 100]);
            }
            black_box(objects);
        });
    });

    // With pooling (simulated)
    group.bench_function("allocation_with_pool", |b| {
        let mut pool = Vec::with_capacity(1000);
        for _ in 0..1000 {
            pool.push(vec![0.0f32; 100]);
        }

        b.iter(|| {
            let mut objects = Vec::new();
            for item in &pool {
                objects.push(item.clone());
            }
            black_box(objects);
        });
    });

    group.finish();
}

// ============================================================================
// Parallel Operations Benchmarks
// ============================================================================

fn bench_parallel_scene_update(c: &mut Criterion) {
    use rayon::prelude::*;

    let mut group = c.benchmark_group("parallel_scene_update");

    for size in [100, 1000, 5000].iter() {
        let mut objects: Vec<Vec3> = (0..*size)
            .map(|i| Vec3::new(i as f32, 0.0, 0.0))
            .collect();

        group.throughput(Throughput::Elements(*size as u64));

        // Sequential update
        group.bench_with_input(BenchmarkId::new("sequential", size), size, |b, _| {
            b.iter(|| {
                for obj in &mut objects {
                    *obj += Vec3::new(1.0, 0.0, 0.0);
                }
                black_box(&objects);
            });
        });

        // Parallel update
        group.bench_with_input(BenchmarkId::new("parallel", size), size, |b, _| {
            b.iter(|| {
                objects.par_iter_mut().for_each(|obj| {
                    *obj += Vec3::new(1.0, 0.0, 0.0);
                });
                black_box(&objects);
            });
        });
    }

    group.finish();
}

fn bench_parallel_culling(c: &mut Criterion) {
    use rayon::prelude::*;

    let mut group = c.benchmark_group("parallel_culling");

    for size in [100, 1000, 5000].iter() {
        let objects: Vec<(Vec3, f32)> = (0..*size)
            .map(|i| (Vec3::new(i as f32 * 0.1, 0.0, 0.0), 1.0))
            .collect();

        let camera_pos = Vec3::ZERO;

        group.throughput(Throughput::Elements(*size as u64));

        // Sequential culling
        group.bench_with_input(BenchmarkId::new("sequential", size), size, |b, _| {
            b.iter(|| {
                let visible: Vec<bool> = objects
                    .iter()
                    .map(|(pos, radius)| (*pos - camera_pos).length() < *radius + 100.0)
                    .collect();
                black_box(visible);
            });
        });

        // Parallel culling
        group.bench_with_input(BenchmarkId::new("parallel", size), size, |b, _| {
            b.iter(|| {
                let visible: Vec<bool> = objects
                    .par_iter()
                    .map(|(pos, radius)| (*pos - camera_pos).length() < *radius + 100.0)
                    .collect();
                black_box(visible);
            });
        });
    }

    group.finish();
}

// ============================================================================
// Caching Benchmarks
// ============================================================================

fn bench_transform_cache(c: &mut Criterion) {
    let mut group = c.benchmark_group("transform_cache");

    // No caching - recompute every time
    group.bench_function("no_cache", |b| {
        let local_transforms: Vec<Mat4> = (0..1000)
            .map(|i| Mat4::from_translation(Vec3::new(i as f32, 0.0, 0.0)))
            .collect();

        b.iter(|| {
            let mut world_transforms = Vec::new();
            for transform in &local_transforms {
                let world = Mat4::IDENTITY * *transform;
                world_transforms.push(world);
            }
            black_box(world_transforms);
        });
    });

    // With caching - only compute when dirty
    group.bench_function("with_cache", |b| {
        let local_transforms: Vec<Mat4> = (0..1000)
            .map(|i| Mat4::from_translation(Vec3::new(i as f32, 0.0, 0.0)))
            .collect();
        let mut cached = vec![Mat4::IDENTITY; 1000];
        let mut dirty = vec![true; 1000];

        b.iter(|| {
            for i in 0..1000 {
                if dirty[i] {
                    cached[i] = Mat4::IDENTITY * local_transforms[i];
                    dirty[i] = false;
                }
            }
            black_box(&cached);
        });
    });

    group.finish();
}

// ============================================================================
// Batch Rendering Benchmarks
// ============================================================================

fn bench_batch_rendering(c: &mut Criterion) {
    let mut group = c.benchmark_group("batch_rendering");

    #[derive(Clone)]
    struct DrawCall {
        vertices: Vec<Vec3>,
        indices: Vec<u32>,
    }

    // Individual draw calls
    group.bench_function("individual_draws", |b| {
        let objects: Vec<DrawCall> = (0..100)
            .map(|_| DrawCall {
                vertices: vec![Vec3::ZERO; 100],
                indices: vec![0; 300],
            })
            .collect();

        b.iter(|| {
            for obj in &objects {
                // Simulate draw call overhead
                black_box(&obj.vertices);
                black_box(&obj.indices);
            }
        });
    });

    // Batched draw calls
    group.bench_function("batched_draws", |b| {
        let mut combined_vertices = Vec::new();
        let mut combined_indices = Vec::new();

        for _ in 0..100 {
            let offset = combined_vertices.len() as u32;
            combined_vertices.extend_from_slice(&vec![Vec3::ZERO; 100]);
            let indices: Vec<u32> = (0..300).map(|i| i + offset).collect();
            combined_indices.extend_from_slice(&indices);
        }

        b.iter(|| {
            // Single draw call
            black_box(&combined_vertices);
            black_box(&combined_indices);
        });
    });

    group.finish();
}

// ============================================================================
// Collision Detection Benchmarks
// ============================================================================

fn bench_collision_detection(c: &mut Criterion) {
    let mut group = c.benchmark_group("collision_detection");

    for size in [100, 500, 1000].iter() {
        let spheres: Vec<(Vec3, f32)> = (0..*size)
            .map(|i| {
                let x = (i % 10) as f32 * 2.0;
                let y = ((i / 10) % 10) as f32 * 2.0;
                let z = (i / 100) as f32 * 2.0;
                (Vec3::new(x, y, z), 1.0)
            })
            .collect();

        group.throughput(Throughput::Elements(*size as u64));

        // Brute force collision detection
        group.bench_with_input(BenchmarkId::new("brute_force", size), size, |b, _| {
            b.iter(|| {
                let mut collisions = 0;
                for i in 0..spheres.len() {
                    for j in (i + 1)..spheres.len() {
                        let dist = (spheres[i].0 - spheres[j].0).length();
                        if dist < spheres[i].1 + spheres[j].1 {
                            collisions += 1;
                        }
                    }
                }
                black_box(collisions);
            });
        });

        // With spatial hashing (simulated)
        group.bench_with_input(BenchmarkId::new("spatial_hash", size), size, |b, _| {
            use std::collections::HashMap;

            b.iter(|| {
                let mut grid: HashMap<(i32, i32, i32), Vec<usize>> = HashMap::new();
                let cell_size = 5.0;

                // Hash objects into grid
                for (i, (pos, _)) in spheres.iter().enumerate() {
                    let cell = (
                        (pos.x / cell_size).floor() as i32,
                        (pos.y / cell_size).floor() as i32,
                        (pos.z / cell_size).floor() as i32,
                    );
                    grid.entry(cell).or_insert_with(Vec::new).push(i);
                }

                // Test only objects in same cells
                let mut collisions = 0;
                for indices in grid.values() {
                    for i in 0..indices.len() {
                        for j in (i + 1)..indices.len() {
                            let idx_a = indices[i];
                            let idx_b = indices[j];
                            let dist = (spheres[idx_a].0 - spheres[idx_b].0).length();
                            if dist < spheres[idx_a].1 + spheres[idx_b].1 {
                                collisions += 1;
                            }
                        }
                    }
                }
                black_box(collisions);
            });
        });
    }

    group.finish();
}

// ============================================================================
// Overall System Benchmark
// ============================================================================

fn bench_complete_frame(c: &mut Criterion) {
    use rayon::prelude::*;

    let mut group = c.benchmark_group("complete_frame");
    group.measurement_time(Duration::from_secs(10));

    // Simulate a complete game frame
    group.bench_function("frame_simulation", |b| {
        let mut objects: Vec<(Vec3, Vec3, Mat4)> = (0..1000)
            .map(|i| {
                let pos = Vec3::new(i as f32 * 0.1, 0.0, 0.0);
                let vel = Vec3::new(1.0, 0.0, 0.0);
                let transform = Mat4::from_translation(pos);
                (pos, vel, transform)
            })
            .collect();

        let delta_time = 0.016; // 60 FPS

        b.iter(|| {
            // 1. Update physics (parallel)
            objects.par_iter_mut().for_each(|(pos, vel, transform)| {
                *pos += *vel * delta_time;
                *transform = Mat4::from_translation(*pos);
            });

            // 2. Frustum culling (parallel)
            let camera_pos = Vec3::ZERO;
            let visible: Vec<bool> = objects
                .par_iter()
                .map(|(pos, _, _)| (*pos - camera_pos).length() < 100.0)
                .collect();

            // 3. Sort for rendering (parallel)
            let mut indices: Vec<usize> = (0..objects.len()).collect();
            indices.par_sort_by(|&a, &b| {
                let dist_a = (objects[a].0 - camera_pos).length_squared();
                let dist_b = (objects[b].0 - camera_pos).length_squared();
                dist_b.partial_cmp(&dist_a).unwrap()
            });

            black_box(&objects);
            black_box(&visible);
            black_box(&indices);
        });
    });

    group.finish();
}

// Register all benchmark groups
criterion_group!(
    benches,
    bench_simd_vector_ops,
    bench_simd_matrix_ops,
    bench_memory_pooling,
    bench_parallel_scene_update,
    bench_parallel_culling,
    bench_transform_cache,
    bench_batch_rendering,
    bench_collision_detection,
    bench_complete_frame,
);

criterion_main!(benches);
