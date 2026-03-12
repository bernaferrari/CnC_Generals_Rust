//! Vehicle curve implementation
//!
//! A vehicle curve represents the path a vehicle would take through a series of points.
//! Each point on the curve passes through a turn-arc of the vehicle. The size of this
//! arc is determined by the turn radius used to initialize the curve.

use crate::curve::{BaseCurve3D, Curve3D};
use crate::{Matrix3D, Vector3, WWMath};
use std::f32::consts::PI;

/// Information about a turn arc at a waypoint
#[derive(Debug, Clone)]
pub struct ArcInfo {
    pub center: Vector3,
    pub point_in: Vector3,
    pub point_out: Vector3,
    pub point_angle: f32,
    pub radius: f32,
    pub angle_in_delta: f32,
    pub angle_out_delta: f32,
}

impl Default for ArcInfo {
    fn default() -> Self {
        Self::new()
    }
}

impl ArcInfo {
    pub fn new() -> Self {
        Self {
            center: Vector3::ZERO,
            point_in: Vector3::ZERO,
            point_out: Vector3::ZERO,
            point_angle: 0.0,
            radius: 0.0,
            angle_in_delta: 0.0,
            angle_out_delta: 0.0,
        }
    }
}

/// Vehicle curve for realistic vehicle path following
#[derive(Debug, Clone)]
pub struct VehicleCurve {
    pub base: BaseCurve3D,
    pub radius: f32,
    pub arc_list: Vec<ArcInfo>,
    pub is_dirty: bool,
    pub last_time: f32,
    pub sharpness: f32,
    pub sharpness_pos: Vector3,
}

impl Default for VehicleCurve {
    fn default() -> Self {
        Self::new()
    }
}

impl VehicleCurve {
    pub fn new() -> Self {
        Self {
            base: BaseCurve3D::new(),
            radius: 0.0,
            arc_list: Vec::new(),
            is_dirty: true,
            last_time: 0.0,
            sharpness: 0.0,
            sharpness_pos: Vector3::ZERO,
        }
    }

    pub fn new_with_radius(radius: f32) -> Self {
        Self {
            base: BaseCurve3D::new(),
            radius,
            arc_list: Vec::new(),
            is_dirty: true,
            last_time: 0.0,
            sharpness: 0.0,
            sharpness_pos: Vector3::ZERO,
        }
    }

    /// Initialize the turn radius for the vehicle
    pub fn initialize_arc(&mut self, radius: f32) {
        self.radius = radius;
        self.is_dirty = true;
    }

    /// Get the current sharpness value and position
    pub fn get_current_sharpness(&self) -> (f32, Vector3) {
        (self.sharpness, self.sharpness_pos)
    }

    /// Get the time of the last evaluation
    pub fn get_last_eval_time(&self) -> f32 {
        self.last_time
    }

