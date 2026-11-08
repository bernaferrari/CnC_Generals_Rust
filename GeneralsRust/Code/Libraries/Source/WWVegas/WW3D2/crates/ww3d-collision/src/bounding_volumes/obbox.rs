//! WWMath `obbox.h` representation: center + basis * local coordinates.
//!
//! Construction, affine transformation, parametric points, axis projection, and
//! enclosing AABox extents retain the C++ coefficients without normalization.
//! Collision SAT requires orthonormal bases and nonnegative extents, as in
//! `colmathobbobb.cpp`. Point containment/closest-point are Rust extensions for
//! nonzero orthogonal (optionally scaled) axes; they do not support shear.
//! Affine transforms may introduce shear: corners/projections/bounds and the
//! Rust world-volume extension remain valid, not orthogonal collision queries.

use glam::{Mat3, Mat4, Vec3};

/// Oriented bounding box; see the module's affine versus collision contracts.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct OBBoxClass {
    /// Center in the coordinate space of the box.
    pub center: Vec3,
    /// Half-extents along each local axis
    pub extent: Vec3,
    /// Basis vectors describing orientation (columns of a 3x3 matrix)
    pub basis: [Vec3; 3],
}

impl OBBoxClass {
    /// Create an empty OBBox located at the origin.
    pub fn empty() -> Self {
        Self::from_center_extent(Vec3::ZERO, Vec3::ZERO)
    }

    /// Create an OBBox aligned with the world axes.
    pub fn from_center_extent(center: Vec3, extent: Vec3) -> Self {
        Self::new(center, extent, [Vec3::X, Vec3::Y, Vec3::Z])
    }

    /// Create an OBBox from an AABox definition.
    pub fn from_aabox(aabox: &super::AABoxClass) -> Self {
        Self::from_center_extent(aabox.center, aabox.extent)
    }

    /// Create an OBBox from center, extent and basis vectors.
    pub fn new(center: Vec3, extent: Vec3, basis: [Vec3; 3]) -> Self {
        Self {
            center,
            extent,
            basis,
        }
    }

    /// Backwards-compatible helper mirroring the historical constructor name.
    pub fn from_center_extent_basis(center: Vec3, extent: Vec3, basis: [Vec3; 3]) -> Self {
        Self::new(center, extent, basis)
    }

    /// Returns the center of the box.
    pub fn center(&self) -> Vec3 {
        self.center
    }

    /// Returns the half-extents.
    pub fn extent(&self) -> Vec3 {
        self.extent
    }

    /// Sets the center.
    pub fn set_center(&mut self, center: Vec3) {
        self.center = center;
    }

    /// Sets the half-extents.
    pub fn set_extent(&mut self, extent: Vec3) {
        self.extent = extent;
    }

    /// Returns the eight corners (also valid for affine, sheared bases).
    pub fn get_corners(&self) -> [Vec3; 8] {
        let offsets = [
            Vec3::new(-1.0, -1.0, -1.0),
            Vec3::new(1.0, -1.0, -1.0),
            Vec3::new(1.0, 1.0, -1.0),
            Vec3::new(-1.0, 1.0, -1.0),
            Vec3::new(-1.0, -1.0, 1.0),
            Vec3::new(1.0, -1.0, 1.0),
            Vec3::new(1.0, 1.0, 1.0),
            Vec3::new(-1.0, 1.0, 1.0),
        ];

        offsets.map(|offset| self.compute_point(offset))
    }

    /// Inclusive containment for nonzero orthogonal axes and nonnegative extents.
    /// On unit axes this is C++ `Overlap_Test(OBBox, point)`; division by squared
    /// axis length extends the operation to orthogonal scaled bases.
    pub fn contains_point(&self, point: Vec3) -> bool {
        let local = point - self.center;
        self.basis.iter().zip(self.extent.to_array()).all(|(axis, extent)| {
            (local.dot(*axis) / axis.length_squared()).abs() <= extent
        })
    }

    /// Rust extension: closest point for nonzero orthogonal axes.
    /// Extents must be nonnegative. A zero extent is a plane, not an omitted axis.
    pub fn closest_point(&self, point: Vec3) -> Vec3 {
        let mut result = self.center;
        let local = point - self.center;
        for (axis, extent) in self.basis.iter().zip(self.extent.to_array()) {
            let projection = local.dot(*axis) / axis.length_squared();
            result += *axis * projection.clamp(-extent, extent);
        }
        result
    }

    /// Distance from the box surface to the supplied point.
    pub fn distance_to_point(&self, point: Vec3) -> f32 {
        (point - self.closest_point(point)).length()
    }

    /// Squared distance from the box surface to the supplied point.
    pub fn distance_squared_to_point(&self, point: Vec3) -> f32 {
        (point - self.closest_point(point)).length_squared()
    }

    /// C++ `Compute_Point`: params in [-1, 1] select a point in the box.
    pub fn compute_point(&self, params: Vec3) -> Vec3 {
        let point = self.extent * params;
        let rotated = self.basis[0] * point.x + self.basis[1] * point.y + self.basis[2] * point.z;
        self.center + rotated
    }

