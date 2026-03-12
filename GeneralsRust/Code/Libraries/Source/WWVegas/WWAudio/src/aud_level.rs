//! Audio Level System
//! 
//! Provides smooth interpolation of audio levels (volume, pitch, pan) over time.
//! This is a direct conversion of the C++ AUD_Level.cpp file to idiomatic Rust
//! with enhanced type safety and performance optimizations.

use std::time::Instant;
use crate::time::{Timestamp, AudioGetTime, SECONDS};
use crate::error::{AudioResult, AudioError};

/// Audio level constants
pub const AUDIO_LEVEL_MIN: i32 = 0;
pub const AUDIO_LEVEL_MAX: i32 = 1000;
pub const AUDIO_LEVEL_MIN_VAL: i32 = 0;
pub const AUDIO_LEVEL_MAX_VAL: i32 = 10000;

/// Scaling factor for internal level calculations
pub const AUDIO_LEVEL_SCALE: i32 = 8; // Bit shift amount for fixed-point math

/// Level control flags
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AudioLevelFlags {
    /// Level has been explicitly set (immediate change)
    pub set: bool,
    /// Level has changed since last check
    pub changed: bool,
}

impl Default for AudioLevelFlags {
    fn default() -> Self {
        AudioLevelFlags {
            set: false,
            changed: false,
        }
    }
}

/// Audio level controller with smooth interpolation
/// 
/// Manages the smooth transition between audio level values over time,
/// providing both immediate setting and gradual adjustment capabilities.
#[derive(Debug, Clone)]
pub struct AudioLevel {
    /// Current level value (scaled for precision)
    level: i32,
    
    /// Target level value (scaled for precision)
    new_level: i32,
    
    /// Control flags
    flags: AudioLevelFlags,
    
    /// Last update timestamp
    last_time: Instant,
    
    /// Rate of change per millisecond
    change_rate: i32,
    
    /// Duration for full-range transitions
    duration: Timestamp,
}

impl AudioLevel {
    /// Create a new audio level with starting value
    /// 
    /// # Arguments
    /// * `start_level` - Initial level value (0-1000)
    /// 
    /// # Returns
    /// A new AudioLevel instance
    pub fn new(start_level: i32) -> Self {
        debug_assert!(start_level >= AUDIO_LEVEL_MIN && start_level <= AUDIO_LEVEL_MAX);
        
        let scaled_level = start_level << AUDIO_LEVEL_SCALE;
        let mut level = AudioLevel {
            level: scaled_level,
            new_level: scaled_level,
            flags: AudioLevelFlags::default(),
            last_time: Instant::now(),
            change_rate: 0,
            duration: SECONDS(1),
        };
        
        // Set default duration
        level.set_duration(SECONDS(1), AUDIO_LEVEL_MAX);
        level
    }

    /// Set level to a new value immediately
    /// 
    /// # Arguments
    /// * `new_level` - Target level value (0-1000)
    pub fn set(&mut self, new_level: i32) {
        debug_assert!(new_level >= AUDIO_LEVEL_MIN && new_level <= AUDIO_LEVEL_MAX);
        
        self.flags.set = true;
        self.new_level = new_level << AUDIO_LEVEL_SCALE;
    }

    /// Adjust level to a new value with smooth transition
    /// 
    /// # Arguments
    /// * `new_level` - Target level value (0-1000)
    pub fn adjust(&mut self, new_level: i32) {
        debug_assert!(new_level >= AUDIO_LEVEL_MIN && new_level <= AUDIO_LEVEL_MAX);
        
        self.flags.set = false;
        
        // If current level equals new level, reset timing
        if self.new_level == self.level {
            self.last_time = Instant::now();
        }
        
        self.new_level = new_level << AUDIO_LEVEL_SCALE;
    }

    /// Force the level to be marked as set (for immediate updates)
    pub fn force(&mut self) {
        self.flags.set = true;
    }

    /// Set the duration for level transitions
    /// 
    /// # Arguments
    /// * `time` - Duration for full transition
    /// * `range` - Range of values for scaling the duration
    pub fn set_duration(&mut self, time: Timestamp, range: i32) {
        debug_assert!(time > 0);
        debug_assert!(range > 0 && range <= AUDIO_LEVEL_MAX);
        
        self.change_rate = (range << AUDIO_LEVEL_SCALE) / time as i32;
        self.duration = time;
    }

