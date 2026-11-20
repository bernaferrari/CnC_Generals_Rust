//! Glam-backed 4D vector alias with WWMath compatibility helpers.

use crate::WWMath;
use glam::Vec4;

/// Primary 4D vector type. Glam supplies native alignment and arithmetic.
/// Use component arrays at binary boundaries with original WWMath data.
pub type Vector4 = Vec4;

/// Additional WWMath-era helpers for `Vector4`.
pub trait Vector4Ext {
    /// Set the four components in X, Y, Z, W order.
    fn set(&mut self, x: f32, y: f32, z: f32, w: f32);
    /// Copy all four components from another vector.
    fn set_from(&mut self, other: Vector4);
    /// Apply portable WWMath normalization, retaining the bits of a zero vector.
    fn normalize_in_place(&mut self);
    /// Apply portable WWMath normalization, returning canonical zero for zero length.
    fn normalized_or_zero(self) -> Vector4;
    /// Test whether all four components are finite.
    fn is_valid(&self) -> bool;
    /// Store the result of the original value-returning interpolation.
    fn lerp_into(result: &mut Vector4, a: Vector4, b: Vector4, alpha: f32);
    /// Exchange the components of distinct vectors.
    fn swap(a: &mut Vector4, b: &mut Vector4);
}

fn wwmath_length_squared(vector: Vector4) -> f32 {
    vector.x * vector.x + vector.y * vector.y + vector.z * vector.z + vector.w * vector.w
}

impl Vector4Ext for Vector4 {
    fn set(&mut self, x: f32, y: f32, z: f32, w: f32) {
        *self = Vector4::new(x, y, z, w);
    }

    fn set_from(&mut self, other: Vector4) {
        *self = other;
    }

    fn normalize_in_place(&mut self) {
        let len2 = wwmath_length_squared(*self);
        if len2 != 0.0 {
            *self *= 1.0 / (f64::from(len2).sqrt() as f32);
        }
    }

    fn normalized_or_zero(self) -> Vector4 {
        let len2 = wwmath_length_squared(self);
        if len2 != 0.0 {
            self * (1.0 / (f64::from(len2).sqrt() as f32))
        } else {
            Vector4::ZERO
        }
    }

    fn is_valid(&self) -> bool {
        WWMath::is_valid_float(self.x)
            && WWMath::is_valid_float(self.y)
            && WWMath::is_valid_float(self.z)
            && WWMath::is_valid_float(self.w)
    }

    fn lerp_into(result: &mut Vector4, a: Vector4, b: Vector4, alpha: f32) {
        *result = a + (b - a) * alpha;
    }

    fn swap(a: &mut Vector4, b: &mut Vector4) {
        std::mem::swap(a, b);
    }
}
