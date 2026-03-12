//! Catmull-Rom spline implementation
//!
//! Catmull-Rom splines are a special case of Cardinal splines with a fixed tightness of 0.5.
//! They provide smooth curves that pass through all control points.

use crate::curve::{Curve1D, Curve3D};
use crate::hermite_spline::{HermiteSpline1D, HermiteSpline3D};
use crate::Vector3;

/// 3D Catmull-Rom spline
#[derive(Debug, Clone)]
pub struct CatmullRomSpline3D {
    pub hermite: HermiteSpline3D,
}

impl Default for CatmullRomSpline3D {
    fn default() -> Self {
        Self::new()
    }
}

impl CatmullRomSpline3D {
    pub fn new() -> Self {
        Self {
            hermite: HermiteSpline3D::new(),
        }
    }

    /// Update tangents using Catmull-Rom algorithm
    fn update_tangents_impl(&mut self) {
        if self.hermite.base.keys.len() < 2 {
            // Not enough points for tangent calculation
            for i in 0..self.hermite.base.keys.len() {
                if i < self.hermite.tangents.len() {
                    self.hermite.tangents[i].in_tangent = Vector3::ZERO;
                    self.hermite.tangents[i].out_tangent = Vector3::ZERO;
                }
            }
            return;
        }

        let key_count = self.hermite.base.keys.len();
        let end_idx = key_count - 1;

        // Initialize first and last tangents
        if !self.hermite.tangents.is_empty() {
            self.hermite.tangents[0].in_tangent = Vector3::ZERO;
            if key_count > 1 {
                self.hermite.tangents[end_idx].out_tangent = Vector3::ZERO;
            }
        }

        if self.hermite.base.is_looping && key_count > 2 {
            // Looping curve - connect first and last points
            let p_prev = self.hermite.base.keys[end_idx - 1].point;
            let p_next = self.hermite.base.keys[1].point;

            // Catmull-Rom uses 0.5 as the tangent scale factor
            self.hermite.tangents[0].out_tangent = 0.5 * (p_next - p_prev);
            self.hermite.tangents[end_idx].in_tangent = self.hermite.tangents[0].out_tangent;

            // Apply time-based scaling for non-uniform spacing
            let total_time = (self.hermite.base.keys[1].time - self.hermite.base.keys[0].time)
                + (self.hermite.base.keys[end_idx].time - self.hermite.base.keys[end_idx - 1].time);
            let in_factor = 2.0
                * (self.hermite.base.keys[end_idx].time - self.hermite.base.keys[end_idx - 1].time)
                / total_time;
            let out_factor = 2.0
                * (self.hermite.base.keys[1].time - self.hermite.base.keys[0].time)
                / total_time;

            self.hermite.tangents[end_idx].in_tangent *= in_factor;
            self.hermite.tangents[0].out_tangent *= out_factor;
        } else {
            // Non-looping curve - handle endpoints with reduced tangent scale
            if key_count >= 2 {
                // For endpoints, use a smaller scale factor to avoid overshoot
                self.hermite.tangents[0].out_tangent =
                    0.25 * (self.hermite.base.keys[1].point - self.hermite.base.keys[0].point);

                self.hermite.tangents[end_idx].in_tangent = 0.25
                    * (self.hermite.base.keys[end_idx].point
                        - self.hermite.base.keys[end_idx - 1].point);

                // Apply time-based scaling
                let total_time = (self.hermite.base.keys[1].time - self.hermite.base.keys[0].time)
                    + (self.hermite.base.keys[end_idx].time
                        - self.hermite.base.keys[end_idx - 1].time);
                let in_factor = 2.0
                    * (self.hermite.base.keys[end_idx].time
                        - self.hermite.base.keys[end_idx - 1].time)
                    / total_time;
                let out_factor = 2.0
                    * (self.hermite.base.keys[1].time - self.hermite.base.keys[0].time)
                    / total_time;

                self.hermite.tangents[end_idx].in_tangent *= in_factor;
                self.hermite.tangents[0].out_tangent *= out_factor;
            }
        }

        // Calculate tangents for interior points using standard Catmull-Rom formula
        for i in 1..key_count.saturating_sub(1) {
            let p_prev = self.hermite.base.keys[i - 1].point;
            let p_next = self.hermite.base.keys[i + 1].point;

            // Classic Catmull-Rom: tangent = 0.5 * (next - prev)
            let tangent = 0.5 * (p_next - p_prev);
            self.hermite.tangents[i].in_tangent = tangent;
            self.hermite.tangents[i].out_tangent = tangent;

            // Apply time-based scaling for non-uniform keyframe spacing
            let total_time =
                self.hermite.base.keys[i + 1].time - self.hermite.base.keys[i - 1].time;
            let in_factor = 2.0
                * (self.hermite.base.keys[i].time - self.hermite.base.keys[i - 1].time)
                / total_time;
            let out_factor = 2.0
                * (self.hermite.base.keys[i + 1].time - self.hermite.base.keys[i].time)
                / total_time;

            self.hermite.tangents[i].in_tangent *= in_factor;
            self.hermite.tangents[i].out_tangent *= out_factor;
        }

        self.hermite.tangents_dirty = false;
    }
}

