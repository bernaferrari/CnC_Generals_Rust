//! Glam-backed 2D integer vector alias with WWMath compatibility helpers.

use glam::IVec2;

/// Primary 2D integer vector type used throughout the math library.
pub type Vector2i = IVec2;

/// Additional WWMath-era helpers for `Vector2i`.
pub trait Vector2iExt {
    fn set(&mut self, i: i32, j: i32);
    fn swap(&mut self, other: &mut Vector2i);
}

impl Vector2iExt for Vector2i {
    fn set(&mut self, i: i32, j: i32) {
        *self = Vector2i::new(i, j);
    }

    fn swap(&mut self, other: &mut Vector2i) {
        std::mem::swap(self, other);
    }
}

#[cfg(test)]
mod tests {
    use super::Vector2iExt;
    use super::*;

    #[test]
    fn set_and_swap_work() {
        let mut a = Vector2i::new(1, 2);
        let mut b = Vector2i::new(3, 4);
        a.set(5, 6);
        assert_eq!(a, Vector2i::new(5, 6));
        a.swap(&mut b);
        assert_eq!(a, Vector2i::new(3, 4));
        assert_eq!(b, Vector2i::new(5, 6));
    }
}
