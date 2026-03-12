//! Geometry-Specific Collision Detection
//!
//! This module provides collision detection functionality specifically
//! for geometric primitives like meshes, triangles, and complex shapes.
//! It extends the basic collision detection from ww3d-collision with
//! geometry-specific optimizations and algorithms.

use crate::*;
use glam::Vec3;

/// Triangle collision mesh for efficient collision detection
#[derive(Debug, Clone)]
pub struct CollisionMesh {
    pub vertices: Vec<Vec3>,
    pub triangles: Vec<[u32; 3]>,
    pub bounding_box: AABox,
}

impl CollisionMesh {
    /// Create a new collision mesh from vertices and triangles
    pub fn new(vertices: Vec<Vec3>, triangles: Vec<[u32; 3]>) -> Self {
        let bounding_box = Self::compute_bounding_box(&vertices);
        Self {
            vertices,
            triangles,
            bounding_box,
        }
    }

    /// Compute AABB for the mesh
    fn compute_bounding_box(vertices: &[Vec3]) -> AABox {
        if vertices.is_empty() {
            return AABox::new(Vec3::ZERO, Vec3::ZERO);
        }

        let mut min = vertices[0];
        let mut max = vertices[0];

        for vertex in vertices {
            min = min.min(*vertex);
            max = max.max(*vertex);
        }

        let center = (min + max) / 2.0;
        let extent = (max - min) / 2.0;
        AABox::new(center, extent)
    }

    /// Test ray intersection with the mesh
    pub fn ray_intersect(&self, ray_origin: Vec3, ray_direction: Vec3) -> Option<RayIntersection> {
        let mut closest_hit: Option<RayIntersection> = None;
        let normalized_dir = ray_direction.normalize();

        for triangle_indices in &self.triangles {
            let v0 = self.vertices[triangle_indices[0] as usize];
            let v1 = self.vertices[triangle_indices[1] as usize];
            let v2 = self.vertices[triangle_indices[2] as usize];

            let triangle = Triangle::new(v0, v1, v2);

            if let Some(intersection) = crate::intersection::ray_triangle_intersection(
                &Ray::new(ray_origin, normalized_dir),
                &triangle,
                false,
            ) {
                let distance = (intersection.point - ray_origin).length();

                if closest_hit.is_none() || distance < closest_hit.as_ref().unwrap().distance {
                    closest_hit = Some(RayIntersection {
                        point: intersection.point,
                        distance,
                        triangle_index: self
                            .triangles
                            .iter()
                            .position(|&t| t == *triangle_indices)
                            .unwrap(),
                        barycentric_coords: barycentric_coordinates(intersection.point, &triangle),
                    });
                }
            }
        }

        closest_hit
    }

    /// Test AABB intersection with the mesh
    pub fn aabb_intersect(&self, aabb: &AABox) -> bool {
        // Quick AABB vs AABB test first
        if !self.bounding_box.intersects_aabox(aabb) {
            return false;
        }

        // Test triangles against AABB
        for triangle_indices in &self.triangles {
            let v0 = self.vertices[triangle_indices[0] as usize];
            let v1 = self.vertices[triangle_indices[1] as usize];
            let v2 = self.vertices[triangle_indices[2] as usize];

            let triangle = Triangle::new(v0, v1, v2);
            if triangle_aabb_intersect(&triangle, aabb) {
                return true;
            }
        }

        false
    }

    /// Test sphere intersection with the mesh
    pub fn sphere_intersect(&self, sphere: &Sphere) -> bool {
        // Quick sphere vs AABB test first
        if !sphere_aabb_intersect(sphere, &self.bounding_box) {
            return false;
        }

        // Test triangles against sphere
        for triangle_indices in &self.triangles {
            let v0 = self.vertices[triangle_indices[0] as usize];
            let v1 = self.vertices[triangle_indices[1] as usize];
            let v2 = self.vertices[triangle_indices[2] as usize];

            let triangle = Triangle::new(v0, v1, v2);
            if triangle_sphere_intersect(&triangle, sphere) {
                return true;
            }
        }

        false
    }
}

/// Ray intersection result
#[derive(Debug, Clone, Copy)]
pub struct RayIntersection {
    pub point: Vec3,
    pub distance: f32,
    pub triangle_index: usize,
    pub barycentric_coords: (f32, f32, f32),
}

/// Collision detection functions for geometric primitives