impl Curve3D for CatmullRomSpline3D {
    fn evaluate(&mut self, time: f32) -> Vector3 {
        if self.hermite.tangents_dirty {
            self.update_tangents_impl();
        }
        self.hermite.evaluate(time)
    }

    fn is_looping(&self) -> bool {
        self.hermite.is_looping()
    }

    fn set_looping(&mut self, looping: bool) {
        self.hermite.set_looping(looping);
    }

    fn key_count(&self) -> usize {
        self.hermite.key_count()
    }

    fn get_key(&self, index: usize) -> Option<(Vector3, f32)> {
        self.hermite.get_key(index)
    }

    fn set_key(&mut self, index: usize, point: Vector3) {
        self.hermite.set_key(index, point);
    }

    fn add_key(&mut self, point: Vector3, time: f32) -> usize {
        self.hermite.add_key(point, time)
    }

    fn remove_key(&mut self, index: usize) {
        self.hermite.remove_key(index);
    }

    fn clear_keys(&mut self) {
        self.hermite.clear_keys();
    }

    fn get_start_time(&self) -> f32 {
        self.hermite.get_start_time()
    }

    fn get_end_time(&self) -> f32 {
        self.hermite.get_end_time()
    }
}

/// 1D Catmull-Rom spline
#[derive(Debug, Clone)]
pub struct CatmullRomSpline1D {
    pub hermite: HermiteSpline1D,
}

impl Default for CatmullRomSpline1D {
    fn default() -> Self {
        Self::new()
    }
}

impl CatmullRomSpline1D {
    pub fn new() -> Self {
        Self {
            hermite: HermiteSpline1D::new(),
        }
    }

