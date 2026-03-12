//! Vector4 Module
//!
//! Idiomatic Rust port of `Tools/WW3D/pluglib/vector4.h`.

use core::ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Neg, Sub, SubAssign};

/// 4D vector storing `f32` components.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vector4 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub w: f32,
}

impl Vector4 {
    /// Zero vector.
    pub const ZERO: Self = Self::new(0.0, 0.0, 0.0, 0.0);

    /// Construct from components.
    #[inline]
    #[must_use]
    pub const fn new(x: f32, y: f32, z: f32, w: f32) -> Self {
        Self { x, y, z, w }
    }

    /// Construct from slice.
    #[inline]
    #[must_use]
    pub fn from_slice(slice: &[f32]) -> Self {
        debug_assert!(slice.len() >= 4, "Vector4::from_slice expects at least 4 elements");
        Self::new(slice[0], slice[1], slice[2], slice[3])
    }

    /// Convert to array.
    #[inline]
    #[must_use]
    pub const fn to_array(self) -> [f32; 4] {
        [self.x, self.y, self.z, self.w]
    }

    /// Dot product.
    #[inline]
    #[must_use]
    pub fn dot(self, other: Self) -> f32 {
        self.x * other.x + self.y * other.y + self.z * other.z + self.w * other.w
    }

    /// Length squared.
    #[inline]
    #[must_use]
    pub fn length_squared(self) -> f32 {
        self.dot(self)
    }

    /// Vector length.
    #[inline]
    #[must_use]
    pub fn length(self) -> f32 {
        self.length_squared().sqrt()
    }

    /// Normalized vector (returns zero when length is too small).
    #[inline]
    #[must_use]
    pub fn normalize(self) -> Self {
        let len = self.length();
        if len > f32::EPSILON {
            self / len
        } else {
            Self::ZERO
        }
    }
}

impl Default for Vector4 {
    fn default() -> Self {
        Self::ZERO
    }
}

impl From<[f32; 4]> for Vector4 {
    fn from(value: [f32; 4]) -> Self {
        Self::new(value[0], value[1], value[2], value[3])
    }
}

impl From<Vector4> for [f32; 4] {
    fn from(value: Vector4) -> Self {
        value.to_array()
    }
}

impl Add for Vector4 {
    type Output = Self;

    #[inline]
    fn add(self, rhs: Self) -> Self::Output {
        Self::new(self.x + rhs.x, self.y + rhs.y, self.z + rhs.z, self.w + rhs.w)
    }
}

impl AddAssign for Vector4 {
    #[inline]
    fn add_assign(&mut self, rhs: Self) {
        self.x += rhs.x;
        self.y += rhs.y;
        self.z += rhs.z;
        self.w += rhs.w;
    }
}

impl Sub for Vector4 {
    type Output = Self;

    #[inline]
    fn sub(self, rhs: Self) -> Self::Output {
        Self::new(self.x - rhs.x, self.y - rhs.y, self.z - rhs.z, self.w - rhs.w)
    }
}

impl SubAssign for Vector4 {
    #[inline]
    fn sub_assign(&mut self, rhs: Self) {
        self.x -= rhs.x;
        self.y -= rhs.y;
        self.z -= rhs.z;
        self.w -= rhs.w;
    }
}

impl Mul<f32> for Vector4 {
    type Output = Self;

    #[inline]
    fn mul(self, rhs: f32) -> Self::Output {
        Self::new(self.x * rhs, self.y * rhs, self.z * rhs, self.w * rhs)
    }
}

impl MulAssign<f32> for Vector4 {
    #[inline]
    fn mul_assign(&mut self, rhs: f32) {
        self.x *= rhs;
        self.y *= rhs;
        self.z *= rhs;
        self.w *= rhs;
    }
}

impl Mul<Vector4> for f32 {
    type Output = Vector4;

    #[inline]
    fn mul(self, rhs: Vector4) -> Self::Output {
        rhs * self
    }
}

impl Div<f32> for Vector4 {
    type Output = Self;

    #[inline]
    fn div(self, rhs: f32) -> Self::Output {
        debug_assert!(rhs != 0.0, "Division by zero in Vector4::div");
        let inv = 1.0 / rhs;
        Self::new(self.x * inv, self.y * inv, self.z * inv, self.w * inv)
    }
}

impl DivAssign<f32> for Vector4 {
    #[inline]
    fn div_assign(&mut self, rhs: f32) {
        debug_assert!(rhs != 0.0, "Division by zero in Vector4::div_assign");
        let inv = 1.0 / rhs;
        self.x *= inv;
        self.y *= inv;
        self.z *= inv;
        self.w *= inv;
    }
}

impl Neg for Vector4 {
    type Output = Self;

    #[inline]
    fn neg(self) -> Self::Output {
        Self::new(-self.x, -self.y, -self.z, -self.w)
    }
}

#[cfg(test)]
mod tests {
    use super::Vector4;

    #[test]
    fn basic_ops() {
        let mut v = Vector4::new(1.0, 2.0, 3.0, 4.0);
        v += Vector4::new(1.0, 1.0, 1.0, 1.0);
        assert_eq!(v, Vector4::new(2.0, 3.0, 4.0, 5.0));
        v *= 2.0;
        assert_eq!(v, Vector4::new(4.0, 6.0, 8.0, 10.0));
        v = v.normalize();
        assert!((v.length() - 1.0).abs() < 1e-6);
    }
}
