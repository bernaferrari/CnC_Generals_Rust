//! Advanced Bounding Volume Operations
//!
//! This module provides advanced bounding volume calculations,
//! optimizations, and utilities beyond the basic bounding volumes.

use crate::collision::sphere_aabb_intersect as sphere_aabb_intersect_fn;
use crate::spatial_partitioning::SpatialObject;
use crate::*;
use glam::{Mat4, Vec3};

/// Advanced bounding volume utilities
pub struct BoundingVolumeUtils;

impl BoundingVolumeUtils {
    /// Compute tight bounding sphere for a set of points using Ritter's algorithm
    pub fn compute_bounding_sphere(points: &[Vec3]) -> Sphere {
        if points.is_empty() {
            return Sphere::new(Vec3::ZERO, 0.0);
        }

        // Start with sphere containing first two points
        let mut sphere = if points.len() == 1 {
            Sphere::new(points[0], 0.0)
        } else {
            let center = (points[0] + points[1]) / 2.0;
            let radius = (points[1] - points[0]).length() / 2.0;
            Sphere::new(center, radius)
        };

        // Iteratively add remaining points
        for &point in &points[2..] {
            sphere = Self::expand_sphere_to_point(sphere, point);
        }

        // Refine the sphere using Ritter's algorithm
        sphere = Self::ritter_refinement(points, sphere);

        sphere
    }

    /// Expand sphere to include a point
    pub fn expand_sphere_to_point(sphere: Sphere, point: Vec3) -> Sphere {
        let distance = (point - sphere.center).length();

        if distance <= sphere.radius {
            return sphere; // Point is already inside
        }

        let new_radius = (sphere.radius + distance) / 2.0;
        let direction = (point - sphere.center).normalize();
        let new_center = sphere.center + direction * (distance - sphere.radius) / 2.0;

        Sphere::new(new_center, new_radius)
    }

    /// Ritter's algorithm for sphere refinement
    fn ritter_refinement(points: &[Vec3], initial_sphere: Sphere) -> Sphere {
        let mut sphere = initial_sphere;

        loop {
            let mut max_distance = 0.0;
            let mut farthest_point = Vec3::ZERO;

            // Find point farthest from current sphere
            for &point in points {
                let distance = (point - sphere.center).length();
                if distance > max_distance {
                    max_distance = distance;
                    farthest_point = point;
                }
            }

            if max_distance <= sphere.radius * 1.0001 {
                break; // Sphere is good enough
            }

            // Expand sphere to include farthest point
            sphere = Self::expand_sphere_to_point(sphere, farthest_point);
        }

        sphere
    }

    /// Compute optimal AABB for a set of points
    pub fn compute_optimal_aabb(points: &[Vec3]) -> AABox {
        if points.is_empty() {
            return AABox::new(Vec3::ZERO, Vec3::ZERO);
        }

        let mut min = points[0];
        let mut max = points[0];

        for &point in points {
            min = min.min(point);
            max = max.max(point);
        }

        let center = (min + max) / 2.0;
        let extent = (max - min) / 2.0;

        AABox::new(center, extent)
    }

    /// Compute oriented bounding box using principal component analysis
    pub fn compute_obb_pca(points: &[Vec3]) -> OBBox {
        if points.is_empty() {
            return OBBox::new(Vec3::ZERO, Vec3::ZERO, Mat4::IDENTITY);
        }

        // Compute centroid
        let centroid = points.iter().fold(Vec3::ZERO, |acc, &p| acc + p) / points.len() as f32;

        // Compute covariance matrix
        let mut covariance = [[0.0; 3]; 3];
        for &point in points {
            let diff = point - centroid;
            covariance[0][0] += diff.x * diff.x;
            covariance[0][1] += diff.x * diff.y;
            covariance[0][2] += diff.x * diff.z;
            covariance[1][0] += diff.y * diff.x;
            covariance[1][1] += diff.y * diff.y;
            covariance[1][2] += diff.y * diff.z;
            covariance[2][0] += diff.z * diff.x;
            covariance[2][1] += diff.z * diff.y;
            covariance[2][2] += diff.z * diff.z;
        }

        // Normalize covariance
        let n = points.len() as f32;
        for i in 0..3 {
            for j in 0..3 {
                covariance[i][j] /= n;
            }
        }

        // Find eigenvectors (simplified - using power iteration)
        let basis = Self::compute_basis_from_covariance(covariance);

        // Transform points to local space
        let mut local_points = Vec::new();
        for &point in points {
            let local = basis.inverse().transform_point3(point - centroid);
            local_points.push(local);
        }

        // Compute AABB in local space
        let local_aabb = Self::compute_optimal_aabb(&local_points);

        OBBox::new(centroid, local_aabb.extent, basis)
    }

    /// Simplified eigenvector computation using power iteration
    fn compute_basis_from_covariance(_covariance: [[f32; 3]; 3]) -> Mat4 {
        // This is a simplified implementation
        // In practice, you'd use a proper eigenvalue solver

        // Start with identity basis
        let basis = Mat4::IDENTITY;

        // For now, just return identity (simplified)
        // A full implementation would compute eigenvectors
        basis
    }

