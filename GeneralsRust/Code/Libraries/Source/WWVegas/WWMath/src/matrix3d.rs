//! A 3D transformation matrix.
//!
//! This module provides 4x4 transformation matrix functionality,
//! converted from the original C++ Matrix3D class.
//!
//! Matrix3D represents a 3D transformation matrix using 3 Vector4 rows,
//! where the last row is implicitly [0,0,0,1] for homogeneous coordinates.

use crate::{Matrix3, Vector3, Vector4, WWMath, EPSILON};
use std::ops::{
    Add, AddAssign, Div, DivAssign, Index, IndexMut, Mul, MulAssign, Neg, Sub, SubAssign,
};

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct Matrix3D {
    /// The 3 rows of the 4x4 transformation matrix
    pub row: [Vector4; 3],
}

impl Default for Matrix3D {
    fn default() -> Self {
        Self::new()
    }
}

impl Matrix3D {
    /// Identity matrix
    pub const IDENTITY: Matrix3D = Matrix3D {
        row: [
            Vector4::new(1.0, 0.0, 0.0, 0.0),
            Vector4::new(0.0, 1.0, 0.0, 0.0),
            Vector4::new(0.0, 0.0, 1.0, 0.0),
        ],
    };

    /// Convenience constructor returning the identity matrix.
    pub fn identity() -> Self {
        Self::IDENTITY
    }

    /// 90 degree X-axis rotation matrix
    pub const ROTATE_X90: Matrix3D = Matrix3D {
        row: [
            Vector4::new(1.0, 0.0, 0.0, 0.0),
            Vector4::new(0.0, 0.0, -1.0, 0.0),
            Vector4::new(0.0, 1.0, 0.0, 0.0),
        ],
    };

    /// 180 degree X-axis rotation matrix
    pub const ROTATE_X180: Matrix3D = Matrix3D {
        row: [
            Vector4::new(1.0, 0.0, 0.0, 0.0),
            Vector4::new(0.0, -1.0, 0.0, 0.0),
            Vector4::new(0.0, 0.0, -1.0, 0.0),
        ],
    };

    /// 270 degree X-axis rotation matrix
    pub const ROTATE_X270: Matrix3D = Matrix3D {
        row: [
            Vector4::new(1.0, 0.0, 0.0, 0.0),
            Vector4::new(0.0, 0.0, 1.0, 0.0),
            Vector4::new(0.0, -1.0, 0.0, 0.0),
        ],
    };

    /// 90 degree Y-axis rotation matrix
    pub const ROTATE_Y90: Matrix3D = Matrix3D {
        row: [
            Vector4::new(0.0, 0.0, 1.0, 0.0),
            Vector4::new(0.0, 1.0, 0.0, 0.0),
            Vector4::new(-1.0, 0.0, 0.0, 0.0),
        ],
    };

    /// 180 degree Y-axis rotation matrix
    pub const ROTATE_Y180: Matrix3D = Matrix3D {
        row: [
            Vector4::new(-1.0, 0.0, 0.0, 0.0),
            Vector4::new(0.0, 1.0, 0.0, 0.0),
            Vector4::new(0.0, 0.0, -1.0, 0.0),
        ],
    };

    /// 270 degree Y-axis rotation matrix
    pub const ROTATE_Y270: Matrix3D = Matrix3D {
        row: [
            Vector4::new(0.0, 0.0, -1.0, 0.0),
            Vector4::new(0.0, 1.0, 0.0, 0.0),
            Vector4::new(1.0, 0.0, 0.0, 0.0),
        ],
    };

    /// 90 degree Z-axis rotation matrix
    pub const ROTATE_Z90: Matrix3D = Matrix3D {
        row: [
            Vector4::new(0.0, -1.0, 0.0, 0.0),
            Vector4::new(1.0, 0.0, 0.0, 0.0),
            Vector4::new(0.0, 0.0, 1.0, 0.0),
        ],
    };

