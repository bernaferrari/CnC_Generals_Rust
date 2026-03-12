//! A 4x4 matrix.
//!
//! This module provides 4x4 matrix functionality,
//! converted from the original C++ Matrix4x4 class.

use crate::matrix3::Matrix3;
use crate::matrix3d::Matrix3D;
use crate::vector3::Vector3;
use crate::vector4::Vector4;
use crate::WWMath;
use std::ops::{
    Add, AddAssign, Div, DivAssign, Index, IndexMut, Mul, MulAssign, Neg, Sub, SubAssign,
};

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct Matrix4 {
    pub row: [Vector4; 4],
}

impl Matrix4 {
    pub const ZERO: Matrix4 = Matrix4 {
        row: [Vector4::ZERO, Vector4::ZERO, Vector4::ZERO, Vector4::ZERO],
    };

    pub const IDENTITY: Matrix4 = Matrix4 {
        row: [
            Vector4::new(1.0, 0.0, 0.0, 0.0),
            Vector4::new(0.0, 1.0, 0.0, 0.0),
            Vector4::new(0.0, 0.0, 1.0, 0.0),
            Vector4::new(0.0, 0.0, 0.0, 1.0),
        ],
    };

    /// Create a new Matrix4 from four row vectors
    pub fn new(r0: Vector4, r1: Vector4, r2: Vector4, r3: Vector4) -> Self {
        Self {
            row: [r0, r1, r2, r3],
        }
    }

    /// Create a Matrix4 from individual values
    #[allow(clippy::too_many_arguments)]
    pub fn from_values(
        m11: f32,
        m12: f32,
        m13: f32,
        m14: f32,
        m21: f32,
        m22: f32,
        m23: f32,
        m24: f32,
        m31: f32,
        m32: f32,
        m33: f32,
        m34: f32,
        m41: f32,
        m42: f32,
        m43: f32,
        m44: f32,
    ) -> Self {
        Self {
            row: [
                Vector4::new(m11, m12, m13, m14),
                Vector4::new(m21, m22, m23, m24),
                Vector4::new(m31, m32, m33, m34),
                Vector4::new(m41, m42, m43, m44),
            ],
        }
    }

    /// Create from identity flag
    pub fn with_identity(identity: bool) -> Self {
        if identity {
            Self::IDENTITY
        } else {
            Self::ZERO
        }
    }

    /// Create from Matrix3D
    pub fn from_matrix3d(m: Matrix3D) -> Self {
        Self {
            row: [
                m.row[0],
                m.row[1],
                m.row[2],
                Vector4::new(0.0, 0.0, 0.0, 1.0),
            ],
        }
    }

    /// Create from Matrix3
    pub fn from_matrix3(m: Matrix3) -> Self {
        Self {
            row: [
                Vector4::new(m.row[0].x, m.row[0].y, m.row[0].z, 0.0),
                Vector4::new(m.row[1].x, m.row[1].y, m.row[1].z, 0.0),
                Vector4::new(m.row[2].x, m.row[2].y, m.row[2].z, 0.0),
                Vector4::new(0.0, 0.0, 0.0, 1.0),
            ],
        }
    }

    /// Create a scale matrix
    pub fn create_scale(scale: Vector3) -> Self {
        Self {
            row: [
                Vector4::new(scale.x, 0.0, 0.0, 0.0),
                Vector4::new(0.0, scale.y, 0.0, 0.0),
                Vector4::new(0.0, 0.0, scale.z, 0.0),
                Vector4::new(0.0, 0.0, 0.0, 1.0),
            ],
        }
    }

    /// Create a translation matrix
    pub fn create_translation(translation: Vector3) -> Self {
        Self {
            row: [
                Vector4::new(1.0, 0.0, 0.0, translation.x),
                Vector4::new(0.0, 1.0, 0.0, translation.y),
                Vector4::new(0.0, 0.0, 1.0, translation.z),
                Vector4::new(0.0, 0.0, 0.0, 1.0),
            ],
        }
    }

    /// Set matrix to identity
    pub fn make_identity(&mut self) {
        *self = Self::IDENTITY;
    }

    /// Initialize from Matrix3D
    pub fn init_from_matrix3d(&mut self, m: Matrix3D) {
        self.row[0] = m.row[0];
        self.row[1] = m.row[1];
        self.row[2] = m.row[2];
        self.row[3] = Vector4::new(0.0, 0.0, 0.0, 1.0);
    }