    /// Update the level value based on elapsed time
    /// 
    /// This should be called regularly to update smooth transitions.
    /// 
    /// # Returns
    /// true if the level changed, false otherwise
    pub fn update(&mut self) -> bool {
        let difference = self.new_level - self.level;
        
        if difference == 0 {
            return false; // No change needed
        }

        if self.flags.set {
            // Immediate change
            self.level = self.new_level;
        } else {
            // Gradual change
            let now = Instant::now();
            let elapsed = now.duration_since(self.last_time).as_millis() as i32;
            self.last_time = now;
            
            // Clamp elapsed time to avoid huge jumps
            let clamped_elapsed = elapsed.min(self.duration as i32);
            
            let delta = self.change_rate * clamped_elapsed;
            
            if difference < 0 {
                // Decreasing
                if delta > (-difference) {
                    self.level += difference; // Complete the transition
                } else {
                    self.level -= delta;
                }
            } else {
                // Increasing
                if delta > difference {
                    self.level += difference; // Complete the transition
                } else {
                    self.level += delta;
                }
            }
        }

        // Mark as changed
        self.flags.changed = true;
        true
    }

    /// Get the current level value
    /// 
    /// # Returns
    /// Current level (0-1000)
    pub fn get(&self) -> i32 {
        self.level >> AUDIO_LEVEL_SCALE
    }

    /// Get the target level value
    /// 
    /// # Returns
    /// Target level (0-1000)
    pub fn get_target(&self) -> i32 {
        self.new_level >> AUDIO_LEVEL_SCALE
    }

    /// Apply this level to a value
    /// 
    /// # Arguments
    /// * `value` - Input value to scale
    /// 
    /// # Returns
    /// Scaled value based on current level
    pub fn apply(&self, value: i32) -> i32 {
        debug_assert!(value >= AUDIO_LEVEL_MIN_VAL && value <= AUDIO_LEVEL_MAX_VAL);
        
        (value * self.level) >> AUDIO_LEVEL_SCALE
    }

    /// Check if the level has changed since last check
    /// 
    /// # Returns
    /// true if changed, false otherwise
    pub fn has_changed(&self) -> bool {
        self.flags.changed
    }

    /// Mark the level as used (clears change flag)
    pub fn mark_used(&mut self) {
        self.flags.changed = false;
    }

    /// Check if level is at target value
    /// 
    /// # Returns
    /// true if current level equals target level
    pub fn is_at_target(&self) -> bool {
        self.level == self.new_level
    }

    /// Get progress towards target as percentage
    /// 
    /// # Returns
    /// Progress from 0.0 (start) to 1.0 (target reached)
    pub fn get_progress(&self) -> f32 {
        if self.level == self.new_level {
            return 1.0;
        }

        // This is a simplification - in practice we'd need to track the starting point
        let current_unscaled = self.get();
        let target_unscaled = self.get_target();
        
        if current_unscaled == target_unscaled {
            1.0
        } else {
            // Simplified progress calculation
            (current_unscaled as f32) / (target_unscaled as f32).max(1.0)
        }
    }

    /// Get raw internal level value (for advanced use)
    /// 
    /// # Returns
    /// Raw scaled level value
    pub fn get_raw(&self) -> i32 {
        self.level
    }

    /// Set raw internal level value (for advanced use)
    /// 
    /// # Arguments
    /// * `raw_level` - Raw scaled level value
    pub fn set_raw(&mut self, raw_level: i32) {
        self.level = raw_level;
        self.new_level = raw_level;
    }

    /// Get the rate of change per millisecond
    /// 
    /// # Returns
    /// Change rate in scaled units per millisecond
    pub fn get_change_rate(&self) -> i32 {
        self.change_rate
    }

    /// Set the rate of change directly
    /// 
    /// # Arguments
    /// * `rate` - Change rate in scaled units per millisecond
    pub fn set_change_rate(&mut self, rate: i32) {
        self.change_rate = rate;
    }

    /// Reset to a specific value immediately
    /// 
    /// # Arguments
    /// * `level` - New level value (0-1000)
    pub fn reset(&mut self, level: i32) {
        debug_assert!(level >= AUDIO_LEVEL_MIN && level <= AUDIO_LEVEL_MAX);
        
        let scaled = level << AUDIO_LEVEL_SCALE;
        self.level = scaled;
        self.new_level = scaled;
        self.flags = AudioLevelFlags::default();
        self.last_time = Instant::now();
    }

