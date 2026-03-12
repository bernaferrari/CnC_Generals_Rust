//! Audio Attributes System
//! 
//! Manages audio attributes like volume, pitch, and panning with
//! smooth transitions and level control. This is a direct conversion
//! of the C++ AUD_Attributes.cpp file to idiomatic Rust.

use crate::level::AudioLevel;
use crate::time::{Timestamp, SECONDS};
use crate::error::{AudioResult, AudioError};

/// Audio volume constants
pub const AUDIO_VOLUME_MAX: i32 = 100;
pub const AUDIO_VOLUME_MIN: i32 = 0;

/// Audio pan constants  
pub const AUDIO_PAN_LEFT: i32 = -100;
pub const AUDIO_PAN_CENTER: i32 = 0;
pub const AUDIO_PAN_RIGHT: i32 = 100;

/// Audio level constants
pub const AUDIO_LEVEL_MAX: i32 = 1000;
pub const AUDIO_LEVEL_MIN_VAL: i32 = 0;
pub const AUDIO_LEVEL_MAX_VAL: i32 = 10000;

/// Audio attributes container
/// 
/// This structure holds volume, pitch, and pan controls with
/// smooth interpolation support for seamless audio transitions.
#[derive(Debug, Clone)]
pub struct AudioAttribs {
    /// Volume level controller
    pub volume_level: AudioLevel,
    
    /// Pitch level controller (as percentage, 100 = normal)
    pub pitch_level: AudioLevel,
    
    /// Pan position controller (-100 = left, 0 = center, 100 = right)
    pub pan_position: AudioLevel,
}

impl AudioAttribs {
    /// Initialize audio attributes with default values
    /// 
    /// # Returns
    /// A new AudioAttribs instance with default settings
    pub fn new() -> Self {
        let mut attribs = AudioAttribs {
            volume_level: AudioLevel::new(AUDIO_VOLUME_MAX),
            pitch_level: AudioLevel::new(100), // 100% = normal pitch
            pan_position: AudioLevel::new(AUDIO_PAN_CENTER),
        };

        // Set default durations for smooth transitions
        attribs.set_pitch_duration(SECONDS(1), 10);
        attribs.set_volume_duration(SECONDS(1), AUDIO_LEVEL_MAX);
        attribs.set_pan_duration(SECONDS(1), AUDIO_LEVEL_MAX);

        // Set initial volume to maximum
        attribs.volume_level.set(AUDIO_VOLUME_MAX);

        attribs
    }

    /// Initialize with custom starting values
    /// 
    /// # Arguments
    /// * `volume` - Initial volume level (0-100)
    /// * `pitch` - Initial pitch level (percentage, 100 = normal)  
    /// * `pan` - Initial pan position (-100 to 100)
    pub fn with_values(volume: i32, pitch: i32, pan: i32) -> AudioResult<Self> {
        if volume < AUDIO_VOLUME_MIN || volume > AUDIO_VOLUME_MAX {
            return Err(AudioError::InvalidParameter("Volume out of range".to_string()));
        }
        
        if pan < AUDIO_PAN_LEFT || pan > AUDIO_PAN_RIGHT {
            return Err(AudioError::InvalidParameter("Pan out of range".to_string()));
        }

        let mut attribs = Self::new();
        attribs.volume_level.set(volume);
        attribs.pitch_level.set(pitch);
        attribs.pan_position.set(pan);
        
        Ok(attribs)
    }

    /// Update all audio levels
    /// 
    /// This should be called regularly to update smooth transitions.
    /// Returns true if any attribute changed.
    pub fn update(&mut self) -> bool {
        let volume_changed = self.volume_level.update();
        let pitch_changed = self.pitch_level.update();
        let pan_changed = self.pan_position.update();
        
        volume_changed || pitch_changed || pan_changed
    }

    /// Check if any attribute has changed since last check
    /// 
    /// # Returns
    /// true if volume, pitch, or pan has changed
    pub fn has_changed(&self) -> bool {
        self.volume_level.has_changed() || 
        self.pitch_level.has_changed() || 
        self.pan_position.has_changed()
    }

