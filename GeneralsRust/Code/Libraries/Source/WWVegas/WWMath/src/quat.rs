//! Quaternion implementation for 3D rotations.
//!
//! This module provides quaternion mathematics for 3D rotations,
//! converted from the original C++ Quaternion class.
//!
//! Quaternions provide an efficient and numerically stable way to represent
//! 3D rotations, avoiding the gimbal lock issues that can occur with Euler angles.

use crate::{Matrix3, Matrix3D, Matrix4, Vector3, WWMath, EPSILON, SQRT2};
use std::ops::{
    Add, AddAssign, Div, DivAssign, Index, IndexMut, Mul, MulAssign, Neg, Sub, SubAssign,
};

/// SLERP epsilon constant for comparison
const SLERP_EPSILON: f32 = 0.001;

/// Cached SLERP information structure
#[derive(Debug, Copy, Clone)]
pub struct SlerpInfo {
    pub sin_t: f32,
    pub theta: f32,
    pub flip: bool,
    pub linear: bool,
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct Quaternion {
    /// X component (imaginary part)
    pub x: f32,
    /// Y component (imaginary part)
    pub y: f32,
    /// Z component (imaginary part)
    pub z: f32,
    /// W component (real part)
    pub w: f32,
}

impl Default for Quaternion {
    fn default() -> Self {
        Self::new()
    }
}

impl Quaternion {
    /// Identity quaternion (no rotation)
    pub const IDENTITY: Quaternion = Quaternion {
        x: 0.0,
        y: 0.0,
        z: 0.0,
        w: 1.0,
    };

    /// Create a new quaternion
    pub fn new() -> Self {
        Self::IDENTITY
    }

    /// Create a quaternion from individual components
    pub fn from_components(x: f32, y: f32, z: f32, w: f32) -> Self {
        Self { x, y, z, w }
    }

    /// Create a quaternion from an axis-angle rotation
    pub fn from_axis_angle(axis: Vector3, angle: f32) -> Self {
        let half_angle = angle * 0.5;
        let sin_half = half_angle.sin();
        let cos_half = half_angle.cos();

        let axis = axis.normalize();

        Self {
            x: axis.x * sin_half,
            y: axis.y * sin_half,
            z: axis.z * sin_half,
            w: cos_half,
        }
    }

    /// Create a quaternion representing rotation around X axis
    pub fn from_rotation_x(theta: f32) -> Self {
        let half_theta = theta * 0.5;
        Self {
            x: half_theta.sin(),
            y: 0.0,
            z: 0.0,
            w: half_theta.cos(),
        }
    }

    /// Create a quaternion representing rotation around Y axis
    pub fn from_rotation_y(theta: f32) -> Self {
        let half_theta = theta * 0.5;
        Self {
            x: 0.0,
            y: half_theta.sin(),
            z: 0.0,
            w: half_theta.cos(),
        }
    }

    /// Create a quaternion representing rotation around Z axis
    pub fn from_rotation_z(theta: f32) -> Self {
        let half_theta = theta * 0.5;
        Self {
            x: 0.0,
            y: 0.0,
            z: half_theta.sin(),
            w: half_theta.cos(),
        }
    }

    /// Create an identity quaternion
    pub fn identity() -> Self {
        Self::IDENTITY
    }

    /// Set quaternion to identity
    pub fn make_identity(&mut self) {
        self.x = 0.0;
        self.y = 0.0;
        self.z = 0.0;
        self.w = 1.0;
    }

    /// Set quaternion components
    pub fn set(&mut self, x: f32, y: f32, z: f32, w: f32) {
        self.x = x;
        self.y = y;
        self.z = z;
        self.w = w;
    }

    /// Scale the quaternion by a scalar
    pub fn scale(&mut self, s: f32) {
        self.x *= s;
        self.y *= s;
        self.z *= s;
        self.w *= s;
    }

    /// Get the square of the magnitude
    pub fn length_squared(&self) -> f32 {
        self.x * self.x + self.y * self.y + self.z * self.z + self.w * self.w
    }

    /// Get the magnitude
    pub fn length(&self) -> f32 {
        self.length_squared().sqrt()
    }

    /// Normalize the quaternion
    pub fn normalize(&mut self) {
        let len = self.length();
        if len > 0.0 {
            let inv_len = 1.0 / len;
            self.x *= inv_len;
            self.y *= inv_len;
            self.z *= inv_len;
            self.w *= inv_len;
        }
    }

    /// Get a normalized copy of the quaternion
    pub fn normalized(&self) -> Self {
        let mut result = *self;
        result.normalize();
        result
    }

    /// Check if the quaternion is normalized (unit length)
    pub fn is_normalized(&self) -> bool {
        (self.length_squared() - 1.0).abs() < EPSILON
    }

