//! Glam-backed 2D vector alias with WWMath compatibility helpers.

use glam::Vec2;

/// Primary 2D vector type used throughout the math library.
/// This is a direct alias to `glam::Vec2` so all glam functionality is available.
pub type Vector2 = Vec2;

/// Additional WWMath-era helpers for `Vector2`.
pub trait Vector2Ext {
    /// Read the X component using its texture coordinate name.
    fn u(&self) -> f32;
    /// Read the Y component using its texture coordinate name.
    fn v(&self) -> f32;
    /// Set the U (X) component.
    fn set_u(&mut self, u: f32);
    /// Set the V (Y) component.
    fn set_v(&mut self, v: f32);
    /// Set both components in X, Y order.
    fn set(&mut self, x: f32, y: f32);
    /// Copy both components from another vector.
    fn set_from(&mut self, other: Vector2);
    /// Normalize using portable WWMath arithmetic and preserve zero length vectors in place.
    fn normalize_in_place(&mut self);
    /// Rotate counterclockwise by an angle in radians.
    fn rotate(&mut self, theta: f32);
    /// Rotate using an authored sine and cosine.
    fn rotate_with_sin_cos(&mut self, sin_theta: f32, cos_theta: f32);
    /// Rotate a unit vector towards a unit target by up to the given angle in radians.
    /// Write the original turn direction and return whether the target was reached.
    fn rotate_towards_vector(
        &mut self,
        target: &Vector2,
        max_theta: f32,
        positive_turn: &mut bool,
    ) -> bool;
    /// Rotate towards a unit target using the maximum angle's sine and cosine.
    fn rotate_towards_vector_with_sin_cos(
        &mut self,
        target: &Vector2,
        max_sin: f32,
        max_cos: f32,
        positive_turn: &mut bool,
    ) -> bool;
    /// Check that both components are finite.
    fn is_valid(&self) -> bool;
    /// Replace each component only when the corresponding value is strictly smaller.
    fn update_min(&mut self, other: &Vector2);
    /// Replace each component only when the corresponding value is strictly larger.
    fn update_max(&mut self, other: &Vector2);
    /// Multiply the components by separate scale factors.
    fn scale(&mut self, scale_x: f32, scale_y: f32);
    /// Multiply each component by the corresponding component of another vector.
    fn scale_by_vector(&mut self, scale: &Vector2);
    /// Compare the absolute component differences against a strict tolerance.
    fn equal_within_epsilon(&self, other: Vector2, epsilon: f32) -> bool;
    /// Return the original approximate distance, which operates on signed differences.
    fn quick_distance(&self, other: Vector2) -> f32;
    /// Store the original interpolation, including extrapolation outside 0..=1.
    fn lerp_into(result: &mut Vector2, a: Vector2, b: Vector2, alpha: f32);
    /// Exchange the components of two distinct vectors.
    fn swap(a: &mut Vector2, b: &mut Vector2);
}

/// The original free `Normalize` overload divides by the reciprocal length.
/// Use `Vector2Ext::normalize_in_place` for the original unit-length overload.
pub fn normalize(vector: Vector2) -> Vector2 {
    let length_squared = length_squared(vector);
    if length_squared == 0.0 {
        return Vector2::ZERO;
    }
    let reciprocal = inverse_sqrt(length_squared);
    vector * (1.0 / reciprocal)
}

fn length_squared(vector: Vector2) -> f32 {
    vector.x * vector.x + vector.y * vector.y
}

fn inverse_sqrt(value: f32) -> f32 {
    1.0 / (f64::from(value).sqrt() as f32)
}

pub(crate) fn perp_dot_product(a: Vector2, b: Vector2) -> f32 {
    a.x * -b.y + a.y * b.x
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

    fn normalize_in_place(&mut self) {
        let length_squared = length_squared(*self);
        if length_squared != 0.0 {
            *self *= inverse_sqrt(length_squared);
        }
    }

    fn rotate(&mut self, theta: f32) {
        let (sin_theta, cos_theta) = theta.sin_cos();
        self.rotate_with_sin_cos(sin_theta, cos_theta);
    }

    fn rotate_with_sin_cos(&mut self, sin_theta: f32, cos_theta: f32) {
        let new_x = self.x * cos_theta + self.y * -sin_theta;
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
        let (max_sin, max_cos) = max_theta.sin_cos();
        self.rotate_towards_vector_with_sin_cos(target, max_sin, max_cos, positive_turn)
    }

    fn rotate_towards_vector_with_sin_cos(
        &mut self,
        target: &Vector2,
        max_sin: f32,
        max_cos: f32,
        positive_turn: &mut bool,
    ) -> bool {
        *positive_turn = perp_dot_product(*target, *self) > 0.0;

        if self.dot(*target) >= max_cos {
            *self = *target;
            true
        } else {
            let sin = if *positive_turn { max_sin } else { -max_sin };
            self.rotate_with_sin_cos(sin, max_cos);
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
        self.scale(scale.x, scale.y);
    }

    fn equal_within_epsilon(&self, other: Vector2, epsilon: f32) -> bool {
        (self.x - other.x).abs() < epsilon && (self.y - other.y).abs() < epsilon
    }

    fn quick_distance(&self, other: Vector2) -> f32 {
        // The calls to Fabs in vector2.h discard their return values.
        let delta = *self - other;
        if delta.x > delta.y {
            delta.y * 0.5 + delta.x
        } else {
            delta.x * 0.5 + delta.y
        }
    }

    fn lerp_into(result: &mut Vector2, a: Vector2, b: Vector2, alpha: f32) {
        *result = a + (b - a) * alpha;
    }

    fn swap(a: &mut Vector2, b: &mut Vector2) {
        std::mem::swap(a, b);
    }
}
