//! Curve and spline implementation
//!
//! This module provides a comprehensive set of curve and spline types for 3D and 1D interpolation.
//! Based on the original C&C Generals Zero Hour WWMath library.

use crate::Vector3;

/// Represents a keyframe with a position and time value
#[derive(Debug, Clone, PartialEq)]
pub struct CurveKey3D {
    pub point: Vector3,
    pub time: f32,
}

impl CurveKey3D {
    pub fn new(point: Vector3, time: f32) -> Self {
        Self { point, time }
    }
}

/// Represents a 1D keyframe with a value and time
#[derive(Debug, Clone, PartialEq)]
pub struct CurveKey1D {
    pub point: f32,
    pub time: f32,
    pub extra: u32, // Additional data field
}

impl CurveKey1D {
    pub fn new(point: f32, time: f32, extra: u32) -> Self {
        Self { point, time, extra }
    }
}

/// Tangent information for Hermite splines
#[derive(Debug, Clone, PartialEq)]
pub struct Tangents3D {
    pub in_tangent: Vector3,
    pub out_tangent: Vector3,
}

impl Tangents3D {
    pub fn new(in_tangent: Vector3, out_tangent: Vector3) -> Self {
        Self {
            in_tangent,
            out_tangent,
        }
    }

    pub fn zero() -> Self {
        Self {
            in_tangent: Vector3::ZERO,
            out_tangent: Vector3::ZERO,
        }
    }
}

/// 1D tangent information
#[derive(Debug, Clone, PartialEq)]
pub struct Tangents1D {
    pub in_tangent: f32,
    pub out_tangent: f32,
}

impl Tangents1D {
    pub fn new(in_tangent: f32, out_tangent: f32) -> Self {
        Self {
            in_tangent,
            out_tangent,
        }
    }

    pub fn zero() -> Self {
        Self {
            in_tangent: 0.0,
            out_tangent: 0.0,
        }
    }
}

/// Trait for 3D curve evaluation and manipulation
pub trait Curve3D {
    /// Evaluate the curve at a given time
    fn evaluate(&mut self, time: f32) -> Vector3;

    /// Check if the curve is looping
    fn is_looping(&self) -> bool;

    /// Set whether the curve should loop
    fn set_looping(&mut self, looping: bool);

    /// Get the number of keyframes
    fn key_count(&self) -> usize;

    /// Get a keyframe at the specified index
    fn get_key(&self, index: usize) -> Option<(Vector3, f32)>;

    /// Set the position of a keyframe at the specified index
    fn set_key(&mut self, index: usize, point: Vector3);

    /// Add a new keyframe, returns the index where it was inserted
    fn add_key(&mut self, point: Vector3, time: f32) -> usize;

    /// Remove a keyframe at the specified index
    fn remove_key(&mut self, index: usize);

    /// Clear all keyframes
    fn clear_keys(&mut self);

    /// Get the start time of the curve
    fn get_start_time(&self) -> f32;

    /// Get the end time of the curve
    fn get_end_time(&self) -> f32;
}

/// Trait for 1D curve evaluation and manipulation
pub trait Curve1D {
    /// Evaluate the curve at a given time
    fn evaluate(&mut self, time: f32) -> f32;

    /// Check if the curve is looping
    fn is_looping(&self) -> bool;

    /// Set whether the curve should loop
    fn set_looping(&mut self, looping: bool);

    /// Get the number of keyframes
    fn key_count(&self) -> usize;

    /// Get a keyframe at the specified index
    fn get_key(&self, index: usize) -> Option<(f32, f32, u32)>;

    /// Set the value of a keyframe at the specified index
    fn set_key(&mut self, index: usize, point: f32, extra: u32);

    /// Add a new keyframe, returns the index where it was inserted
    fn add_key(&mut self, point: f32, time: f32, extra: u32) -> usize;

    /// Remove a keyframe at the specified index
    fn remove_key(&mut self, index: usize);

    /// Clear all keyframes
    fn clear_keys(&mut self);

    /// Get the start time of the curve
    fn get_start_time(&self) -> f32;

    /// Get the end time of the curve
    fn get_end_time(&self) -> f32;
}