    /// Get the conjugate of the quaternion
    pub fn conjugate(&self) -> Self {
        Self {
            x: -self.x,
            y: -self.y,
            z: -self.z,
            w: self.w,
        }
    }

    /// Get the inverse of the quaternion
    pub fn inverse(&self) -> Self {
        let len_squared = self.length_squared();
        if len_squared > 0.0 {
            let inv_len_squared = 1.0 / len_squared;
            Self {
                x: -self.x * inv_len_squared,
                y: -self.y * inv_len_squared,
                z: -self.z * inv_len_squared,
                w: self.w * inv_len_squared,
            }
        } else {
            // Return identity if quaternion has zero length
            Self::IDENTITY
        }
    }

    /// Rotate the quaternion around X axis
    pub fn rotate_x(&mut self, theta: f32) {
        let rot = Self::from_rotation_x(theta);
        *self = rot * *self;
    }

    /// Rotate the quaternion around Y axis
    pub fn rotate_y(&mut self, theta: f32) {
        let rot = Self::from_rotation_y(theta);
        *self = rot * *self;
    }

    /// Rotate the quaternion around Z axis
    pub fn rotate_z(&mut self, theta: f32) {
        let rot = Self::from_rotation_z(theta);
        *self = rot * *self;
    }

    /// Transform (rotate) a vector using this quaternion
    /// This matches the C++ implementation exactly
    pub fn rotate_vector(&self, v: Vector3) -> Vector3 {
        let x = self.w * v.x + (self.y * v.z - v.y * self.z);
        let y = self.w * v.y - (self.x * v.z - v.x * self.z);
        let z = self.w * v.z + (self.x * v.y - v.x * self.y);
        let w = -(self.x * v.x + self.y * v.y + self.z * v.z);

        Vector3::new(
            w * (-self.x) + self.w * x + (y * (-self.z) - (-self.y) * z),
            w * (-self.y) + self.w * y - (x * (-self.z) - (-self.x) * z),
            w * (-self.z) + self.w * z + (x * (-self.y) - (-self.x) * y),
        )
    }

    /// Transform (rotate) a vector using this quaternion (in-place)
    pub fn rotate_vector_in_place(&self, v: &mut Vector3) {
        *v = self.rotate_vector(*v);
    }

    /// Check if the quaternion contains valid float values
    pub fn is_valid(&self) -> bool {
        WWMath::is_valid_float(self.x)
            && WWMath::is_valid_float(self.y)
            && WWMath::is_valid_float(self.z)
            && WWMath::is_valid_float(self.w)
    }

    /// Make this quaternion the closest representation to the given quaternion
    /// (handles the q and -q equivalence)
    pub fn make_closest(&mut self, other: &Self) {
        let dot_product = self.x * other.x + self.y * other.y + self.z * other.z + self.w * other.w;

        if dot_product < 0.0 {
            self.x = -self.x;
            self.y = -self.y;
            self.z = -self.z;
            self.w = -self.w;
        }
    }

    /// Generate a random unit quaternion
    /// Matches the C++ implementation approach
    pub fn randomize(&mut self) {
        // Use a simple random approach matching C++ style
        self.x = WWMath::random_float();
        self.y = WWMath::random_float();
        self.z = WWMath::random_float();
        self.w = WWMath::random_float();

        self.normalize();
    }

    /// Convert quaternion to Matrix3
    pub fn to_matrix3(&self) -> Matrix3 {
        let mut matrix = Matrix3::IDENTITY;

        let xx = self.x * self.x;
        let xy = self.x * self.y;
        let xz = self.x * self.z;
        let xw = self.x * self.w;

        let yy = self.y * self.y;
        let yz = self.y * self.z;
        let yw = self.y * self.w;

        let zz = self.z * self.z;
        let zw = self.z * self.w;

        matrix.row[0].x = 1.0 - 2.0 * (yy + zz);
        matrix.row[0].y = 2.0 * (xy - zw);
        matrix.row[0].z = 2.0 * (xz + yw);

        matrix.row[1].x = 2.0 * (xy + zw);
        matrix.row[1].y = 1.0 - 2.0 * (xx + zz);
        matrix.row[1].z = 2.0 * (yz - xw);

        matrix.row[2].x = 2.0 * (xz - yw);
        matrix.row[2].y = 2.0 * (yz + xw);
        matrix.row[2].z = 1.0 - 2.0 * (xx + yy);

        matrix
    }

