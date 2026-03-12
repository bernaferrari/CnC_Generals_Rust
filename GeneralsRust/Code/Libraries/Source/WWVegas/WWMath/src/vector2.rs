//! Glam-backed 2D vector alias with WWMath compatibility helpers.

use glam::Vec2;

/// Primary 2D vector type used throughout the math library.
/// This is a direct alias to `glam::Vec2` so all glam functionality is available.
pub type Vector2 = Vec2;

/// Additional WWMath-era helpers for `Vector2`.
pub trait Vector2Ext {
    fn u(&self) -> f32;
    fn v(&self) -> f32;
    fn set_u(&mut self, u: f32);
    fn set_v(&mut self, v: f32);
    fn set(&mut self, x: f32, y: f32);
    fn set_from(&mut self, other: Vector2);
    fn rotate(&mut self, theta: f32);
    fn rotate_with_sin_cos(&mut self, sin_theta: f32, cos_theta: f32);
    fn rotate_towards_vector(
        &mut self,
        target: &Vector2,
        max_theta: f32,
        positive_turn: &mut bool,
    ) -> bool;
    fn rotate_towards_vector_with_sin_cos(
        &mut self,
        target: &Vector2,
        max_sin: f32,
        max_cos: f32,
        positive_turn: &mut bool,
    ) -> bool;
    fn is_valid(&self) -> bool;
    fn update_min(&mut self, other: &Vector2);
    fn update_max(&mut self, other: &Vector2);
    fn scale(&mut self, scale_x: f32, scale_y: f32);
    fn scale_by_vector(&mut self, scale: &Vector2);
}

impl Vector2Ext for Vector2 {
    fn u(&self) -> f32 {
        self.x
    }

    fn v(&self) -> f32 {
        self.y
    }

    fn set_u(&mut self, u: f32) {
        self.x = u;
    }

    fn set_v(&mut self, v: f32) {
        self.y = v;
    }

    fn set(&mut self, x: f32, y: f32) {
        *self = Vector2::new(x, y);
    }

    fn set_from(&mut self, other: Vector2) {
        *self = other;
    }

    fn rotate(&mut self, theta: f32) {
        self.rotate_with_sin_cos(theta.sin(), theta.cos());
    }

    fn rotate_with_sin_cos(&mut self, sin_theta: f32, cos_theta: f32) {
        let new_x = self.x * cos_theta - self.y * sin_theta;
        let new_y = self.x * sin_theta + self.y * cos_theta;
        self.x = new_x;
        self.y = new_y;
    }

    fn rotate_towards_vector(
        &mut self,
        target: &Vector2,
        max_theta: f32,
        positive_turn: &mut bool,
    ) -> bool {
        let max_sin = max_theta.sin();
        let max_cos = max_theta.cos();
        self.rotate_towards_vector_with_sin_cos(target, max_sin, max_cos, positive_turn)
    }

    fn rotate_towards_vector_with_sin_cos(
        &mut self,
        target: &Vector2,
        max_sin: f32,
        max_cos: f32,
        positive_turn: &mut bool,
    ) -> bool {
        *positive_turn = Vector2::perp_dot(*target, *self) > 0.0;

        if self.dot(*target) >= max_cos {
            *self = *target;
            true
        } else {
            if *positive_turn {
                self.rotate_with_sin_cos(max_sin, max_cos);
            } else {
                self.rotate_with_sin_cos(-max_sin, max_cos);
            }
            false
        }
    }

    fn is_valid(&self) -> bool {
        crate::WWMath::is_valid_float(self.x) && crate::WWMath::is_valid_float(self.y)
    }

    fn update_min(&mut self, other: &Vector2) {
        if other.x < self.x {
            self.x = other.x;
        }
        if other.y < self.y {
            self.y = other.y;
        }
    }

    fn update_max(&mut self, other: &Vector2) {
        if other.x > self.x {
            self.x = other.x;
        }
        if other.y > self.y {
            self.y = other.y;
        }
    }

    fn scale(&mut self, scale_x: f32, scale_y: f32) {
        self.x *= scale_x;
        self.y *= scale_y;
    }

    fn scale_by_vector(&mut self, scale: &Vector2) {
        self.x *= scale.x;
        self.y *= scale.y;
    }
}

#[cfg(test)]
mod tests {
    use super::Vector2Ext;
    use super::*;

    #[test]
    fn perp_dot_matches_expected() {
        let a = Vector2::new(1.0, 0.0);
        let b = Vector2::new(0.0, 1.0);
        assert_eq!(Vector2::perp_dot(a, b), 1.0);
    }

    #[test]
    fn rotate_towards_vector_moves_towards_target() {
        let mut v = Vector2::new(1.0, 0.0);
        let target = Vector2::new(0.0, 1.0);
        let mut positive = false;
        let rotated = v.rotate_towards_vector(&target, std::f32::consts::FRAC_PI_2, &mut positive);
        assert!(rotated);
        assert!(positive);
        assert!((v.x.abs()) < 1e-6);
        assert!((v.y - 1.0).abs() < 1e-6);
    }
}
