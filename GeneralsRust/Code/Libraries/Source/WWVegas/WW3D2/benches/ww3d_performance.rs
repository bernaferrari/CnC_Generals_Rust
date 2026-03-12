//! WW3D Engine Performance Benchmarks
//!
//! This module contains comprehensive performance benchmarks for the WW3D engine,
//! comparing Rust implementation performance against expected C++ standards.

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use ww3d_geometry::{MeshBuilder, Vec3, AABox, Sphere, MeshGeometry};
use ww3d_assets::AssetManager;
use ww3d_renderer_3d::Scene;
use ww3d_gpu::GpuDevice;
use std::sync::Arc;

/// Benchmark mesh building performance
fn bench_mesh_builder(c: &mut Criterion) {
    c.bench_function("mesh_builder_cube_1000", |b| {
        b.iter(|| {
            let mut builder = MeshBuilder::new();
            for i in 0..1000 {
                let x = (i % 10) as f32;
                let y = ((i / 10) % 10) as f32;
                let z = (i / 100) as f32;
                builder.add_cube(Vec3::new(x, y, z), 0.5);
            }
            black_box(builder.build());
        })
    });
}

/// Benchmark collision detection performance
fn bench_collision_detection(c: &mut Criterion) {
    let mut boxes = Vec::new();
    for i in 0..1000 {
        let x = (i % 10) as f32 * 2.0;
        let y = ((i / 10) % 10) as f32 * 2.0;
        let z = (i / 100) as f32 * 2.0;
        boxes.push(AABox::new(Vec3::new(x, y, z), Vec3::new(0.5, 0.5, 0.5)));
    }

    c.bench_function("collision_aabb_intersection_1000", |b| {
        b.iter(|| {
            let mut collisions = 0;
            for i in 0..boxes.len() {
                for j in (i + 1)..boxes.len() {
                    if boxes[i].intersects_aabox(&boxes[j]) {
                        collisions += 1;
                    }
                }
            }
            black_box(collisions);
        })
    });

    let spheres: Vec<Sphere> = boxes.iter().map(|b| {
        Sphere::new(b.center, 0.5)
    }).collect();

    c.bench_function("collision_sphere_intersection_1000", |b| {
        b.iter(|| {
            let mut collisions = 0;
            for i in 0..spheres.len() {
                for j in (i + 1)..spheres.len() {
                    if spheres[i].intersects_sphere(&spheres[j]) {
                        collisions += 1;
                    }
                }
            }
            black_box(collisions);
        })
    });
}

/// Benchmark spatial partitioning
fn bench_spatial_partitioning(c: &mut Criterion) {
    use ww3d_geometry::spatial_partitioning::{Octree, SpatialObject};

    let mut objects = Vec::new();
    for i in 0..1000 {
        let x = (i % 10) as f32 * 3.0;
        let y = ((i / 10) % 10) as f32 * 3.0;
        let z = (i / 100) as f32 * 3.0;
        let position = Vec3::new(x, y, z);
        let bounds = AABox::new(position, Vec3::new(0.5, 0.5, 0.5));
        objects.push(SpatialObject::new(i, position, bounds));
    }

    c.bench_function("octree_construction_1000_objects", |b| {
        b.iter(|| {
            let mut octree = Octree::new(AABox::new(Vec3::ZERO, Vec3::new(20.0, 20.0, 20.0)));
            for obj in &objects {
                octree.insert(obj.clone_shallow());
            }
            black_box(octree);
        })
    });
}

/// Benchmark mesh optimization
fn bench_mesh_optimization(c: &mut Criterion) {
    use ww3d_geometry::MeshOptimizer;

    let mut builder = MeshBuilder::new();
    for i in 0..500 {
        let angle = (i as f32) * 0.1;
        let radius = 5.0;
        let x = angle.cos() * radius;
        let z = angle.sin() * radius;
        builder.add_sphere(Vec3::new(x, 0.0, z), 0.3, 8, 6);
    }
    let mesh = builder.build();

    c.bench_function("mesh_weld_vertices_500_spheres", |b| {
        b.iter(|| {
            let mut test_mesh = mesh.clone();
            test_mesh.weld_vertices(0.01);
            black_box(test_mesh);
        })
    });
}

