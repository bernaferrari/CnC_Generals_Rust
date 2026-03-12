//! A 3x3 matrix.
//!
//! This module provides 3x3 matrix functionality,
//! converted from the original C++ Matrix3x3 class.

use crate::{Vector3, WWMath, EPSILON};
use std::ops::{
    Add, AddAssign, Div, DivAssign, Index, IndexMut, Mul, MulAssign, Neg, Sub, SubAssign,
};

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct Matrix3 {
    pub row: [Vector3; 3],
}

impl Default for Matrix3 {
    fn default() -> Self {
        Self::new()
    }
}

impl Matrix3 {
    /// Identity matrix
    pub const IDENTITY: Matrix3 = Matrix3 {
        row: [
            Vector3 {
                x: 1.0,
                y: 0.0,
                z: 0.0,
            },
            Vector3 {
                x: 0.0,
                y: 1.0,
                z: 0.0,
            },
            Vector3 {
                x: 0.0,
                y: 0.0,
                z: 1.0,
            },
        ],
    };

    /// 90 degree X-axis rotation matrix
    pub const ROTATE_X90: Matrix3 = Matrix3 {
        row: [
            Vector3 {
                x: 1.0,
                y: 0.0,
                z: 0.0,
            },
            Vector3 {
                x: 0.0,
                y: 0.0,
                z: -1.0,
            },
            Vector3 {
                x: 0.0,
                y: 1.0,
                z: 0.0,
            },
        ],
    };

    /// 180 degree X-axis rotation matrix
    pub const ROTATE_X180: Matrix3 = Matrix3 {
        row: [
            Vector3 {
                x: 1.0,
                y: 0.0,
                z: 0.0,
            },
            Vector3 {
                x: 0.0,
                y: -1.0,
                z: 0.0,
            },
            Vector3 {
                x: 0.0,
                y: 0.0,
                z: -1.0,
            },
        ],
    };

    /// 270 degree X-axis rotation matrix
    pub const ROTATE_X270: Matrix3 = Matrix3 {
        row: [
            Vector3 {
                x: 1.0,
                y: 0.0,
                z: 0.0,
            },
            Vector3 {
                x: 0.0,
                y: 0.0,
                z: 1.0,
            },
            Vector3 {
                x: 0.0,
                y: -1.0,
                z: 0.0,
            },
        ],
    };

    /// 90 degree Y-axis rotation matrix
    pub const ROTATE_Y90: Matrix3 = Matrix3 {
        row: [
            Vector3 {
                x: 0.0,
                y: 0.0,
                z: 1.0,
            },
            Vector3 {
                x: 0.0,
                y: 1.0,
                z: 0.0,
            },
            Vector3 {
                x: -1.0,
                y: 0.0,
                z: 0.0,
            },
        ],
    };

    /// 180 degree Y-axis rotation matrix
    pub const ROTATE_Y180: Matrix3 = Matrix3 {
        row: [
            Vector3 {
                x: -1.0,
                y: 0.0,
                z: 0.0,
            },
            Vector3 {
                x: 0.0,
                y: 1.0,
                z: 0.0,
            },
            Vector3 {
                x: 0.0,
                y: 0.0,
                z: -1.0,
            },
        ],
    };

    /// 270 degree Y-axis rotation matrix
    pub const ROTATE_Y270: Matrix3 = Matrix3 {
        row: [
            Vector3 {
                x: 0.0,
                y: 0.0,
                z: -1.0,
            },
            Vector3 {
                x: 0.0,
                y: 1.0,
                z: 0.0,
            },
            Vector3 {
                x: 1.0,
                y: 0.0,
                z: 0.0,
            },
        ],
    };

    /// 90 degree Z-axis rotation matrix
    pub const ROTATE_Z90: Matrix3 = Matrix3 {
        row: [
            Vector3 {
                x: 0.0,
                y: -1.0,
                z: 0.0,
            },
            Vector3 {
                x: 1.0,
                y: 0.0,
                z: 0.0,
            },
            Vector3 {
                x: 0.0,
                y: 0.0,
                z: 1.0,
            },
        ],
    };

    /// 180 degree Z-axis rotation matrix
    pub const ROTATE_Z180: Matrix3 = Matrix3 {
        row: [
            Vector3 {
                x: -1.0,
                y: 0.0,
                z: 0.0,
            },
            Vector3 {
                x: 0.0,
                y: -1.0,
                z: 0.0,
            },
            Vector3 {
                x: 0.0,
                y: 0.0,
                z: 1.0,
            },
        ],
    };

