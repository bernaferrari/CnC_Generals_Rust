//! Optimized AABTree traversal with SIMD and early-out tests
//!
//! Performance-optimized version of AABTree matching C++ SSE performance

use crate::aabtree::{AABTree, CullNode, MeshGeometry};
use crate::collision_tests::{AABoxCollisionTest, CollisionTest};
use crate::intersection::{RayCollisionTest, Triangle};
use glam::Vec3;

#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;

/// Optimized AABTree extensions
pub trait AABTreeOptimized {
    /// Fast ray cast with SIMD optimizations
    fn cast_ray_optimized(&self, ray_test: &mut RayCollisionTest, mesh: &dyn MeshGeometry) -> bool;

    /// Fast AABB cast with early-out
    fn cast_aabox_optimized(&self, box_test: &mut AABoxCollisionTest, mesh: &dyn MeshGeometry) -> bool;

    /// Batch ray casting
    fn cast_rays_batch(&self, ray_tests: &mut [RayCollisionTest], mesh: &dyn MeshGeometry) -> Vec<bool>;
}

impl AABTreeOptimized for AABTree {
    fn cast_ray_optimized(&self, ray_test: &mut RayCollisionTest, mesh: &dyn MeshGeometry) -> bool {
        if self.nodes.is_empty() {
            return false;
        }

        #[cfg(target_arch = "x86_64")]
        {
            if is_x86_feature_detected!("sse4.1") {
                unsafe {
                    return cast_ray_simd(&self.nodes, &self.poly_indices, ray_test, mesh);
                }
            }
        }

        // Fallback to standard traversal
        self.cast_ray(ray_test, mesh)
    }

    fn cast_aabox_optimized(&self, box_test: &mut AABoxCollisionTest, mesh: &dyn MeshGeometry) -> bool {
        if self.nodes.is_empty() {
            return false;
        }

        // Use optimized traversal with early-out
        cast_aabox_with_early_out(&self.nodes, &self.poly_indices, box_test, mesh)
    }

    fn cast_rays_batch(&self, ray_tests: &mut [RayCollisionTest], mesh: &dyn MeshGeometry) -> Vec<bool> {
        use rayon::prelude::*;

        ray_tests.par_iter_mut().map(|test| {
            self.cast_ray_optimized(test, mesh)
        }).collect()
    }
}

/// SIMD-optimized ray casting
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "sse4.1")]
unsafe fn cast_ray_simd(
    nodes: &[CullNode],
    poly_indices: &[u32],
    ray_test: &mut RayCollisionTest,
    mesh: &dyn MeshGeometry,
) -> bool {
    // Stack-based traversal (faster than recursion)
    let mut stack = Vec::with_capacity(64);
    stack.push(0usize);

    let mut hit = false;

    // Load ray data into SIMD registers
    let ray_origin = _mm_set_ps(0.0, ray_test.ray_origin.z, ray_test.ray_origin.y, ray_test.ray_origin.x);
    let ray_dir = _mm_set_ps(0.0, ray_test.ray_direction.z, ray_test.ray_direction.y, ray_test.ray_direction.x);
    let inv_dir = _mm_set_ps(
        0.0,
        1.0 / ray_test.ray_direction.z,
        1.0 / ray_test.ray_direction.y,
        1.0 / ray_test.ray_direction.x,
    );

    while let Some(node_idx) = stack.pop() {
        if node_idx >= nodes.len() {
            continue;
        }

        let node = &nodes[node_idx];

        // SIMD AABB-ray intersection test
        if !test_ray_aabb_simd(ray_origin, inv_dir, node, ray_test.result.fraction) {
            continue;
        }

        if node.is_leaf() {
            // Test against polygons
            if test_ray_against_polys(node, poly_indices, ray_test, mesh) {
                hit = true;
                if ray_test.result.start_bad {
                    break;
                }
            }
        } else {
            // Add children to stack (closer child last for better early-out)
            let front_child = node.get_front_child();
            let back_child = node.get_back_child();

            // Determine which child is closer
            let mid_point = (node.min + node.max) * 0.5;
            let to_mid = mid_point - ray_test.ray_origin;
            let dist_front = to_mid.dot(ray_test.ray_direction);

            if dist_front > 0.0 {
                // Ray going towards front, push back first
                if back_child < nodes.len() {
                    stack.push(back_child);
                }
                if front_child < nodes.len() {
                    stack.push(front_child);
                }
            } else {
                // Ray going towards back, push front first
                if front_child < nodes.len() {
                    stack.push(front_child);
                }
                if back_child < nodes.len() {
                    stack.push(back_child);
                }
            }
        }
    }

    hit
}

