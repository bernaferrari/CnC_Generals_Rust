use super::Vector2;
use std::ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Sub, SubAssign};

/// A 2D axis-aligned rectangle defined by left, top, right, bottom coordinates
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct Rect {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

impl Default for Rect {
    fn default() -> Self {
        Self {
            left: 0.0,
            top: 0.0,
            right: 0.0,
            bottom: 0.0,
        }
    }
}

impl Rect {
    /// Create a new rectangle with the given coordinates
    pub fn new(left: f32, top: f32, right: f32, bottom: f32) -> Self {
        Self {
            left,
            top,
            right,
            bottom,
        }
    }

    /// Create a rectangle from two corner points
    pub fn from_corners(top_left: Vector2, bottom_right: Vector2) -> Self {
        Self {
            left: top_left.x,
            top: top_left.y,
            right: bottom_right.x,
            bottom: bottom_right.y,
        }
    }

    /// Create a rectangle from center point and size
    pub fn from_center_size(center: Vector2, size: Vector2) -> Self {
        let half_size = size * 0.5;
        Self {
            left: center.x - half_size.x,
            top: center.y - half_size.y,
            right: center.x + half_size.x,
            bottom: center.y + half_size.y,
        }
    }

    /// Set the rectangle coordinates
    pub fn set(&mut self, left: f32, top: f32, right: f32, bottom: f32) {
        self.left = left;
        self.top = top;
        self.right = right;
        self.bottom = bottom;
    }

    /// Set the rectangle from two corner points
    pub fn set_from_corners(&mut self, top_left: Vector2, bottom_right: Vector2) {
        self.left = top_left.x;
        self.top = top_left.y;
        self.right = bottom_right.x;
        self.bottom = bottom_right.y;
    }

    /// Set from another rectangle
    pub fn set_from_rect(&mut self, other: &Rect) {
        self.left = other.left;
        self.top = other.top;
        self.right = other.right;
        self.bottom = other.bottom;
    }

    /// Get the width of the rectangle
    pub fn width(&self) -> f32 {
        self.right - self.left
    }

    /// Get the height of the rectangle
    pub fn height(&self) -> f32 {
        self.bottom - self.top
    }

    /// Get the center point of the rectangle
    pub fn center(&self) -> Vector2 {
        Vector2::new(
            (self.left + self.right) * 0.5,
            (self.top + self.bottom) * 0.5,
        )
    }

    /// Get the extent (half-size) of the rectangle
    pub fn extent(&self) -> Vector2 {
        Vector2::new(
            (self.right - self.left) * 0.5,
            (self.bottom - self.top) * 0.5,
        )
    }

    /// Get the upper-left corner
    pub fn upper_left(&self) -> Vector2 {
        Vector2::new(self.left, self.top)
    }

    /// Get the lower-right corner
    pub fn lower_right(&self) -> Vector2 {
        Vector2::new(self.right, self.bottom)
    }

    /// Get the upper-right corner
    pub fn upper_right(&self) -> Vector2 {
        Vector2::new(self.right, self.top)
    }

    /// Get the lower-left corner
    pub fn lower_left(&self) -> Vector2 {
        Vector2::new(self.left, self.bottom)
    }

    /// Scale the rectangle relative to its center
    pub fn scale_relative_center(&mut self, scale: f32) -> &mut Self {
        let center = self.center();
        *self -= center;
        self.left *= scale;
        self.top *= scale;
        self.right *= scale;
        self.bottom *= scale;
        *self += center;
        self
    }

    /// Scale the rectangle uniformly
    pub fn scale(&mut self, scale: f32) -> &mut Self {
        self.left *= scale;
        self.top *= scale;
        self.right *= scale;
        self.bottom *= scale;
        self
    }

    /// Scale the rectangle with different factors for X and Y
    pub fn scale_vec(&mut self, scale: Vector2) -> &mut Self {
        self.left *= scale.x;
        self.top *= scale.y;
        self.right *= scale.x;
        self.bottom *= scale.y;
        self
    }

    /// Scale the rectangle by the inverse of the given factors
    pub fn inverse_scale(&mut self, scale: Vector2) -> &mut Self {
        self.left /= scale.x;
        self.top /= scale.y;
        self.right /= scale.x;
        self.bottom /= scale.y;
        self
    }

    /// Inflate the rectangle by the given amount
    pub fn inflate(&mut self, offset: Vector2) {
        self.left -= offset.x;
        self.top -= offset.y;
        self.right += offset.x;
        self.bottom += offset.y;
    }

    /// Union this rectangle with another (expand to contain both)
    pub fn union_with(&mut self, other: &Rect) {
        self.left = self.left.min(other.left);
        self.top = self.top.min(other.top);
        self.right = self.right.max(other.right);
        self.bottom = self.bottom.max(other.bottom);
    }

    /// Test if this rectangle contains a point
    pub fn contains(&self, point: Vector2) -> bool {
        point.x >= self.left
            && point.x <= self.right
            && point.y >= self.top
            && point.y <= self.bottom
    }

    /// Test if this rectangle intersects with another rectangle
    pub fn intersects(&self, other: &Rect) -> bool {
        !(self.right < other.left
            || self.left > other.right
            || self.bottom < other.top
            || self.top > other.bottom)
    }

    /// Get the intersection of this rectangle with another
    pub fn intersection(&self, other: &Rect) -> Option<Rect> {
        if !self.intersects(other) {
            return None;
        }

        Some(Rect {
            left: self.left.max(other.left),
            top: self.top.max(other.top),
            right: self.right.min(other.right),
            bottom: self.bottom.min(other.bottom),
        })
    }

    /// Test if this rectangle is completely inside another rectangle
    pub fn is_inside(&self, other: &Rect) -> bool {
        self.left >= other.left
            && self.top >= other.top
            && self.right <= other.right
            && self.bottom <= other.bottom
    }

    /// Get the area of the rectangle
    pub fn area(&self) -> f32 {
        self.width() * self.height()
    }

    /// Test if the rectangle is empty (zero or negative area)
    pub fn is_empty(&self) -> bool {
        self.right <= self.left || self.bottom <= self.top
    }

    /// Snap coordinates to unit grid
    pub fn snap_to_units(&mut self, units: Vector2) {
        self.left = (self.left / units.x + 0.5).floor() * units.x;
        self.right = (self.right / units.x + 0.5).floor() * units.x;
        self.top = (self.top / units.y + 0.5).floor() * units.y;
        self.bottom = (self.bottom / units.y + 0.5).floor() * units.y;
    }

    /// Normalize the rectangle (ensure left < right, top < bottom)
    pub fn normalize(&mut self) {
        if self.left > self.right {
            std::mem::swap(&mut self.left, &mut self.right);
        }
        if self.top > self.bottom {
            std::mem::swap(&mut self.top, &mut self.bottom);
        }
    }

    /// Get a normalized copy of the rectangle
    pub fn normalized(&self) -> Rect {
        let mut result = *self;
        result.normalize();
        result
    }
}

// Scaling operations
impl MulAssign<f32> for Rect {
    fn mul_assign(&mut self, scale: f32) {
        self.scale(scale);
    }
}

impl DivAssign<f32> for Rect {
    fn div_assign(&mut self, scale: f32) {
        self.scale(1.0 / scale);
    }
}

impl Mul<f32> for Rect {
    type Output = Rect;

    fn mul(mut self, scale: f32) -> Rect {
        self *= scale;
        self
    }
}

impl Div<f32> for Rect {
    type Output = Rect;

    fn div(mut self, scale: f32) -> Rect {
        self /= scale;
        self
    }
}

// Offset operations
impl AddAssign<Vector2> for Rect {
    fn add_assign(&mut self, offset: Vector2) {
        self.left += offset.x;
        self.top += offset.y;
        self.right += offset.x;
        self.bottom += offset.y;
    }
}

impl SubAssign<Vector2> for Rect {
    fn sub_assign(&mut self, offset: Vector2) {
        self.left -= offset.x;
        self.top -= offset.y;
        self.right -= offset.x;
        self.bottom -= offset.y;
    }
}

impl Add<Vector2> for Rect {
    type Output = Rect;

    fn add(mut self, offset: Vector2) -> Rect {
        self += offset;
        self
    }
}

impl Sub<Vector2> for Rect {
    type Output = Rect;

    fn sub(mut self, offset: Vector2) -> Rect {
        self -= offset;
        self
    }
}

// Union operation
impl AddAssign<Rect> for Rect {
    fn add_assign(&mut self, other: Rect) {
        self.union_with(&other);
    }
}

impl Add<Rect> for Rect {
    type Output = Rect;

    fn add(mut self, other: Rect) -> Rect {
        self += other;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rect_new() {
        let rect = Rect::new(1.0, 2.0, 5.0, 8.0);
        assert_eq!(rect.left, 1.0);
        assert_eq!(rect.top, 2.0);
        assert_eq!(rect.right, 5.0);
        assert_eq!(rect.bottom, 8.0);
    }

    #[test]
    fn test_rect_from_corners() {
        let top_left = Vector2::new(1.0, 2.0);
        let bottom_right = Vector2::new(5.0, 8.0);
        let rect = Rect::from_corners(top_left, bottom_right);

        assert_eq!(rect.left, 1.0);
        assert_eq!(rect.top, 2.0);
        assert_eq!(rect.right, 5.0);
        assert_eq!(rect.bottom, 8.0);
    }

    #[test]
    fn test_rect_from_center_size() {
        let center = Vector2::new(5.0, 5.0);
        let size = Vector2::new(4.0, 6.0);
        let rect = Rect::from_center_size(center, size);

        assert_eq!(rect.left, 3.0);
        assert_eq!(rect.top, 2.0);
        assert_eq!(rect.right, 7.0);
        assert_eq!(rect.bottom, 8.0);
    }

    #[test]
    fn test_rect_dimensions() {
        let rect = Rect::new(1.0, 2.0, 5.0, 8.0);

        assert_eq!(rect.width(), 4.0);
        assert_eq!(rect.height(), 6.0);
        assert_eq!(rect.area(), 24.0);
    }

    #[test]
    fn test_rect_center() {
        let rect = Rect::new(0.0, 0.0, 4.0, 6.0);
        let center = rect.center();

        assert_eq!(center, Vector2::new(2.0, 3.0));
    }

    #[test]
    fn test_rect_corners() {
        let rect = Rect::new(1.0, 2.0, 5.0, 8.0);

        assert_eq!(rect.upper_left(), Vector2::new(1.0, 2.0));
        assert_eq!(rect.upper_right(), Vector2::new(5.0, 2.0));
        assert_eq!(rect.lower_left(), Vector2::new(1.0, 8.0));
        assert_eq!(rect.lower_right(), Vector2::new(5.0, 8.0));
    }

    #[test]
    fn test_rect_contains() {
        let rect = Rect::new(0.0, 0.0, 10.0, 10.0);

        assert!(rect.contains(Vector2::new(5.0, 5.0)));
        assert!(rect.contains(Vector2::new(0.0, 0.0))); // Edge case
        assert!(rect.contains(Vector2::new(10.0, 10.0))); // Edge case
        assert!(!rect.contains(Vector2::new(-1.0, 5.0)));
        assert!(!rect.contains(Vector2::new(11.0, 5.0)));
    }

    #[test]
    fn test_rect_intersects() {
        let rect1 = Rect::new(0.0, 0.0, 5.0, 5.0);
        let rect2 = Rect::new(3.0, 3.0, 8.0, 8.0);
        let rect3 = Rect::new(10.0, 10.0, 15.0, 15.0);

        assert!(rect1.intersects(&rect2));
        assert!(!rect1.intersects(&rect3));
    }

    #[test]
    fn test_rect_intersection() {
        let rect1 = Rect::new(0.0, 0.0, 5.0, 5.0);
        let rect2 = Rect::new(3.0, 3.0, 8.0, 8.0);
        let rect3 = Rect::new(10.0, 10.0, 15.0, 15.0);

        let intersection = rect1.intersection(&rect2);
        assert!(intersection.is_some());
        let intersection = intersection.unwrap();
        assert_eq!(intersection, Rect::new(3.0, 3.0, 5.0, 5.0));

        assert!(rect1.intersection(&rect3).is_none());
    }

    #[test]
    fn test_rect_union() {
        let mut rect1 = Rect::new(0.0, 0.0, 3.0, 3.0);
        let rect2 = Rect::new(2.0, 2.0, 5.0, 5.0);

        rect1.union_with(&rect2);
        assert_eq!(rect1, Rect::new(0.0, 0.0, 5.0, 5.0));
    }

    #[test]
    fn test_rect_scale() {
        let mut rect = Rect::new(1.0, 2.0, 3.0, 4.0);
        rect.scale(2.0);

        assert_eq!(rect, Rect::new(2.0, 4.0, 6.0, 8.0));
    }

    #[test]
    fn test_rect_offset() {
        let mut rect = Rect::new(1.0, 2.0, 3.0, 4.0);
        rect += Vector2::new(10.0, 20.0);

        assert_eq!(rect, Rect::new(11.0, 22.0, 13.0, 24.0));
    }

    #[test]
    fn test_rect_inflate() {
        let mut rect = Rect::new(5.0, 5.0, 15.0, 15.0);
        rect.inflate(Vector2::new(2.0, 3.0));

        assert_eq!(rect, Rect::new(3.0, 2.0, 17.0, 18.0));
    }

    #[test]
    fn test_rect_is_inside() {
        let outer = Rect::new(0.0, 0.0, 10.0, 10.0);
        let inner = Rect::new(2.0, 2.0, 8.0, 8.0);
        let overlapping = Rect::new(-1.0, -1.0, 5.0, 5.0);

        assert!(inner.is_inside(&outer));
        assert!(!overlapping.is_inside(&outer));
    }

    #[test]
    fn test_rect_is_empty() {
        let empty1 = Rect::new(5.0, 5.0, 5.0, 5.0);
        let empty2 = Rect::new(5.0, 5.0, 3.0, 8.0); // negative width
        let not_empty = Rect::new(0.0, 0.0, 1.0, 1.0);

        assert!(empty1.is_empty());
        assert!(empty2.is_empty());
        assert!(!not_empty.is_empty());
    }

    #[test]
    fn test_rect_normalize() {
        let mut rect = Rect::new(5.0, 8.0, 2.0, 3.0); // inverted
        rect.normalize();

        assert_eq!(rect, Rect::new(2.0, 3.0, 5.0, 8.0));
    }

    #[test]
    fn test_rect_snap_to_units() {
        let mut rect = Rect::new(1.3, 2.7, 5.4, 8.9);
        rect.snap_to_units(Vector2::new(1.0, 1.0));

        assert_eq!(rect, Rect::new(1.0, 3.0, 5.0, 9.0));
    }

    #[test]
    fn test_rect_equality() {
        let rect1 = Rect::new(1.0, 2.0, 3.0, 4.0);
        let rect2 = Rect::new(1.0, 2.0, 3.0, 4.0);
        let rect3 = Rect::new(1.0, 2.0, 3.0, 5.0);

        assert_eq!(rect1, rect2);
        assert_ne!(rect1, rect3);
    }

    #[test]
    fn test_rect_operator_overloads() {
        let rect1 = Rect::new(1.0, 2.0, 3.0, 4.0);
        let rect2 = Rect::new(0.0, 1.0, 5.0, 6.0);
        let offset = Vector2::new(10.0, 20.0);

        // Test union operator
        let union = rect1 + rect2;
        assert_eq!(union, Rect::new(0.0, 1.0, 5.0, 6.0));

        // Test offset operators
        let offset_rect = rect1 + offset;
        assert_eq!(offset_rect, Rect::new(11.0, 22.0, 13.0, 24.0));

        // Test scale operators
        let scaled_rect = rect1 * 2.0;
        assert_eq!(scaled_rect, Rect::new(2.0, 4.0, 6.0, 8.0));
    }
}
