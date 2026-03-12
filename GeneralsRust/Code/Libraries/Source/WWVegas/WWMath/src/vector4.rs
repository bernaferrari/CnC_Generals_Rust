//! Glam-backed 4D vector alias with WWMath compatibility helpers.

use crate::WWMath;
use glam::Vec4;

/// Primary 4D vector type used throughout the math library.
pub type Vector4 = Vec4;

/// Additional WWMath-era helpers for `Vector4`.
pub trait Vector4Ext {
    fn set(&mut self, x: f32, y: f32, z: f32, w: f32);
    fn set_from(&mut self, other: Vector4);
    fn normalize_in_place(&mut self);
    fn normalized_or_zero(self) -> Vector4;
    fn is_valid(&self) -> bool;
    fn lerp_into(result: &mut Vector4, a: Vector4, b: Vector4, alpha: f32);
    fn swap(a: &mut Vector4, b: &mut Vector4);
}

impl Vector4Ext for Vector4 {
    fn set(&mut self, x: f32, y: f32, z: f32, w: f32) {
        *self = Vector4::new(x, y, z, w);
    }

    fn set_from(&mut self, other: Vector4) {
        *self = other;
    }

    fn normalize_in_place(&mut self) {
        let len2 = self.length_squared();
        if len2 != 0.0 {
            let inv_len = WWMath::inv_sqrt(len2);
            *self *= inv_len;
        }
    }

    fn normalized_or_zero(self) -> Vector4 {
        let len2 = self.length_squared();
        if len2 != 0.0 {
            let inv_len = WWMath::inv_sqrt(len2);
            self * inv_len
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
        *result = Vector4::lerp(a, b, alpha);
    }

    fn swap(a: &mut Vector4, b: &mut Vector4) {
        std::mem::swap(a, b);
    }
}

#[cfg(test)]
mod tests {
    use super::Vector4Ext;
    use super::*;

    #[test]
    fn normalize_in_place_matches_expected() {
        let mut v = Vector4::new(0.0, 3.0, 0.0, 0.0);
        v.normalize_in_place();
        assert!((v.length() - 1.0).abs() < 1e-6);
    }

    #[test]
    fn lerp_into_produces_expected_result() {
        let mut result = Vector4::ZERO;
        let a = Vector4::ZERO;
        let b = Vector4::new(10.0, 20.0, 30.0, 40.0);
        Vector4Ext::lerp_into(&mut result, a, b, 0.5);
        assert_eq!(result, Vector4::new(5.0, 10.0, 15.0, 20.0));
    }
}
