//! Intersection System
//!
//! This module handles various geometric intersection tests, ported from intersec.cpp.

use crate::bounding_volumes::{AABox, OBBox};
use glam::Vec3;

// CRITICAL: Match C++ COINCIDENCE_EPSILON for near-coincident sphere detection
const COINCIDENCE_EPSILON: f32 = 0.000001;

/// Intersection result containing hit information
#[derive(Clone, Debug)]
pub struct IntersectionResult {
    pub intersects: bool,
    pub range: f32,
    pub intersection: Vec3,
    pub normal: Vec3,
    pub collision_type: u32,
    pub surface_type: u32,
    pub start_bad: bool,
}

impl Default for IntersectionResult {
    fn default() -> Self {
        Self {
            intersects: false,
            range: f32::MAX,
            intersection: Vec3::ZERO,
            normal: Vec3::ZERO,
            collision_type: 0,
            surface_type: 0,
            start_bad: false,
        }
    }
}

/// Cast result for sweeping tests
#[derive(Clone, Debug)]
pub struct CastResult {
    pub start_bad: bool,
    pub fraction: f32,
    pub normal: Vec3,
    pub surface_type: u32,
}

impl Default for CastResult {
    fn default() -> Self {
        Self {
            start_bad: false,
            fraction: 1.0,
            normal: Vec3::ZERO,
            surface_type: 0,
        }
    }
}

/// Triangle structure for intersection tests
#[derive(Clone, Debug)]
pub struct Triangle {
    pub vertices: [Vec3; 3],
    pub normal: Vec3,
}

/// Main intersection class - handles various collision tests
pub struct IntersectionClass {
    pub ray_location: Vec3,
    pub ray_direction: Vec3,
    pub result: IntersectionResult,
    pub convex_test: bool,
}

impl Default for IntersectionClass {
    fn default() -> Self {
        Self::new()
    }
}

impl IntersectionClass {
    pub fn new() -> Self {
        Self {
            ray_location: Vec3::ZERO,
            ray_direction: Vec3::ZERO,
            result: IntersectionResult::default(),
            convex_test: false,
        }
    }

    /// Set ray for intersection tests
    pub fn set_ray(&mut self, origin: Vec3, direction: Vec3) {
        self.ray_location = origin;
        self.ray_direction = direction.normalize();
    }

    /// Ray-Box intersection test (ported from intersec.cpp)
    pub fn intersect_box(&self, box_min: Vec3, box_max: Vec3) -> IntersectionResult {
        let mut result = IntersectionResult::default();

        // Fast Ray-Box Intersection, modified from code written by Andrew Woo from "Graphics Gems"
        const RIGHT: usize = 0;
        const LEFT: usize = 1;
        const MIDDLE: usize = 2;
        const PLANE_COUNT: usize = 3;

        let mut inside = true;
        let mut quadrant = [MIDDLE; PLANE_COUNT];
        let mut distance = [-1.0f32; PLANE_COUNT];
        let mut candidate_plane = [0.0f32; PLANE_COUNT];

        // Find candidate planes and determine if the ray is outside the box
        for i in 0..PLANE_COUNT {
            if self.ray_location[i] < box_min[i] {
                quadrant[i] = LEFT;
                candidate_plane[i] = box_min[i];
                inside = false;
            } else if self.ray_location[i] > box_max[i] {
                quadrant[i] = RIGHT;
                candidate_plane[i] = box_max[i];
                inside = false;
            } else {
                quadrant[i] = MIDDLE;
            }
        }

        // Check if ray origin is inside bounding box
        if inside {
            result.intersection = self.ray_location;
            result.intersects = true;
            return result;
        }

        // Calculate distances to candidate planes
        for i in 0..PLANE_COUNT {
            if quadrant[i] != MIDDLE && self.ray_direction[i] != 0.0 {
                distance[i] = (candidate_plane[i] - self.ray_location[i]) / self.ray_direction[i];
            } else {
                distance[i] = -1.0;
            }
        }

        // Get the largest of the distances for final choice of intersection
        let mut nearest_plane = 0;
        for i in 1..PLANE_COUNT {
            if distance[nearest_plane] < distance[i] {
                nearest_plane = i;
            }
        }

        // Check if nearest plane is behind the ray
        if distance[nearest_plane] < 0.0 {
            return result; // No intersection
        }

        // Calculate intersection point
        let mut intersection = Vec3::ZERO;
        for i in 0..PLANE_COUNT {
            if nearest_plane != i {
                intersection[i] =
                    self.ray_location[i] + distance[nearest_plane] * self.ray_direction[i];
                if intersection[i] < box_min[i] || intersection[i] > box_max[i] {
                    return result; // Outside box
                }
            } else {
                intersection[i] = candidate_plane[i];
            }
        }

        result.intersection = intersection;
        result.intersects = true;
        result.range = distance[nearest_plane];
        result
    }

    /// Ray-Triangle intersection
    pub fn intersect_triangle(&self, triangle: &Triangle) -> IntersectionResult {
        let mut result = IntersectionResult::default();

        let edge1 = triangle.vertices[1] - triangle.vertices[0];
        let edge2 = triangle.vertices[2] - triangle.vertices[0];

        let h = self.ray_direction.cross(edge2);
        let a = edge1.dot(h);

        if a.abs() < 0.0001 {
            return result; // Ray is parallel to triangle
        }

        let f = 1.0 / a;
        let s = self.ray_location - triangle.vertices[0];
        let u = f * s.dot(h);

        if !(0.0..=1.0).contains(&u) {
            return result;
        }

        let q = s.cross(edge1);
        let v = f * self.ray_direction.dot(q);

        if v < 0.0 || u + v > 1.0 {
            return result;
        }

        let t = f * edge2.dot(q);

        if t > 0.0001 {
            result.intersection = self.ray_location + self.ray_direction * t;
            result.normal = triangle.normal;
            result.range = t;
            result.intersects = true;
        }

        result
    }

