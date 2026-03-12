//! Primitive Animation System
//!
//! Provides animated channels for procedural effects like rings, spheres, shields, and particles.
//! This system supports keyframe animation with linear interpolation (LERP) and other
//! interpolation methods.
//!
//! The C++ equivalent is in `prim_anim.cpp/h`.

use std::fmt::Debug;

/// Trait for types that can be linearly interpolated
pub trait Interpolate: Clone {
    /// Linear interpolation between self and other
    /// t = 0.0 returns self, t = 1.0 returns other
    fn lerp(&self, other: &Self, t: f32) -> Self;
}

// Implement Interpolate for common types
impl Interpolate for f32 {
    fn lerp(&self, other: &Self, t: f32) -> Self {
        self + (other - self) * t
    }
}

impl Interpolate for glam::Vec2 {
    fn lerp(&self, other: &Self, t: f32) -> Self {
        glam::Vec2::lerp(*self, *other, t)
    }
}

impl Interpolate for glam::Vec3 {
    fn lerp(&self, other: &Self, t: f32) -> Self {
        glam::Vec3::lerp(*self, *other, t)
    }
}

impl Interpolate for glam::Vec4 {
    fn lerp(&self, other: &Self, t: f32) -> Self {
        glam::Vec4::lerp(*self, *other, t)
    }
}

impl Interpolate for glam::Quat {
    fn lerp(&self, other: &Self, t: f32) -> Self {
        self.slerp(*other, t)
    }
}

/// Animation keyframe containing a value and time
#[derive(Debug, Clone)]
pub struct AnimationKey<T: Clone> {
    /// The value at this keyframe
    pub value: T,
    /// Time of this keyframe in seconds
    pub time: f32,
}

impl<T: Clone> AnimationKey<T> {
    pub fn new(value: T, time: f32) -> Self {
        Self { value, time }
    }

    pub fn get_time(&self) -> f32 {
        self.time
    }

    pub fn get_value(&self) -> &T {
        &self.value
    }

    pub fn set_time(&mut self, time: f32) {
        self.time = time;
    }

    pub fn set_value(&mut self, value: T) {
        self.value = value;
    }
}

/// Base animation channel trait
pub trait AnimationChannel<T: Clone + Interpolate> {
    /// Evaluate the animation at a given time
    fn evaluate(&mut self, time: f32) -> T;

    /// Get the number of keyframes
    fn get_key_count(&self) -> usize;

    /// Get a keyframe by index
    fn get_key(&self, index: usize) -> Option<&AnimationKey<T>>;

    /// Set a keyframe at an index
    fn set_key(&mut self, index: usize, value: T, time: f32);

    /// Set only the value of a keyframe
    fn set_key_value(&mut self, index: usize, value: T);

    /// Add a keyframe to the end
    fn add_key(&mut self, value: T, time: f32);

    /// Insert a keyframe at a specific index
    fn insert_key(&mut self, index: usize, value: T, time: f32);

    /// Delete a keyframe at an index
    fn delete_key(&mut self, index: usize);

    /// Reset the animation channel
    fn reset(&mut self);
}

/// Linear interpolation animation channel
///
/// This is the most common type of animation channel, providing smooth
/// linear transitions between keyframes.
#[derive(Debug, Clone)]
pub struct LERPAnimationChannel<T: Clone + Interpolate> {
    /// Keyframe data
    keys: Vec<AnimationKey<T>>,
    /// Last accessed keyframe index (optimization for sequential playback)
    last_index: usize,
}

impl<T: Clone + Interpolate> LERPAnimationChannel<T> {
    /// Create a new empty animation channel
    pub fn new() -> Self {
        Self {
            keys: Vec::new(),
            last_index: 0,
        }
    }

