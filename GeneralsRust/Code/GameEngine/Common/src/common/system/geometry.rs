//! Geometric Utilities and Types
//!
//! C++ Reference: /GeneralsMD/Code/GameEngine/Source/Common/System/Geometry.cpp
//! C++ Header:   /GeneralsMD/Code/GameEngine/Include/Common/Geometry.h

use crate::common::system::{Snapshotable, Xfer, XferVersion};
use serde::{Deserialize, Serialize};

/// 2D Point structure (`Coord2D` in C++ `BaseType.h`).
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Point2D {
    pub x: f32,
    pub y: f32,
}

impl Point2D {
    pub const ZERO: Point2D = Point2D { x: 0.0, y: 0.0 };

    pub fn new(x: f32, y: f32) -> Self {
        Point2D { x, y }
    }

    pub fn zero() -> Self {
        Self::ZERO
    }

    pub fn distance(&self, other: &Point2D) -> f32 {
        ((self.x - other.x).powi(2) + (self.y - other.y).powi(2)).sqrt()
    }

    /// C++ `Coord2D::length`.
    pub fn length(&self) -> f32 {
        (self.x * self.x + self.y * self.y).sqrt()
    }

    /// C++ `Coord2D::normalize` — in-place; a zero vector stays zero.
    pub fn normalize_in_place(&mut self) {
        let len = self.length();
        if len != 0.0 {
            self.x /= len;
            self.y /= len;
        }
    }

    /// Returning copy of C++ `Coord2D::normalize`.
    pub fn normalize(&self) -> Point2D {
        let mut copy = *self;
        copy.normalize_in_place();
        copy
    }

    /// C++ `Coord2D::toAngle` — angle 0 is +X, negative Y is clockwise.
    pub fn to_angle(&self) -> f32 {
        coord2d_to_angle(self.x, self.y)
    }
}

/// C++ `Coord2D::toAngle`. Shared so glam `Vec2` ports can match exactly.
pub fn coord2d_to_angle(x: f32, y: f32) -> f32 {
    let len = (x * x + y * y).sqrt();
    if len == 0.0 {
        return 0.0;
    }
    let mut c = x / len;
    if c < -1.0 {
        c = -1.0;
    } else if c > 1.0 {
        c = 1.0;
    }
    if y < 0.0 {
        -c.acos()
    } else {
        c.acos()
    }
}

/// Axis-aligned 2D region (`lo`/`hi`), matching C++ `Region2D`.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct GeometryRegion2D {
    pub lo: Point2D,
    pub hi: Point2D,
}

impl GeometryRegion2D {
    pub fn new(lo: Point2D, hi: Point2D) -> Self {
        Self { lo, hi }
    }

    /// C++ `Region2D::width`.
    pub fn width(&self) -> f32 {
        self.hi.x - self.lo.x
    }

    /// C++ `Region2D::height`.
    pub fn height(&self) -> f32 {
        self.hi.y - self.lo.y
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
    pub const ZERO: Point3D = Point3D {
        x: 0.0,
        y: 0.0,
        z: 0.0,
    };

    pub const ONE: Point3D = Point3D {
        x: 1.0,
        y: 1.0,
        z: 1.0,
    };

    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Point3D { x, y, z }
    }

    pub fn zero() -> Self {
        Self::ZERO
    }