    /// Merge two AABBs
    pub fn merge_aabbs(aabb1: &AABox, aabb2: &AABox) -> AABox {
        let min1 = aabb1.center - aabb1.extent;
        let max1 = aabb1.center + aabb1.extent;
        let min2 = aabb2.center - aabb2.extent;
        let max2 = aabb2.center + aabb2.extent;

        let merged_min = min1.min(min2);
        let merged_max = max1.max(max2);

        let center = (merged_min + merged_max) / 2.0;
        let extent = (merged_max - merged_min) / 2.0;

        AABox::new(center, extent)
    }

    /// Merge two spheres
    pub fn merge_spheres(sphere1: &Sphere, sphere2: &Sphere) -> Sphere {
        let distance = (sphere2.center - sphere1.center).length();

        if distance <= (sphere1.radius - sphere2.radius).abs() {
            // One sphere contains the other
            if sphere1.radius >= sphere2.radius {
                *sphere1
            } else {
                *sphere2
            }
        } else {
            // Spheres are separate
            let radius = (sphere1.radius + sphere2.radius + distance) / 2.0;
            let direction = (sphere2.center - sphere1.center).normalize();
            let center = sphere1.center + direction * (radius - sphere1.radius);

            Sphere::new(center, radius)
        }
    }

    /// Transform AABB by matrix
    pub fn transform_aabb(aabb: &AABox, transform: &Mat4) -> AABox {
        // Transform all 8 corners and find new AABB
        let corners = [
            aabb.center + Vec3::new(-aabb.extent.x, -aabb.extent.y, -aabb.extent.z),
            aabb.center + Vec3::new(aabb.extent.x, -aabb.extent.y, -aabb.extent.z),
            aabb.center + Vec3::new(-aabb.extent.x, aabb.extent.y, -aabb.extent.z),
            aabb.center + Vec3::new(aabb.extent.x, aabb.extent.y, -aabb.extent.z),
            aabb.center + Vec3::new(-aabb.extent.x, -aabb.extent.y, aabb.extent.z),
            aabb.center + Vec3::new(aabb.extent.x, -aabb.extent.y, aabb.extent.z),
            aabb.center + Vec3::new(-aabb.extent.x, aabb.extent.y, aabb.extent.z),
            aabb.center + Vec3::new(aabb.extent.x, aabb.extent.y, aabb.extent.z),
        ];

        let mut transformed_corners = Vec::new();
        for corner in &corners {
            let transformed = transform.transform_point3(*corner);
            transformed_corners.push(transformed);
        }

        Self::compute_optimal_aabb(&transformed_corners)
    }

    /// Transform sphere by matrix
    pub fn transform_sphere(sphere: &Sphere, transform: &Mat4) -> Sphere {
        let new_center = transform.transform_point3(sphere.center);

        // For uniform scaling, we can compute the new radius
        // For general transforms, this is an approximation
        let scale_x = transform.col(0).truncate().length();
        let scale_y = transform.col(1).truncate().length();
        let scale_z = transform.col(2).truncate().length();
        let max_scale = scale_x.max(scale_y).max(scale_z);

        Sphere::new(new_center, sphere.radius * max_scale)
    }

    /// Compute bounding volume hierarchy for efficient collision detection
    pub fn build_bvh(objects: &[SpatialObject]) -> BVHNode {
        if objects.is_empty() {
            return BVHNode::leaf(AABox::new(Vec3::ZERO, Vec3::ZERO), Vec::new());
        }

        if objects.len() == 1 {
            return BVHNode::leaf(objects[0].bounds, vec![objects[0].clone()]);
        }

        // Find the axis with the largest extent
        let bounds = objects.iter().fold(objects[0].bounds, |acc, obj| {
            Self::merge_aabbs(&acc, &obj.bounds)
        });
        let extent = bounds.extent;

        let axis = if extent.x >= extent.y && extent.x >= extent.z {
            0 // X axis
        } else if extent.y >= extent.z {
            1 // Y axis
        } else {
            2 // Z axis
        };

        // Sort objects along the chosen axis
        let mut sorted_objects = objects.to_vec();
        sorted_objects.sort_by(|a, b| {
            let a_center = a.position[axis];
            let b_center = b.position[axis];
            a_center.partial_cmp(&b_center).unwrap()
        });

        // Split into two halves
        let mid = sorted_objects.len() / 2;
        let left_objects = &sorted_objects[..mid];
        let right_objects = &sorted_objects[mid..];

        let left_child = Self::build_bvh(left_objects);
        let right_child = Self::build_bvh(right_objects);

        let bounds = Self::merge_aabbs(left_child.bounds(), right_child.bounds());

        BVHNode::internal(bounds, Box::new(left_child), Box::new(right_child))
    }

    /// Compute surface area of AABB
    pub fn aabb_surface_area(aabb: &AABox) -> f32 {
        let e = aabb.extent;
        8.0 * (e.x * e.y + e.y * e.z + e.z * e.x)
    }

