//! Plane - Plane collision primitive
//!
//! This module implements plane collision primitives for spatial partitioning
//! and collision detection, converted from the original PlaneClass.

use glam::{Mat4, Vec3};

/// Plane collision primitive defined by normal and distance from origin
#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(C)]
pub struct PlaneClass {
    /// Normal vector of the plane (should be normalized)
    pub normal: Vec3,
    /// Distance from origin to plane along normal
    pub distance: f32,
}

impl PlaneClass {
    /// Create a new plane
    pub fn new(normal: Vec3, distance: f32) -> Self {
        let length = normal.length();
        let normalized_normal = normal.normalize();
        let normalized_distance = if length > 0.0 {
            distance / length
        } else {
            distance
        };
        Self {
            normal: normalized_normal,
            distance: normalized_distance,
        }
    }

    /// Create plane from point and normal
    pub fn from_point_normal(point: Vec3, normal: Vec3) -> Self {
        let normalized_normal = normal.normalize();
        let distance = -normalized_normal.dot(point);
        Self {
            normal: normalized_normal,
            distance,
        }
    }

    /// Create plane from three points
    pub fn from_points(p0: Vec3, p1: Vec3, p2: Vec3) -> Self {
        let v1 = p1 - p0;
        let v2 = p2 - p0;
        let normal = v1.cross(v2).normalize();
        Self::from_point_normal(p0, normal)
    }

    /// Create plane from normal and a point on the plane
    pub fn from_normal_point(normal: Vec3, point: Vec3) -> Self {
        Self::from_point_normal(point, normal)
    }

    /// Get the normal vector
    pub fn normal(&self) -> Vec3 {
        self.normal
    }

    /// Get the distance from origin
    pub fn distance(&self) -> f32 {
        self.distance
    }

    /// Set the normal vector (will be normalized)
    pub fn set_normal(&mut self, normal: Vec3) {
        self.normal = normal.normalize();
    }

    /// Set the distance from origin
    pub fn set_distance(&mut self, distance: f32) {
        self.distance = distance;
    }

    /// Initialize plane
    pub fn init(&mut self, normal: Vec3, distance: f32) {
        self.normal = normal.normalize();
        self.distance = distance;
    }

    /// Initialize from point and normal
    pub fn init_from_point_normal(&mut self, point: Vec3, normal: Vec3) {
        *self = Self::from_point_normal(point, normal);
    }

    /// Initialize from three points
    pub fn init_from_points(&mut self, p0: Vec3, p1: Vec3, p2: Vec3) {
        *self = Self::from_points(p0, p1, p2);
    }

    /// Calculate signed distance from point to plane
    pub fn distance_to_point(&self, point: Vec3) -> f32 {
        self.normal.dot(point) + self.distance
    }

    /// Project point onto the plane
    pub fn project_point(&self, point: Vec3) -> Vec3 {
        let distance = self.distance_to_point(point);
        point - self.normal * distance
    }

    /// Classify point relative to plane
    pub fn classify_point(&self, point: Vec3) -> PlaneClassification {
        let dist = self.distance_to_point(point);
        if dist > 0.01 {
            PlaneClassification::Front
        } else if dist < -0.01 {
            PlaneClassification::Back
        } else {
            PlaneClassification::OnPlane
        }
    }

    /// Classify point with epsilon
    pub fn classify_point_epsilon(&self, point: Vec3, epsilon: f32) -> PlaneClassification {
        let dist = self.distance_to_point(point);
        if dist > epsilon {
            PlaneClassification::Front
        } else if dist < -epsilon {
            PlaneClassification::Back
        } else {
            PlaneClassification::OnPlane
        }
    }

    /// Check if point is in front of plane
    pub fn is_in_front(&self, point: Vec3) -> bool {
        self.distance_to_point(point) > 0.0
    }

    /// Check if point is behind plane
    pub fn is_behind(&self, point: Vec3) -> bool {
        self.distance_to_point(point) < 0.0
    }

    /// Check if point is on plane
    pub fn is_on_plane(&self, point: Vec3) -> bool {
        self.distance_to_point(point).abs() < 0.01
    }

