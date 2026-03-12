//! Point module providing 2D and 3D point types with full mathematical operations.
//!
//! This module contains generic point types that support arbitrary numeric types,
//! providing vector operations, arithmetic, and comparison operations.
//!
//! # Examples
//!
//! ```rust
//! use wwlib_rust::point::{TPoint2D, TPoint3D, Point2D, Point3D};
//!
//! // 2D point operations
//! let p1 = Point2D::new(3, 4);
//! let p2 = Point2D::new(1, 2);
//! let result = p1 + p2;
//! assert_eq!(result, Point2D::new(4, 6));
//!
//! // 3D point operations
//! let p3d1 = Point3D::new(1, 2, 3);
//! let p3d2 = Point3D::new(4, 5, 6);
//! let cross = p3d1.cross_product(p3d2);
//! ```

use std::fmt::{Debug, Display, Formatter, Result as FmtResult};
use std::ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Neg, Sub, SubAssign};

/// Generic 2D point type supporting arbitrary numeric types.
///
/// This class describes a point in 2 dimensional space using arbitrary
/// components. The interpretation of which is outside the scope
/// of this class. This class is the successor to the old style COORDINATE
/// and CELL types but also serves anywhere an X and Y value are treated
/// as a logical object (e.g., pixel location).
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct TPoint2D<T> {
    /// X coordinate
    pub x: T,
    /// Y coordinate  
    pub y: T,
}

impl<T> TPoint2D<T> {
    /// Creates a new 2D point with the given coordinates.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use wwlib_rust::point::TPoint2D;
    /// let point = TPoint2D::new(10, 20);
    /// assert_eq!(point.x, 10);
    /// assert_eq!(point.y, 20);
    /// ```
    pub fn new(x: T, y: T) -> Self {
        TPoint2D { x, y }
    }
}

impl<T> Default for TPoint2D<T>
where
    T: Default,
{
    /// Creates a default point (0, 0) for types that support Default.
    fn default() -> Self {
        TPoint2D {
            x: T::default(),
            y: T::default(),
        }
    }
}

// Arithmetic operations for TPoint2D

impl<T> Add for TPoint2D<T>
where
    T: Add<Output = T>,
{
    type Output = TPoint2D<T>;

    fn add(self, rhs: TPoint2D<T>) -> Self::Output {
        TPoint2D::new(self.x + rhs.x, self.y + rhs.y)
    }
}

impl<T> AddAssign for TPoint2D<T>
where
    T: AddAssign,
{
    fn add_assign(&mut self, rhs: TPoint2D<T>) {
        self.x += rhs.x;
        self.y += rhs.y;
    }
}

impl<T> Sub for TPoint2D<T>
where
    T: Sub<Output = T>,
{
    type Output = TPoint2D<T>;

    fn sub(self, rhs: TPoint2D<T>) -> Self::Output {
        TPoint2D::new(self.x - rhs.x, self.y - rhs.y)
    }
}

impl<T> SubAssign for TPoint2D<T>
where
    T: SubAssign,
{
    fn sub_assign(&mut self, rhs: TPoint2D<T>) {
        self.x -= rhs.x;
        self.y -= rhs.y;
    }
}

impl<T> Mul<T> for TPoint2D<T>
where
    T: Mul<Output = T> + Copy,
{
    type Output = TPoint2D<T>;

    fn mul(self, scalar: T) -> Self::Output {
        TPoint2D::new(self.x * scalar, self.y * scalar)
    }
}

impl<T> MulAssign<T> for TPoint2D<T>
where
    T: MulAssign + Copy,
{
    fn mul_assign(&mut self, scalar: T) {
        self.x *= scalar;
        self.y *= scalar;
    }
}

impl<T> Div<T> for TPoint2D<T>
where
    T: Div<Output = T> + Copy + PartialEq + Default,
{
    type Output = TPoint2D<T>;

    fn div(self, scalar: T) -> Self::Output {
        if scalar == T::default() {
            TPoint2D::new(T::default(), T::default())
        } else {
            TPoint2D::new(self.x / scalar, self.y / scalar)
        }
    }
}