    /// Compute volume of AABB
    pub fn aabb_volume(aabb: &AABox) -> f32 {
        let e = aabb.extent;
        8.0 * e.x * e.y * e.z
    }

    /// Compute surface area of sphere
    pub fn sphere_surface_area(sphere: &Sphere) -> f32 {
        4.0 * PI * sphere.radius * sphere.radius
    }

    /// Compute volume of sphere
    pub fn sphere_volume(sphere: &Sphere) -> f32 {
        (4.0 / 3.0) * PI * sphere.radius * sphere.radius * sphere.radius
    }
}

/// Bounding Volume Hierarchy node
#[derive(Debug, Clone)]
pub enum BVHNode {
    Leaf {
        bounds: AABox,
        objects: Vec<SpatialObject>,
    },
    Internal {
        bounds: AABox,
        left: Box<BVHNode>,
        right: Box<BVHNode>,
    },
}

impl BVHNode {
    pub fn leaf(bounds: AABox, objects: Vec<SpatialObject>) -> Self {
        BVHNode::Leaf { bounds, objects }
    }

    pub fn internal(bounds: AABox, left: Box<BVHNode>, right: Box<BVHNode>) -> Self {
        BVHNode::Internal {
            bounds,
            left,
            right,
        }
    }

    pub fn bounds(&self) -> &AABox {
        match self {
            BVHNode::Leaf { bounds, .. } => bounds,
            BVHNode::Internal { bounds, .. } => bounds,
        }
    }

    /// Query objects within an AABB
    pub fn query_aabb(&self, query_bounds: &AABox) -> Vec<&SpatialObject> {
        let mut result = Vec::new();

        if !self.bounds().intersects_aabox(query_bounds) {
            return result;
        }

        match self {
            BVHNode::Leaf { objects, .. } => {
                for object in objects {
                    if object.bounds.intersects_aabox(query_bounds) {
                        result.push(object);
                    }
                }
            }
            BVHNode::Internal { left, right, .. } => {
                result.extend(left.query_aabb(query_bounds));
                result.extend(right.query_aabb(query_bounds));
            }
        }

        result
    }

    /// Query objects within a sphere
    pub fn query_sphere(&self, sphere: &Sphere) -> Vec<&SpatialObject> {
        let mut result = Vec::new();

        if !sphere_aabb_intersect_fn(sphere, self.bounds()) {
            return result;
        }

        match self {
            BVHNode::Leaf { objects, .. } => {
                for object in objects {
                    if sphere_aabb_intersect_fn(sphere, &object.bounds) {
                        result.push(object);
                    }
                }
            }
            BVHNode::Internal { left, right, .. } => {
                result.extend(left.query_sphere(sphere));
                result.extend(right.query_sphere(sphere));
            }
        }

        result
    }
}

/// Helper function for sphere-AABB intersection (should be in intersection.rs but needed here)
#[allow(dead_code)] // C++ parity
fn sphere_aabb_intersect(sphere: &Sphere, aabb: &AABox) -> bool {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bounding_sphere_computation() {
        let points = vec![
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::new(0.0, 0.0, 1.0),
        ];

        let sphere = BoundingVolumeUtils::compute_bounding_sphere(&points);
        assert!(sphere.radius > 0.0);

        // Check that all points are inside the sphere
        for point in points {
            assert!((point - sphere.center).length() <= sphere.radius + EPSILON);
        }
    }

    #[test]
    fn test_aabb_merge() {
        let aabb1 = AABox::new(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0));
        let aabb2 = AABox::new(Vec3::new(2.0, 0.0, 0.0), Vec3::new(1.0, 1.0, 1.0));

        let merged = BoundingVolumeUtils::merge_aabbs(&aabb1, &aabb2);

        assert_eq!(merged.center, Vec3::new(1.0, 0.0, 0.0));
        assert_eq!(merged.extent, Vec3::new(2.0, 1.0, 1.0));
    }

    #[test]
    fn test_sphere_merge() {
        let sphere1 = Sphere::new(Vec3::ZERO, 1.0);
        let sphere2 = Sphere::new(Vec3::new(2.0, 0.0, 0.0), 1.0);

        let merged = BoundingVolumeUtils::merge_spheres(&sphere1, &sphere2);

        assert!((merged.center - Vec3::new(1.0, 0.0, 0.0)).length() < EPSILON);
        assert!((merged.radius - 2.0).abs() < EPSILON);
    }

    #[test]
    fn test_bvh_construction() {
        let objects = vec![
            SpatialObject::new(
                1,
                Vec3::new(0.0, 0.0, 0.0),
                AABox::new(Vec3::ZERO, Vec3::new(0.5, 0.5, 0.5)),
            ),
            SpatialObject::new(
                2,
                Vec3::new(2.0, 0.0, 0.0),
                AABox::new(Vec3::ZERO, Vec3::new(0.5, 0.5, 0.5)),
            ),
        ];

        let bvh = BoundingVolumeUtils::build_bvh(&objects);

        match bvh {
            BVHNode::Internal { .. } => {
                // Should create an internal node for two objects
            }
            _ => panic!("Expected internal node"),
        }
    }
}
