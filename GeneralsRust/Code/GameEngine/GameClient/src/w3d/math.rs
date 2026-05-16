//! W3D Mathematical Utilities

use ultraviolet::{Mat4 as UvMat4, Rotor3 as UvRotor3, Vec3 as UvVec3, Vec4 as UvVec4};

/// W3D Matrix type (compatible with original W3D format)
pub type W3DMatrix = UvMat4;

/// W3D Vector type
pub type W3DVector = UvVec3;

/// W3D Vector4 type  
pub type W3DVector4 = UvVec4;

/// W3D Quaternion type
pub type W3DQuaternion = UvRotor3;

/// W3D Transform
pub struct W3DTransform {
    pub translation: W3DVector,
    pub rotation: W3DQuaternion,
    pub scale: W3DVector,
}

impl W3DTransform {
    pub fn identity() -> Self {
        Self {
            translation: W3DVector::zero(),
            rotation: W3DQuaternion::identity(),
            scale: W3DVector::one(),
        }
    }

    pub fn to_matrix(&self) -> W3DMatrix {
        W3DMatrix::from_translation(self.translation)
            * self.rotation.into_matrix().into_homogeneous()
            * W3DMatrix::from_nonuniform_scale(self.scale)
    }
}
