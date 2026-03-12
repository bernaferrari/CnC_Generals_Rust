//! Vector extension methods
//!
//! Provides additional utility methods for glam types to match
//! C++ vector math API.

use crate::common::Real;
use glam::Vec3;

/// Extension trait for Point3 (represented as Vec3) to provide C++ style length methods
pub trait Point3Ext {
    /// Get the squared length/magnitude of the vector from origin to this point
    fn length_sqr(&self) -> Real;

    /// Get the length/magnitude of the vector from origin to this point  
    fn length(&self) -> Real;

    /// Get the squared distance to another point
    fn distance_sqr(&self, other: &Vec3) -> Real;
}

impl Point3Ext for Vec3 {
    fn length_sqr(&self) -> Real {
        Vec3::length_squared(*self)
    }

    fn length(&self) -> Real {
        Vec3::length(*self)
    }

    fn distance_sqr(&self, other: &Vec3) -> Real {
        (*self - *other).length_squared()
    }
}

/// Extension trait for Vector3 (Vec3) to provide C++ style length methods
pub trait Vector3Ext {
    /// Get the squared length/magnitude of the vector
    fn length_sqr(&self) -> Real;

    /// Get the length/magnitude of the vector
    fn length(&self) -> Real;
}

impl Vector3Ext for Vec3 {
    fn length_sqr(&self) -> Real {
        Vec3::length_squared(*self)
    }

    fn length(&self) -> Real {
        Vec3::length(*self)
    }
}
