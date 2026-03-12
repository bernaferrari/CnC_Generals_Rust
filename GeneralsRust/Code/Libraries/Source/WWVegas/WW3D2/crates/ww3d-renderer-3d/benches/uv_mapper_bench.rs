//! Performance benchmarks for UV texture mapper system
//!
//! Measures the performance characteristics of animated texture coordinate transformations.
//! Validates that UV mapper operations maintain 60+ FPS performance.

use bytemuck::Pod;
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use glam::Vec2;

/// Benchmark LinearOffset mapper performance
/// Effect: Scrolling textures (water, lava, conveyor belts)
fn bench_linear_offset_mapper(c: &mut Criterion) {
    let mut group = c.benchmark_group("uv_mapper_linear_offset");

    // Single UV transformation
    group.bench_function("linear_offset_single_uv", |b| {
        let uv = black_box(Vec2::new(0.5, 0.5));
        let u_speed = black_box(0.1);
        let v_speed = black_box(0.05);
        let time = black_box(1.5);

        b.iter(|| {
            let new_u = uv.x + u_speed * time;
            let new_v = uv.y + v_speed * time;
            black_box((new_u, new_v))
        })
    });

    // Batch UV transformations (typical mesh: 1000 vertices)
    group.bench_function("linear_offset_1000_vertices", |b| {
        let uvs: Vec<Vec2> = (0..1000)
            .map(|i| Vec2::new((i % 10) as f32 * 0.1, (i / 10) as f32 * 0.1))
            .collect();

        let u_speed = black_box(0.1);
        let v_speed = black_box(0.05);
        let time = black_box(1.5);

        b.iter(|| {
            let mut total = Vec2::ZERO;
            for uv in &uvs {
                let new_u = uv.x + u_speed * time;
                let new_v = uv.y + v_speed * time;
                total += Vec2::new(new_u, new_v);
            }
            black_box(total)
        })
    });

    // Batch with multiple frames (animation sequence)
    group.bench_function("linear_offset_100_frames_1000_vertices", |b| {
        let uvs: Vec<Vec2> = (0..1000)
            .map(|i| Vec2::new((i % 10) as f32 * 0.1, (i / 10) as f32 * 0.1))
            .collect();

        let u_speed = black_box(0.1);
        let v_speed = black_box(0.05);

        b.iter(|| {
            let mut total = 0.0;
            for frame in 0..100 {
                let time = frame as f32 * 0.016; // 60 FPS
                for uv in &uvs {
                    let new_u = uv.x + u_speed * time;
                    let new_v = uv.y + v_speed * time;
                    total += new_u + new_v;
                }
            }
            black_box(total)
        })
    });

    group.finish();
}

/// Benchmark Grid mapper performance (sprite sheets)
/// Effect: Sprite sheet animation
fn bench_grid_mapper(c: &mut Criterion) {
    let mut group = c.benchmark_group("uv_mapper_grid");

    // Single UV transformation
    group.bench_function("grid_single_uv", |b| {
        let uv = black_box(Vec2::new(0.25, 0.25));
        let u_tiles = black_box(4.0);
        let v_tiles = black_box(4.0);
        let u_offset = black_box(0.0);
        let v_offset = black_box(0.0);

        b.iter(|| {
            let new_u = uv.x * u_tiles + u_offset;
            let new_v = uv.y * v_tiles + v_offset;
            black_box((new_u, new_v))
        })
    });

    // Batch UV transformations
    group.bench_function("grid_1000_vertices", |b| {
        let uvs: Vec<Vec2> = (0..1000)
            .map(|i| Vec2::new((i % 10) as f32 * 0.1, (i / 10) as f32 * 0.1))
            .collect();

        let u_tiles = black_box(4.0);
        let v_tiles = black_box(4.0);
        let u_offset = black_box(0.0);
        let v_offset = black_box(0.0);

        b.iter(|| {
            let mut total = Vec2::ZERO;
            for uv in &uvs {
                let new_u = uv.x * u_tiles + u_offset;
                let new_v = uv.y * v_tiles + v_offset;
                total += Vec2::new(new_u, new_v);
            }
            black_box(total)
        })
    });

    group.finish();
}