    /// Convert quaternion to Matrix3D
    pub fn to_matrix3d(&self) -> Matrix3D {
        let mut matrix = Matrix3D::IDENTITY;

        // Use array indexing like C++ version for exact match
        matrix.row[0].x = 1.0 - 2.0 * (self.y * self.y + self.z * self.z);
        matrix.row[0].y = 2.0 * (self.x * self.y - self.z * self.w);
        matrix.row[0].z = 2.0 * (self.z * self.x + self.y * self.w);

        matrix.row[1].x = 2.0 * (self.x * self.y + self.z * self.w);
        matrix.row[1].y = 1.0 - 2.0 * (self.z * self.z + self.x * self.x);
        matrix.row[1].z = 2.0 * (self.y * self.z - self.x * self.w);

        matrix.row[2].x = 2.0 * (self.z * self.x - self.y * self.w);
        matrix.row[2].y = 2.0 * (self.y * self.z + self.x * self.w);
        matrix.row[2].z = 1.0 - 2.0 * (self.y * self.y + self.x * self.x);

        // No translation
        matrix.row[0].w = 0.0;
        matrix.row[1].w = 0.0;
        matrix.row[2].w = 0.0;

        matrix
    }

    /// Convert quaternion to Matrix4
    pub fn to_matrix4(&self) -> Matrix4 {
        use crate::Vector4;

        Matrix4 {
            row: [
                Vector4::new(
                    1.0 - 2.0 * (self.y * self.y + self.z * self.z),
                    2.0 * (self.x * self.y - self.z * self.w),
                    2.0 * (self.z * self.x + self.y * self.w),
                    0.0,
                ),
                Vector4::new(
                    2.0 * (self.x * self.y + self.z * self.w),
                    1.0 - 2.0 * (self.z * self.z + self.x * self.x),
                    2.0 * (self.y * self.z - self.x * self.w),
                    0.0,
                ),
                Vector4::new(
                    2.0 * (self.z * self.x - self.y * self.w),
                    2.0 * (self.y * self.z + self.x * self.w),
                    1.0 - 2.0 * (self.y * self.y + self.x * self.x),
                    0.0,
                ),
                Vector4::new(0.0, 0.0, 0.0, 1.0),
            ],
        }
    }

    /// Create quaternion from Matrix3 (matches C++ Build_Quaternion exactly)
    pub fn from_matrix3(mat: &Matrix3) -> Self {
        const NXT: [usize; 3] = [1, 2, 0];
        let mut q = Self::IDENTITY;

        // Sum the diagonal of the rotation matrix
        let tr = mat.row[0].x + mat.row[1].y + mat.row[2].z;

        if tr > 0.0 {
            let s = (tr + 1.0).sqrt();
            q.w = s * 0.5;
            let s = 0.5 / s;

            q.x = (mat.row[2].y - mat.row[1].z) * s;
            q.y = (mat.row[0].z - mat.row[2].x) * s;
            q.z = (mat.row[1].x - mat.row[0].y) * s;
        } else {
            // Create lookup table for diagonal elements
            let diag = [mat.row[0].x, mat.row[1].y, mat.row[2].z];
            let mut i = 0;
            if diag[1] > diag[0] {
                i = 1;
            }
            if diag[2] > diag[i] {
                i = 2;
            }

            let j = NXT[i];
            let k = NXT[j];

            let s = ((diag[i] - (diag[j] + diag[k])) + 1.0).sqrt();

            q[i] = s * 0.5;
            let s = if s != 0.0 { 0.5 / s } else { 0.0 };

            // Manual matrix access for different combinations
            let (kj, jk, ji, ij, ki, ik) = match (i, j, k) {
                (0, 1, 2) => (
                    mat.row[2].y,
                    mat.row[1].z,
                    mat.row[1].x,
                    mat.row[0].y,
                    mat.row[2].x,
                    mat.row[0].z,
                ),
                (1, 2, 0) => (
                    mat.row[0].z,
                    mat.row[2].x,
                    mat.row[2].y,
                    mat.row[1].z,
                    mat.row[0].y,
                    mat.row[1].x,
                ),
                (2, 0, 1) => (
                    mat.row[1].x,
                    mat.row[0].y,
                    mat.row[0].z,
                    mat.row[2].x,
                    mat.row[1].z,
                    mat.row[2].y,
                ),
                _ => panic!("Invalid matrix indices"),
            };

            q.w = (kj - jk) * s;
            q[j] = (ji + ij) * s;
            q[k] = (ki + ik) * s;
        }

        q
    }