    pub fn origin() -> Self {
        Self::ZERO
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

    /// C++ `Coord3D::length`.
    pub fn length(&self) -> f32 {
        (self.x * self.x + self.y * self.y + self.z * self.z).sqrt()
    }

    /// C++ `Coord3D::lengthSqr`.
    pub fn length_sqr(&self) -> f32 {
        self.x * self.x + self.y * self.y + self.z * self.z
    }

    /// C++ `Coord3D::normalize` — in-place; a zero vector stays zero.
    pub fn normalize_in_place(&mut self) {
        let len = self.length();
        if len != 0.0 {
            self.x /= len;
            self.y /= len;
            self.z /= len;
        }
    }

    /// C++ `Coord3D::crossProduct`.
    pub fn cross_product(a: &Point3D, b: &Point3D) -> Point3D {
        Point3D::new(
            a.y * b.z - a.z * b.y,
            a.z * b.x - a.x * b.z,
            a.x * b.y - a.y * b.x,
        )
    }

    /// C++ `Coord3D::zero`.
    pub fn zero_in_place(&mut self) {
        self.x = 0.0;
        self.y = 0.0;
        self.z = 0.0;
    }

    /// C++ `Coord3D::add`.
    pub fn add(&mut self, a: &Point3D) {
        self.x += a.x;
        self.y += a.y;
        self.z += a.z;
    }

    /// C++ `Coord3D::add` (legacy name).
    pub fn add_coord(&mut self, a: &Point3D) {
        self.add(a);
    }

    /// C++ `Coord3D::sub`.
    pub fn sub(&mut self, a: &Point3D) {
        self.x -= a.x;
        self.y -= a.y;
        self.z -= a.z;
    }

    /// C++ `Coord3D::sub` (legacy name).
    pub fn sub_coord(&mut self, a: &Point3D) {
        self.sub(a);
    }

    /// C++ `Coord3D::set(const Coord3D *)`.
    pub fn set(&mut self, a: &Point3D) {
        self.x = a.x;
        self.y = a.y;
        self.z = a.z;
    }

    /// C++ `Coord3D::set` (legacy name).
    pub fn set_from(&mut self, a: &Point3D) {
        self.set(a);
    }

    /// C++ `Coord3D::set(Real, Real, Real)`.
    pub fn set_xyz(&mut self, x: f32, y: f32, z: f32) {
        self.x = x;
        self.y = y;
        self.z = z;
    }

    /// C++ `Coord3D::scale`.
    pub fn scale(&mut self, scale: f32) {
        self.x *= scale;
        self.y *= scale;
        self.z *= scale;
    }

    /// C++ `Coord3D::equals`.
    pub fn equals(&self, r: &Point3D) -> bool {
        self.x == r.x && self.y == r.y && self.z == r.z
    }
}

/// C++ leftover / GameLogic `Coord3D` is Z-up: `(x, y_ground, z_height)`.
/// Host/render is Y-up: `(x, height, z_ground)`.
pub fn host_yup_to_cpp_zup(x: f32, y_height: f32, z_ground: f32) -> Coord3D {
    Coord3D::new(x, z_ground, y_height)
}

/// Inverse of [`host_yup_to_cpp_zup`].
pub fn cpp_zup_to_host_yup(coord: Coord3D) -> (f32, f32, f32) {
    (coord.x, coord.z, coord.y)
}

impl Default for Point3D {
    fn default() -> Self {
        Point3D::ZERO
    }
}

impl From<(f32, f32, f32)> for Point3D {
    fn from(tuple: (f32, f32, f32)) -> Self {
        Point3D::new(tuple.0, tuple.1, tuple.2)
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

/// C++ `Coord2D`.
pub type Coord2D = Point2D;

/// C++ `Region2D`.
pub type Region2D = GeometryRegion2D;

/// C++ `ICoord2D`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ICoord2D {
    pub x: i32,
    pub y: i32,
}

impl ICoord2D {
    pub const ZERO: ICoord2D = ICoord2D { x: 0, y: 0 };

    pub fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }

    pub fn zero() -> Self {
        Self::ZERO
    }

    /// C++ `ICoord2D::length` — `(Int)sqrt((double)(x*x + y*y))`.
    pub fn length(&self) -> i32 {
        let sum = self
            .x
            .wrapping_mul(self.x)
            .wrapping_add(self.y.wrapping_mul(self.y));
        (sum as f64).sqrt() as i32
    }
}

/// C++ `ICoord3D`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ICoord3D {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl ICoord3D {
    pub const ZERO: ICoord3D = ICoord3D { x: 0, y: 0, z: 0 };

    pub fn new(x: i32, y: i32, z: i32) -> Self {
        Self { x, y, z }
    }

    pub fn zero() -> Self {
        Self::ZERO
    }

    /// C++ `ICoord3D::length` — `(Int)sqrt((double)(x*x + y*y + z*z))`.
    pub fn length(&self) -> i32 {
        let sum = self
            .x
            .wrapping_mul(self.x)
            .wrapping_add(self.y.wrapping_mul(self.y))
            .wrapping_add(self.z.wrapping_mul(self.z));
        (sum as f64).sqrt() as i32
    }

    /// C++ `ICoord3D::zero`.
    pub fn zero_in_place(&mut self) {
        self.x = 0;
        self.y = 0;
        self.z = 0;
    }
}

/// C++ `Region3D` — exclusive interior tests (`lo < q < hi`).
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Region3D {
    pub lo: Coord3D,
    pub hi: Coord3D,
}

impl Region3D {
    pub fn new(lo: Coord3D, hi: Coord3D) -> Self {
        Self { lo, hi }
    }