/// Benchmark Rotate mapper performance
/// Effect: Rotating textures (spinning objects)
fn bench_rotate_mapper(c: &mut Criterion) {
    let mut group = c.benchmark_group("uv_mapper_rotate");

    // Single UV transformation
    group.bench_function("rotate_single_uv", |b| {
        let uv = black_box(Vec2::new(0.5, 0.5));
        let angle = black_box(1.5f32);
        let center_u = black_box(0.5);
        let center_v = black_box(0.5);

        b.iter(|| {
            // Translate to center
            let u = uv.x - center_u;
            let v = uv.y - center_v;

            // Rotate
            let cos_a = angle.cos();
            let sin_a = angle.sin();
            let rotated_u = u * cos_a - v * sin_a;
            let rotated_v = u * sin_a + v * cos_a;

            // Translate back
            let new_u = rotated_u + center_u;
            let new_v = rotated_v + center_v;

            black_box((new_u, new_v))
        })
    });

    // Batch UV transformations
    group.bench_function("rotate_1000_vertices", |b| {
        let uvs: Vec<Vec2> = (0..1000)
            .map(|i| Vec2::new((i % 10) as f32 * 0.1, (i / 10) as f32 * 0.1))
            .collect();

        let angle = black_box(1.5f32);
        let center_u = black_box(0.5);
        let center_v = black_box(0.5);
        let cos_a = angle.cos();
        let sin_a = angle.sin();

        b.iter(|| {
            let mut total = Vec2::ZERO;
            for uv in &uvs {
                let u = uv.x - center_u;
                let v = uv.y - center_v;

                let rotated_u = u * cos_a - v * sin_a;
                let rotated_v = u * sin_a + v * cos_a;

                let new_u = rotated_u + center_u;
                let new_v = rotated_v + center_v;

                total += Vec2::new(new_u, new_v);
            }
            black_box(total)
        })
    });

    group.finish();
}

/// Benchmark SineLinearOffset mapper performance
/// Effect: Wave effects
fn bench_sine_linear_offset_mapper(c: &mut Criterion) {
    let mut group = c.benchmark_group("uv_mapper_sine_linear_offset");

    // Single UV transformation
    group.bench_function("sine_linear_offset_single_uv", |b| {
        let uv = black_box(Vec2::new(0.5, 0.5));
        let u_amplitude = black_box(0.05);
        let v_amplitude = black_box(0.1);
        let frequency = black_box(0.5);
        let phase = black_box(0.0);
        let time = black_box(1.5);

        b.iter(|| {
            let angle = 2.0 * std::f32::consts::PI * frequency * time + phase;
            let wave = angle.sin();
            let new_u = uv.x + u_amplitude * wave;
            let new_v = uv.y + v_amplitude * wave;
            black_box((new_u, new_v))
        })
    });

    // Batch UV transformations
    group.bench_function("sine_linear_offset_1000_vertices", |b| {
        let uvs: Vec<Vec2> = (0..1000)
            .map(|i| Vec2::new((i % 10) as f32 * 0.1, (i / 10) as f32 * 0.1))
            .collect();

        let u_amplitude = black_box(0.05);
        let v_amplitude = black_box(0.1);
        let frequency = black_box(0.5);
        let phase = black_box(0.0);
        let time = black_box(1.5);

        let angle = 2.0 * std::f32::consts::PI * frequency * time + phase;
        let wave = angle.sin();

        b.iter(|| {
            let mut total = Vec2::ZERO;
            for uv in &uvs {
                let new_u = uv.x + u_amplitude * wave;
                let new_v = uv.y + v_amplitude * wave;
                total += Vec2::new(new_u, new_v);
            }
            black_box(total)
        })
    });

    group.finish();
}