    /// Create quaternion from Matrix3D (matches C++ Build_Quaternion exactly)
    pub fn from_matrix3d(mat: &Matrix3D) -> Self {
        const NXT: [usize; 3] = [1, 2, 0];
        let mut q = Self::IDENTITY;

        // Sum the diagonal of the rotation matrix
        let tr = mat.row[0].x + mat.row[1].y + mat.row[2].z;

        if tr > 0.0 {
            let s = (tr + 1.0).sqrt();
            q.w = s * 0.5;
            let s = 0.5 / s;

            q.x = (mat.row[2].y - mat.row[1].z) * s;
            q.y = (mat.row[0].z - mat.row[2].x) * s;
            q.z = (mat.row[1].x - mat.row[0].y) * s;
        } else {
            // Create lookup table for diagonal elements
            let diag = [mat.row[0].x, mat.row[1].y, mat.row[2].z];
            let mut i = 0;
            if diag[1] > diag[0] {
                i = 1;
            }
            if diag[2] > diag[i] {
                i = 2;
            }

            let j = NXT[i];
            let k = NXT[j];

            let s = ((diag[i] - (diag[j] + diag[k])) + 1.0).sqrt();

            q[i] = s * 0.5;
            let s = if s != 0.0 { 0.5 / s } else { 0.0 };

            // Manual matrix access for different combinations
            let (kj, jk, ji, ij, ki, ik) = match (i, j, k) {
                (0, 1, 2) => (
                    mat.row[2].y,
                    mat.row[1].z,
                    mat.row[1].x,
                    mat.row[0].y,
                    mat.row[2].x,
                    mat.row[0].z,
                ),
                (1, 2, 0) => (
                    mat.row[0].z,
                    mat.row[2].x,
                    mat.row[2].y,
                    mat.row[1].z,
                    mat.row[0].y,
                    mat.row[1].x,
                ),
                (2, 0, 1) => (
                    mat.row[1].x,
                    mat.row[0].y,
                    mat.row[0].z,
                    mat.row[2].x,
                    mat.row[1].z,
                    mat.row[2].y,
                ),
                _ => panic!("Invalid matrix indices"),
            };

            q.w = (kj - jk) * s;
            q[j] = (ji + ij) * s;
            q[k] = (ki + ik) * s;
        }

        q
    }

    /// Create quaternion from Matrix4 (matches C++ Build_Quaternion exactly)
    pub fn from_matrix4(mat: &Matrix4) -> Self {
        const NXT: [usize; 3] = [1, 2, 0];
        let mut q = Self::IDENTITY;

        // Extract 3x3 rotation part and sum the diagonal
        let tr = mat.row[0].x + mat.row[1].y + mat.row[2].z;

        if tr > 0.0 {
            let s = (tr + 1.0).sqrt();
            q.w = s * 0.5;
            let s = 0.5 / s;

            q.x = (mat.row[2].y - mat.row[1].z) * s;
            q.y = (mat.row[0].z - mat.row[2].x) * s;
            q.z = (mat.row[1].x - mat.row[0].y) * s;
        } else {
            let diag = [mat.row[0].x, mat.row[1].y, mat.row[2].z];
            let mut i = 0;
            if diag[1] > diag[0] {
                i = 1;
            }
            if diag[2] > diag[i] {
                i = 2;
            }

            let j = NXT[i];
            let k = NXT[j];

            let s = ((diag[i] - (diag[j] + diag[k])) + 1.0).sqrt();

            q[i] = s * 0.5;
            let s = if s != 0.0 { 0.5 / s } else { 0.0 };

            // Manual matrix access for different combinations
            let (kj, jk, ji, ij, ki, ik) = match (i, j, k) {
                (0, 1, 2) => (
                    mat.row[2].y,
                    mat.row[1].z,
                    mat.row[1].x,
                    mat.row[0].y,
                    mat.row[2].x,
                    mat.row[0].z,
                ),
                (1, 2, 0) => (
                    mat.row[0].z,
                    mat.row[2].x,
                    mat.row[2].y,
                    mat.row[1].z,
                    mat.row[0].y,
                    mat.row[1].x,
                ),
                (2, 0, 1) => (
                    mat.row[1].x,
                    mat.row[0].y,
                    mat.row[0].z,
                    mat.row[2].x,
                    mat.row[1].z,
                    mat.row[2].y,
                ),
                _ => panic!("Invalid matrix indices"),
            };

            q.w = (kj - jk) * s;
            q[j] = (ji + ij) * s;
            q[k] = (ki + ik) * s;
        }

        q
    }

    /// Spherical linear interpolation between two quaternions (accurate version)
    /// Matches the C++ Slerp function exactly
    pub fn slerp(p: Self, q: Self, alpha: f32) -> Self {
        let mut result = Self::IDENTITY;
        slerp_impl(&mut result, p, q, alpha);
        result
    }

    /// In-place SLERP
    pub fn slerp_inplace(result: &mut Self, p: Self, q: Self, alpha: f32) {
        slerp_impl(result, p, q, alpha);
    }

    /// Fast spherical linear interpolation (approximate)
    pub fn fast_slerp(p: Self, q: Self, alpha: f32) -> Self {
        let mut result = Self::IDENTITY;
        fast_slerp_impl(&mut result, p, q, alpha);
        result
    }