    /// C++ `Region3D::width`.
    pub fn width(&self) -> f32 {
        self.hi.x - self.lo.x
    }

    /// C++ `Region3D::height`.
    pub fn height(&self) -> f32 {
        self.hi.y - self.lo.y
    }

    /// C++ `Region3D::depth`.
    pub fn depth(&self) -> f32 {
        self.hi.z - self.lo.z
    }

    /// C++ `Region3D::zero`.
    pub fn zero(&mut self) {
        self.lo.zero_in_place();
        self.hi.zero_in_place();
    }

    pub fn zeroed() -> Self {
        Self {
            lo: Coord3D::ZERO,
            hi: Coord3D::ZERO,
        }
    }

    /// `hi - lo` convenience (not a C++ method).
    pub fn get_size(&self) -> Coord3D {
        Coord3D::new(self.width(), self.height(), self.depth())
    }

    /// C++ `Region3D::isInRegionNoZ` — strict inequalities.
    pub fn is_in_region_no_z(&self, query: &Coord3D) -> bool {
        self.lo.x < query.x
            && query.x < self.hi.x
            && self.lo.y < query.y
            && query.y < self.hi.y
    }

    /// C++ `Region3D::isInRegionWithZ` — strict inequalities.
    pub fn is_in_region_with_z(&self, query: &Coord3D) -> bool {
        self.is_in_region_no_z(query) && self.lo.z < query.z && query.z < self.hi.z
    }
}

/// C++ `IRegion2D`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct IRegion2D {
    pub lo: ICoord2D,
    pub hi: ICoord2D,
}

impl IRegion2D {
    pub fn new(lo: ICoord2D, hi: ICoord2D) -> Self {
        Self { lo, hi }
    }

    pub fn width(&self) -> i32 {
        self.hi.x - self.lo.x
    }

    pub fn height(&self) -> i32 {
        self.hi.y - self.lo.y
    }
}

/// C++ `IRegion3D`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct IRegion3D {
    pub lo: ICoord3D,
    pub hi: ICoord3D,
}

impl IRegion3D {
    pub fn new(lo: ICoord3D, hi: ICoord3D) -> Self {
        Self { lo, hi }
    }

    pub fn width(&self) -> i32 {
        self.hi.x - self.lo.x
    }

    pub fn height(&self) -> i32 {
        self.hi.y - self.lo.y
    }

    pub fn depth(&self) -> i32 {
        self.hi.z - self.lo.z
    }
}

/// C++ `RGBColor` (`red`/`green`/`blue` in 0..1).
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct RGBColor {
    pub red: f32,
    pub green: f32,
    pub blue: f32,
}

impl RGBColor {
    pub fn new(red: f32, green: f32, blue: f32) -> Self {
        Self { red, green, blue }
    }

    /// C++ `RGBColor::getAsInt`.
    pub fn get_as_int(&self) -> i32 {
        ((self.red * 255.0) as i32) << 16
            | ((self.green * 255.0) as i32) << 8
            | ((self.blue * 255.0) as i32)
    }

    /// C++ `RGBColor::setFromInt`.
    pub fn set_from_int(&mut self, c: i32) {
        self.red = ((c >> 16) & 0xff) as f32 / 255.0;
        self.green = ((c >> 8) & 0xff) as f32 / 255.0;
        self.blue = (c & 0xff) as f32 / 255.0;
    }
}

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

/// Geometry type enumeration - matches C++ Geometry.h lines 25-33
/// GEOMETRY_SPHERE=0, GEOMETRY_CYLINDER=1, GEOMETRY_BOX=2
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum GeometryType {
    Sphere = 0,
    Cylinder = 1,
    Box = 2,
}

/// Geometry information structure
/// C++ Reference: Geometry.h - mirrors m_type, m_isSmall, m_height, m_majorRadius,
///   m_minorRadius, m_boundingCircleRadius, m_boundingSphereRadius
/// Note: `width` and `depth` are legacy aliases for `major_radius` and `minor_radius`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GeometryInfo {
    pub geometry_type: GeometryType,
    pub is_small: bool,
    pub height: f32,
    /// C++ m_majorRadius. Legacy code refers to this as `width` (half-extent in forward dir).
    pub width: f32,
    /// C++ m_minorRadius. Legacy code refers to this as `depth` (half-extent in side dir).
    pub depth: f32,
    pub bounding_circle_radius: f32,
    pub bounding_sphere_radius: f32,
}

