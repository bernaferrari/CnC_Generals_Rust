//! AABox - Axis-aligned bounding box
//!
//! This module implements axis-aligned bounding boxes for collision detection
//! and spatial partitioning, converted from the original AABoxClass.

use glam::{Mat4, Vec3};
use std::f32;

/// Center-extent axis-aligned bounding box
#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[repr(C)]
pub struct AABoxClass {
    /// Center point of the box
    pub center: Vec3,
    /// Extent (half-size) in each dimension
    pub extent: Vec3,
}

impl AABoxClass {
    /// Create empty AABox
    pub fn new() -> Self {
        Self {
            center: Vec3::ZERO,
            extent: Vec3::ZERO,
        }
    }

    /// Create AABox from center and extent
    pub fn from_center_and_extent(center: Vec3, extent: Vec3) -> Self {
        Self { center, extent }
    }

    /// Create AABox from center and extent (alias for compatibility)
    pub fn from_center_extent(center: Vec3, extent: Vec3) -> Self {
        Self::from_center_and_extent(center, extent)
    }

    /// Get the extents (half-size) of the box - for C++ WW3D2 API compatibility
    pub fn extents(&self) -> Vec3 {
        self.extent
    }

    /// Get the center of the box - for C++ WW3D2 API compatibility
    pub fn center(&self) -> Vec3 {
        self.center
    }

    /// Get the min corner of the box
    pub fn min(&self) -> Vec3 {
        self.center - self.extent
    }

    /// Get the max corner of the box
    pub fn max(&self) -> Vec3 {
        self.center + self.extent
    }

    /// Get the min corner of the box (alias for compatibility)
    pub fn get_min(&self) -> Vec3 {
        self.min()
    }

    /// Get the max corner of the box (alias for compatibility)
    pub fn get_max(&self) -> Vec3 {
        self.max()
    }

    /// Create AABox from min/max corners
    pub fn from_min_max(min_corner: Vec3, max_corner: Vec3) -> Self {
        let center = (min_corner + max_corner) * 0.5;
        let extent = (max_corner - min_corner) * 0.5;
        Self { center, extent }
    }

    /// Create AABox from a set of points
    pub fn from_points(points: &[Vec3]) -> Self {
        if points.is_empty() {
            return Self::new();
        }

        let mut min_corner = Vec3::new(f32::INFINITY, f32::INFINITY, f32::INFINITY);
        let mut max_corner = Vec3::new(f32::NEG_INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY);

        for &point in points {
            min_corner = min_corner.min(point);
            max_corner = max_corner.max(point);
        }

        Self::from_min_max(min_corner, max_corner)
    }

    /// Initialize AABox from center and extent
    pub fn init_center_extent(&mut self, center: Vec3, extent: Vec3) {
        self.center = center;
        self.extent = extent;
    }

    /// Initialize AABox from a set of points
    pub fn init_from_points(&mut self, points: &[Vec3]) {
        *self = Self::from_points(points);
    }

    /// Initialize AABox from min/max corners
    pub fn init_min_max(&mut self, min_corner: Vec3, max_corner: Vec3) {
        *self = Self::from_min_max(min_corner, max_corner);
    }

    /// Get the min corner (mutable access)
    pub fn min_corner(&self) -> Vec3 {
        self.min()
    }

    /// Get the max corner (mutable access)
    pub fn max_corner(&self) -> Vec3 {
        self.max()
    }

    /// Check if point is inside the box
    pub fn contains_point(&self, point: &Vec3) -> bool {
        let min = self.min();
        let max = self.max();
        point.x >= min.x
            && point.x <= max.x
            && point.y >= min.y
            && point.y <= max.y
            && point.z >= min.z
            && point.z <= max.z
    }

    /// Find the closest point on the box to the given point
    pub fn closest_point(&self, point: &Vec3) -> Vec3 {
        let min = self.min();
        let max = self.max();
        Vec3::new(
            point.x.clamp(min.x, max.x),
            point.y.clamp(min.y, max.y),
            point.z.clamp(min.z, max.z),
        )
    }

