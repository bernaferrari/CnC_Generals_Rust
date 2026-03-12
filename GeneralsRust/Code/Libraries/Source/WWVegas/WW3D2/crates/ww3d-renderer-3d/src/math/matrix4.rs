//! 4x4 Matrix implementation

use super::vector3::Vec3;
use std::fmt;
use std::ops::{Mul, MulAssign};

/// 4x4 Matrix class
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Matrix4 {
    pub row: [Vector4; 4],
}

impl Matrix4 {
    /// Create a new matrix from rows
    pub fn new(row0: Vector4, row1: Vector4, row2: Vector4, row3: Vector4) -> Self {
        Self {
            row: [row0, row1, row2, row3],
        }
    }

    /// Create an identity matrix
    pub fn identity() -> Self {
        Self::new(
            Vector4::new(1.0, 0.0, 0.0, 0.0),
            Vector4::new(0.0, 1.0, 0.0, 0.0),
            Vector4::new(0.0, 0.0, 1.0, 0.0),
            Vector4::new(0.0, 0.0, 0.0, 1.0),
        )
    }

    /// Create a translation matrix
    pub fn translation(translation: Vec3) -> Self {
        Self::new(
            Vector4::new(1.0, 0.0, 0.0, translation.x),
            Vector4::new(0.0, 1.0, 0.0, translation.y),
            Vector4::new(0.0, 0.0, 1.0, translation.z),
            Vector4::new(0.0, 0.0, 0.0, 1.0),
        )
    }

    /// Create a scale matrix
    pub fn scale(scale: Vec3) -> Self {
        Self::new(
            Vector4::new(scale.x, 0.0, 0.0, 0.0),
            Vector4::new(0.0, scale.y, 0.0, 0.0),
            Vector4::new(0.0, 0.0, scale.z, 0.0),
            Vector4::new(0.0, 0.0, 0.0, 1.0),
        )
    }

    /// Get the translation component
    pub fn get_translation(&self) -> Vec3 {
        Vec3::new(self.row[0].w, self.row[1].w, self.row[2].w)
    }

    /// Set the translation component
    pub fn set_translation(&mut self, translation: Vec3) {
        self.row[0].w = translation.x;
        self.row[1].w = translation.y;
        self.row[2].w = translation.z;
    }

    /// Transform a vector by this matrix
    pub fn transform_vector(&self, vector: Vec3) -> Vec3 {
        let x = self.row[0].x * vector.x
            + self.row[0].y * vector.y
            + self.row[0].z * vector.z
            + self.row[0].w;
        let y = self.row[1].x * vector.x
            + self.row[1].y * vector.y
            + self.row[1].z * vector.z
            + self.row[1].w;
        let z = self.row[2].x * vector.x
            + self.row[2].y * vector.y
            + self.row[2].z * vector.z
            + self.row[2].w;
        Vec3::new(x, y, z)
    }

    /// Transform a point by this matrix
    pub fn transform_point(&self, point: Vec3) -> Vec3 {
        let x = self.row[0].x * point.x
            + self.row[0].y * point.y
            + self.row[0].z * point.z
            + self.row[0].w;
        let y = self.row[1].x * point.x
            + self.row[1].y * point.y
            + self.row[1].z * point.z
            + self.row[1].w;
        let z = self.row[2].x * point.x
            + self.row[2].y * point.y
            + self.row[2].z * point.z
            + self.row[2].w;
        let w = self.row[3].x * point.x
            + self.row[3].y * point.y
            + self.row[3].z * point.z
            + self.row[3].w;

        if w != 0.0 {
            Vec3::new(x / w, y / w, z / w)
        } else {
            Vec3::new(x, y, z)
        }
    }

    /// Transpose the matrix
    pub fn transpose(&self) -> Self {
        Self::new(
            Vector4::new(self.row[0].x, self.row[1].x, self.row[2].x, self.row[3].x),
            Vector4::new(self.row[0].y, self.row[1].y, self.row[2].y, self.row[3].y),
            Vector4::new(self.row[0].z, self.row[1].z, self.row[2].z, self.row[3].z),
            Vector4::new(self.row[0].w, self.row[1].w, self.row[2].w, self.row[3].w),
        )
    }
}

impl fmt::Display for Matrix4 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "[{}, {}, {}, {}]",
            self.row[0], self.row[1], self.row[2], self.row[3]
        )
    }
}

impl Mul for Matrix4 {
    type Output = Self;

    fn mul(self, other: Self) -> Self {
        let mut result = Matrix4::identity();

        for i in 0..4 {
            for j in 0..4 {
                let mut sum = 0.0;
                for k in 0..4 {
                    sum += self.row[i][k] * other.row[k][j];
                }
                result.row[i][j] = sum;
            }
        }

        result
    }
}

impl MulAssign for Matrix4 {
    fn mul_assign(&mut self, other: Self) {
        *self = *self * other;
    }
}

/// 4D Vector class
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vector4 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub w: f32,
}

impl Vector4 {
    /// Create a new 4D vector
    pub fn new(x: f32, y: f32, z: f32, w: f32) -> Self {
        Self { x, y, z, w }
    }

    /// Create a 3D vector with w=1
    pub fn from_vector3(v: Vec3, w: f32) -> Self {
        Self::new(v.x, v.y, v.z, w)
    }
}

impl std::ops::Index<usize> for Vector4 {
    type Output = f32;

    fn index(&self, index: usize) -> &Self::Output {
        match index {
            0 => &self.x,
            1 => &self.y,
            2 => &self.z,
            3 => &self.w,
            _ => panic!("Vector4 index out of bounds"),
        }
    }
}

impl std::ops::IndexMut<usize> for Vector4 {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        match index {
            0 => &mut self.x,
            1 => &mut self.y,
            2 => &mut self.z,
            3 => &mut self.w,
            _ => panic!("Vector4 index out of bounds"),
        }
    }
}

impl fmt::Display for Vector4 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({}, {}, {}, {})", self.x, self.y, self.z, self.w)
    }
}