/// Test ray-triangle intersection
pub fn ray_triangle_intersect(ray: &Ray, triangle: &Triangle, cull_backface: bool) -> Option<Vec3> {
    let edge1 = triangle.v1 - triangle.v0;
    let edge2 = triangle.v2 - triangle.v0;

    let h = ray.direction.cross(edge2);
    let a = edge1.dot(h);

    if cull_backface && a < EPSILON {
        return None;
    }
    if a.abs() < EPSILON {
        return None;
    }

    let f = 1.0 / a;
    let s = ray.origin - triangle.v0;
    let u = f * s.dot(h);

    if u < 0.0 || u > 1.0 {
        return None;
    }

    let q = s.cross(edge1);
    let v = f * ray.direction.dot(q);

    if v < 0.0 || u + v > 1.0 {
        return None;
    }

    let t = f * edge2.dot(q);
    if t > EPSILON {
        Some(ray.origin + ray.direction * t)
    } else {
        None
    }
}

/// Test triangle-AABB intersection using SAT (Separating Axis Theorem)
pub fn triangle_aabb_intersect(triangle: &Triangle, aabb: &AABox) -> bool {
    let box_center = aabb.center;
    let box_extent = aabb.extent;

    // Translate triangle to AABB space
    let v0 = triangle.v0 - box_center;
    let v1 = triangle.v1 - box_center;
    let v2 = triangle.v2 - box_center;

    // Test AABB planes
    let min_x = v0.x.min(v1.x).min(v2.x);
    let max_x = v0.x.max(v1.x).max(v2.x);
    if max_x < -box_extent.x || min_x > box_extent.x {
        return false;
    }

    let min_y = v0.y.min(v1.y).min(v2.y);
    let max_y = v0.y.max(v1.y).max(v2.y);
    if max_y < -box_extent.y || min_y > box_extent.y {
        return false;
    }

    let min_z = v0.z.min(v1.z).min(v2.z);
    let max_z = v0.z.max(v1.z).max(v2.z);
    if max_z < -box_extent.z || min_z > box_extent.z {
        return false;
    }

    // Test triangle normal
    let normal = (v1 - v0).cross(v2 - v0);
    if !test_axis_projection(&[v0, v1, v2], normal, box_extent) {
        return false;
    }

    // Test AABB axes
    let axes = [
        Vec3::new(1.0, 0.0, 0.0),
        Vec3::new(0.0, 1.0, 0.0),
        Vec3::new(0.0, 0.0, 1.0),
    ];

    for axis in &axes {
        if !test_axis_projection(&[v0, v1, v2], *axis, box_extent) {
            return false;
        }
    }

    // Test cross products of triangle edges and AABB axes
    let edges = [v1 - v0, v2 - v1, v0 - v2];
    for edge in &edges {
        for axis in &axes {
            let cross = edge.cross(*axis);
            if !test_axis_projection(&[v0, v1, v2], cross, box_extent) {
                return false;
            }
        }
    }

    true
}

/// Test triangle-OBBox intersection and return a representative contact point when intersecting.
pub fn triangle_obbox_intersection(triangle: &Triangle, obbox: &OBBox) -> Option<Vec3> {
    let inv_basis = obbox.basis.inverse();

    let to_local = |point: Vec3| -> Vec3 { inv_basis.transform_point3(point - obbox.center) };

    let v0_local = to_local(triangle.v0);
    let v1_local = to_local(triangle.v1);
    let v2_local = to_local(triangle.v2);

    let local_triangle = Triangle::new(v0_local, v1_local, v2_local);
    let local_aabb = AABox::new(Vec3::ZERO, obbox.extent);

    if !triangle_aabb_intersect(&local_triangle, &local_aabb) {
        return None;
    }

    // Find a stable contact point in local space by clamping the closest point on the triangle.
    let closest_local = closest_point_on_triangle(Vec3::ZERO, &local_triangle);
    let clamped_local = Vec3::new(
        closest_local
            .x
            .clamp(-local_aabb.extent.x, local_aabb.extent.x),
        closest_local
            .y
            .clamp(-local_aabb.extent.y, local_aabb.extent.y),
        closest_local
            .z
            .clamp(-local_aabb.extent.z, local_aabb.extent.z),
    );

    let contact_local = if local_aabb.contains_point(closest_local) {
        closest_local
    } else {
        clamped_local
    };

    let contact_world = obbox.basis.transform_point3(contact_local) + obbox.center;
    Some(contact_world)
}