impl<T> DivAssign<T> for TPoint2D<T>
where
    T: DivAssign + Copy + PartialEq + Default,
{
    fn div_assign(&mut self, scalar: T) {
        if scalar != T::default() {
            self.x /= scalar;
            self.y /= scalar;
        }
    }
}

impl<T> Neg for TPoint2D<T>
where
    T: Neg<Output = T>,
{
    type Output = TPoint2D<T>;

    fn neg(self) -> Self::Output {
        TPoint2D::new(-self.x, -self.y)
    }
}

// Component-wise multiplication (dot product in terms of components)
impl<T> Mul<TPoint2D<T>> for TPoint2D<T>
where
    T: Mul<Output = T>,
{
    type Output = TPoint2D<T>;

    fn mul(self, rhs: TPoint2D<T>) -> Self::Output {
        TPoint2D::new(self.x * rhs.x, self.y * rhs.y)
    }
}

// Vector operations for TPoint2D
impl<T> TPoint2D<T>
where
    T: Copy
        + Add<Output = T>
        + Sub<Output = T>
        + Mul<Output = T>
        + Div<Output = T>
        + PartialEq
        + Default,
{
    /// Component-wise multiplication (dot product components).
    ///
    /// # Examples
    ///
    /// ```rust
    /// use wwlib_rust::point::TPoint2D;
    /// let p1 = TPoint2D::new(2, 3);
    /// let p2 = TPoint2D::new(4, 5);
    /// let result = p1.dot_product(p2);
    /// assert_eq!(result, TPoint2D::new(8, 15));
    /// ```
    pub fn dot_product(self, rhs: TPoint2D<T>) -> TPoint2D<T> {
        TPoint2D::new(self.x * rhs.x, self.y * rhs.y)
    }

    /// 2D cross product (returns perpendicular vector).
    ///
    /// # Examples
    ///
    /// ```rust
    /// use wwlib_rust::point::TPoint2D;
    /// let p1 = TPoint2D::new(1, 2);
    /// let p2 = TPoint2D::new(3, 4);
    /// let cross = p1.cross_product(p2);
    /// assert_eq!(cross, TPoint2D::new(-2, 2));
    /// ```
    pub fn cross_product(self, rhs: TPoint2D<T>) -> TPoint2D<T> {
        TPoint2D::new(self.y - rhs.y, rhs.x - self.x)
    }

    /// Find distance between two points.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use wwlib_rust::point::TPoint2D;
    /// let p1 = TPoint2D::new(0.0, 0.0);
    /// let p2 = TPoint2D::new(3.0, 4.0);
    /// let distance = p1.distance_to(p2);
    /// assert!((distance - 5.0).abs() < f64::EPSILON);
    /// ```
    pub fn distance_to(self, point: TPoint2D<T>) -> f64
    where
        T: Into<f64>,
    {
        (self - point).length()
    }
}

impl<T> TPoint2D<T>
where
    T: Copy + Mul<Output = T> + Add<Output = T> + Into<f64>,
{
    /// Calculate the length (magnitude) of the vector.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use wwlib_rust::point::TPoint2D;
    /// let point = TPoint2D::new(3.0, 4.0);
    /// let length = point.length();
    /// assert!((length - 5.0).abs() < f64::EPSILON);
    /// ```
    pub fn length(self) -> f64 {
        let x_f64: f64 = self.x.into();
        let y_f64: f64 = self.y.into();
        (x_f64 * x_f64 + y_f64 * y_f64).sqrt()
    }
}

impl<T> TPoint2D<T>
where
    T: Copy + Mul<Output = T> + Add<Output = T> + Into<f64> + From<f64>,
{
    /// Normalize the vector to unit length.
    ///
    /// Returns the original vector if length is zero.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use wwlib_rust::point::TPoint2D;
    /// let point = TPoint2D::new(3.0, 4.0);
    /// let normalized = point.normalize();
    /// assert!((normalized.length() - 1.0).abs() < f64::EPSILON);
    /// ```
    pub fn normalize(self) -> TPoint2D<T> {
        let x_f64: f64 = self.x.into();
        let y_f64: f64 = self.y.into();
        let len = (x_f64 * x_f64 + y_f64 * y_f64).sqrt();

        if len != 0.0 {
            TPoint2D::new(T::from(x_f64 / len), T::from(y_f64 / len))
        } else {
            self
        }
    }
}