    /// Create with an initial capacity
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            keys: Vec::with_capacity(capacity),
            last_index: 0,
        }
    }

    /// Find the two keyframes surrounding a given time
    fn find_keys(&mut self, time: f32) -> Option<(usize, usize)> {
        if self.keys.is_empty() {
            return None;
        }

        // If only one key, return it twice
        if self.keys.len() == 1 {
            return Some((0, 0));
        }

        // Check if time is before first key
        if time <= self.keys[0].time {
            return Some((0, 0));
        }

        // Check if time is after last key
        if time >= self.keys[self.keys.len() - 1].time {
            let last = self.keys.len() - 1;
            return Some((last, last));
        }

        // Reset last_index if it's invalid
        if self.last_index >= self.keys.len() || time < self.keys[self.last_index].time {
            self.last_index = 0;
        }

        // Search for the surrounding keys starting from last_index
        for i in self.last_index..(self.keys.len() - 1) {
            if time >= self.keys[i].time && time < self.keys[i + 1].time {
                self.last_index = i;
                return Some((i, i + 1));
            }
        }

        // Fallback to last key if not found
        let last = self.keys.len() - 1;
        Some((last, last))
    }
}

impl<T: Clone + Interpolate> AnimationChannel<T> for LERPAnimationChannel<T> {
    fn evaluate(&mut self, time: f32) -> T {
        if self.keys.is_empty() {
            panic!("Cannot evaluate empty animation channel");
        }

        if self.keys.len() == 1 {
            return self.keys[0].value.clone();
        }

        // Find surrounding keyframes
        if let Some((idx1, idx2)) = self.find_keys(time) {
            if idx1 == idx2 {
                // Exact match or outside range
                return self.keys[idx1].value.clone();
            }

            let key1 = &self.keys[idx1];
            let key2 = &self.keys[idx2];

            // Calculate interpolation factor
            let time_diff = key2.time - key1.time;
            let t = if time_diff > 0.0 {
                (time - key1.time) / time_diff
            } else {
                0.0
            };

            // Interpolate between the two values
            key1.value.lerp(&key2.value, t.clamp(0.0, 1.0))
        } else {
            self.keys[0].value.clone()
        }
    }

    fn get_key_count(&self) -> usize {
        self.keys.len()
    }

    fn get_key(&self, index: usize) -> Option<&AnimationKey<T>> {
        self.keys.get(index)
    }

    fn set_key(&mut self, index: usize, value: T, time: f32) {
        if index < self.keys.len() {
            self.keys[index].value = value;
            self.keys[index].time = time;
        }
    }

    fn set_key_value(&mut self, index: usize, value: T) {
        if index < self.keys.len() {
            self.keys[index].value = value;
        }
    }

    fn add_key(&mut self, value: T, time: f32) {
        self.keys.push(AnimationKey::new(value, time));
        // Sort keys by time to maintain proper order
        self.keys
            .sort_by(|a, b| a.time.partial_cmp(&b.time).unwrap());
    }

    fn insert_key(&mut self, index: usize, value: T, time: f32) {
        if index <= self.keys.len() {
            self.keys.insert(index, AnimationKey::new(value, time));
            // Sort keys by time to maintain proper order
            self.keys
                .sort_by(|a, b| a.time.partial_cmp(&b.time).unwrap());
        }
    }

    fn delete_key(&mut self, index: usize) {
        if index < self.keys.len() {
            self.keys.remove(index);
            if self.last_index >= self.keys.len() && self.last_index > 0 {
                self.last_index = self.keys.len() - 1;
            }
        }
    }

    fn reset(&mut self) {
        self.keys.clear();
        self.last_index = 0;
    }
}

impl<T: Clone + Interpolate> Default for LERPAnimationChannel<T> {
    fn default() -> Self {
        Self::new()
    }
}

/// Step animation channel (no interpolation, holds value until next key)
#[derive(Debug, Clone)]
pub struct StepAnimationChannel<T: Clone + Interpolate> {
    keys: Vec<AnimationKey<T>>,
}

impl<T: Clone + Interpolate> StepAnimationChannel<T> {
    pub fn new() -> Self {
        Self { keys: Vec::new() }
    }
}