    /// Sphere intersection test
    pub fn intersect_sphere(&self, center: Vec3, radius: f32) -> IntersectionResult {
        let mut result = IntersectionResult::default();

        let oc = self.ray_location - center;
        let a = self.ray_direction.dot(self.ray_direction);
        let b = 2.0 * oc.dot(self.ray_direction);
        let c = oc.dot(oc) - radius * radius;
        let discriminant = b * b - 4.0 * a * c;

        if discriminant >= 0.0 {
            let sqrt_discriminant = discriminant.sqrt();
            let t1 = (-b - sqrt_discriminant) / (2.0 * a);
            let t2 = (-b + sqrt_discriminant) / (2.0 * a);

            let t = if t1 > 0.0001 { t1 } else { t2 };

            if t > 0.0001 {
                result.intersection = self.ray_location + self.ray_direction * t;
                result.normal = (result.intersection - center).normalize();
                result.range = t;
                result.intersects = true;
            }
        }

        result
    }
}

/// Collision math utilities
pub struct CollisionMath;

impl CollisionMath {
    /// Test if two AABBs intersect
    pub fn aabb_intersect(a: &AABox, b: &AABox) -> bool {
        let a_min = a.center - a.extent;
        let a_max = a.center + a.extent;
        let b_min = b.center - b.extent;
        let b_max = b.center + b.extent;

        a_min.x <= b_max.x
            && a_max.x >= b_min.x
            && a_min.y <= b_max.y
            && a_max.y >= b_min.y
            && a_min.z <= b_max.z
            && a_max.z >= b_min.z
    }

    /// Test if point is inside AABB
    pub fn point_in_aabb(point: Vec3, aabb: &AABox) -> bool {
        let min = aabb.center - aabb.extent;
        let max = aabb.center + aabb.extent;

        point.x >= min.x
            && point.x <= max.x
            && point.y >= min.y
            && point.y <= max.y
            && point.z >= min.z
            && point.z <= max.z
    }

    /// Test if two spheres intersect
    /// CRITICAL: Uses COINCIDENCE_EPSILON for near-coincident sphere detection (C++ parity)
    pub fn sphere_intersect(a_center: Vec3, a_radius: f32, b_center: Vec3, b_radius: f32) -> bool {
        let distance_squared = (a_center - b_center).length_squared();
        let radius_sum = a_radius + b_radius;
        // Add epsilon tolerance to handle near-coincident spheres correctly
        let radius_sum_with_epsilon = radius_sum + COINCIDENCE_EPSILON;
        distance_squared <= radius_sum_with_epsilon * radius_sum_with_epsilon
    }

    /// Test if point is inside sphere
    pub fn point_in_sphere(point: Vec3, center: Vec3, radius: f32) -> bool {
        (point - center).length_squared() <= radius * radius
    }

    /// OBB-Triangle intersection test
    pub fn obb_triangle_intersect(obb: &OBBox, triangle: &Triangle) -> bool {
        // Implement SAT (Separating Axis Theorem) test
        // This is a complex algorithm - implementing a simplified version

        // Test triangle vertices against OBB
        for vertex in &triangle.vertices {
            if Self::point_in_obb(*vertex, obb) {
                return true;
            }
        }

        // Test OBB vertices against triangle plane
        let obb_vertices = Self::get_obb_vertices(obb);
        for vertex in &obb_vertices {
            if Self::point_on_triangle_side(*vertex, triangle) {
                return true;
            }
        }

        false
    }

    /// Check if point is inside OBB
    pub fn point_in_obb(point: Vec3, obb: &OBBox) -> bool {
        let local_point = point - obb.center;

        for i in 0..3 {
            let projection = local_point.dot(obb.basis[i]);
            if projection.abs() > obb.extent[i] {
                return false;
            }
        }

        true
    }

    /// Get OBB vertices
    fn get_obb_vertices(obb: &OBBox) -> [Vec3; 8] {
        let mut vertices = [Vec3::ZERO; 8];

        for i in 0..8 {
            let signs = [
                if i & 1 != 0 { 1.0 } else { -1.0 },
                if i & 2 != 0 { 1.0 } else { -1.0 },
                if i & 4 != 0 { 1.0 } else { -1.0 },
            ];

            vertices[i] = obb.center
                + obb.basis[0] * (obb.extent.x * signs[0])
                + obb.basis[1] * (obb.extent.y * signs[1])
                + obb.basis[2] * (obb.extent.z * signs[2]);
        }

        vertices
    }

    /// Check if point is on the same side of triangle
    fn point_on_triangle_side(point: Vec3, triangle: &Triangle) -> bool {
        let plane_distance = triangle.normal.dot(point - triangle.vertices[0]);
        plane_distance.abs() < 0.001 // On plane tolerance
    }
}

/// Ray collision test for AABTree
pub struct RayCollisionTest {
    pub ray_origin: Vec3,
    pub ray_direction: Vec3,
    pub result: CastResult,
}

impl RayCollisionTest {
    pub fn new(origin: Vec3, direction: Vec3) -> Self {
        Self {
            ray_origin: origin,
            ray_direction: direction.normalize(),
            result: CastResult::default(),
        }
    }

    /// Cull test against AABB bounds
    pub fn cull(&self, min: Vec3, max: Vec3) -> bool {
        let intersection = IntersectionClass::new();
        let result = intersection.intersect_box(min, max);
        !result.intersects
    }
}