impl<T> Display for TPoint2D<T>
where
    T: Display,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "({}, {})", self.x, self.y)
    }
}

// Generic 3D point type
/// Generic 3D point type supporting arbitrary numeric types.
///
/// This describes a point in 3 dimensional space using arbitrary
/// components. This is the successor to the COORDINATE type for those
/// times when height needs to be tracked.
///
/// Notice that it contains a 2D point as its base, allowing for easy
/// conversion and interoperability between 2D and 3D operations.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct TPoint3D<T> {
    /// X coordinate
    pub x: T,
    /// Y coordinate
    pub y: T,
    /// Z coordinate
    pub z: T,
}

impl<T> TPoint3D<T> {
    /// Creates a new 3D point with the given coordinates.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use wwlib_rust::point::TPoint3D;
    /// let point = TPoint3D::new(1, 2, 3);
    /// assert_eq!(point.x, 1);
    /// assert_eq!(point.y, 2);
    /// assert_eq!(point.z, 3);
    /// ```
    pub fn new(x: T, y: T, z: T) -> Self {
        TPoint3D { x, y, z }
    }

    /// Get the 2D component of this 3D point.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use wwlib_rust::point::{TPoint3D, TPoint2D};
    /// let point3d = TPoint3D::new(1, 2, 3);
    /// let point2d = point3d.as_2d();
    /// assert_eq!(point2d, TPoint2D::new(1, 2));
    /// ```
    pub fn as_2d(self) -> TPoint2D<T> {
        TPoint2D::new(self.x, self.y)
    }
}

impl<T> Default for TPoint3D<T>
where
    T: Default,
{
    /// Creates a default point (0, 0, 0) for types that support Default.
    fn default() -> Self {
        TPoint3D {
            x: T::default(),
            y: T::default(),
            z: T::default(),
        }
    }
}

// From conversions between 2D and 3D points
impl<T> From<TPoint2D<T>> for TPoint3D<T>
where
    T: Default,
{
    fn from(point: TPoint2D<T>) -> Self {
        TPoint3D::new(point.x, point.y, T::default())
    }
}

impl<T> From<TPoint3D<T>> for TPoint2D<T> {
    fn from(point: TPoint3D<T>) -> Self {
        TPoint2D::new(point.x, point.y)
    }
}

// Arithmetic operations for TPoint3D with TPoint3D

impl<T> Add for TPoint3D<T>
where
    T: Add<Output = T>,
{
    type Output = TPoint3D<T>;

    fn add(self, rhs: TPoint3D<T>) -> Self::Output {
        TPoint3D::new(self.x + rhs.x, self.y + rhs.y, self.z + rhs.z)
    }
}

impl<T> Add<TPoint2D<T>> for TPoint3D<T>
where
    T: Add<Output = T>,
{
    type Output = TPoint3D<T>;

    fn add(self, rhs: TPoint2D<T>) -> Self::Output {
        TPoint3D::new(self.x + rhs.x, self.y + rhs.y, self.z)
    }
}

impl<T> AddAssign for TPoint3D<T>
where
    T: AddAssign,
{
    fn add_assign(&mut self, rhs: TPoint3D<T>) {
        self.x += rhs.x;
        self.y += rhs.y;
        self.z += rhs.z;
    }
}

impl<T> AddAssign<TPoint2D<T>> for TPoint3D<T>
where
    T: AddAssign,
{
    fn add_assign(&mut self, rhs: TPoint2D<T>) {
        self.x += rhs.x;
        self.y += rhs.y;
    }
}

impl<T> Sub for TPoint3D<T>
where
    T: Sub<Output = T>,
{
    type Output = TPoint3D<T>;

    fn sub(self, rhs: TPoint3D<T>) -> Self::Output {
        TPoint3D::new(self.x - rhs.x, self.y - rhs.y, self.z - rhs.z)
    }
}