impl GeometryInfo {
    pub fn new(
        geometry_type: GeometryType,
        is_small: bool,
        width: f32,
        height: f32,
        depth: f32,
    ) -> Self {
        let mut info = GeometryInfo {
            geometry_type,
            is_small,
            height,
            width,
            depth,
            bounding_circle_radius: 0.0,
            bounding_sphere_radius: 0.0,
        };
        // C++ `GeometryInfo::set` copies sphere/cylinder radii before bounds.
        match geometry_type {
            GeometryType::Sphere => {
                info.depth = width;
                info.height = width;
            }
            GeometryType::Cylinder => {
                info.depth = width;
            }
            GeometryType::Box => {}
        }
        info.calc_bounding_stuff();
        info
    }

    /// C++ `GeometryInfo::set`.
    pub fn set(
        &mut self,
        geometry_type: GeometryType,
        is_small: bool,
        height: f32,
        major_radius: f32,
        minor_radius: f32,
    ) {
        self.geometry_type = geometry_type;
        self.is_small = is_small;
        match geometry_type {
            GeometryType::Sphere => {
                self.width = major_radius;
                self.depth = major_radius;
                self.height = major_radius;
            }
            GeometryType::Cylinder => {
                self.width = major_radius;
                self.depth = major_radius;
                self.height = height;
            }
            GeometryType::Box => {
                self.width = major_radius;
                self.depth = minor_radius;
                self.height = height;
            }
        }
        self.calc_bounding_stuff();
    }

    pub fn major_radius(&self) -> f32 {
        self.width
    }

    pub fn minor_radius(&self) -> f32 {
        self.depth
    }

    /// C++ `GeometryInfo::setMajorRadius` — writes only `m_majorRadius`.
    pub fn set_major_radius(&mut self, r: f32) {
        self.width = r;
        self.calc_bounding_stuff();
    }

    /// C++ `GeometryInfo::setMinorRadius` — writes only `m_minorRadius`.
    pub fn set_minor_radius(&mut self, r: f32) {
        self.depth = r;
        self.calc_bounding_stuff();
    }

    /// C++ `GeometryInfo::calcBoundingStuff`.
    pub fn calc_bounding_stuff(&mut self) {
        match self.geometry_type {
            GeometryType::Sphere => {
                self.bounding_sphere_radius = self.width;
                self.bounding_circle_radius = self.width;
            }
            GeometryType::Cylinder => {
                self.bounding_circle_radius = self.width;
                let half_h = self.height * 0.5;
                self.bounding_sphere_radius = if half_h < self.width {
                    self.width
                } else {
                    half_h
                };
            }
            GeometryType::Box => {
                self.bounding_circle_radius =
                    (self.width * self.width + self.depth * self.depth).sqrt();
                let half_h = self.height * 0.5;
                self.bounding_sphere_radius =
                    (self.width * self.width + self.depth * self.depth + half_h * half_h).sqrt();
            }
        }
    }

    pub fn bounding_sphere_radius(&self) -> f32 {
        self.bounding_sphere_radius
    }

    pub fn bounding_circle_radius(&self) -> f32 {
        self.bounding_circle_radius
    }

    /// C++ `GeometryInfo::isIntersectedByLineSegment` (sphere approximation).
    pub fn is_intersected_by_line_segment(
        &self,
        loc: &Coord3D,
        from: &Coord3D,
        to: &Coord3D,
    ) -> bool {
        let dist_sq = point_to_line_dist_squared(loc, from, to);
        let r = self.bounding_sphere_radius;
        dist_sq <= r * r
    }

    /// C++ `GeometryInfo::calcPitches`.
    pub fn calc_pitches(
        &self,
        this_pos: &Coord3D,
        that: &GeometryInfo,
        that_pos: &Coord3D,
    ) -> (f32, f32) {
        let this_center = self.get_center_position(this_pos);
        let dx = that_pos.x - this_center.x;
        let dy = that_pos.y - this_center.y;
        let dxy = (dx * dx + dy * dy).sqrt();
        let max_dz = (that_pos.z + that.get_max_height_above_position()) - this_center.z;
        let min_dz = (that_pos.z - that.get_max_height_below_position()) - this_center.z;
        (min_dz.atan2(dxy), max_dz.atan2(dxy))
    }

    /// C++ `GeometryInfo::setMaxHeightAbovePosition`.
    pub fn set_max_height_above_position(&mut self, z: f32) {
        match self.geometry_type {
            GeometryType::Sphere => self.width = z,
            GeometryType::Box | GeometryType::Cylinder => self.height = z,
        }
        self.calc_bounding_stuff();
    }

