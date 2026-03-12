//! Affine transform helpers built on glam::Affine3A

use glam::{Affine3A, Mat3A, Quat, Vec3, Vec3A};

/// Re-exported type alias for 3D affine transforms
pub type Affine3D = Affine3A;

/// Extension helpers for Affine3D
pub trait AffineExtensions {
    /// Build from translation, rotation (Quat), and scale
    fn from_trs(translation: Vec3, rotation: Quat, scale: Vec3) -> Affine3D;

    /// Transform a point (includes translation)
    fn transform_point(self, p: Vec3) -> Vec3;

    /// Transform a vector (ignores translation)
    fn transform_vector(self, v: Vec3) -> Vec3;

    /// Get the 3x3 linear part (rotation/scale)
    fn linear(self) -> Mat3A;

    /// X axis (first column of linear part)
    fn x_axis(self) -> Vec3;

    /// Y axis (second column of linear part)
    fn y_axis(self) -> Vec3;

    /// Z axis (third column of linear part)
    fn z_axis(self) -> Vec3;
}

impl AffineExtensions for Affine3D {
    fn from_trs(translation: Vec3, rotation: Quat, scale: Vec3) -> Affine3D {
        Affine3A::from_scale_rotation_translation(scale, rotation, translation)
    }

    fn transform_point(self, p: Vec3) -> Vec3 {
        (self.transform_point3a(Vec3A::from(p))).into()
    }

    fn transform_vector(self, v: Vec3) -> Vec3 {
        (self.transform_vector3a(Vec3A::from(v))).into()
    }

    fn linear(self) -> Mat3A {
        self.matrix3
    }

    fn x_axis(self) -> Vec3 {
        self.matrix3.x_axis.into()
    }

    fn y_axis(self) -> Vec3 {
        self.matrix3.y_axis.into()
    }

    fn z_axis(self) -> Vec3 {
        self.matrix3.z_axis.into()
    }
}