    /// Check if point is on plane with custom epsilon
    pub fn is_on_plane_epsilon(&self, point: Vec3, epsilon: f32) -> bool {
        self.distance_to_point(point).abs() < epsilon
    }

    /// Find intersection with line segment
    pub fn intersect_line_segment(&self, start: Vec3, end: Vec3) -> Option<Vec3> {
        let start_dist = self.distance_to_point(start);
        let end_dist = self.distance_to_point(end);

        // Check if segment crosses the plane
        if (start_dist > 0.0) == (end_dist > 0.0) && start_dist.abs() > 0.01 {
            return None; // Both points on same side
        }

        // Calculate intersection point
        let dist_diff = start_dist - end_dist;
        if dist_diff.abs() < 0.0001 {
            // Segment is parallel to plane, return midpoint if on plane
            if start_dist.abs() < 0.01 {
                return Some((start + end) * 0.5);
            } else {
                return None;
            }
        }

        let t = start_dist / dist_diff;
        let intersection = start + (end - start) * t;

        Some(intersection)
    }

    /// Find intersection with ray
    pub fn intersect_ray(&self, origin: Vec3, direction: Vec3) -> Option<f32> {
        let denom = self.normal.dot(direction);

        if denom.abs() < 0.0001 {
            return None; // Ray is parallel to plane
        }

        let t = -(self.normal.dot(origin) + self.distance) / denom;

        if t >= 0.0 {
            Some(t)
        } else {
            None
        }
    }

    /// Flip the plane (reverse normal and distance)
    pub fn flip(&mut self) {
        self.normal = -self.normal;
        self.distance = -self.distance;
    }

    /// Get flipped plane
    pub fn flipped(&self) -> Self {
        Self {
            normal: -self.normal,
            distance: -self.distance,
        }
    }

    /// Normalize the plane (ensure normal is unit length)
    pub fn normalize(&mut self) {
        let length = self.normal.length();
        if length > 0.0 {
            self.normal /= length;
            self.distance /= length;
        }
    }

    /// Get normalized plane
    pub fn normalized(&self) -> Self {
        let mut result = *self;
        result.normalize();
        result
    }

    /// Check if plane is normalized
    pub fn is_normalized(&self) -> bool {
        (self.normal.length() - 1.0).abs() < 0.01
    }

    /// Transform plane by matrix
    pub fn transform(&mut self, transform: &Mat4) {
        // Transform the normal by the inverse transpose
        let inverse_transpose = transform.inverse().transpose();
        let new_normal = (inverse_transpose * self.normal.extend(0.0)).truncate();

        // Transform a point on the plane
        let point_on_plane = -self.normal * self.distance;
        let transformed_point = transform.transform_point3(point_on_plane);

        // Reconstruct plane from transformed normal and point
        let normalized_normal = new_normal.normalize();
        let new_distance = -normalized_normal.dot(transformed_point);

        self.normal = normalized_normal;
        self.distance = new_distance;
    }

    /// Get transformed plane
    pub fn transformed(&self, transform: &Mat4) -> Self {
        let mut result = *self;
        result.transform(transform);
        result
    }

    /// Calculate distance between parallel planes
    pub fn distance_to_plane(&self, other: &PlaneClass) -> Option<f32> {
        if self.normal.dot(other.normal).abs() < 0.9999 {
            return None; // Planes are not parallel
        }

        Some((self.distance - other.distance).abs())
    }

    /// Check if two planes are parallel
    pub fn is_parallel_to(&self, other: &PlaneClass) -> bool {
        self.normal.cross(other.normal).length_squared() < 0.0001
    }

    /// Check if two planes are coincident (same plane)
    pub fn is_coincident_with(&self, other: &PlaneClass) -> bool {
        self.is_parallel_to(other) && (self.distance - other.distance).abs() < 0.01
    }