impl<T> Sub<TPoint2D<T>> for TPoint3D<T>
where
    T: Sub<Output = T>,
{
    type Output = TPoint3D<T>;

    fn sub(self, rhs: TPoint2D<T>) -> Self::Output {
        TPoint3D::new(self.x - rhs.x, self.y - rhs.y, self.z)
    }
}

impl<T> SubAssign for TPoint3D<T>
where
    T: SubAssign,
{
    fn sub_assign(&mut self, rhs: TPoint3D<T>) {
        self.x -= rhs.x;
        self.y -= rhs.y;
        self.z -= rhs.z;
    }
}

impl<T> SubAssign<TPoint2D<T>> for TPoint3D<T>
where
    T: SubAssign,
{
    fn sub_assign(&mut self, rhs: TPoint2D<T>) {
        self.x -= rhs.x;
        self.y -= rhs.y;
    }
}

impl<T> Mul<T> for TPoint3D<T>
where
    T: Mul<Output = T> + Copy,
{
    type Output = TPoint3D<T>;

    fn mul(self, scalar: T) -> Self::Output {
        TPoint3D::new(self.x * scalar, self.y * scalar, self.z * scalar)
    }
}

impl<T> MulAssign<T> for TPoint3D<T>
where
    T: MulAssign + Copy,
{
    fn mul_assign(&mut self, scalar: T) {
        self.x *= scalar;
        self.y *= scalar;
        self.z *= scalar;
    }
}

impl<T> Div<T> for TPoint3D<T>
where
    T: Div<Output = T> + Copy + PartialEq + Default,
{
    type Output = TPoint3D<T>;

    fn div(self, scalar: T) -> Self::Output {
        if scalar == T::default() {
            TPoint3D::new(T::default(), T::default(), T::default())
        } else {
            TPoint3D::new(self.x / scalar, self.y / scalar, self.z / scalar)
        }
    }
}

impl<T> DivAssign<T> for TPoint3D<T>
where
    T: DivAssign + Copy + PartialEq + Default,
{
    fn div_assign(&mut self, scalar: T) {
        if scalar != T::default() {
            self.x /= scalar;
            self.y /= scalar;
            self.z /= scalar;
        }
    }
}

impl<T> Neg for TPoint3D<T>
where
    T: Neg<Output = T>,
{
    type Output = TPoint3D<T>;

    fn neg(self) -> Self::Output {
        TPoint3D::new(-self.x, -self.y, -self.z)
    }
}

// Component-wise multiplication
impl<T> Mul<TPoint3D<T>> for TPoint3D<T>
where
    T: Mul<Output = T>,
{
    type Output = TPoint3D<T>;

    fn mul(self, rhs: TPoint3D<T>) -> Self::Output {
        TPoint3D::new(self.x * rhs.x, self.y * rhs.y, self.z * rhs.z)
    }
}

// Vector operations for TPoint3D
impl<T> TPoint3D<T>
where
    T: Copy
        + Add<Output = T>
        + Sub<Output = T>
        + Mul<Output = T>
        + Div<Output = T>
        + PartialEq
        + Default,
{
    /// Component-wise multiplication (dot product components).
    ///
    /// # Examples
    ///
    /// ```rust
    /// use wwlib_rust::point::TPoint3D;
    /// let p1 = TPoint3D::new(2, 3, 4);
    /// let p2 = TPoint3D::new(5, 6, 7);
    /// let result = p1.dot_product(p2);
    /// assert_eq!(result, TPoint3D::new(10, 18, 28));
    /// ```
    pub fn dot_product(self, rhs: TPoint3D<T>) -> TPoint3D<T> {
        TPoint3D::new(self.x * rhs.x, self.y * rhs.y, self.z * rhs.z)
    }

    /// 3D cross product.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use wwlib_rust::point::TPoint3D;
    /// let p1 = TPoint3D::new(1, 0, 0);
    /// let p2 = TPoint3D::new(0, 1, 0);
    /// let cross = p1.cross_product(p2);
    /// assert_eq!(cross, TPoint3D::new(0, 0, 1));
    /// ```
    pub fn cross_product(self, rhs: TPoint3D<T>) -> TPoint3D<T> {
        TPoint3D::new(
            self.y * rhs.z - self.z * rhs.y,
            self.z * rhs.x - self.x * rhs.z,
            self.x * rhs.y - self.y * rhs.x,
        )
    }

    /// Find distance between two 3D points.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use wwlib_rust::point::TPoint3D;
    /// let p1 = TPoint3D::new(0.0, 0.0, 0.0);
    /// let p2 = TPoint3D::new(1.0, 1.0, 1.0);
    /// let distance = p1.distance_to(p2);
    /// assert!((distance - 3.0_f64.sqrt()).abs() < 1e-10);
    /// ```
    pub fn distance_to(self, point: TPoint3D<T>) -> f64
    where
        T: Into<f64>,
    {
        (self - point).length()
    }

    /// Find distance to a 2D point (using base 2D coordinates).
    ///
    /// # Examples
    ///
    /// ```rust
    /// use wwlib_rust::point::{TPoint3D, TPoint2D};
    /// let p3d = TPoint3D::new(0.0, 0.0, 5.0);
    /// let p2d = TPoint2D::new(3.0, 4.0);
    /// let distance = p3d.distance_to_2d(p2d);
    /// assert!((distance - 5.0).abs() < f64::EPSILON);
    /// ```
    pub fn distance_to_2d(self, point: TPoint2D<T>) -> f64
    where
        T: Into<f64>,
    {
        self.as_2d().distance_to(point)
    }
}

