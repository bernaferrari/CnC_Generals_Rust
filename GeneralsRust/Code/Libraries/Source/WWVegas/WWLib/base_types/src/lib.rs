//! Base types and fundamental data structures for Command & Conquer Generals Zero Hour
//!
//! This crate contains the core types that are used throughout the game engine,
//! including vectors, colors, ranges, and other fundamental mathematical constructs.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Mathematical constants
pub const PI: f32 = std::f32::consts::PI;
pub const TWO_PI: f32 = 2.0 * PI;

/// Fundamental numeric types
pub type Real = f32;
pub type Int = i32;
pub type UnsignedInt = u32;
pub type UnsignedShort = u16;
pub type Short = i16;
pub type UnsignedByte = u8;
pub type Byte = i8;
pub type Char = char;
pub type Bool = bool;
pub type Int64 = i64;
pub type UnsignedInt64 = u64;

/// Wide character type for multi-byte text
pub type WideChar = char;

/// Utility macros for bit manipulation
#[macro_export]
macro_rules! bit_test {
    ($x:expr, $i:expr) => {
        (($x) & ($i)) != 0
    };
}

#[macro_export]
macro_rules! bit_set {
    ($x:expr, $i:expr) => {
        $x |= $i
    };
}

#[macro_export]
macro_rules! bit_clear {
    ($x:expr, $i:expr) => {
        $x &= !$i
    };
}

#[macro_export]
macro_rules! bit_toggle {
    ($x:expr, $i:expr) => {
        $x ^= $i
    };
}

/// Mathematical utility functions
pub mod math {
    use super::Real;

    /// Square a number
    #[inline]
    pub fn sqr<T: std::ops::Mul<Output = T> + Copy>(x: T) -> T {
        x * x
    }

    /// Clamp a value between low and high bounds
    #[inline]
    pub fn clamp<T: PartialOrd>(lo: T, val: T, hi: T) -> T {
        if val < lo {
            lo
        } else if val > hi {
            hi
        } else {
            val
        }
    }

    /// Return the sign of a number (-1, 0, or 1)
    #[inline]
    pub fn sign<T: PartialOrd + Default>(x: T) -> i32 {
        use std::cmp::Ordering;
        match x.partial_cmp(&T::default()).unwrap_or(Ordering::Equal) {
            Ordering::Less => -1,
            Ordering::Equal => 0,
            Ordering::Greater => 1,
        }
    }

    /// Convert radians to degrees
    #[inline]
    pub fn rad2deg(rad: Real) -> Real {
        rad * (180.0 / super::PI)
    }

    /// Convert degrees to radians
    #[inline]
    pub fn deg2rad(deg: Real) -> Real {
        deg * (super::PI / 180.0)
    }
}

/// Real-valued range defined by low and high values
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct RealRange {
    pub lo: Real,
    pub hi: Real,
}

impl RealRange {
    /// Create a new RealRange
    pub fn new(lo: Real, hi: Real) -> Self {
        Self { lo, hi }
    }

    /// Combine the given range with this one such that this range now encompasses both
    pub fn combine(&mut self, other: &RealRange) {
        self.lo = self.lo.min(other.lo);
        self.hi = self.hi.max(other.hi);
    }
}

impl fmt::Display for RealRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}, {}]", self.lo, self.hi)
    }
}

/// 2D coordinate with Real precision
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Coord2D {
    pub x: Real,
    pub y: Real,
}

impl Coord2D {
    /// Create a new Coord2D
    pub fn new(x: Real, y: Real) -> Self {
        Self { x, y }
    }

    /// Calculate the length (magnitude) of the vector
    pub fn length(&self) -> Real {
        (self.x * self.x + self.y * self.y).sqrt()
    }

    /// Normalize the vector (make it unit length)
    pub fn normalize(&mut self) {
        let len = self.length();
        if len != 0.0 {
            self.x /= len;
            self.y /= len;
        }
    }

    /// Return a normalized copy of the vector
    pub fn normalized(&self) -> Self {
        let mut result = *self;
        result.normalize();
        result
    }

