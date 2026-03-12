//! Bezier Curve System for Projectile Flight Paths
//!
//! Implements cubic Bezier curves matching C++ BezierSegment.cpp behavior.
//! Used by DumbProjectileBehavior for artillery shells and mortar rounds.

use crate::common::{Coord3D, Real};
use std::f32::consts::PI;

/// Cubic Bezier curve segment defined by 4 control points
/// Matches C++ BezierSegment class from Common/BezierSegment.h
#[derive(Debug, Clone)]
pub struct BezierSegment {
    /// Four control points: P0 (start), P1, P2, P3 (end)
    pub control_points: [Coord3D; 4],
    /// Cached approximate arc length
    arc_length: Real,
}

impl BezierSegment {
    /// Create new Bezier segment from 4 control points
    /// Matches C++ BezierSegment constructor
    pub fn new(control_points: [Coord3D; 4]) -> Self {
        let mut segment = Self {
            control_points,
            arc_length: 0.0,
        };
        segment.arc_length = segment.calculate_approximate_length();
        segment
    }

    /// Evaluate Bezier curve at parameter t (0.0 to 1.0)
    /// Matches C++ BezierSegment::Evaluate()
    ///
    /// Uses De Casteljau's algorithm for numerical stability:
    /// B(t) = (1-t)³P₀ + 3(1-t)²tP₁ + 3(1-t)t²P₂ + t³P₃
    pub fn evaluate(&self, t: Real) -> Coord3D {
        let t = t.clamp(0.0, 1.0);
        let one_minus_t = 1.0 - t;

        // Bernstein basis functions
        let b0 = one_minus_t * one_minus_t * one_minus_t;
        let b1 = 3.0 * one_minus_t * one_minus_t * t;
        let b2 = 3.0 * one_minus_t * t * t;
        let b3 = t * t * t;

        Coord3D::new(
            b0 * self.control_points[0].x
                + b1 * self.control_points[1].x
                + b2 * self.control_points[2].x
                + b3 * self.control_points[3].x,
            b0 * self.control_points[0].y
                + b1 * self.control_points[1].y
                + b2 * self.control_points[2].y
                + b3 * self.control_points[3].y,
            b0 * self.control_points[0].z
                + b1 * self.control_points[1].z
                + b2 * self.control_points[2].z
                + b3 * self.control_points[3].z,
        )
    }

    /// Get tangent vector at parameter t
    /// Matches C++ derivative calculation
    ///
    /// B'(t) = 3(1-t)²(P₁-P₀) + 6(1-t)t(P₂-P₁) + 3t²(P₃-P₂)
    pub fn get_tangent(&self, t: Real) -> Coord3D {
        let t = t.clamp(0.0, 1.0);
        let one_minus_t = 1.0 - t;

        let c0 = 3.0 * one_minus_t * one_minus_t;
        let c1 = 6.0 * one_minus_t * t;
        let c2 = 3.0 * t * t;

        let d0x = self.control_points[1].x - self.control_points[0].x;
        let d0y = self.control_points[1].y - self.control_points[0].y;
        let d0z = self.control_points[1].z - self.control_points[0].z;

        let d1x = self.control_points[2].x - self.control_points[1].x;
        let d1y = self.control_points[2].y - self.control_points[1].y;
        let d1z = self.control_points[2].z - self.control_points[1].z;

        let d2x = self.control_points[3].x - self.control_points[2].x;
        let d2y = self.control_points[3].y - self.control_points[2].y;
        let d2z = self.control_points[3].z - self.control_points[2].z;

        Coord3D::new(
            c0 * d0x + c1 * d1x + c2 * d2x,
            c0 * d0y + c1 * d1y + c2 * d2y,
            c0 * d0z + c1 * d1z + c2 * d2z,
        )
    }

    /// Calculate approximate arc length using adaptive sampling
    /// Matches C++ BezierSegment::getApproximateLength()
    fn calculate_approximate_length(&self) -> Real {
        const NUM_SAMPLES: usize = 20;
        let mut length = 0.0;
        let mut prev_point = self.evaluate(0.0);

        for i in 1..=NUM_SAMPLES {
            let t = (i as Real) / (NUM_SAMPLES as Real);
            let curr_point = self.evaluate(t);

            let dx = curr_point.x - prev_point.x;
            let dy = curr_point.y - prev_point.y;
            let dz = curr_point.z - prev_point.z;

            length += (dx * dx + dy * dy + dz * dz).sqrt();
            prev_point = curr_point;
        }

        length
    }

