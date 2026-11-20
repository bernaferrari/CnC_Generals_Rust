//! Game-specific extensions for glam Vec2 types.

use glam::Vec2;

/// Game-specific extensions for Vec2
pub trait Vec2Extensions {
    /// WWMath dot product with the clockwise perpendicular of the right-hand vector.
    fn perp_dot_product(self, rhs: Vec2) -> f32;

    /// Check if all components are valid floats
    #[allow(clippy::wrong_self_convention)]
    fn is_valid(self) -> bool;
}

impl Vec2Extensions for Vec2 {
    fn perp_dot_product(self, rhs: Vec2) -> f32 {
        perp_dot_product(self, rhs)
    }

    fn is_valid(self) -> bool {
        crate::vector2::Vector2Ext::is_valid(&self)
    }
}

/// WWMath dot product with the clockwise perpendicular of `b`.
pub fn perp_dot_product(a: Vec2, b: Vec2) -> f32 {
    crate::vector2::perp_dot_product(a, b)
}
