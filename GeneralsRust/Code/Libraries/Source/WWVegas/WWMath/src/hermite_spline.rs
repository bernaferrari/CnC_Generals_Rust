//! Hermite spline implementation
//!
//! Hermite splines use explicit tangent vectors to control the shape of the curve.
//! They provide precise control over curve behavior at keyframes.

use crate::curve::{BaseCurve1D, BaseCurve3D, Curve1D, Curve3D, Tangents1D, Tangents3D};
use crate::{Vector3, WWMath};

/// 3D Hermite spline
#[derive(Debug, Clone)]
pub struct HermiteSpline3D {
    pub base: BaseCurve3D,
    pub tangents: Vec<Tangents3D>,
    pub tangents_dirty: bool,
}

impl Default for HermiteSpline3D {
    fn default() -> Self {
        Self::new()
    }
}

impl HermiteSpline3D {
    pub fn new() -> Self {
        Self {
            base: BaseCurve3D::new(),
            tangents: Vec::new(),
            tangents_dirty: true,
        }
    }

    /// Set the in and out tangents for a specific keyframe
    pub fn set_tangents(&mut self, index: usize, in_tangent: Vector3, out_tangent: Vector3) {
        if index < self.tangents.len() {
            self.tangents[index].in_tangent = in_tangent;
            self.tangents[index].out_tangent = out_tangent;
        }
    }

    /// Get the in and out tangents for a specific keyframe
    pub fn get_tangents(&self, index: usize) -> Option<(Vector3, Vector3)> {
        self.tangents
            .get(index)
            .map(|t| (t.in_tangent, t.out_tangent))
    }

    /// Evaluate the curve and its derivative at a given time
    pub fn evaluate_derivative(&mut self, time: f32) -> Vector3 {
        if self.base.keys.is_empty() {
            return Vector3::ZERO;
        }

        // Clamp time to curve bounds for derivative evaluation
        let min_time = self.base.keys[0].time;
        let max_time = self.base.keys[self.base.keys.len() - 1].time;
        let clamped_time = WWMath::clamp(time, min_time, max_time);

        if self.tangents_dirty {
            self.update_tangents();
        }

        let (i0, i1, t) = self.base.find_interval(clamped_time);

        if i1 >= self.base.keys.len() {
            return Vector3::ZERO;
        }

        let t2 = t * t;

        // Derivatives of Hermite basis functions
        let dh0 = 6.0 * t2 - 6.0 * t;
        let dh1 = -6.0 * t2 + 6.0 * t;
        let dh2 = 3.0 * t2 - 4.0 * t + 1.0;
        let dh3 = 3.0 * t2 - 2.0 * t;

        let key0 = &self.base.keys[i0];
        let key1 = &self.base.keys[i1];
        let tang0 = &self.tangents[i0];
        let tang1 = &self.tangents[i1];

        Vector3::new(
            dh0 * key0.point.x
                + dh1 * key1.point.x
                + dh2 * tang0.out_tangent.x
                + dh3 * tang1.in_tangent.x,
            dh0 * key0.point.y
                + dh1 * key1.point.y
                + dh2 * tang0.out_tangent.y
                + dh3 * tang1.in_tangent.y,
            dh0 * key0.point.z
                + dh1 * key1.point.z
                + dh2 * tang0.out_tangent.z
                + dh3 * tang1.in_tangent.z,
        )
    }

    /// Update tangents - override this in derived classes for automatic tangent computation
    pub fn update_tangents(&mut self) {
        self.tangents_dirty = false;
    }
}

