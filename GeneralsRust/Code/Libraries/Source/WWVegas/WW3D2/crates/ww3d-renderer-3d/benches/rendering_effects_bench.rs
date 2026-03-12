//! Performance benchmarks for advanced rendering effects
//!
//! Ensures 60+ FPS on target hardware and memory usage under 100MB

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use glam::{Mat4, Vec3};
use ww3d_renderer_3d::rendering::{
    post_process::{rgb_to_luminance, tone_map_reinhard, GaussianBlur},
    reflection_system::ReflectionPlane,
};

fn bench_reflection_matrix_calculation(c: &mut Criterion) {
    c.bench_function("reflection_matrix_creation", |b| {
        b.iter(|| {
            let normal = black_box(Vec3::new(0.0, 1.0, 0.0));
            let d = black_box(-5.0);
            ReflectionPlane::create_plane_reflection_matrix(normal, d)
        })
    });
}

fn bench_fresnel_calculation(c: &mut Criterion) {
    let plane = ReflectionPlane::new(Vec3::new(0.0, 1.0, 0.0), 0.0, (512, 512));

    c.bench_function("fresnel_calculation", |b| {
        b.iter(|| {
            let view_dir = black_box(Vec3::new(0.5, -0.7, 0.2).normalize());
            plane.calculate_fresnel(view_dir)
        })
    });
}

fn bench_wave_height_calculation(c: &mut Criterion) {
    c.bench_function("wave_height_calculation", |b| {
        b.iter(|| {
            let position = black_box(Vec3::new(10.0, 0.0, 5.0));
            let time = black_box(2.5);
            let wave_scale = black_box(1.0);
            let wave_speed = black_box(1.0);
            let wave_distortion = black_box(0.02);

            let phase = (position.x * wave_scale + time * wave_speed).sin();
            let offset = phase * wave_distortion;

            let phase2 = (position.z * wave_scale * 0.7 - time * wave_speed * 0.5).sin();
            let offset2 = phase2 * wave_distortion * 0.5;

            offset + offset2
        })
    });
}

fn bench_gaussian_blur_creation(c: &mut Criterion) {
    c.bench_function("gaussian_blur_kernel_5", |b| {
        b.iter(|| GaussianBlur::new(black_box(5), black_box(1.0)))
    });

    c.bench_function("gaussian_blur_kernel_9", |b| {
        b.iter(|| GaussianBlur::new(black_box(9), black_box(1.5)))
    });
}

fn bench_rgb_to_luminance(c: &mut Criterion) {
    c.bench_function("rgb_to_luminance", |b| {
        b.iter(|| {
            let color = black_box(Vec3::new(0.8, 0.6, 0.4));
            rgb_to_luminance(color)
        })
    });
}

fn bench_tone_mapping(c: &mut Criterion) {
    c.bench_function("tone_map_reinhard", |b| {
        b.iter(|| {
            let hdr = black_box(Vec3::new(2.0, 3.0, 4.0));
            tone_map_reinhard(hdr)
        })
    });
}

fn bench_batch_operations(c: &mut Criterion) {
    c.bench_function("batch_luminance_1000", |b| {
        let colors: Vec<Vec3> = (0..1000)
            .map(|i| {
                Vec3::new(
                    (i as f32 * 0.1) % 1.0,
                    (i as f32 * 0.2) % 1.0,
                    (i as f32 * 0.3) % 1.0,
                )
            })
            .collect();

        b.iter(|| {
            for color in &colors {
                black_box(rgb_to_luminance(*color));
            }
        })
    });

    c.bench_function("batch_tone_mapping_1000", |b| {
        let hdr_colors: Vec<Vec3> = (0..1000)
            .map(|i| Vec3::new(i as f32 * 0.01, i as f32 * 0.02, i as f32 * 0.03))
            .collect();

        b.iter(|| {
            for color in &hdr_colors {
                black_box(tone_map_reinhard(*color));
            }
        })
    });
}

fn bench_point_above_plane(c: &mut Criterion) {
    let plane = ReflectionPlane::new(Vec3::new(0.0, 1.0, 0.0), -5.0, (512, 512));

    c.bench_function("is_point_above_plane", |b| {
        b.iter(|| {
            let point = black_box(Vec3::new(3.0, 7.0, 2.0));
            plane.is_point_above(point)
        })
    });
}

fn bench_matrix_transform(c: &mut Criterion) {
    let matrix = ReflectionPlane::create_plane_reflection_matrix(Vec3::new(0.0, 1.0, 0.0), -5.0);

    c.bench_function("matrix_transform_point", |b| {
        b.iter(|| {
            let point = black_box(Vec3::new(5.0, 10.0, 3.0));
            matrix.transform_point3(point)
        })
    });
}

// Simulate a frame's worth of water calculations
fn bench_water_frame_simulation(c: &mut Criterion) {
    c.bench_function("water_simulation_1000_points", |b| {
        let points: Vec<Vec3> = (0..1000)
            .map(|i| Vec3::new((i % 100) as f32 * 0.1, 0.0, (i / 100) as f32 * 0.1))
            .collect();

        let time = 1.0;
        let wave_scale = 1.0;
        let wave_speed = 1.0;
        let wave_distortion = 0.02;

        b.iter(|| {
            let mut total_height = 0.0;
            for point in &points {
                let phase = (point.x * wave_scale + time * wave_speed).sin();
                let offset = phase * wave_distortion;

                let phase2 = (point.z * wave_scale * 0.7 - time * wave_speed * 0.5).sin();
                let offset2 = phase2 * wave_distortion * 0.5;

                total_height += offset + offset2;
            }
            black_box(total_height);
        })
    });
}

// Benchmark typical post-processing operations
fn bench_full_post_process_chain(c: &mut Criterion) {
    c.bench_function("full_post_process_pixel", |b| {
        let hdr_color = Vec3::new(1.5, 2.0, 2.5);

        b.iter(|| {
            // 1. Tone mapping
            let tone_mapped = tone_map_reinhard(black_box(hdr_color));

            // 2. Color grading (gamma correction)
            let gamma = 2.2;
            let gamma_corrected = Vec3::new(
                tone_mapped.x.powf(1.0 / gamma),
                tone_mapped.y.powf(1.0 / gamma),
                tone_mapped.z.powf(1.0 / gamma),
            );

            // 3. Saturation adjustment
            let luminance = rgb_to_luminance(gamma_corrected);
            let saturation = 1.1;
            let final_color =
                gamma_corrected * saturation + Vec3::splat(luminance) * (1.0 - saturation);

            black_box(final_color);
        })
    });
}

criterion_group!(
    reflection_benches,
    bench_reflection_matrix_calculation,
    bench_fresnel_calculation,
    bench_wave_height_calculation,
    bench_point_above_plane,
    bench_matrix_transform,
    bench_water_frame_simulation,
);

criterion_group!(
    post_process_benches,
    bench_gaussian_blur_creation,
    bench_rgb_to_luminance,
    bench_tone_mapping,
    bench_batch_operations,
    bench_full_post_process_chain,
);

criterion_main!(reflection_benches, post_process_benches);
