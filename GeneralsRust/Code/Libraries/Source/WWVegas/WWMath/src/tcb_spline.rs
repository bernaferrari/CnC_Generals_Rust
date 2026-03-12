//! TCB (Tension-Continuity-Bias) spline implementation
//!
//! TCB splines, also known as Kochanek-Bartels cubic splines, provide fine control
//! over curve shape using three parameters:
//! - Tension: Controls how sharply the curve bends (tight vs loose)
//! - Continuity: Controls the rate of change in direction
//! - Bias: Controls the direction of the curve as it passes through a keyframe

use crate::curve::{Curve1D, Curve3D};
use crate::hermite_spline::{HermiteSpline1D, HermiteSpline3D};
use crate::Vector3;

/// TCB parameters for a single keyframe
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TcbParams {
    pub tension: f32,    // 0.0 = loose, 1.0 = tight
    pub continuity: f32, // -1.0 to 1.0, controls rate of direction change
    pub bias: f32,       // -1.0 to 1.0, controls direction through keyframe
}

impl TcbParams {
    pub fn new(tension: f32, continuity: f32, bias: f32) -> Self {
        Self {
            tension,
            continuity,
            bias,
        }
    }
}

/// 3D TCB spline
#[derive(Debug, Clone)]
pub struct TcbSpline3D {
    pub hermite: HermiteSpline3D,
    pub params: Vec<TcbParams>,
}

impl Default for TcbSpline3D {
    fn default() -> Self {
        Self::new()
    }
}

impl TcbSpline3D {
    pub fn new() -> Self {
        Self {
            hermite: HermiteSpline3D::new(),
            params: Vec::new(),
        }
    }

    /// Set the TCB parameters for a specific keyframe
    pub fn set_tcb_params(&mut self, index: usize, tension: f32, continuity: f32, bias: f32) {
        if index < self.params.len() {
            self.params[index] = TcbParams::new(tension, continuity, bias);
            self.hermite.tangents_dirty = true;
        }
    }

    /// Get the TCB parameters for a specific keyframe
    pub fn get_tcb_params(&self, index: usize) -> Option<TcbParams> {
        self.params.get(index).cloned()
    }

    /// Update tangents using TCB algorithm
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
            let tcb = self.params.first().cloned().unwrap_or_default();

            let k0 = 0.5 * ((1.0 - tcb.tension) * (1.0 - tcb.continuity) * (1.0 - tcb.bias));
            let k1 = 0.5 * ((1.0 - tcb.tension) * (1.0 + tcb.continuity) * (1.0 + tcb.bias));
            let k2 = 0.5 * ((1.0 - tcb.tension) * (1.0 + tcb.continuity) * (1.0 - tcb.bias));
            let k3 = 0.5 * ((1.0 - tcb.tension) * (1.0 - tcb.continuity) * (1.0 + tcb.bias));

            let p_prev = self.hermite.base.keys[end_idx - 1].point;
            let p_curr = self.hermite.base.keys[0].point;
            let p_next = self.hermite.base.keys[1].point;

            let dp_in = p_curr - p_prev;
            let dp_out = p_next - p_curr;

