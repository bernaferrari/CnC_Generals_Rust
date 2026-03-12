//! Game-specific extensions for glam matrix types.

use crate::WWMath;
use glam::{Mat3, Mat4, Quat, Vec3, Vec4};

/// Game-specific extensions for Mat3
pub trait Mat3Extensions {
    /// Get element at row, column (0-indexed)
    fn get(&self, row: usize, col: usize) -> f32;

    /// Set element at row, column (0-indexed)
    fn set(&mut self, row: usize, col: usize, value: f32);

    /// Check if matrix is valid (no NaN or infinite values)
    fn is_valid(&self) -> bool;
}

/// Game-specific extensions for Mat4
pub trait Mat4Extensions {
    /// Check if matrix is valid (no NaN or infinite values)
    #[allow(clippy::wrong_self_convention)]
    fn is_valid(self) -> bool;

    /// Build a look-at matrix (view matrix)
    fn look_at_lh(eye: Vec3, center: Vec3, up: Vec3) -> Mat4;
    fn look_at_rh(eye: Vec3, center: Vec3, up: Vec3) -> Mat4;

    /// Build perspective projection matrices
    fn perspective_lh(fovy: f32, aspect: f32, near: f32, far: f32) -> Mat4;
    fn perspective_rh(fovy: f32, aspect: f32, near: f32, far: f32) -> Mat4;

    /// Build orthographic projection matrices
    fn orthographic_lh(left: f32, right: f32, bottom: f32, top: f32, near: f32, far: f32) -> Mat4;
    fn orthographic_rh(left: f32, right: f32, bottom: f32, top: f32, near: f32, far: f32) -> Mat4;

    /// Extract translation from transformation matrix
    fn get_translation(self) -> Vec3;

    /// Extract scale from transformation matrix
    fn get_scale(self) -> Vec3;

    /// Extract rotation quaternion from transformation matrix
    fn get_rotation(self) -> Quat;

    /// Build transformation matrix from components
    fn from_trs(translation: Vec3, rotation: Quat, scale: Vec3) -> Mat4;

    /// Apply transformation to a point (w = 1)
    fn transform_point(self, point: Vec3) -> Vec3;

    /// Apply transformation to a vector (w = 0)
    fn transform_vector(self, vector: Vec3) -> Vec3;
}

impl Mat4Extensions for Mat4 {
    /// Check if matrix is valid (no NaN or infinite values)
    #[allow(clippy::wrong_self_convention)]
    fn is_valid(self) -> bool {
        self.to_cols_array()
            .iter()
            .all(|&f| WWMath::is_valid_float(f))
    }

    /// Build a look-at matrix (view matrix) - left-handed coordinate system
    fn look_at_lh(eye: Vec3, center: Vec3, up: Vec3) -> Mat4 {
        let f = (center - eye).normalize();
        let s = f.cross(up).normalize();
        let u = s.cross(f);

        Mat4::from_cols(
            Vec4::new(s.x, u.x, -f.x, 0.0),
            Vec4::new(s.y, u.y, -f.y, 0.0),
            Vec4::new(s.z, u.z, -f.z, 0.0),
            Vec4::new(-s.dot(eye), -u.dot(eye), f.dot(eye), 1.0),
        )
    }

    /// Build a look-at matrix (view matrix) - right-handed coordinate system
    fn look_at_rh(eye: Vec3, center: Vec3, up: Vec3) -> Mat4 {
        let f = (center - eye).normalize();
        let s = f.cross(up).normalize();
        let u = s.cross(f);

        Mat4::from_cols(
            Vec4::new(s.x, u.x, f.x, 0.0),
            Vec4::new(s.y, u.y, f.y, 0.0),
            Vec4::new(s.z, u.z, f.z, 0.0),
            Vec4::new(-s.dot(eye), -u.dot(eye), -f.dot(eye), 1.0),
        )
    }

    /// Build perspective projection matrix - left-handed coordinate system
    fn perspective_lh(fovy: f32, aspect: f32, near: f32, far: f32) -> Mat4 {
        let tan_half_fovy = (fovy / 2.0).tan();

        Mat4::from_cols(
            Vec4::new(1.0 / (aspect * tan_half_fovy), 0.0, 0.0, 0.0),
            Vec4::new(0.0, 1.0 / tan_half_fovy, 0.0, 0.0),
            Vec4::new(0.0, 0.0, far / (far - near), 1.0),
            Vec4::new(0.0, 0.0, -(far * near) / (far - near), 0.0),
        )
    }

