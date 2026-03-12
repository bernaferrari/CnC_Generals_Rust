//! Geometric Utilities and Types
//!
//! Provides geometric calculations and data structures for 2D and 3D operations.

use serde::{Deserialize, Serialize};

/// 2D Point structure
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Point2D {
    pub x: f32,
    pub y: f32,
}

impl Point2D {
    pub fn new(x: f32, y: f32) -> Self {
        Point2D { x, y }
    }

    pub fn distance(&self, other: &Point2D) -> f32 {
        ((self.x - other.x).powi(2) + (self.y - other.y).powi(2)).sqrt()
    }
}

/// 3D Point structure
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Point3D {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Point3D {
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Point3D { x, y, z }
    }

    pub fn distance(&self, other: &Point3D) -> f32 {
        ((self.x - other.x).powi(2) + (self.y - other.y).powi(2) + (self.z - other.z).powi(2))
            .sqrt()
    }

    pub fn distance_to(&self, other: &Point3D) -> f32 {
        self.distance(other)
    }

    pub fn distance_to_2d(&self, other: &Point3D) -> f32 {
        ((self.x - other.x).powi(2) + (self.y - other.y).powi(2)).sqrt()
    }

    /// Check if this point is null (all components are zero)
    pub fn is_null(&self) -> bool {
        self.x == 0.0 && self.y == 0.0 && self.z == 0.0
    }

    /// Normalize this vector to unit length
    pub fn normalize(&self) -> Point3D {
        let length = (self.x * self.x + self.y * self.y + self.z * self.z).sqrt();
        if length == 0.0 {
            Point3D::new(0.0, 0.0, 0.0)
        } else {
            Point3D::new(self.x / length, self.y / length, self.z / length)
        }
    }
}

impl Default for Point3D {
    fn default() -> Self {
        Point3D::new(0.0, 0.0, 0.0)
    }
}

/// 2D Rectangle structure
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Rectangle {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rectangle {
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Rectangle {
            x,
            y,
            width,
            height,
        }
    }

    pub fn contains_point(&self, point: &Point2D) -> bool {
        point.x >= self.x
            && point.x <= self.x + self.width
            && point.y >= self.y
            && point.y <= self.y + self.height
    }
}

/// 3D Bounding Box structure
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BoundingBox {
    pub min: Point3D,
    pub max: Point3D,
}

impl BoundingBox {
    pub fn new(min: Point3D, max: Point3D) -> Self {
        BoundingBox { min, max }
    }

    pub fn contains_point(&self, point: &Point3D) -> bool {
        point.x >= self.min.x
            && point.x <= self.max.x
            && point.y >= self.min.y
            && point.y <= self.max.y
            && point.z >= self.min.z
            && point.z <= self.max.z
    }
}

/// 3D Coordinate structure (alias for Point3D for compatibility)
pub type Coord3D = Point3D;

/// 3D Matrix structure for transformations
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Matrix3D {
    pub m: [[f32; 4]; 4],
}

impl Matrix3D {
    /// Create an identity matrix
    pub fn identity() -> Self {
        let mut m = [[0.0; 4]; 4];
        m[0][0] = 1.0;
        m[1][1] = 1.0;
        m[2][2] = 1.0;
        m[3][3] = 1.0;
        Matrix3D { m }
    }

    /// Create a new matrix with given values
    pub fn new(m: [[f32; 4]; 4]) -> Self {
        Matrix3D { m }
    }

    /// Set the translation components of the matrix
    pub fn set_translation(&mut self, x: f32, y: f32, z: f32) {
        self.m[0][3] = x;
        self.m[1][3] = y;
        self.m[2][3] = z;
    }

    /// Set only the Z translation component
    pub fn set_z_translation(&mut self, z: f32) {
        self.m[2][3] = z;
    }

    /// Get X translation component
    pub fn get_x_translation(&self) -> f32 {
        self.m[0][3]
    }

    /// Get Y translation component
    pub fn get_y_translation(&self) -> f32 {
        self.m[1][3]
    }

    /// Get Z translation component
    pub fn get_z_translation(&self) -> f32 {
        self.m[2][3]
    }

