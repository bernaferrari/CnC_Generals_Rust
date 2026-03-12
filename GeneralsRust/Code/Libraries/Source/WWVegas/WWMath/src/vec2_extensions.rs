//! Game-specific extensions for glam Vec2 types.

use glam::Vec2;

/// Game-specific extensions for Vec2
pub trait Vec2Extensions {
    /// Perpendicular dot product - equivalent to perp_dot but named for C++ compatibility
    fn perp_dot_product(self, rhs: Vec2) -> f32;

    /// Check if all components are valid floats
    #[allow(clippy::wrong_self_convention)]
    fn is_valid(self) -> bool;
}

impl Vec2Extensions for Vec2 {
    fn perp_dot_product(self, rhs: Vec2) -> f32 {
        self.perp_dot(rhs)
    }

    fn is_valid(self) -> bool {
        use crate::WWMath;
        WWMath::is_valid_float(self.x) && WWMath::is_valid_float(self.y)
    }
}

/// Static perpendicular dot product function for C++ compatibility
pub fn perp_dot_product(a: Vec2, b: Vec2) -> f32 {
    a.perp_dot(b)
}