    /// Initialize from Matrix3
    pub fn init_from_matrix3(&mut self, m: Matrix3) {
        self.row[0] = Vector4::new(m.row[0].x, m.row[0].y, m.row[0].z, 0.0);
        self.row[1] = Vector4::new(m.row[1].x, m.row[1].y, m.row[1].z, 0.0);
        self.row[2] = Vector4::new(m.row[2].x, m.row[2].y, m.row[2].z, 0.0);
        self.row[3] = Vector4::new(0.0, 0.0, 0.0, 1.0);
    }

    /// Initialize from four row vectors
    pub fn init_from_rows(&mut self, r0: Vector4, r1: Vector4, r2: Vector4, r3: Vector4) {
        self.row[0] = r0;
        self.row[1] = r1;
        self.row[2] = r2;
        self.row[3] = r3;
    }

    /// Initialize from individual values
    #[allow(clippy::too_many_arguments)]
    pub fn init_from_values(
        &mut self,
        m11: f32,
        m12: f32,
        m13: f32,
        m14: f32,
        m21: f32,
        m22: f32,
        m23: f32,
        m24: f32,
        m31: f32,
        m32: f32,
        m33: f32,
        m34: f32,
        m41: f32,
        m42: f32,
        m43: f32,
        m44: f32,
    ) {
        self.row[0] = Vector4::new(m11, m12, m13, m14);
        self.row[1] = Vector4::new(m21, m22, m23, m24);
        self.row[2] = Vector4::new(m31, m32, m33, m34);
        self.row[3] = Vector4::new(m41, m42, m43, m44);
    }

    /// Initialize orthographic projection matrix
    pub fn init_ortho(
        &mut self,
        left: f32,
        right: f32,
        bottom: f32,
        top: f32,
        znear: f32,
        zfar: f32,
    ) {
        assert!(znear >= 0.0);
        assert!(zfar > znear);

        self.make_identity();
        self.row[0][0] = 2.0 / (right - left);
        self.row[0][3] = -(right + left) / (right - left);
        self.row[1][1] = 2.0 / (top - bottom);
        self.row[1][3] = -(top + bottom) / (top - bottom);
        self.row[2][2] = -2.0 / (zfar - znear);
        self.row[2][3] = -(zfar + znear) / (zfar - znear);
    }

    /// Initialize perspective projection matrix from field of view
    pub fn init_perspective(&mut self, hfov: f32, vfov: f32, znear: f32, zfar: f32) {
        assert!(znear > 0.0);
        assert!(zfar > znear);

        self.make_identity();
        self.row[0][0] = 1.0 / (hfov * 0.5).tan();
        self.row[1][1] = 1.0 / (vfov * 0.5).tan();
        self.row[2][2] = -(zfar + znear) / (zfar - znear);
        self.row[2][3] = -(2.0 * zfar * znear) / (zfar - znear);
        self.row[3][2] = -1.0;
        self.row[3][3] = 0.0;
    }

    /// Initialize perspective projection matrix from frustum
    pub fn init_perspective_frustum(
        &mut self,
        left: f32,
        right: f32,
        bottom: f32,
        top: f32,
        znear: f32,
        zfar: f32,
    ) {
        assert!(znear > 0.0);
        assert!(zfar > 0.0);

        self.make_identity();
        self.row[0][0] = 2.0 * znear / (right - left);
        self.row[0][2] = (right + left) / (right - left);
        self.row[1][1] = 2.0 * znear / (top - bottom);
        self.row[1][2] = (top + bottom) / (top - bottom);
        self.row[2][2] = -(zfar + znear) / (zfar - znear);
        self.row[2][3] = -(2.0 * zfar * znear) / (zfar - znear);
        self.row[3][2] = -1.0;
        self.row[3][3] = 0.0;
    }

    /// Get transpose of the matrix
    pub fn transpose(&self) -> Self {
        Self {
            row: [
                Vector4::new(
                    self.row[0][0],
                    self.row[1][0],
                    self.row[2][0],
                    self.row[3][0],
                ),
                Vector4::new(
                    self.row[0][1],
                    self.row[1][1],
                    self.row[2][1],
                    self.row[3][1],
                ),
                Vector4::new(
                    self.row[0][2],
                    self.row[1][2],
                    self.row[2][2],
                    self.row[3][2],
                ),
                Vector4::new(
                    self.row[0][3],
                    self.row[1][3],
                    self.row[2][3],
                    self.row[3][3],
                ),
            ],
        }
    }