    /// C++ `Project_To_Axis`; the projection axis need not have unit length.
    pub fn project_to_axis(&self, axis: Vec3) -> f32 {
        (self.extent.x * axis.dot(self.basis[0])).abs()
            + (self.extent.y * axis.dot(self.basis[1])).abs()
            + (self.extent.z * axis.dot(self.basis[2])).abs()
    }

    /// C++ `Compute_Axis_Aligned_Extent`, valid even for an affine basis.
    pub fn compute_axis_aligned_extent(&self) -> Vec3 {
        (self.basis[0] * self.extent.x).abs()
            + (self.basis[1] * self.extent.y).abs()
            + (self.basis[2] * self.extent.z).abs()
    }

    /// Enclosing AABox without subtracting large, nearly equal corner positions.
    pub fn to_aabox(&self) -> super::AABoxClass {
        super::AABoxClass::from_center_and_extent(self.center, self.compute_axis_aligned_extent())
    }

    /// Backwards-compatible helper for call sites that previously requested the bounding AABox.
    pub fn bounding_aabox(&self) -> super::AABoxClass {
        self.to_aabox()
    }

    /// C++ `Intersection_Test(OBBox, AABox)`: the same 15-axis SAT as OBBox/OBBox.
    pub fn intersects_aabox(&self, aabox: &super::AABoxClass) -> bool {
        self.intersects_obbox(&Self::from_aabox(aabox))
    }

    /// C++ 15-axis SAT; both bases must be orthonormal and extents nonnegative.
    pub fn intersects_obbox(&self, other: &OBBoxClass) -> bool {
        crate::collision_math::CollisionMath::obbox_obbox_intersect(self, other)
    }

    /// Intersection test against a sphere.
    pub fn intersects_sphere(&self, sphere: &super::SphereClass) -> bool {
        self.distance_squared_to_point(sphere.center) <= sphere.radius * sphere.radius
    }

    /// Rust extension: world volume, including scale/shear/reflection.
    /// On a rotation basis this equals C++ `Volume()` (8 * extent product).
    pub fn volume(&self) -> f32 {
        let determinant = self.basis[0].dot(self.basis[1].cross(self.basis[2]));
        (8.0 * self.extent.x * self.extent.y * self.extent.z * determinant).abs()
    }

    /// C++ affine transform: multiply basis and center, leaving extents unchanged.
    pub fn transformed(&self, transform: Mat4) -> Self {
        let center = transform.transform_point3(self.center);

        let rotation = Mat3::from_mat4(transform);
        let mut basis = self.basis;
        basis[0] = rotation.mul_vec3(basis[0]);
        basis[1] = rotation.mul_vec3(basis[1]);
        basis[2] = rotation.mul_vec3(basis[2]);

        Self {
            center,
            extent: self.extent,
            basis,
        }
    }

