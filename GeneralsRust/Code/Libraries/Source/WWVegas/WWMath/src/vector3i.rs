//! Integer vector helpers built on glam types.

use glam::IVec3;
use std::ops::{Index, IndexMut};

/// Primary 3D integer vector type used throughout the math library.
pub type Vector3i = IVec3;

/// Extension helpers for `Vector3i` to mirror the legacy API.
pub trait Vector3iExt {
    fn to_array(self) -> [i32; 3];
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

/// 16-bit integer vector used by WWMath serialization.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct Vector3i16 {
    pub i: u16,
    pub j: u16,
    pub k: u16,
}

impl Vector3i16 {
    pub const ZERO: Vector3i16 = Vector3i16 { i: 0, j: 0, k: 0 };

    pub fn new(i: u16, j: u16, k: u16) -> Self {
        Self { i, j, k }
    }

    pub fn from_array(arr: [u16; 3]) -> Self {
        Self {
            i: arr[0],
            j: arr[1],
            k: arr[2],
        }
    }

    pub fn to_array(self) -> [u16; 3] {
        [self.i, self.j, self.k]
    }

    pub fn to_i32(self) -> Vector3i {
        Vector3i::new(self.i as i32, self.j as i32, self.k as i32)
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

#[cfg(test)]
mod tests {
    use super::Vector3iExt;
    use super::*;

    #[test]
    fn vector3i_from_and_to_array() {
        let arr = [1, 2, 3];
        let v = Vector3i::from_array(&arr);
        assert_eq!(v, Vector3i::new(1, 2, 3));
        assert_eq!(v.to_array(), arr);
    }

    #[test]
    fn vector3i16_converts_to_i32() {
        let v16 = Vector3i16::new(1, 2, 3);
        let v = v16.to_i32();
        assert_eq!(v, Vector3i::new(1, 2, 3));
    }
}