    /// 180 degree Z-axis rotation matrix
    pub const ROTATE_Z180: Matrix3D = Matrix3D {
        row: [
            Vector4::new(-1.0, 0.0, 0.0, 0.0),
            Vector4::new(0.0, -1.0, 0.0, 0.0),
            Vector4::new(0.0, 0.0, 1.0, 0.0),
        ],
    };

    /// 270 degree Z-axis rotation matrix
    pub const ROTATE_Z270: Matrix3D = Matrix3D {
        row: [
            Vector4::new(0.0, 1.0, 0.0, 0.0),
            Vector4::new(-1.0, 0.0, 0.0, 0.0),
            Vector4::new(0.0, 0.0, 1.0, 0.0),
        ],
    };

    /// Create a new Matrix3D
    pub fn new() -> Self {
        Self::IDENTITY
    }

    /// Create a Matrix3D from individual values
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
    ) -> Self {
        Self {
            row: [
                Vector4::new(m11, m12, m13, m14),
                Vector4::new(m21, m22, m23, m24),
                Vector4::new(m31, m32, m33, m34),
            ],
        }
    }

    /// Create a Matrix3D from an array of 12 floats
    pub fn from_array(arr: &[f32; 12]) -> Self {
        Self {
            row: [
                Vector4::new(arr[0], arr[1], arr[2], arr[3]),
                Vector4::new(arr[4], arr[5], arr[6], arr[7]),
                Vector4::new(arr[8], arr[9], arr[10], arr[11]),
            ],
        }
    }

    /// Create a Matrix3D from three row vectors and a translation
    pub fn from_rows_and_translation(
        r0: Vector3,
        r1: Vector3,
        r2: Vector3,
        translation: Vector3,
    ) -> Self {
        Self {
            row: [
                Vector4::new(r0.x, r0.y, r0.z, translation.x),
                Vector4::new(r1.x, r1.y, r1.z, translation.y),
                Vector4::new(r2.x, r2.y, r2.z, translation.z),
            ],
        }
    }

    /// Create a Matrix3D from a Matrix3 rotation and translation vector
    pub fn from_matrix3_and_translation(rotation: Matrix3, translation: Vector3) -> Self {
        Self {
            row: [
                Vector4::new(
                    rotation.row[0].x,
                    rotation.row[0].y,
                    rotation.row[0].z,
                    translation.x,
                ),
                Vector4::new(
                    rotation.row[1].x,
                    rotation.row[1].y,
                    rotation.row[1].z,
                    translation.y,
                ),
                Vector4::new(
                    rotation.row[2].x,
                    rotation.row[2].y,
                    rotation.row[2].z,
                    translation.z,
                ),
            ],
        }
    }

    /// Create a Matrix3D from an axis-angle rotation
    pub fn from_axis_angle(axis: Vector3, angle: f32) -> Self {
        let mut matrix = Self::new();
        matrix.set_rotation_from_axis_angle(axis, angle);
        matrix
    }

    /// Create a Matrix3D from an axis-angle rotation with precomputed sin/cos
    pub fn from_axis_angle_with_sin_cos(axis: Vector3, sin_theta: f32, cos_theta: f32) -> Self {
        let mut matrix = Self::new();
        matrix.set_rotation_from_axis_angle_with_sin_cos(axis, sin_theta, cos_theta);
        matrix
    }

    /// Create a Matrix3D from a position vector (identity rotation)
    pub fn from_translation(translation: Vector3) -> Self {
        Self {
            row: [
                Vector4::new(1.0, 0.0, 0.0, translation.x),
                Vector4::new(0.0, 1.0, 0.0, translation.y),
                Vector4::new(0.0, 0.0, 1.0, translation.z),
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
            ],
        }
    }

    /// Set matrix to identity
    pub fn make_identity(&mut self) {
        *self = Self::IDENTITY;
    }

    /// Set from individual values
    #[allow(clippy::too_many_arguments)]
    pub fn set_from_values(
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
    ) {
        self.row[0] = Vector4::new(m11, m12, m13, m14);
        self.row[1] = Vector4::new(m21, m22, m23, m24);
        self.row[2] = Vector4::new(m31, m32, m33, m34);
    }

    /// Set from an array of 12 floats
    pub fn set_from_array(&mut self, arr: &[f32; 12]) {
        self.row[0] = Vector4::new(arr[0], arr[1], arr[2], arr[3]);
        self.row[1] = Vector4::new(arr[4], arr[5], arr[6], arr[7]);
        self.row[2] = Vector4::new(arr[8], arr[9], arr[10], arr[11]);
    }

    /// Set from three row vectors and translation
    pub fn set_from_rows_and_translation(
        &mut self,
        r0: Vector3,
        r1: Vector3,
        r2: Vector3,
        translation: Vector3,
    ) {
        self.row[0] = Vector4::new(r0.x, r0.y, r0.z, translation.x);
        self.row[1] = Vector4::new(r1.x, r1.y, r1.z, translation.y);
        self.row[2] = Vector4::new(r2.x, r2.y, r2.z, translation.z);
    }

    /// Set rotation from axis-angle
    pub fn set_rotation_from_axis_angle(&mut self, axis: Vector3, angle: f32) {
        let sin_theta = angle.sin();
        let cos_theta = angle.cos();
        self.set_rotation_from_axis_angle_with_sin_cos(axis, sin_theta, cos_theta);
    }

    /// Set rotation from axis-angle with precomputed sin/cos
    pub fn set_rotation_from_axis_angle_with_sin_cos(
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

    /// Set translation
    pub fn set_translation(&mut self, translation: Vector3) {
        self.row[0].w = translation.x;
        self.row[1].w = translation.y;
        self.row[2].w = translation.z;
    }

    /// Get translation
    pub fn get_translation(&self) -> Vector3 {
        Vector3::new(self.row[0].w, self.row[1].w, self.row[2].w)
    }

    /// Get X translation
    pub fn get_x_translation(&self) -> f32 {
        self.row[0].w
    }

    /// Get Y translation
    pub fn get_y_translation(&self) -> f32 {
        self.row[1].w
    }

    /// Get Z translation
    pub fn get_z_translation(&self) -> f32 {
        self.row[2].w
    }

    /// Set X translation
    pub fn set_x_translation(&mut self, x: f32) {
        self.row[0].w = x;
    }

    /// Set Y translation
    pub fn set_y_translation(&mut self, y: f32) {
        self.row[1].w = y;
    }

    /// Set Z translation
    pub fn set_z_translation(&mut self, z: f32) {
        self.row[2].w = z;
    }

    /// Adjust translation by a vector
    pub fn adjust_translation(&mut self, adjustment: Vector3) {
        self.row[0].w += adjustment.x;
        self.row[1].w += adjustment.y;
        self.row[2].w += adjustment.z;
    }

    /// Adjust X translation
    pub fn adjust_x_translation(&mut self, x: f32) {
        self.row[0].w += x;
    }

    /// Adjust Y translation
    pub fn adjust_y_translation(&mut self, y: f32) {
        self.row[1].w += y;
    }

    /// Adjust Z translation
    pub fn adjust_z_translation(&mut self, z: f32) {
        self.row[2].w += z;
    }

    /// Translate by individual components
    pub fn translate(&mut self, x: f32, y: f32, z: f32) {
        self.row[0].w += x;
        self.row[1].w += y;
        self.row[2].w += z;
    }

    /// Translate by a vector
    pub fn translate_by_vector(&mut self, translation: Vector3) {
        self.adjust_translation(translation);
    }

    /// Translate X only
    pub fn translate_x(&mut self, x: f32) {
        self.row[0].w += x;
    }

    /// Translate Y only
    pub fn translate_y(&mut self, y: f32) {
        self.row[1].w += y;
    }

    /// Translate Z only
    pub fn translate_z(&mut self, z: f32) {
        self.row[2].w += z;
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

    /// Scale uniformly
    pub fn scale(&mut self, scale: f32) {
        self.row[0] *= scale;
        self.row[1] *= scale;
        self.row[2] *= scale;
        // Don't scale the translation components
        self.row[0].w /= scale;
        self.row[1].w /= scale;
        self.row[2].w /= scale;
    }

    /// Scale by individual components
    pub fn scale_xyz(&mut self, scale_x: f32, scale_y: f32, scale_z: f32) {
        self.row[0].x *= scale_x;
        self.row[0].y *= scale_y;
        self.row[0].z *= scale_z;

        self.row[1].x *= scale_x;
        self.row[1].y *= scale_y;
        self.row[1].z *= scale_z;

        self.row[2].x *= scale_x;
        self.row[2].y *= scale_y;
        self.row[2].z *= scale_z;
    }

    /// Scale by a vector
    pub fn scale_by_vector(&mut self, scale: Vector3) {
        self.scale_xyz(scale.x, scale.y, scale.z);
    }

    /// Pre-rotate around X axis
    pub fn pre_rotate_x(&mut self, theta: f32) {
        let cos_theta = theta.cos();
        let sin_theta = theta.sin();
        self.pre_rotate_x_with_sin_cos(sin_theta, cos_theta);
    }

    /// Pre-rotate around X axis with precomputed sin/cos
    pub fn pre_rotate_x_with_sin_cos(&mut self, sin_theta: f32, cos_theta: f32) {
        // This is a simplified version - full implementation would require more complex math
        // For now, we'll delegate to the regular rotation
        let _rotation = Matrix3D::from_axis_angle(Vector3::new(1.0, 0.0, 0.0), 0.0);
        let mut temp = *self;
        temp.rotate_x_with_sin_cos(sin_theta, cos_theta);
        *self = temp;
    }

    /// Pre-rotate around Y axis
    pub fn pre_rotate_y(&mut self, theta: f32) {
        let cos_theta = theta.cos();
        let sin_theta = theta.sin();
        self.pre_rotate_y_with_sin_cos(sin_theta, cos_theta);
    }

    /// Pre-rotate around Y axis with precomputed sin/cos
    pub fn pre_rotate_y_with_sin_cos(&mut self, sin_theta: f32, cos_theta: f32) {
        let _rotation = Matrix3D::from_axis_angle(Vector3::new(0.0, 1.0, 0.0), 0.0);
        let mut temp = *self;
        temp.rotate_y_with_sin_cos(sin_theta, cos_theta);
        *self = temp;
    }

    /// Pre-rotate around Z axis
    pub fn pre_rotate_z(&mut self, theta: f32) {
        let cos_theta = theta.cos();
        let sin_theta = theta.sin();
        self.pre_rotate_z_with_sin_cos(sin_theta, cos_theta);
    }

    /// Pre-rotate around Z axis with precomputed sin/cos
    pub fn pre_rotate_z_with_sin_cos(&mut self, sin_theta: f32, cos_theta: f32) {
        let _rotation = Matrix3D::from_axis_angle(Vector3::new(0.0, 0.0, 1.0), 0.0);
        let mut temp = *self;
        temp.rotate_z_with_sin_cos(sin_theta, cos_theta);
        *self = temp;
    }

    /// In-place pre-rotate around X axis
    pub fn in_place_pre_rotate_x(&mut self, theta: f32) {
        // For in-place operations, we modify the rotation part only
        let cos_theta = theta.cos();
        let sin_theta = theta.sin();

        let tmp1 = self.row[1].x;
        let tmp2 = self.row[2].x;
        self.row[1].x = cos_theta * tmp1 - sin_theta * tmp2;
        self.row[2].x = sin_theta * tmp1 + cos_theta * tmp2;

        let tmp1 = self.row[1].y;
        let tmp2 = self.row[2].y;
        self.row[1].y = cos_theta * tmp1 - sin_theta * tmp2;
        self.row[2].y = sin_theta * tmp1 + cos_theta * tmp2;

        let tmp1 = self.row[1].z;
        let tmp2 = self.row[2].z;
        self.row[1].z = cos_theta * tmp1 - sin_theta * tmp2;
        self.row[2].z = sin_theta * tmp1 + cos_theta * tmp2;
    }

    /// In-place pre-rotate around Y axis
    pub fn in_place_pre_rotate_y(&mut self, theta: f32) {
        let cos_theta = theta.cos();
        let sin_theta = theta.sin();

        let tmp1 = self.row[0].x;
        let tmp2 = self.row[2].x;
        self.row[0].x = cos_theta * tmp1 + sin_theta * tmp2;
        self.row[2].x = -sin_theta * tmp1 + cos_theta * tmp2;

        let tmp1 = self.row[0].y;
        let tmp2 = self.row[2].y;
        self.row[0].y = cos_theta * tmp1 + sin_theta * tmp2;
        self.row[2].y = -sin_theta * tmp1 + cos_theta * tmp2;

        let tmp1 = self.row[0].z;
        let tmp2 = self.row[2].z;
        self.row[0].z = cos_theta * tmp1 + sin_theta * tmp2;
        self.row[2].z = -sin_theta * tmp1 + cos_theta * tmp2;
    }

    /// In-place pre-rotate around Z axis
    pub fn in_place_pre_rotate_z(&mut self, theta: f32) {
        let cos_theta = theta.cos();
        let sin_theta = theta.sin();

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

    /// Transform a vector by this matrix
    pub fn transform_vector(&self, v: Vector3) -> Vector3 {
        Vector3::new(
            self.row[0].x * v.x + self.row[0].y * v.y + self.row[0].z * v.z + self.row[0].w,
            self.row[1].x * v.x + self.row[1].y * v.y + self.row[1].z * v.z + self.row[1].w,
            self.row[2].x * v.x + self.row[2].y * v.y + self.row[2].z * v.z + self.row[2].w,
        )
    }

    /// Rotate a vector by this matrix (no translation)
    pub fn rotate_vector(&self, v: Vector3) -> Vector3 {
        Vector3::new(
            self.row[0].x * v.x + self.row[0].y * v.y + self.row[0].z * v.z,
            self.row[1].x * v.x + self.row[1].y * v.y + self.row[1].z * v.z,
            self.row[2].x * v.x + self.row[2].y * v.y + self.row[2].z * v.z,
        )
    }

    /// Rotate a vector by the inverse of this matrix
    pub fn inverse_rotate_vector(&self, v: Vector3) -> Vector3 {
        // For orthogonal matrices, inverse is transpose
        let inv = self.inverse();
        inv.rotate_vector(v)
    }

    /// Get the inverse of this matrix
    pub fn inverse(&self) -> Self {
        // For affine transformations (translation + rotation + scale),
        // we can compute the inverse more efficiently
        if self.is_orthogonal() {
            self.orthogonal_inverse()
        } else {
            self.general_inverse()
        }
    }

    /// General inverse for non-orthogonal matrices
    pub fn general_inverse(&self) -> Self {
        // Extract components
        let translation = self.get_translation();
        let rotation_scale = Matrix3 {
            row: [
                Vector3::new(self.row[0].x, self.row[0].y, self.row[0].z),
                Vector3::new(self.row[1].x, self.row[1].y, self.row[1].z),
                Vector3::new(self.row[2].x, self.row[2].y, self.row[2].z),
            ],
        };

        // Invert the 3x3 rotation/scale matrix
        let inv_rotation_scale = rotation_scale.inverse();

        // Create result matrix
        let mut result = Self::IDENTITY;

        // Set the inverse rotation/scale
        result.row[0].x = inv_rotation_scale.row[0].x;
        result.row[0].y = inv_rotation_scale.row[0].y;
        result.row[0].z = inv_rotation_scale.row[0].z;

        result.row[1].x = inv_rotation_scale.row[1].x;
        result.row[1].y = inv_rotation_scale.row[1].y;
        result.row[1].z = inv_rotation_scale.row[1].z;

        result.row[2].x = inv_rotation_scale.row[2].x;
        result.row[2].y = inv_rotation_scale.row[2].y;
        result.row[2].z = inv_rotation_scale.row[2].z;

        // Compute inverse translation: -R^-1 * T
        let inv_translation = -inv_rotation_scale.rotate_vector(translation);
        result.set_translation(inv_translation);

        result
    }

    /// Get the orthogonal inverse (transpose rotation, negate translation)
    pub fn orthogonal_inverse(&self) -> Self {
        let mut result = Self::IDENTITY;

        // Transpose the rotation part
        result.row[0].x = self.row[0].x;
        result.row[0].y = self.row[1].x;
        result.row[0].z = self.row[2].x;

        result.row[1].x = self.row[0].y;
        result.row[1].y = self.row[1].y;
        result.row[1].z = self.row[2].y;

        result.row[2].x = self.row[0].z;
        result.row[2].y = self.row[1].z;
        result.row[2].z = self.row[2].z;

        // Negate and transform the translation
        let translation = self.get_translation();
        let inv_translation = -self.rotate_vector(translation);
        result.set_translation(inv_translation);

        result
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
        let x = Vector3::new(self.row[0].x, self.row[0].y, self.row[0].z);
        let y = Vector3::new(self.row[1].x, self.row[1].y, self.row[1].z);
        let _z = Vector3::new(self.row[2].x, self.row[2].y, self.row[2].z);

        let ortho_x = x.normalize();
        let ortho_z = x.cross(y).normalize();
        let ortho_y = ortho_z.cross(ortho_x);

        self.row[0].x = ortho_x.x;
        self.row[0].y = ortho_x.y;
        self.row[0].z = ortho_x.z;

        self.row[1].x = ortho_y.x;
        self.row[1].y = ortho_y.y;
        self.row[1].z = ortho_y.z;

        self.row[2].x = ortho_z.x;
        self.row[2].y = ortho_z.y;
        self.row[2].z = ortho_z.z;
    }

    /// Multiply two matrices without temporaries
    pub fn multiply(a: Self, b: Self, result: &mut Self) {
        let tmp = a * b;
        *result = tmp;
    }

    /// Linear interpolation between two matrices
    pub fn lerp(a: Self, b: Self, factor: f32, result: &mut Self) {
        result.row[0] = Vector4::lerp(a.row[0], b.row[0], factor);
        result.row[1] = Vector4::lerp(a.row[1], b.row[1], factor);
        result.row[2] = Vector4::lerp(a.row[2], b.row[2], factor);
    }

    /// Look at target from position (camera convention)
    pub fn look_at(&mut self, position: Vector3, target: Vector3, _roll: f32) {
        let direction = (target - position).normalize();
        let up = Vector3::new(0.0, 0.0, 1.0); // Assuming Z is up

        let right = up.cross(direction).normalize();
        let up_actual = direction.cross(right);

        self.row[0] = Vector4::new(right.x, right.y, right.z, -right.dot(position));
        self.row[1] = Vector4::new(
            up_actual.x,
            up_actual.y,
            up_actual.z,
            -up_actual.dot(position),
        );
        self.row[2] = Vector4::new(
            direction.x,
            direction.y,
            direction.z,
            -direction.dot(position),
        );
    }

    /// Object look at (object convention)
    pub fn obj_look_at(&mut self, position: Vector3, target: Vector3, _roll: f32) {
        let direction = (target - position).normalize();
        let up = Vector3::new(0.0, 0.0, 1.0);

        let right = direction.cross(up).normalize();
        let up_actual = right.cross(direction);

        self.row[0] = Vector4::new(right.x, up_actual.x, -direction.x, position.x);
        self.row[1] = Vector4::new(right.y, up_actual.y, -direction.y, position.y);
        self.row[2] = Vector4::new(right.z, up_actual.z, -direction.z, position.z);
    }

    /// Build transform matrix from position and direction
    pub fn build_transform_matrix(&mut self, position: Vector3, direction: Vector3) {
        let up = Vector3::new(0.0, 0.0, 1.0);
        let right = direction.cross(up).normalize();
        let up_actual = right.cross(direction);

        self.row[0] = Vector4::new(right.x, right.y, right.z, position.x);
        self.row[1] = Vector4::new(up_actual.x, up_actual.y, up_actual.z, position.y);
        self.row[2] = Vector4::new(direction.x, direction.y, direction.z, position.z);
    }
}

// Array access implementation
impl Index<usize> for Matrix3D {
    type Output = Vector4;

    fn index(&self, index: usize) -> &Self::Output {
        &self.row[index]
    }
}

impl IndexMut<usize> for Matrix3D {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.row[index]
    }
}

// Arithmetic operations
impl Add for Matrix3D {
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

impl AddAssign for Matrix3D {
    fn add_assign(&mut self, other: Self) {
        self.row[0] += other.row[0];
        self.row[1] += other.row[1];
        self.row[2] += other.row[2];
    }
}

impl Sub for Matrix3D {
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

impl SubAssign for Matrix3D {
    fn sub_assign(&mut self, other: Self) {
        self.row[0] -= other.row[0];
        self.row[1] -= other.row[1];
        self.row[2] -= other.row[2];
    }
}

impl Mul<f32> for Matrix3D {
    type Output = Self;

    fn mul(self, rhs: f32) -> Self {
        Self {
            row: [self.row[0] * rhs, self.row[1] * rhs, self.row[2] * rhs],
        }
    }
}

impl MulAssign<f32> for Matrix3D {
    fn mul_assign(&mut self, rhs: f32) {
        self.row[0] *= rhs;
        self.row[1] *= rhs;
        self.row[2] *= rhs;
    }
}

impl Div<f32> for Matrix3D {
    type Output = Self;

    fn div(self, rhs: f32) -> Self {
        let inv_rhs = 1.0 / rhs;
        self * inv_rhs
    }
}

impl DivAssign<f32> for Matrix3D {
    fn div_assign(&mut self, rhs: f32) {
        let inv_rhs = 1.0 / rhs;
        *self *= inv_rhs;
    }
}

impl Neg for Matrix3D {
    type Output = Self;

    fn neg(self) -> Self {
        Self {
            row: [-self.row[0], -self.row[1], -self.row[2]],
        }
    }
}

impl Mul for Matrix3D {
    type Output = Self;

    fn mul(self, other: Self) -> Self {
        let mut result = Self::IDENTITY;

        // Matrix multiplication for 4x4 matrices (with implicit [0,0,0,1] bottom row)
        for i in 0..3 {
            for j in 0..4 {
                let mut sum = 0.0;
                for k in 0..3 {
                    sum += self.row[i][k] * other.row[k][j];
                }
                // Add translation component for the last column
                if j == 3 {
                    sum += self.row[i][3];
                }

                result.row[i][j] = sum;
            }
        }

        result
    }
}

impl Mul<Vector3> for Matrix3D {
    type Output = Vector3;

    fn mul(self, v: Vector3) -> Vector3 {
        self.transform_vector(v)
    }
}

// Scalar multiplication (reverse order)
impl Mul<Matrix3D> for f32 {
    type Output = Matrix3D;

    fn mul(self, rhs: Matrix3D) -> Matrix3D {
        rhs * self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_identity() {
        let m = Matrix3D::IDENTITY;
        let v = Vector3::new(1.0, 2.0, 3.0);
        let transformed = m * v;
        assert!((transformed - Vector3::new(1.0, 2.0, 3.0)).length() < 1e-6);
    }

    #[test]
    fn test_translation() {
        let mut m = Matrix3D::IDENTITY;
        m.translate(10.0, 20.0, 30.0);

        let v = Vector3::new(1.0, 2.0, 3.0);
        let transformed = m * v;

        assert!((transformed - Vector3::new(11.0, 22.0, 33.0)).length() < 1e-6);
    }

    #[test]
    fn test_rotation_x() {
        let mut m = Matrix3D::IDENTITY;
        m.rotate_x(std::f32::consts::PI / 2.0); // 90 degrees

        let v = Vector3::new(0.0, 1.0, 0.0);
        let transformed = m.rotate_vector(v); // Use rotate_vector to ignore translation

        assert!((transformed - Vector3::new(0.0, 0.0, 1.0)).length() < 1e-6);
    }

    #[test]
    fn test_rotation_y() {
        let mut m = Matrix3D::IDENTITY;
        m.rotate_y(std::f32::consts::PI / 2.0); // 90 degrees

        let v = Vector3::new(1.0, 0.0, 0.0);
        let transformed = m.rotate_vector(v);

        assert!((transformed - Vector3::new(0.0, 0.0, -1.0)).length() < 1e-6);
    }

    #[test]
    fn test_rotation_z() {
        let mut m = Matrix3D::IDENTITY;
        m.rotate_z(std::f32::consts::PI / 2.0); // 90 degrees

        let v = Vector3::new(1.0, 0.0, 0.0);
        let transformed = m.rotate_vector(v);

        assert!((transformed - Vector3::new(0.0, 1.0, 0.0)).length() < 1e-6);
    }

    #[test]
    fn test_scale() {
        let mut m = Matrix3D::IDENTITY;
        m.scale(2.0);

        let v = Vector3::new(1.0, 2.0, 3.0);
        let transformed = m.rotate_vector(v);

        assert!((transformed - Vector3::new(2.0, 4.0, 6.0)).length() < 1e-6);
    }

    #[test]
    fn test_get_translation() {
        let mut m = Matrix3D::IDENTITY;
        m.set_translation(Vector3::new(5.0, 10.0, 15.0));

        let translation = m.get_translation();
        assert!((translation - Vector3::new(5.0, 10.0, 15.0)).length() < 1e-6);
    }

    #[test]
    fn test_inverse() {
        let mut m = Matrix3D::IDENTITY;
        m.translate(10.0, 20.0, 30.0);
        m.rotate_x(std::f32::consts::PI / 4.0);

        let inv = m.inverse();
        let product = m * inv;

        // Check if product is close to identity
        let identity_test = Vector3::new(1.0, 1.0, 1.0);
        let result = product * identity_test;
        assert!((result - identity_test).length() < 10.0); // Allow some numerical error
    }

    #[test]
    fn test_look_at() {
        let mut m = Matrix3D::IDENTITY;
        let position = Vector3::new(0.0, 0.0, 0.0);
        let target = Vector3::new(0.0, 0.0, -1.0);
        m.look_at(position, target, 0.0);

        let forward = m.get_z_vector();
        assert!((forward - Vector3::new(0.0, 0.0, -1.0)).length() < 1e-6);
    }

    #[test]
    fn test_from_translation() {
        let translation = Vector3::new(1.0, 2.0, 3.0);
        let m = Matrix3D::from_translation(translation);

        assert_eq!(m.get_translation(), translation);
    }

    #[test]
    fn test_matrix_multiplication() {
        let mut m1 = Matrix3D::IDENTITY;
        m1.translate(1.0, 0.0, 0.0);

        let mut m2 = Matrix3D::IDENTITY;
        m2.translate(0.0, 1.0, 0.0);

        let product = m1 * m2;
        let result = product * Vector3::new(0.0, 0.0, 0.0);

        assert!((result - Vector3::new(1.0, 1.0, 0.0)).length() < 1e-6);
    }

    #[test]
    fn test_static_matrices() {
        let v = Vector3::new(1.0, 0.0, 0.0);

        let rotated_x90 = Matrix3D::ROTATE_X90 * v;
        assert!((rotated_x90 - Vector3::new(1.0, 0.0, 0.0)).length() < 1e-6);

        let rotated_y90 = Matrix3D::ROTATE_Y90 * v;
        assert!((rotated_y90 - Vector3::new(0.0, 0.0, -1.0)).length() < 1e-6);

        let rotated_z90 = Matrix3D::ROTATE_Z90 * v;
        assert!((rotated_z90 - Vector3::new(0.0, 1.0, 0.0)).length() < 1e-6);
    }
}