    /// Apply modifying attributes to these attributes
    /// 
    /// This combines the current attributes with modifier attributes,
    /// applying volume multiplication, pitch scaling, and pan adjustment.
    /// 
    /// # Arguments
    /// * `modifier` - Attributes to apply as modifiers
    pub fn apply(&mut self, modifier: &AudioAttribs) {
        // Apply volume (multiplicative)
        let new_volume = self.volume_level.apply(modifier.get_volume());
        self.volume_level.set(new_volume);
        self.volume_level.update();

        // Apply pitch (multiplicative scaling)
        let current_pitch = self.get_pitch();
        let modifier_pitch = modifier.get_pitch();
        let new_pitch = (current_pitch * modifier_pitch) / 100;
        self.set_pitch(new_pitch);
        self.pitch_level.update();

        // Apply pan (additive with clamping)
        let modifier_pan_offset = modifier.get_pan() - AUDIO_PAN_CENTER;
        if modifier_pan_offset != 0 {
            let current_pan = self.get_pan();
            let new_pan = (current_pan + modifier_pan_offset)
                .max(AUDIO_PAN_LEFT)
                .min(AUDIO_PAN_RIGHT);
            self.set_pan(new_pan);
        }
        self.pan_position.update();
    }

    /// Mark all levels as used (clears changed flags)
    pub fn mark_used(&mut self) {
        self.volume_level.mark_used();
        self.pitch_level.mark_used();
        self.pan_position.mark_used();
    }

    /// Calculate pitch scaling for a given base pitch
    /// 
    /// # Arguments
    /// * `base_pitch` - Base pitch value to scale
    /// 
    /// # Returns
    /// Scaled pitch value
    pub fn calc_pitch(&self, base_pitch: i32) -> i32 {
        let level = self.get_pitch();
        (base_pitch * level) / 100
    }

    // Volume control methods

    /// Get current volume level
    pub fn get_volume(&self) -> i32 {
        self.volume_level.get()
    }

    /// Set volume level immediately
    /// 
    /// # Arguments
    /// * `volume` - New volume level (0-100)
    pub fn set_volume(&mut self, volume: i32) -> AudioResult<()> {
        if volume < AUDIO_VOLUME_MIN || volume > AUDIO_VOLUME_MAX {
            return Err(AudioError::InvalidParameter("Volume out of range".to_string()));
        }
        self.volume_level.set(volume);
        Ok(())
    }

    /// Adjust volume level with smooth transition
    /// 
    /// # Arguments  
    /// * `volume` - Target volume level (0-100)
    pub fn adjust_volume(&mut self, volume: i32) -> AudioResult<()> {
        if volume < AUDIO_VOLUME_MIN || volume > AUDIO_VOLUME_MAX {
            return Err(AudioError::InvalidParameter("Volume out of range".to_string()));
        }
        self.volume_level.adjust(volume);
        Ok(())
    }

    /// Set volume transition duration
    /// 
    /// # Arguments
    /// * `duration` - Time for volume transitions
    /// * `range` - Range of values for scaling
    pub fn set_volume_duration(&mut self, duration: Timestamp, range: i32) {
        self.volume_level.set_duration(duration, range);
    }

    // Pitch control methods

    /// Get current pitch level (as percentage)
    pub fn get_pitch(&self) -> i32 {
        self.pitch_level.get()
    }

    /// Set pitch level immediately
    /// 
    /// # Arguments
    /// * `pitch` - New pitch level (percentage, 100 = normal)
    pub fn set_pitch(&mut self, pitch: i32) {
        self.pitch_level.set(pitch);
    }

    /// Adjust pitch level with smooth transition
    /// 
    /// # Arguments
    /// * `pitch` - Target pitch level (percentage, 100 = normal)
    pub fn adjust_pitch(&mut self, pitch: i32) {
        self.pitch_level.adjust(pitch);
    }

    /// Set pitch transition duration
    /// 
    /// # Arguments
    /// * `duration` - Time for pitch transitions
    /// * `range` - Range of values for scaling
    pub fn set_pitch_duration(&mut self, duration: Timestamp, range: i32) {
        self.pitch_level.set_duration(duration, range);
    }

    // Pan control methods

    /// Get current pan position
    pub fn get_pan(&self) -> i32 {
        self.pan_position.get()
    }

    /// Set pan position immediately
    /// 
    /// # Arguments
    /// * `pan` - New pan position (-100 to 100)
    pub fn set_pan(&mut self, pan: i32) -> AudioResult<()> {
        if pan < AUDIO_PAN_LEFT || pan > AUDIO_PAN_RIGHT {
            return Err(AudioError::InvalidParameter("Pan out of range".to_string()));
        }
        self.pan_position.set(pan);
        Ok(())
    }