    /// Update the arc list based on current keyframes
    fn update_arc_list(&mut self) {
        self.arc_list.clear();

        let count = self.base.keys.len();
        if count == 0 {
            return;
        }

        // Add a record for the starting point
        let mut arc_start = ArcInfo::new();
        arc_start.point_in = self.base.keys[0].point;
        arc_start.point_out = self.base.keys[0].point;
        arc_start.center = self.base.keys[0].point;
        self.arc_list.push(arc_start);

        // Loop over each interior point and generate arc information
        for index in 1..count.saturating_sub(1) {
            let prev_pt = self.base.keys[index - 1].point;
            let curr_pt = self.base.keys[index].point;
            let next_pt = self.base.keys[index + 1].point;

            // Determine the last known point on the path
            let last_path_pt = self.arc_list[index - 1].point_out;

            // Create a transformation matrix to simulate the vehicle's position
            let x_vector = (curr_pt - last_path_pt).normalize();
            let z_vector = Vector3::new(0.0, 0.0, 1.0);
            let y_vector = z_vector.cross(x_vector);
            let transform =
                Matrix3D::from_rows_and_translation(x_vector, y_vector, z_vector, last_path_pt);

            // Find where the turn arc should be centered
            let (arc_center, is_right_turn) =
                self.find_turn_arc(&transform, prev_pt, curr_pt, next_pt);

            // Determine tangent angles
            let (point_angle, angle_in_delta, angle_out_delta) =
                self.find_tangents(last_path_pt, curr_pt, next_pt, arc_center, is_right_turn);

            // Determine intersection points
            let point_in = Vector3::new(
                arc_center.x + (self.radius * WWMath::sin(point_angle + angle_in_delta)),
                arc_center.y + (self.radius * -WWMath::cos(point_angle + angle_in_delta)),
                arc_center.z,
            );

            let point_out = Vector3::new(
                arc_center.x + (self.radius * WWMath::sin(point_angle + angle_out_delta)),
                arc_center.y + (self.radius * -WWMath::cos(point_angle + angle_out_delta)),
                arc_center.z,
            );

            // Sanity check for excessive turn angles
            if angle_in_delta.abs() > WWMath::deg_to_rad(200.0)
                || angle_out_delta.abs() > WWMath::deg_to_rad(200.0)
            {
                // Use current point directly for sharp turns
                let mut arc_info = ArcInfo::new();
                arc_info.center = curr_pt;
                arc_info.point_in = curr_pt;
                arc_info.point_out = curr_pt;
                self.arc_list.push(arc_info);
            } else {
                // Use calculated arc
                let mut arc_info = ArcInfo::new();
                arc_info.center = arc_center;
                arc_info.point_angle = point_angle;
                arc_info.point_in = point_in;
                arc_info.point_out = point_out;
                arc_info.radius = self.radius;
                arc_info.angle_in_delta = angle_in_delta;
                arc_info.angle_out_delta = angle_out_delta;
                self.arc_list.push(arc_info);
            }
        }

        // Add a record for the ending point
        if count > 1 {
            let mut arc_end = ArcInfo::new();
            arc_end.point_in = self.base.keys[count - 1].point;
            arc_end.point_out = self.base.keys[count - 1].point;
            arc_end.center = self.base.keys[count - 1].point;
            self.arc_list.push(arc_end);
        }

        self.is_dirty = false;
    }

    /// Find the center and direction of a turn arc
    fn find_turn_arc(
        &self,
        transform: &Matrix3D,
        prev_pt: Vector3,
        curr_pt: Vector3,
        next_pt: Vector3,
    ) -> (Vector3, bool) {
        // Calculate angles to previous and next points
        let angle1 = WWMath::atan2(
            (prev_pt.y - curr_pt.y) as f64,
            (prev_pt.x - curr_pt.x) as f64,
        );
        let angle1 = WWMath::wrap(angle1 as f32, 0.0, 2.0 * PI);

        let angle2 = WWMath::atan2(
            (next_pt.y - curr_pt.y) as f64,
            (next_pt.x - curr_pt.x) as f64,
        );
        let angle2 = WWMath::wrap(angle2 as f32, 0.0, 2.0 * PI);

        // Find shortest angular distance
        let delta1 = self.get_angle_delta(angle1, angle2, true).abs();
        let delta2 = self.get_angle_delta(angle1, angle2, false).abs();

        let avg_angle = if delta1 < delta2 {
            angle1 - (delta1 * 0.5)
        } else {
            angle1 + (delta2 * 0.5)
        };

        // Calculate arc center
        let arc_center = Vector3::new(
            curr_pt.x + (self.radius * WWMath::cos(avg_angle)),
            curr_pt.y + (self.radius * WWMath::sin(avg_angle)),
            curr_pt.z,
        );

        // Determine if it's a right turn
        let rel_center = transform.inverse().transform_vector(arc_center);
        let is_right_turn = rel_center.y > 0.0;

        (arc_center, is_right_turn)
    }

