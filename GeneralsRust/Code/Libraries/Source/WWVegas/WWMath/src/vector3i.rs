//! The signed and unsigned three-component vectors from WWMath's `Vector3i.h`.
//!
//! Rust requires initialized values and checks component indices.

use glam::IVec3;
use std::ops::{Index, IndexMut};

/// Primary 3D integer vector type used throughout the math library.
pub type Vector3i = IVec3;

/// Extension helpers for `Vector3i` to mirror the legacy API.
pub trait Vector3iExt {
    /// Return the signed components in their original order.
    fn to_array(self) -> [i32; 3];
    /// Construct from an array in component order.
    fn from_array(arr: &[i32; 3]) -> Self;
}

impl Vector3iExt for Vector3i {
    fn to_array(self) -> [i32; 3] {
        [self.x, self.y, self.z]
    }

    fn from_array(arr: &[i32; 3]) -> Self {
        Self::new(arr[0], arr[1], arr[2])
    }
}

/// Unsigned 16-bit components in the original layout, including mesh indices.
#[repr(C)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct Vector3i16 {
    /// First component.
    pub i: u16,
    /// Second component.
    pub j: u16,
    /// Third component.
    pub k: u16,
}

impl Vector3i16 {
    /// The zero vector.
    pub const ZERO: Vector3i16 = Vector3i16 { i: 0, j: 0, k: 0 };

    /// Construct from three unsigned components.
    pub fn new(i: u16, j: u16, k: u16) -> Self {
        Self { i, j, k }
    }

    /// Construct from an array in component order.
    pub fn from_array(arr: [u16; 3]) -> Self {
        Self {
            i: arr[0],
            j: arr[1],
            k: arr[2],
        }
    }

    /// Return the unsigned components in their original order.
    pub fn to_array(self) -> [u16; 3] {
        [self.i, self.j, self.k]
    }

    /// Widen each unsigned component to the signed vector type.
    pub fn to_i32(self) -> Vector3i {
        Vector3i::new(i32::from(self.i), i32::from(self.j), i32::from(self.k))
    }
}

impl PartialEq<Vector3i> for Vector3i16 {
    /// C++ compares unsigned short components after promotion to signed int.
    fn eq(&self, other: &Vector3i) -> bool {
        self.to_i32() == *other
    }
}

impl Index<usize> for Vector3i16 {
    type Output = u16;

    fn index(&self, index: usize) -> &Self::Output {
        match index {
            0 => &self.i,
            1 => &self.j,
            2 => &self.k,
            _ => panic!("Index out of bounds for Vector3i16"),
        }
    }
}

impl IndexMut<usize> for Vector3i16 {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        match index {
            0 => &mut self.i,
            1 => &mut self.j,
            2 => &mut self.k,
            _ => panic!("Index out of bounds for Vector3i16"),
        }
    }
}