    /// Calculate distance from point to box
    pub fn distance_to_point(&self, point: &Vec3) -> f32 {
        let closest = self.closest_point(point);
        (*point - closest).length()
    }

    /// Calculate squared distance from point to box
    pub fn distance_squared_to_point(&self, point: &Vec3) -> f32 {
        let closest = self.closest_point(point);
        (*point - closest).length_squared()
    }

    /// Add a point to the box, expanding it if necessary
    pub fn add_point(&mut self, point: &Vec3) {
        if !self.contains_point(point) {
            let min = self.min();
            let max = self.max();
            let new_min = Vec3::new(min.x.min(point.x), min.y.min(point.y), min.z.min(point.z));
            let new_max = Vec3::new(max.x.max(point.x), max.y.max(point.y), max.z.max(point.z));
            self.init_min_max(new_min, new_max);
        }
    }

    /// Add multiple points to the box
    pub fn add_points(&mut self, points: &[Vec3]) {
        for point in points {
            self.add_point(point);
        }
    }

    /// Translate the box by an offset
    pub fn translate(&mut self, offset: &Vec3) {
        self.center += *offset;
    }

    /// Scale the box
    pub fn scale(&mut self, scale: &Vec3) {
        self.extent *= *scale;
    }

    /// Transform the box by a matrix
    pub fn transform(&mut self, transform: &Mat4) {
        // Transform the center
        self.center = transform.transform_point3(self.center);

        // For extents, we need to transform each corner and find new bounds
        let corners = self.get_corners();
        let mut transformed_corners = [Vec3::ZERO; 8];

        for (i, corner) in corners.iter().enumerate() {
            transformed_corners[i] = transform.transform_point3(*corner);
        }

        // Recalculate extents from transformed corners
        self.init_from_points(&transformed_corners);
    }

    /// Project the box onto an axis
    pub fn project_to_axis(&self, axis: &Vec3) -> f32 {
        let corners = self.get_corners();
        let mut min_proj = f32::INFINITY;
        let mut max_proj = f32::NEG_INFINITY;

        for corner in &corners {
            let proj = corner.dot(*axis);
            min_proj = min_proj.min(proj);
            max_proj = max_proj.max(proj);
        }

        max_proj - min_proj
    }

    /// Get all 8 corners of the box
    pub fn get_corners(&self) -> [Vec3; 8] {
        let min = self.min();
        let max = self.max();

        [
            Vec3::new(min.x, min.y, min.z), // 000
            Vec3::new(max.x, min.y, min.z), // 100
            Vec3::new(max.x, max.y, min.z), // 110
            Vec3::new(min.x, max.y, min.z), // 010
            Vec3::new(min.x, min.y, max.z), // 001
            Vec3::new(max.x, min.y, max.z), // 101
            Vec3::new(max.x, max.y, max.z), // 111
            Vec3::new(min.x, max.y, max.z), // 011
        ]
    }

    /// Check intersection with another AABox
    pub fn intersects_aabox(&self, other: &AABoxClass) -> bool {
        let self_min = self.min();
        let self_max = self.max();
        let other_min = other.min();
        let other_max = other.max();

        self_min.x <= other_max.x
            && self_max.x >= other_min.x
            && self_min.y <= other_max.y
            && self_max.y >= other_min.y
            && self_min.z <= other_max.z
            && self_max.z >= other_min.z
    }

    /// Check if this box completely contains another box
    pub fn contains_aabox(&self, other: &AABoxClass) -> bool {
        let self_min = self.min();
        let self_max = self.max();
        let other_min = other.min();
        let other_max = other.max();

        self_min.x <= other_min.x
            && self_max.x >= other_max.x
            && self_min.y <= other_min.y
            && self_max.y >= other_max.y
            && self_min.z <= other_min.z
            && self_max.z >= other_max.z
    }