    /// Find tangent angles for arc entry and exit
    fn find_tangents(
        &self,
        prev_pt: Vector3,
        curr_pt: Vector3,
        next_pt: Vector3,
        arc_center: Vector3,
        is_right_turn: bool,
    ) -> (f32, f32, f32) {
        // Find tangent angles
        let angle_in = self
            .find_tangent(arc_center, prev_pt, is_right_turn)
            .unwrap_or(0.0);
        let angle_out = self
            .find_tangent(arc_center, next_pt, !is_right_turn)
            .unwrap_or(0.0);

        // Find current position angle on arc
        let point_angle = WWMath::atan2(
            (curr_pt.x - arc_center.x) as f64,
            (-(curr_pt.y - arc_center.y)) as f64,
        );
        let point_angle = WWMath::wrap(point_angle as f32, 0.0, 2.0 * PI);

        // Calculate deltas
        let angle_in_delta = self.get_angle_delta(angle_in, point_angle, is_right_turn);
        let angle_out_delta = self.get_angle_delta(angle_out, point_angle, !is_right_turn);

        (point_angle, angle_in_delta, angle_out_delta)
    }

    /// Find tangent from external point to circle
    fn find_tangent(&self, center: Vector3, point: Vector3, clockwise: bool) -> Option<f32> {
        let delta_x = point.x - center.x;
        let delta_y = point.y - center.y;
        let dist = (delta_x * delta_x + delta_y * delta_y).sqrt();

        if dist >= self.radius {
            let angle_offset = WWMath::acos(self.radius / dist);
            let base_angle = WWMath::atan2(delta_x as f64, (-delta_y) as f64);
            let base_angle = WWMath::wrap(base_angle as f32, 0.0, 2.0 * PI);

            let angle = if clockwise {
                base_angle - angle_offset
            } else {
                base_angle + angle_offset
            };

            Some(WWMath::wrap(angle, 0.0, 2.0 * PI))
        } else {
            None
        }
    }

    /// Get angle delta considering orientation
    fn get_angle_delta(&self, angle1: f32, angle2: f32, clockwise: bool) -> f32 {
        let mut result = angle1 - angle2;

        if clockwise {
            if angle1 < angle2 {
                result = angle1 - (angle2 - 2.0 * PI);
            }
        } else if angle1 > angle2 {
            result = (angle1 - 2.0 * PI) - angle2;
        }

        result
    }
}