    /// 270 degree Z-axis rotation matrix
    pub const ROTATE_Z270: Matrix3 = Matrix3 {
        row: [
            Vector3 {
                x: 0.0,
                y: 1.0,
                z: 0.0,
            },
            Vector3 {
                x: -1.0,
                y: 0.0,
                z: 0.0,
            },
            Vector3 {
                x: 0.0,
                y: 0.0,
                z: 1.0,
            },
        ],
    };

    /// Create a new Matrix3
    pub fn new() -> Self {
        Self::IDENTITY
    }

    /// Create a Matrix3 from individual values
    #[allow(clippy::too_many_arguments)]
    pub fn from_values(
        m11: f32,
        m12: f32,
        m13: f32,
        m21: f32,
        m22: f32,
        m23: f32,
        m31: f32,
        m32: f32,
        m33: f32,
    ) -> Self {
        Self {
            row: [
                Vector3::new(m11, m12, m13),
                Vector3::new(m21, m22, m23),
                Vector3::new(m31, m32, m33),
            ],
        }
    }

    /// Create a Matrix3 from three row vectors
    pub fn from_rows(r0: Vector3, r1: Vector3, r2: Vector3) -> Self {
        Self { row: [r0, r1, r2] }
    }

    /// Create a Matrix3 from an axis and angle
    pub fn from_axis_angle(axis: Vector3, angle: f32) -> Self {
        let mut matrix = Self::new();
        matrix.set_from_axis_angle(axis, angle);
        matrix
    }

    /// Create a Matrix3 from an axis and precomputed sin/cos
    pub fn from_axis_angle_with_sin_cos(axis: Vector3, sin_theta: f32, cos_theta: f32) -> Self {
        let mut matrix = Self::new();
        matrix.set_from_axis_angle_with_sin_cos(axis, sin_theta, cos_theta);
        matrix
    }

    /// Create a Matrix3 from a Matrix3D (extract rotation part)
    pub fn from_matrix3d(m: &crate::matrix3d::Matrix3D) -> Self {
        Self {
            row: [
                Vector3::new(m.row[0].x, m.row[0].y, m.row[0].z),
                Vector3::new(m.row[1].x, m.row[1].y, m.row[1].z),
                Vector3::new(m.row[2].x, m.row[2].y, m.row[2].z),
            ],
        }
    }

    /// Create a Matrix3 from a Matrix4 (extract rotation part)
    pub fn from_matrix4(m: &crate::matrix4::Matrix4) -> Self {
        Self {
            row: [
                Vector3::new(m.row[0].x, m.row[0].y, m.row[0].z),
                Vector3::new(m.row[1].x, m.row[1].y, m.row[1].z),
                Vector3::new(m.row[2].x, m.row[2].y, m.row[2].z),
            ],
        }
    }

    /// Create a Matrix3 from a Quaternion
    pub fn from_quaternion(q: &crate::wwmath::Quaternion) -> Self {
        let mut matrix = Self::new();
        matrix.set_from_quaternion(q);
        matrix
    }

    /// Set matrix to identity
    pub fn make_identity(&mut self) {
        *self = Self::IDENTITY;
    }

    /// Set from three row vectors
    pub fn set_from_rows(&mut self, r0: Vector3, r1: Vector3, r2: Vector3) {
        self.row[0] = r0;
        self.row[1] = r1;
        self.row[2] = r2;
    }

    /// Set from individual values
    #[allow(clippy::too_many_arguments)]
    pub fn set_from_values(
        &mut self,
        m11: f32,
        m12: f32,
        m13: f32,
        m21: f32,
        m22: f32,
        m23: f32,
        m31: f32,
        m32: f32,
        m33: f32,
    ) {
        self.row[0] = Vector3::new(m11, m12, m13);
        self.row[1] = Vector3::new(m21, m22, m23);
        self.row[2] = Vector3::new(m31, m32, m33);
    }

    /// Set from axis and angle
    pub fn set_from_axis_angle(&mut self, axis: Vector3, angle: f32) {
        let sin_theta = angle.sin();
        let cos_theta = angle.cos();
        self.set_from_axis_angle_with_sin_cos(axis, sin_theta, cos_theta);
    }

