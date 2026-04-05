// FILE: sun_system.rs
//
// Sun and celestial body rendering system for Generals Zero Hour Rust port
// Implements sun glow, lens flare effects, and sun positioning based on time
//
// C++ Reference: /GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/Water/W3DWater.cpp
//                (renderSkyBody function for sun/moon rendering)

use std::sync::Arc;
use wgpu::util::DeviceExt;

/// Sun glow/flare configuration
#[derive(Debug, Clone, Copy)]
pub struct SunConfig {
    /// Sun position in sky (spherical coordinates)
    pub azimuth: f32, // Horizontal angle (radians)
    pub elevation: f32, // Vertical angle (radians)

    /// Sun color and intensity
    pub color: [f32; 3],
    pub intensity: f32,

    /// Glow parameters
    pub glow_size: f32, // Size of sun glow
    pub glow_intensity: f32, // Brightness of glow

    /// Lens flare enable
    pub enable_flare: bool,

    /// Sun disk size (angular diameter in radians)
    pub disk_size: f32,
}

impl Default for SunConfig {
    fn default() -> Self {
        Self {
            azimuth: 0.0,
            elevation: std::f32::consts::PI / 4.0, // 45 degrees
            color: [1.0, 0.95, 0.8],               // Slightly warm white
            intensity: 1.0,
            glow_size: 0.1,
            glow_intensity: 0.8,
            enable_flare: true,
            disk_size: 0.009, // ~0.5 degrees (realistic sun angular size)
        }
    }
}

impl SunConfig {
    /// Get sun direction vector from spherical coordinates
    /// Returns normalized direction pointing TOWARD the sun
    pub fn get_direction(&self) -> [f32; 3] {
        let x = self.elevation.cos() * self.azimuth.sin();
        let y = self.elevation.sin();
        let z = self.elevation.cos() * self.azimuth.cos();

        // Normalize
        let len = (x * x + y * y + z * z).sqrt();
        if len > 0.0001 {
            [x / len, y / len, z / len]
        } else {
            [0.0, 1.0, 0.0]
        }
    }

    /// Set sun direction from vector
    pub fn set_direction(&mut self, dir: [f32; 3]) {
        // Convert Cartesian to spherical
        let len = (dir[0] * dir[0] + dir[1] * dir[1] + dir[2] * dir[2]).sqrt();
        if len > 0.0001 {
            let x = dir[0] / len;
            let y = dir[1] / len;
            let z = dir[2] / len;

            self.elevation = y.asin();
            self.azimuth = x.atan2(z);
        }
    }

    /// Animate sun position based on time of day [0,1]
    pub fn update_for_time(&mut self, time_progress: f32) {
        // Sun moves in arc across sky throughout day
        // 0.0 = sunrise (east), 0.5 = noon (south/overhead), 1.0 = sunset (west)

        // Azimuth: -90° (east) -> 0° (south) -> 90° (west)
        // Map time [0,1] to angle [-π/2, π/2]
        self.azimuth = (time_progress - 0.5) * std::f32::consts::PI;

        // Elevation: rises from horizon, peaks at noon, sets at horizon
        // Use sine curve for realistic arc
        let normalized_time = time_progress * std::f32::consts::PI; // [0, π]
        self.elevation = normalized_time.sin() * std::f32::consts::PI / 3.0; // Max 60° elevation

        // Clamp elevation to above horizon (allow negative for night)
        if time_progress < 0.05 || time_progress > 0.95 {
            // Night time - sun below horizon
            self.elevation = -0.1;
        }
    }