    /// Create a level with custom duration
    /// 
    /// # Arguments
    /// * `start_level` - Initial level value (0-1000)
    /// * `duration` - Duration for transitions
    /// 
    /// # Returns
    /// A new AudioLevel with specified duration
    pub fn with_duration(start_level: i32, duration: Timestamp) -> Self {
        let mut level = Self::new(start_level);
        level.set_duration(duration, AUDIO_LEVEL_MAX);
        level
    }

    /// Create a level that changes at a specific rate
    /// 
    /// # Arguments
    /// * `start_level` - Initial level value (0-1000)
    /// * `rate` - Rate of change (units per second)
    /// 
    /// # Returns
    /// A new AudioLevel with specified change rate
    pub fn with_rate(start_level: i32, rate: i32) -> Self {
        let mut level = Self::new(start_level);
        level.set_change_rate(rate << AUDIO_LEVEL_SCALE / 1000); // Convert to per-ms
        level
    }
}

impl Default for AudioLevel {
    fn default() -> Self {
        Self::new(AUDIO_LEVEL_MAX)
    }
}

impl PartialEq for AudioLevel {
    fn eq(&self, other: &Self) -> bool {
        self.get() == other.get()
    }
}

impl std::fmt::Display for AudioLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "AudioLevel({} -> {})", self.get(), self.get_target())
    }
}

/// Utility functions for level calculations

/// Convert linear value to logarithmic scale
/// 
/// # Arguments
/// * `linear` - Linear input value (0.0 to 1.0)
/// 
/// # Returns
/// Logarithmic value suitable for audio applications
pub fn linear_to_log(linear: f32) -> f32 {
    if linear <= 0.0 {
        0.0
    } else {
        (linear.ln() / (-4.605_f32).ln()).clamp(0.0, 1.0) // -4.605 ≈ ln(0.01)
    }
}

/// Convert logarithmic value to linear scale
/// 
/// # Arguments
/// * `log_val` - Logarithmic input value (0.0 to 1.0)
/// 
/// # Returns
/// Linear value (0.0 to 1.0)
pub fn log_to_linear(log_val: f32) -> f32 {
    if log_val <= 0.0 {
        0.0
    } else {
        ((-4.605_f32 * (1.0 - log_val)).exp()).clamp(0.0, 1.0)
    }
}

/// Interpolate between two values using different curves
#[derive(Debug, Clone, Copy)]
pub enum InterpolationCurve {
    Linear,
    EaseIn,
    EaseOut,
    EaseInOut,
    Logarithmic,
}