impl Curve3D for HermiteSpline3D {
    fn evaluate(&mut self, time: f32) -> Vector3 {
        if self.base.keys.is_empty() {
            return Vector3::ZERO;
        }

        // Handle out-of-bounds cases
        if time < self.base.keys[0].time {
            return self.base.keys[0].point;
        }

        if time > self.base.keys[self.base.keys.len() - 1].time {
            return self.base.keys[self.base.keys.len() - 1].point;
        }

        if self.tangents_dirty {
            self.update_tangents();
        }

        let (i0, i1, t) = self.base.find_interval(time);

        if i1 >= self.base.keys.len() {
            return self.base.keys[i0].point;
        }

        let t2 = t * t;
        let t3 = t2 * t;

        // Hermite basis functions
        let h0 = 2.0 * t3 - 3.0 * t2 + 1.0;
        let h1 = -2.0 * t3 + 3.0 * t2;
        let h2 = t3 - 2.0 * t2 + t;
        let h3 = t3 - t2;

        let key0 = &self.base.keys[i0];
        let key1 = &self.base.keys[i1];
        let tang0 = &self.tangents[i0];
        let tang1 = &self.tangents[i1];

        Vector3::new(
            h0 * key0.point.x
                + h1 * key1.point.x
                + h2 * tang0.out_tangent.x
                + h3 * tang1.in_tangent.x,
            h0 * key0.point.y
                + h1 * key1.point.y
                + h2 * tang0.out_tangent.y
                + h3 * tang1.in_tangent.y,
            h0 * key0.point.z
                + h1 * key1.point.z
                + h2 * tang0.out_tangent.z
                + h3 * tang1.in_tangent.z,
        )
    }

    fn is_looping(&self) -> bool {
        self.base.is_looping
    }

    fn set_looping(&mut self, looping: bool) {
        if self.base.is_looping != looping {
            self.base.is_looping = looping;
            self.tangents_dirty = true;
        }
    }

    fn key_count(&self) -> usize {
        self.base.keys.len()
    }

    fn get_key(&self, index: usize) -> Option<(Vector3, f32)> {
        self.base.get_key(index)
    }

    fn set_key(&mut self, index: usize, point: Vector3) {
        self.base.set_key(index, point);
        self.tangents_dirty = true;
    }

    fn add_key(&mut self, point: Vector3, time: f32) -> usize {
        let index = self.base.add_key(point, time);
        self.tangents.insert(index, Tangents3D::zero());
        self.tangents_dirty = true;
        index
    }

    fn remove_key(&mut self, index: usize) {
        if index < self.tangents.len() {
            self.tangents.remove(index);
        }
        self.base.remove_key(index);
        self.tangents_dirty = true;
    }

    fn clear_keys(&mut self) {
        self.base.clear_keys();
        self.tangents.clear();
        self.tangents_dirty = true;
    }

    fn get_start_time(&self) -> f32 {
        self.base.get_start_time()
    }

    fn get_end_time(&self) -> f32 {
        self.base.get_end_time()
    }
}

/// 1D Hermite spline
#[derive(Debug, Clone)]
pub struct HermiteSpline1D {
    pub base: BaseCurve1D,
    pub tangents: Vec<Tangents1D>,
    pub tangents_dirty: bool,
}

impl Default for HermiteSpline1D {
    fn default() -> Self {
        Self::new()
    }
}

impl HermiteSpline1D {
    pub fn new() -> Self {
        Self {
            base: BaseCurve1D::new(),
            tangents: Vec::new(),
            tangents_dirty: true,
        }
    }

    /// Set the in and out tangents for a specific keyframe
    pub fn set_tangents(&mut self, index: usize, in_tangent: f32, out_tangent: f32) {
        if index < self.tangents.len() {
            self.tangents[index].in_tangent = in_tangent;
            self.tangents[index].out_tangent = out_tangent;
        }
    }

    /// Get the in and out tangents for a specific keyframe
    pub fn get_tangents(&self, index: usize) -> Option<(f32, f32)> {
        self.tangents
            .get(index)
            .map(|t| (t.in_tangent, t.out_tangent))
    }

    /// Update tangents - override this in derived classes for automatic tangent computation
    pub fn update_tangents(&mut self) {
        self.tangents_dirty = false;
    }
}

impl Curve1D for HermiteSpline1D {
    fn evaluate(&mut self, time: f32) -> f32 {
        if self.base.keys.is_empty() {
            return 0.0;
        }

        if self.base.keys.len() == 1 {
            return self.base.keys[0].point;
        }

        if !self.base.is_looping {
            // Handle out-of-bounds cases for non-looping curves
            if time < self.base.keys[0].time {
                return self.base.keys[0].point;
            }

            if time > self.base.keys[self.base.keys.len() - 1].time {
                return self.base.keys[self.base.keys.len() - 1].point;
            }
        }

        if self.tangents_dirty {
            self.update_tangents();
        }

        let (i0, i1, t) = self.base.find_interval(time);

        if i1 >= self.base.keys.len() {
            return self.base.keys[i0].point;
        }

        let t2 = t * t;
        let t3 = t2 * t;

        // Hermite basis functions
        let h0 = 2.0 * t3 - 3.0 * t2 + 1.0;
        let h1 = -2.0 * t3 + 3.0 * t2;
        let h2 = t3 - 2.0 * t2 + t;
        let h3 = t3 - t2;

        let key0 = &self.base.keys[i0];
        let key1 = &self.base.keys[i1];
        let tang0 = &self.tangents[i0];
        let tang1 = &self.tangents[i1];

        h0 * key0.point + h1 * key1.point + h2 * tang0.out_tangent + h3 * tang1.in_tangent
    }

