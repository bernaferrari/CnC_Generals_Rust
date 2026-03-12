//! Axis-Aligned Bounding Box implementation.
//!
//! This module provides `AABox` and `MinMaxAABox` functionality,
//! converted from the original C++ `AABoxClass` implementation.

use crate::vector_extensions::Vec3Extensions;
use crate::{Matrix3D, Vector3, WWMath};

/// Axis-aligned bounding box represented by center and extent.
/// This is the primary `AABox` representation using center + extent form.
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct AABox {
    /// World space center of the box
    pub center: Vector3,
    /// Half-size of the box in each direction (extent from center to face)
    pub extent: Vector3,
}

/// Axis-aligned bounding box represented by minimum and maximum corners.
/// This is an alternative representation that can be faster to build in some cases.
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct MinMaxAABox {
    /// Minimum corner of the box
    pub min_corner: Vector3,
    /// Maximum corner of the box
    pub max_corner: Vector3,
}

/// Overlap test results for collision detection
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum OverlapResult {
    /// No overlap between objects
    Outside,
    /// Partial overlap between objects
    Intersecting,
    /// One object is completely inside the other
    Inside,
}

impl AABox {
    /// Create a zero-sized box at origin
    pub const ZERO: AABox = AABox {
        center: Vector3::ZERO,
        extent: Vector3::ZERO,
    };

    /// Create a new `AABox` from center and extent
    #[must_use]
    pub fn new(center: Vector3, extent: Vector3) -> Self {
        Self { center, extent }
    }

    /// Create `AABox` from array of points
    ///
    /// # Panics
    /// Panics if `points` is empty.
    #[must_use]
    pub fn from_points(points: &[Vector3]) -> Self {
        assert!(
            !points.is_empty(),
            "Need at least one point to create AABox"
        );

        let mut min = points[0];
        let mut max = points[0];

        for point in points.iter().skip(1) {
            min.update_min(point);
            max.update_max(point);
        }

        Self::from_min_max(min, max)
    }

    /// Create `AABox` from `MinMaxAABox`
    #[must_use]
    pub fn from_min_max_box(minmax_box: &MinMaxAABox) -> Self {
        let center = (minmax_box.max_corner + minmax_box.min_corner) * 0.5;
        let extent = (minmax_box.max_corner - minmax_box.min_corner) * 0.5;
        Self { center, extent }
    }

    /// Create `AABox` from minimum and maximum corners
    #[must_use]
    pub fn from_min_max(min: Vector3, max: Vector3) -> Self {
        let center = (max + min) * 0.5;
        let extent = (max - min) * 0.5;
        Self { center, extent }
    }

    /// Initialize from center and extent
    pub fn init(&mut self, center: Vector3, extent: Vector3) {
        self.center = center;
        self.extent = extent;
    }

    /// Initialize from array of points
    pub fn init_from_points(&mut self, points: &[Vector3]) {
        *self = Self::from_points(points);
    }

    /// Initialize from `MinMaxAABox`
    pub fn init_from_min_max_box(&mut self, minmax_box: &MinMaxAABox) {
        *self = Self::from_min_max_box(minmax_box);
    }

    /// Initialize from min and max vectors
    pub fn init_from_min_max(&mut self, min: Vector3, max: Vector3) {
        *self = Self::from_min_max(min, max);
    }

    /// Initialize to random values within given ranges
    pub fn init_random(
        &mut self,
        min_center: f32,
        max_center: f32,
        min_extent: f32,
        max_extent: f32,
    ) {
        self.center.x = min_center + WWMath::random_float() * (max_center - min_center);
        self.center.y = min_center + WWMath::random_float() * (max_center - min_center);
        self.center.z = min_center + WWMath::random_float() * (max_center - min_center);

        self.extent.x = min_extent + WWMath::random_float() * (max_extent - min_extent);
        self.extent.y = min_extent + WWMath::random_float() * (max_extent - min_extent);
        self.extent.z = min_extent + WWMath::random_float() * (max_extent - min_extent);
    }