/// SIMD ray-AABB intersection test
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "sse4.1")]
unsafe fn test_ray_aabb_simd(
    ray_origin: __m128,
    inv_dir: __m128,
    node: &CullNode,
    max_dist: f32,
) -> bool {
    let min = _mm_set_ps(0.0, node.min.z, node.min.y, node.min.x);
    let max = _mm_set_ps(0.0, node.max.z, node.max.y, node.max.x);

    let t1 = _mm_mul_ps(_mm_sub_ps(min, ray_origin), inv_dir);
    let t2 = _mm_mul_ps(_mm_sub_ps(max, ray_origin), inv_dir);

    let tmin = _mm_min_ps(t1, t2);
    let tmax = _mm_max_ps(t1, t2);

    // Extract components
    let mut tmin_array = [0.0f32; 4];
    let mut tmax_array = [0.0f32; 4];
    _mm_storeu_ps(tmin_array.as_mut_ptr(), tmin);
    _mm_storeu_ps(tmax_array.as_mut_ptr(), tmax);

    let t_enter = tmin_array[0].max(tmin_array[1]).max(tmin_array[2]);
    let t_exit = tmax_array[0].min(tmax_array[1]).min(tmax_array[2]);

    t_enter <= t_exit && t_exit >= 0.0 && t_enter <= max_dist
}

/// Test ray against polygons in a leaf node
fn test_ray_against_polys(
    node: &CullNode,
    poly_indices: &[u32],
    ray_test: &mut RayCollisionTest,
    mesh: &dyn MeshGeometry,
) -> bool {
    if node.get_poly_count() == 0 {
        return false;
    }

    let vertices = mesh.get_vertex_array();
    let polygons = mesh.get_polygon_array();
    let poly0 = node.get_poly0();
    let poly_count = node.get_poly_count();

    let mut hit = false;

    for poly_counter in 0..poly_count {
        if let Some(&poly_index_u32) = poly_indices.get(poly0 + poly_counter) {
            let poly_index = poly_index_u32 as usize;

            if poly_index >= polygons.len() {
                continue;
            }

            let triangle_indices = &polygons[poly_index];

            // Build triangle
            let v0 = vertices[triangle_indices[0] as usize];
            let v1 = vertices[triangle_indices[1] as usize];
            let v2 = vertices[triangle_indices[2] as usize];

            // Moller-Trumbore intersection
            if ray_triangle_intersect_fast(ray_test, v0, v1, v2) {
                hit = true;
                ray_test.result.surface_type = mesh.get_poly_surface_type(poly_index);

                if ray_test.result.start_bad {
                    return true;
                }
            }
        }
    }

    hit
}

/// Fast Moller-Trumbore ray-triangle intersection
#[inline(always)]
fn ray_triangle_intersect_fast(
    ray_test: &mut RayCollisionTest,
    v0: Vec3,
    v1: Vec3,
    v2: Vec3,
) -> bool {
    const EPSILON: f32 = 0.0001;

    let edge1 = v1 - v0;
    let edge2 = v2 - v0;

    let h = ray_test.ray_direction.cross(edge2);
    let a = edge1.dot(h);

    // Early out for parallel ray
    if a.abs() < EPSILON {
        return false;
    }

    let f = 1.0 / a;
    let s = ray_test.ray_origin - v0;
    let u = f * s.dot(h);

    // Early out for barycentric coords
    if u < 0.0 || u > 1.0 {
        return false;
    }

    let q = s.cross(edge1);
    let v = f * ray_test.ray_direction.dot(q);

    if v < 0.0 || u + v > 1.0 {
        return false;
    }

    let t = f * edge2.dot(q);

    if t > EPSILON && t < ray_test.result.fraction {
        ray_test.result.fraction = t;
        ray_test.result.normal = edge1.cross(edge2).normalize_or_zero();
        return true;
    }

    false
}

