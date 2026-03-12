//! Vector2 Module
//!
//! Idiomatic Rust port of `Tools/WW3D/pluglib/vector2.h` providing
//! lightweight 2D vector mathematics.

use core::ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Neg, Sub, SubAssign};

/// 2D floating point vector.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vector2 {
    pub x: f32,
    pub y: f32,
}

impl Vector2 {
    /// Zero vector.
    pub const ZERO: Self = Self::new(0.0, 0.0);

    /// Unit X vector.
    pub const X: Self = Self::new(1.0, 0.0);

    /// Unit Y vector.
    pub const Y: Self = Self::new(0.0, 1.0);

    /// Construct from components.
    #[inline]
    #[must_use]
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    /// Construct from slice.
    #[inline]
    #[must_use]
    pub fn from_slice(slice: &[f32]) -> Self {
        debug_assert!(slice.len() >= 2, "Vector2::from_slice expects at least 2 elements");
        Self::new(slice[0], slice[1])
    }

    /// Convert to array.
    #[inline]
    #[must_use]
    pub const fn to_array(self) -> [f32; 2] {
        [self.x, self.y]
    }

    /// Dot product.
    #[inline]
    #[must_use]
    pub fn dot(self, other: Self) -> f32 {
        self.x * other.x + self.y * other.y
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

    /// Normalize the vector (returns zero when the length is too small).
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

    /// Linear interpolation.
    #[inline]
    #[must_use]
    pub fn lerp(self, other: Self, t: f32) -> Self {
        self + (other - self) * t
    }

    /// Returns whether all components are finite numbers.
    #[inline]
    #[must_use]
    pub fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite()
    }
}

impl Default for Vector2 {
    fn default() -> Self {
        Self::ZERO
    }
}

impl From<[f32; 2]> for Vector2 {
    fn from(value: [f32; 2]) -> Self {
        Self::new(value[0], value[1])
    }
}

impl From<Vector2> for [f32; 2] {
    fn from(value: Vector2) -> Self {
        value.to_array()
    }
}

impl Add for Vector2 {
    type Output = Self;

    #[inline]
    fn add(self, rhs: Self) -> Self::Output {
        Self::new(self.x + rhs.x, self.y + rhs.y)
    }
}

impl AddAssign for Vector2 {
    #[inline]
    fn add_assign(&mut self, rhs: Self) {
        self.x += rhs.x;
        self.y += rhs.y;
    }
}

impl Sub for Vector2 {
    type Output = Self;

    #[inline]
    fn sub(self, rhs: Self) -> Self::Output {
        Self::new(self.x - rhs.x, self.y - rhs.y)
    }
}

impl SubAssign for Vector2 {
    #[inline]
    fn sub_assign(&mut self, rhs: Self) {
        self.x -= rhs.x;
        self.y -= rhs.y;
    }
}

impl Mul<f32> for Vector2 {
    type Output = Self;

    #[inline]
    fn mul(self, rhs: f32) -> Self::Output {
        Self::new(self.x * rhs, self.y * rhs)
    }
}

impl MulAssign<f32> for Vector2 {
    #[inline]
    fn mul_assign(&mut self, rhs: f32) {
        self.x *= rhs;
        self.y *= rhs;
    }
}

impl Mul<Vector2> for f32 {
    type Output = Vector2;

    #[inline]
    fn mul(self, rhs: Vector2) -> Self::Output {
        rhs * self
    }
}

impl Div<f32> for Vector2 {
    type Output = Self;

    #[inline]
    fn div(self, rhs: f32) -> Self::Output {
        debug_assert!(rhs != 0.0, "Division by zero in Vector2::div");
        let inv = 1.0 / rhs;
        Self::new(self.x * inv, self.y * inv)
    }
}

impl DivAssign<f32> for Vector2 {
    #[inline]
    fn div_assign(&mut self, rhs: f32) {
        debug_assert!(rhs != 0.0, "Division by zero in Vector2::div_assign");
        let inv = 1.0 / rhs;
        self.x *= inv;
        self.y *= inv;
    }
}

impl Neg for Vector2 {
    type Output = Self;

    #[inline]
    fn neg(self) -> Self::Output {
        Self::new(-self.x, -self.y)
    }
}

#[cfg(test)]
mod tests {
    use super::Vector2;

    #[test]
    fn basics() {
        let mut v = Vector2::new(1.0, 2.0);
        v += Vector2::new(1.0, -1.0);
        assert_eq!(v, Vector2::new(2.0, 1.0));
        assert!((v.length() - (5.0f32).sqrt()).abs() < 1e-6);
        assert_eq!(v.normalize().length(), 1.0);
    }
}