    /// Set from axis and precomputed sin/cos
    pub fn set_from_axis_angle_with_sin_cos(
        &mut self,
        axis: Vector3,
        sin_theta: f32,
        cos_theta: f32,
    ) {
        let axis = axis.normalize();
        let one_minus_cos = 1.0 - cos_theta;

        self.row[0].x = axis.x * axis.x * one_minus_cos + cos_theta;
        self.row[0].y = axis.x * axis.y * one_minus_cos - axis.z * sin_theta;
        self.row[0].z = axis.z * axis.x * one_minus_cos + axis.y * sin_theta;

        self.row[1].x = axis.x * axis.y * one_minus_cos + axis.z * sin_theta;
        self.row[1].y = axis.y * axis.y * one_minus_cos + cos_theta;
        self.row[1].z = axis.y * axis.z * one_minus_cos - axis.x * sin_theta;

        self.row[2].x = axis.z * axis.x * one_minus_cos - axis.y * sin_theta;
        self.row[2].y = axis.y * axis.z * one_minus_cos + axis.x * sin_theta;
        self.row[2].z = axis.z * axis.z * one_minus_cos + cos_theta;
    }

    /// Set from a Matrix3D (extract rotation part)
    pub fn set_from_matrix3d(&mut self, m: &crate::matrix3d::Matrix3D) {
        self.row[0] = Vector3::new(m.row[0].x, m.row[0].y, m.row[0].z);
        self.row[1] = Vector3::new(m.row[1].x, m.row[1].y, m.row[1].z);
        self.row[2] = Vector3::new(m.row[2].x, m.row[2].y, m.row[2].z);
    }

    /// Set from a Matrix4 (extract rotation part)
    pub fn set_from_matrix4(&mut self, m: &crate::matrix4::Matrix4) {
        self.row[0] = Vector3::new(m.row[0].x, m.row[0].y, m.row[0].z);
        self.row[1] = Vector3::new(m.row[1].x, m.row[1].y, m.row[1].z);
        self.row[2] = Vector3::new(m.row[2].x, m.row[2].y, m.row[2].z);
    }

    /// Set from a Quaternion  
    pub fn set_from_quaternion(&mut self, q: &crate::wwmath::Quaternion) {
        // Convert quaternion to matrix using the standard formula
        // Matrix = I + 2*K*q + 2*K^2*q where K is the skew-symmetric matrix of q.xyz
        let qx = q.x;
        let qy = q.y;
        let qz = q.z;
        let qw = q.w;

        self.row[0].x = 1.0 - 2.0 * (qy * qy + qz * qz);
        self.row[0].y = 2.0 * (qx * qy - qz * qw);
        self.row[0].z = 2.0 * (qz * qx + qy * qw);

        self.row[1].x = 2.0 * (qx * qy + qz * qw);
        self.row[1].y = 1.0 - 2.0 * (qz * qz + qx * qx);
        self.row[1].z = 2.0 * (qy * qz - qx * qw);

        self.row[2].x = 2.0 * (qz * qx - qy * qw);
        self.row[2].y = 2.0 * (qy * qz + qx * qw);
        self.row[2].z = 1.0 - 2.0 * (qy * qy + qx * qx);
    }

    /// Transpose this matrix
    pub fn transpose(&self) -> Self {
        Self {
            row: [
                Vector3::new(self.row[0].x, self.row[1].x, self.row[2].x),
                Vector3::new(self.row[0].y, self.row[1].y, self.row[2].y),
                Vector3::new(self.row[0].z, self.row[1].z, self.row[2].z),
            ],
        }
    }

    /// Calculate determinant
    pub fn determinant(&self) -> f32 {
        self.row[0].x * (self.row[1].y * self.row[2].z - self.row[1].z * self.row[2].y)
            - self.row[0].y * (self.row[1].x * self.row[2].z - self.row[1].z * self.row[2].x)
            + self.row[0].z * (self.row[1].x * self.row[2].y - self.row[1].y * self.row[2].x)
    }

