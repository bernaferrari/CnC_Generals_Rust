//! OBBox - Oriented bounding box utilities.
//!
//! This module provides an oriented bounding box type that mirrors the behaviour of the
//! original `OBBoxClass` present in the C++ renderer while embracing glam math utilities.

use glam::{Mat3, Mat4, Vec3};

/// Oriented bounding box (matches C++ OBBoxClass semantics)
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct OBBoxClass {
    /// Center of the box in object space
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
            extent: extent.abs(),
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
        self.extent = extent.abs();
    }

    /// Returns the eight world-space corners of the OBB.
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

        offsets.map(|offset| {
            let scaled = Vec3::new(
                offset.x * self.extent.x,
                offset.y * self.extent.y,
                offset.z * self.extent.z,
            );
            self.center
                + self.basis[0] * scaled.x
                + self.basis[1] * scaled.y
                + self.basis[2] * scaled.z
        })
    }

    /// Checks whether a point lies inside the OBB (inclusive).
    pub fn contains_point(&self, point: Vec3) -> bool {
        let local = point - self.center;
        for (idx, (axis, extent)) in self.basis.iter().zip(self.extent.to_array()).enumerate() {
            if extent <= f32::EPSILON {
                continue;
            }

            let len = axis.length();
            let (axis_dir, axis_len) = if len > 1e-6 {
                (*axis / len, len)
            } else {
                (
                    match idx {
                        0 => Vec3::X,
                        1 => Vec3::Y,
                        _ => Vec3::Z,
                    },
                    1.0,
                )
            };

            let world_extent = extent * axis_len;
            let projection = local.dot(axis_dir);
            if projection.abs() > world_extent + f32::EPSILON {
                return false;
            }
        }
        true
    }

    /// Returns the closest point on the OBB to the supplied point.
    pub fn closest_point(&self, point: Vec3) -> Vec3 {
        let mut result = self.center;
        let local = point - self.center;

        for (idx, (axis, extent)) in self.basis.iter().zip(self.extent.to_array()).enumerate() {
            if extent <= f32::EPSILON {
                continue;
            }

            let len = axis.length();
            let (axis_dir, axis_len) = if len > 1e-6 {
                (*axis / len, len)
            } else {
                (
                    match idx {
                        0 => Vec3::X,
                        1 => Vec3::Y,
                        _ => Vec3::Z,
                    },
                    1.0,
                )
            };

            let world_extent = extent * axis_len;
            let projection = local.dot(axis_dir);
            let clamped = projection.clamp(-world_extent, world_extent);
            result += axis_dir * clamped;
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

    /// Computes an axis-aligned bounding box that encloses this OBB.
    pub fn to_aabox(&self) -> super::AABoxClass {
        let mut min = Vec3::splat(f32::INFINITY);
        let mut max = Vec3::splat(f32::NEG_INFINITY);
        for corner in &self.get_corners() {
            min = min.min(*corner);
            max = max.max(*corner);
        }
        super::AABoxClass::from_min_max(min, max)
    }

    /// Backwards-compatible helper for call sites that previously requested the bounding AABox.
    pub fn bounding_aabox(&self) -> super::AABoxClass {
        self.to_aabox()
    }

    /// Conservative intersection test against an AABox.
    pub fn intersects_aabox(&self, aabox: &super::AABoxClass) -> bool {
        let obb_aabb = self.to_aabox();
        if !obb_aabb.intersects_aabox(aabox) {
            return false;
        }

        // Check corners of each volume against the other for a quick overlap test.
        if self
            .get_corners()
            .iter()
            .any(|corner| aabox.contains_point(corner))
        {
            return true;
        }

        for corner in aabox_corners(aabox) {
            if self.contains_point(corner) {
                return true;
            }
        }

        false
    }

    /// Basic intersection test against another OBB using their bounding AABBs.
    pub fn intersects_obbox(&self, other: &OBBoxClass) -> bool {
        self.to_aabox().intersects_aabox(&other.to_aabox())
    }

    /// Intersection test against a sphere.
    pub fn intersects_sphere(&self, sphere: &super::SphereClass) -> bool {
        self.distance_squared_to_point(sphere.center) <= sphere.radius * sphere.radius
    }

    /// Returns the world-space volume of the box.
    pub fn volume(&self) -> f32 {
        let mut volume = 8.0;
        for (axis, extent) in self.basis.iter().zip(self.extent.to_array()) {
            volume *= (extent * axis.length()).max(0.0);
        }
        volume
    }

    /// Transform this OBB by an affine matrix.
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

fn aabox_corners(box_obj: &super::AABoxClass) -> [Vec3; 8] {
    let min = box_obj.min();
    let max = box_obj.max();

    [
        Vec3::new(min.x, min.y, min.z),
        Vec3::new(max.x, min.y, min.z),
        Vec3::new(max.x, max.y, min.z),
        Vec3::new(min.x, max.y, min.z),
        Vec3::new(min.x, min.y, max.z),
        Vec3::new(max.x, min.y, max.z),
        Vec3::new(max.x, max.y, max.z),
        Vec3::new(min.x, max.y, max.z),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
