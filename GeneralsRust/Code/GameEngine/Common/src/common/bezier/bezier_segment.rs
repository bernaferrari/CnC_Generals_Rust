////////////////////////////////////////////////////////////////////////////////
//                                                                            //
//  (c) 2001-2003 Electronic Arts Inc.                                       //
//                                                                            //
////////////////////////////////////////////////////////////////////////////////

use super::bez_fwd_iterator::BezFwdIterator;

// Type aliases to match C++ types
pub type Real = f32;
pub type Int = i32;
pub type Bool = bool;

const USUAL_TOLERANCE: Real = 1.0;

// 3D coordinate structure
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Coord3D {
    pub x: Real,
    pub y: Real,
    pub z: Real,
}

impl Coord3D {
    pub fn new(x: Real, y: Real, z: Real) -> Self {
        Self { x, y, z }
    }

    pub fn zero(&mut self) {
        self.x = 0.0;
        self.y = 0.0;
        self.z = 0.0;
    }

    pub fn add(&mut self, other: &Coord3D) {
        self.x += other.x;
        self.y += other.y;
        self.z += other.z;
    }

    pub fn scale(&mut self, factor: Real) {
        self.x *= factor;
        self.y *= factor;
        self.z *= factor;
    }

    pub fn length(&self) -> Real {
        (self.x * self.x + self.y * self.y + self.z * self.z).sqrt()
    }
}

// 4D vector structure to replace D3DXVECTOR4
#[derive(Debug, Clone, Copy, Default)]
pub struct Vector4 {
    pub x: Real,
    pub y: Real,
    pub z: Real,
    pub w: Real,
}

impl Vector4 {
    pub fn new(x: Real, y: Real, z: Real, w: Real) -> Self {
        Self { x, y, z, w }
    }

    pub fn dot(&self, other: &Vector4) -> Real {
        self.x * other.x + self.y * other.y + self.z * other.z + self.w * other.w
    }
}

// 4x4 matrix structure to replace D3DXMATRIX
#[derive(Debug, Clone, Copy)]
pub struct Matrix4x4 {
    pub m: [[Real; 4]; 4],
}

impl Matrix4x4 {
    pub fn new(
        m00: Real,
        m01: Real,
        m02: Real,
        m03: Real,
        m10: Real,
        m11: Real,
        m12: Real,
        m13: Real,
        m20: Real,
        m21: Real,
        m22: Real,
        m23: Real,
        m30: Real,
        m31: Real,
        m32: Real,
        m33: Real,
    ) -> Self {
        Self {
            m: [
                [m00, m01, m02, m03],
                [m10, m11, m12, m13],
                [m20, m21, m22, m23],
                [m30, m31, m32, m33],
            ],
        }
    }

    // Transform a Vector4 by this matrix
    pub fn transform(&self, vec: &Vector4) -> Vector4 {
        Vector4::new(
            self.m[0][0] * vec.x
                + self.m[0][1] * vec.y
                + self.m[0][2] * vec.z
                + self.m[0][3] * vec.w,
            self.m[1][0] * vec.x
                + self.m[1][1] * vec.y
                + self.m[1][2] * vec.z
                + self.m[1][3] * vec.w,
            self.m[2][0] * vec.x
                + self.m[2][1] * vec.y
                + self.m[2][2] * vec.z
                + self.m[2][3] * vec.w,
            self.m[3][0] * vec.x
                + self.m[3][1] * vec.y
                + self.m[3][2] * vec.z
                + self.m[3][3] * vec.w,
        )
    }
}

// Vector type for collections of Coord3D
pub type VecCoord3D = Vec<Coord3D>;

/// Bezier Segment implementation
///
/// John K McDonald, Jr.
/// September 2002
///
/// A cubic Bezier segment defined by 4 control points
#[derive(Debug, Clone)]
pub struct BezierSegment {
    pub(crate) control_points: [Coord3D; 4],
}

impl BezierSegment {
    // The Basis Matrix for a bezier segment
    const BEZ_BASIS_MATRIX: Matrix4x4 = Matrix4x4 {
        m: [
            [-1.0, 3.0, -3.0, 1.0],
            [3.0, -6.0, 3.0, 0.0],
            [-3.0, 3.0, 0.0, 0.0],
            [1.0, 0.0, 0.0, 0.0],
        ],
    };

    /// Default constructor - creates a bezier segment with all control points at origin
    pub fn new() -> Self {
        Self {
            control_points: [Coord3D::default(); 4],
        }
    }