    /// Convert 2D vector to angle (where angle 0 is down the +x axis)
    pub fn to_angle(&self) -> Real {
        let len = self.length();
        if len == 0.0 {
            return 0.0;
        }

        let c = self.x / len;
        // bound it in case of numerical error
        let c = math::clamp(-1.0, c, 1.0);

        if self.y < 0.0 {
            -c.acos()
        } else {
            c.acos()
        }
    }
}

impl fmt::Display for Coord2D {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({:.3}, {:.3})", self.x, self.y)
    }
}

/// 2D coordinate with integer precision
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ICoord2D {
    pub x: Int,
    pub y: Int,
}

impl ICoord2D {
    /// Create a new ICoord2D
    pub fn new(x: Int, y: Int) -> Self {
        Self { x, y }
    }

    /// Calculate the length (magnitude) of the vector
    pub fn length(&self) -> Int {
        ((self.x * self.x + self.y * self.y) as f64).sqrt() as Int
    }
}

impl fmt::Display for ICoord2D {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({}, {})", self.x, self.y)
    }
}

/// 2D rectangular region with Real precision
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Region2D {
    pub lo: Coord2D,
    pub hi: Coord2D,
}

impl Region2D {
    /// Create a new Region2D
    pub fn new(lo: Coord2D, hi: Coord2D) -> Self {
        Self { lo, hi }
    }

    /// Get the width of the region
    pub fn width(&self) -> Real {
        self.hi.x - self.lo.x
    }

    /// Get the height of the region
    pub fn height(&self) -> Real {
        self.hi.y - self.lo.y
    }
}

impl fmt::Display for Region2D {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{} to {}]", self.lo, self.hi)
    }
}

/// 2D rectangular region with integer precision
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct IRegion2D {
    pub lo: ICoord2D,
    pub hi: ICoord2D,
}

impl IRegion2D {
    /// Create a new IRegion2D
    pub fn new(lo: ICoord2D, hi: ICoord2D) -> Self {
        Self { lo, hi }
    }

    /// Get the width of the region
    pub fn width(&self) -> Int {
        self.hi.x - self.lo.x
    }

    /// Get the height of the region
    pub fn height(&self) -> Int {
        self.hi.y - self.lo.y
    }
}

impl fmt::Display for IRegion2D {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{} to {}]", self.lo, self.hi)
    }
}

/// 3D coordinate with Real precision
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Coord3D {
    pub x: Real,
    pub y: Real,
    pub z: Real,
}

impl Coord3D {
    /// Create a new Coord3D
    pub fn new(x: Real, y: Real, z: Real) -> Self {
        Self { x, y, z }
    }

    /// Calculate the length (magnitude) of the vector
    pub fn length(&self) -> Real {
        (self.x * self.x + self.y * self.y + self.z * self.z).sqrt()
    }

    /// Calculate the squared length of the vector
    pub fn length_sqr(&self) -> Real {
        self.x * self.x + self.y * self.y + self.z * self.z
    }

    /// Normalize the vector (make it unit length)
    pub fn normalize(&mut self) {
        let len = self.length();
        if len != 0.0 {
            self.x /= len;
            self.y /= len;
            self.z /= len;
        }
    }

    /// Return a normalized copy of the vector
    pub fn normalized(&self) -> Self {
        let mut result = *self;
        result.normalize();
        result
    }

    /// Calculate cross product of two vectors
    pub fn cross_product(a: &Coord3D, b: &Coord3D) -> Coord3D {
        Coord3D {
            x: a.y * b.z - a.z * b.y,
            y: a.z * b.x - a.x * b.z,
            z: a.x * b.y - a.y * b.x,
        }
    }

    /// Set all components to zero
    pub fn zero(&mut self) {
        self.x = 0.0;
        self.y = 0.0;
        self.z = 0.0;
    }

    /// Add another vector to this one
    pub fn add(&mut self, other: &Coord3D) {
        self.x += other.x;
        self.y += other.y;
        self.z += other.z;
    }

    /// Subtract another vector from this one
    pub fn sub(&mut self, other: &Coord3D) {
        self.x -= other.x;
        self.y -= other.y;
        self.z -= other.z;
    }

    /// Set this vector to be equal to another
    pub fn set(&mut self, other: &Coord3D) {
        self.x = other.x;
        self.y = other.y;
        self.z = other.z;
    }

