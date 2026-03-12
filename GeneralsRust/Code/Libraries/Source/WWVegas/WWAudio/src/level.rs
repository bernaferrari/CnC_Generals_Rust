//! Audio level management and volume control utilities.

use crate::{error::Result, Volume};
use std::collections::HashMap;

/// Audio level types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LevelType {
    Master,
    Music,
    SFX,
    Voice,
    UI,
    Ambient,
}

/// Audio level configuration
#[derive(Debug, Clone)]
pub struct LevelConfig {
    pub volume: Volume,
    pub muted: bool,
    pub fade_duration_ms: u64,
    pub min_volume: Volume,
    pub max_volume: Volume,
}

/// Audio level manager
pub struct AudioLevelManager {
    levels: HashMap<LevelType, LevelConfig>,
    global_mute: bool,
}

/// Level change event
#[derive(Debug, Clone)]
pub struct LevelChangeEvent {
    pub level_type: LevelType,
    pub old_volume: Volume,
    pub new_volume: Volume,
    pub muted: bool,
}

/// Fade operation
#[derive(Debug, Clone)]
pub struct FadeOperation {
    pub level_type: LevelType,
    pub start_volume: Volume,
    pub target_volume: Volume,
    pub duration_ms: u64,
    pub start_time: std::time::Instant,
}

impl AudioLevelManager {
    /// Create new level manager with default levels
    pub fn new() -> Self {
        let mut levels = HashMap::new();

        // Master defaults to full volume; others use engine default
        let mut master = LevelConfig::default();
        master.volume = crate::MAX_VOLUME;
        levels.insert(LevelType::Master, master);
        levels.insert(LevelType::Music, LevelConfig::default());
        levels.insert(LevelType::SFX, LevelConfig::default());
        levels.insert(LevelType::Voice, LevelConfig::default());
        levels.insert(LevelType::UI, LevelConfig::default());
        levels.insert(LevelType::Ambient, LevelConfig::default());

        Self {
            levels,
            global_mute: false,
        }
    }

    /// Set volume for specific level type
    pub fn set_volume(
        &mut self,
        level_type: LevelType,
        volume: Volume,
    ) -> Result<LevelChangeEvent> {
        crate::audio_assert_volume!(volume);

        let config = self.levels.get_mut(&level_type).ok_or_else(|| {
            crate::error::Error::Audio(format!("Unknown level type: {:?}", level_type))
        })?;

        let old_volume = config.volume;
        config.volume = volume.clamp(config.min_volume, config.max_volume);

        Ok(LevelChangeEvent {
            level_type,
            old_volume,
            new_volume: config.volume,
            muted: config.muted,
        })
    }

    /// Get volume for specific level type
    pub fn get_volume(&self, level_type: LevelType) -> Option<Volume> {
        self.levels.get(&level_type).map(|config| config.volume)
    }

    /// Get effective volume (considering mute states and master volume)
    pub fn get_effective_volume(&self, level_type: LevelType) -> Volume {
        if self.global_mute {
            return 0;
        }

        let master_config = self.levels.get(&LevelType::Master).unwrap();
        let level_config = self.levels.get(&level_type);

        if master_config.muted {
            return 0;
        }

        if let Some(config) = level_config {
            if config.muted {
                return 0;
            }

            // Calculate combined volume (master * level)
            let combined = (u32::from(master_config.volume) * u32::from(config.volume)) / 100;
            combined.min(100) as Volume
        } else {
            master_config.volume
        }
    }

    /// Mute/unmute specific level type
    pub fn set_mute(&mut self, level_type: LevelType, muted: bool) -> Result<()> {
        let config = self.levels.get_mut(&level_type).ok_or_else(|| {
            crate::error::Error::Audio(format!("Unknown level type: {:?}", level_type))
        })?;

        config.muted = muted;
        Ok(())
    }

    /// Check if level type is muted
    pub fn is_muted(&self, level_type: LevelType) -> bool {
        self.levels
            .get(&level_type)
            .map(|config| config.muted)
            .unwrap_or(false)
    }

    /// Set global mute state
    pub fn set_global_mute(&mut self, muted: bool) {
        self.global_mute = muted;
    }

    /// Check global mute state
    pub fn is_global_mute(&self) -> bool {
        self.global_mute
    }

    /// Create fade operation
    pub fn create_fade(
        &self,
        level_type: LevelType,
        target_volume: Volume,
        duration_ms: u64,
    ) -> Option<FadeOperation> {
        crate::audio_assert_volume!(target_volume);

        let current_volume = self.get_volume(level_type)?;

        Some(FadeOperation {
            level_type,
            start_volume: current_volume,
            target_volume,
            duration_ms,
            start_time: std::time::Instant::now(),
        })
    }