    /// Setup cached SLERP information
    pub fn slerp_setup(p: Self, q: Self) -> SlerpInfo {
        let cos_t = p.x * q.x + p.y * q.y + p.z * q.z + p.w * q.w;

        let mut info = SlerpInfo {
            sin_t: 0.0,
            theta: 0.0,
            flip: false,
            linear: false,
        };

        let cos_t = if cos_t < 0.0 {
            info.flip = true;
            -cos_t
        } else {
            info.flip = false;
            cos_t
        };

        if 1.0 - cos_t < SLERP_EPSILON {
            info.linear = true;
            info.theta = 0.0;
            info.sin_t = 0.0;
        } else {
            info.linear = false;
            info.theta = WWMath::acos(cos_t);
            info.sin_t = WWMath::sin(info.theta);
        }

        info
    }

    /// Cached SLERP using pre-computed SlerpInfo
    pub fn cached_slerp(p: Self, q: Self, alpha: f32, info: &SlerpInfo) -> Self {
        let (beta, alpha) = if info.linear {
            (1.0 - alpha, alpha)
        } else {
            let oo_sin_t = 1.0 / info.sin_t;
            let beta = WWMath::sin(info.theta - alpha * info.theta) * oo_sin_t;
            let alpha = WWMath::sin(alpha * info.theta) * oo_sin_t;
            (beta, alpha)
        };

        let alpha = if info.flip { -alpha } else { alpha };

        Self {
            x: beta * p.x + alpha * q.x,
            y: beta * p.y + alpha * q.y,
            z: beta * p.z + alpha * q.z,
            w: beta * p.w + alpha * q.w,
        }
    }

    /// Linear interpolation between two quaternions (not normalized)
    pub fn lerp(a: Self, b: Self, t: f32) -> Self {
        let factor1 = 1.0 - t;
        let factor2 = t;

        Self {
            x: a.x * factor1 + b.x * factor2,
            y: a.y * factor1 + b.y * factor2,
            z: a.z * factor1 + b.z * factor2,
            w: a.w * factor1 + b.w * factor2,
        }
    }

    /// Check if two quaternions are equal within epsilon
    pub fn equal_within_epsilon(a: Self, b: Self, epsilon: f32) -> bool {
        (a.x - b.x).abs() < epsilon
            && (a.y - b.y).abs() < epsilon
            && (a.z - b.z).abs() < epsilon
            && (a.w - b.w).abs() < epsilon
    }
}

// Array access implementation
impl Index<usize> for Quaternion {
    type Output = f32;

    fn index(&self, index: usize) -> &Self::Output {
        match index {
            0 => &self.x,
            1 => &self.y,
            2 => &self.z,
            3 => &self.w,
            _ => panic!("Quaternion index out of bounds: {}", index),
        }
    }
}

impl IndexMut<usize> for Quaternion {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        match index {
            0 => &mut self.x,
            1 => &mut self.y,
            2 => &mut self.z,
            3 => &mut self.w,
            _ => panic!("Quaternion index out of bounds: {}", index),
        }
    }
}

// Arithmetic operations
impl Add for Quaternion {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Self {
            x: self.x + other.x,
            y: self.y + other.y,
            z: self.z + other.z,
            w: self.w + other.w,
        }
    }
}

impl AddAssign for Quaternion {
    fn add_assign(&mut self, other: Self) {
        self.x += other.x;
        self.y += other.y;
        self.z += other.z;
        self.w += other.w;
    }
}

impl Sub for Quaternion {
    type Output = Self;

    fn sub(self, other: Self) -> Self {
        Self {
            x: self.x - other.x,
            y: self.y - other.y,
            z: self.z - other.z,
            w: self.w - other.w,
        }
    }
}

impl SubAssign for Quaternion {
    fn sub_assign(&mut self, other: Self) {
        self.x -= other.x;
        self.y -= other.y;
        self.z -= other.z;
        self.w -= other.w;
    }
}

impl Mul<f32> for Quaternion {
    type Output = Self;

    fn mul(self, rhs: f32) -> Self {
        Self {
            x: self.x * rhs,
            y: self.y * rhs,
            z: self.z * rhs,
            w: self.w * rhs,
        }
    }
}

impl MulAssign<f32> for Quaternion {
    fn mul_assign(&mut self, rhs: f32) {
        self.x *= rhs;
        self.y *= rhs;
        self.z *= rhs;
        self.w *= rhs;
    }
}

impl Div<f32> for Quaternion {
    type Output = Self;

    fn div(self, rhs: f32) -> Self {
        let inv_rhs = 1.0 / rhs;
        self * inv_rhs
    }
}

impl DivAssign<f32> for Quaternion {
    fn div_assign(&mut self, rhs: f32) {
        let inv_rhs = 1.0 / rhs;
        *self *= inv_rhs;
    }
}

impl Neg for Quaternion {
    type Output = Self;