    /// Get angle between planes
    pub fn angle_with(&self, other: &PlaneClass) -> f32 {
        self.normal.dot(other.normal).acos()
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PlaneClassification {
    Front,
    Back,
    OnPlane,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plane_creation() {
        let plane = PlaneClass::new(Vec3::new(0.0, 1.0, 0.0), 5.0);
        assert_eq!(plane.normal, Vec3::new(0.0, 1.0, 0.0));
        assert_eq!(plane.distance, 5.0);
    }

    #[test]
    fn test_plane_from_point_normal() {
        let point = Vec3::new(0.0, 5.0, 0.0);
        let normal = Vec3::new(0.0, 1.0, 0.0);
        let plane = PlaneClass::from_point_normal(point, normal);

        assert_eq!(plane.normal, Vec3::new(0.0, 1.0, 0.0));
        assert_eq!(plane.distance, -5.0);
    }

    #[test]
    fn test_plane_distance_to_point() {
        let plane = PlaneClass::from_point_normal(Vec3::ZERO, Vec3::new(0.0, 1.0, 0.0));

        assert_eq!(plane.distance_to_point(Vec3::new(0.0, 5.0, 0.0)), 5.0);
        assert_eq!(plane.distance_to_point(Vec3::new(0.0, -3.0, 0.0)), -3.0);
        assert_eq!(plane.distance_to_point(Vec3::ZERO), 0.0);
    }

    #[test]
    fn test_plane_classification() {
        let plane = PlaneClass::from_point_normal(Vec3::ZERO, Vec3::new(0.0, 1.0, 0.0));

        assert_eq!(
            plane.classify_point(Vec3::new(0.0, 1.0, 0.0)),
            PlaneClassification::Front
        );
        assert_eq!(
            plane.classify_point(Vec3::new(0.0, -1.0, 0.0)),
            PlaneClassification::Back
        );
        assert_eq!(
            plane.classify_point(Vec3::ZERO),
            PlaneClassification::OnPlane
        );
    }

    #[test]
    fn test_plane_intersect_ray() {
        let plane = PlaneClass::from_point_normal(Vec3::ZERO, Vec3::new(0.0, 1.0, 0.0));

        // Ray hitting plane
        let t = plane
            .intersect_ray(Vec3::new(0.0, 5.0, 0.0), Vec3::new(0.0, -1.0, 0.0))
            .unwrap();

        assert_eq!(t, 5.0);

        // Ray parallel to plane
        assert!(plane
            .intersect_ray(Vec3::new(0.0, 5.0, 0.0), Vec3::new(1.0, 0.0, 0.0))
            .is_none());

        // Ray away from plane
        assert!(plane
            .intersect_ray(Vec3::new(0.0, 5.0, 0.0), Vec3::new(0.0, 1.0, 0.0))
            .is_none());
    }

    #[test]
    fn test_plane_normalize() {
        let mut plane = PlaneClass::new(Vec3::new(0.0, 2.0, 0.0), 10.0);
        plane.normalize();

        assert_eq!(plane.normal, Vec3::new(0.0, 1.0, 0.0));
        assert_eq!(plane.distance, 5.0);
    }

    #[test]
    fn test_plane_parallel() {
        let plane1 = PlaneClass::from_point_normal(Vec3::ZERO, Vec3::new(0.0, 1.0, 0.0));
        let plane2 =
            PlaneClass::from_point_normal(Vec3::new(0.0, 2.0, 0.0), Vec3::new(0.0, 1.0, 0.0));
        let plane3 = PlaneClass::from_point_normal(Vec3::ZERO, Vec3::new(1.0, 0.0, 0.0));

        assert!(plane1.is_parallel_to(&plane2));
        assert!(!plane1.is_parallel_to(&plane3));
    }

    #[test]
    fn test_plane_coincident() {
        let plane1 = PlaneClass::from_point_normal(Vec3::ZERO, Vec3::new(0.0, 1.0, 0.0));
        let plane2 = PlaneClass::from_point_normal(Vec3::ZERO, Vec3::new(0.0, 1.0, 0.0));
        let plane3 =
            PlaneClass::from_point_normal(Vec3::new(0.0, 1.0, 0.0), Vec3::new(0.0, 1.0, 0.0));

        assert!(plane1.is_coincident_with(&plane2));
        assert!(!plane1.is_coincident_with(&plane3));
    }
}