impl<T> TPoint3D<T>
where
    T: Copy + Mul<Output = T> + Add<Output = T> + Into<f64>,
{
    /// Calculate the length (magnitude) of the 3D vector.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use wwlib_rust::point::TPoint3D;
    /// let point = TPoint3D::new(1.0, 2.0, 2.0);
    /// let length = point.length();
    /// assert!((length - 3.0).abs() < f64::EPSILON);
    /// ```
    pub fn length(self) -> f64 {
        let x_f64: f64 = self.x.into();
        let y_f64: f64 = self.y.into();
        let z_f64: f64 = self.z.into();
        (x_f64 * x_f64 + y_f64 * y_f64 + z_f64 * z_f64).sqrt()
    }
}

impl<T> TPoint3D<T>
where
    T: Copy + Mul<Output = T> + Add<Output = T> + Into<f64> + From<f64>,
{
    /// Normalize the 3D vector to unit length.
    ///
    /// Returns the original vector if length is zero.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use wwlib_rust::point::TPoint3D;
    /// let point = TPoint3D::new(1.0, 2.0, 2.0);
    /// let normalized = point.normalize();
    /// assert!((normalized.length() - 1.0).abs() < f64::EPSILON);
    /// ```
    pub fn normalize(self) -> TPoint3D<T> {
        let x_f64: f64 = self.x.into();
        let y_f64: f64 = self.y.into();
        let z_f64: f64 = self.z.into();
        let len = (x_f64 * x_f64 + y_f64 * y_f64 + z_f64 * z_f64).sqrt();

        if len != 0.0 {
            TPoint3D::new(
                T::from(x_f64 / len),
                T::from(y_f64 / len),
                T::from(z_f64 / len),
            )
        } else {
            self
        }
    }
}

impl<T> Display for TPoint3D<T>
where
    T: Display,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "({}, {}, {})", self.x, self.y, self.z)
    }
}

/// Type alias for integer 2D points - provides a simple uncluttered type name.
pub type Point2D = TPoint2D<i32>;