    /// Get inverse of the matrix using Gauss-Jordan elimination
    pub fn inverse(&self) -> Self {
        let mut a = *self; // As a evolves from original matrix into identity
        let mut b = Self::IDENTITY; // b evolves from identity into inverse(a)

        // Loop over columns of a from left to right, eliminating above and below diagonal
        for j in 0..4 {
            // Find largest pivot in column j among rows j..4
            let mut i1 = j;
            for i in (j + 1)..4 {
                if WWMath::fabs(a.row[i][j]) > WWMath::fabs(a.row[i1][j]) {
                    i1 = i;
                }
            }

            // Swap rows i1 and j in a and b to put pivot on diagonal
            a.row.swap(i1, j);
            b.row.swap(i1, j);

            // Scale row j to have a unit diagonal
            if a.row[j][j] != 0.0 {
                let scale = 1.0 / a.row[j][j];
                b.row[j] *= scale;
                a.row[j] *= scale;
            }

            // Eliminate off-diagonal elements in column j of a, doing identical ops to b
            for i in 0..4 {
                if i != j {
                    let factor = a.row[i][j];
                    b.row[i] -= factor * b.row[j];
                    a.row[i] -= factor * a.row[j];
                }
            }
        }
        b
    }

    /// Transform a Vector3 by this matrix (assumes w=1.0)
    pub fn transform_vector3(&self, v: Vector3) -> Vector4 {
        Vector4::new(
            self.row[0][0] * v.x
                + self.row[0][1] * v.y
                + self.row[0][2] * v.z
                + self.row[0][3] * 1.0,
            self.row[1][0] * v.x
                + self.row[1][1] * v.y
                + self.row[1][2] * v.z
                + self.row[1][3] * 1.0,
            self.row[2][0] * v.x
                + self.row[2][1] * v.y
                + self.row[2][2] * v.z
                + self.row[2][3] * 1.0,
            self.row[3][0] * v.x
                + self.row[3][1] * v.y
                + self.row[3][2] * v.z
                + self.row[3][3] * 1.0,
        )
    }

    /// Transform a Vector4 by this matrix
    pub fn transform_vector4(&self, v: Vector4) -> Vector4 {
        Vector4::new(
            self.row[0][0] * v.x
                + self.row[0][1] * v.y
                + self.row[0][2] * v.z
                + self.row[0][3] * v.w,
            self.row[1][0] * v.x
                + self.row[1][1] * v.y
                + self.row[1][2] * v.z
                + self.row[1][3] * v.w,
            self.row[2][0] * v.x
                + self.row[2][1] * v.y
                + self.row[2][2] * v.z
                + self.row[2][3] * v.w,
            self.row[3][0] * v.x
                + self.row[3][1] * v.y
                + self.row[3][2] * v.z
                + self.row[3][3] * v.w,
        )
    }

    /// Static transformation function for Vector3
    pub fn transform_vector3_static(tm: Self, input: Vector3) -> Vector3 {
        Vector3::new(
            tm.row[0][0] * input.x + tm.row[0][1] * input.y + tm.row[0][2] * input.z + tm.row[0][3],
            tm.row[1][0] * input.x + tm.row[1][1] * input.y + tm.row[1][2] * input.z + tm.row[1][3],
            tm.row[2][0] * input.x + tm.row[2][1] * input.y + tm.row[2][2] * input.z + tm.row[2][3],
        )
    }

    /// Static transformation function for Vector3 to Vector4
    pub fn transform_vector3_to_vector4_static(tm: Self, input: Vector3) -> Vector4 {
        Vector4::new(
            tm.row[0][0] * input.x + tm.row[0][1] * input.y + tm.row[0][2] * input.z + tm.row[0][3],
            tm.row[1][0] * input.x + tm.row[1][1] * input.y + tm.row[1][2] * input.z + tm.row[1][3],
            tm.row[2][0] * input.x + tm.row[2][1] * input.y + tm.row[2][2] * input.z + tm.row[2][3],
            1.0,
        )
    }