    /// Get the minimum corner of the box
    #[must_use]
    pub fn min_corner(&self) -> Vector3 {
        self.center - self.extent
    }

    /// Get the maximum corner of the box
    #[must_use]
    pub fn max_corner(&self) -> Vector3 {
        self.center + self.extent
    }

    /// Calculate the volume of the box
    #[must_use]
    pub fn volume(&self) -> f32 {
        2.0 * self.extent.x * 2.0 * self.extent.y * 2.0 * self.extent.z
    }

    /// Expand the box to contain the given point
    pub fn add_point(&mut self, point: Vector3) {
        let mut min = self.min_corner();
        let mut max = self.max_corner();

        min.update_min(&point);
        max.update_max(&point);

        self.center = (max + min) * 0.5;
        self.extent = (max - min) * 0.5;
    }

    /// Expand this box to enclose another `AABox`
    pub fn add_box(&mut self, other: &AABox) {
        let mut new_min = self.min_corner();
        let mut new_max = self.max_corner();

        let other_min = other.min_corner();
        let other_max = other.max_corner();

        new_min.update_min(&other_min);
        new_max.update_max(&other_max);

        self.center = (new_max + new_min) * 0.5;
        self.extent = (new_max - new_min) * 0.5;
    }

    /// Expand this box to enclose a `MinMaxAABox`
    pub fn add_min_max_box(&mut self, other: &MinMaxAABox) {
        let mut new_min = self.min_corner();
        let mut new_max = self.max_corner();

        new_min.update_min(&other.min_corner);
        new_max.update_max(&other.max_corner);

        self.center = (new_max + new_min) * 0.5;
        self.extent = (new_max - new_min) * 0.5;
    }

    /// Project the box onto the given axis
    #[must_use]
    pub fn project_to_axis(&self, axis: Vector3) -> f32 {
        let x = self.extent.x * axis.x;
        let y = self.extent.y * axis.y;
        let z = self.extent.z * axis.z;

        // Projection is the sum of absolute values of the projections of the three extents
        x.abs() + y.abs() + z.abs()
    }

    /// Test if this box contains a point
    #[must_use]
    pub fn contains_point(&self, point: Vector3) -> bool {
        self.overlap_test_point(point) == OverlapResult::Inside
    }

    /// Test if this box completely contains another `AABox`
    #[must_use]
    pub fn contains_box(&self, other: &AABox) -> bool {
        self.overlap_test_box(other) == OverlapResult::Inside
    }

    /// Test if this box completely contains a `MinMaxAABox`
    #[must_use]
    pub fn contains_min_max_box(&self, other: &MinMaxAABox) -> bool {
        let min = self.min_corner();
        let max = self.max_corner();

        other.min_corner.x >= min.x
            && other.min_corner.y >= min.y
            && other.min_corner.z >= min.z
            && other.max_corner.x <= max.x
            && other.max_corner.y <= max.y
            && other.max_corner.z <= max.z
    }

    /// Test overlap with a point
    #[must_use]
    pub fn overlap_test_point(&self, point: Vector3) -> OverlapResult {
        let diff = point - self.center;

        if diff.x.abs() <= self.extent.x
            && diff.y.abs() <= self.extent.y
            && diff.z.abs() <= self.extent.z
        {
            OverlapResult::Inside
        } else {
            OverlapResult::Outside
        }
    }

    /// Test overlap with another `AABox`
    #[must_use]
    pub fn overlap_test_box(&self, other: &AABox) -> OverlapResult {
        let diff = self.center - other.center;
        let combined_extent = self.extent + other.extent;

        // Check if boxes are separated on any axis
        if diff.x.abs() > combined_extent.x
            || diff.y.abs() > combined_extent.y
            || diff.z.abs() > combined_extent.z
        {
            return OverlapResult::Outside;
        }

        // Check if one box is inside the other
        let self_min = self.min_corner();
        let self_max = self.max_corner();
        let other_min = other.min_corner();
        let other_max = other.max_corner();

        // Check if other is inside self
        if other_min.x >= self_min.x
            && other_min.y >= self_min.y
            && other_min.z >= self_min.z
            && other_max.x <= self_max.x
            && other_max.y <= self_max.y
            && other_max.z <= self_max.z
        {
            return OverlapResult::Inside;
        }

        OverlapResult::Intersecting
    }