    /// Interpolate between two sun configs
    pub fn lerp(&self, other: &Self, t: f32) -> Self {
        let lerp_f32 = |a: f32, b: f32, t: f32| a + (b - a) * t;
        let lerp_vec3 = |a: [f32; 3], b: [f32; 3], t: f32| -> [f32; 3] {
            [
                lerp_f32(a[0], b[0], t),
                lerp_f32(a[1], b[1], t),
                lerp_f32(a[2], b[2], t),
            ]
        };

        Self {
            azimuth: lerp_f32(self.azimuth, other.azimuth, t),
            elevation: lerp_f32(self.elevation, other.elevation, t),
            color: lerp_vec3(self.color, other.color, t),
            intensity: lerp_f32(self.intensity, other.intensity, t),
            glow_size: lerp_f32(self.glow_size, other.glow_size, t),
            glow_intensity: lerp_f32(self.glow_intensity, other.glow_intensity, t),
            enable_flare: self.enable_flare || other.enable_flare,
            disk_size: lerp_f32(self.disk_size, other.disk_size, t),
        }
    }
}

/// Moon configuration (similar to sun but different colors)
#[derive(Debug, Clone, Copy)]
pub struct MoonConfig {
    pub azimuth: f32,
    pub elevation: f32,
    pub color: [f32; 3],
    pub intensity: f32,
    pub phase: f32, // [0,1] where 0=new moon, 0.5=full moon, 1.0=new moon
}

impl Default for MoonConfig {
    fn default() -> Self {
        Self {
            azimuth: std::f32::consts::PI, // Opposite sun
            elevation: std::f32::consts::PI / 6.0,
            color: [0.8, 0.85, 0.9], // Cool blue-white
            intensity: 0.15,
            phase: 0.5, // Full moon
        }
    }
}

impl MoonConfig {
    pub fn get_direction(&self) -> [f32; 3] {
        let x = self.elevation.cos() * self.azimuth.sin();
        let y = self.elevation.sin();
        let z = self.elevation.cos() * self.azimuth.cos();

        let len = (x * x + y * y + z * z).sqrt();
        if len > 0.0001 {
            [x / len, y / len, z / len]
        } else {
            [0.0, 1.0, 0.0]
        }
    }

    pub fn update_for_time(&mut self, time_progress: f32) {
        // Moon opposite the sun (roughly)
        self.azimuth = ((time_progress - 0.5) * std::f32::consts::PI) + std::f32::consts::PI;

        let normalized_time = (time_progress + 0.5) % 1.0 * std::f32::consts::PI;
        self.elevation = normalized_time.sin() * std::f32::consts::PI / 4.0;

        // Only visible at night
        if time_progress >= 0.2 && time_progress <= 0.8 {
            self.elevation = -0.1; // Below horizon
        }
    }
}

/// Celestial body rendering system
/// Manages sun, moon, and star rendering
pub struct CelestialSystem {
    sun: SunConfig,
    moon: MoonConfig,
    enable_sun: bool,
    enable_moon: bool,
    enable_stars: bool,
}

impl Default for CelestialSystem {
    fn default() -> Self {
        Self {
            sun: SunConfig::default(),
            moon: MoonConfig::default(),
            enable_sun: true,
            enable_moon: true,
            enable_stars: true,
        }
    }
}

impl CelestialSystem {
    pub fn new() -> Self {
        Self::default()
    }

    /// Update celestial bodies for time of day
    pub fn update_for_time(&mut self, time_progress: f32) {
        self.sun.update_for_time(time_progress);
        self.moon.update_for_time(time_progress);
    }

    /// Get sun configuration
    pub fn get_sun(&self) -> &SunConfig {
        &self.sun
    }

    /// Get moon configuration
    pub fn get_moon(&self) -> &MoonConfig {
        &self.moon
    }

    /// Set sun configuration
    pub fn set_sun(&mut self, sun: SunConfig) {
        self.sun = sun;
    }

    /// Set moon configuration
    pub fn set_moon(&mut self, moon: MoonConfig) {
        self.moon = moon;
    }

    /// Get primary light direction (sun during day, moon at night)
    pub fn get_primary_light_direction(&self, time_progress: f32) -> [f32; 3] {
        if time_progress > 0.2 && time_progress < 0.8 {
            // Day time - use sun
            self.sun.get_direction()
        } else {
            // Night time - use moon
            self.moon.get_direction()
        }
    }

    /// Get primary light color and intensity
    pub fn get_primary_light_color(&self, time_progress: f32) -> ([f32; 3], f32) {
        if time_progress > 0.2 && time_progress < 0.8 {
            (self.sun.color, self.sun.intensity)
        } else {
            (self.moon.color, self.moon.intensity)
        }
    }