    /// Build perspective projection matrix - right-handed coordinate system
    fn perspective_rh(fovy: f32, aspect: f32, near: f32, far: f32) -> Mat4 {
        let tan_half_fovy = (fovy / 2.0).tan();

        Mat4::from_cols(
            Vec4::new(1.0 / (aspect * tan_half_fovy), 0.0, 0.0, 0.0),
            Vec4::new(0.0, 1.0 / tan_half_fovy, 0.0, 0.0),
            Vec4::new(0.0, 0.0, -(far + near) / (far - near), -1.0),
            Vec4::new(0.0, 0.0, -(2.0 * far * near) / (far - near), 0.0),
        )
    }

    /// Build orthographic projection matrix - left-handed coordinate system
    fn orthographic_lh(left: f32, right: f32, bottom: f32, top: f32, near: f32, far: f32) -> Mat4 {
        Mat4::from_cols(
            Vec4::new(2.0 / (right - left), 0.0, 0.0, 0.0),
            Vec4::new(0.0, 2.0 / (top - bottom), 0.0, 0.0),
            Vec4::new(0.0, 0.0, 1.0 / (far - near), 0.0),
            Vec4::new(
                -(right + left) / (right - left),
                -(top + bottom) / (top - bottom),
                -near / (far - near),
                1.0,
            ),
        )
    }

    /// Build orthographic projection matrix - right-handed coordinate system
    fn orthographic_rh(left: f32, right: f32, bottom: f32, top: f32, near: f32, far: f32) -> Mat4 {
        Mat4::from_cols(
            Vec4::new(2.0 / (right - left), 0.0, 0.0, 0.0),
            Vec4::new(0.0, 2.0 / (top - bottom), 0.0, 0.0),
            Vec4::new(0.0, 0.0, -2.0 / (far - near), 0.0),
            Vec4::new(
                -(right + left) / (right - left),
                -(top + bottom) / (top - bottom),
                -(far + near) / (far - near),
                1.0,
            ),
        )
    }

    /// Extract translation from transformation matrix
    fn get_translation(self) -> Vec3 {
        self.w_axis.truncate()
    }

    /// Extract scale from transformation matrix (assuming no shear)
    fn get_scale(self) -> Vec3 {
        Vec3::new(
            self.x_axis.truncate().length(),
            self.y_axis.truncate().length(),
            self.z_axis.truncate().length(),
        )
    }

    /// Extract rotation quaternion from transformation matrix
    fn get_rotation(self) -> Quat {
        // Remove scale to get pure rotation matrix
        let scale = self.get_scale();
        let rot_matrix = Mat4::from_cols(
            self.x_axis / scale.x,
            self.y_axis / scale.y,
            self.z_axis / scale.z,
            Vec4::new(0.0, 0.0, 0.0, 1.0),
        );

        Quat::from_mat4(&rot_matrix)
    }

    /// Build transformation matrix from translation, rotation, and scale
    fn from_trs(translation: Vec3, rotation: Quat, scale: Vec3) -> Mat4 {
        Mat4::from_scale_rotation_translation(scale, rotation, translation)
    }

    /// Apply transformation to a point (treats as homogeneous coordinate with w=1)
    fn transform_point(self, point: Vec3) -> Vec3 {
        let homogeneous = self * Vec4::new(point.x, point.y, point.z, 1.0);
        if homogeneous.w != 0.0 {
            homogeneous.truncate() / homogeneous.w
        } else {
            homogeneous.truncate()
        }
    }

    /// Apply transformation to a vector (treats as homogeneous coordinate with w=0)
    fn transform_vector(self, vector: Vec3) -> Vec3 {
        (self * Vec4::new(vector.x, vector.y, vector.z, 0.0)).truncate()
    }
}

impl Mat3Extensions for Mat3 {
    fn get(&self, row: usize, col: usize) -> f32 {
        match (row, col) {
            (0, 0) => self.x_axis.x,
            (0, 1) => self.y_axis.x,
            (0, 2) => self.z_axis.x,
            (1, 0) => self.x_axis.y,
            (1, 1) => self.y_axis.y,
            (1, 2) => self.z_axis.y,
            (2, 0) => self.x_axis.z,
            (2, 1) => self.y_axis.z,
            (2, 2) => self.z_axis.z,
            _ => panic!("Mat3 index out of bounds: [{}, {}]", row, col),
        }
    }

    fn set(&mut self, row: usize, col: usize, value: f32) {
        match (row, col) {
            (0, 0) => self.x_axis.x = value,
            (0, 1) => self.y_axis.x = value,
            (0, 2) => self.z_axis.x = value,
            (1, 0) => self.x_axis.y = value,
            (1, 1) => self.y_axis.y = value,
            (1, 2) => self.z_axis.y = value,
            (2, 0) => self.x_axis.z = value,
            (2, 1) => self.y_axis.z = value,
            (2, 2) => self.z_axis.z = value,
            _ => panic!("Mat3 index out of bounds: [{}, {}]", row, col),
        }
    }

    fn is_valid(&self) -> bool {
        self.to_cols_array()
            .iter()
            .all(|&f| WWMath::is_valid_float(f))
    }
}