/// Base implementation for 3D curves
#[derive(Debug, Clone)]
pub struct BaseCurve3D {
    pub keys: Vec<CurveKey3D>,
    pub is_looping: bool,
}

impl Default for BaseCurve3D {
    fn default() -> Self {
        Self::new()
    }
}

impl BaseCurve3D {
    pub fn new() -> Self {
        Self {
            keys: Vec::new(),
            is_looping: false,
        }
    }

    /// Find the interval containing the given time and return interpolation factor
    pub fn find_interval(&self, time: f32) -> (usize, usize, f32) {
        debug_assert!(!self.keys.is_empty());
        debug_assert!(time >= self.keys[0].time);
        debug_assert!(time <= self.keys[self.keys.len() - 1].time);

        let mut i = 0;
        while i < self.keys.len() - 1 && time > self.keys[i + 1].time {
            i += 1;
        }

        let i0 = i;
        let i1 = i + 1;
        let t = if i1 < self.keys.len() {
            (time - self.keys[i0].time) / (self.keys[i1].time - self.keys[i0].time)
        } else {
            0.0
        };

        (i0, i1, t)
    }
}

/// Base implementation for 1D curves
#[derive(Debug, Clone)]
pub struct BaseCurve1D {
    pub keys: Vec<CurveKey1D>,
    pub is_looping: bool,
}

impl Default for BaseCurve1D {
    fn default() -> Self {
        Self::new()
    }
}

impl BaseCurve1D {
    pub fn new() -> Self {
        Self {
            keys: Vec::new(),
            is_looping: false,
        }
    }

    /// Find the interval containing the given time and return interpolation factor
    pub fn find_interval(&self, time: f32) -> (usize, usize, f32) {
        if self.is_looping {
            self.find_interval_looping(time)
        } else {
            self.find_interval_non_looping(time)
        }
    }

    fn find_interval_non_looping(&self, time: f32) -> (usize, usize, f32) {
        debug_assert!(!self.keys.is_empty());
        debug_assert!(time >= self.keys[0].time);
        debug_assert!(time <= self.keys[self.keys.len() - 1].time);

        let mut i = 0;
        while i < self.keys.len() - 1 && time > self.keys[i + 1].time {
            i += 1;
        }

        let i0 = i;
        let i1 = i + 1;
        let t = if i1 < self.keys.len() {
            (time - self.keys[i0].time) / (self.keys[i1].time - self.keys[i0].time)
        } else {
            0.0
        };

        (i0, i1, t)
    }

    fn find_interval_looping(&self, time: f32) -> (usize, usize, f32) {
        if self.keys.is_empty() {
            return (0, 0, 0.0);
        }

        if time < self.keys[0].time {
            let i0 = self.keys.len() - 1;
            let i1 = 0;
            let interval = 1.0 - self.keys[i0].time + self.keys[i1].time;
            let t = (1.0 - self.keys[i0].time + time) / interval;
            (i0, i1, t)
        } else if time > self.keys[self.keys.len() - 1].time {
            let i0 = self.keys.len() - 1;
            let i1 = 0;
            let interval = 1.0 - self.keys[i0].time + self.keys[i1].time;
            let t = (time - self.keys[i0].time) / interval;
            (i0, i1, t)
        } else {
            self.find_interval_non_looping(time)
        }
    }
}

/// Implements common curve operations for 3D curves
impl Curve3D for BaseCurve3D {
    fn evaluate(&mut self, time: f32) -> Vector3 {
        // This will be overridden by specific curve types
        if self.keys.is_empty() {
            return Vector3::ZERO;
        }

        if time < self.keys[0].time {
            return self.keys[0].point;
        }

        if time >= self.keys[self.keys.len() - 1].time {
            return self.keys[self.keys.len() - 1].point;
        }

        let (i0, i1, t) = self.find_interval(time);

        // Linear interpolation as default
        self.keys[i0].point + t * (self.keys[i1].point - self.keys[i0].point)
    }

    fn is_looping(&self) -> bool {
        self.is_looping
    }

    fn set_looping(&mut self, looping: bool) {
        self.is_looping = looping;
    }

