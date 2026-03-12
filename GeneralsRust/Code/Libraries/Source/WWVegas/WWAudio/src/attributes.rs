//! Audio attributes and metadata management.

use serde::{Deserialize, Serialize};

/// Audio attributes container
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioAttributes {
    /// Volume level (0-100)
    pub volume: crate::Volume,
    /// Playback speed multiplier  
    pub speed: f32,
    /// Pitch adjustment (semitones)
    pub pitch: f32,
    /// 3D position (if applicable)
    pub position: Option<Position3D>,
    /// Doppler effect settings
    pub doppler: DopplerSettings,
    /// Reverb settings
    pub reverb: ReverbSettings,
    /// Custom metadata
    pub metadata: std::collections::HashMap<String, String>,
}

/// 3D position in space
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Position3D {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

/// Doppler effect configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DopplerSettings {
    pub enabled: bool,
    pub factor: f32,
    pub velocity: Option<Velocity3D>,
}

/// 3D velocity for Doppler effect
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Velocity3D {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

/// Reverb effect settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReverbSettings {
    pub enabled: bool,
    pub room_size: f32,
    pub damping: f32,
    pub wet_level: f32,
    pub dry_level: f32,
}

impl AudioAttributes {
    /// Create new attributes with defaults
    pub fn new() -> Self {
        Self::default()
    }

    /// Set volume level
    pub fn with_volume(mut self, volume: crate::Volume) -> Self {
        self.volume = volume;
        self
    }

    /// Set playback speed
    pub fn with_speed(mut self, speed: f32) -> Self {
        self.speed = speed;
        self
    }

    /// Set 3D position
    pub fn with_position(mut self, position: Position3D) -> Self {
        self.position = Some(position);
        self
    }

    /// Enable Doppler effect
    pub fn with_doppler(mut self, settings: DopplerSettings) -> Self {
        self.doppler = settings;
        self
    }

    /// Enable reverb effect
    pub fn with_reverb(mut self, settings: ReverbSettings) -> Self {
        self.reverb = settings;
        self
    }

    /// Add custom metadata
    pub fn with_metadata(mut self, key: String, value: String) -> Self {
        self.metadata.insert(key, value);
        self
    }
}

impl Default for AudioAttributes {
    fn default() -> Self {
        Self {
            volume: crate::DEFAULT_VOLUME,
            speed: 1.0,
            pitch: 0.0,
            position: None,
            doppler: DopplerSettings::default(),
            reverb: ReverbSettings::default(),
            metadata: std::collections::HashMap::new(),
        }
    }
}

impl Default for DopplerSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            factor: 1.0,
            velocity: None,
        }
    }
}

impl Default for ReverbSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            room_size: 0.5,
            damping: 0.5,
            wet_level: 0.3,
            dry_level: 0.7,
        }
    }
}