    /// Static transformation function for Vector4
    pub fn transform_vector4_static(tm: Self, input: Vector4) -> Vector4 {
        Vector4::new(
            tm.row[0][0] * input.x
                + tm.row[0][1] * input.y
                + tm.row[0][2] * input.z
                + tm.row[0][3] * input.w,
            tm.row[1][0] * input.x
                + tm.row[1][1] * input.y
                + tm.row[1][2] * input.z
                + tm.row[1][3] * input.w,
            tm.row[2][0] * input.x
                + tm.row[2][1] * input.y
                + tm.row[2][2] * input.z
                + tm.row[2][3] * input.w,
            tm.row[3][0] * input.x
                + tm.row[3][1] * input.y
                + tm.row[3][2] * input.z
                + tm.row[3][3] * input.w,
        )
    }

    /// Multiply two matrices without temporaries
    pub fn multiply(a: Self, b: Self) -> Self {
        let mut result = Self::ZERO;

        for i in 0..4 {
            for j in 0..4 {
                let mut sum = 0.0;
                for k in 0..4 {
                    sum += a.row[i][k] * b.row[k][j];
                }
                result.row[i][j] = sum;
            }
        }

        result
    }

    /// Multiply Matrix3D by Matrix4
    pub fn multiply_matrix3d_matrix4(a: Matrix3D, b: Self) -> Self {
        let mut result = Self::ZERO;

        // First 3 rows
        for i in 0..3 {
            for j in 0..4 {
                let mut sum = 0.0;
                for k in 0..3 {
                    sum += a.row[i][k] * b.row[k][j];
                }
                sum += a.row[i][3] * b.row[3][j]; // Translation component
                result.row[i][j] = sum;
            }
        }

        // Last row is unchanged from b (assuming last row of a is [0,0,0,1])
        result.row[3] = b.row[3];

        result
    }

    /// Multiply Matrix4 by Matrix3D
    pub fn multiply_matrix4_matrix3d(a: Self, b: Matrix3D) -> Self {
        let mut result = Self::ZERO;

        for i in 0..4 {
            // First 3 columns
            for j in 0..3 {
                let mut sum = 0.0;
                for k in 0..3 {
                    sum += a.row[i][k] * b.row[k][j];
                }
                result.row[i][j] = sum;
            }

            // Last column (translation)
            let mut sum = 0.0;
            for k in 0..3 {
                sum += a.row[i][k] * b.row[k][3];
            }
            sum += a.row[i][3]; // Add translation component from a
            result.row[i][3] = sum;
        }

        result
    }
}

// Array access implementation
impl Index<usize> for Matrix4 {
    type Output = Vector4;

    fn index(&self, index: usize) -> &Self::Output {
        &self.row[index]
    }
}

impl IndexMut<usize> for Matrix4 {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.row[index]
    }
}

// Arithmetic operations
impl Add for Matrix4 {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Self {
            row: [
                self.row[0] + other.row[0],
                self.row[1] + other.row[1],
                self.row[2] + other.row[2],
                self.row[3] + other.row[3],
            ],
        }
    }
}

impl AddAssign for Matrix4 {
    fn add_assign(&mut self, other: Self) {
        self.row[0] += other.row[0];
        self.row[1] += other.row[1];
        self.row[2] += other.row[2];
        self.row[3] += other.row[3];
    }
}

impl Sub for Matrix4 {
    type Output = Self;

    fn sub(self, other: Self) -> Self {
        Self {
            row: [
                self.row[0] - other.row[0],
                self.row[1] - other.row[1],
                self.row[2] - other.row[2],
                self.row[3] - other.row[3],
            ],
        }
    }
}

impl SubAssign for Matrix4 {
    fn sub_assign(&mut self, other: Self) {
        self.row[0] -= other.row[0];
        self.row[1] -= other.row[1];
        self.row[2] -= other.row[2];
        self.row[3] -= other.row[3];
    }
}

impl Mul<f32> for Matrix4 {
    type Output = Self;

    fn mul(self, rhs: f32) -> Self {
        Self {
            row: [
                self.row[0] * rhs,
                self.row[1] * rhs,
                self.row[2] * rhs,
                self.row[3] * rhs,
            ],
        }
    }
}

impl MulAssign<f32> for Matrix4 {
    fn mul_assign(&mut self, rhs: f32) {
        self.row[0] *= rhs;
        self.row[1] *= rhs;
        self.row[2] *= rhs;
        self.row[3] *= rhs;
    }
}

impl Div<f32> for Matrix4 {
    type Output = Self;

    fn div(self, rhs: f32) -> Self {
        let inv_rhs = 1.0 / rhs;
        self * inv_rhs
    }
}