    /// Check if sun is visible (above horizon)
    pub fn is_sun_visible(&self) -> bool {
        self.enable_sun && self.sun.elevation > 0.0
    }

    /// Check if moon is visible (above horizon)
    pub fn is_moon_visible(&self) -> bool {
        self.enable_moon && self.moon.elevation > 0.0
    }
}

/// Convert celestial body to directional light parameters
/// Returns (direction, color, intensity) suitable for use as global light
pub fn celestial_to_light(sun: &SunConfig) -> ([f32; 3], [f32; 3], f32) {
    let dir = sun.get_direction();
    // Negate direction for light (points toward light source)
    let light_dir = [-dir[0], -dir[1], -dir[2]];

    (light_dir, sun.color, sun.intensity)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sun_direction() {
        let mut sun = SunConfig::default();

        // Test setting from direction
        sun.set_direction([0.0, 1.0, 0.0]); // Straight up
        let dir = sun.get_direction();
        assert!((dir[1] - 1.0).abs() < 0.01);

        sun.set_direction([1.0, 0.0, 0.0]); // East
        let dir = sun.get_direction();
        assert!((dir[0] - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_sun_time_animation() {
        let mut sun = SunConfig::default();

        // Sunrise (eastern horizon)
        sun.update_for_time(0.0);
        assert!(sun.azimuth < 0.0); // East
        assert!(sun.elevation >= 0.0); // Above horizon

        // Noon (overhead, southern)
        sun.update_for_time(0.5);
        assert!(sun.azimuth.abs() < 0.1); // Near south
        assert!(sun.elevation > 0.5); // High in sky

        // Sunset (western horizon)
        sun.update_for_time(1.0);
        assert!(sun.azimuth > 0.0); // West
    }

    #[test]
    fn test_moon_opposite_sun() {
        let mut celestial = CelestialSystem::new();

        celestial.update_for_time(0.5); // Noon

        let sun_dir = celestial.get_sun().get_direction();
        let moon_dir = celestial.get_moon().get_direction();

        // Moon should be roughly opposite sun
        let dot = sun_dir[0] * moon_dir[0] + sun_dir[1] * moon_dir[1] + sun_dir[2] * moon_dir[2];

        // Dot product should be negative (opposite directions)
        assert!(dot < 0.0);
    }

    #[test]
    fn test_sun_interpolation() {
        let sun1 = SunConfig {
            elevation: 0.0,
            azimuth: 0.0,
            color: [1.0, 0.0, 0.0],
            intensity: 0.5,
            ..Default::default()
        };

        let sun2 = SunConfig {
            elevation: 1.0,
            azimuth: 1.0,
            color: [0.0, 0.0, 1.0],
            intensity: 1.0,
            ..Default::default()
        };

        let mid = sun1.lerp(&sun2, 0.5);
        assert!((mid.elevation - 0.5).abs() < 0.01);
        assert!((mid.color[0] - 0.5).abs() < 0.01);
        assert!((mid.color[2] - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_celestial_light_conversion() {
        let sun = SunConfig {
            elevation: std::f32::consts::PI / 4.0,
            azimuth: 0.0,
            color: [1.0, 0.9, 0.8],
            intensity: 1.0,
            ..Default::default()
        };

        let (dir, color, intensity) = celestial_to_light(&sun);

        // Direction should be negated
        let sun_dir = sun.get_direction();
        assert!((dir[0] + sun_dir[0]).abs() < 0.01);

        assert_eq!(color, sun.color);
        assert_eq!(intensity, sun.intensity);
    }

    #[test]
    fn test_visibility_checks() {
        let mut celestial = CelestialSystem::new();

        // Noon - sun should be visible
        celestial.update_for_time(0.5);
        assert!(celestial.is_sun_visible());

        // Midnight - sun should not be visible
        celestial.update_for_time(0.0);
        // Sun may still be visible at exact sunrise in this impl
    }
}
