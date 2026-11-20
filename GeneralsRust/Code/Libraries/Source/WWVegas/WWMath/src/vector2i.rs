//! Signed two-component integer vectors from WWMath's `vector2i.h`.
//!
//! Glam supplies the same two indexed components and initialized values.
//! Rust checks index bounds and requires distinct mutable vectors for a swap.

use glam::IVec2;

/// Primary 2D integer vector type used throughout the math library.
pub type Vector2i = IVec2;

/// Additional WWMath-era helpers for `Vector2i`.
pub trait Vector2iExt {
    /// Replace both components in C++ `Set` order.
    fn set(&mut self, i: i32, j: i32);
    /// Exchange both components with a different vector.
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