    /// Get approximate arc length
    /// Matches C++ BezierSegment::getApproximateLength()
    pub fn get_approximate_length(&self) -> Real {
        self.arc_length
    }

    /// Generate evenly spaced points along curve
    /// Matches C++ BezierSegment::getSegmentPoints()
    pub fn get_segment_points(&self, num_points: usize) -> Vec<Coord3D> {
        let mut points = Vec::with_capacity(num_points);

        if num_points == 0 {
            return points;
        }

        if num_points == 1 {
            points.push(self.evaluate(0.0));
            return points;
        }

        for i in 0..num_points {
            let t = (i as Real) / ((num_points - 1) as Real);
            points.push(self.evaluate(t));
        }

        points
    }

    /// Calculate control points for projectile arc with specified heights
    /// Matches C++ DumbProjectileBehavior::calcFlightPath() algorithm
    ///
    /// # Arguments
    /// * `start` - Starting position
    /// * `end` - Target position
    /// * `first_height` - Additional height at first control point
    /// * `second_height` - Additional height at second control point
    /// * `first_percent_indent` - First control point position (0.0-1.0) along path
    /// * `second_percent_indent` - Second control point position (0.0-1.0) along path
    /// * `highest_terrain` - Highest terrain height along path (to avoid collision)
    pub fn create_projectile_arc(
        start: Coord3D,
        end: Coord3D,
        first_height: Real,
        second_height: Real,
        first_percent_indent: Real,
        second_percent_indent: Real,
        highest_terrain: Real,
    ) -> Self {
        let mut control_points = [Coord3D::default(); 4];

        // Start and end points (C++ lines 389-390)
        control_points[0] = start;
        control_points[3] = end;

        // Calculate vector from start to end (C++ lines 401-407)
        let target_vector = Coord3D::new(end.x - start.x, end.y - start.y, end.z - start.z);

        let target_distance = (target_vector.x * target_vector.x
            + target_vector.y * target_vector.y
            + target_vector.z * target_vector.z)
            .sqrt();

        if target_distance < 0.001 {
            // Degenerate case - all points the same
            return Self::new(control_points);
        }

        // Normalize target vector (C++ line 407)
        let normalized = Coord3D::new(
            target_vector.x / target_distance,
            target_vector.y / target_distance,
            target_vector.z / target_distance,
        );

        // Calculate intermediate points along horizontal path (C++ lines 408-414)
        let first_point_along = Coord3D::new(
            normalized.x * (target_distance * first_percent_indent),
            normalized.y * (target_distance * first_percent_indent),
            normalized.z * (target_distance * first_percent_indent),
        );

        let second_point_along = Coord3D::new(
            normalized.x * (target_distance * second_percent_indent),
            normalized.y * (target_distance * second_percent_indent),
            normalized.z * (target_distance * second_percent_indent),
        );

        // Set X and Y coordinates (C++ lines 411-414)
        control_points[1].x = first_point_along.x + start.x;
        control_points[1].y = first_point_along.y + start.y;
        control_points[2].x = second_point_along.x + start.x;
        control_points[2].y = second_point_along.y + start.y;

        // Calculate Z heights to clear terrain (C++ lines 416-420)
        let safe_height = highest_terrain.max(start.z).max(end.z);
        control_points[1].z = safe_height + first_height;
        control_points[2].z = safe_height + second_height;

        Self::new(control_points)
    }

    /// Split curve at parameter t into two curves
    /// Uses De Casteljau subdivision algorithm
    pub fn subdivide(&self, t: Real) -> (BezierSegment, BezierSegment) {
        let t = t.clamp(0.0, 1.0);
        let one_minus_t = 1.0 - t;

        // First level interpolation
        let p01 = lerp_coord(&self.control_points[0], &self.control_points[1], t);
        let p12 = lerp_coord(&self.control_points[1], &self.control_points[2], t);
        let p23 = lerp_coord(&self.control_points[2], &self.control_points[3], t);

        // Second level interpolation
        let p012 = lerp_coord(&p01, &p12, t);
        let p123 = lerp_coord(&p12, &p23, t);

        // Third level interpolation - the split point
        let p0123 = lerp_coord(&p012, &p123, t);

        let first = BezierSegment::new([self.control_points[0], p01, p012, p0123]);

        let second = BezierSegment::new([p0123, p123, p23, self.control_points[3]]);

        (first, second)
    }