    /// Update fade operation and return current volume
    pub fn update_fade(&self, fade: &FadeOperation) -> (Volume, bool) {
        let elapsed = fade.start_time.elapsed().as_millis() as u64;

        if elapsed >= fade.duration_ms {
            // Fade complete
            (fade.target_volume, true)
        } else {
            // Calculate interpolated volume
            let progress = elapsed as f32 / fade.duration_ms as f32;
            let start = fade.start_volume as f32;
            let target = fade.target_volume as f32;
            let current = start + (target - start) * progress;

            (current.round() as Volume, false)
        }
    }

    /// Get all level types and their configs
    pub fn get_all_levels(&self) -> HashMap<LevelType, LevelConfig> {
        self.levels.clone()
    }

    /// Reset all levels to defaults
    pub fn reset_to_defaults(&mut self) {
        for config in self.levels.values_mut() {
            *config = LevelConfig::default();
        }
        if let Some(master) = self.levels.get_mut(&LevelType::Master) {
            master.volume = crate::MAX_VOLUME;
        }
        self.global_mute = false;
    }

    /// Load levels from configuration
    pub fn load_from_config(&mut self, config: HashMap<LevelType, LevelConfig>) {
        for (level_type, level_config) in config {
            self.levels.insert(level_type, level_config);
        }
    }
}

impl Default for LevelConfig {
    fn default() -> Self {
        Self {
            volume: crate::DEFAULT_VOLUME,
            muted: false,
            fade_duration_ms: 1000, // 1 second default fade
            min_volume: crate::MIN_VOLUME,
            max_volume: crate::MAX_VOLUME,
        }
    }
}

impl Default for AudioLevelManager {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for LevelType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LevelType::Master => write!(f, "Master"),
            LevelType::Music => write!(f, "Music"),
            LevelType::SFX => write!(f, "SFX"),
            LevelType::Voice => write!(f, "Voice"),
            LevelType::UI => write!(f, "UI"),
            LevelType::Ambient => write!(f, "Ambient"),
        }
    }
}

/// Utility functions for volume calculations
pub struct VolumeUtils;

impl VolumeUtils {
    /// Convert volume (0-100) to linear gain (0.0-1.0)
    pub fn volume_to_linear(volume: Volume) -> f32 {
        (volume as f32) / 100.0
    }

    /// Convert volume (0-100) to decibel gain
    pub fn volume_to_db(volume: Volume) -> f32 {
        if volume == 0 {
            -60.0 // Silence
        } else {
            20.0 * (volume as f32 / 100.0).log10()
        }
    }

    /// Convert linear gain (0.0-1.0) to volume (0-100)
    pub fn linear_to_volume(gain: f32) -> Volume {
        (gain * 100.0).clamp(0.0, 100.0) as Volume
    }

    /// Convert decibel gain to volume (0-100)
    pub fn db_to_volume(db: f32) -> Volume {
        if db <= -60.0 {
            0
        } else {
            let linear = 10.0_f32.powf(db / 20.0);
            Self::linear_to_volume(linear)
        }
    }

    /// Mix two volumes together
    pub fn mix_volumes(vol1: Volume, vol2: Volume) -> Volume {
        ((u32::from(vol1) * u32::from(vol2)) / 100).min(100) as Volume
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_level_manager() {
        let mut manager = AudioLevelManager::new();

        // Test setting volume
        let event = manager.set_volume(LevelType::SFX, 75).unwrap();
        assert_eq!(event.new_volume, 75);
        assert_eq!(manager.get_volume(LevelType::SFX), Some(75));
    }

    #[test]
    fn test_effective_volume() {
        let mut manager = AudioLevelManager::new();
        manager.set_volume(LevelType::Master, 80).unwrap();
        manager.set_volume(LevelType::SFX, 50).unwrap();

        // Effective volume should be master * sfx / 100 = 80 * 50 / 100 = 40
        assert_eq!(manager.get_effective_volume(LevelType::SFX), 40);
    }

    #[test]
    fn test_mute() {
        let mut manager = AudioLevelManager::new();
        manager.set_volume(LevelType::SFX, 80).unwrap();

        assert_eq!(manager.get_effective_volume(LevelType::SFX), 80);

        manager.set_mute(LevelType::SFX, true).unwrap();
        assert_eq!(manager.get_effective_volume(LevelType::SFX), 0);
    }

    #[test]
    fn test_volume_utils() {
        assert_eq!(VolumeUtils::volume_to_linear(100), 1.0);
        assert_eq!(VolumeUtils::volume_to_linear(0), 0.0);
        assert_eq!(VolumeUtils::volume_to_linear(50), 0.5);

        assert_eq!(VolumeUtils::linear_to_volume(1.0), 100);
        assert_eq!(VolumeUtils::linear_to_volume(0.0), 0);
        assert_eq!(VolumeUtils::linear_to_volume(0.5), 50);
    }
}