    /// Calculate inverse using Gauss-Jordan elimination
    pub fn inverse(&self) -> Self {
        let mut a = *self; // As a evolves from original matrix into identity
        let mut b = Self::IDENTITY; // b evolves from identity into inverse(a)

        // Loop over columns of a from left to right, eliminating above and below diagonal
        for j in 0..3 {
            // Find largest pivot in column j among rows j..3
            let mut i1 = j;
            for i in (j + 1)..3 {
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
            for i in 0..3 {
                if i != j {
                    let factor = a.row[i][j];
                    b.row[i] -= factor * b.row[j];
                    a.row[i] -= factor * a.row[j];
                }
            }
        }
        b
    }

    /// Rotate around X axis
    pub fn rotate_x(&mut self, theta: f32) {
        let cos_theta = theta.cos();
        let sin_theta = theta.sin();
        self.rotate_x_with_sin_cos(sin_theta, cos_theta);
    }

    /// Rotate around X axis with precomputed sin/cos
    pub fn rotate_x_with_sin_cos(&mut self, sin_theta: f32, cos_theta: f32) {
        let tmp1 = self.row[0].y;
        let tmp2 = self.row[0].z;
        self.row[0].y = cos_theta * tmp1 + sin_theta * tmp2;
        self.row[0].z = -sin_theta * tmp1 + cos_theta * tmp2;

        let tmp1 = self.row[1].y;
        let tmp2 = self.row[1].z;
        self.row[1].y = cos_theta * tmp1 + sin_theta * tmp2;
        self.row[1].z = -sin_theta * tmp1 + cos_theta * tmp2;

        let tmp1 = self.row[2].y;
        let tmp2 = self.row[2].z;
        self.row[2].y = cos_theta * tmp1 + sin_theta * tmp2;
        self.row[2].z = -sin_theta * tmp1 + cos_theta * tmp2;
    }

    /// Rotate around Y axis
    pub fn rotate_y(&mut self, theta: f32) {
        let cos_theta = theta.cos();
        let sin_theta = theta.sin();
        self.rotate_y_with_sin_cos(sin_theta, cos_theta);
    }

    /// Rotate around Y axis with precomputed sin/cos
    pub fn rotate_y_with_sin_cos(&mut self, sin_theta: f32, cos_theta: f32) {
        let tmp1 = self.row[0].x;
        let tmp2 = self.row[0].z;
        self.row[0].x = cos_theta * tmp1 - sin_theta * tmp2;
        self.row[0].z = sin_theta * tmp1 + cos_theta * tmp2;

        let tmp1 = self.row[1].x;
        let tmp2 = self.row[1].z;
        self.row[1].x = cos_theta * tmp1 - sin_theta * tmp2;
        self.row[1].z = sin_theta * tmp1 + cos_theta * tmp2;

        let tmp1 = self.row[2].x;
        let tmp2 = self.row[2].z;
        self.row[2].x = cos_theta * tmp1 - sin_theta * tmp2;
        self.row[2].z = sin_theta * tmp1 + cos_theta * tmp2;
    }

    /// Rotate around Z axis
    pub fn rotate_z(&mut self, theta: f32) {
        let cos_theta = theta.cos();
        let sin_theta = theta.sin();
        self.rotate_z_with_sin_cos(sin_theta, cos_theta);
    }

    /// Rotate around Z axis with precomputed sin/cos
    pub fn rotate_z_with_sin_cos(&mut self, sin_theta: f32, cos_theta: f32) {
        let tmp1 = self.row[0].x;
        let tmp2 = self.row[0].y;
        self.row[0].x = cos_theta * tmp1 + sin_theta * tmp2;
        self.row[0].y = -sin_theta * tmp1 + cos_theta * tmp2;

        let tmp1 = self.row[1].x;
        let tmp2 = self.row[1].y;
        self.row[1].x = cos_theta * tmp1 + sin_theta * tmp2;
        self.row[1].y = -sin_theta * tmp1 + cos_theta * tmp2;

        let tmp1 = self.row[2].x;
        let tmp2 = self.row[2].y;
        self.row[2].x = cos_theta * tmp1 + sin_theta * tmp2;
        self.row[2].y = -sin_theta * tmp1 + cos_theta * tmp2;
    }

    /// Get X vector (first column as row vector)
    pub fn get_x_vector(&self) -> Vector3 {
        Vector3::new(self.row[0].x, self.row[1].x, self.row[2].x)
    }

    /// Get Y vector (second column as row vector)
    pub fn get_y_vector(&self) -> Vector3 {
        Vector3::new(self.row[0].y, self.row[1].y, self.row[2].y)
    }

    /// Get Z vector (third column as row vector)
    pub fn get_z_vector(&self) -> Vector3 {
        Vector3::new(self.row[0].z, self.row[1].z, self.row[2].z)
    }

    /// Get X rotation approximation
    pub fn get_x_rotation(&self) -> f32 {
        let v = *self * Vector3::new(0.0, 1.0, 0.0);
        WWMath::atan2f(v.z, v.y)
    }

    /// Get Y rotation approximation
    pub fn get_y_rotation(&self) -> f32 {
        let v = *self * Vector3::new(0.0, 0.0, 1.0);
        WWMath::atan2f(v.x, v.z)
    }

    /// Get Z rotation approximation
    pub fn get_z_rotation(&self) -> f32 {
        let v = *self * Vector3::new(1.0, 0.0, 0.0);
        WWMath::atan2f(v.y, v.x)
    }

    /// Check if matrix is orthogonal
    pub fn is_orthogonal(&self) -> bool {
        let x = Vector3::new(self.row[0].x, self.row[0].y, self.row[0].z);
        let y = Vector3::new(self.row[1].x, self.row[1].y, self.row[1].z);
        let z = Vector3::new(self.row[2].x, self.row[2].y, self.row[2].z);

        if x.dot(y).abs() > EPSILON {
            return false;
        }
        if y.dot(z).abs() > EPSILON {
            return false;
        }
        if z.dot(x).abs() > EPSILON {
            return false;
        }

        if WWMath::fabs(x.length() - 1.0) > EPSILON {
            return false;
        }
        if WWMath::fabs(y.length() - 1.0) > EPSILON {
            return false;
        }
        if WWMath::fabs(z.length() - 1.0) > EPSILON {
            return false;
        }

        true
    }

    /// Re-orthogonalize the matrix
    pub fn re_orthogonalize(&mut self) {
        let mut x = Vector3::new(self.row[0].x, self.row[0].y, self.row[0].z);
        let mut y = Vector3::new(self.row[1].x, self.row[1].y, self.row[1].z);
        let mut z = x.cross(y);
        y = z.cross(x);

        let len_x = x.length();
        if len_x < EPSILON {
            *self = Self::IDENTITY;
            return;
        } else {
            x /= len_x;
        }

        let len_y = y.length();
        if len_y < EPSILON {
            *self = Self::IDENTITY;
            return;
        } else {
            y /= len_y;
        }

        let len_z = z.length();
        if len_z < EPSILON {
            *self = Self::IDENTITY;
            return;
        } else {
            z /= len_z;
        }

        self.row[0].x = x.x;
        self.row[0].y = x.y;
        self.row[0].z = x.z;

        self.row[1].x = y.x;
        self.row[1].y = y.y;
        self.row[1].z = y.z;

        self.row[2].x = z.x;
        self.row[2].y = z.y;
        self.row[2].z = z.z;
    }

    /// Multiply two matrices into a result matrix
    pub fn multiply(a: Self, b: Self, result: &mut Self) {
        let tmp = a * b;
        *result = tmp;
    }

    /// Multiply Matrix3D with Matrix3 into a result matrix
    pub fn multiply_matrix3d_matrix3(a: &crate::matrix3d::Matrix3D, b: &Self, result: &mut Self) {
        // Extract 3x3 rotation part from Matrix3D and multiply with Matrix3
        result.row[0].x =
            a.row[0].x * b.row[0].x + a.row[0].y * b.row[1].x + a.row[0].z * b.row[2].x;
        result.row[0].y =
            a.row[0].x * b.row[0].y + a.row[0].y * b.row[1].y + a.row[0].z * b.row[2].y;
        result.row[0].z =
            a.row[0].x * b.row[0].z + a.row[0].y * b.row[1].z + a.row[0].z * b.row[2].z;

        result.row[1].x =
            a.row[1].x * b.row[0].x + a.row[1].y * b.row[1].x + a.row[1].z * b.row[2].x;
        result.row[1].y =
            a.row[1].x * b.row[0].y + a.row[1].y * b.row[1].y + a.row[1].z * b.row[2].y;
        result.row[1].z =
            a.row[1].x * b.row[0].z + a.row[1].y * b.row[1].z + a.row[1].z * b.row[2].z;

        result.row[2].x =
            a.row[2].x * b.row[0].x + a.row[2].y * b.row[1].x + a.row[2].z * b.row[2].x;
        result.row[2].y =
            a.row[2].x * b.row[0].y + a.row[2].y * b.row[1].y + a.row[2].z * b.row[2].y;
        result.row[2].z =
            a.row[2].x * b.row[0].z + a.row[2].y * b.row[1].z + a.row[2].z * b.row[2].z;
    }

    /// Multiply Matrix3 with Matrix3D into a result matrix
    pub fn multiply_matrix3_matrix3d(a: &Self, b: &crate::matrix3d::Matrix3D, result: &mut Self) {
        // Multiply Matrix3 with 3x3 rotation part of Matrix3D
        result.row[0].x =
            a.row[0].x * b.row[0].x + a.row[0].y * b.row[1].x + a.row[0].z * b.row[2].x;
        result.row[0].y =
            a.row[0].x * b.row[0].y + a.row[0].y * b.row[1].y + a.row[0].z * b.row[2].y;
        result.row[0].z =
            a.row[0].x * b.row[0].z + a.row[0].y * b.row[1].z + a.row[0].z * b.row[2].z;

        result.row[1].x =
            a.row[1].x * b.row[0].x + a.row[1].y * b.row[1].x + a.row[1].z * b.row[2].x;
        result.row[1].y =
            a.row[1].x * b.row[0].y + a.row[1].y * b.row[1].y + a.row[1].z * b.row[2].y;
        result.row[1].z =
            a.row[1].x * b.row[0].z + a.row[1].y * b.row[1].z + a.row[1].z * b.row[2].z;

        result.row[2].x =
            a.row[2].x * b.row[0].x + a.row[2].y * b.row[1].x + a.row[2].z * b.row[2].x;
        result.row[2].y =
            a.row[2].x * b.row[0].y + a.row[2].y * b.row[1].y + a.row[2].z * b.row[2].y;
        result.row[2].z =
            a.row[2].x * b.row[0].z + a.row[2].y * b.row[1].z + a.row[2].z * b.row[2].z;
    }

    /// Rotate a vector by this matrix
    pub fn rotate_vector(&self, v: Vector3) -> Vector3 {
        Vector3::new(
            self.row[0].x * v.x + self.row[0].y * v.y + self.row[0].z * v.z,
            self.row[1].x * v.x + self.row[1].y * v.y + self.row[1].z * v.z,
            self.row[2].x * v.x + self.row[2].y * v.y + self.row[2].z * v.z,
        )
    }

    /// Rotate a vector by the transpose of this matrix
    pub fn transpose_rotate_vector(&self, v: Vector3) -> Vector3 {
        self.transpose().rotate_vector(v)
    }

    /// Add two matrices into a result matrix
    pub fn add(a: Self, b: Self, result: &mut Self) {
        result.row[0] = a.row[0] + b.row[0];
        result.row[1] = a.row[1] + b.row[1];
        result.row[2] = a.row[2] + b.row[2];
    }

    /// Subtract two matrices into a result matrix
    pub fn subtract(a: Self, b: Self, result: &mut Self) {
        result.row[0] = a.row[0] - b.row[0];
        result.row[1] = a.row[1] - b.row[1];
        result.row[2] = a.row[2] - b.row[2];
    }
}

// Array access implementation
impl Index<usize> for Matrix3 {
    type Output = Vector3;