    fn neg(self) -> Self {
        Self {
            x: -self.x,
            y: -self.y,
            z: -self.z,
            w: -self.w,
        }
    }
}

impl Mul for Quaternion {
    type Output = Self;

    fn mul(self, other: Self) -> Self {
        Self {
            x: self.w * other.x + other.w * self.x + (self.y * other.z - other.y * self.z),
            y: self.w * other.y + other.w * self.y - (self.x * other.z - other.x * self.z),
            z: self.w * other.z + other.w * self.z + (self.x * other.y - other.x * self.y),
            w: self.w * other.w - (self.x * other.x + self.y * other.y + self.z * other.z),
        }
    }
}

// Scalar multiplication (reverse order)
impl Mul<Quaternion> for f32 {
    type Output = Quaternion;

    fn mul(self, rhs: Quaternion) -> Quaternion {
        rhs * self
    }
}

// Free functions
/// Get the inverse of a quaternion
pub fn inverse(q: Quaternion) -> Quaternion {
    q.inverse()
}

/// Get the conjugate of a quaternion
pub fn conjugate(q: Quaternion) -> Quaternion {
    q.conjugate()
}

/// Normalize a quaternion
pub fn normalize(q: Quaternion) -> Quaternion {
    let mut q = q;
    q.normalize();
    q
}

/// Create a quaternion from an axis and angle
pub fn axis_to_quat(axis: Vector3, angle: f32) -> Quaternion {
    Quaternion::from_axis_angle(axis, angle)
}

/// Spherical linear interpolation between two quaternions
pub fn slerp(p: Quaternion, q: Quaternion, t: f32) -> Quaternion {
    Quaternion::slerp(p, q, t)
}

/// Fast spherical linear interpolation (approximate)
pub fn fast_slerp(p: Quaternion, q: Quaternion, t: f32) -> Quaternion {
    Quaternion::fast_slerp(p, q, t)
}

/// Create a quaternion from a Matrix3
pub fn build_quaternion(matrix: &Matrix3) -> Quaternion {
    Quaternion::from_matrix3(matrix)
}

/// Create a quaternion from a Matrix3D
pub fn build_quaternion_from_matrix3d(matrix: &Matrix3D) -> Quaternion {
    Quaternion::from_matrix3d(matrix)
}

/// Create a Matrix3 from a quaternion
pub fn build_matrix3(q: &Quaternion) -> Matrix3 {
    q.to_matrix3()
}

/// Create a Matrix3D from a quaternion
pub fn build_matrix3d(q: &Quaternion) -> Matrix3D {
    q.to_matrix3d()
}

/// Create a Matrix4 from a quaternion
pub fn build_matrix4(q: &Quaternion) -> Matrix4 {
    q.to_matrix4()
}

/// Create quaternion from Matrix4
pub fn build_quaternion_from_matrix4(matrix: &Matrix4) -> Quaternion {
    Quaternion::from_matrix4(matrix)
}

/// Setup SLERP info for cached interpolation
pub fn slerp_setup(p: Quaternion, q: Quaternion) -> SlerpInfo {
    Quaternion::slerp_setup(p, q)
}

/// Cached SLERP using pre-computed info
pub fn cached_slerp(p: Quaternion, q: Quaternion, alpha: f32, info: &SlerpInfo) -> Quaternion {
    Quaternion::cached_slerp(p, q, alpha, info)
}

/// In-place cached SLERP
pub fn cached_slerp_inplace(
    result: &mut Quaternion,
    p: Quaternion,
    q: Quaternion,
    alpha: f32,
    info: &SlerpInfo,
) {
    *result = Quaternion::cached_slerp(p, q, alpha, info);
}

/// Check if two quaternions are equal within epsilon
pub fn equal_within_epsilon(a: Quaternion, b: Quaternion, epsilon: f32) -> bool {
    Quaternion::equal_within_epsilon(a, b, epsilon)
}

/// Trackball quaternion computation from 2D mouse coordinates
/// This creates an intuitive viewing control system by projecting mouse movement onto a sphere
pub fn trackball(x0: f32, y0: f32, x1: f32, y1: f32, sph_size: f32) -> Quaternion {
    // If no movement, return identity
    if (x0 == x1) && (y0 == y1) {
        return Quaternion::from_components(0.0, 0.0, 0.0, 1.0);
    }

    // Project coordinates to sphere
    let p1 = Vector3::new(x0, y0, project_to_sphere(sph_size, x0, y0));

    let p2 = Vector3::new(x1, y1, project_to_sphere(sph_size, x1, y1));

    // Find cross product (axis of rotation)
    let axis = p2.cross(p1);

    // Compute angle
    let d = p1 - p2;
    let mut t = d.length() / (2.0 * sph_size);

    // Avoid problems with out of control values
    t = t.clamp(-1.0, 1.0);
    let phi = 2.0 * WWMath::asin(t);

    axis_to_quat(axis, phi)
}