    /// Test if this box intersects with another (any overlap)
    #[must_use]
    pub fn intersects(&self, other: &AABox) -> bool {
        self.overlap_test_box(other) != OverlapResult::Outside
    }

    /// Transform the box by a matrix (expands to enclose transformed form)
    pub fn transform(&mut self, matrix: &Matrix3D) {
        let old_center = self.center;
        let old_extent = self.extent;
        self.transform_center_extent(matrix, old_center, old_extent);
    }

    /// Transform center and extent by matrix
    pub fn transform_center_extent(&mut self, matrix: &Matrix3D, center: Vector3, extent: Vector3) {
        // Transform center
        self.center = matrix.transform_vector(center);

        // Transform extent by computing bounding box of all transformed corners
        // This is a more straightforward approach than the optimized C++ version
        let corners = [
            Vector3::new(-extent.x, -extent.y, -extent.z),
            Vector3::new(extent.x, -extent.y, -extent.z),
            Vector3::new(-extent.x, extent.y, -extent.z),
            Vector3::new(extent.x, extent.y, -extent.z),
            Vector3::new(-extent.x, -extent.y, extent.z),
            Vector3::new(extent.x, -extent.y, extent.z),
            Vector3::new(-extent.x, extent.y, extent.z),
            Vector3::new(extent.x, extent.y, extent.z),
        ];

        let mut min_corner = matrix.rotate_vector(corners[0]);
        let mut max_corner = min_corner;

        for corner in corners.iter().skip(1) {
            let transformed = matrix.rotate_vector(*corner);
            min_corner.update_min(&transformed);
            max_corner.update_max(&transformed);
        }

        self.extent = (max_corner - min_corner) * 0.5;
    }

    /// Static transform function
    #[must_use]
    pub fn transform_box(matrix: &Matrix3D, input: &AABox) -> AABox {
        let mut result = *input;
        result.transform(matrix);
        result
    }

    /// Translate the box by a vector
    pub fn translate(&mut self, translation: Vector3) {
        self.center += translation;
    }
}

impl MinMaxAABox {
    /// Create empty box (invalid state for building)
    #[must_use]
    pub fn empty() -> Self {
        Self {
            min_corner: Vector3::new(f32::MAX, f32::MAX, f32::MAX),
            max_corner: Vector3::new(f32::MIN, f32::MIN, f32::MIN),
        }
    }

    /// Create new `MinMaxAABox` from corners
    #[must_use]
    pub fn new(min_corner: Vector3, max_corner: Vector3) -> Self {
        Self {
            min_corner,
            max_corner,
        }
    }

    /// Create from array of points
    ///
    /// # Panics
    /// Panics if `points` is empty.
    #[must_use]
    pub fn from_points(points: &[Vector3]) -> Self {
        assert!(
            !points.is_empty(),
            "Need at least one point to create MinMaxAABox"
        );

        let mut result = Self {
            min_corner: points[0],
            max_corner: points[0],
        };

        for point in points.iter().skip(1) {
            result.min_corner.update_min(point);
            result.max_corner.update_max(point);
        }

        result
    }

    /// Create from `AABox`
    #[must_use]
    pub fn from_aabox(aabox: &AABox) -> Self {
        Self {
            min_corner: aabox.min_corner(),
            max_corner: aabox.max_corner(),
        }
    }

    /// Initialize to empty state
    pub fn init_empty(&mut self) {
        self.min_corner = Vector3::new(f32::MAX, f32::MAX, f32::MAX);
        self.max_corner = Vector3::new(f32::MIN, f32::MIN, f32::MIN);
    }

    /// Initialize from points
    pub fn init_from_points(&mut self, points: &[Vector3]) {
        *self = Self::from_points(points);
    }

    /// Initialize from `AABox`
    pub fn init_from_aabox(&mut self, aabox: &AABox) {
        *self = Self::from_aabox(aabox);
    }

