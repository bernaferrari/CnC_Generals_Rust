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

    /// C++ `Coord3D::lengthSqr`.
    fn length_sqr(&self) -> Real {
        self.length_squared()
    }

    /// C++ `Coord3D::normalize` — in-place; zero stays zero.
    fn normalize_in_place(&mut self);

    /// 2D (x,y) distance matching C++ `Coord3D` ground-plane length.
    fn distance_2d(&self, other: &Coord3D) -> Real;
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
        // C++ `Coord3D::normalize` leaves a zero vector as zero. glam's
        // `Vec3::normalize` yields NaNs.
        let len = glam::Vec3::length(*self);
        if len == 0.0 {
            Coord3D::ZERO
        } else {
            *self / len
        }
    }

    #[inline]
    fn length(&self) -> Real {
        glam::Vec3::length(*self)
    }

    #[inline]
    fn length_squared(&self) -> Real {
        glam::Vec3::length_squared(*self)
    }

    fn normalize_in_place(&mut self) {
        *self = self.normalized();
    }

    fn distance_2d(&self, other: &Coord3D) -> Real {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        (dx * dx + dy * dy).sqrt()
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

    /// C++ `Coord2D::normalize` — in-place; zero stays zero.
    fn normalize_in_place(&mut self);

    /// C++ `Coord2D::toAngle`.
    fn to_angle(&self) -> Real;
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
        let len = glam::Vec2::length(*self);
        if len == 0.0 {
            Coord2D::ZERO
        } else {
            *self / len
        }
    }

    #[inline]
    fn length(&self) -> Real {
        glam::Vec2::length(*self)
    }

    #[inline]
    fn length_squared(&self) -> Real {
        glam::Vec2::length_squared(*self)
    }

    fn normalize_in_place(&mut self) {
        *self = self.normalized();
    }

    fn to_angle(&self) -> Real {
        game_engine::common::system::geometry::coord2d_to_angle(self.x, self.y)
    }
}