    /// C++ `GeometryInfo::getMaxHeightAbovePosition`.
    pub fn get_max_height_above_position(&self) -> f32 {
        match self.geometry_type {
            GeometryType::Sphere => self.width,
            GeometryType::Box | GeometryType::Cylinder => self.height,
        }
    }

    /// C++ `GeometryInfo::getMaxHeightBelowPosition`.
    pub fn get_max_height_below_position(&self) -> f32 {
        match self.geometry_type {
            GeometryType::Sphere => self.width,
            GeometryType::Box | GeometryType::Cylinder => 0.0,
        }
    }

    /// C++ `GeometryInfo::getZDeltaToCenterPosition`.
    pub fn get_z_delta_to_center_position(&self) -> f32 {
        if self.geometry_type == GeometryType::Sphere {
            0.0
        } else {
            self.height * 0.5
        }
    }

    /// C++ `GeometryInfo::getCenterPosition`.
    pub fn get_center_position(&self, pos: &Coord3D) -> Coord3D {
        Coord3D {
            x: pos.x,
            y: pos.y,
            z: pos.z + self.get_z_delta_to_center_position(),
        }
    }

    /// C++ `GeometryInfo::expandFootprint`.
    pub fn expand_footprint(&mut self, radius: f32) {
        self.width += radius;
        self.depth += radius;
        self.calc_bounding_stuff();
    }

    /// C++ `GeometryInfo::get2DBounds`.
    pub fn get_2d_bounds(&self, geom_center: &Coord3D, angle: f32) -> GeometryRegion2D {
        match self.geometry_type {
            GeometryType::Sphere | GeometryType::Cylinder => GeometryRegion2D {
                lo: Point2D::new(geom_center.x - self.width, geom_center.y - self.width),
                hi: Point2D::new(geom_center.x + self.width, geom_center.y + self.width),
            },
            GeometryType::Box => {
                let c = angle.cos();
                let s = angle.sin();
                let exc = self.width * c;
                let eyc = self.depth * c;
                let exs = self.width * s;
                let eys = self.depth * s;
                let corners = [
                    (geom_center.x - exc - eys, geom_center.y + eyc - exs),
                    (geom_center.x + exc - eys, geom_center.y + eyc + exs),
                    (geom_center.x + exc + eys, geom_center.y - eyc + exs),
                    (geom_center.x - exc + eys, geom_center.y - eyc - exs),
                ];
                let mut lo_x = corners[0].0;
                let mut lo_y = corners[0].1;
                let mut hi_x = corners[0].0;
                let mut hi_y = corners[0].1;
                for &(x, y) in &corners[1..] {
                    if lo_x > x {
                        lo_x = x;
                    }
                    if lo_y > y {
                        lo_y = y;
                    }
                    if hi_x < x {
                        hi_x = x;
                    }
                    if hi_y < y {
                        hi_y = y;
                    }
                }
                GeometryRegion2D {
                    lo: Point2D::new(lo_x, lo_y),
                    hi: Point2D::new(hi_x, hi_y),
                }
            }
        }
    }

    /// C++ `GeometryInfo::clipPointToFootprint`.
    pub fn clip_point_to_footprint(&self, geom_center: &Coord3D, pt: &mut Coord3D) {
        match self.geometry_type {
            GeometryType::Sphere | GeometryType::Cylinder => {
                let dx = pt.x - geom_center.x;
                let dy = pt.y - geom_center.y;
                let radius = (dx * dx + dy * dy).sqrt();
                if radius > self.width {
                    let ratio = self.width / radius;
                    pt.x = geom_center.x + dx * ratio;
                    pt.y = geom_center.y + dy * ratio;
                }
            }
            GeometryType::Box => {
                pt.x =
                    pt.x.clamp(geom_center.x - self.width, geom_center.x + self.width);
                pt.y =
                    pt.y.clamp(geom_center.y - self.depth, geom_center.y + self.depth);
            }
        }
    }

    /// C++ `GeometryInfo::isPointInFootprint`.
    pub fn is_point_in_footprint(&self, geom_center: &Coord3D, pt: &Coord3D) -> bool {
        match self.geometry_type {
            GeometryType::Sphere | GeometryType::Cylinder => {
                let dx = pt.x - geom_center.x;
                let dy = pt.y - geom_center.y;
                (dx * dx + dy * dy).sqrt() <= self.width
            }
            GeometryType::Box => {
                is_within(geom_center.x - self.width, pt.x, geom_center.x + self.width)
                    && is_within(geom_center.y - self.depth, pt.y, geom_center.y + self.depth)
            }
        }
    }