/// Helper function for trackball - projects a point to sphere surface
fn project_to_sphere(r: f32, x: f32, y: f32) -> f32 {
    let sqrt2 = SQRT2;
    let d = WWMath::sqrt(x * x + y * y);

    if d < r * (sqrt2 / 2.0) {
        // Inside sphere
        WWMath::sqrt(r * r - d * d)
    } else {
        // On hyperbola
        let t = r / sqrt2;
        t * t / d
    }
}

/// SLERP implementation matching the C++ version exactly
fn slerp_impl(result: &mut Quaternion, p: Quaternion, q: Quaternion, alpha: f32) {
    let mut cos_t = p.x * q.x + p.y * q.y + p.z * q.z + p.w * q.w;
    let qflip = cos_t < 0.0;

    if qflip {
        cos_t = -cos_t;
    }

    let (beta, alpha) = if 1.0 - cos_t < EPSILON * EPSILON {
        // Very close, use linear interpolation
        (1.0 - alpha, alpha)
    } else {
        // Normal SLERP
        let theta = WWMath::acos(cos_t);
        let sin_t = WWMath::sin(theta);
        let oo_sin_t = 1.0 / sin_t;
        let beta = WWMath::sin(theta - alpha * theta) * oo_sin_t;
        let alpha = WWMath::sin(alpha * theta) * oo_sin_t;
        (beta, alpha)
    };

    let alpha = if qflip { -alpha } else { alpha };

    result.x = beta * p.x + alpha * q.x;
    result.y = beta * p.y + alpha * q.y;
    result.z = beta * p.z + alpha * q.z;
    result.w = beta * p.w + alpha * q.w;
}