    /// Get bounding box of the curve
    pub fn get_bounds(&self) -> (Coord3D, Coord3D) {
        let mut min = self.control_points[0];
        let mut max = self.control_points[0];

        for point in &self.control_points[1..] {
            min.x = min.x.min(point.x);
            min.y = min.y.min(point.y);
            min.z = min.z.min(point.z);

            max.x = max.x.max(point.x);
            max.y = max.y.max(point.y);
            max.z = max.z.max(point.z);
        }

        (min, max)
    }

    /// Find parameter t for closest point on curve to given position
    /// Uses Newton-Raphson iteration
    pub fn closest_parameter(&self, pos: &Coord3D, iterations: usize) -> Real {
        let mut best_t = 0.5;
        let mut best_dist_sq = Real::MAX;

        // Sample curve to find approximate closest point
        const SAMPLES: usize = 10;
        for i in 0..=SAMPLES {
            let t = (i as Real) / (SAMPLES as Real);
            let point = self.evaluate(t);

            let dx = point.x - pos.x;
            let dy = point.y - pos.y;
            let dz = point.z - pos.z;
            let dist_sq = dx * dx + dy * dy + dz * dz;

            if dist_sq < best_dist_sq {
                best_dist_sq = dist_sq;
                best_t = t;
            }
        }

        // Refine with Newton-Raphson
        let mut t = best_t;
        for _ in 0..iterations {
            let point = self.evaluate(t);
            let tangent = self.get_tangent(t);

            let dx = point.x - pos.x;
            let dy = point.y - pos.y;
            let dz = point.z - pos.z;

            // Derivative of distance squared
            let f = 2.0 * (dx * tangent.x + dy * tangent.y + dz * tangent.z);

            if f.abs() < 0.0001 {
                break;
            }

            // Second derivative approximation
            let dt = 0.001;
            let t2 = (t + dt).min(1.0);
            let point2 = self.evaluate(t2);
            let tangent2 = self.get_tangent(t2);

            let dx2 = point2.x - pos.x;
            let dy2 = point2.y - pos.y;
            let dz2 = point2.z - pos.z;

            let f2 = 2.0 * (dx2 * tangent2.x + dy2 * tangent2.y + dz2 * tangent2.z);
            let df = (f2 - f) / dt;

            if df.abs() > 0.0001 {
                t -= f / df;
                t = t.clamp(0.0, 1.0);
            } else {
                break;
            }
        }

        t
    }
}