    /// Update tangents using Catmull-Rom algorithm
    fn update_tangents_impl(&mut self) {
        if self.hermite.base.keys.len() < 2 {
            // Not enough points for tangent calculation
            for i in 0..self.hermite.base.keys.len() {
                if i < self.hermite.tangents.len() {
                    self.hermite.tangents[i].in_tangent = 0.0;
                    self.hermite.tangents[i].out_tangent = 0.0;
                }
            }
            return;
        }

        let key_count = self.hermite.base.keys.len();
        let end_idx = key_count - 1;

        // Initialize first and last tangents
        if !self.hermite.tangents.is_empty() {
            self.hermite.tangents[0].in_tangent = 0.0;
            if key_count > 1 {
                self.hermite.tangents[end_idx].out_tangent = 0.0;
            }
        }

        if self.hermite.base.is_looping && key_count > 2 {
            // Looping curve - connect first and last points
            let p_prev = self.hermite.base.keys[end_idx - 1].point;
            let p_next = self.hermite.base.keys[1].point;

            // Catmull-Rom uses 0.5 as the tangent scale factor
            self.hermite.tangents[0].out_tangent = 0.5 * (p_next - p_prev);
            self.hermite.tangents[end_idx].in_tangent = self.hermite.tangents[0].out_tangent;

            // Apply time-based scaling
            let total_time = (self.hermite.base.keys[1].time - self.hermite.base.keys[0].time)
                + (self.hermite.base.keys[end_idx].time - self.hermite.base.keys[end_idx - 1].time);
            let in_factor = 2.0
                * (self.hermite.base.keys[end_idx].time - self.hermite.base.keys[end_idx - 1].time)
                / total_time;
            let out_factor = 2.0
                * (self.hermite.base.keys[1].time - self.hermite.base.keys[0].time)
                / total_time;

            self.hermite.tangents[end_idx].in_tangent *= in_factor;
            self.hermite.tangents[0].out_tangent *= out_factor;
        } else {
            // Non-looping curve - handle endpoints with reduced tangent scale
            if key_count >= 2 {
                // For endpoints, use a smaller scale factor to avoid overshoot
                self.hermite.tangents[0].out_tangent =
                    0.25 * (self.hermite.base.keys[1].point - self.hermite.base.keys[0].point);

                self.hermite.tangents[end_idx].in_tangent = 0.25
                    * (self.hermite.base.keys[end_idx].point
                        - self.hermite.base.keys[end_idx - 1].point);

                // Apply time-based scaling
                let total_time = (self.hermite.base.keys[1].time - self.hermite.base.keys[0].time)
                    + (self.hermite.base.keys[end_idx].time
                        - self.hermite.base.keys[end_idx - 1].time);
                let in_factor = 2.0
                    * (self.hermite.base.keys[end_idx].time
                        - self.hermite.base.keys[end_idx - 1].time)
                    / total_time;
                let out_factor = 2.0
                    * (self.hermite.base.keys[1].time - self.hermite.base.keys[0].time)
                    / total_time;

                self.hermite.tangents[end_idx].in_tangent *= in_factor;
                self.hermite.tangents[0].out_tangent *= out_factor;
            }
        }

        // Calculate tangents for interior points using standard Catmull-Rom formula
        for i in 1..key_count.saturating_sub(1) {
            let p_prev = self.hermite.base.keys[i - 1].point;
            let p_next = self.hermite.base.keys[i + 1].point;

            // Classic Catmull-Rom: tangent = 0.5 * (next - prev)
            let tangent = 0.5 * (p_next - p_prev);
            self.hermite.tangents[i].in_tangent = tangent;
            self.hermite.tangents[i].out_tangent = tangent;

            // Apply time-based scaling for non-uniform keyframe spacing
            let total_time =
                self.hermite.base.keys[i + 1].time - self.hermite.base.keys[i - 1].time;
            let in_factor = 2.0
                * (self.hermite.base.keys[i].time - self.hermite.base.keys[i - 1].time)
                / total_time;
            let out_factor = 2.0
                * (self.hermite.base.keys[i + 1].time - self.hermite.base.keys[i].time)
                / total_time;

            self.hermite.tangents[i].in_tangent *= in_factor;
            self.hermite.tangents[i].out_tangent *= out_factor;
        }

        self.hermite.tangents_dirty = false;
    }
}

impl Curve1D for CatmullRomSpline1D {
    fn evaluate(&mut self, time: f32) -> f32 {
        if self.hermite.tangents_dirty {
            self.update_tangents_impl();
        }
        self.hermite.evaluate(time)
    }

    fn is_looping(&self) -> bool {
        self.hermite.is_looping()
    }

    fn set_looping(&mut self, looping: bool) {
        self.hermite.set_looping(looping);
    }

    fn key_count(&self) -> usize {
        self.hermite.key_count()
    }

    fn get_key(&self, index: usize) -> Option<(f32, f32, u32)> {
        self.hermite.get_key(index)
    }

    fn set_key(&mut self, index: usize, point: f32, extra: u32) {
        self.hermite.set_key(index, point, extra);
    }

    fn add_key(&mut self, point: f32, time: f32, extra: u32) -> usize {
        self.hermite.add_key(point, time, extra)
    }

    fn remove_key(&mut self, index: usize) {
        self.hermite.remove_key(index);
    }

    fn clear_keys(&mut self) {
        self.hermite.clear_keys();
    }

    fn get_start_time(&self) -> f32 {
        self.hermite.get_start_time()
    }

