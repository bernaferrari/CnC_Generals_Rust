// FILE: atmospheric_fog.rs
//
// Atmospheric fog and distance fog system for Generals Zero Hour Rust port
// Implements distance-based fog with color interpolation for atmosphere simulation
//
// C++ Reference: /GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/W3DScene.cpp
//                /GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/scene.cpp

use std::sync::Arc;
use wgpu::util::DeviceExt;

/// Fog mode enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FogMode {
    /// No fog
    None,
    /// Linear fog (start, end)
    Linear,
    /// Exponential fog
    Exponential,
    /// Exponential squared fog (denser)
    Exponential2,
}

/// Atmospheric fog configuration
#[derive(Debug, Clone, Copy)]
pub struct FogConfig {
    /// Fog mode
    pub mode: FogMode,

    /// Fog color (RGB)
    pub color: [f32; 3],

    /// Fog density (for exponential modes) [0,1]
    pub density: f32,

    /// Fog start distance (for linear mode)
    pub start: f32,

    /// Fog end distance (for linear mode)
    pub end: f32,

    /// Enable fog rendering
    pub enabled: bool,
}

impl Default for FogConfig {
    fn default() -> Self {
        Self {
            mode: FogMode::Linear,
            color: [0.7, 0.8, 0.9], // Light blue-grey
            density: 0.0002,
            start: 100.0,
            end: 500.0,
            enabled: true,
        }
    }
}

impl FogConfig {
    /// Calculate fog factor for a given distance
    /// Returns value in [0,1] where 0=no fog, 1=full fog
    pub fn calculate_fog_factor(&self, distance: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }

        match self.mode {
            FogMode::None => 0.0,

            FogMode::Linear => {
                // Linear fog: factor = (end - distance) / (end - start)
                if distance <= self.start {
                    0.0
                } else if distance >= self.end {
                    1.0
                } else {
                    (distance - self.start) / (self.end - self.start)
                }
            }

            FogMode::Exponential => {
                // Exponential fog: factor = 1 - exp(-density * distance)
                let factor = 1.0 - (-self.density * distance).exp();
                factor.clamp(0.0, 1.0)
            }

            FogMode::Exponential2 => {
                // Exponential^2 fog: factor = 1 - exp(-(density * distance)^2)
                let factor = 1.0 - (-(self.density * distance).powi(2)).exp();
                factor.clamp(0.0, 1.0)
            }
        }
    }

    /// Lerp between two fog configs based on time of day
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
            mode: if t < 0.5 { self.mode } else { other.mode },
            color: lerp_vec3(self.color, other.color, t),
            density: lerp_f32(self.density, other.density, t),
            start: lerp_f32(self.start, other.start, t),
            end: lerp_f32(self.end, other.end, t),
            enabled: self.enabled || other.enabled,
        }
    }
}

/// Fog uniform data for shaders
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct FogUniforms {
    pub fog_color: [f32; 4],  // RGB + padding
    pub fog_params: [f32; 4], // start, end, density, mode (as f32)
}

impl From<&FogConfig> for FogUniforms {
    fn from(config: &FogConfig) -> Self {
        let mode_value = match config.mode {
            FogMode::None => 0.0,
            FogMode::Linear => 1.0,
            FogMode::Exponential => 2.0,
            FogMode::Exponential2 => 3.0,
        };

        Self {
            fog_color: [config.color[0], config.color[1], config.color[2], 1.0],
            fog_params: [config.start, config.end, config.density, mode_value],
        }
    }
}

/// Atmospheric rendering system
pub struct AtmosphericSystem {
    // Fog configuration per time of day
    fog_configs: [FogConfig; 4], // Morning, Afternoon, Evening, Night

    // Current interpolated fog
    current_fog: FogConfig,

    // Scattering parameters (simple Rayleigh approximation)
    rayleigh_coefficient: [f32; 3], // Wavelength-dependent scattering

    // Sun scattering
    sun_intensity: f32,
    sun_scatter_factor: f32,
}