    fn index(&self, index: usize) -> &Self::Output {
        &self.row[index]
    }
}

impl IndexMut<usize> for Matrix3 {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.row[index]
    }
}

// Arithmetic operations
impl Add for Matrix3 {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Self {
            row: [
                self.row[0] + other.row[0],
                self.row[1] + other.row[1],
                self.row[2] + other.row[2],
            ],
        }
    }
}

impl AddAssign for Matrix3 {
    fn add_assign(&mut self, other: Self) {
        self.row[0] += other.row[0];
        self.row[1] += other.row[1];
        self.row[2] += other.row[2];
    }
}

impl Sub for Matrix3 {
    type Output = Self;

    fn sub(self, other: Self) -> Self {
        Self {
            row: [
                self.row[0] - other.row[0],
                self.row[1] - other.row[1],
                self.row[2] - other.row[2],
            ],
        }
    }
}

impl SubAssign for Matrix3 {
    fn sub_assign(&mut self, other: Self) {
        self.row[0] -= other.row[0];
        self.row[1] -= other.row[1];
        self.row[2] -= other.row[2];
    }
}

impl Mul<f32> for Matrix3 {
    type Output = Self;

    fn mul(self, rhs: f32) -> Self {
        Self {
            row: [self.row[0] * rhs, self.row[1] * rhs, self.row[2] * rhs],
        }
    }
}