impl Curve3D for VehicleCurve {
    fn evaluate(&mut self, time: f32) -> Vector3 {
        let count = self.base.keys.len();
        self.sharpness = 0.0;

        if count == 0 {
            return Vector3::ZERO;
        }

        if time < self.base.keys[0].time {
            self.last_time = self.base.keys[0].time;
            return self.base.keys[0].point;
        }

        if time >= self.base.keys[count - 1].time {
            self.last_time = self.base.keys[count - 1].time;
            return self.base.keys[count - 1].point;
        }

        // Update arc information if needed
        if self.is_dirty {
            self.update_arc_list();
        }

        // Find the segment
        let (index0, index1, seg_time) = self.base.find_interval(time);

        if index0 >= self.arc_list.len() || index1 >= self.arc_list.len() {
            // Fallback to linear interpolation
            self.last_time = time;
            return self.base.keys[index0].point
                + seg_time * (self.base.keys[index1].point - self.base.keys[index0].point);
        }

        let arc_info0 = &self.arc_list[index0];
        let arc_info1 = &self.arc_list[index1];

        // Calculate segment lengths
        let arc_length0 = arc_info0.radius * arc_info0.angle_out_delta.abs();
        let arc_length1 = arc_info1.radius * arc_info1.angle_in_delta.abs();
        let straight_length = (arc_info1.point_in - arc_info0.point_out).length() * 0.5;
        let total_length = arc_length0 + arc_length1 + straight_length;

        // Calculate time boundaries
        let time1 = if total_length > 0.0 {
            arc_length0 / total_length
        } else {
            0.0
        };
        let time2 = if total_length > 0.0 {
            (arc_length0 + straight_length) / total_length
        } else {
            0.0
        };

        let mut result = Vector3::ZERO;

        if seg_time < time1 {
            // On the exit arc of the first point
            let angle = arc_info0.point_angle + arc_info0.angle_out_delta;
            result.x = arc_info0.center.x + (arc_info0.radius * WWMath::sin(angle));
            result.y = arc_info0.center.y + (arc_info0.radius * -WWMath::cos(angle));
            result.z = self.base.keys[index0].point.z
                + (self.base.keys[index1].point.z - self.base.keys[index0].point.z) * seg_time;

            self.sharpness = WWMath::clamp(
                arc_info0.angle_out_delta.abs() / WWMath::deg_to_rad(15.0),
                0.0,
                1.0,
            );
            self.sharpness_pos = Vector3::new(result.x, result.y, result.z);
            self.last_time = self.base.keys[index0].time
                + (self.base.keys[index1].time - self.base.keys[index0].time) * time1;
        } else if seg_time < time2 {
            // On the straight line between arcs
            let percent = (seg_time - time1) / (time2 - time1);
            if percent == 0.0 {
                result = arc_info0.point_out;
            } else {
                result = arc_info1.point_in;
            }
            result.z = self.base.keys[index0].point.z
                + (self.base.keys[index1].point.z - self.base.keys[index0].point.z) * seg_time;

            self.sharpness = WWMath::clamp(
                arc_info1.angle_out_delta.abs() / WWMath::deg_to_rad(15.0),
                0.0,
                1.0,
            );
            self.sharpness_pos = arc_info1.point_in;
            self.last_time = self.base.keys[index0].time
                + (self.base.keys[index1].time - self.base.keys[index0].time) * time2;
        } else {
            // On the entrance arc of the second point
            let angle = arc_info1.point_angle + arc_info1.angle_out_delta;
            result.x = arc_info1.center.x + (arc_info1.radius * WWMath::sin(angle));
            result.y = arc_info1.center.y + (arc_info1.radius * -WWMath::cos(angle));
            result.z = self.base.keys[index0].point.z
                + (self.base.keys[index1].point.z - self.base.keys[index0].point.z) * seg_time;

            self.sharpness = WWMath::clamp(
                arc_info1.angle_out_delta.abs() / WWMath::deg_to_rad(15.0),
                0.0,
                1.0,
            );
            self.sharpness_pos = Vector3::new(result.x, result.y, result.z);
            self.last_time = self.base.keys[index1].time;
        }

        result
    }

    fn is_looping(&self) -> bool {
        self.base.is_looping
    }

    fn set_looping(&mut self, looping: bool) {
        self.base.is_looping = looping;
        self.is_dirty = true;
    }

    fn key_count(&self) -> usize {
        self.base.keys.len()
    }

    fn get_key(&self, index: usize) -> Option<(Vector3, f32)> {
        self.base.get_key(index)
    }

    fn set_key(&mut self, index: usize, point: Vector3) {
        self.base.set_key(index, point);
        self.is_dirty = true;
    }

    fn add_key(&mut self, point: Vector3, time: f32) -> usize {
        let index = self.base.add_key(point, time);
        self.is_dirty = true;
        index
    }

    fn remove_key(&mut self, index: usize) {
        self.base.remove_key(index);
        self.is_dirty = true;
    }

    fn clear_keys(&mut self) {
        self.base.clear_keys();
        self.is_dirty = true;
    }

    fn get_start_time(&self) -> f32 {
        self.base.get_start_time()
    }