    /// Backwards-compatible helper retaining the historic method name.
    pub fn transform(&self, transform: Mat4) -> Self {
        self.transformed(transform)
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enclosing_bounds_do_not_lose_extents_at_large_centers() {
        let obbox = OBBoxClass::from_center_extent(Vec3::splat(1e8), Vec3::ONE);
        let bounds = obbox.to_aabox();
        assert_eq!(bounds.center, obbox.center);
        assert_eq!(bounds.extent, Vec3::ONE);
    }

    #[test]
    fn zero_extents_and_tiny_extents_are_not_unbounded() {
        let flat = OBBoxClass::from_center_extent(Vec3::ZERO, Vec3::new(0.0, 1.0, 1.0));
        assert!(flat.contains_point(Vec3::Y));
        assert!(!flat.contains_point(Vec3::X * f32::EPSILON));
        assert_eq!(flat.closest_point(Vec3::new(3.0, 2.0, 0.0)), Vec3::Y);
        let tiny = OBBoxClass::from_center_extent(Vec3::ZERO, Vec3::splat(1e-8));
        assert!(tiny.contains_point(Vec3::X * 1e-8));
        assert!(!tiny.contains_point(Vec3::X * 2e-8));
        assert_eq!(tiny.closest_point(Vec3::X), Vec3::X * 1e-8);
        let point = OBBoxClass::empty();
        assert!(point.contains_point(Vec3::ZERO));
        assert!(!point.contains_point(Vec3::Z));
    }

    #[test]
    fn aabox_crossing_without_contained_corners_intersects() {
        let horizontal = OBBoxClass::from_center_extent(Vec3::ZERO, Vec3::new(3.0, 0.5, 0.5));
        let vertical = super::super::AABoxClass::from_center_and_extent(
            Vec3::ZERO, Vec3::new(0.5, 3.0, 0.5),
        );
        assert!(horizontal.intersects_aabox(&vertical));
        let touching = super::super::AABoxClass::from_center_and_extent(
            Vec3::new(3.5, 0.0, 0.0), Vec3::splat(0.5),
        );
        assert!(horizontal.intersects_aabox(&touching));
        let separated = super::super::AABoxClass::from_center_and_extent(
            Vec3::new(3.50001, 0.0, 0.0), Vec3::splat(0.5),
        );
        assert!(!horizontal.intersects_aabox(&separated));
    }

    #[test]
    fn affine_points_projection_and_bounds_preserve_basis_and_extents() {
        let obb = OBBoxClass::from_center_extent(Vec3::new(1.0, 2.0, 3.0), Vec3::new(2.0, 3.0, 4.0));
        let affine = Mat4::from_cols(
            glam::Vec4::new(-2.0, 0.0, 0.0, 0.0),
            glam::Vec4::new(1.0, 3.0, 0.0, 0.0),
            glam::Vec4::new(0.0, 0.0, 4.0, 0.0),
            glam::Vec4::new(7.0, 8.0, 9.0, 1.0),
        );
        let transformed = obb.transformed(affine);
        assert_eq!(transformed.extent, obb.extent);
        for (before, after) in obb.get_corners().into_iter().zip(transformed.get_corners()) {
            assert_eq!(affine.transform_point3(before), after);
        }
        assert_eq!(transformed.project_to_axis(Vec3::X), 7.0);
        assert_eq!(transformed.project_to_axis(Vec3::Y * 2.0), 18.0);
        let bounds = transformed.to_aabox();
        assert_eq!(bounds.center, transformed.center);
        assert_eq!(bounds.extent, Vec3::new(7.0, 9.0, 16.0));
        assert_eq!(transformed.volume(), 8.0 * 2.0 * 3.0 * 4.0 * 24.0);
    }

    #[test]
    fn raw_extents_are_not_silently_rewritten() {
        let mut obb = OBBoxClass::from_center_extent(Vec3::ZERO, -Vec3::ONE);
        assert_eq!(obb.extent, -Vec3::ONE);
        obb.set_extent(Vec3::new(-2.0, 3.0, 4.0));
        assert_eq!(obb.extent, Vec3::new(-2.0, 3.0, 4.0));
    }

    #[test]
    fn obbox_preserves_basis() {
        let basis = [Vec3::new(2.0, 0.0, 0.0), Vec3::Y, Vec3::Z];
        let obb = OBBoxClass::new(Vec3::ZERO, Vec3::splat(1.0), basis);

        assert_eq!(obb.extent, Vec3::splat(1.0));
        assert_eq!(obb.basis[0], Vec3::new(2.0, 0.0, 0.0));
    }

    #[test]
    fn obbox_contains_point() {
        let obb = OBBoxClass::from_center_extent(Vec3::ZERO, Vec3::new(1.0, 2.0, 3.0));
        assert!(obb.contains_point(Vec3::new(0.5, 0.0, 0.0)));
        assert!(!obb.contains_point(Vec3::new(1.1, 0.0, 0.0)));
    }

    #[test]
    fn obbox_contains_point_with_scaled_axes() {
        let mut obb = OBBoxClass::from_center_extent(Vec3::ZERO, Vec3::ONE);
        obb.basis[0] = Vec3::new(2.0, 0.0, 0.0);
        assert!(obb.contains_point(Vec3::new(1.5, 0.0, 0.0)));
        assert!(!obb.contains_point(Vec3::new(2.5, 0.0, 0.0)));
    }

    #[test]
    fn obbox_volume_respects_basis_scale() {
        let mut obb = OBBoxClass::from_center_extent(Vec3::ZERO, Vec3::ONE);
        obb.basis[0] = Vec3::new(2.0, 0.0, 0.0);
        assert!((obb.volume() - 16.0).abs() < 1e-5);
    }

    #[test]
    fn obbox_transform() {
        let obb = OBBoxClass::from_center_extent(Vec3::ZERO, Vec3::ONE);
        let transform = Mat4::from_translation(Vec3::new(1.0, 0.0, 0.0))
            * Mat4::from_rotation_y(std::f32::consts::FRAC_PI_2);

        let transformed = obb.transform(transform);
        assert!((transformed.center - Vec3::new(1.0, 0.0, 0.0)).length() < 1e-5);
        assert!((transformed.extent.x - 1.0).abs() < 1e-5);
    }

    // C++ colmathobbobb.cpp — rotated thin OBB is not its local AABB
    #[test]
    fn intersects_obbox_uses_orientation() {
        let angle = std::f32::consts::FRAC_PI_4;
        let (s, c) = angle.sin_cos();
        let thin = OBBoxClass::new(
            Vec3::ZERO,
            Vec3::new(2.0, 0.05, 0.05),
            [Vec3::new(c, s, 0.0), Vec3::new(-s, c, 0.0), Vec3::Z],
        );
        let beside = OBBoxClass::from_center_extent(Vec3::new(1.5, 0.0, 0.0), Vec3::splat(0.2));
        assert!(!thin.intersects_obbox(&beside));
        let along_diag = OBBoxClass::from_center_extent(Vec3::new(1.0, 1.0, 0.0), Vec3::splat(0.2));
        assert!(thin.intersects_obbox(&along_diag));
    }
}