    /// Add a point to the box
    pub fn add_point(&mut self, point: Vector3) {
        self.min_corner.update_min(&point);
        self.max_corner.update_max(&point);
    }

    /// Add another `MinMaxAABox`
    pub fn add_min_max_box(&mut self, other: &MinMaxAABox) {
        // Skip zero-extent boxes
        if other.min_corner == other.max_corner {
            return;
        }

        self.min_corner.update_min(&other.min_corner);
        self.max_corner.update_max(&other.max_corner);
    }

    /// Add an `AABox`
    pub fn add_aabox(&mut self, aabox: &AABox) {
        // Skip zero-extent boxes
        if aabox.extent == Vector3::ZERO {
            return;
        }

        let min = aabox.min_corner();
        let max = aabox.max_corner();
        self.min_corner.update_min(&min);
        self.max_corner.update_max(&max);
    }

    /// Add box defined by min/max corners
    pub fn add_box(&mut self, min_corner: Vector3, max_corner: Vector3) {
        // Skip zero-extent boxes
        if min_corner == max_corner {
            return;
        }

        self.min_corner.update_min(&min_corner);
        self.max_corner.update_max(&max_corner);
    }

    /// Calculate volume
    #[must_use]
    pub fn volume(&self) -> f32 {
        let size = self.max_corner - self.min_corner;
        size.x * size.y * size.z
    }

    /// Transform the box by a matrix
    pub fn transform(&mut self, matrix: &Matrix3D) {
        let old_min = self.min_corner;
        let old_max = self.max_corner;
        self.transform_min_max(matrix, old_min, old_max);
    }

    /// Transform min/max corners by matrix
    pub fn transform_min_max(&mut self, matrix: &Matrix3D, min: Vector3, max: Vector3) {
        // Transform all 8 corners and find new min/max
        let corners = [
            Vector3::new(min.x, min.y, min.z),
            Vector3::new(max.x, min.y, min.z),
            Vector3::new(min.x, max.y, min.z),
            Vector3::new(max.x, max.y, min.z),
            Vector3::new(min.x, min.y, max.z),
            Vector3::new(max.x, min.y, max.z),
            Vector3::new(min.x, max.y, max.z),
            Vector3::new(max.x, max.y, max.z),
        ];

        let transformed_corner = matrix.transform_vector(corners[0]);
        self.min_corner = transformed_corner;
        self.max_corner = transformed_corner;

        for corner in corners.iter().skip(1) {
            let transformed = matrix.transform_vector(*corner);
            self.min_corner.update_min(&transformed);
            self.max_corner.update_max(&transformed);
        }
    }

    /// Translate the box
    pub fn translate(&mut self, translation: Vector3) {
        self.min_corner += translation;
        self.max_corner += translation;
    }
}

impl From<AABox> for MinMaxAABox {
    fn from(aabox: AABox) -> Self {
        MinMaxAABox::from_aabox(&aabox)
    }
}