impl MulAssign<f32> for Matrix3 {
    fn mul_assign(&mut self, rhs: f32) {
        self.row[0] *= rhs;
        self.row[1] *= rhs;
        self.row[2] *= rhs;
    }
}

impl Div<f32> for Matrix3 {
    type Output = Self;

    fn div(self, rhs: f32) -> Self {
        let inv_rhs = 1.0 / rhs;
        self * inv_rhs
    }
}

impl DivAssign<f32> for Matrix3 {
    fn div_assign(&mut self, rhs: f32) {
        let inv_rhs = 1.0 / rhs;
        *self *= inv_rhs;
    }
}

impl Neg for Matrix3 {
    type Output = Self;

    fn neg(self) -> Self {
        Self {
            row: [-self.row[0], -self.row[1], -self.row[2]],
        }
    }
}

impl Mul for Matrix3 {
    type Output = Self;

    fn mul(self, other: Self) -> Self {
        Self {
            row: [
                Vector3::new(
                    self.row[0].x * other.row[0].x
                        + self.row[0].y * other.row[1].x
                        + self.row[0].z * other.row[2].x,
                    self.row[0].x * other.row[0].y
                        + self.row[0].y * other.row[1].y
                        + self.row[0].z * other.row[2].y,
                    self.row[0].x * other.row[0].z
                        + self.row[0].y * other.row[1].z
                        + self.row[0].z * other.row[2].z,
                ),
                Vector3::new(
                    self.row[1].x * other.row[0].x
                        + self.row[1].y * other.row[1].x
                        + self.row[1].z * other.row[2].x,
                    self.row[1].x * other.row[0].y
                        + self.row[1].y * other.row[1].y
                        + self.row[1].z * other.row[2].y,
                    self.row[1].x * other.row[0].z
                        + self.row[1].y * other.row[1].z
                        + self.row[1].z * other.row[2].z,
                ),
                Vector3::new(
                    self.row[2].x * other.row[0].x
                        + self.row[2].y * other.row[1].x
                        + self.row[2].z * other.row[2].x,
                    self.row[2].x * other.row[0].y
                        + self.row[2].y * other.row[1].y
                        + self.row[2].z * other.row[2].y,
                    self.row[2].x * other.row[0].z
                        + self.row[2].y * other.row[1].z
                        + self.row[2].z * other.row[2].z,
                ),
            ],
        }
    }
}