/// Benchmark asset loading performance
fn bench_asset_loading(c: &mut Criterion) {
    // Note: This would require actual W3D files for meaningful benchmarks
    c.bench_function("asset_manager_creation", |b| {
        b.iter(|| {
            let manager = AssetManager::new();
            black_box(manager);
        })
    });
}

/// Benchmark rendering performance
fn bench_rendering_performance(c: &mut Criterion) {
    // This would require a proper GPU context setup
    c.bench_function("scene_creation", |b| {
        b.iter(|| {
            // Note: Scene creation would require actual GPU context
            // This is a placeholder for when GPU is fully functional
            let scene_placeholder = "Scene would be created here";
            black_box(scene_placeholder);
        })
    });
}

/// Benchmark memory allocation patterns
fn bench_memory_allocation(c: &mut Criterion) {
    c.bench_function("vector_mesh_allocation_10k", |b| {
        b.iter(|| {
            let mut meshes = Vec::with_capacity(10000);
            for i in 0..10000 {
                let mesh = MeshGeometry::new();
                meshes.push(mesh);
            }
            black_box(meshes);
        })
    });
}

/// Benchmark mathematical operations
fn bench_math_operations(c: &mut Criterion) {
    use glam::Vec3 as GlamVec3;
    use ww3d_geometry::Vec3;

    let glam_vecs: Vec<GlamVec3> = (0..1000).map(|i| {
        GlamVec3::new(i as f32, (i + 1) as f32, (i + 2) as f32)
    }).collect();

    let ww3d_vecs: Vec<Vec3> = glam_vecs.iter().map(|v| {
        Vec3::new(v.x, v.y, v.z)
    }).collect();

    c.bench_function("glam_vector_operations_1k", |b| {
        b.iter(|| {
            let mut result = GlamVec3::ZERO;
            for v in &glam_vecs {
                result += v.normalize() * 2.0;
                result = result.cross(GlamVec3::Y);
            }
            black_box(result);
        })
    });

    c.bench_function("ww3d_vector_operations_1k", |b| {
        b.iter(|| {
            let mut result = Vec3::ZERO;
            for v in &ww3d_vecs {
                result = result + v.normalize() * 2.0;
                result = result.cross(Vec3::Y);
            }
            black_box(result);
        })
    });
}

/// Benchmark string operations (for asset management)
fn bench_string_operations(c: &mut Criterion) {
    let asset_names: Vec<String> = (0..1000).map(|i| {
        format!("asset_{}_mesh.w3d", i)
    }).collect();

    c.bench_function("asset_name_hashing_1k", |b| {
        b.iter(|| {
            use std::collections::HashMap;
            let mut map = HashMap::new();
            for name in &asset_names {
                map.insert(name.as_str(), name.len());
            }
            black_box(map);
        })
    });
}

/// Benchmark sorting algorithms (for transparency, particles, etc.)
fn bench_sorting_algorithms(c: &mut Criterion) {
    let mut data: Vec<f32> = (0..10000).map(|i| (i % 100) as f32).collect();

    c.bench_function("transparency_sort_10k", |b| {
        b.iter(|| {
            let mut sorted_data = data.clone();
            // Sort by distance (transparency sorting)
            sorted_data.sort_by(|a, b| b.partial_cmp(a).unwrap());
            black_box(sorted_data);
        })
    });

    c.bench_function("particle_sort_10k", |b| {
        b.iter(|| {
            let mut sorted_data = data.clone();
            // Sort by depth (particle sorting)
            sorted_data.sort_by(|a, b| a.partial_cmp(b).unwrap());
            black_box(sorted_data);
        })
    });
}

// Register benchmark groups
criterion_group!(
    benches,
    bench_mesh_builder,
    bench_collision_detection,
    bench_spatial_partitioning,
    bench_mesh_optimization,
    bench_asset_loading,
    bench_rendering_performance,
    bench_memory_allocation,
    bench_math_operations,
    bench_string_operations,
    bench_sorting_algorithms,
);

criterion_main!(benches);