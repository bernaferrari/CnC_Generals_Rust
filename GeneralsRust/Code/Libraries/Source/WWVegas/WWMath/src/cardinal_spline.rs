//! Cardinal spline implementation
//!
//! Cardinal splines are a special case of Hermite splines where tangents are automatically
//! calculated based on neighboring points and a tightness parameter.

use crate::curve::{Curve1D, Curve3D};
use crate::hermite_spline::{HermiteSpline1D, HermiteSpline3D};
use crate::Vector3;

/// 3D Cardinal spline
#[derive(Debug, Clone)]
pub struct CardinalSpline3D {
    pub hermite: HermiteSpline3D,
    pub tightness: Vec<f32>,
}

impl Default for CardinalSpline3D {
    fn default() -> Self {
        Self::new()
    }
}

impl CardinalSpline3D {
    pub fn new() -> Self {
        Self {
            hermite: HermiteSpline3D::new(),
            tightness: Vec::new(),
        }
    }

    /// Set the tightness parameter for a specific keyframe
    /// Tightness of 0.0 = loose, 1.0 = tight
    pub fn set_tightness(&mut self, index: usize, tightness: f32) {
        if index < self.tightness.len() {
            self.tightness[index] = tightness;
            self.hermite.tangents_dirty = true;
        }
    }

    /// Get the tightness parameter for a specific keyframe
    pub fn get_tightness(&self, index: usize) -> Option<f32> {
        self.tightness.get(index).copied()
    }

    /// Update tangents based on neighboring points and tightness values
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
            let tightness_0 = self.tightness.first().copied().unwrap_or(0.5);
            let tangent_scale = 1.0 - tightness_0;

            let p_prev = self.hermite.base.keys[end_idx - 1].point;
            let _p_curr = self.hermite.base.keys[0].point;
            let p_next = self.hermite.base.keys[1].point;