    /// Constructor with individual coordinate values
    pub fn from_coordinates(
        x0: Real,
        y0: Real,
        z0: Real,
        x1: Real,
        y1: Real,
        z1: Real,
        x2: Real,
        y2: Real,
        z2: Real,
        x3: Real,
        y3: Real,
        z3: Real,
    ) -> Self {
        Self {
            control_points: [
                Coord3D::new(x0, y0, z0),
                Coord3D::new(x1, y1, z1),
                Coord3D::new(x2, y2, z2),
                Coord3D::new(x3, y3, z3),
            ],
        }
    }

    /// Constructor from array of 12 Real values (x0, y0, z0, x1, y1, z1, ...)
    pub fn from_array(cp: &[Real; 12]) -> Self {
        Self {
            control_points: [
                Coord3D::new(cp[0], cp[1], cp[2]),
                Coord3D::new(cp[3], cp[4], cp[5]),
                Coord3D::new(cp[6], cp[7], cp[8]),
                Coord3D::new(cp[9], cp[10], cp[11]),
            ],
        }
    }

    /// Constructor from 4 Coord3D points
    pub fn from_points(cp0: Coord3D, cp1: Coord3D, cp2: Coord3D, cp3: Coord3D) -> Self {
        Self {
            control_points: [cp0, cp1, cp2, cp3],
        }
    }

    /// Constructor from array of 4 Coord3D points
    pub fn from_coord_array(cp: &[Coord3D; 4]) -> Self {
        Self {
            control_points: *cp,
        }
    }

    /// Evaluate the bezier segment at parameter t (0.0 to 1.0)
    pub fn evaluate_at_t(&self, t_value: Real) -> Option<Coord3D> {
        let t_vec = Vector4::new(t_value * t_value * t_value, t_value * t_value, t_value, 1.0);

        let x_coords = Vector4::new(
            self.control_points[0].x,
            self.control_points[1].x,
            self.control_points[2].x,
            self.control_points[3].x,
        );
        let y_coords = Vector4::new(
            self.control_points[0].y,
            self.control_points[1].y,
            self.control_points[2].y,
            self.control_points[3].y,
        );
        let z_coords = Vector4::new(
            self.control_points[0].z,
            self.control_points[1].z,
            self.control_points[2].z,
            self.control_points[3].z,
        );

        let t_result = Self::BEZ_BASIS_MATRIX.transform(&t_vec);

        Some(Coord3D::new(
            x_coords.dot(&t_result),
            y_coords.dot(&t_result),
            z_coords.dot(&t_result),
        ))
    }

    /// Generate a series of points along the bezier segment
    pub fn get_segment_points(&self, num_segments: Int) -> VecCoord3D {
        let mut result = Vec::with_capacity(num_segments as usize);

        if num_segments <= 0 {
            return result;
        }

        let mut iter = BezFwdIterator::new(num_segments, self);
        iter.start();

        while !iter.done() {
            result.push(iter.get_current());
            iter.next();
        }

        result
    }

    /// Get an approximation of the bezier segment's length
    /// This uses recursive subdivision to achieve the desired tolerance
    pub fn get_approximate_length(&self, within_tolerance: Real) -> Real {
        let p0p1 = Coord3D::new(
            self.control_points[1].x - self.control_points[0].x,
            self.control_points[1].y - self.control_points[0].y,
            self.control_points[1].z - self.control_points[0].z,
        );

        let p1p2 = Coord3D::new(
            self.control_points[2].x - self.control_points[1].x,
            self.control_points[2].y - self.control_points[1].y,
            self.control_points[2].z - self.control_points[1].z,
        );

        let p2p3 = Coord3D::new(
            self.control_points[3].x - self.control_points[2].x,
            self.control_points[3].y - self.control_points[2].y,
            self.control_points[3].z - self.control_points[2].z,
        );

        let p0p3 = Coord3D::new(
            self.control_points[3].x - self.control_points[0].x,
            self.control_points[3].y - self.control_points[0].y,
            self.control_points[3].z - self.control_points[0].z,
        );

        let length0 = p0p3.length();
        let length1 = p0p1.length() + p1p2.length() + p2p3.length();

        if (length1 - length0) > within_tolerance {
            let (seg1, seg2) = self.split_segment_at_t(0.5);
            seg1.get_approximate_length(within_tolerance)
                + seg2.get_approximate_length(within_tolerance)
        } else {
            (length0 + length1) / 2.0
        }
    }

    /// Get approximate length with default tolerance
    pub fn get_approximate_length_default(&self) -> Real {
        self.get_approximate_length(USUAL_TOLERANCE)
    }