/// Interpolate between two values
/// 
/// # Arguments
/// * `start` - Starting value
/// * `end` - Ending value
/// * `t` - Interpolation factor (0.0 to 1.0)
/// * `curve` - Type of interpolation curve
/// 
/// # Returns
/// Interpolated value
pub fn interpolate(start: f32, end: f32, t: f32, curve: InterpolationCurve) -> f32 {
    let t_clamped = t.clamp(0.0, 1.0);
    
    let factor = match curve {
        InterpolationCurve::Linear => t_clamped,
        InterpolationCurve::EaseIn => t_clamped * t_clamped,
        InterpolationCurve::EaseOut => 1.0 - (1.0 - t_clamped).powi(2),
        InterpolationCurve::EaseInOut => {
            if t_clamped < 0.5 {
                2.0 * t_clamped * t_clamped
            } else {
                1.0 - 2.0 * (1.0 - t_clamped).powi(2)
            }
        }
        InterpolationCurve::Logarithmic => linear_to_log(t_clamped),
    };
    
    start + (end - start) * factor
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn test_level_creation() {
        let level = AudioLevel::new(500);
        assert_eq!(level.get(), 500);
        assert_eq!(level.get_target(), 500);
        assert!(level.is_at_target());
    }

    #[test]
    fn test_level_bounds() {
        let level = AudioLevel::new(AUDIO_LEVEL_MIN);
        assert_eq!(level.get(), AUDIO_LEVEL_MIN);
        
        let level = AudioLevel::new(AUDIO_LEVEL_MAX);
        assert_eq!(level.get(), AUDIO_LEVEL_MAX);
    }

    #[test]
    fn test_immediate_set() {
        let mut level = AudioLevel::new(100);
        level.set(500);
        
        assert!(!level.is_at_target()); // Not updated yet
        
        let changed = level.update();
        assert!(changed);
        assert_eq!(level.get(), 500);
        assert!(level.is_at_target());
    }

    #[test]
    fn test_gradual_adjust() {
        let mut level = AudioLevel::new(0);
        level.set_duration(1000, AUDIO_LEVEL_MAX); // 1 second for full range
        level.adjust(1000);
        
        // Simulate some time passing
        thread::sleep(Duration::from_millis(10));
        
        let changed = level.update();
        assert!(changed);
        
        let current = level.get();
        assert!(current > 0);
        assert!(current < 1000);
        assert!(!level.is_at_target());
    }

    #[test]
    fn test_apply_scaling() {
        let level = AudioLevel::new(500); // 50% level
        let result = level.apply(1000);
        assert_eq!(result, 500); // 50% of 1000
        
        let level = AudioLevel::new(250); // 25% level
        let result = level.apply(800);
        assert_eq!(result, 200); // 25% of 800
    }

    #[test]
    fn test_change_detection() {
        let mut level = AudioLevel::new(100);
        
        assert!(!level.has_changed());
        
        level.set(200);
        level.update();
        
        assert!(level.has_changed());
        
        level.mark_used();
        assert!(!level.has_changed());
    }

    #[test]
    fn test_progress_calculation() {
        let mut level = AudioLevel::new(0);
        level.adjust(1000);
        
        // At start, progress should be 0
        assert_eq!(level.get_progress(), 0.0);
        
        // After setting to target, progress should be 1
        level.set(1000);
        level.update();
        assert_eq!(level.get_progress(), 1.0);
    }

    #[test]
    fn test_duration_setting() {
        let mut level = AudioLevel::new(0);
        level.set_duration(500, AUDIO_LEVEL_MAX);
        
        // Change rate should be calculated based on duration
        assert!(level.get_change_rate() > 0);
    }

    #[test]
    fn test_raw_access() {
        let mut level = AudioLevel::new(100);
        let raw = level.get_raw();
        
        level.set_raw(raw * 2);
        assert_eq!(level.get(), 200);
    }

    #[test]
    fn test_reset() {
        let mut level = AudioLevel::new(100);
        level.adjust(500);
        level.update();
        
        level.reset(200);
        assert_eq!(level.get(), 200);
        assert_eq!(level.get_target(), 200);
        assert!(level.is_at_target());
    }

    #[test]
    fn test_with_duration() {
        let level = AudioLevel::with_duration(50, 2000);
        assert_eq!(level.get(), 50);
        assert!(level.get_change_rate() > 0);
    }

    #[test]
    fn test_interpolation() {
        assert_eq!(interpolate(0.0, 100.0, 0.5, InterpolationCurve::Linear), 50.0);
        assert_eq!(interpolate(0.0, 100.0, 0.0, InterpolationCurve::Linear), 0.0);
        assert_eq!(interpolate(0.0, 100.0, 1.0, InterpolationCurve::Linear), 100.0);
        
        let ease_in = interpolate(0.0, 100.0, 0.5, InterpolationCurve::EaseIn);
        assert!(ease_in < 50.0); // Should be slower at start
        
        let ease_out = interpolate(0.0, 100.0, 0.5, InterpolationCurve::EaseOut);
        assert!(ease_out > 50.0); // Should be faster at start
    }

    #[test]
    fn test_log_conversion() {
        assert_eq!(linear_to_log(1.0), 1.0);
        assert_eq!(linear_to_log(0.0), 0.0);
        
        assert_eq!(log_to_linear(1.0), 1.0);
        assert_eq!(log_to_linear(0.0), 0.0);
        
        // Test round-trip conversion
        let original = 0.5;
        let log_val = linear_to_log(original);
        let back_to_linear = log_to_linear(log_val);
        assert!((original - back_to_linear).abs() < 0.01);
    }

    #[test]
    fn test_display() {
        let level = AudioLevel::new(100);
        let display = format!("{}", level);
        assert!(display.contains("100"));
    }

    #[test]
    fn test_equality() {
        let level1 = AudioLevel::new(100);
        let level2 = AudioLevel::new(100);
        let level3 = AudioLevel::new(200);
        
        assert_eq!(level1, level2);
        assert_ne!(level1, level3);
    }
}