            self.hermite.tangents[0].out_tangent = tangent_scale * (p_next - p_prev);
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
            // Non-looping curve - handle endpoints specially
            if key_count >= 2 {
                let tightness_0 = self.tightness.first().copied().unwrap_or(0.5);
                let tangent_scale = 1.0 - tightness_0;

                self.hermite.tangents[0].out_tangent = tangent_scale
                    * (self.hermite.base.keys[1].point - self.hermite.base.keys[0].point);

                let tightness_end = self.tightness.get(end_idx).copied().unwrap_or(0.5);
                let tangent_scale_end = 1.0 - tightness_end;

                self.hermite.tangents[end_idx].in_tangent = tangent_scale_end
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

        // Calculate tangents for interior points
        for i in 1..key_count.saturating_sub(1) {
            let tightness_i = self.tightness.get(i).copied().unwrap_or(0.5);
            let tangent_scale = 1.0 - tightness_i;

            let p_prev = self.hermite.base.keys[i - 1].point;
            let _p_curr = self.hermite.base.keys[i].point;
            let p_next = self.hermite.base.keys[i + 1].point;

            let tangent = tangent_scale * (p_next - p_prev);
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

impl Curve3D for CardinalSpline3D {
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
        self.tightness.insert(index, 0.5); // Default tightness
        index
    }

    fn remove_key(&mut self, index: usize) {
        if index < self.tightness.len() {
            self.tightness.remove(index);
        }
        self.hermite.remove_key(index);
    }

    fn clear_keys(&mut self) {
        self.tightness.clear();
        self.hermite.clear_keys();
    }

    fn get_start_time(&self) -> f32 {
        self.hermite.get_start_time()
    }

    fn get_end_time(&self) -> f32 {
        self.hermite.get_end_time()
    }
}

/// 1D Cardinal spline
#[derive(Debug, Clone)]
pub struct CardinalSpline1D {
    pub hermite: HermiteSpline1D,
    pub tightness: Vec<f32>,
}

impl Default for CardinalSpline1D {
    fn default() -> Self {
        Self::new()
    }
}

impl CardinalSpline1D {
    pub fn new() -> Self {
        Self {
            hermite: HermiteSpline1D::new(),
            tightness: Vec::new(),
        }
    }

    /// Set the tightness parameter for a specific keyframe
    pub fn set_tightness(&mut self, index: usize, tightness: f32) {
        if index < self.tightness.len() {
            self.tightness[index] = tightness;
            self.hermite.tangents_dirty = true;
        }
    }

    /// Get the tightness parameter for a specific keyframe
    pub fn get_tightness(&self, index: usize) -> Option<f32> {
        self.tightness.get(index).copied()
    }

    /// Update tangents based on neighboring points and tightness values
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

        if self.hermite.base.is_looping && key_count > 2 {
            // Looping curve
            let tightness_0 = self.tightness.first().copied().unwrap_or(0.5);
            let tangent_scale = 1.0 - tightness_0;

            let p_prev = self.hermite.base.keys[end_idx - 1].point;
            let _p_curr = self.hermite.base.keys[0].point;
            let p_next = self.hermite.base.keys[1].point;

            self.hermite.tangents[0].out_tangent = tangent_scale * (p_next - p_prev);
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
            // Non-looping curve
            if key_count >= 2 {
                let tightness_0 = self.tightness.first().copied().unwrap_or(0.5);
                let tangent_scale = 1.0 - tightness_0;

                self.hermite.tangents[0].out_tangent = tangent_scale
                    * (self.hermite.base.keys[1].point - self.hermite.base.keys[0].point);

                let tightness_end = self.tightness.get(end_idx).copied().unwrap_or(0.5);
                let tangent_scale_end = 1.0 - tightness_end;

                self.hermite.tangents[end_idx].in_tangent = tangent_scale_end
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

        // Calculate tangents for interior points
        for i in 1..key_count.saturating_sub(1) {
            let tightness_i = self.tightness.get(i).copied().unwrap_or(0.5);
            let tangent_scale = 1.0 - tightness_i;

            let p_prev = self.hermite.base.keys[i - 1].point;
            let p_next = self.hermite.base.keys[i + 1].point;

            let tangent = tangent_scale * (p_next - p_prev);
            self.hermite.tangents[i].in_tangent = tangent;
            self.hermite.tangents[i].out_tangent = tangent;

            // Apply time-based scaling
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

impl Curve1D for CardinalSpline1D {
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
        self.tightness.insert(index, 0.5); // Default tightness
        index
    }

    fn remove_key(&mut self, index: usize) {
        if index < self.tightness.len() {
            self.tightness.remove(index);
        }
        self.hermite.remove_key(index);
    }

    fn clear_keys(&mut self) {
        self.tightness.clear();
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
    fn test_cardinal_spline_3d() {
        let mut spline = CardinalSpline3D::new();

        spline.add_key(Vector3::new(0.0, 0.0, 0.0), 0.0);
        spline.add_key(Vector3::new(10.0, 10.0, 10.0), 0.5);
        spline.add_key(Vector3::new(20.0, 0.0, 20.0), 1.0);

        // Test different tightness values
        spline.set_tightness(1, 0.0); // Very loose
        let loose_result = spline.evaluate(0.25);

        spline.set_tightness(1, 1.0); // Very tight
        let tight_result = spline.evaluate(0.25);

        // Different tightness should produce different results
        assert_ne!(loose_result, tight_result);
    }

    #[test]
    fn test_cardinal_spline_1d() {
        let mut spline = CardinalSpline1D::new();

        spline.add_key(0.0, 0.0, 0);
        spline.add_key(100.0, 0.5, 0);
        spline.add_key(0.0, 1.0, 0);

        // Test that it creates a smooth curve
        let result = spline.evaluate(0.25);
        // Should be influenced by middle point
        assert!(result > 0.0);
    }

    #[test]
    fn test_tightness_access() {
        let mut spline = CardinalSpline3D::new();

        spline.add_key(Vector3::ZERO, 0.0);
        spline.set_tightness(0, 0.75);

        assert_eq!(spline.get_tightness(0), Some(0.75));
        assert_eq!(spline.get_tightness(1), None);
    }

    #[test]
    fn test_looping_cardinal() {
        let mut spline = CardinalSpline3D::new();

        spline.add_key(Vector3::new(0.0, 0.0, 0.0), 0.0);
        spline.add_key(Vector3::new(10.0, 10.0, 10.0), 0.25);
        spline.add_key(Vector3::new(0.0, 20.0, 0.0), 0.5);
        spline.add_key(Vector3::new(-10.0, 10.0, -10.0), 0.75);
        spline.add_key(Vector3::new(0.0, 0.0, 0.0), 1.0); // Same as first for smooth loop

        spline.set_looping(true);

        // Should produce smooth results across the loop boundary
        let result_before = spline.evaluate(0.9);
        let result_after = spline.evaluate(0.1);

        // Both results should be valid (no panics or NaN values)
        assert!(result_before.is_valid());
        assert!(result_after.is_valid());
    }
}