    /// C++ `GeometryInfo::makeRandomOffsetWithinFootprint`.
    pub fn make_random_offset_within_footprint(&self) -> Coord3D {
        match self.geometry_type {
            GeometryType::Sphere | GeometryType::Cylinder => {
                let max_dist_sq = self.width * self.width;
                loop {
                    let x = crate::common::random_value::get_game_logic_random_value_real(
                        -self.width,
                        self.width,
                    );
                    let y = crate::common::random_value::get_game_logic_random_value_real(
                        -self.width,
                        self.width,
                    );
                    if x * x + y * y <= max_dist_sq {
                        return Coord3D { x, y, z: 0.0 };
                    }
                }
            }
            GeometryType::Box => Coord3D {
                x: crate::common::random_value::get_game_logic_random_value_real(
                    -self.width,
                    self.width,
                ),
                y: crate::common::random_value::get_game_logic_random_value_real(
                    -self.depth,
                    self.depth,
                ),
                z: 0.0,
            },
        }
    }

    /// C++ `GeometryInfo::makeRandomOffsetOnPerimeter`.
    pub fn make_random_offset_on_perimeter(&self) -> Coord3D {
        match self.geometry_type {
            GeometryType::Sphere | GeometryType::Cylinder => Coord3D {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            GeometryType::Box => {
                let mut pt = Coord3D {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                };
                if crate::common::random_value::get_game_logic_random_value_real(0.0, 1.0) < 0.5 {
                    pt.x = crate::common::random_value::get_game_logic_random_value_real(
                        -self.width,
                        self.width,
                    );
                    pt.y =
                        if crate::common::random_value::get_game_logic_random_value_real(0.0, 1.0)
                            < 0.5
                        {
                            -self.depth
                        } else {
                            self.depth
                        };
                } else {
                    pt.y = crate::common::random_value::get_game_logic_random_value_real(
                        -self.depth,
                        self.depth,
                    );
                    pt.x =
                        if crate::common::random_value::get_game_logic_random_value_real(0.0, 1.0)
                            < 0.5
                        {
                            -self.width
                        } else {
                            self.width
                        };
                }
                pt
            }
        }
    }

    /// C++ `GeometryInfo::getFootprintArea`.
    pub fn get_footprint_area(&self) -> f32 {
        match self.geometry_type {
            GeometryType::Sphere | GeometryType::Cylinder => {
                std::f32::consts::PI * self.bounding_circle_radius * self.bounding_circle_radius
            }
            GeometryType::Box => 4.0 * self.width * self.depth,
        }
    }
}

fn is_within(a: f32, b: f32, c: f32) -> bool {
    a <= b && b <= c
}

fn calc_dot(a: &Coord3D, b: &Coord3D) -> f32 {
    a.x * b.x + a.y * b.y + a.z * b.z
}

fn calc_dist_squared(a: &Coord3D, b: &Coord3D) -> f32 {
    let dx = a.x - b.x;
    let dy = a.y - b.y;
    let dz = a.z - b.z;
    dx * dx + dy * dy + dz * dz
}

fn point_to_line_dist_squared(pt: &Coord3D, line_start: &Coord3D, line_end: &Coord3D) -> f32 {
    let line = Coord3D {
        x: line_end.x - line_start.x,
        y: line_end.y - line_start.y,
        z: line_end.z - line_start.z,
    };
    let line_to_pt = Coord3D {
        x: pt.x - line_start.x,
        y: pt.y - line_start.y,
        z: pt.z - line_start.z,
    };
    let dot = calc_dot(&line_to_pt, &line);
    if dot <= 0.0 {
        return calc_dist_squared(pt, line_start);
    }
    let line_len_sq = calc_dist_squared(line_start, line_end);
    if line_len_sq <= dot {
        return calc_dist_squared(pt, line_end);
    }
    let tmp = dot / line_len_sq;
    let closest = Coord3D {
        x: line_start.x + tmp * line.x,
        y: line_start.y + tmp * line.y,
        z: line_start.z + tmp * line.z,
    };
    calc_dist_squared(pt, &closest)
}

impl Default for GeometryInfo {
    fn default() -> Self {
        GeometryInfo {
            geometry_type: GeometryType::Sphere,
            is_small: false,
            height: 0.0,
            width: 0.0,
            depth: 0.0,
            bounding_circle_radius: 0.0,
            bounding_sphere_radius: 0.0,
        }
    }
}

// ------------------------------------------------------------------------------------------------
// Snapshotable implementation for GeometryInfo
// C++ Reference: Geometry.cpp lines 534-581
// ------------------------------------------------------------------------------------------------

impl Snapshotable for GeometryInfo {
    /// CRC - matches C++ GeometryInfo::crc() (Geometry.cpp line 534)
    /// C++ implementation is empty.
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    /// Save/Load transfer - matches C++ GeometryInfo::xfer() (Geometry.cpp lines 544-573)
    ///
    /// Version Info:
    /// 1: Initial version
    ///
    /// Fields xfer'd (Geometry.cpp lines 553-571):
    ///   1. type (GeometryType via xferUser, sizeof(enum)=4 on MSVC)
    ///   2. isSmall (Bool)
    ///   3. height (Real)
    ///   4. majorRadius (Real)
    ///   5. minorRadius (Real)
    ///   6. boundingCircleRadius (Real)
    ///   7. boundingSphereRadius (Real)
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        const CURRENT_VERSION: XferVersion = 1;
        let mut version: XferVersion = CURRENT_VERSION;
        xfer.xfer_version(&mut version, CURRENT_VERSION)
            .map_err(|e| format!("GeometryInfo::xfer version error: {}", e))?;