/// Optimized AABB cast with early-out
fn cast_aabox_with_early_out(
    nodes: &[CullNode],
    poly_indices: &[u32],
    box_test: &mut AABoxCollisionTest,
    mesh: &dyn MeshGeometry,
) -> bool {
    let mut stack = Vec::with_capacity(64);
    stack.push(0usize);

    let mut hit = false;

    // Precompute sweep bounds
    let box_min = box_test.aabb.center - box_test.aabb.extent;
    let box_max = box_test.aabb.center + box_test.aabb.extent;
    let end_min = box_test.aabb.center + box_test.movement - box_test.aabb.extent;
    let end_max = box_test.aabb.center + box_test.movement + box_test.aabb.extent;
    let sweep_min = box_min.min(end_min);
    let sweep_max = box_max.max(end_max);

    while let Some(node_idx) = stack.pop() {
        if node_idx >= nodes.len() {
            continue;
        }

        let node = &nodes[node_idx];

        // Quick AABB overlap test
        if !test_aabb_overlap(sweep_min, sweep_max, node.min, node.max) {
            continue;
        }

        if node.is_leaf() {
            // Test against polygons
            if test_aabox_against_polys(node, poly_indices, box_test, mesh) {
                hit = true;
                if box_test.result.start_bad {
                    break;
                }
            }
        } else {
            // Add children to stack
            let front_child = node.get_front_child();
            let back_child = node.get_back_child();

            if front_child < nodes.len() {
                stack.push(front_child);
            }
            if back_child < nodes.len() {
                stack.push(back_child);
            }
        }
    }

    hit
}

/// Fast AABB overlap test
#[inline(always)]
fn test_aabb_overlap(min1: Vec3, max1: Vec3, min2: Vec3, max2: Vec3) -> bool {
    min1.x <= max2.x
        && max1.x >= min2.x
        && min1.y <= max2.y
        && max1.y >= min2.y
        && min1.z <= max2.z
        && max1.z >= min2.z
}

/// Test AABB against polygons in a leaf node
fn test_aabox_against_polys(
    node: &CullNode,
    poly_indices: &[u32],
    box_test: &mut AABoxCollisionTest,
    mesh: &dyn MeshGeometry,
) -> bool {
    if node.get_poly_count() == 0 {
        return false;
    }

    let vertices = mesh.get_vertex_array();
    let polygons = mesh.get_polygon_array();
    let poly0 = node.get_poly0();
    let poly_count = node.get_poly_count();

    let mut hit = false;

    for poly_counter in 0..poly_count {
        if let Some(&poly_index_u32) = poly_indices.get(poly0 + poly_counter) {
            let poly_index = poly_index_u32 as usize;

            if poly_index >= polygons.len() {
                continue;
            }

            let triangle_indices = &polygons[poly_index];

            // Build triangle bounds
            let v0 = vertices[triangle_indices[0] as usize];
            let v1 = vertices[triangle_indices[1] as usize];
            let v2 = vertices[triangle_indices[2] as usize];

            let tri_min = v0.min(v1).min(v2);
            let tri_max = v0.max(v1).max(v2);

            let box_min = box_test.aabb.center - box_test.aabb.extent;
            let box_max = box_test.aabb.center + box_test.aabb.extent;

            // Quick overlap test
            if test_aabb_overlap(box_min, box_max, tri_min, tri_max) {
                hit = true;
                box_test.result.surface_type = mesh.get_poly_surface_type(poly_index);

                if box_test.result.start_bad {
                    return true;
                }
            }
        }
    }

    hit
}

/// Performance-optimized tree construction hints
pub struct AABTreeBuildHints {
    /// Maximum polygons per leaf
    pub max_polys_per_leaf: usize,
    /// Maximum tree depth
    pub max_depth: usize,
    /// Use SAH (Surface Area Heuristic) for better quality
    pub use_sah: bool,
}

impl Default for AABTreeBuildHints {
    fn default() -> Self {
        Self {
            max_polys_per_leaf: 8,
            max_depth: 32,
            use_sah: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_aabb_overlap() {
        let min1 = Vec3::new(0.0, 0.0, 0.0);
        let max1 = Vec3::new(1.0, 1.0, 1.0);

        let min2 = Vec3::new(0.5, 0.5, 0.5);
        let max2 = Vec3::new(1.5, 1.5, 1.5);

        assert!(test_aabb_overlap(min1, max1, min2, max2));

        let min3 = Vec3::new(2.0, 2.0, 2.0);
        let max3 = Vec3::new(3.0, 3.0, 3.0);

        assert!(!test_aabb_overlap(min1, max1, min3, max3));
    }
}
