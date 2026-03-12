////////////////////////////////////////////////////////////////////////////////
//                                                                            //
//  (c) 2001-2003 Electronic Arts Inc.                                       //
//                                                                            //
////////////////////////////////////////////////////////////////////////////////

use super::bezier_segment::{BezierSegment, Bool, Coord3D, Int, Real, Vector4};

/// Bezier Forward Iterator
///
/// A forward difference iterator for efficiently evaluating points along a Bezier curve.
/// This uses the forward difference method for fast iteration through multiple points
/// on the curve without re-evaluating the entire curve equation at each step.
#[derive(Debug, Clone)]
pub struct BezFwdIterator {
    step: Int,
    steps_desired: Int,
    bez_seg: BezierSegment,
    curr_point: Coord3D,
    dq: Coord3D,   // First derivative
    ddq: Coord3D,  // Second derivative
    dddq: Coord3D, // Third derivative
}

impl BezFwdIterator {
    /// Default constructor - creates an iterator with zero steps
    pub fn new_default() -> Self {
        Self {
            step: 0,
            steps_desired: 0,
            bez_seg: BezierSegment::new(),
            curr_point: Coord3D::default(),
            dq: Coord3D::default(),
            ddq: Coord3D::default(),
            dddq: Coord3D::default(),
        }
    }

    /// Constructor with desired steps and bezier segment
    pub fn new(steps_desired: Int, bez_seg: &BezierSegment) -> Self {
        Self {
            step: 0,
            steps_desired,
            bez_seg: bez_seg.clone(),
            curr_point: Coord3D::default(),
            dq: Coord3D::default(),
            ddq: Coord3D::default(),
            dddq: Coord3D::default(),
        }
    }

    /// Initialize the iterator to start iteration
    pub fn start(&mut self) {
        self.step = 0;

        if self.steps_desired <= 1 {
            return;
        }

        let d = 1.0 / (self.steps_desired - 1) as Real;
        let d2 = d * d;
        let d3 = d * d2;

        let px = Vector4::new(
            self.bez_seg.control_points()[0].x,
            self.bez_seg.control_points()[1].x,
            self.bez_seg.control_points()[2].x,
            self.bez_seg.control_points()[3].x,
        );
        let py = Vector4::new(
            self.bez_seg.control_points()[0].y,
            self.bez_seg.control_points()[1].y,
            self.bez_seg.control_points()[2].y,
            self.bez_seg.control_points()[3].y,
        );
        let pz = Vector4::new(
            self.bez_seg.control_points()[0].z,
            self.bez_seg.control_points()[1].z,
            self.bez_seg.control_points()[2].z,
            self.bez_seg.control_points()[3].z,
        );

        // The Bezier basis matrix for transformations
        let bez_basis_matrix = super::bezier_segment::Matrix4x4::new(
            -1.0, 3.0, -3.0, 1.0, 3.0, -6.0, 3.0, 0.0, -3.0, 3.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0,
        );

        // Transform coordinate vectors with the Bezier basis matrix
        let c_vec = [
            bez_basis_matrix.transform(&px),
            bez_basis_matrix.transform(&py),
            bez_basis_matrix.transform(&pz),
        ];

        self.curr_point = self.bez_seg.control_points()[0];

        // Calculate forward differences for x, y, z components
        for i in 0..3 {
            let a = c_vec[i].x;
            let b = c_vec[i].y;
            let c = c_vec[i].z;

            let (p_d, p_dd, p_ddd) = match i {
                0 => (&mut self.dq.x, &mut self.ddq.x, &mut self.dddq.x),
                1 => (&mut self.dq.y, &mut self.ddq.y, &mut self.dddq.y),
                2 => (&mut self.dq.z, &mut self.ddq.z, &mut self.dddq.z),
                _ => unreachable!(),
            };

            *p_d = a * d3 + b * d2 + c * d;
            *p_dd = 6.0 * a * d3 + 2.0 * b * d2;
            *p_ddd = 6.0 * a * d3;
        }
    }

    /// Check if iteration is complete
    pub fn done(&self) -> Bool {
        self.step >= self.steps_desired
    }

    /// Get the current point in the iteration
    pub fn get_current(&self) -> Coord3D {
        self.curr_point
    }