    fn get_end_time(&self) -> f32 {
        self.base.get_end_time()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vehicle_curve_basic() {
        let mut curve = VehicleCurve::new_with_radius(5.0);

        // Create a simple path
        curve.add_key(Vector3::new(0.0, 0.0, 0.0), 0.0);
        curve.add_key(Vector3::new(10.0, 0.0, 0.0), 0.5);
        curve.add_key(Vector3::new(10.0, 10.0, 0.0), 1.0);

        // Test evaluation
        let start = curve.evaluate(0.0);
        assert_eq!(start, Vector3::new(0.0, 0.0, 0.0));

        let mid = curve.evaluate(0.5);
        assert!(mid.is_valid());

        let end = curve.evaluate(1.0);
        assert_eq!(end, Vector3::new(10.0, 10.0, 0.0));
    }

    #[test]
    fn test_vehicle_curve_turn_radius() {
        let mut small_radius = VehicleCurve::new_with_radius(1.0);
        let mut large_radius = VehicleCurve::new_with_radius(10.0);

        // Same path for both curves
        let points = vec![
            (Vector3::new(0.0, 0.0, 0.0), 0.0),
            (Vector3::new(10.0, 0.0, 0.0), 0.5),
            (Vector3::new(10.0, 10.0, 0.0), 1.0),
        ];

        for (point, time) in &points {
            small_radius.add_key(*point, *time);
            large_radius.add_key(*point, *time);
        }

        // Different turn radii should produce different curves
        let small_result = small_radius.evaluate(0.75);
        let large_result = large_radius.evaluate(0.75);

        assert_ne!(small_result, large_result);
        assert!(small_result.is_valid());
        assert!(large_result.is_valid());
    }

    #[test]
    fn test_vehicle_curve_sharpness() {
        let mut curve = VehicleCurve::new_with_radius(5.0);

        // Create a sharp turn
        curve.add_key(Vector3::new(0.0, 0.0, 0.0), 0.0);
        curve.add_key(Vector3::new(10.0, 0.0, 0.0), 0.5);
        curve.add_key(Vector3::new(10.0, 10.0, 0.0), 1.0);

        // Evaluate at turn point
        curve.evaluate(0.75);
        let (sharpness, _) = curve.get_current_sharpness();

        // Should have some sharpness value for the turn
        assert!(sharpness >= 0.0 && sharpness <= 1.0);
    }

    #[test]
    fn test_vehicle_curve_empty() {
        let mut curve = VehicleCurve::new();

        let result = curve.evaluate(0.5);
        assert_eq!(result, Vector3::ZERO);
    }

    #[test]
    fn test_vehicle_curve_single_point() {
        let mut curve = VehicleCurve::new_with_radius(5.0);

        curve.add_key(Vector3::new(5.0, 5.0, 5.0), 0.5);

        let result = curve.evaluate(0.5);
        assert_eq!(result, Vector3::new(5.0, 5.0, 5.0));
    }

    #[test]
    fn test_vehicle_curve_straight_line() {
        let mut curve = VehicleCurve::new_with_radius(5.0);

        // Straight line - should not need arcs
        curve.add_key(Vector3::new(0.0, 0.0, 0.0), 0.0);
        curve.add_key(Vector3::new(10.0, 0.0, 0.0), 1.0);

        let mid = curve.evaluate(0.5);
        // Should be approximately at the midpoint
        assert!((mid - Vector3::new(5.0, 0.0, 0.0)).length() < 1.0);
    }

    #[test]
    fn test_vehicle_curve_bounds() {
        let mut curve = VehicleCurve::new_with_radius(5.0);

        curve.add_key(Vector3::new(0.0, 0.0, 0.0), 1.0);
        curve.add_key(Vector3::new(10.0, 10.0, 10.0), 2.0);

        // Test out-of-bounds evaluation
        let before = curve.evaluate(0.5); // Before start time
        assert_eq!(before, Vector3::new(0.0, 0.0, 0.0));

        let after = curve.evaluate(3.0); // After end time
        assert_eq!(after, Vector3::new(10.0, 10.0, 10.0));
    }
}