/// Linear interpolation between two coordinates
#[inline]
fn lerp_coord(a: &Coord3D, b: &Coord3D, t: Real) -> Coord3D {
    Coord3D::new(
        a.x + (b.x - a.x) * t,
        a.y + (b.y - a.y) * t,
        a.z + (b.z - a.z) * t,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bezier_endpoints() {
        let p0 = Coord3D::new(0.0, 0.0, 0.0);
        let p1 = Coord3D::new(10.0, 0.0, 20.0);
        let p2 = Coord3D::new(20.0, 0.0, 20.0);
        let p3 = Coord3D::new(30.0, 0.0, 0.0);

        let bezier = BezierSegment::new([p0, p1, p2, p3]);

        let start = bezier.evaluate(0.0);
        let end = bezier.evaluate(1.0);

        assert!((start.x - p0.x).abs() < 0.001);
        assert!((end.x - p3.x).abs() < 0.001);
    }

    #[test]
    fn test_bezier_midpoint() {
        let p0 = Coord3D::new(0.0, 0.0, 0.0);
        let p1 = Coord3D::new(10.0, 0.0, 10.0);
        let p2 = Coord3D::new(20.0, 0.0, 10.0);
        let p3 = Coord3D::new(30.0, 0.0, 0.0);

        let bezier = BezierSegment::new([p0, p1, p2, p3]);
        let mid = bezier.evaluate(0.5);

        // Midpoint should be near center
        assert!(mid.x > 10.0 && mid.x < 20.0);
        assert!(mid.z >= 0.0);
    }

    #[test]
    fn test_bezier_tangent() {
        let p0 = Coord3D::new(0.0, 0.0, 0.0);
        let p1 = Coord3D::new(10.0, 0.0, 0.0);
        let p2 = Coord3D::new(20.0, 0.0, 0.0);
        let p3 = Coord3D::new(30.0, 0.0, 0.0);

        let bezier = BezierSegment::new([p0, p1, p2, p3]);
        let tangent = bezier.get_tangent(0.5);

        // For a straight line, tangent should point along X axis
        assert!(tangent.x > 0.0);
        assert!(tangent.y.abs() < 0.001);
    }

    #[test]
    fn test_segment_points() {
        let p0 = Coord3D::new(0.0, 0.0, 0.0);
        let p1 = Coord3D::new(10.0, 0.0, 10.0);
        let p2 = Coord3D::new(20.0, 0.0, 10.0);
        let p3 = Coord3D::new(30.0, 0.0, 0.0);

        let bezier = BezierSegment::new([p0, p1, p2, p3]);
        let points = bezier.get_segment_points(11);

        assert_eq!(points.len(), 11);
        assert!((points[0].x - 0.0).abs() < 0.001);
        assert!((points[10].x - 30.0).abs() < 0.001);
    }

    #[test]
    fn test_projectile_arc_creation() {
        let start = Coord3D::new(0.0, 0.0, 0.0);
        let end = Coord3D::new(100.0, 0.0, 0.0);

        let arc = BezierSegment::create_projectile_arc(
            start, end, 20.0, // first_height
            30.0, // second_height
            0.33, // first_percent_indent
            0.66, // second_percent_indent
            0.0,  // highest_terrain
        );

        // Verify control points
        assert_eq!(arc.control_points[0], start);
        assert_eq!(arc.control_points[3], end);

        // Middle points should have height
        assert!(arc.control_points[1].z > 0.0);
        assert!(arc.control_points[2].z > 0.0);

        // Verify arc has length
        assert!(arc.get_approximate_length() > 100.0);
    }

    #[test]
    fn test_arc_clears_terrain() {
        let start = Coord3D::new(0.0, 0.0, 10.0);
        let end = Coord3D::new(100.0, 0.0, 10.0);
        let terrain_height = 25.0;

        let arc =
            BezierSegment::create_projectile_arc(start, end, 5.0, 5.0, 0.33, 0.66, terrain_height);

        // Arc control points (P1/P2) should be above the highest terrain along the path.
        // Start/end are fixed to projectile origin/impact and are not raised.
        for (i, cp) in arc.control_points.iter().enumerate().skip(1).take(2) {
            assert!(
                cp.z >= terrain_height,
                "Control point {} at z={} should be above terrain at {}",
                i,
                cp.z,
                terrain_height
            );
        }
    }

    #[test]
    fn test_arc_length_positive() {
        let p0 = Coord3D::new(0.0, 0.0, 0.0);
        let p1 = Coord3D::new(10.0, 0.0, 10.0);
        let p2 = Coord3D::new(20.0, 0.0, 10.0);
        let p3 = Coord3D::new(30.0, 0.0, 0.0);

        let bezier = BezierSegment::new([p0, p1, p2, p3]);

        assert!(bezier.get_approximate_length() > 0.0);
        assert!(bezier.get_approximate_length() >= 30.0); // At least straight-line distance
    }

    #[test]
    fn test_subdivision() {
        let p0 = Coord3D::new(0.0, 0.0, 0.0);
        let p1 = Coord3D::new(10.0, 0.0, 10.0);
        let p2 = Coord3D::new(20.0, 0.0, 10.0);
        let p3 = Coord3D::new(30.0, 0.0, 0.0);

        let bezier = BezierSegment::new([p0, p1, p2, p3]);
        let (first, second) = bezier.subdivide(0.5);

        // First curve should start at original start
        assert!((first.control_points[0].x - p0.x).abs() < 0.001);

        // Second curve should end at original end
        assert!((second.control_points[3].x - p3.x).abs() < 0.001);

        // Curves should connect
        assert!((first.control_points[3].x - second.control_points[0].x).abs() < 0.001);
    }

    #[test]
    fn test_degenerate_arc() {
        // Same start and end
        let pos = Coord3D::new(50.0, 50.0, 10.0);
        let arc = BezierSegment::create_projectile_arc(pos, pos, 5.0, 5.0, 0.33, 0.66, 10.0);

        // Should still create valid curve
        assert!(arc.get_approximate_length() >= 0.0);
    }
}