impl Default for AtmosphericSystem {
    fn default() -> Self {
        // Default fog configs for each time of day
        let morning_fog = FogConfig {
            mode: FogMode::Exponential,
            color: [0.8, 0.85, 0.9], // Light blue-grey morning haze
            density: 0.0003,
            start: 100.0,
            end: 600.0,
            enabled: true,
        };

        let afternoon_fog = FogConfig {
            mode: FogMode::Linear,
            color: [0.7, 0.8, 0.95], // Clear blue afternoon
            density: 0.0001,
            start: 200.0,
            end: 800.0,
            enabled: true,
        };

        let evening_fog = FogConfig {
            mode: FogMode::Exponential,
            color: [0.9, 0.7, 0.6], // Warm orange-red evening
            density: 0.0004,
            start: 100.0,
            end: 500.0,
            enabled: true,
        };

        let night_fog = FogConfig {
            mode: FogMode::Exponential2,
            color: [0.1, 0.15, 0.25], // Dark blue night
            density: 0.0005,
            start: 50.0,
            end: 400.0,
            enabled: true,
        };

        Self {
            fog_configs: [morning_fog, afternoon_fog, evening_fog, night_fog],
            current_fog: afternoon_fog,
            // Rayleigh scattering coefficients (approximated for RGB wavelengths)
            // Blue scatters more than red, giving blue sky
            rayleigh_coefficient: [0.0005, 0.0015, 0.005],
            sun_intensity: 1.0,
            sun_scatter_factor: 0.8,
        }
    }
}

impl AtmosphericSystem {
    pub fn new() -> Self {
        Self::default()
    }

    /// Update fog based on time of day progress [0,1]
    pub fn update_for_time(&mut self, time_progress: f32) {
        // Determine which two fog configs to interpolate
        use super::sky_rendering::TimeOfDay;

        let (tod1, tod2, t) = TimeOfDay::interpolation_factor(time_progress);
        let fog1 = &self.fog_configs[tod1 as usize];
        let fog2 = &self.fog_configs[tod2 as usize];

        self.current_fog = fog1.lerp(fog2, t);
    }

    /// Get current fog configuration
    pub fn get_current_fog(&self) -> &FogConfig {
        &self.current_fog
    }

    /// Set fog configuration for a specific time of day
    pub fn set_fog_for_time(
        &mut self,
        time_of_day: super::sky_rendering::TimeOfDay,
        fog: FogConfig,
    ) {
        self.fog_configs[time_of_day as usize] = fog;
    }

    /// Calculate atmospheric color based on view direction and sun direction
    /// Simple Rayleigh scattering approximation
    pub fn calculate_atmosphere_color(
        &self,
        view_direction: [f32; 3],
        sun_direction: [f32; 3],
    ) -> [f32; 3] {
        // Dot product between view and sun direction
        let cos_angle = view_direction[0] * sun_direction[0]
            + view_direction[1] * sun_direction[1]
            + view_direction[2] * sun_direction[2];

        // Rayleigh phase function: (3/16π)(1 + cos²θ)
        let phase = 0.75 * (1.0 + cos_angle * cos_angle);

        // Apply scattering based on wavelength
        let r = self.rayleigh_coefficient[0] * phase * self.sun_scatter_factor * self.sun_intensity;
        let g = self.rayleigh_coefficient[1] * phase * self.sun_scatter_factor * self.sun_intensity;
        let b = self.rayleigh_coefficient[2] * phase * self.sun_scatter_factor * self.sun_intensity;

        // Blend with base fog color
        let fog_color = self.current_fog.color;
        [
            (r + fog_color[0]).min(1.0),
            (g + fog_color[1]).min(1.0),
            (b + fog_color[2]).min(1.0),
        ]
    }

    /// Get fog uniforms for shader
    pub fn get_fog_uniforms(&self) -> FogUniforms {
        FogUniforms::from(&self.current_fog)
    }
}

/// Fog shader functions (to be included in other shaders)
pub const FOG_SHADER_FUNCTIONS: &str = r#"
// Fog calculation functions

fn calculate_fog_factor(distance: f32, fog_params: vec4<f32>) -> f32 {
    let start = fog_params.x;
    let end = fog_params.y;
    let density = fog_params.z;
    let mode = fog_params.w;

    if mode < 0.5 {
        // None
        return 0.0;
    } else if mode < 1.5 {
        // Linear
        if distance <= start {
            return 0.0;
        } else if distance >= end {
            return 1.0;
        } else {
            return (distance - start) / (end - start);
        }
    } else if mode < 2.5 {
        // Exponential
        return 1.0 - exp(-density * distance);
    } else {
        // Exponential^2
        let factor = density * distance;
        return 1.0 - exp(-factor * factor);
    }
}