    /// Split the bezier segment at parameter t into two segments
    pub fn split_segment_at_t(&self, t_value: Real) -> (BezierSegment, BezierSegment) {
        let mut p0p1 = Coord3D::new(
            self.control_points[1].x - self.control_points[0].x,
            self.control_points[1].y - self.control_points[0].y,
            self.control_points[1].z - self.control_points[0].z,
        );

        let mut p1p2 = Coord3D::new(
            self.control_points[2].x - self.control_points[1].x,
            self.control_points[2].y - self.control_points[1].y,
            self.control_points[2].z - self.control_points[1].z,
        );

        let mut p2p3 = Coord3D::new(
            self.control_points[3].x - self.control_points[2].x,
            self.control_points[3].y - self.control_points[2].y,
            self.control_points[3].z - self.control_points[2].z,
        );

        p0p1.scale(t_value);
        p1p2.scale(t_value);
        p2p3.scale(t_value);

        p0p1.add(&self.control_points[0]);
        p1p2.add(&self.control_points[1]);
        p2p3.add(&self.control_points[2]);

        let mut tri_left = Coord3D::new(p1p2.x - p0p1.x, p1p2.y - p0p1.y, p1p2.z - p0p1.z);

        let mut tri_right = Coord3D::new(p2p3.x - p1p2.x, p2p3.y - p1p2.y, p2p3.z - p1p2.z);

        tri_left.scale(t_value);
        tri_right.scale(t_value);

        tri_left.add(&p0p1);
        tri_right.add(&p1p2);

        let split_point = self.evaluate_at_t(t_value).unwrap_or_default();

        let out_seg1 =
            BezierSegment::from_points(self.control_points[0], p0p1, tri_left, split_point);

        let out_seg2 =
            BezierSegment::from_points(split_point, tri_right, p2p3, self.control_points[3]);

        (out_seg1, out_seg2)
    }

    /// Get a reference to the control points array
    pub fn control_points(&self) -> &[Coord3D; 4] {
        &self.control_points
    }

    /// Get a mutable reference to the control points array
    pub fn control_points_mut(&mut self) -> &mut [Coord3D; 4] {
        &mut self.control_points
    }
}

impl Default for BezierSegment {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bezier_segment_creation() {
        let segment = BezierSegment::new();
        assert_eq!(segment.control_points[0], Coord3D::default());
        assert_eq!(segment.control_points[3], Coord3D::default());
    }

    #[test]
    fn test_bezier_segment_from_coordinates() {
        let segment = BezierSegment::from_coordinates(
            0.0, 0.0, 0.0, 1.0, 1.0, 0.0, 2.0, 1.0, 0.0, 3.0, 0.0, 0.0,
        );

        assert_eq!(segment.control_points[0], Coord3D::new(0.0, 0.0, 0.0));
        assert_eq!(segment.control_points[1], Coord3D::new(1.0, 1.0, 0.0));
        assert_eq!(segment.control_points[2], Coord3D::new(2.0, 1.0, 0.0));
        assert_eq!(segment.control_points[3], Coord3D::new(3.0, 0.0, 0.0));
    }

    #[test]
    fn test_evaluate_at_t() {
        let segment = BezierSegment::from_coordinates(
            0.0, 0.0, 0.0, 1.0, 1.0, 0.0, 2.0, 1.0, 0.0, 3.0, 0.0, 0.0,
        );

        // At t=0, should be at first control point
        let result = segment.evaluate_at_t(0.0).unwrap();
        assert!((result.x - 0.0).abs() < 0.001);
        assert!((result.y - 0.0).abs() < 0.001);
        assert!((result.z - 0.0).abs() < 0.001);

        // At t=1, should be at last control point
        let result = segment.evaluate_at_t(1.0).unwrap();
        assert!((result.x - 3.0).abs() < 0.001);
        assert!((result.y - 0.0).abs() < 0.001);
        assert!((result.z - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_coord3d_operations() {
        let mut coord = Coord3D::new(1.0, 2.0, 3.0);
        coord.scale(2.0);
        assert_eq!(coord, Coord3D::new(2.0, 4.0, 6.0));

        let other = Coord3D::new(1.0, 1.0, 1.0);
        coord.add(&other);
        assert_eq!(coord, Coord3D::new(3.0, 5.0, 7.0));

        let length = coord.length();
        assert!((length - (3.0f32 * 3.0 + 5.0 * 5.0 + 7.0 * 7.0).sqrt()).abs() < 0.001);
    }
}