    /// Advance to the next point in the iteration
    pub fn next(&mut self) {
        self.curr_point.add(&self.dq);
        self.dq.add(&self.ddq);
        self.ddq.add(&self.dddq);

        self.step += 1;
    }

    /// Reset the iterator to the beginning
    pub fn reset(&mut self) {
        self.start();
    }

    /// Get the total number of steps this iterator will produce
    pub fn total_steps(&self) -> Int {
        self.steps_desired
    }

    /// Get the current step number (0-based)
    pub fn current_step(&self) -> Int {
        self.step
    }

    /// Get progress as a percentage (0.0 to 1.0)
    pub fn progress(&self) -> Real {
        if self.steps_desired <= 0 {
            1.0
        } else {
            self.step as Real / self.steps_desired as Real
        }
    }
}

/// Implement Iterator trait for BezFwdIterator
impl Iterator for BezFwdIterator {
    type Item = Coord3D;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done() {
            None
        } else {
            let current = self.get_current();
            BezFwdIterator::next(self); // Call our next method, not Iterator's
            Some(current)
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = (self.steps_desired - self.step).max(0) as usize;
        (remaining, Some(remaining))
    }
}

impl ExactSizeIterator for BezFwdIterator {
    fn len(&self) -> usize {
        (self.steps_desired - self.step).max(0) as usize
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::bezier::bezier_segment::BezierSegment;

    #[test]
    fn test_bez_fwd_iterator_creation() {
        let segment = BezierSegment::from_coordinates(
            0.0, 0.0, 0.0, 1.0, 1.0, 0.0, 2.0, 1.0, 0.0, 3.0, 0.0, 0.0,
        );

        let iter = BezFwdIterator::new(10, &segment);
        assert_eq!(iter.total_steps(), 10);
        assert_eq!(iter.current_step(), 0);
        assert!(!iter.done());
    }

    #[test]
    fn test_bez_fwd_iterator_start_end_points() {
        let segment = BezierSegment::from_coordinates(
            0.0, 0.0, 0.0, 1.0, 1.0, 0.0, 2.0, 1.0, 0.0, 3.0, 0.0, 0.0,
        );

        let mut iter = BezFwdIterator::new(2, &segment);
        iter.start();

        // First point should be at the start of the curve
        assert!(!iter.done());
        let first_point = iter.get_current();
        assert!((first_point.x - 0.0).abs() < 0.001);
        assert!((first_point.y - 0.0).abs() < 0.001);
        assert!((first_point.z - 0.0).abs() < 0.001);

        iter.next();

        // Second point should be at the end of the curve
        assert!(!iter.done());
        let second_point = iter.get_current();
        assert!((second_point.x - 3.0).abs() < 0.001);
        assert!((second_point.y - 0.0).abs() < 0.001);
        assert!((second_point.z - 0.0).abs() < 0.001);

        iter.next();

        // Should be done now
        assert!(iter.done());
    }

    #[test]
    fn test_bez_fwd_iterator_as_rust_iterator() {
        let segment = BezierSegment::from_coordinates(
            0.0, 0.0, 0.0, 1.0, 1.0, 0.0, 2.0, 1.0, 0.0, 3.0, 0.0, 0.0,
        );

        let mut iter = BezFwdIterator::new(5, &segment);
        iter.start();

        let points: Vec<Coord3D> = iter.collect();
        assert_eq!(points.len(), 5);

        // First point should be start of curve
        assert!((points[0].x - 0.0).abs() < 0.001);
        assert!((points[0].y - 0.0).abs() < 0.001);

        // Last point should be end of curve
        assert!((points[4].x - 3.0).abs() < 0.001);
        assert!((points[4].y - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_progress_tracking() {
        let segment = BezierSegment::new();
        let mut iter = BezFwdIterator::new(4, &segment);
        iter.start();

        assert!((iter.progress() - 0.0).abs() < 0.001);

        iter.next();
        assert!((iter.progress() - 0.25).abs() < 0.001);

        iter.next();
        assert!((iter.progress() - 0.5).abs() < 0.001);

        iter.next();
        assert!((iter.progress() - 0.75).abs() < 0.001);

        iter.next();
        assert!((iter.progress() - 1.0).abs() < 0.001);
        assert!(iter.done());
    }
}
