//! Fog system integration for WW3D2 scene rendering
//!
//! This module provides fog integration between the SceneClass and the shader system.
//! It implements the C++ fog logic from shader.cpp lines 280-327 and 491-532.

use glam::Vec3;

/// Fog configuration structure
///
/// Fog parameters that get passed from SceneClass to the shader system's
/// FrameUniforms buffer.
#[derive(Debug, Clone, Copy)]
pub struct FogSettings {
    /// Whether fog is enabled for the scene
    pub enabled: bool,
    /// Fog color (RGB)
    pub color: Vec3,
    /// Distance at which fog starts (in world units)
    pub start: f32,
    /// Distance at which fog reaches maximum density (in world units)
    pub end: f32,
}

impl Default for FogSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            color: Vec3::new(0.5, 0.5, 0.5),
            start: 0.0,
            end: 1000.0,
        }
    }
}

impl FogSettings {
    /// Create new fog settings
    pub fn new(enabled: bool, color: Vec3, start: f32, end: f32) -> Self {
        Self {
            enabled,
            color,
            start,
            end,
        }
    }

    /// Check if fog is enabled and valid
    pub fn is_active(&self) -> bool {
        self.enabled && self.start < self.end
    }

    /// Get fog density at a given distance
    /// Returns a value from 0.0 (no fog) to 1.0 (full fog)
    pub fn get_fog_factor(&self, distance: f32) -> f32 {
        if !self.is_active() {
            return 0.0;
        }

        // Linear fog calculation: f = (end - dist) / (end - start)
        // Clamped to [0, 1] range
        let factor = (self.end - distance) / (self.end - self.start);
        factor.clamp(0.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fog_settings_default() {
        let fog = FogSettings::default();
        assert!(!fog.enabled);
        assert_eq!(fog.start, 0.0);
        assert_eq!(fog.end, 1000.0);
    }

    #[test]
    fn test_fog_settings_active() {
        let mut fog = FogSettings::default();
        assert!(!fog.is_active());

        fog.enabled = true;
        fog.start = 10.0;
        fog.end = 100.0;
        assert!(fog.is_active());
    }

    #[test]
    fn test_fog_factor_calculation() {
        let fog = FogSettings {
            enabled: true,
            color: Vec3::new(0.5, 0.5, 0.5),
            start: 10.0,
            end: 100.0,
        };

        // At start distance, fog factor should be 1.0 (no fog)
        assert!((fog.get_fog_factor(10.0) - 1.0).abs() < 0.001);

        // At end distance, fog factor should be 0.0 (full fog)
        assert!((fog.get_fog_factor(100.0) - 0.0).abs() < 0.001);

        // At midpoint, fog factor should be ~0.5
        assert!((fog.get_fog_factor(55.0) - 0.5).abs() < 0.001);

        // Beyond end, should clamp to 0.0
        assert!((fog.get_fog_factor(200.0) - 0.0).abs() < 0.001);

        // Before start, should clamp to 1.0
        assert!((fog.get_fog_factor(0.0) - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_fog_disabled() {
        let fog = FogSettings {
            enabled: false,
            color: Vec3::new(0.5, 0.5, 0.5),
            start: 10.0,
            end: 100.0,
        };

        // When disabled, fog factor should always be 0.0
        assert_eq!(fog.get_fog_factor(50.0), 0.0);
    }
}