fn apply_fog(original_color: vec3<f32>, fog_color: vec3<f32>, fog_factor: f32) -> vec3<f32> {
    // Linear interpolation between original color and fog color
    return mix(original_color, fog_color, fog_factor);
}
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_linear_fog_factor() {
        let fog = FogConfig {
            mode: FogMode::Linear,
            start: 100.0,
            end: 500.0,
            enabled: true,
            ..Default::default()
        };

        // Before start
        assert_eq!(fog.calculate_fog_factor(50.0), 0.0);

        // At start
        assert_eq!(fog.calculate_fog_factor(100.0), 0.0);

        // Midpoint
        let mid_factor = fog.calculate_fog_factor(300.0);
        assert!((mid_factor - 0.5).abs() < 0.01);

        // At end
        assert_eq!(fog.calculate_fog_factor(500.0), 1.0);

        // Beyond end
        assert_eq!(fog.calculate_fog_factor(600.0), 1.0);
    }

    #[test]
    fn test_exponential_fog() {
        let fog = FogConfig {
            mode: FogMode::Exponential,
            density: 0.001,
            enabled: true,
            ..Default::default()
        };

        // At origin
        assert_eq!(fog.calculate_fog_factor(0.0), 0.0);

        // Increases with distance
        let f1 = fog.calculate_fog_factor(100.0);
        let f2 = fog.calculate_fog_factor(200.0);
        assert!(f2 > f1);

        // Never exceeds 1.0
        let f_far = fog.calculate_fog_factor(10000.0);
        assert!(f_far <= 1.0);
    }

    #[test]
    fn test_fog_disabled() {
        let fog = FogConfig {
            mode: FogMode::Linear,
            enabled: false,
            ..Default::default()
        };

        assert_eq!(fog.calculate_fog_factor(1000.0), 0.0);
    }

    #[test]
    fn test_fog_lerp() {
        let fog1 = FogConfig {
            color: [0.0, 0.0, 0.0],
            density: 0.001,
            start: 100.0,
            end: 500.0,
            ..Default::default()
        };

        let fog2 = FogConfig {
            color: [1.0, 1.0, 1.0],
            density: 0.002,
            start: 200.0,
            end: 600.0,
            ..Default::default()
        };

        let mid = fog1.lerp(&fog2, 0.5);
        assert!((mid.color[0] - 0.5).abs() < 0.01);
        assert!((mid.density - 0.0015).abs() < 0.0001);
        assert!((mid.start - 150.0).abs() < 0.1);
    }

    #[test]
    fn test_atmospheric_system() {
        let mut atmo = AtmosphericSystem::new();

        // Morning (0.0)
        atmo.update_for_time(0.0);
        let fog = atmo.get_current_fog();
        // Should have morning fog characteristics

        // Afternoon (0.25)
        atmo.update_for_time(0.25);
        let fog = atmo.get_current_fog();
        // Should be interpolated toward afternoon

        // Evening (0.5)
        atmo.update_for_time(0.5);
        let fog = atmo.get_current_fog();
        // Should have evening characteristics

        // Night (0.75)
        atmo.update_for_time(0.75);
        let fog = atmo.get_current_fog();
        // Should have night fog
    }

    #[test]
    fn test_rayleigh_scattering() {
        let atmo = AtmosphericSystem::new();

        // View direction toward sun
        let view_toward_sun = [0.0, -1.0, 0.0];
        let sun_dir = [0.0, -1.0, 0.0];
        let color_sun = atmo.calculate_atmosphere_color(view_toward_sun, sun_dir);

        // View direction away from sun
        let view_away = [0.0, 1.0, 0.0];
        let color_away = atmo.calculate_atmosphere_color(view_away, sun_dir);

        // Both should produce valid colors
        assert!(color_sun[0] >= 0.0 && color_sun[0] <= 1.0);
        assert!(color_away[0] >= 0.0 && color_away[0] <= 1.0);

        // Blue channel should scatter most (highest coefficient)
        assert!(color_sun[2] > color_sun[0]);
    }
}