    /// Set this vector to specific values
    pub fn set_xyz(&mut self, x: Real, y: Real, z: Real) {
        self.x = x;
        self.y = y;
        self.z = z;
    }

    /// Scale this vector by a scalar value
    pub fn scale(&mut self, scale: Real) {
        self.x *= scale;
        self.y *= scale;
        self.z *= scale;
    }

    /// Check if this vector equals another
    pub fn equals(&self, other: &Coord3D) -> Bool {
        self.x == other.x && self.y == other.y && self.z == other.z
    }
}

impl std::ops::Add for Coord3D {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Self {
            x: self.x + other.x,
            y: self.y + other.y,
            z: self.z + other.z,
        }
    }
}

impl std::ops::Sub for Coord3D {
    type Output = Self;

    fn sub(self, other: Self) -> Self {
        Self {
            x: self.x - other.x,
            y: self.y - other.y,
            z: self.z - other.z,
        }
    }
}

impl std::ops::Mul<Real> for Coord3D {
    type Output = Self;

    fn mul(self, scalar: Real) -> Self {
        Self {
            x: self.x * scalar,
            y: self.y * scalar,
            z: self.z * scalar,
        }
    }
}

impl fmt::Display for Coord3D {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({:.3}, {:.3}, {:.3})", self.x, self.y, self.z)
    }
}

/// 3D coordinate with integer precision
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ICoord3D {
    pub x: Int,
    pub y: Int,
    pub z: Int,
}

impl ICoord3D {
    /// Create a new ICoord3D
    pub fn new(x: Int, y: Int, z: Int) -> Self {
        Self { x, y, z }
    }

    /// Calculate the length (magnitude) of the vector
    pub fn length(&self) -> Int {
        ((self.x * self.x + self.y * self.y + self.z * self.z) as f64).sqrt() as Int
    }

    /// Set all components to zero
    pub fn zero(&mut self) {
        self.x = 0;
        self.y = 0;
        self.z = 0;
    }
}

impl fmt::Display for ICoord3D {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({}, {}, {})", self.x, self.y, self.z)
    }
}

/// 3D rectangular region (axis-aligned bounding box) with Real precision
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Region3D {
    pub lo: Coord3D,
    pub hi: Coord3D,
}

impl Region3D {
    /// Create a new Region3D
    pub fn new(lo: Coord3D, hi: Coord3D) -> Self {
        Self { lo, hi }
    }

    /// Get the width of the region
    pub fn width(&self) -> Real {
        self.hi.x - self.lo.x
    }

    /// Get the height of the region
    pub fn height(&self) -> Real {
        self.hi.y - self.lo.y
    }

    /// Get the depth of the region
    pub fn depth(&self) -> Real {
        self.hi.z - self.lo.z
    }

    /// Set all components to zero
    pub fn zero(&mut self) {
        self.lo.zero();
        self.hi.zero();
    }

    /// Check if a point is within the region (ignoring Z coordinate)
    pub fn is_in_region_no_z(&self, query: &Coord3D) -> Bool {
        self.lo.x < query.x && query.x < self.hi.x && self.lo.y < query.y && query.y < self.hi.y
    }

    /// Check if a point is within the region (including Z coordinate)
    pub fn is_in_region_with_z(&self, query: &Coord3D) -> Bool {
        self.lo.x < query.x
            && query.x < self.hi.x
            && self.lo.y < query.y
            && query.y < self.hi.y
            && self.lo.z < query.z
            && query.z < self.hi.z
    }
}

impl fmt::Display for Region3D {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{} to {}]", self.lo, self.hi)
    }
}

/// 3D rectangular region with integer precision
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct IRegion3D {
    pub lo: ICoord3D,
    pub hi: ICoord3D,
}

impl IRegion3D {
    /// Create a new IRegion3D
    pub fn new(lo: ICoord3D, hi: ICoord3D) -> Self {
        Self { lo, hi }
    }

    /// Get the width of the region
    pub fn width(&self) -> Int {
        self.hi.x - self.lo.x
    }

    /// Get the height of the region
    pub fn height(&self) -> Int {
        self.hi.y - self.lo.y
    }