impl DivAssign<f32> for Matrix4 {
    fn div_assign(&mut self, rhs: f32) {
        let inv_rhs = 1.0 / rhs;
        *self *= inv_rhs;
    }
}

impl Neg for Matrix4 {
    type Output = Self;

    fn neg(self) -> Self {
        Self {
            row: [-self.row[0], -self.row[1], -self.row[2], -self.row[3]],
        }
    }
}

impl Mul for Matrix4 {
    type Output = Self;

    fn mul(self, other: Self) -> Self {
        let mut result = Self::ZERO;

        for i in 0..4 {
            for j in 0..4 {
                let mut sum = 0.0;
                for k in 0..4 {
                    sum += self.row[i][k] * other.row[k][j];
                }
                result.row[i][j] = sum;
            }
        }

        result
    }
}

impl Mul<Vector4> for Matrix4 {
    type Output = Vector4;

    fn mul(self, v: Vector4) -> Vector4 {
        Vector4::new(
            self.row[0][0] * v.x
                + self.row[0][1] * v.y
                + self.row[0][2] * v.z
                + self.row[0][3] * v.w,
            self.row[1][0] * v.x
                + self.row[1][1] * v.y
                + self.row[1][2] * v.z
                + self.row[1][3] * v.w,
            self.row[2][0] * v.x
                + self.row[2][1] * v.y
                + self.row[2][2] * v.z
                + self.row[2][3] * v.w,
            self.row[3][0] * v.x
                + self.row[3][1] * v.y
                + self.row[3][2] * v.z
                + self.row[3][3] * v.w,
        )
    }
}

impl Mul<Vector3> for Matrix4 {
    type Output = Vector4;

    fn mul(self, v: Vector3) -> Vector4 {
        self.transform_vector3(v)
    }
}

impl Mul<Matrix3D> for Matrix4 {
    type Output = Self;

    fn mul(self, rhs: Matrix3D) -> Self {
        Self::multiply_matrix4_matrix3d(self, rhs)
    }
}

// Scalar multiplication (reverse order)
impl Mul<Matrix4> for f32 {
    type Output = Matrix4;

    fn mul(self, rhs: Matrix4) -> Matrix4 {
        rhs * self
    }
}

// Cross-type multiplications
impl Mul<Matrix4> for Matrix3D {
    type Output = Matrix4;