        // C++ Geometry.cpp:553 `xferUser(&m_type, sizeof(GeometryType))` — unscoped enum is int.
        let mut geo_type = self.geometry_type as i32;
        xfer.xfer_int(&mut geo_type)
            .map_err(|e| format!("GeometryInfo::xfer type error: {}", e))?;
        self.geometry_type = match geo_type {
            0 => GeometryType::Sphere,
            1 => GeometryType::Cylinder,
            2 => GeometryType::Box,
            _ => GeometryType::Sphere,
        };

        xfer.xfer_bool(&mut self.is_small)
            .map_err(|e| format!("GeometryInfo::xfer isSmall error: {}", e))?;

        xfer.xfer_real(&mut self.height)
            .map_err(|e| format!("GeometryInfo::xfer height error: {}", e))?;

        // C++ xfers m_majorRadius (our `width`) and m_minorRadius (our `depth`)
        xfer.xfer_real(&mut self.width)
            .map_err(|e| format!("GeometryInfo::xfer majorRadius error: {}", e))?;

        xfer.xfer_real(&mut self.depth)
            .map_err(|e| format!("GeometryInfo::xfer minorRadius error: {}", e))?;

        xfer.xfer_real(&mut self.bounding_circle_radius)
            .map_err(|e| format!("GeometryInfo::xfer boundingCircleRadius error: {}", e))?;

        xfer.xfer_real(&mut self.bounding_sphere_radius)
            .map_err(|e| format!("GeometryInfo::xfer boundingSphereRadius error: {}", e))?;

        Ok(())
    }

    /// Load post process - matches C++ GeometryInfo::loadPostProcess() (Geometry.cpp line 578)
    /// C++ implementation is empty.
    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

#[cfg(test)]
mod coord3d_cpp_parity {
    use super::{cpp_zup_to_host_yup, host_yup_to_cpp_zup, Coord3D};

    #[test]
    fn length_and_length_sqr_match_base_type() {
        let c = Coord3D::new(3.0, 4.0, 12.0);
        assert_eq!(c.length_sqr(), 9.0 + 16.0 + 144.0);
        assert_eq!(c.length(), 13.0);
    }

    #[test]
    fn normalize_in_place_leaves_zero_vector() {
        let mut z = Coord3D::new(0.0, 0.0, 0.0);
        z.normalize_in_place();
        assert!(z.equals(&Coord3D::new(0.0, 0.0, 0.0)));
    }

    #[test]
    fn normalize_in_place_matches_cpp_divide() {
        let mut c = Coord3D::new(0.0, 0.0, 2.0);
        c.normalize_in_place();
        assert_eq!(c, Coord3D::new(0.0, 0.0, 1.0));
    }

    #[test]
    fn cross_product_matches_cpp() {
        let a = Coord3D::new(1.0, 0.0, 0.0);
        let b = Coord3D::new(0.0, 1.0, 0.0);
        let r = Coord3D::cross_product(&a, &b);
        assert_eq!(r, Coord3D::new(0.0, 0.0, 1.0));
    }