            self.hermite.tangents[end_idx].in_tangent = k0 * dp_in + k1 * dp_out;
            self.hermite.tangents[0].out_tangent = k2 * dp_out + k3 * dp_in;

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
            // Non-looping curve - handle endpoints specially
            if key_count >= 2 {
                // First point
                let tcb_0 = self.params.first().cloned().unwrap_or_default();
                let k2 =
                    0.25 * ((1.0 - tcb_0.tension) * (1.0 + tcb_0.continuity) * (1.0 - tcb_0.bias));
                let k3 =
                    0.25 * ((1.0 - tcb_0.tension) * (1.0 - tcb_0.continuity) * (1.0 + tcb_0.bias));

                let dp_out = self.hermite.base.keys[1].point - self.hermite.base.keys[0].point;
                self.hermite.tangents[0].out_tangent = k2 * dp_out + k3 * dp_out;

                // Last point
                let tcb_end = self.params.get(end_idx).cloned().unwrap_or_default();
                let k0 = 0.25
                    * ((1.0 - tcb_end.tension) * (1.0 - tcb_end.continuity) * (1.0 - tcb_end.bias));
                let k1 = 0.25
                    * ((1.0 - tcb_end.tension) * (1.0 + tcb_end.continuity) * (1.0 + tcb_end.bias));

                let dp_in = self.hermite.base.keys[end_idx].point
                    - self.hermite.base.keys[end_idx - 1].point;
                self.hermite.tangents[end_idx].in_tangent = k0 * dp_in + k1 * dp_in;

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

        // Calculate tangents for interior points using TCB formula
        for i in 1..key_count.saturating_sub(1) {
            let tcb = self.params.get(i).cloned().unwrap_or_default();

            // Calculate the four TCB coefficients
            let k0 = 0.5 * ((1.0 - tcb.tension) * (1.0 - tcb.continuity) * (1.0 - tcb.bias));
            let k1 = 0.5 * ((1.0 - tcb.tension) * (1.0 + tcb.continuity) * (1.0 + tcb.bias));
            let k2 = 0.5 * ((1.0 - tcb.tension) * (1.0 + tcb.continuity) * (1.0 - tcb.bias));
            let k3 = 0.5 * ((1.0 - tcb.tension) * (1.0 - tcb.continuity) * (1.0 + tcb.bias));

            let p_prev = self.hermite.base.keys[i - 1].point;
            let p_curr = self.hermite.base.keys[i].point;
            let p_next = self.hermite.base.keys[i + 1].point;

            let dp_in = p_curr - p_prev;
            let dp_out = p_next - p_curr;

            // TCB tangent calculations
            self.hermite.tangents[i].in_tangent = k0 * dp_out + k1 * dp_in;
            self.hermite.tangents[i].out_tangent = k2 * dp_out + k3 * dp_in;

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

impl Curve3D for TcbSpline3D {
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
        let index = self.hermite.add_key(point, time);
        self.params.insert(index, TcbParams::default());
        index
    }

    fn remove_key(&mut self, index: usize) {
        if index < self.params.len() {
            self.params.remove(index);
        }
        self.hermite.remove_key(index);
    }

    fn clear_keys(&mut self) {
        self.params.clear();
        self.hermite.clear_keys();
    }

    fn get_start_time(&self) -> f32 {
        self.hermite.get_start_time()
    }

    fn get_end_time(&self) -> f32 {
        self.hermite.get_end_time()
    }
}

/// 1D TCB spline (only supports 3D version in original C++ code)
/// We provide this for completeness but it wasn't in the original implementation
#[derive(Debug, Clone)]
pub struct TcbSpline1D {
    pub hermite: HermiteSpline1D,
    pub params: Vec<TcbParams>,
}

impl Default for TcbSpline1D {
    fn default() -> Self {
        Self::new()
    }
}

impl TcbSpline1D {
    pub fn new() -> Self {
        Self {
            hermite: HermiteSpline1D::new(),
            params: Vec::new(),
        }
    }

    /// Set the TCB parameters for a specific keyframe
    pub fn set_tcb_params(&mut self, index: usize, tension: f32, continuity: f32, bias: f32) {
        if index < self.params.len() {
            self.params[index] = TcbParams::new(tension, continuity, bias);
            self.hermite.tangents_dirty = true;
        }
    }

    /// Get the TCB parameters for a specific keyframe
    pub fn get_tcb_params(&self, index: usize) -> Option<TcbParams> {
        self.params.get(index).cloned()
    }

    /// Update tangents using TCB algorithm for 1D
    fn update_tangents_impl(&mut self) {
        if self.hermite.base.keys.len() < 2 {
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

        // Handle endpoints
        if key_count >= 2 {
            // First point
            let tcb_0 = self.params.first().cloned().unwrap_or_default();
            let k2 = 0.25 * ((1.0 - tcb_0.tension) * (1.0 + tcb_0.continuity) * (1.0 - tcb_0.bias));
            let k3 = 0.25 * ((1.0 - tcb_0.tension) * (1.0 - tcb_0.continuity) * (1.0 + tcb_0.bias));

            let dp_out = self.hermite.base.keys[1].point - self.hermite.base.keys[0].point;
            self.hermite.tangents[0].out_tangent = k2 * dp_out + k3 * dp_out;

            // Last point
            let tcb_end = self.params.get(end_idx).cloned().unwrap_or_default();
            let k0 = 0.25
                * ((1.0 - tcb_end.tension) * (1.0 - tcb_end.continuity) * (1.0 - tcb_end.bias));
            let k1 = 0.25
                * ((1.0 - tcb_end.tension) * (1.0 + tcb_end.continuity) * (1.0 + tcb_end.bias));

            let dp_in =
                self.hermite.base.keys[end_idx].point - self.hermite.base.keys[end_idx - 1].point;
            self.hermite.tangents[end_idx].in_tangent = k0 * dp_in + k1 * dp_in;
        }

        // Calculate tangents for interior points
        for i in 1..key_count.saturating_sub(1) {
            let tcb = self.params.get(i).cloned().unwrap_or_default();

            let k0 = 0.5 * ((1.0 - tcb.tension) * (1.0 - tcb.continuity) * (1.0 - tcb.bias));
            let k1 = 0.5 * ((1.0 - tcb.tension) * (1.0 + tcb.continuity) * (1.0 + tcb.bias));
            let k2 = 0.5 * ((1.0 - tcb.tension) * (1.0 + tcb.continuity) * (1.0 - tcb.bias));
            let k3 = 0.5 * ((1.0 - tcb.tension) * (1.0 - tcb.continuity) * (1.0 + tcb.bias));

            let p_prev = self.hermite.base.keys[i - 1].point;
            let p_curr = self.hermite.base.keys[i].point;
            let p_next = self.hermite.base.keys[i + 1].point;

            let dp_in = p_curr - p_prev;
            let dp_out = p_next - p_curr;

            self.hermite.tangents[i].in_tangent = k0 * dp_out + k1 * dp_in;
            self.hermite.tangents[i].out_tangent = k2 * dp_out + k3 * dp_in;
        }

        self.hermite.tangents_dirty = false;
    }
}

impl Curve1D for TcbSpline1D {
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
        let index = self.hermite.add_key(point, time, extra);
        self.params.insert(index, TcbParams::default());
        index
    }

    fn remove_key(&mut self, index: usize) {
        if index < self.params.len() {
            self.params.remove(index);
        }
        self.hermite.remove_key(index);
    }

    fn clear_keys(&mut self) {
        self.params.clear();
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
    fn test_tcb_spline_3d_default() {
        let mut spline = TcbSpline3D::new();

        spline.add_key(Vector3::new(0.0, 0.0, 0.0), 0.0);
        spline.add_key(Vector3::new(10.0, 10.0, 10.0), 0.5);
        spline.add_key(Vector3::new(20.0, 0.0, 20.0), 1.0);

        // With default TCB values (0,0,0), should behave like Catmull-Rom
        let result = spline.evaluate(0.25);
        assert!(result.is_valid());
    }

    #[test]
    fn test_tcb_tension_effect() {
        let mut tight_spline = TcbSpline3D::new();
        let mut loose_spline = TcbSpline3D::new();

        // Same keyframes for both splines
        let points = vec![
            (Vector3::new(0.0, 0.0, 0.0), 0.0),
            (Vector3::new(10.0, 10.0, 10.0), 0.5),
            (Vector3::new(20.0, 0.0, 20.0), 1.0),
        ];

        for (point, time) in &points {
            tight_spline.add_key(*point, *time);
            loose_spline.add_key(*point, *time);
        }

        // Set different tension values for middle point
        tight_spline.set_tcb_params(1, 1.0, 0.0, 0.0); // High tension = tight
        loose_spline.set_tcb_params(1, -1.0, 0.0, 0.0); // Low tension = loose

        let tight_result = tight_spline.evaluate(0.5);
        let loose_result = loose_spline.evaluate(0.5);

        // Different tension should produce different curves
        assert_ne!(tight_result, loose_result);

        // Both should pass through the middle control point
        assert!((tight_result - Vector3::new(10.0, 10.0, 10.0)).length() < 0.01);
        assert!((loose_result - Vector3::new(10.0, 10.0, 10.0)).length() < 0.01);
    }

    #[test]
    fn test_tcb_bias_effect() {
        let mut positive_bias_spline = TcbSpline3D::new();
        let mut negative_bias_spline = TcbSpline3D::new();

        // Create a curve that changes direction
        let points = vec![
            (Vector3::new(0.0, 0.0, 0.0), 0.0),
            (Vector3::new(10.0, 5.0, 10.0), 0.5),
            (Vector3::new(0.0, 10.0, 0.0), 1.0),
        ];

        for (point, time) in &points {
            positive_bias_spline.add_key(*point, *time);
            negative_bias_spline.add_key(*point, *time);
        }

        // Set different bias values for middle point
        positive_bias_spline.set_tcb_params(1, 0.0, 0.0, 0.5); // Positive bias
        negative_bias_spline.set_tcb_params(1, 0.0, 0.0, -0.5); // Negative bias

        let pos_result = positive_bias_spline.evaluate(0.4);
        let neg_result = negative_bias_spline.evaluate(0.6);

        // Different bias should produce different results
        assert_ne!(pos_result, neg_result);
        assert!(pos_result.is_valid());
        assert!(neg_result.is_valid());
    }

    #[test]
    fn test_tcb_continuity_effect() {
        let mut smooth_spline = TcbSpline3D::new();
        let mut sharp_spline = TcbSpline3D::new();

        let points = vec![
            (Vector3::new(0.0, 0.0, 0.0), 0.0),
            (Vector3::new(10.0, 10.0, 10.0), 0.5),
            (Vector3::new(20.0, 0.0, 20.0), 1.0),
        ];

        for (point, time) in &points {
            smooth_spline.add_key(*point, *time);
            sharp_spline.add_key(*point, *time);
        }

        // Set different continuity values
        smooth_spline.set_tcb_params(1, 0.0, 1.0, 0.0); // High continuity = smooth
        sharp_spline.set_tcb_params(1, 0.0, -1.0, 0.0); // Low continuity = sharp

        let smooth_result = smooth_spline.evaluate(0.25);
        let sharp_result = sharp_spline.evaluate(0.25);

        // Different continuity should produce different results
        assert_ne!(smooth_result, sharp_result);
        assert!(smooth_result.is_valid());
        assert!(sharp_result.is_valid());
    }

    #[test]
    fn test_tcb_params_access() {
        let mut spline = TcbSpline3D::new();

        spline.add_key(Vector3::ZERO, 0.0);
        spline.set_tcb_params(0, 0.5, -0.25, 0.75);

        let params = spline.get_tcb_params(0);
        assert!(params.is_some());

        let tcb = params.unwrap();
        assert_eq!(tcb.tension, 0.5);
        assert_eq!(tcb.continuity, -0.25);
        assert_eq!(tcb.bias, 0.75);

        // Test non-existent index
        assert!(spline.get_tcb_params(5).is_none());
    }

    #[test]
    fn test_tcb_spline_1d() {
        let mut spline = TcbSpline1D::new();

        spline.add_key(0.0, 0.0, 0);
        spline.add_key(100.0, 0.5, 0);
        spline.add_key(0.0, 1.0, 0);

        // Set some TCB parameters
        spline.set_tcb_params(1, 0.5, 0.25, -0.5);

        let result = spline.evaluate(0.25);
        assert!(result.is_finite());

        // Test params access
        let params = spline.get_tcb_params(1);
        assert!(params.is_some());
        let tcb = params.unwrap();
        assert_eq!(tcb.tension, 0.5);
        assert_eq!(tcb.continuity, 0.25);
        assert_eq!(tcb.bias, -0.5);
    }
}