/// Type alias for integer 3D points - provides a simple uncluttered type name.
pub type Point3D = TPoint3D<i32>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_point2d_creation() {
        let p = Point2D::new(10, 20);
        assert_eq!(p.x, 10);
        assert_eq!(p.y, 20);
    }

    #[test]
    fn test_point2d_default() {
        let p = Point2D::default();
        assert_eq!(p.x, 0);
        assert_eq!(p.y, 0);
    }

    #[test]
    fn test_point2d_arithmetic() {
        let p1 = Point2D::new(3, 4);
        let p2 = Point2D::new(1, 2);

        // Addition
        let sum = p1 + p2;
        assert_eq!(sum, Point2D::new(4, 6));

        // Subtraction
        let diff = p1 - p2;
        assert_eq!(diff, Point2D::new(2, 2));

        // Scalar multiplication
        let scaled = p1 * 2;
        assert_eq!(scaled, Point2D::new(6, 8));

        // Scalar division
        let divided = p1 / 2;
        assert_eq!(divided, Point2D::new(1, 2));

        // Negation
        let negated = -p1;
        assert_eq!(negated, Point2D::new(-3, -4));
    }

    #[test]
    fn test_point2d_assignment_operators() {
        let mut p = Point2D::new(3, 4);

        // Add assign
        p += Point2D::new(1, 2);
        assert_eq!(p, Point2D::new(4, 6));

        // Sub assign
        p -= Point2D::new(1, 1);
        assert_eq!(p, Point2D::new(3, 5));

        // Mul assign
        p *= 2;
        assert_eq!(p, Point2D::new(6, 10));

        // Div assign
        p /= 2;
        assert_eq!(p, Point2D::new(3, 5));
    }

    #[test]
    fn test_point2d_vector_operations() {
        let p1 = TPoint2D::new(3.0, 4.0);
        let p2 = TPoint2D::new(1.0, 2.0);

        // Length
        assert!((p1.length() - 5.0).abs() < f64::EPSILON);

        // Normalize
        let normalized = p1.normalize();
        assert!((normalized.length() - 1.0).abs() < f64::EPSILON);

        // Dot product
        let dot = p1.dot_product(p2);
        assert_eq!(dot, TPoint2D::new(3.0, 8.0));

        // Cross product
        let cross = p1.cross_product(p2);
        assert_eq!(cross, TPoint2D::new(2.0, -2.0));

        // Distance
        let distance = p1.distance_to(p2);
        assert!((distance - (8.0_f64).sqrt()).abs() < f64::EPSILON);
    }

    #[test]
    fn test_point2d_division_by_zero() {
        let p = Point2D::new(4, 6);
        let result = p / 0;
        assert_eq!(result, Point2D::new(0, 0));

        let mut p_mut = Point2D::new(4, 6);
        p_mut /= 0;
        assert_eq!(p_mut, Point2D::new(4, 6)); // Should remain unchanged
    }

    #[test]
    fn test_point3d_creation() {
        let p = Point3D::new(1, 2, 3);
        assert_eq!(p.x, 1);
        assert_eq!(p.y, 2);
        assert_eq!(p.z, 3);
    }

    #[test]
    fn test_point3d_default() {
        let p = Point3D::default();
        assert_eq!(p.x, 0);
        assert_eq!(p.y, 0);
        assert_eq!(p.z, 0);
    }

    #[test]
    fn test_point3d_arithmetic() {
        let p1 = Point3D::new(1, 2, 3);
        let p2 = Point3D::new(4, 5, 6);

        // Addition
        let sum = p1 + p2;
        assert_eq!(sum, Point3D::new(5, 7, 9));

        // Subtraction
        let diff = p2 - p1;
        assert_eq!(diff, Point3D::new(3, 3, 3));

        // Scalar multiplication
        let scaled = p1 * 2;
        assert_eq!(scaled, Point3D::new(2, 4, 6));

        // Scalar division
        let divided = p2 / 2;
        assert_eq!(divided, Point3D::new(2, 2, 3));

        // Negation
        let negated = -p1;
        assert_eq!(negated, Point3D::new(-1, -2, -3));
    }

    #[test]
    fn test_point3d_2d_operations() {
        let p3d = Point3D::new(1, 2, 3);
        let p2d = Point2D::new(4, 5);

        // 3D + 2D
        let sum = p3d + p2d;
        assert_eq!(sum, Point3D::new(5, 7, 3));

        // 3D - 2D
        let diff = p3d - p2d;
        assert_eq!(diff, Point3D::new(-3, -3, 3));

        // Assignment operations
        let mut p3d_mut = Point3D::new(1, 2, 3);
        p3d_mut += p2d;
        assert_eq!(p3d_mut, Point3D::new(5, 7, 3));

        p3d_mut -= p2d;
        assert_eq!(p3d_mut, Point3D::new(1, 2, 3));
    }

    #[test]
    fn test_point3d_vector_operations() {
        let p1 = TPoint3D::new(1.0, 2.0, 2.0);
        let p2 = TPoint3D::new(2.0, 1.0, 0.0);

        // Length
        assert!((p1.length() - 3.0).abs() < f64::EPSILON);

        // Normalize
        let normalized = p1.normalize();
        assert!((normalized.length() - 1.0).abs() < f64::EPSILON);

        // Dot product
        let dot = p1.dot_product(p2);
        assert_eq!(dot, TPoint3D::new(2.0, 2.0, 0.0));

        // Cross product
        let cross = p1.cross_product(p2);
        assert_eq!(cross, TPoint3D::new(-2.0, 4.0, -3.0));

        // Distance
        let distance = p1.distance_to(p2);
        assert!((distance - (6.0_f64).sqrt()).abs() < f64::EPSILON);
    }

    #[test]
    fn test_cross_product_standard_vectors() {
        // Test standard basis vector cross products
        let x_axis = TPoint3D::new(1.0, 0.0, 0.0);
        let y_axis = TPoint3D::new(0.0, 1.0, 0.0);
        let z_axis = TPoint3D::new(0.0, 0.0, 1.0);

        // x × y = z
        let cross = x_axis.cross_product(y_axis);
        assert_eq!(cross, z_axis);

        // y × z = x
        let cross = y_axis.cross_product(z_axis);
        assert_eq!(cross, x_axis);

        // z × x = y
        let cross = z_axis.cross_product(x_axis);
        assert_eq!(cross, y_axis);
    }

    #[test]
    fn test_conversions() {
        let p2d = Point2D::new(10, 20);
        let p3d_from_2d: Point3D = p2d.into();
        assert_eq!(p3d_from_2d, Point3D::new(10, 20, 0));

        let p3d = Point3D::new(1, 2, 3);
        let p2d_from_3d: Point2D = p3d.into();
        assert_eq!(p2d_from_3d, Point2D::new(1, 2));

        // Test as_2d method
        let p2d_as = p3d.as_2d();
        assert_eq!(p2d_as, Point2D::new(1, 2));
    }

    #[test]
    fn test_distance_to_2d() {
        let p3d = TPoint3D::new(0.0, 0.0, 5.0);
        let p2d = TPoint2D::new(3.0, 4.0);
        let distance = p3d.distance_to_2d(p2d);
        assert!((distance - 5.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_display_formatting() {
        let p2d = Point2D::new(10, 20);
        assert_eq!(format!("{}", p2d), "(10, 20)");

        let p3d = Point3D::new(1, 2, 3);
        assert_eq!(format!("{}", p3d), "(1, 2, 3)");
    }

    #[test]
    fn test_normalize_zero_vector() {
        let p2d = TPoint2D::new(0.0, 0.0);
        let normalized = p2d.normalize();
        assert_eq!(normalized, p2d); // Should return original vector

        let p3d = TPoint3D::new(0.0, 0.0, 0.0);
        let normalized = p3d.normalize();
        assert_eq!(normalized, p3d); // Should return original vector
    }

    #[test]
    fn test_component_wise_multiplication() {
        let p1 = Point2D::new(2, 3);
        let p2 = Point2D::new(4, 5);
        let result = p1 * p2;
        assert_eq!(result, Point2D::new(8, 15));

        let p3d1 = Point3D::new(2, 3, 4);
        let p3d2 = Point3D::new(5, 6, 7);
        let result3d = p3d1 * p3d2;
        assert_eq!(result3d, Point3D::new(10, 18, 28));
    }

    #[test]
    fn test_equality_and_inequality() {
        let p1 = Point2D::new(1, 2);
        let p2 = Point2D::new(1, 2);
        let p3 = Point2D::new(2, 3);

        assert_eq!(p1, p2);
        assert_ne!(p1, p3);

        let p3d1 = Point3D::new(1, 2, 3);
        let p3d2 = Point3D::new(1, 2, 3);
        let p3d3 = Point3D::new(1, 2, 4);

        assert_eq!(p3d1, p3d2);
        assert_ne!(p3d1, p3d3);
    }
}