    /// Adjust pan position with smooth transition
    /// 
    /// # Arguments
    /// * `pan` - Target pan position (-100 to 100)  
    pub fn adjust_pan(&mut self, pan: i32) -> AudioResult<()> {
        if pan < AUDIO_PAN_LEFT || pan > AUDIO_PAN_RIGHT {
            return Err(AudioError::InvalidParameter("Pan out of range".to_string()));
        }
        self.pan_position.adjust(pan);
        Ok(())
    }

    /// Set pan transition duration
    /// 
    /// # Arguments
    /// * `duration` - Time for pan transitions
    /// * `range` - Range of values for scaling
    pub fn set_pan_duration(&mut self, duration: Timestamp, range: i32) {
        self.pan_position.set_duration(duration, range);
    }

    /// Set all adjustment durations at once
    /// 
    /// # Arguments
    /// * `duration` - Time for all transitions
    pub fn set_adjust_duration(&mut self, duration: Timestamp) {
        self.volume_level.set_duration(duration, AUDIO_LEVEL_MAX);
        self.pitch_level.set_duration(duration, AUDIO_LEVEL_MAX);
        self.pan_position.set_duration(duration, AUDIO_LEVEL_MAX);
    }

    /// Reset all attributes to default values
    pub fn reset(&mut self) {
        self.volume_level.set(AUDIO_VOLUME_MAX);
        self.pitch_level.set(100);
        self.pan_position.set(AUDIO_PAN_CENTER);
    }

    /// Get linear volume (0.0 to 1.0) for mixing calculations
    pub fn get_linear_volume(&self) -> f32 {
        self.get_volume() as f32 / AUDIO_VOLUME_MAX as f32
    }

    /// Get pitch multiplier for frequency scaling
    pub fn get_pitch_multiplier(&self) -> f32 {
        self.get_pitch() as f32 / 100.0
    }

    /// Get pan coefficients for stereo positioning
    /// 
    /// # Returns
    /// Tuple of (left_gain, right_gain) coefficients
    pub fn get_pan_coefficients(&self) -> (f32, f32) {
        let pan = self.get_pan() as f32 / 100.0; // Normalize to -1.0 to 1.0
        
        // Equal power panning
        let angle = (pan + 1.0) * std::f32::consts::PI / 4.0; // 0 to PI/2
        let left_gain = angle.cos();
        let right_gain = angle.sin();
        
        (left_gain, right_gain)
    }
}

impl Default for AudioAttribs {
    fn default() -> Self {
        Self::new()
    }
}

/// Utility functions for audio attribute calculations

/// Convert volume percentage to decibel value
/// 
/// # Arguments
/// * `volume_percent` - Volume as percentage (0-100)
/// 
/// # Returns
/// Volume in decibels
pub fn volume_to_db(volume_percent: i32) -> f32 {
    if volume_percent <= 0 {
        -60.0 // Effectively silence
    } else {
        20.0 * (volume_percent as f32 / 100.0).log10()
    }
}

/// Convert decibel value to volume percentage
/// 
/// # Arguments
/// * `db` - Volume in decibels
/// 
/// # Returns  
/// Volume as percentage (0-100)
pub fn db_to_volume(db: f32) -> i32 {
    if db <= -60.0 {
        0
    } else {
        ((10.0_f32.powf(db / 20.0)) * 100.0) as i32
    }
}