impl From<MinMaxAABox> for AABox {
    fn from(minmax: MinMaxAABox) -> Self {
        AABox::from_min_max_box(&minmax)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::EPSILON;

    #[test]
    fn test_aabox_creation() {
        let center = Vector3::new(1.0, 2.0, 3.0);
        let extent = Vector3::new(0.5, 1.0, 1.5);
        let bbox = AABox::new(center, extent);

        assert_eq!(bbox.center, center);
        assert_eq!(bbox.extent, extent);
    }

    #[test]
    fn test_aabox_from_points() {
        let points = vec![
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(2.0, 4.0, 6.0),
            Vector3::new(-1.0, -2.0, -3.0),
        ];

        let bbox = AABox::from_points(&points);

        // Expected: min=(-1,-2,-3), max=(2,4,6)
        // Center = (0.5, 1.0, 1.5), Extent = (1.5, 3.0, 4.5)
        assert_eq!(bbox.center, Vector3::new(0.5, 1.0, 1.5));
        assert_eq!(bbox.extent, Vector3::new(1.5, 3.0, 4.5));
    }

    #[test]
    fn test_aabox_from_min_max() {
        let min = Vector3::new(-1.0, -2.0, -3.0);
        let max = Vector3::new(3.0, 4.0, 5.0);
        let bbox = AABox::from_min_max(min, max);

        assert_eq!(bbox.center, Vector3::new(1.0, 1.0, 1.0));
        assert_eq!(bbox.extent, Vector3::new(2.0, 3.0, 4.0));
        assert_eq!(bbox.min_corner(), min);
        assert_eq!(bbox.max_corner(), max);
    }

    #[test]
    fn test_volume() {
        let bbox = AABox::new(Vector3::ZERO, Vector3::new(1.0, 2.0, 3.0));
        // Volume = (2*1) * (2*2) * (2*3) = 2 * 4 * 6 = 48
        assert_eq!(bbox.volume(), 48.0);
    }

    #[test]
    fn test_add_point() {
        let mut bbox = AABox::new(Vector3::ZERO, Vector3::new(1.0, 1.0, 1.0));

        // Add a point outside the box
        bbox.add_point(Vector3::new(3.0, 0.0, 0.0));

        // Box should expand to contain the new point
        assert_eq!(bbox.min_corner(), Vector3::new(-1.0, -1.0, -1.0));
        assert_eq!(bbox.max_corner(), Vector3::new(3.0, 1.0, 1.0));
    }

    #[test]
    fn test_add_box() {
        let mut bbox1 = AABox::new(Vector3::ZERO, Vector3::new(1.0, 1.0, 1.0));
        let bbox2 = AABox::new(Vector3::new(2.0, 2.0, 2.0), Vector3::new(1.0, 1.0, 1.0));

        bbox1.add_box(&bbox2);

        // Combined box should go from (-1,-1,-1) to (3,3,3)
        assert_eq!(bbox1.min_corner(), Vector3::new(-1.0, -1.0, -1.0));
        assert_eq!(bbox1.max_corner(), Vector3::new(3.0, 3.0, 3.0));
    }

    #[test]
    fn test_contains_point() {
        let bbox = AABox::new(Vector3::ZERO, Vector3::new(2.0, 2.0, 2.0));

        assert!(bbox.contains_point(Vector3::new(1.0, 1.0, 1.0)));
        assert!(bbox.contains_point(Vector3::new(-2.0, -2.0, -2.0))); // On boundary
        assert!(!bbox.contains_point(Vector3::new(3.0, 0.0, 0.0))); // Outside
    }

    #[test]
    fn test_overlap_test_point() {
        let bbox = AABox::new(Vector3::ZERO, Vector3::new(1.0, 1.0, 1.0));

        assert_eq!(
            bbox.overlap_test_point(Vector3::new(0.5, 0.5, 0.5)),
            OverlapResult::Inside
        );
        assert_eq!(
            bbox.overlap_test_point(Vector3::new(1.0, 1.0, 1.0)),
            OverlapResult::Inside
        );
        assert_eq!(
            bbox.overlap_test_point(Vector3::new(2.0, 0.0, 0.0)),
            OverlapResult::Outside
        );
    }

    #[test]
    fn test_overlap_test_box() {
        let bbox1 = AABox::new(Vector3::ZERO, Vector3::new(2.0, 2.0, 2.0));
        let bbox2 = AABox::new(Vector3::new(1.0, 1.0, 1.0), Vector3::new(0.5, 0.5, 0.5)); // Inside
        let bbox3 = AABox::new(Vector3::new(1.5, 1.5, 1.5), Vector3::new(1.0, 1.0, 1.0)); // Intersecting
        let bbox4 = AABox::new(Vector3::new(5.0, 5.0, 5.0), Vector3::new(1.0, 1.0, 1.0)); // Outside

        assert_eq!(bbox1.overlap_test_box(&bbox2), OverlapResult::Inside);
        assert_eq!(bbox1.overlap_test_box(&bbox3), OverlapResult::Intersecting);
        assert_eq!(bbox1.overlap_test_box(&bbox4), OverlapResult::Outside);
    }

    #[test]
    fn test_intersects() {
        let bbox1 = AABox::new(Vector3::ZERO, Vector3::new(1.0, 1.0, 1.0));
        let bbox2 = AABox::new(Vector3::new(0.5, 0.5, 0.5), Vector3::new(1.0, 1.0, 1.0)); // Intersecting
        let bbox3 = AABox::new(Vector3::new(3.0, 3.0, 3.0), Vector3::new(1.0, 1.0, 1.0)); // Outside

        assert!(bbox1.intersects(&bbox2));
        assert!(!bbox1.intersects(&bbox3));
    }

    #[test]
    fn test_project_to_axis() {
        let bbox = AABox::new(Vector3::ZERO, Vector3::new(1.0, 2.0, 3.0));
        let axis = Vector3::new(1.0, 0.0, 0.0); // X-axis

        assert_eq!(bbox.project_to_axis(axis), 1.0); // Just the X extent

        let axis = Vector3::new(1.0, 1.0, 1.0); // Diagonal
        assert_eq!(bbox.project_to_axis(axis), 6.0); // Sum of all extents
    }

    #[test]
    fn test_translate() {
        let mut bbox = AABox::new(Vector3::ZERO, Vector3::new(1.0, 1.0, 1.0));
        let translation = Vector3::new(2.0, 3.0, 4.0);

        bbox.translate(translation);

        assert_eq!(bbox.center, translation);
        assert_eq!(bbox.extent, Vector3::new(1.0, 1.0, 1.0)); // Extent unchanged
    }

    #[test]
    fn test_minmax_aabox() {
        let min = Vector3::new(-1.0, -2.0, -3.0);
        let max = Vector3::new(1.0, 2.0, 3.0);
        let minmax = MinMaxAABox::new(min, max);

        assert_eq!(minmax.min_corner, min);
        assert_eq!(minmax.max_corner, max);
        assert_eq!(minmax.volume(), 2.0 * 4.0 * 6.0); // width * height * depth
    }

    #[test]
    fn test_minmax_add_point() {
        let mut minmax = MinMaxAABox::new(Vector3::ZERO, Vector3::new(1.0, 1.0, 1.0));

        minmax.add_point(Vector3::new(2.0, 3.0, 4.0));

        assert_eq!(minmax.min_corner, Vector3::ZERO);
        assert_eq!(minmax.max_corner, Vector3::new(2.0, 3.0, 4.0));
    }

    #[test]
    fn test_conversions() {
        let aabox = AABox::new(Vector3::new(1.0, 2.0, 3.0), Vector3::new(0.5, 1.0, 1.5));
        let minmax = MinMaxAABox::from(aabox);
        let aabox2 = AABox::from(minmax);

        // Should be equivalent after conversion
        assert!((aabox.center - aabox2.center).length() < EPSILON);
        assert!((aabox.extent - aabox2.extent).length() < EPSILON);
    }

    #[test]
    fn test_equality() {
        let bbox1 = AABox::new(Vector3::new(1.0, 2.0, 3.0), Vector3::new(0.5, 1.0, 1.5));
        let bbox2 = AABox::new(Vector3::new(1.0, 2.0, 3.0), Vector3::new(0.5, 1.0, 1.5));
        let bbox3 = AABox::new(Vector3::new(1.0, 2.0, 3.0), Vector3::new(0.6, 1.0, 1.5));

        assert_eq!(bbox1, bbox2);
        assert_ne!(bbox1, bbox3);
    }

    #[test]
    fn test_empty_minmax_box() {
        let mut minmax = MinMaxAABox::empty();
        assert_eq!(
            minmax.min_corner,
            Vector3::new(f32::MAX, f32::MAX, f32::MAX)
        );
        assert_eq!(
            minmax.max_corner,
            Vector3::new(f32::MIN, f32::MIN, f32::MIN)
        );

        minmax.add_point(Vector3::new(1.0, 2.0, 3.0));
        assert_eq!(minmax.min_corner, Vector3::new(1.0, 2.0, 3.0));
        assert_eq!(minmax.max_corner, Vector3::new(1.0, 2.0, 3.0));
    }
}