/// Benchmark dispatcher function selection
/// Simulates the branch prediction and dispatch overhead
fn bench_mapper_dispatch(c: &mut Criterion) {
    let mut group = c.benchmark_group("uv_mapper_dispatch");

    // Test all mapper types with consistent geometry
    for mapper_type in [0u32, 4, 7, 8, 9].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("dispatch_type_{}", mapper_type)),
            mapper_type,
            |b, &mapper_type| {
                let uvs: Vec<Vec2> = (0..100)
                    .map(|i| Vec2::new((i % 10) as f32 * 0.1, (i / 10) as f32 * 0.1))
                    .collect();

                let mapper_args = [1000i32, 500, 0, 0];
                let time = 1.5f32;

                b.iter(|| {
                    let mut total = Vec2::ZERO;
                    for uv in &uvs {
                        // Dispatcher logic
                        let transformed_uv = match black_box(mapper_type) {
                            0 => *uv, // Pass-through
                            4 => {
                                // LinearOffset
                                let u_speed = mapper_args[0] as f32 / 1000.0;
                                let v_speed = mapper_args[1] as f32 / 1000.0;
                                Vec2::new(uv.x + u_speed * time, uv.y + v_speed * time)
                            }
                            7 => {
                                // Grid
                                let u_tiles = mapper_args[0] as f32;
                                let v_tiles = mapper_args[1] as f32;
                                Vec2::new(uv.x * u_tiles, uv.y * v_tiles)
                            }
                            8 => {
                                // Rotate
                                let angle = mapper_args[0] as f32 / 100.0 * time;
                                let cos_a = angle.cos();
                                let sin_a = angle.sin();
                                let u = uv.x - 0.5;
                                let v = uv.y - 0.5;
                                Vec2::new(u * cos_a - v * sin_a + 0.5, u * sin_a + v * cos_a + 0.5)
                            }
                            9 => {
                                // SineLinearOffset
                                let frequency = mapper_args[2] as f32 / 100.0;
                                let angle = 2.0 * std::f32::consts::PI * frequency * time;
                                let wave = angle.sin();
                                Vec2::new(uv.x + mapper_args[0] as f32 / 1000.0 * wave, uv.y)
                            }
                            _ => *uv,
                        };
                        total += transformed_uv;
                    }
                    black_box(total)
                })
            },
        );
    }

    group.finish();
}

/// Benchmark realistic mesh rendering scenario
/// Simulates actual rendering with multiple materials and mappers
fn bench_realistic_mesh_rendering(c: &mut Criterion) {
    let mut group = c.benchmark_group("uv_mapper_realistic_scenario");

    // Scenario 1: Single mesh with LinearOffset
    group.bench_function("render_mesh_linear_offset", |b| {
        let vertex_count = 1000;
        let uvs: Vec<Vec2> = (0..vertex_count)
            .map(|i| Vec2::new((i % 10) as f32 * 0.1, (i / 10) as f32 * 0.1))
            .collect();

        let u_speed = 0.1f32;
        let v_speed = 0.05f32;
        let time = 1.5f32;

        b.iter(|| {
            let mut transformed = Vec::with_capacity(vertex_count);
            for uv in &uvs {
                transformed.push(Vec2::new(uv.x + u_speed * time, uv.y + v_speed * time));
            }
            black_box(transformed)
        })
    });

    // Scenario 2: Large mesh with Grid mapper (4x4 sprite sheet)
    group.bench_function("render_mesh_grid_sprite", |b| {
        let vertex_count = 5000;
        let uvs: Vec<Vec2> = (0..vertex_count)
            .map(|i| Vec2::new((i % 10) as f32 * 0.1, (i / 10) as f32 * 0.1))
            .collect();

        let u_tiles = 4.0f32;
        let v_tiles = 4.0f32;

        b.iter(|| {
            let mut transformed = Vec::with_capacity(vertex_count);
            for uv in &uvs {
                transformed.push(Vec2::new(uv.x * u_tiles, uv.y * v_tiles));
            }
            black_box(transformed)
        })
    });

    // Scenario 3: Complex mesh with rotating mapper
    group.bench_function("render_mesh_rotate", |b| {
        let vertex_count = 2000;
        let uvs: Vec<Vec2> = (0..vertex_count)
            .map(|i| Vec2::new((i % 10) as f32 * 0.1, (i / 10) as f32 * 0.1))
            .collect();

        let angle = 1.5f32;
        let center = Vec2::splat(0.5);
        let cos_a = angle.cos();
        let sin_a = angle.sin();

        b.iter(|| {
            let mut transformed = Vec::with_capacity(vertex_count);
            for uv in &uvs {
                let offset = *uv - center;
                let rotated = Vec2::new(
                    offset.x * cos_a - offset.y * sin_a,
                    offset.x * sin_a + offset.y * cos_a,
                );
                transformed.push(rotated + center);
            }
            black_box(transformed)
        })
    });

    // Scenario 4: Multiple mappers in sequence (chained transformations)
    group.bench_function("render_mesh_chained_mappers", |b| {
        let vertex_count = 1000;
        let uvs: Vec<Vec2> = (0..vertex_count)
            .map(|i| Vec2::new((i % 10) as f32 * 0.1, (i / 10) as f32 * 0.1))
            .collect();

        let u_speed = 0.1f32;
        let v_speed = 0.05f32;
        let time = 1.5f32;
        let angle = 0.5f32;

        let cos_a = angle.cos();
        let sin_a = angle.sin();

        b.iter(|| {
            let mut transformed = Vec::with_capacity(vertex_count);
            for uv in &uvs {
                // First: LinearOffset
                let mut current = Vec2::new(uv.x + u_speed * time, uv.y + v_speed * time);

                // Second: Rotate around center
                let offset = current - Vec2::splat(0.5);
                let rotated = Vec2::new(
                    offset.x * cos_a - offset.y * sin_a,
                    offset.x * sin_a + offset.y * cos_a,
                );
                current = rotated + Vec2::splat(0.5);

                transformed.push(current);
            }
            black_box(transformed)
        })
    });

    group.finish();
}

