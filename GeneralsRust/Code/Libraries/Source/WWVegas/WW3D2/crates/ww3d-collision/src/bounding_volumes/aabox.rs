//! AABox - Axis-aligned bounding box
//!
//! The legacy operations follow `WWMath/aabox.h`, `colmathaabox.h`, and
//! `Matrix3D::Transform_Center_Extent_AABox` in `WWMath/matrix3d.cpp`.
//! Inputs are finite, with nonnegative extents; transforms are affine.
//! Unlike C++'s uninitialized default and nonempty point-array precondition,
//! Rust defaults and empty point slices produce a zero box at the origin.
//! Closest-point, distance, surface-area, validity and component-wise scaling
//! helpers extend the C++ API. Scaling changes extents, not the center.

use glam::{Mat4, Vec3};

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

    /// Get the min corner by value.
    pub fn min_corner(&self) -> Vec3 {
        self.min()
    }

    /// Get the max corner by value.
    pub fn max_corner(&self) -> Vec3 {
        self.max()
    }

    /// Check if point is inside the box, including its boundary.
    pub fn contains_point(&self, point: &Vec3) -> bool {
        let delta = (*point - self.center).abs();
        delta.x <= self.extent.x && delta.y <= self.extent.y && delta.z <= self.extent.z
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
        let min = self.min().min(*point);
        let max = self.max().max(*point);
        self.init_min_max(min, max);
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

    /// Enclose the affine-transformed box using the legacy center/extent formula.
    pub fn transform(&mut self, transform: &Mat4) {
        let old_center = self.center;
        let old_extent = self.extent;
        for i in 0..3 {
            let row = transform.row(i);
            self.center[i] = row[3];
            self.extent[i] = 0.0;
            for j in 0..3 {
                self.center[i] += row[j] * old_center[j];
                self.extent[i] += (row[j] * old_extent[j]).abs();
            }
        }
    }

    /// Return the projected half-width, independent of center or axis length.
    ///
    /// Matches `AABoxClass::Project_To_Axis`, not the full interval width.
    pub fn project_to_axis(&self, axis: &Vec3) -> f32 {
        let projection = (self.extent * *axis).abs();
        projection.x + projection.y + projection.z
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
        let delta = (other.center - self.center).abs();
        let extent_sum = self.extent + other.extent;
        delta.x <= extent_sum.x && delta.y <= extent_sum.y && delta.z <= extent_sum.z
    }

    /// Check if this box completely contains another box
    pub fn contains_aabox(&self, other: &AABoxClass) -> bool {
        let delta = other.center - self.center;
        let upper = delta + other.extent;
        let lower = delta - other.extent;
        upper.x <= self.extent.x
            && upper.y <= self.extent.y
            && upper.z <= self.extent.z
            && lower.x >= -self.extent.x
            && lower.y >= -self.extent.y
            && lower.z >= -self.extent.z
    }

    /// Get the volume of the box
    pub fn volume(&self) -> f32 {
        // The unsuffixed 2.0 literals in aabox.h promote this chain to double.
        (2.0 * f64::from(self.extent.x)
            * 2.0
            * f64::from(self.extent.y)
            * 2.0
            * f64::from(self.extent.z)) as f32
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
    fn box_extensions_keep_center_and_query_geometry() {
        let mut bounds = AABoxClass::from_center_extent(Vec3::splat(10.0), Vec3::ONE);
        bounds.scale(&Vec3::new(2.0, 3.0, 4.0));
        assert_eq!(bounds.center(), Vec3::splat(10.0));
        assert_eq!(bounds.volume(), 192.0);
        assert_eq!(bounds.surface_area(), 208.0);
        assert!(bounds.is_valid());
        assert!(!bounds.is_degenerate());
        for corner in bounds.get_corners() {
            assert!(bounds.contains_point(&corner));
            assert_eq!(bounds.distance_to_point(&corner), 0.0);
        }
        bounds.extent.x = -1.0;
        assert!(!bounds.is_valid());
        bounds.make_valid();
        assert!(bounds.is_valid());
        assert!(bounds.is_degenerate());
        assert_eq!(AABoxClass::from_points(&[]), AABoxClass::new());
    }

    #[test]
    fn cpp_transform_applies_translation_once_and_preserves_axis_extent_order() {
        let mut bounds =
            AABoxClass::from_center_extent(Vec3::new(1.0, 2.0, 3.0), Vec3::new(2.0, 3.0, 4.0));
        let transform = Mat4::from_cols(
            Vec3::new(0.0, 2.0, 0.0).extend(0.0),
            Vec3::new(-3.0, 0.0, 0.0).extend(0.0),
            Vec3::new(1.0, 0.0, -4.0).extend(0.0),
            Vec3::new(10.0, 20.0, 30.0).extend(1.0),
        );
        bounds.transform(&transform);
        assert_eq!(bounds.center, Vec3::new(7.0, 22.0, 18.0));
        assert_eq!(bounds.extent, Vec3::new(13.0, 4.0, 16.0));
        assert!(bounds.contains_point(&Vec3::new(20.0, 26.0, 34.0)));
        assert!(!bounds.contains_point(&Vec3::new(20.25, 26.0, 34.0)));
    }

    #[test]
    fn cpp_axis_projection_is_half_width_and_ignores_translation() {
        let mut bounds =
            AABoxClass::from_center_extent(Vec3::new(100.0, -50.0, 3.0), Vec3::new(2.0, 3.0, 4.0));
        assert_eq!(bounds.project_to_axis(&Vec3::new(-2.0, 3.0, -4.0)), 29.0);
        bounds.translate(&Vec3::splat(100.0));
        assert_eq!(bounds.project_to_axis(&Vec3::new(-2.0, 3.0, -4.0)), 29.0);
        assert_eq!(bounds.project_to_axis(&Vec3::ZERO), 0.0);
    }

    #[test]
    fn cpp_bounds_growth_and_boundary_overlap() {
        let mut bounds = AABoxClass::from_points(&[
            Vec3::new(-2.0, 1.0, -3.0),
            Vec3::new(4.0, 5.0, 1.0),
        ]);
        bounds.add_point(&Vec3::new(-4.0, 3.0, 5.0));
        assert_eq!(bounds.center, Vec3::new(0.0, 3.0, 1.0));
        assert_eq!(bounds.extent, Vec3::splat(4.0) * Vec3::new(1.0, 0.5, 1.0));
        let boundary = AABoxClass::from_center_extent(Vec3::new(5.0, 3.0, 1.0), Vec3::ONE);
        assert!(bounds.intersects_aabox(&boundary));
        assert!(!bounds.contains_aabox(&boundary));
        assert!(bounds.contains_aabox(&bounds));
        let corner = Vec3::new(4.0, 5.0, 5.0);
        assert!(bounds.contains_point(&corner));
        assert_eq!(bounds.closest_point(&Vec3::new(7.0, 9.0, 5.0)), corner);
        assert_eq!(bounds.distance_squared_to_point(&Vec3::new(7.0, 9.0, 5.0)), 25.0);
    }

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