impl<T: Clone + Interpolate> AnimationChannel<T> for StepAnimationChannel<T> {
    fn evaluate(&mut self, time: f32) -> T {
        if self.keys.is_empty() {
            panic!("Cannot evaluate empty animation channel");
        }

        // Find the last key before or at the current time
        for i in (0..self.keys.len()).rev() {
            if time >= self.keys[i].time {
                return self.keys[i].value.clone();
            }
        }

        // If time is before all keys, return first key
        self.keys[0].value.clone()
    }

    fn get_key_count(&self) -> usize {
        self.keys.len()
    }

    fn get_key(&self, index: usize) -> Option<&AnimationKey<T>> {
        self.keys.get(index)
    }

    fn set_key(&mut self, index: usize, value: T, time: f32) {
        if index < self.keys.len() {
            self.keys[index].value = value;
            self.keys[index].time = time;
        }
    }

    fn set_key_value(&mut self, index: usize, value: T) {
        if index < self.keys.len() {
            self.keys[index].value = value;
        }
    }

    fn add_key(&mut self, value: T, time: f32) {
        self.keys.push(AnimationKey::new(value, time));
        self.keys
            .sort_by(|a, b| a.time.partial_cmp(&b.time).unwrap());
    }

    fn insert_key(&mut self, index: usize, value: T, time: f32) {
        if index <= self.keys.len() {
            self.keys.insert(index, AnimationKey::new(value, time));
            self.keys
                .sort_by(|a, b| a.time.partial_cmp(&b.time).unwrap());
        }
    }

    fn delete_key(&mut self, index: usize) {
        if index < self.keys.len() {
            self.keys.remove(index);
        }
    }

    fn reset(&mut self) {
        self.keys.clear();
    }
}

impl<T: Clone + Interpolate> Default for StepAnimationChannel<T> {
    fn default() -> Self {
        Self::new()
    }
}

/// Animation controller for managing multiple channels
#[derive(Debug, Clone)]
pub struct AnimationController {
    /// Current animation time
    pub current_time: f32,
    /// Animation playback speed multiplier
    pub speed: f32,
    /// Whether the animation is looping
    pub looping: bool,
    /// Total duration of the animation
    pub duration: f32,
}

impl AnimationController {
    pub fn new(duration: f32) -> Self {
        Self {
            current_time: 0.0,
            speed: 1.0,
            looping: true,
            duration,
        }
    }

    /// Update the animation time
    pub fn update(&mut self, delta_time: f32) {
        self.current_time += delta_time * self.speed;

        if self.looping && self.duration > 0.0 {
            self.current_time = self.current_time % self.duration;
        } else if self.current_time > self.duration {
            self.current_time = self.duration;
        }
    }

    /// Reset the animation to the beginning
    pub fn reset(&mut self) {
        self.current_time = 0.0;
    }

    /// Set the animation time directly
    pub fn set_time(&mut self, time: f32) {
        self.current_time = time.clamp(0.0, self.duration);
    }

    /// Check if the animation has finished (for non-looping animations)
    pub fn is_finished(&self) -> bool {
        !self.looping && self.current_time >= self.duration
    }
}