    fn get_end_time(&self) -> f32 {
        self.hermite.get_end_time()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_catmull_rom_spline_3d() {
        let mut spline = CatmullRomSpline3D::new();

        // Create a simple S-curve
        spline.add_key(Vector3::new(0.0, 0.0, 0.0), 0.0);
        spline.add_key(Vector3::new(5.0, 10.0, 5.0), 0.33);
        spline.add_key(Vector3::new(15.0, 10.0, 15.0), 0.67);
        spline.add_key(Vector3::new(20.0, 0.0, 20.0), 1.0);

        // Test that curve passes through control points
        let start = spline.evaluate(0.0);
        assert_eq!(start, Vector3::new(0.0, 0.0, 0.0));

        let middle1 = spline.evaluate(0.33);
        assert!((middle1 - Vector3::new(5.0, 10.0, 5.0)).length() < 0.01);

        let end = spline.evaluate(1.0);
        assert_eq!(end, Vector3::new(20.0, 0.0, 20.0));

        // Test smoothness - curve should be smooth between points
        let intermediate = spline.evaluate(0.5);
        assert!(intermediate.is_valid());
    }

    #[test]
    fn test_catmull_rom_spline_1d() {
        let mut spline = CatmullRomSpline1D::new();

        // Create a wave-like pattern
        spline.add_key(0.0, 0.0, 0);
        spline.add_key(10.0, 0.25, 0);
        spline.add_key(-10.0, 0.75, 0);
        spline.add_key(0.0, 1.0, 0);

        // Test that curve passes through control points
        let start = spline.evaluate(0.0);
        assert_eq!(start, 0.0);

        let peak = spline.evaluate(0.25);
        assert!((peak - 10.0).abs() < 0.01);

        let trough = spline.evaluate(0.75);
        assert!((trough - (-10.0)).abs() < 0.01);

        let end = spline.evaluate(1.0);
        assert_eq!(end, 0.0);
    }

    #[test]
    fn test_catmull_rom_looping() {
        let mut spline = CatmullRomSpline3D::new();

        // Create a square-like loop
        spline.add_key(Vector3::new(0.0, 0.0, 0.0), 0.0);
        spline.add_key(Vector3::new(10.0, 0.0, 0.0), 0.25);
        spline.add_key(Vector3::new(10.0, 10.0, 0.0), 0.5);
        spline.add_key(Vector3::new(0.0, 10.0, 0.0), 0.75);
        spline.add_key(Vector3::new(0.0, 0.0, 0.0), 1.0); // Same as start

        spline.set_looping(true);

        // Test smooth transitions across loop boundary
        let near_end = spline.evaluate(0.95);
        let near_start = spline.evaluate(0.05);

        assert!(near_end.is_valid());
        assert!(near_start.is_valid());

        // Should create smooth curve
        let mid_result = spline.evaluate(0.5);
        assert!(mid_result.is_valid());
    }

    #[test]
    fn test_catmull_rom_minimal_points() {
        let mut spline = CatmullRomSpline3D::new();

        // Test with single point
        spline.add_key(Vector3::new(5.0, 5.0, 5.0), 0.5);
        let result = spline.evaluate(0.5);
        assert_eq!(result, Vector3::new(5.0, 5.0, 5.0));

        // Test with two points (should behave like linear)
        spline.add_key(Vector3::new(10.0, 10.0, 10.0), 1.0);
        let mid_result = spline.evaluate(0.75);
        // Should be somewhere between the two points
        assert!(mid_result.x > 5.0 && mid_result.x < 10.0);
    }

    #[test]
    fn test_catmull_rom_uniform_vs_nonuniform_timing() {
        let mut uniform_spline = CatmullRomSpline3D::new();
        let mut nonuniform_spline = CatmullRomSpline3D::new();

        // Uniform timing
        uniform_spline.add_key(Vector3::new(0.0, 0.0, 0.0), 0.0);
        uniform_spline.add_key(Vector3::new(10.0, 10.0, 10.0), 0.33);
        uniform_spline.add_key(Vector3::new(20.0, 0.0, 20.0), 0.67);
        uniform_spline.add_key(Vector3::new(30.0, 10.0, 30.0), 1.0);

        // Non-uniform timing
        nonuniform_spline.add_key(Vector3::new(0.0, 0.0, 0.0), 0.0);
        nonuniform_spline.add_key(Vector3::new(10.0, 10.0, 10.0), 0.1); // Earlier
        nonuniform_spline.add_key(Vector3::new(20.0, 0.0, 20.0), 0.9); // Later
        nonuniform_spline.add_key(Vector3::new(30.0, 10.0, 30.0), 1.0);

        let uniform_mid = uniform_spline.evaluate(0.5);
        let nonuniform_mid = nonuniform_spline.evaluate(0.5);

        // Different timing should produce different results
        assert_ne!(uniform_mid, nonuniform_mid);

        // Both should be valid
        assert!(uniform_mid.is_valid());
        assert!(nonuniform_mid.is_valid());
    }
}
