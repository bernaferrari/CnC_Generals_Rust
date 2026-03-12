//! Coordinate extensions for locomotor and physics calculations

use super::types::{Coord2D, Coord3D, Real};

/// Extension trait for Coord3D (Vec3) operations
pub trait Coord3DExt {
    /// Calculate distance to another coordinate
    fn distance_to(&self, other: &Coord3D) -> Real;

    /// Calculate squared distance (faster, no sqrt)
    fn distance_squared_to(&self, other: &Coord3D) -> Real;

    /// Normalize the coordinate (return unit vector)
    fn normalized(&self) -> Coord3D;

    /// Get length of the vector
    fn length(&self) -> Real;

    /// Get squared length (faster, no sqrt)
    fn length_squared(&self) -> Real;
}

impl Coord3DExt for Coord3D {
    #[inline]
    fn distance_to(&self, other: &Coord3D) -> Real {
        self.distance(*other)
    }

    #[inline]
    fn distance_squared_to(&self, other: &Coord3D) -> Real {
        self.distance_squared(*other)
    }

    #[inline]
    fn normalized(&self) -> Coord3D {
        self.normalize()
    }

    #[inline]
    fn length(&self) -> Real {
        glam::Vec3::length(*self)
    }

    #[inline]
    fn length_squared(&self) -> Real {
        glam::Vec3::length_squared(*self)
    }
}

/// Extension trait for Coord2D (Vec2) operations
pub trait Coord2DExt {
    /// Calculate distance to another coordinate
    fn distance_to(&self, other: &Coord2D) -> Real;

    /// Calculate squared distance (faster, no sqrt)
    fn distance_squared_to(&self, other: &Coord2D) -> Real;

    /// Normalize the coordinate (return unit vector)
    fn normalized(&self) -> Coord2D;

    /// Get length of the vector
    fn length(&self) -> Real;

    /// Get squared length (faster, no sqrt)
    fn length_squared(&self) -> Real;
}

impl Coord2DExt for Coord2D {
    #[inline]
    fn distance_to(&self, other: &Coord2D) -> Real {
        self.distance(*other)
    }

    #[inline]
    fn distance_squared_to(&self, other: &Coord2D) -> Real {
        self.distance_squared(*other)
    }

    #[inline]
    fn normalized(&self) -> Coord2D {
        self.normalize()
    }

    #[inline]
    fn length(&self) -> Real {
        glam::Vec2::length(*self)
    }

    #[inline]
    fn length_squared(&self) -> Real {
        glam::Vec2::length_squared(*self)
    }
}
