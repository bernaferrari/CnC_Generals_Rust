use super::{Matrix3D, Vector3, WWMath};
use crate::EPSILON;

/// A 3D line segment defined by two endpoints
///
/// The line segment is represented by start and end points, with cached
/// direction vector, normalized direction, and length for efficiency.
#[derive(Debug, Clone, PartialEq)]
pub struct LineSegment {
    p0: Vector3,        // Start point
    p1: Vector3,        // End point
    dp: Vector3,        // Difference vector (p1 - p0)
    direction: Vector3, // Normalized direction
    length: f32,        // Length of the segment
}

impl Default for LineSegment {
    fn default() -> Self {
        Self {
            p0: Vector3::ZERO,
            p1: Vector3::ZERO,
            dp: Vector3::ZERO,
            direction: Vector3::new(1.0, 0.0, 0.0),
            length: 0.0,
        }
    }
}

impl LineSegment {
    /// Create a new line segment from two points
    pub fn new(p0: Vector3, p1: Vector3) -> Self {
        let mut segment = Self {
            p0,
            p1,
            dp: Vector3::ZERO,
            direction: Vector3::ZERO,
            length: 0.0,
        };
        segment.recalculate();
        segment
    }

    /// Create a line segment by transforming another line segment
    pub fn from_transformed(other: &LineSegment, transform: &Matrix3D) -> Self {
        let mut segment = Self::default();
        segment.set_transformed(other, transform);
        segment
    }

    /// Set the line segment from two points
    pub fn set(&mut self, p0: Vector3, p1: Vector3) {
        self.p0 = p0;
        self.p1 = p1;
        self.recalculate();
    }

    /// Set the line segment by transforming another line segment
    pub fn set_transformed(&mut self, other: &LineSegment, transform: &Matrix3D) {
        // Transform endpoints
        self.p0 = transform.transform_vector(other.p0);
        self.p1 = transform.transform_vector(other.p1);

        // Calculate difference vector
        self.dp = self.p1 - self.p0;

        // Rotate the direction vector (assuming orthogonal transform)
        self.direction = transform.transform_vector(other.direction);

        // Length should be unchanged for orthogonal transforms
        self.length = other.length;
    }

    /// Set the line segment to random points within the given bounds
    pub fn set_random(&mut self, min: Vector3, max: Vector3) {
        let mut frac = WWMath::random_float();
        self.p0.x = min.x + frac * (max.x - min.x);
        frac = WWMath::random_float();
        self.p0.y = min.y + frac * (max.y - min.y);
        frac = WWMath::random_float();
        self.p0.z = min.z + frac * (max.z - min.z);

        frac = WWMath::random_float();
        self.p1.x = min.x + frac * (max.x - min.x);
        frac = WWMath::random_float();
        self.p1.y = min.y + frac * (max.y - min.y);
        frac = WWMath::random_float();
        self.p1.z = min.z + frac * (max.z - min.z);

        self.recalculate();
    }

    /// Get the start point
    pub fn get_p0(&self) -> Vector3 {
        self.p0
    }

    /// Get the end point
    pub fn get_p1(&self) -> Vector3 {
        self.p1
    }

    /// Get the start point (alias for compatibility)
    pub fn start(&self) -> Vector3 {
        self.p0
    }

    /// Get the end point (alias for compatibility)
    pub fn end(&self) -> Vector3 {
        self.p1
    }

    /// Get the difference vector (p1 - p0)
    pub fn get_dp(&self) -> Vector3 {
        self.dp
    }

    /// Get the normalized direction vector
    pub fn get_direction(&self) -> Vector3 {
        self.direction
    }

    /// Get the length of the segment
    pub fn get_length(&self) -> f32 {
        self.length
    }

    /// Compute a point along the line segment at parameter t
    /// t = 0.0 gives p0, t = 1.0 gives p1
    pub fn compute_point(&self, t: f32) -> Vector3 {
        self.p0 + self.dp * t
    }

    /// Find the point on the line segment closest to a given position
    pub fn find_point_closest_to(&self, pos: Vector3) -> Vector3 {
        let v_0_pos = pos - self.p0;
        let dot_product = self.direction.dot(v_0_pos);

        // Check if point is past either endpoint
        if dot_product <= 0.0 {
            self.p0
        } else if dot_product >= self.length {
            self.p1
        } else {
            // Find point on line segment closest to pos
            self.p0 + self.direction * dot_product
        }
    }

    /// Find the closest points between this line segment and another
    /// Returns (success, point1, fraction1, point2, fraction2)
    /// where point1 is on this line, point2 is on the other line,
    /// and fractions are the parameters along each line
    pub fn find_intersection(&self, other: &LineSegment) -> (bool, Vector3, f32, Vector3, f32) {
        let cross1 = self.direction.cross(other.direction);
        let cross2 = (other.p0 - self.p0).cross(other.direction);
        let top1 = cross2.dot(cross1);
        let bottom1 = cross1.dot(cross1);

        let cross3 = other.direction.cross(self.direction);
        let cross4 = (self.p0 - other.p0).cross(self.direction);
        let top2 = cross4.dot(cross3);
        let bottom2 = cross3.dot(cross3);

        // If either divisor is 0, the lines are parallel
        if bottom1.abs() < EPSILON || bottom2.abs() < EPSILON {
            return (false, Vector3::ZERO, 0.0, Vector3::ZERO, 0.0);
        }

        let length1 = top1 / bottom1;
        let length2 = top2 / bottom2;

        // Calculate closest points on both lines
        let p1 = self.p0 + self.direction * length1;
        let p2 = other.p0 + other.direction * length2;

        // Convert to fractions along the line segments
        let fraction1 = if self.length > EPSILON {
            length1 / self.length
        } else {
            0.0
        };
        let fraction2 = if other.length > EPSILON {
            length2 / other.length
        } else {
            0.0
        };

        (true, p1, fraction1, p2, fraction2)
    }