    #[test]
    fn add_sub_scale_set_match_cpp() {
        let mut c = Coord3D::new(1.0, 2.0, 3.0);
        c.add_coord(&Coord3D::new(1.0, 1.0, 1.0));
        assert_eq!(c, Coord3D::new(2.0, 3.0, 4.0));
        c.sub_coord(&Coord3D::new(1.0, 1.0, 1.0));
        assert_eq!(c, Coord3D::new(1.0, 2.0, 3.0));
        c.scale(2.0);
        assert_eq!(c, Coord3D::new(2.0, 4.0, 6.0));
        c.set_xyz(7.0, 8.0, 9.0);
        assert_eq!(c, Coord3D::new(7.0, 8.0, 9.0));
        c.zero_in_place();
        assert!(c.is_null());
    }

    #[test]
    fn host_yup_and_cpp_zup_round_trip() {
        let zup = host_yup_to_cpp_zup(10.0, 5.0, 20.0);
        assert_eq!(zup, Coord3D::new(10.0, 20.0, 5.0));
        assert_eq!(cpp_zup_to_host_yup(zup), (10.0, 5.0, 20.0));
    }
}

#[cfg(test)]
mod basetype_cpp_parity {
    use super::{coord2d_to_angle, Coord2D, ICoord2D, ICoord3D, RGBColor, Region3D};

    #[test]
    fn coord2d_length_and_zero_normalize() {
        let c = Coord2D::new(3.0, 4.0);
        assert_eq!(c.length(), 5.0);
        let mut z = Coord2D::new(0.0, 0.0);
        z.normalize_in_place();
        assert_eq!(z, Coord2D::ZERO);
    }

    #[test]
    fn coord2d_to_angle_matches_base_type() {
        assert_eq!(coord2d_to_angle(0.0, 0.0), 0.0);
        assert!((Coord2D::new(1.0, 0.0).to_angle() - 0.0).abs() < 1e-6);
        assert!((Coord2D::new(0.0, 1.0).to_angle() - std::f32::consts::FRAC_PI_2).abs() < 1e-6);
        assert!((Coord2D::new(-1.0, 0.0).to_angle() - std::f32::consts::PI).abs() < 1e-6);
        assert!((Coord2D::new(0.0, -1.0).to_angle() + std::f32::consts::FRAC_PI_2).abs() < 1e-6);
    }

    #[test]
    fn iccoord_length_truncates_like_cpp() {
        assert_eq!(ICoord2D::new(3, 4).length(), 5);
        assert_eq!(ICoord2D::new(1, 1).length(), 1);
        assert_eq!(ICoord3D::new(2, 3, 6).length(), 7);
    }

    #[test]
    fn region3d_is_exclusive_like_cpp() {
        let r = Region3D::new(
            super::Coord3D::new(0.0, 0.0, 0.0),
            super::Coord3D::new(100.0, 100.0, 100.0),
        );
        assert!(r.is_in_region_no_z(&super::Coord3D::new(50.0, 50.0, 0.0)));
        assert!(!r.is_in_region_no_z(&super::Coord3D::new(0.0, 50.0, 0.0)));
        assert!(!r.is_in_region_no_z(&super::Coord3D::new(100.0, 50.0, 0.0)));
        assert!(r.is_in_region_with_z(&super::Coord3D::new(50.0, 50.0, 50.0)));
        assert!(!r.is_in_region_with_z(&super::Coord3D::new(50.0, 50.0, 0.0)));
        assert!(!r.is_in_region_with_z(&super::Coord3D::new(50.0, 50.0, 100.0)));
    }

    #[test]
    fn rgb_color_get_as_int_matches_cpp() {
        let c = RGBColor::new(1.0, 0.5, 0.0);
        assert_eq!(c.get_as_int(), (255 << 16) | (127 << 8) | 0);
        let mut back = RGBColor::default();
        back.set_from_int(c.get_as_int());
        assert!((back.red - 1.0).abs() < 1e-5);
        assert_eq!((back.green * 255.0) as i32, 127);
        assert_eq!((back.blue * 255.0) as i32, 0);
    }
}

#[cfg(test)]
mod geometry_info_cpp_parity {
    use super::{GeometryInfo, GeometryType};

    #[test]
    fn set_major_radius_writes_only_major() {
        let mut info = GeometryInfo::new(GeometryType::Cylinder, false, 10.0, 20.0, 5.0);
        info.depth = 5.0;
        info.height = 20.0;
        info.set_major_radius(12.0);
        assert_eq!(info.width, 12.0);
        assert_eq!(info.depth, 5.0);
        assert_eq!(info.height, 20.0);
    }
}