    /// Get the volume of the box
    pub fn volume(&self) -> f32 {
        let size = self.extent * 2.0;
        size.x * size.y * size.z
    }

    /// Get the surface area of the box
    pub fn surface_area(&self) -> f32 {
        let size = self.extent * 2.0;
        2.0 * (size.x * size.y + size.y * size.z + size.z * size.x)
    }

    /// Check if the box is valid (non-negative extents)
    pub fn is_valid(&self) -> bool {
        self.extent.x >= 0.0 && self.extent.y >= 0.0 && self.extent.z >= 0.0
    }

    /// Make the box valid by ensuring non-negative extents
    pub fn make_valid(&mut self) {
        self.extent = self.extent.max(Vec3::ZERO);
    }

    /// Check if the box is degenerate (has zero volume)
    pub fn is_degenerate(&self) -> bool {
        self.extent.x <= 0.0 || self.extent.y <= 0.0 || self.extent.z <= 0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_aabox_creation() {
        let aabox =
            AABoxClass::from_center_and_extent(Vec3::new(1.0, 2.0, 3.0), Vec3::new(0.5, 1.0, 1.5));

        assert_eq!(aabox.center, Vec3::new(1.0, 2.0, 3.0));
        assert_eq!(aabox.extent, Vec3::new(0.5, 1.0, 1.5));
        assert_eq!(aabox.min(), Vec3::new(0.5, 1.0, 1.5));
        assert_eq!(aabox.max(), Vec3::new(1.5, 3.0, 4.5));
    }

    #[test]
    fn test_aabox_from_min_max() {
        let min_corner = Vec3::new(0.0, 0.0, 0.0);
        let max_corner = Vec3::new(2.0, 4.0, 6.0);
        let aabox = AABoxClass::from_min_max(min_corner, max_corner);

        assert_eq!(aabox.center, Vec3::new(1.0, 2.0, 3.0));
        assert_eq!(aabox.extent, Vec3::new(1.0, 2.0, 3.0));
    }

    #[test]
    fn test_aabox_contains_point() {
        let aabox =
            AABoxClass::from_center_and_extent(Vec3::new(0.0, 0.0, 0.0), Vec3::new(1.0, 1.0, 1.0));

        assert!(aabox.contains_point(&Vec3::ZERO));
        assert!(aabox.contains_point(&Vec3::new(1.0, 0.0, 0.0)));
        assert!(!aabox.contains_point(&Vec3::new(2.0, 0.0, 0.0)));
    }

    #[test]
    fn test_aabox_intersection() {
        let a =
            AABoxClass::from_center_and_extent(Vec3::new(0.0, 0.0, 0.0), Vec3::new(1.0, 1.0, 1.0));
        let b =
            AABoxClass::from_center_and_extent(Vec3::new(1.0, 0.0, 0.0), Vec3::new(1.0, 1.0, 1.0));

        assert!(a.intersects_aabox(&b)); // Touching edges

        let c =
            AABoxClass::from_center_and_extent(Vec3::new(3.0, 0.0, 0.0), Vec3::new(1.0, 1.0, 1.0));

        assert!(!a.intersects_aabox(&c)); // No intersection
    }

    #[test]
    fn test_aabox_volume() {
        let aabox = AABoxClass::from_center_and_extent(Vec3::ZERO, Vec3::new(1.0, 2.0, 3.0));

        // Volume = (2*1) * (2*2) * (2*3) = 2 * 4 * 6 = 48
        assert_eq!(aabox.volume(), 48.0);
    }

    #[test]
    fn test_aabox_surface_area() {
        let aabox = AABoxClass::from_center_and_extent(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0));

        // Surface area = 2 * (2*2 + 2*2 + 2*2) = 2 * 12 = 24
        assert_eq!(aabox.surface_area(), 24.0);
    }
}