    /// Get the distance between this line segment and a point
    pub fn distance_to_point(&self, point: Vector3) -> f32 {
        let closest = self.find_point_closest_to(point);
        (point - closest).length()
    }

    /// Test if a point is within a given distance of the line segment
    pub fn is_point_within_distance(&self, point: Vector3, distance: f32) -> bool {
        self.distance_to_point(point) <= distance
    }

    /// Recalculate cached values after changing endpoints
    fn recalculate(&mut self) {
        self.dp = self.p1 - self.p0;
        self.length = self.dp.length();

        if self.length > EPSILON {
            self.direction = self.dp / self.length;
        } else {
            self.direction = Vector3::new(1.0, 0.0, 0.0); // Default direction
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_line_segment_new() {
        let p0 = Vector3::new(0.0, 0.0, 0.0);
        let p1 = Vector3::new(3.0, 4.0, 0.0);
        let segment = LineSegment::new(p0, p1);

        assert_eq!(segment.get_p0(), p0);
        assert_eq!(segment.get_p1(), p1);
        assert_eq!(segment.get_length(), 5.0);
    }

    #[test]
    fn test_line_segment_compute_point() {
        let segment = LineSegment::new(Vector3::new(0.0, 0.0, 0.0), Vector3::new(10.0, 0.0, 0.0));

        assert_eq!(segment.compute_point(0.0), Vector3::new(0.0, 0.0, 0.0));
        assert_eq!(segment.compute_point(1.0), Vector3::new(10.0, 0.0, 0.0));
        assert_eq!(segment.compute_point(0.5), Vector3::new(5.0, 0.0, 0.0));
    }

    #[test]
    fn test_line_segment_find_point_closest_to() {
        let segment = LineSegment::new(Vector3::new(0.0, 0.0, 0.0), Vector3::new(10.0, 0.0, 0.0));

        // Point directly above middle of segment
        let closest = segment.find_point_closest_to(Vector3::new(5.0, 5.0, 0.0));
        assert_eq!(closest, Vector3::new(5.0, 0.0, 0.0));

        // Point before start
        let closest = segment.find_point_closest_to(Vector3::new(-5.0, 0.0, 0.0));
        assert_eq!(closest, Vector3::new(0.0, 0.0, 0.0));

        // Point after end
        let closest = segment.find_point_closest_to(Vector3::new(15.0, 0.0, 0.0));
        assert_eq!(closest, Vector3::new(10.0, 0.0, 0.0));
    }

    #[test]
    fn test_line_segment_distance_to_point() {
        let segment = LineSegment::new(Vector3::new(0.0, 0.0, 0.0), Vector3::new(10.0, 0.0, 0.0));

        // Point on the line
        assert!((segment.distance_to_point(Vector3::new(5.0, 0.0, 0.0)) - 0.0).abs() < EPSILON);

        // Point perpendicular to line
        assert!((segment.distance_to_point(Vector3::new(5.0, 3.0, 0.0)) - 3.0).abs() < EPSILON);
    }

    #[test]
    fn test_line_segment_set() {
        let mut segment = LineSegment::default();
        let p0 = Vector3::new(1.0, 2.0, 3.0);
        let p1 = Vector3::new(4.0, 6.0, 8.0);

        segment.set(p0, p1);

        assert_eq!(segment.get_p0(), p0);
        assert_eq!(segment.get_p1(), p1);
        assert_eq!(segment.get_dp(), p1 - p0);
    }

    #[test]
    fn test_line_segment_find_intersection() {
        // Two perpendicular line segments that intersect
        let segment1 = LineSegment::new(Vector3::new(-1.0, 0.0, 0.0), Vector3::new(1.0, 0.0, 0.0));
        let segment2 = LineSegment::new(Vector3::new(0.0, -1.0, 0.0), Vector3::new(0.0, 1.0, 0.0));

        let (success, p1, t1, p2, t2) = segment1.find_intersection(&segment2);

        assert!(success);
        assert!((p1 - Vector3::new(0.0, 0.0, 0.0)).length() < EPSILON);
        assert!((p2 - Vector3::new(0.0, 0.0, 0.0)).length() < EPSILON);
        assert!((t1 - 0.5).abs() < EPSILON);
        assert!((t2 - 0.5).abs() < EPSILON);
    }

    #[test]
    fn test_line_segment_is_point_within_distance() {
        let segment = LineSegment::new(Vector3::new(0.0, 0.0, 0.0), Vector3::new(10.0, 0.0, 0.0));

        assert!(segment.is_point_within_distance(Vector3::new(5.0, 1.0, 0.0), 2.0));
        assert!(!segment.is_point_within_distance(Vector3::new(5.0, 1.0, 0.0), 0.5));
    }
}