impl Mul<Vector3> for Matrix3 {
    type Output = Vector3;

    fn mul(self, v: Vector3) -> Vector3 {
        self.rotate_vector(v)
    }
}

// Scalar multiplication (reverse order)
impl Mul<Matrix3> for f32 {
    type Output = Matrix3;

    fn mul(self, rhs: Matrix3) -> Matrix3 {
        rhs * self
    }
}

// Implement From traits for convenient conversions
impl From<&crate::matrix3d::Matrix3D> for Matrix3 {
    fn from(m: &crate::matrix3d::Matrix3D) -> Self {
        Self::from_matrix3d(m)
    }
}

impl From<&crate::matrix4::Matrix4> for Matrix3 {
    fn from(m: &crate::matrix4::Matrix4) -> Self {
        Self::from_matrix4(m)
    }
}

impl From<&crate::wwmath::Quaternion> for Matrix3 {
    fn from(q: &crate::wwmath::Quaternion) -> Self {
        Self::from_quaternion(q)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_identity() {
        let m = Matrix3::IDENTITY;
        let v = Vector3::new(1.0, 2.0, 3.0);
        assert_eq!(m * v, v);
    }

    #[test]
    fn test_transpose() {
        let m = Matrix3::from_values(1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0);
        let t = m.transpose();
        assert_eq!(t.row[0], Vector3::new(1.0, 4.0, 7.0));
        assert_eq!(t.row[1], Vector3::new(2.0, 5.0, 8.0));
        assert_eq!(t.row[2], Vector3::new(3.0, 6.0, 9.0));
    }

    #[test]
    fn test_determinant() {
        let m = Matrix3::from_values(1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0);
        assert_eq!(m.determinant(), 0.0); // This matrix is singular

        let m2 = Matrix3::from_values(2.0, 0.0, 0.0, 0.0, 2.0, 0.0, 0.0, 0.0, 2.0);
        assert_eq!(m2.determinant(), 8.0);
    }

    #[test]
    fn test_inverse() {
        let m = Matrix3::from_values(2.0, 0.0, 0.0, 0.0, 2.0, 0.0, 0.0, 0.0, 2.0);
        let inv = m.inverse();
        let expected = Matrix3::from_values(0.5, 0.0, 0.0, 0.0, 0.5, 0.0, 0.0, 0.0, 0.5);
        assert!((inv.row[0] - expected.row[0]).length() < 1e-6);
        assert!((inv.row[1] - expected.row[1]).length() < 1e-6);
        assert!((inv.row[2] - expected.row[2]).length() < 1e-6);
    }

    #[test]
    fn test_rotate_x() {
        let mut m = Matrix3::IDENTITY;
        m.rotate_x(std::f32::consts::PI / 2.0); // 90 degrees

        let result = m * Vector3::new(0.0, 1.0, 0.0);
        assert!((result - Vector3::new(0.0, 0.0, 1.0)).length() < 1e-6);
    }

    #[test]
    fn test_rotate_y() {
        let mut m = Matrix3::IDENTITY;
        m.rotate_y(std::f32::consts::PI / 2.0); // 90 degrees

        let result = m * Vector3::new(1.0, 0.0, 0.0);
        assert!((result - Vector3::new(0.0, 0.0, -1.0)).length() < 1e-6);
    }

    #[test]
    fn test_rotate_z() {
        let mut m = Matrix3::IDENTITY;
        m.rotate_z(std::f32::consts::PI / 2.0); // 90 degrees

        let result = m * Vector3::new(1.0, 0.0, 0.0);
        assert!((result - Vector3::new(0.0, 1.0, 0.0)).length() < 1e-6);
    }

    #[test]
    fn test_get_vectors() {
        let m = Matrix3::from_values(1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0);

        assert_eq!(m.get_x_vector(), Vector3::new(1.0, 4.0, 7.0));
        assert_eq!(m.get_y_vector(), Vector3::new(2.0, 5.0, 8.0));
        assert_eq!(m.get_z_vector(), Vector3::new(3.0, 6.0, 9.0));
    }

    #[test]
    fn test_matrix_multiplication() {
        let m1 = Matrix3::from_values(1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0);
        let m2 = Matrix3::IDENTITY;

        assert_eq!(m1 * m2, m1);
    }

    #[test]
    fn test_vector_transformation() {
        let m = Matrix3::ROTATE_Z90;
        let v = Vector3::new(1.0, 0.0, 0.0);
        let result = m * v;
        assert!((result - Vector3::new(0.0, 1.0, 0.0)).length() < 1e-6);
    }

    #[test]
    fn test_add_subtract() {
        let m1 = Matrix3::from_values(1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0);
        let m2 = Matrix3::from_values(0.5, 1.0, 1.5, 2.0, 2.5, 3.0, 3.5, 4.0, 4.5);

        let sum = m1 + m2;
        let diff = m1 - m2;

        assert_eq!(sum.row[0], Vector3::new(1.5, 3.0, 4.5));
        assert_eq!(diff.row[0], Vector3::new(0.5, 1.0, 1.5));
    }

    #[test]
    fn test_scalar_operations() {
        let m = Matrix3::from_values(1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0);

        let scaled = m * 2.0;
        assert_eq!(scaled.row[0], Vector3::new(2.0, 4.0, 6.0));

        let divided = m / 2.0;
        assert_eq!(divided.row[0], Vector3::new(0.5, 1.0, 1.5));
    }

    #[test]
    fn test_negation() {
        let m = Matrix3::from_values(1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0);

        let neg = -m;
        assert_eq!(neg.row[0], Vector3::new(-1.0, -2.0, -3.0));
    }

    #[test]
    fn test_indexing() {
        let m = Matrix3::from_values(1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0);

        assert_eq!(m[0], Vector3::new(1.0, 2.0, 3.0));
        assert_eq!(m[1], Vector3::new(4.0, 5.0, 6.0));
        assert_eq!(m[2], Vector3::new(7.0, 8.0, 9.0));
    }

    #[test]
    fn test_quaternion_conversion() {
        // Test with identity quaternion
        let q = crate::wwmath::Quaternion::from_components(0.0, 0.0, 0.0, 1.0); // w=1 for identity
        let m = Matrix3::from_quaternion(&q);
        let identity_diff = (m - Matrix3::IDENTITY);

        // Check that conversion gives identity matrix
        assert!(identity_diff.row[0].length() < 1e-6);
        assert!(identity_diff.row[1].length() < 1e-6);
        assert!(identity_diff.row[2].length() < 1e-6);
    }

    #[test]
    fn test_matrix_conversions() {
        // Test Matrix3D conversion
        let m3d = crate::matrix3d::Matrix3D::IDENTITY;
        let m3 = Matrix3::from_matrix3d(&m3d);
        let diff = (m3 - Matrix3::IDENTITY);
        assert!(diff.row[0].length() < 1e-6);
        assert!(diff.row[1].length() < 1e-6);
        assert!(diff.row[2].length() < 1e-6);

        // Test Matrix4 conversion
        let m4 = crate::matrix4::Matrix4::IDENTITY;
        let m3_from_m4 = Matrix3::from_matrix4(&m4);
        let diff4 = (m3_from_m4 - Matrix3::IDENTITY);
        assert!(diff4.row[0].length() < 1e-6);
        assert!(diff4.row[1].length() < 1e-6);
        assert!(diff4.row[2].length() < 1e-6);
    }

    #[test]
    fn test_orthogonality() {
        let m = Matrix3::ROTATE_Z90;
        assert!(m.is_orthogonal());

        // Test a non-orthogonal matrix
        let non_orthogonal = Matrix3::from_values(2.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0);
        assert!(!non_orthogonal.is_orthogonal());
    }

    #[test]
    fn test_re_orthogonalize() {
        let mut m = Matrix3::from_values(1.1, 0.1, 0.0, 0.1, 1.0, 0.0, 0.0, 0.0, 1.0);

        m.re_orthogonalize();
        assert!(m.is_orthogonal());
    }
}