    fn key_count(&self) -> usize {
        self.keys.len()
    }

    fn get_key(&self, index: usize) -> Option<(Vector3, f32)> {
        self.keys.get(index).map(|key| (key.point, key.time))
    }

    fn set_key(&mut self, index: usize, point: Vector3) {
        if let Some(key) = self.keys.get_mut(index) {
            key.point = point;
        }
    }

    fn add_key(&mut self, point: Vector3, time: f32) -> usize {
        // Find insertion point to maintain time ordering
        let mut index = 0;
        while index < self.keys.len() && self.keys[index].time < time {
            index += 1;
        }

        let new_key = CurveKey3D::new(point, time);
        self.keys.insert(index, new_key);
        index
    }

    fn remove_key(&mut self, index: usize) {
        if index < self.keys.len() {
            self.keys.remove(index);
        }
    }

    fn clear_keys(&mut self) {
        self.keys.clear();
    }

    fn get_start_time(&self) -> f32 {
        self.keys.first().map(|key| key.time).unwrap_or(0.0)
    }

    fn get_end_time(&self) -> f32 {
        self.keys.last().map(|key| key.time).unwrap_or(0.0)
    }
}

/// Implements common curve operations for 1D curves
impl Curve1D for BaseCurve1D {
    fn evaluate(&mut self, time: f32) -> f32 {
        if self.keys.is_empty() {
            return 0.0;
        }

        if !self.is_looping {
            if time < self.keys[0].time {
                return self.keys[0].point;
            }

            if time >= self.keys[self.keys.len() - 1].time {
                return self.keys[self.keys.len() - 1].point;
            }
        }

        let (i0, i1, t) = self.find_interval(time);

        if i1 >= self.keys.len() {
            return self.keys[i0].point;
        }

        // Linear interpolation as default
        self.keys[i0].point + t * (self.keys[i1].point - self.keys[i0].point)
    }

    fn is_looping(&self) -> bool {
        self.is_looping
    }

    fn set_looping(&mut self, looping: bool) {
        self.is_looping = looping;
    }

    fn key_count(&self) -> usize {
        self.keys.len()
    }

    fn get_key(&self, index: usize) -> Option<(f32, f32, u32)> {
        self.keys
            .get(index)
            .map(|key| (key.point, key.time, key.extra))
    }

    fn set_key(&mut self, index: usize, point: f32, extra: u32) {
        if let Some(key) = self.keys.get_mut(index) {
            key.point = point;
            key.extra = extra;
        }
    }

    fn add_key(&mut self, point: f32, time: f32, extra: u32) -> usize {
        // Find insertion point to maintain time ordering
        let mut index = 0;
        while index < self.keys.len() && self.keys[index].time < time {
            index += 1;
        }

        let new_key = CurveKey1D::new(point, time, extra);
        self.keys.insert(index, new_key);
        index
    }

    fn remove_key(&mut self, index: usize) {
        if index < self.keys.len() {
            self.keys.remove(index);
        }
    }

    fn clear_keys(&mut self) {
        self.keys.clear();
    }

    fn get_start_time(&self) -> f32 {
        self.keys.first().map(|key| key.time).unwrap_or(0.0)
    }

    fn get_end_time(&self) -> f32 {
        self.keys.last().map(|key| key.time).unwrap_or(0.0)
    }
}

/// Linear curve for 3D interpolation
#[derive(Debug, Clone)]
pub struct LinearCurve3D {
    pub base: BaseCurve3D,
}

impl Default for LinearCurve3D {
    fn default() -> Self {
        Self::new()
    }
}

impl LinearCurve3D {
    pub fn new() -> Self {
        Self {
            base: BaseCurve3D::new(),
        }
    }
}

impl Curve3D for LinearCurve3D {
    fn evaluate(&mut self, time: f32) -> Vector3 {
        self.base.evaluate(time)
    }

