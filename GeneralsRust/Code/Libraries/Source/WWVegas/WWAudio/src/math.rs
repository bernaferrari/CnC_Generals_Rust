//! Minimal math primitives mirroring the legacy Vector3/Matrix3D types.
//!
//! The original C++ implementation relies on a fairly feature rich math library. For the purpose
//! of establishing feature parity we provide lightweight equivalents that cover the behaviour
//! required by the audio subsystem (position, velocity, orientation, simple interpolation).

use std::ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Sub, SubAssign};

/// 3D vector with single precision floating point components.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vector3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vector3 {
    pub const ZERO: Self = Self {
        x: 0.0,
        y: 0.0,
        z: 0.0,
    };

    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    pub fn length_squared(self) -> f32 {
        self.x * self.x + self.y * self.y + self.z * self.z
    }

    pub fn length(self) -> f32 {
        self.length_squared().sqrt()
    }

    pub fn normalize(self) -> Self {
        let len = self.length();
        if len <= f32::EPSILON {
            Self::ZERO
        } else {
            self / len
        }
    }

    pub fn dot(self, other: Self) -> f32 {
        self.x * other.x + self.y * other.y + self.z * other.z
    }

    pub fn distance_squared(self, other: Self) -> f32 {
        (self - other).length_squared()
    }

    pub fn distance(self, other: Self) -> f32 {
        self.distance_squared(other).sqrt()
    }

    pub fn lerp(self, other: Self, t: f32) -> Self {
        self + (other - self) * t
    }
}

impl Default for Vector3 {
    fn default() -> Self {
        Self::ZERO
    }
}

impl Add for Vector3 {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self::new(self.x + rhs.x, self.y + rhs.y, self.z + rhs.z)
    }
}

impl AddAssign for Vector3 {
    fn add_assign(&mut self, rhs: Self) {
        self.x += rhs.x;
        self.y += rhs.y;
        self.z += rhs.z;
    }
}

impl Sub for Vector3 {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self::new(self.x - rhs.x, self.y - rhs.y, self.z - rhs.z)
    }
}

impl SubAssign for Vector3 {
    fn sub_assign(&mut self, rhs: Self) {
        self.x -= rhs.x;
        self.y -= rhs.y;
        self.z -= rhs.z;
    }
}

impl Mul<f32> for Vector3 {
    type Output = Self;

    fn mul(self, rhs: f32) -> Self::Output {
        Self::new(self.x * rhs, self.y * rhs, self.z * rhs)
    }
}

impl MulAssign<f32> for Vector3 {
    fn mul_assign(&mut self, rhs: f32) {
        self.x *= rhs;
        self.y *= rhs;
        self.z *= rhs;
    }
}

impl Div<f32> for Vector3 {
    type Output = Self;

    fn div(self, rhs: f32) -> Self::Output {
        Self::new(self.x / rhs, self.y / rhs, self.z / rhs)
    }
}

impl DivAssign<f32> for Vector3 {
    fn div_assign(&mut self, rhs: f32) {
        self.x /= rhs;
        self.y /= rhs;
        self.z /= rhs;
    }
}

/// Simple 4x4 transform matrix capturing orientation and translation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Matrix3D {
    pub rows: [[f32; 4]; 4],
}

impl Matrix3D {
    pub const IDENTITY: Self = Self {
        rows: [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ],
    };

    pub const fn new(rows: [[f32; 4]; 4]) -> Self {
        Self { rows }
    }

    pub fn translation(translation: Vector3) -> Self {
        let mut m = Self::IDENTITY;
        m.rows[3][0] = translation.x;
        m.rows[3][1] = translation.y;
        m.rows[3][2] = translation.z;
        m
    }

    pub fn get_translation(&self) -> Vector3 {
        Vector3::new(self.rows[3][0], self.rows[3][1], self.rows[3][2])
    }

    pub fn set_translation(&mut self, translation: Vector3) {
        self.rows[3][0] = translation.x;
        self.rows[3][1] = translation.y;
        self.rows[3][2] = translation.z;
    }

    pub fn right_vector(&self) -> Vector3 {
        Vector3::new(self.rows[0][0], self.rows[0][1], self.rows[0][2])
    }

    pub fn up_vector(&self) -> Vector3 {
        Vector3::new(self.rows[1][0], self.rows[1][1], self.rows[1][2])
    }

    pub fn forward_vector(&self) -> Vector3 {
        Vector3::new(self.rows[2][0], self.rows[2][1], self.rows[2][2])
    }
}

impl Default for Matrix3D {
    fn default() -> Self {
        Self::IDENTITY
    }
}