/// Benchmark GPU buffer update performance
/// Measures the CPU cost of updating uniforms
fn bench_uniform_buffer_update(c: &mut Criterion) {
    let mut group = c.benchmark_group("uv_mapper_uniform_update");

    // Simulating uniform struct creation
    group.bench_function("create_uv_transform_uniform", |b| {
        let mapper_type = 4u32;
        let mapper_args = [1000i32, 500, 0, 0];
        let animation_time = 1.5f32;

        b.iter(|| {
            let uniform = (
                mapper_type,
                mapper_args,
                [1.0f32, 1.0, 0.0, 0.0],
                animation_time,
                [0.0f32, 0.0, 0.0],
            );
            black_box(uniform)
        })
    });

    // Simulating buffer update (bytemuck cast)
    group.bench_function("cast_uniform_to_bytes", |b| {
        #[repr(C)]
        #[derive(Debug, Clone, Copy, Pod)]
        struct UVTransformUniform {
            mapper_type: u32,
            mapper_args: [i32; 4],
            mapper_float_args: [f32; 4],
            animation_time: f32,
            _pad: [f32; 3],
        }

        // Safety: UVTransformUniform is a POD type with #[repr(C)]
        // which guarantees proper alignment and no invalid bit patterns
        unsafe impl bytemuck::Zeroable for UVTransformUniform {}

        let uniform = UVTransformUniform {
            mapper_type: 4,
            mapper_args: [1000, 500, 0, 0],
            mapper_float_args: [1.0, 1.0, 0.0, 0.0],
            animation_time: 1.5,
            _pad: [0.0, 0.0, 0.0],
        };

        b.iter(|| {
            // Use bytemuck for safe, type-checked byte casting
            // This is equivalent to from_raw_parts but with compile-time alignment checks
            let bytes = bytemuck::bytes_of(&uniform);
            black_box(bytes)
        })
    });

    group.finish();
}

criterion_group!(
    uv_mapper_benches,
    bench_linear_offset_mapper,
    bench_grid_mapper,
    bench_rotate_mapper,
    bench_sine_linear_offset_mapper,
    bench_mapper_dispatch,
    bench_realistic_mesh_rendering,
    bench_uniform_buffer_update,
);

criterion_main!(uv_mapper_benches);