    fn is_looping(&self) -> bool {
        self.base.is_looping()
    }
    fn set_looping(&mut self, looping: bool) {
        self.base.set_looping(looping)
    }
    fn key_count(&self) -> usize {
        self.base.key_count()
    }
    fn get_key(&self, index: usize) -> Option<(Vector3, f32)> {
        self.base.get_key(index)
    }
    fn set_key(&mut self, index: usize, point: Vector3) {
        self.base.set_key(index, point)
    }
    fn add_key(&mut self, point: Vector3, time: f32) -> usize {
        self.base.add_key(point, time)
    }
    fn remove_key(&mut self, index: usize) {
        self.base.remove_key(index)
    }
    fn clear_keys(&mut self) {
        self.base.clear_keys()
    }
    fn get_start_time(&self) -> f32 {
        self.base.get_start_time()
    }
    fn get_end_time(&self) -> f32 {
        self.base.get_end_time()
    }
}

/// Linear curve for 1D interpolation
#[derive(Debug, Clone)]
pub struct LinearCurve1D {
    pub base: BaseCurve1D,
}

impl Default for LinearCurve1D {
    fn default() -> Self {
        Self::new()
    }
}

impl LinearCurve1D {
    pub fn new() -> Self {
        Self {
            base: BaseCurve1D::new(),
        }
    }
}

impl Curve1D for LinearCurve1D {
    fn evaluate(&mut self, time: f32) -> f32 {
        self.base.evaluate(time)
    }

    fn is_looping(&self) -> bool {
        self.base.is_looping()
    }
    fn set_looping(&mut self, looping: bool) {
        self.base.set_looping(looping)
    }
    fn key_count(&self) -> usize {
        self.base.key_count()
    }
    fn get_key(&self, index: usize) -> Option<(f32, f32, u32)> {
        self.base.get_key(index)
    }
    fn set_key(&mut self, index: usize, point: f32, extra: u32) {
        self.base.set_key(index, point, extra)
    }
    fn add_key(&mut self, point: f32, time: f32, extra: u32) -> usize {
        self.base.add_key(point, time, extra)
    }
    fn remove_key(&mut self, index: usize) {
        self.base.remove_key(index)
    }
    fn clear_keys(&mut self) {
        self.base.clear_keys()
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
    fn test_linear_curve_3d() {
        let mut curve = LinearCurve3D::new();

        // Add some keyframes
        curve.add_key(Vector3::new(0.0, 0.0, 0.0), 0.0);
        curve.add_key(Vector3::new(10.0, 10.0, 10.0), 1.0);

        // Test evaluation
        let result = curve.evaluate(0.5);
        assert_eq!(result, Vector3::new(5.0, 5.0, 5.0));

        // Test bounds
        let start = curve.evaluate(0.0);
        assert_eq!(start, Vector3::new(0.0, 0.0, 0.0));

        let end = curve.evaluate(1.0);
        assert_eq!(end, Vector3::new(10.0, 10.0, 10.0));
    }

    #[test]
    fn test_linear_curve_1d() {
        let mut curve = LinearCurve1D::new();

        // Add some keyframes
        curve.add_key(0.0, 0.0, 0);
        curve.add_key(100.0, 1.0, 1);

        // Test evaluation
        let result = curve.evaluate(0.5);
        assert_eq!(result, 50.0);

        // Test bounds
        let start = curve.evaluate(0.0);
        assert_eq!(start, 0.0);

        let end = curve.evaluate(1.0);
        assert_eq!(end, 100.0);
    }

    #[test]
    fn test_key_management() {
        let mut curve = LinearCurve3D::new();

        assert_eq!(curve.key_count(), 0);

        let index = curve.add_key(Vector3::new(1.0, 2.0, 3.0), 0.5);
        assert_eq!(curve.key_count(), 1);
        assert_eq!(index, 0);

        curve.add_key(Vector3::new(0.0, 0.0, 0.0), 0.0); // Should be inserted at beginning
        assert_eq!(curve.key_count(), 2);

        if let Some((point, time)) = curve.get_key(0) {
            assert_eq!(point, Vector3::new(0.0, 0.0, 0.0));
            assert_eq!(time, 0.0);
        }

        curve.remove_key(0);
        assert_eq!(curve.key_count(), 1);

        curve.clear_keys();
        assert_eq!(curve.key_count(), 0);
    }
}