/// Apply equal-power crossfade between two volume levels
/// 
/// # Arguments
/// * `volume1` - First volume level
/// * `volume2` - Second volume level  
/// * `crossfade` - Crossfade amount (0.0 = volume1, 1.0 = volume2)
/// 
/// # Returns
/// Crossfaded volume levels as (vol1_result, vol2_result)
pub fn equal_power_crossfade(volume1: f32, volume2: f32, crossfade: f32) -> (f32, f32) {
    let angle = crossfade * std::f32::consts::PI / 2.0;
    let vol1_gain = angle.cos();
    let vol2_gain = angle.sin();
    
    (volume1 * vol1_gain, volume2 * vol2_gain)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_attributes_creation() {
        let attribs = AudioAttribs::new();
        assert_eq!(attribs.get_volume(), AUDIO_VOLUME_MAX);
        assert_eq!(attribs.get_pitch(), 100);
        assert_eq!(attribs.get_pan(), AUDIO_PAN_CENTER);
    }

    #[test]
    fn test_volume_control() {
        let mut attribs = AudioAttribs::new();
        
        attribs.set_volume(50).unwrap();
        assert_eq!(attribs.get_volume(), 50);
        
        // Test bounds checking
        assert!(attribs.set_volume(-1).is_err());
        assert!(attribs.set_volume(101).is_err());
    }

    #[test]
    fn test_pitch_control() {
        let mut attribs = AudioAttribs::new();
        
        attribs.set_pitch(150);
        assert_eq!(attribs.get_pitch(), 150);
        
        let base_pitch = 1000;
        let scaled = attribs.calc_pitch(base_pitch);
        assert_eq!(scaled, 1500); // 1000 * 150 / 100
    }

    #[test]
    fn test_pan_control() {
        let mut attribs = AudioAttribs::new();
        
        attribs.set_pan(-50).unwrap();
        assert_eq!(attribs.get_pan(), -50);
        
        // Test bounds checking
        assert!(attribs.set_pan(-101).is_err());
        assert!(attribs.set_pan(101).is_err());
    }

    #[test]
    fn test_apply_attributes() {
        let mut base = AudioAttribs::new();
        base.set_volume(80).unwrap();
        base.set_pitch(100);
        base.set_pan(0).unwrap();
        
        let modifier = AudioAttribs::with_values(50, 150, 25).unwrap();
        
        base.apply(&modifier);
        
        // Volume should be modified multiplicatively
        // Pitch should be scaled: 100 * 150 / 100 = 150
        // Pan should be adjusted: 0 + 25 = 25
        assert_eq!(base.get_pitch(), 150);
        assert_eq!(base.get_pan(), 25);
    }

    #[test]
    fn test_pan_coefficients() {
        let mut attribs = AudioAttribs::new();
        
        // Center pan should give equal gains
        attribs.set_pan(AUDIO_PAN_CENTER).unwrap();
        let (left, right) = attribs.get_pan_coefficients();
        assert!((left - right).abs() < 0.01); // Should be approximately equal
        
        // Full left should give more left gain
        attribs.set_pan(AUDIO_PAN_LEFT).unwrap();
        let (left, right) = attribs.get_pan_coefficients();
        assert!(left > right);
        
        // Full right should give more right gain
        attribs.set_pan(AUDIO_PAN_RIGHT).unwrap();
        let (left, right) = attribs.get_pan_coefficients();
        assert!(right > left);
    }

    #[test]
    fn test_linear_volume() {
        let mut attribs = AudioAttribs::new();
        
        attribs.set_volume(50).unwrap();
        assert_eq!(attribs.get_linear_volume(), 0.5);
        
        attribs.set_volume(0).unwrap();
        assert_eq!(attribs.get_linear_volume(), 0.0);
        
        attribs.set_volume(100).unwrap();
        assert_eq!(attribs.get_linear_volume(), 1.0);
    }

    #[test]
    fn test_pitch_multiplier() {
        let mut attribs = AudioAttribs::new();
        
        attribs.set_pitch(100);
        assert_eq!(attribs.get_pitch_multiplier(), 1.0);
        
        attribs.set_pitch(150);
        assert_eq!(attribs.get_pitch_multiplier(), 1.5);
        
        attribs.set_pitch(50);
        assert_eq!(attribs.get_pitch_multiplier(), 0.5);
    }

    #[test]
    fn test_volume_db_conversion() {
        assert_eq!(volume_to_db(100), 0.0); // 100% = 0dB
        assert!(volume_to_db(0) <= -60.0); // 0% = silence
        
        assert_eq!(db_to_volume(0.0), 100);
        assert_eq!(db_to_volume(-60.0), 0);
    }

    #[test]
    fn test_crossfade() {
        let vol1 = 1.0;
        let vol2 = 1.0;
        
        // At crossfade 0.0, should favor first volume
        let (result1, result2) = equal_power_crossfade(vol1, vol2, 0.0);
        assert!(result1 > result2);
        
        // At crossfade 1.0, should favor second volume
        let (result1, result2) = equal_power_crossfade(vol1, vol2, 1.0);
        assert!(result2 > result1);
        
        // At crossfade 0.5, should be balanced
        let (result1, result2) = equal_power_crossfade(vol1, vol2, 0.5);
        assert!((result1 - result2).abs() < 0.01);
    }

    #[test]
    fn test_update_and_changes() {
        let mut attribs = AudioAttribs::new();
        
        // Initially no changes
        assert!(!attribs.has_changed());
        
        // After adjustment, should have changes
        attribs.adjust_volume(50).unwrap();
        attribs.update();
        
        // Mark as used should clear change flags
        attribs.mark_used();
        assert!(!attribs.has_changed());
    }
}