/// Test triangle-sphere intersection
pub fn triangle_sphere_intersect(triangle: &Triangle, sphere: &Sphere) -> bool {
    // Find closest point on triangle to sphere center
    let closest_point = closest_point_on_triangle(sphere.center, triangle);

    // Check if closest point is within sphere
    (closest_point - sphere.center).length_squared() <= sphere.radius * sphere.radius
}

/// Test sphere-AABB intersection
pub fn sphere_aabb_intersect(sphere: &Sphere, aabb: &AABox) -> bool {
    let closest_point = Vec3::new(
        sphere
            .center
            .x
            .clamp(aabb.center.x - aabb.extent.x, aabb.center.x + aabb.extent.x),
        sphere
            .center
            .y
            .clamp(aabb.center.y - aabb.extent.y, aabb.center.y + aabb.extent.y),
        sphere
            .center
            .z
            .clamp(aabb.center.z - aabb.extent.z, aabb.center.z + aabb.extent.z),
    );

    (closest_point - sphere.center).length_squared() <= sphere.radius * sphere.radius
}

/// Helper function for SAT axis testing
fn test_axis_projection(vertices: &[Vec3], axis: Vec3, box_extent: Vec3) -> bool {
    if axis.length_squared() < EPSILON {
        return true;
    }

    let normalized_axis = axis.normalize();
    let mut min_proj = f32::INFINITY;
    let mut max_proj = f32::NEG_INFINITY;

    for vertex in vertices {
        let proj = vertex.dot(normalized_axis);
        min_proj = min_proj.min(proj);
        max_proj = max_proj.max(proj);
    }

    // Test against AABB projections on this axis
    let box_min = -box_extent.dot(normalized_axis.abs());
    let box_max = box_extent.dot(normalized_axis.abs());

    !(max_proj < box_min || min_proj > box_max)
}

/// Find closest point on triangle to given point
pub fn closest_point_on_triangle(point: Vec3, triangle: &Triangle) -> Vec3 {
    // Check if point is above the triangle
    let (u, v, w) = barycentric_coordinates(point, triangle);
    if u >= 0.0 && v >= 0.0 && w >= 0.0 {
        return point;
    }

    // Check closest point on each edge
    let edges = [
        LineSegment::new(triangle.v0, triangle.v1),
        LineSegment::new(triangle.v1, triangle.v2),
        LineSegment::new(triangle.v2, triangle.v0),
    ];

    let mut closest_point = triangle.v0;
    let mut min_distance_squared = (point - triangle.v0).length_squared();

    for edge in &edges {
        let candidate = closest_point_on_line_segment(point, edge);
        let distance_squared = (point - candidate).length_squared();
        if distance_squared < min_distance_squared {
            min_distance_squared = distance_squared;
            closest_point = candidate;
        }
    }

    closest_point
}

/// Find closest point on line segment to given point
pub fn closest_point_on_line_segment(point: Vec3, segment: &LineSegment) -> Vec3 {
    let direction = segment.end - segment.start;
    let length_squared = direction.length_squared();

    if length_squared < EPSILON {
        return segment.start;
    }

    let t = ((point - segment.start).dot(direction) / length_squared).clamp(0.0, 1.0);
    segment.start + direction * t
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_collision_mesh_creation() {
        let vertices = vec![
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
        ];
        let triangles = vec![[0, 1, 2]];

        let mesh = CollisionMesh::new(vertices, triangles);
        assert_eq!(mesh.vertices.len(), 3);
        assert_eq!(mesh.triangles.len(), 1);
    }

    #[test]
    fn test_ray_triangle_intersection() {
        let triangle = Triangle::new(
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
        );

        let ray = Ray::new(Vec3::new(0.2, 0.2, 1.0), Vec3::new(0.0, 0.0, -1.0));

        let intersection = ray_triangle_intersect(&ray, &triangle, false);
        assert!(intersection.is_some());

        let point = intersection.unwrap();
        assert!((point.z - 0.0).abs() < EPSILON);
    }

    #[test]
    fn test_triangle_aabb_intersection() {
        let triangle = Triangle::new(
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
        );

        let aabb = AABox::new(Vec3::ZERO, Vec3::new(0.5, 0.5, 0.5));
        assert!(triangle_aabb_intersect(&triangle, &aabb));

        let aabb_outside = AABox::new(Vec3::new(2.0, 2.0, 2.0), Vec3::new(0.5, 0.5, 0.5));
        assert!(!triangle_aabb_intersect(&triangle, &aabb_outside));
    }
}