    /// Get the depth of the region
    pub fn depth(&self) -> Int {
        self.hi.z - self.lo.z
    }
}

impl fmt::Display for IRegion3D {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{} to {}]", self.lo, self.hi)
    }
}

/// RGB color with Real precision (range 0.0 to 1.0)
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct RGBColor {
    pub red: Real,
    pub green: Real,
    pub blue: Real,
}

impl RGBColor {
    /// Create a new RGBColor
    pub fn new(red: Real, green: Real, blue: Real) -> Self {
        Self { red, green, blue }
    }

    /// Convert to 32-bit integer representation
    pub fn get_as_int(&self) -> Int {
        ((self.red * 255.0) as Int) << 16
            | ((self.green * 255.0) as Int) << 8
            | ((self.blue * 255.0) as Int)
    }

    /// Set from 32-bit integer representation
    pub fn set_from_int(&mut self, c: Int) {
        self.red = ((c >> 16) & 0xff) as Real / 255.0;
        self.green = ((c >> 8) & 0xff) as Real / 255.0;
        self.blue = (c & 0xff) as Real / 255.0;
    }

    /// Create from 32-bit integer representation
    pub fn from_int(c: Int) -> Self {
        let mut color = Self::new(0.0, 0.0, 0.0);
        color.set_from_int(c);
        color
    }
}

impl fmt::Display for RGBColor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "RGB({:.3}, {:.3}, {:.3})",
            self.red, self.green, self.blue
        )
    }
}

/// RGBA color with Real precision (range 0.0 to 1.0)
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct RGBAColorReal {
    pub red: Real,
    pub green: Real,
    pub blue: Real,
    pub alpha: Real,
}

impl RGBAColorReal {
    /// Create a new RGBAColorReal
    pub fn new(red: Real, green: Real, blue: Real, alpha: Real) -> Self {
        Self {
            red,
            green,
            blue,
            alpha,
        }
    }
}

impl fmt::Display for RGBAColorReal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "RGBA({:.3}, {:.3}, {:.3}, {:.3})",
            self.red, self.green, self.blue, self.alpha
        )
    }
}

/// RGBA color with integer precision (range 0 to 255)
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct RGBAColorInt {
    pub red: UnsignedInt,
    pub green: UnsignedInt,
    pub blue: UnsignedInt,
    pub alpha: UnsignedInt,
}

impl RGBAColorInt {
    /// Create a new RGBAColorInt
    pub fn new(
        red: UnsignedInt,
        green: UnsignedInt,
        blue: UnsignedInt,
        alpha: UnsignedInt,
    ) -> Self {
        Self {
            red,
            green,
            blue,
            alpha,
        }
    }
}

impl fmt::Display for RGBAColorInt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "RGBA({},{},{},{})",
            self.red, self.green, self.blue, self.alpha
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_coord2d_length() {
        let coord = Coord2D::new(3.0, 4.0);
        assert_eq!(coord.length(), 5.0);
    }

    #[test]
    fn test_coord3d_operations() {
        let mut coord = Coord3D::new(1.0, 2.0, 3.0);
        coord.scale(2.0);
        assert_eq!(coord, Coord3D::new(2.0, 4.0, 6.0));
    }

    #[test]
    fn test_rgb_color_conversion() {
        let color = RGBColor::new(1.0, 0.5, 0.0);
        let int_repr = color.get_as_int();
        let reconstructed = RGBColor::from_int(int_repr);
        assert!((color.red - reconstructed.red).abs() < 0.01);
        assert!((color.green - reconstructed.green).abs() < 0.01);
        assert!((color.blue - reconstructed.blue).abs() < 0.01);
    }

    #[test]
    fn test_math_utilities() {
        assert_eq!(math::sqr(5), 25);
        assert_eq!(math::clamp(0, 5, 10), 5);
        assert_eq!(math::clamp(0, 15, 10), 10);
        assert_eq!(math::clamp(0, -5, 10), 0);
        assert_eq!(math::sign(5.0), 1);
        assert_eq!(math::sign(-3.0), -1);
        assert_eq!(math::sign(0.0), 0);
    }
}