/// Fast SLERP implementation matching the C++ version exactly
fn fast_slerp_impl(result: &mut Quaternion, p: Quaternion, q: Quaternion, alpha: f32) {
    let mut cos_t = p.x * q.x + p.y * q.y + p.z * q.z + p.w * q.w;
    let qflip = cos_t < 0.0;

    if qflip {
        cos_t = -cos_t;
    }

    let (beta, alpha) = if 1.0 - cos_t < EPSILON * EPSILON {
        // Very close, use linear interpolation
        (1.0 - alpha, alpha)
    } else {
        // Fast SLERP using regular trigonometric functions (faster versions not available)
        let theta = WWMath::acos(cos_t);
        let sin_t = WWMath::sin(theta);
        let oo_sin_t = 1.0 / sin_t;
        let beta = WWMath::sin(theta - alpha * theta) * oo_sin_t;
        let alpha = WWMath::sin(alpha * theta) * oo_sin_t;
        (beta, alpha)
    };

    let alpha = if qflip { -alpha } else { alpha };

    result.x = beta * p.x + alpha * q.x;
    result.y = beta * p.y + alpha * q.y;
    result.z = beta * p.z + alpha * q.z;
    result.w = beta * p.w + alpha * q.w;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_identity() {
        let q = Quaternion::IDENTITY;
        assert_eq!(q.x, 0.0);
        assert_eq!(q.y, 0.0);
        assert_eq!(q.z, 0.0);
        assert_eq!(q.w, 1.0);
    }

    #[test]
    fn test_from_components() {
        let q = Quaternion::from_components(1.0, 2.0, 3.0, 4.0);
        assert_eq!(q.x, 1.0);
        assert_eq!(q.y, 2.0);
        assert_eq!(q.z, 3.0);
        assert_eq!(q.w, 4.0);
    }

    #[test]
    fn test_from_axis_angle() {
        let axis = Vector3::new(0.0, 0.0, 1.0);
        let angle = std::f32::consts::PI / 2.0; // 90 degrees
        let q = Quaternion::from_axis_angle(axis, angle);

        let expected_sin = (angle / 2.0).sin();
        let expected_cos = (angle / 2.0).cos();

        assert!((q.x - 0.0).abs() < 1e-6);
        assert!((q.y - 0.0).abs() < 1e-6);
        assert!((q.z - expected_sin).abs() < 1e-6);
        assert!((q.w - expected_cos).abs() < 1e-6);
    }

    #[test]
    fn test_length() {
        let q = Quaternion::from_components(1.0, 2.0, 3.0, 4.0);
        let expected_length = (1.0f32 + 4.0f32 + 9.0f32 + 16.0f32).sqrt();
        assert!((q.length() - expected_length).abs() < 1e-6);
    }

    #[test]
    fn test_normalize() {
        let mut q = Quaternion::from_components(1.0, 2.0, 3.0, 4.0);
        q.normalize();

        assert!((q.length() - 1.0).abs() < 1e-6);
        assert!(q.is_normalized());
    }

    #[test]
    fn test_conjugate() {
        let q = Quaternion::from_components(1.0, 2.0, 3.0, 4.0);
        let conj = q.conjugate();

        assert_eq!(conj.x, -1.0);
        assert_eq!(conj.y, -2.0);
        assert_eq!(conj.z, -3.0);
        assert_eq!(conj.w, 4.0);
    }

    #[test]
    fn test_inverse() {
        let q = Quaternion::from_components(1.0, 2.0, 3.0, 4.0);
        let inv = q.inverse();
        let product = q * inv;

        // Should be close to identity
        assert!((product.x).abs() < 0.01);
        assert!((product.y).abs() < 0.01);
        assert!((product.z).abs() < 0.01);
        assert!((product.w - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_multiplication() {
        let q1 = Quaternion::from_components(1.0, 0.0, 0.0, 0.0);
        let q2 = Quaternion::from_components(0.0, 1.0, 0.0, 0.0);
        let result = q1 * q2;

        // i * j = k
        assert!((result.x).abs() < 1e-6);
        assert!((result.y).abs() < 1e-6);
        assert!((result.z - 1.0).abs() < 1e-6);
        assert!((result.w).abs() < 1e-6);
    }

    #[test]
    fn test_rotation_x() {
        let mut q = Quaternion::IDENTITY;
        q.rotate_x(std::f32::consts::PI / 2.0);

        let v = Vector3::new(0.0, 1.0, 0.0);
        let rotated = q.rotate_vector(v);

        // Rotating (0,1,0) 90 degrees around X should give (0,0,1)
        assert!((rotated.x).abs() < 1e-6);
        assert!((rotated.y).abs() < 1e-6);
        assert!((rotated.z - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_rotation_y() {
        let mut q = Quaternion::IDENTITY;
        q.rotate_y(std::f32::consts::PI / 2.0);

        let v = Vector3::new(1.0, 0.0, 0.0);
        let rotated = q.rotate_vector(v);

        // Rotating (1,0,0) 90 degrees around Y should give (0,0,-1)
        assert!((rotated.x).abs() < 1e-6);
        assert!((rotated.y).abs() < 1e-6);
        assert!((rotated.z - (-1.0)).abs() < 1e-6);
    }

    #[test]
    fn test_rotation_z() {
        let mut q = Quaternion::IDENTITY;
        q.rotate_z(std::f32::consts::PI / 2.0);

        let v = Vector3::new(1.0, 0.0, 0.0);
        let rotated = q.rotate_vector(v);

        // Rotating (1,0,0) 90 degrees around Z should give (0,1,0)
        assert!((rotated.x).abs() < 1e-6);
        assert!((rotated.y - 1.0).abs() < 1e-6);
        assert!((rotated.z).abs() < 1e-6);
    }

    #[test]
    fn test_to_matrix3() {
        let axis = Vector3::new(0.0, 0.0, 1.0);
        let angle = std::f32::consts::PI / 2.0;
        let q = Quaternion::from_axis_angle(axis, angle);

        let matrix = q.to_matrix3();

        // The matrix should represent a 90-degree rotation around Z
        assert!((matrix.row[0].x - 0.0).abs() < 1e-6); // cos(90) = 0
        assert!((matrix.row[0].y - (-1.0)).abs() < 1e-6); // -sin(90) = -1
        assert!((matrix.row[1].x - 1.0).abs() < 1e-6); // sin(90) = 1
        assert!((matrix.row[1].y - 0.0).abs() < 1e-6); // cos(90) = 0
    }

    #[test]
    fn test_slerp() {
        let q1 = Quaternion::from_components(0.0, 0.0, 0.0, 1.0);
        let q2 = Quaternion::from_components(0.0, 0.0, 1.0, 0.0);

        let result = Quaternion::slerp(q1, q2, 0.5);

        // Should be normalized
        assert!(result.is_normalized());
    }

    #[test]
    fn test_array_access() {
        let mut q = Quaternion::from_components(1.0, 2.0, 3.0, 4.0);

        assert_eq!(q[0], 1.0);
        assert_eq!(q[1], 2.0);
        assert_eq!(q[2], 3.0);
        assert_eq!(q[3], 4.0);

        q[1] = 5.0;
        assert_eq!(q[1], 5.0);
    }

    #[test]
    fn test_make_closest() {
        let mut q1 = Quaternion::from_components(1.0, 0.0, 0.0, 0.0);
        let q2 = Quaternion::from_components(-1.0, 0.0, 0.0, 0.0);

        q1.make_closest(&q2);

        // Should be flipped to be closer to q2
        assert_eq!(q1.x, -1.0);
        assert_eq!(q1.y, 0.0);
        assert_eq!(q1.z, 0.0);
        assert_eq!(q1.w, 0.0);
    }
}