    fn is_looping(&self) -> bool {
        self.base.is_looping
    }

    fn set_looping(&mut self, looping: bool) {
        if self.base.is_looping != looping {
            self.base.is_looping = looping;
            self.tangents_dirty = true;
        }
    }

    fn key_count(&self) -> usize {
        self.base.keys.len()
    }

    fn get_key(&self, index: usize) -> Option<(f32, f32, u32)> {
        self.base.get_key(index)
    }

    fn set_key(&mut self, index: usize, point: f32, extra: u32) {
        self.base.set_key(index, point, extra);
        self.tangents_dirty = true;
    }

    fn add_key(&mut self, point: f32, time: f32, extra: u32) -> usize {
        let index = self.base.add_key(point, time, extra);
        self.tangents.insert(index, Tangents1D::zero());
        self.tangents_dirty = true;
        index
    }

    fn remove_key(&mut self, index: usize) {
        if index < self.tangents.len() {
            self.tangents.remove(index);
        }
        self.base.remove_key(index);
        self.tangents_dirty = true;
    }

    fn clear_keys(&mut self) {
        self.base.clear_keys();
        self.tangents.clear();
        self.tangents_dirty = true;
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
    fn test_hermite_spline_3d_basic() {
        let mut spline = HermiteSpline3D::new();

        spline.add_key(Vector3::new(0.0, 0.0, 0.0), 0.0);
        spline.add_key(Vector3::new(10.0, 10.0, 10.0), 1.0);

        // Set tangents for smooth curve
        spline.set_tangents(0, Vector3::ZERO, Vector3::new(5.0, 5.0, 5.0));
        spline.set_tangents(1, Vector3::new(5.0, 5.0, 5.0), Vector3::ZERO);

        let result = spline.evaluate(0.5);
        // Should be smooth curve, not linear interpolation
        assert!(result != Vector3::new(5.0, 5.0, 5.0));
    }

    #[test]
    fn test_hermite_spline_1d_basic() {
        let mut spline = HermiteSpline1D::new();

        spline.add_key(0.0, 0.0, 0);
        spline.add_key(100.0, 1.0, 0);

        // Set tangents
        spline.set_tangents(0, 0.0, 50.0);
        spline.set_tangents(1, 50.0, 0.0);

        let result = spline.evaluate(0.5);
        // Should be smooth curve, not linear interpolation
        assert!(result != 50.0);
    }

    #[test]
    fn test_hermite_tangent_access() {
        let mut spline = HermiteSpline3D::new();

        spline.add_key(Vector3::ZERO, 0.0);
        spline.set_tangents(0, Vector3::new(1.0, 2.0, 3.0), Vector3::new(4.0, 5.0, 6.0));

        let tangents = spline.get_tangents(0);
        assert!(tangents.is_some());

        let (in_tan, out_tan) = tangents.unwrap();
        assert_eq!(in_tan, Vector3::new(1.0, 2.0, 3.0));
        assert_eq!(out_tan, Vector3::new(4.0, 5.0, 6.0));
    }

    #[test]
    fn test_hermite_derivative() {
        let mut spline = HermiteSpline3D::new();

        spline.add_key(Vector3::new(0.0, 0.0, 0.0), 0.0);
        spline.add_key(Vector3::new(10.0, 0.0, 0.0), 1.0);

        // Set horizontal tangents
        spline.set_tangents(0, Vector3::ZERO, Vector3::new(10.0, 0.0, 0.0));
        spline.set_tangents(1, Vector3::new(10.0, 0.0, 0.0), Vector3::ZERO);

        let derivative = spline.evaluate_derivative(0.0);
        // At start, derivative should match out tangent
        assert_eq!(derivative.x, 10.0);
    }
}