    /// Get the X-axis vector from the matrix
    pub fn get_x_vector(&self) -> Coord3D {
        Coord3D {
            x: self.m[0][0],
            y: self.m[1][0],
            z: self.m[2][0],
        }
    }

    /// Get Z rotation angle from the matrix
    pub fn get_z_rotation(&self) -> f32 {
        // Extract rotation angle from matrix components
        // This is a simplified version - in reality this would be more complex
        self.m[0][0].atan2(self.m[1][0])
    }

    /// Transform a vector by this matrix
    pub fn transform_vector(&self, input: &Coord3D) -> Coord3D {
        Coord3D {
            x: self.m[0][0] * input.x
                + self.m[0][1] * input.y
                + self.m[0][2] * input.z
                + self.m[0][3],
            y: self.m[1][0] * input.x
                + self.m[1][1] * input.y
                + self.m[1][2] * input.z
                + self.m[1][3],
            z: self.m[2][0] * input.x
                + self.m[2][1] * input.y
                + self.m[2][2] * input.z
                + self.m[2][3],
        }
    }

    /// Create a matrix from a translation vector
    pub fn from_translation(v: Coord3D) -> Self {
        let mut m = Self::identity();
        m.set_translation(v.x, v.y, v.z);
        m
    }

    /// Rotate around X axis
    pub fn rotate_x(&mut self, angle: f32) {
        let s = angle.sin();
        let c = angle.cos();
        let m11 = self.m[1][1];
        let m12 = self.m[1][2];
        let m21 = self.m[2][1];
        let m22 = self.m[2][2];

        self.m[1][1] = c * m11 + s * m21;
        self.m[1][2] = c * m12 + s * m22;
        self.m[2][1] = -s * m11 + c * m21;
        self.m[2][2] = -s * m12 + c * m22;
    }

    /// Rotate around Y axis
    pub fn rotate_y(&mut self, angle: f32) {
        let s = angle.sin();
        let c = angle.cos();
        let m00 = self.m[0][0];
        let m02 = self.m[0][2];
        let m20 = self.m[2][0];
        let m22 = self.m[2][2];

        self.m[0][0] = c * m00 - s * m20;
        self.m[0][2] = c * m02 - s * m22;
        self.m[2][0] = s * m00 + c * m20;
        self.m[2][2] = s * m02 + c * m22;
    }

    /// Rotate around Z axis
    pub fn rotate_z(&mut self, angle: f32) {
        let s = angle.sin();
        let c = angle.cos();
        let m00 = self.m[0][0];
        let m01 = self.m[0][1];
        let m10 = self.m[1][0];
        let m11 = self.m[1][1];

        self.m[0][0] = c * m00 + s * m10;
        self.m[0][1] = c * m01 + s * m11;
        self.m[1][0] = -s * m00 + c * m10;
        self.m[1][1] = -s * m01 + c * m11;
    }

    /// Multiply this matrix by another matrix
    pub fn multiply(&self, other: &Matrix3D) -> Matrix3D {
        let mut result = [[0.0; 4]; 4];

        for i in 0..4 {
            for j in 0..4 {
                for k in 0..4 {
                    result[i][j] += self.m[i][k] * other.m[k][j];
                }
            }
        }

        Matrix3D::new(result)
    }

    /// Check if this matrix is null/zero
    pub fn is_null(&self) -> bool {
        for i in 0..4 {
            for j in 0..4 {
                if self.m[i][j] != 0.0 {
                    return false;
                }
            }
        }
        true
    }
}

impl Default for Matrix3D {
    fn default() -> Self {
        Self::identity()
    }
}

/// Geometry type enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GeometryType {
    Box,
    Sphere,
    Cylinder,
}

/// Geometry information structure
#[derive(Debug, Clone, PartialEq)]
pub struct GeometryInfo {
    pub geometry_type: GeometryType,
    pub is_small: bool,
    pub width: f32,
    pub height: f32,
    pub depth: f32,
}

impl GeometryInfo {
    pub fn new(
        geometry_type: GeometryType,
        is_small: bool,
        width: f32,
        height: f32,
        depth: f32,
    ) -> Self {
        GeometryInfo {
            geometry_type,
            is_small,
            width,
            height,
            depth,
        }
    }
}