impl Default for AnimationController {
    fn default() -> Self {
        Self::new(1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_f32_interpolation() {
        let mut channel = LERPAnimationChannel::<f32>::new();
        channel.add_key(0.0, 0.0);
        channel.add_key(10.0, 1.0);

        assert_eq!(channel.evaluate(0.0), 0.0);
        assert_eq!(channel.evaluate(0.5), 5.0);
        assert_eq!(channel.evaluate(1.0), 10.0);
    }

    #[test]
    fn test_vec3_interpolation() {
        use glam::Vec3;

        let mut channel = LERPAnimationChannel::<Vec3>::new();
        channel.add_key(Vec3::ZERO, 0.0);
        channel.add_key(Vec3::new(10.0, 10.0, 10.0), 1.0);

        let mid = channel.evaluate(0.5);
        assert!((mid.x - 5.0).abs() < 1e-5);
        assert!((mid.y - 5.0).abs() < 1e-5);
        assert!((mid.z - 5.0).abs() < 1e-5);
    }

    #[test]
    fn test_key_management() {
        let mut channel = LERPAnimationChannel::<f32>::new();

        assert_eq!(channel.get_key_count(), 0);

        channel.add_key(0.0, 0.0);
        channel.add_key(10.0, 1.0);

        assert_eq!(channel.get_key_count(), 2);

        channel.set_key_value(0, 5.0);
        assert_eq!(channel.get_key(0).unwrap().value, 5.0);

        channel.delete_key(0);
        assert_eq!(channel.get_key_count(), 1);

        channel.reset();
        assert_eq!(channel.get_key_count(), 0);
    }

    #[test]
    fn test_out_of_range_times() {
        let mut channel = LERPAnimationChannel::<f32>::new();
        channel.add_key(0.0, 0.0);
        channel.add_key(10.0, 1.0);

        // Before first key
        assert_eq!(channel.evaluate(-1.0), 0.0);

        // After last key
        assert_eq!(channel.evaluate(2.0), 10.0);
    }

    #[test]
    fn test_single_key() {
        let mut channel = LERPAnimationChannel::<f32>::new();
        channel.add_key(5.0, 0.5);

        assert_eq!(channel.evaluate(0.0), 5.0);
        assert_eq!(channel.evaluate(0.5), 5.0);
        assert_eq!(channel.evaluate(1.0), 5.0);
    }

    #[test]
    fn test_step_channel() {
        let mut channel = StepAnimationChannel::<f32>::new();
        channel.add_key(0.0, 0.0);
        channel.add_key(10.0, 1.0);

        // Step should hold value until next key
        assert_eq!(channel.evaluate(0.0), 0.0);
        assert_eq!(channel.evaluate(0.5), 0.0); // Still first value
        assert_eq!(channel.evaluate(0.99), 0.0); // Still first value
        assert_eq!(channel.evaluate(1.0), 10.0); // Now second value
        assert_eq!(channel.evaluate(1.5), 10.0);
    }

    #[test]
    fn test_animation_controller() {
        let mut controller = AnimationController::new(2.0);

        assert_eq!(controller.current_time, 0.0);

        controller.update(1.0);
        assert!((controller.current_time - 1.0).abs() < 1e-5);

        controller.update(0.5);
        assert!((controller.current_time - 1.5).abs() < 1e-5);

        // Test looping
        controller.update(1.0);
        assert!((controller.current_time - 0.5).abs() < 1e-5);

        // Test non-looping
        controller.looping = false;
        controller.reset();
        controller.update(3.0);
        assert!((controller.current_time - 2.0).abs() < 1e-5);
        assert!(controller.is_finished());
    }

    #[test]
    fn test_speed_multiplier() {
        let mut controller = AnimationController::new(10.0);
        controller.speed = 2.0;

        controller.update(1.0);
        assert!((controller.current_time - 2.0).abs() < 1e-5);

        controller.speed = 0.5;
        controller.update(2.0);
        assert!((controller.current_time - 3.0).abs() < 1e-5);
    }

    #[test]
    fn test_multiple_keys_sorted() {
        let mut channel = LERPAnimationChannel::<f32>::new();

        // Add keys out of order
        channel.add_key(10.0, 1.0);
        channel.add_key(0.0, 0.0);
        channel.add_key(5.0, 0.5);

        // Should be sorted by time
        assert_eq!(channel.get_key(0).unwrap().time, 0.0);
        assert_eq!(channel.get_key(1).unwrap().time, 0.5);
        assert_eq!(channel.get_key(2).unwrap().time, 1.0);

        // Interpolation should work correctly
        assert_eq!(channel.evaluate(0.25), 2.5);
    }

    #[test]
    fn test_quat_interpolation() {
        use glam::Quat;

        let mut channel = LERPAnimationChannel::<Quat>::new();

        let rot1 = Quat::from_rotation_x(0.0);
        let rot2 = Quat::from_rotation_x(std::f32::consts::PI);

        channel.add_key(rot1, 0.0);
        channel.add_key(rot2, 1.0);

        let mid = channel.evaluate(0.5);
        // Quaternion SLERP should give smooth rotation
        assert!(mid.length() > 0.99 && mid.length() < 1.01);
    }
}