    fn mul(self, rhs: Matrix4) -> Matrix4 {
        Matrix4::multiply_matrix3d_matrix4(self, rhs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Vector3;

    #[test]
    fn test_identity() {
        let m = Matrix4::IDENTITY;
        let v = Vector4::new(1.0, 2.0, 3.0, 1.0);
        assert_eq!(m * v, v);
    }

    #[test]
    fn test_transpose() {
        let m = Matrix4::from_values(
            1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 11.0, 12.0, 13.0, 14.0, 15.0, 16.0,
        );
        let t = m.transpose();
        assert_eq!(t.row[0], Vector4::new(1.0, 5.0, 9.0, 13.0));
        assert_eq!(t.row[1], Vector4::new(2.0, 6.0, 10.0, 14.0));
        assert_eq!(t.row[2], Vector4::new(3.0, 7.0, 11.0, 15.0));
        assert_eq!(t.row[3], Vector4::new(4.0, 8.0, 12.0, 16.0));
    }

    #[test]
    fn test_matrix_multiplication() {
        let m1 = Matrix4::from_values(
            1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 11.0, 12.0, 13.0, 14.0, 15.0, 16.0,
        );
        let m2 = Matrix4::IDENTITY;

        assert_eq!(m1 * m2, m1);
    }

    #[test]
    fn test_vector_transformation() {
        let m = Matrix4::IDENTITY;
        let v3 = Vector3::new(1.0, 2.0, 3.0);
        let v4 = Vector4::new(1.0, 2.0, 3.0, 1.0);

        let result3 = m * v3;
        let result4 = m * v4;

        assert_eq!(result3, Vector4::new(1.0, 2.0, 3.0, 1.0));
        assert_eq!(result4, v4);
    }

    #[test]
    fn test_add_subtract() {
        let m1 = Matrix4::from_values(
            1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 11.0, 12.0, 13.0, 14.0, 15.0, 16.0,
        );
        let m2 = Matrix4::from_values(
            0.5, 1.0, 1.5, 2.0, 2.5, 3.0, 3.5, 4.0, 4.5, 5.0, 5.5, 6.0, 6.5, 7.0, 7.5, 8.0,
        );

        let sum = m1 + m2;
        let diff = m1 - m2;

        assert_eq!(sum.row[0], Vector4::new(1.5, 3.0, 4.5, 6.0));
        assert_eq!(diff.row[0], Vector4::new(0.5, 1.0, 1.5, 2.0));
    }

    #[test]
    fn test_scalar_operations() {
        let m = Matrix4::from_values(
            1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 11.0, 12.0, 13.0, 14.0, 15.0, 16.0,
        );

        let scaled = m * 2.0;
        assert_eq!(scaled.row[0], Vector4::new(2.0, 4.0, 6.0, 8.0));

        let divided = m / 2.0;
        assert_eq!(divided.row[0], Vector4::new(0.5, 1.0, 1.5, 2.0));
    }

    #[test]
    fn test_negation() {
        let m = Matrix4::from_values(
            1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 11.0, 12.0, 13.0, 14.0, 15.0, 16.0,
        );

        let neg = -m;
        assert_eq!(neg.row[0], Vector4::new(-1.0, -2.0, -3.0, -4.0));
    }

    #[test]
    fn test_indexing() {
        let m = Matrix4::from_values(
            1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 11.0, 12.0, 13.0, 14.0, 15.0, 16.0,
        );

        assert_eq!(m[0], Vector4::new(1.0, 2.0, 3.0, 4.0));
        assert_eq!(m[1], Vector4::new(5.0, 6.0, 7.0, 8.0));
        assert_eq!(m[2], Vector4::new(9.0, 10.0, 11.0, 12.0));
        assert_eq!(m[3], Vector4::new(13.0, 14.0, 15.0, 16.0));
    }

    #[test]
    fn test_projection_matrices() {
        let mut ortho = Matrix4::ZERO;
        ortho.init_ortho(-1.0, 1.0, -1.0, 1.0, 0.1, 100.0);

        let mut persp = Matrix4::ZERO;
        persp.init_perspective(
            std::f32::consts::PI / 4.0,
            std::f32::consts::PI / 4.0,
            0.1,
            100.0,
        );

        // Basic sanity checks - these matrices should not be zero or identity
        assert_ne!(ortho, Matrix4::ZERO);
        assert_ne!(ortho, Matrix4::IDENTITY);
        assert_ne!(persp, Matrix4::ZERO);
        assert_ne!(persp, Matrix4::IDENTITY);
    }

    #[test]
    fn test_from_matrix3d() {
        let matrix3d = Matrix3D::IDENTITY;
        let matrix4 = Matrix4::from_matrix3d(matrix3d);

        assert_eq!(matrix4.row[0], Vector4::new(1.0, 0.0, 0.0, 0.0));
        assert_eq!(matrix4.row[1], Vector4::new(0.0, 1.0, 0.0, 0.0));
        assert_eq!(matrix4.row[2], Vector4::new(0.0, 0.0, 1.0, 0.0));
        assert_eq!(matrix4.row[3], Vector4::new(0.0, 0.0, 0.0, 1.0));
    }

    #[test]
    fn test_from_matrix3() {
        let matrix3 = Matrix3::IDENTITY;
        let matrix4 = Matrix4::from_matrix3(matrix3);

        assert_eq!(matrix4.row[0], Vector4::new(1.0, 0.0, 0.0, 0.0));
        assert_eq!(matrix4.row[1], Vector4::new(0.0, 1.0, 0.0, 0.0));
        assert_eq!(matrix4.row[2], Vector4::new(0.0, 0.0, 1.0, 0.0));
        assert_eq!(matrix4.row[3], Vector4::new(0.0, 0.0, 0.0, 1.0));
    }

    #[test]
    fn test_inverse() {
        let m = Matrix4::from_values(
            2.0, 0.0, 0.0, 0.0, 0.0, 2.0, 0.0, 0.0, 0.0, 0.0, 2.0, 0.0, 0.0, 0.0, 0.0, 1.0,
        );
        let inv = m.inverse();
        let expected = Matrix4::from_values(
            0.5, 0.0, 0.0, 0.0, 0.0, 0.5, 0.0, 0.0, 0.0, 0.0, 0.5, 0.0, 0.0, 0.0, 0.0, 1.0,
        );

        // Check if inverse is approximately equal to expected
        for i in 0..4 {
            for j in 0..4 {
                assert!((inv.row[i][j] - expected.row[i][j]).abs() < 1e-6);
            }
        }
    }
}
