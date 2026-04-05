// FILE: complete_sky_system.rs
//
// Complete integrated sky/sun/atmosphere rendering system
// Combines skybox, sun, fog, and day/night cycle into single coordinated system
//
// C++ Reference: Multiple files integrated:
//   - W3DWater.cpp (skybox rendering)
//   - W3DScene.cpp (global lighting)
//   - GlobalData.cpp (lighting configuration)

use super::atmospheric_fog::{AtmosphericSystem, FogConfig};
use super::sky_rendering::{GlobalLightingConfig, SkyRenderingSystem, TerrainLighting, TimeOfDay};
use super::sun_system::{CelestialSystem, MoonConfig, SunConfig};
use std::sync::Arc;

/// Complete sky system integrating all rendering components
pub struct CompleteSkySystem {
    /// Skybox rendering
    sky_renderer: SkyRenderingSystem,

    /// Atmospheric fog
    atmosphere: AtmosphericSystem,

    /// Celestial bodies (sun/moon)
    celestial: CelestialSystem,

    /// Day/night cycle state
    time_of_day_progress: f32, // [0,1]
    auto_advance: bool,
    cycle_speed: f32, // Seconds per full day

    /// Last update time
    last_update_time: Option<std::time::Instant>,
}

impl CompleteSkySystem {
    /// Create new complete sky system
    pub fn new(
        device: Arc<wgpu::Device>,
        queue: Arc<wgpu::Queue>,
        surface_format: wgpu::TextureFormat,
    ) -> Self {
        Self {
            sky_renderer: SkyRenderingSystem::new(device, queue, surface_format),
            atmosphere: AtmosphericSystem::new(),
            celestial: CelestialSystem::new(),
            time_of_day_progress: 0.5, // Start at noon
            auto_advance: false,
            cycle_speed: 1200.0, // 20 minutes per day
            last_update_time: None,
        }
    }

    /// Update all systems
    pub fn update(&mut self) {
        let now = std::time::Instant::now();

        if let Some(last_time) = self.last_update_time {
            let delta = now.duration_since(last_time).as_secs_f32();

            // Update day/night cycle
            if self.auto_advance && self.cycle_speed > 0.0 {
                self.time_of_day_progress += delta / self.cycle_speed;
                self.time_of_day_progress = self.time_of_day_progress.fract();
            }

            // Update subsystems
            self.sky_renderer.update(delta);
        }

        self.last_update_time = Some(now);

        // Sync all systems to current time of day
        self.sync_to_time_of_day();
    }

    /// Synchronize all subsystems to current time of day
    fn sync_to_time_of_day(&mut self) {
        // Update skybox time
        self.sky_renderer
            .set_time_of_day_progress(self.time_of_day_progress);

        // Update atmosphere
        self.atmosphere.update_for_time(self.time_of_day_progress);

        // Update celestial bodies
        self.celestial.update_for_time(self.time_of_day_progress);
    }

    /// Load lighting configuration from map/INI data
    /// C++ Reference: GlobalData.cpp TerrainLighting parsing
    pub fn load_lighting_config(&mut self, config: GlobalLightingConfig) {
        self.sky_renderer.set_lighting_config(config);
    }

    /// Load skybox texture
    pub fn load_skybox_cubemap(
        &mut self,
        faces: [&[u8]; 6], // [+X, -X, +Y, -Y, +Z, -Z]
        width: u32,
        height: u32,
    ) -> Result<(), String> {
        self.sky_renderer.load_skybox_texture(
            faces[0], faces[1], faces[2], faces[3], faces[4], faces[5], width, height,
        )
    }

    /// Render complete sky system
    pub fn render<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        view_matrix: &[[f32; 4]; 4],
        proj_matrix: &[[f32; 4]; 4],
        camera_position: [f32; 3],
    ) {
        // Render skybox (background)
        self.sky_renderer
            .render_skybox(render_pass, view_matrix, proj_matrix, camera_position);

        // Additional rendering (sun glow, stars, etc.) would go here
        // For now, skybox provides the background
    }

    /// Get current global directional lights for scene rendering
    /// Returns light parameters suitable for W3D scene lighting
    pub fn get_global_lights(&self) -> Vec<GlobalLight> {
        let terrain_lights = self.sky_renderer.get_current_global_lights(false);
        let sun_dir = self
            .celestial
            .get_primary_light_direction(self.time_of_day_progress);
        let (sun_color, sun_intensity) = self
            .celestial
            .get_primary_light_color(self.time_of_day_progress);

        let mut lights = Vec::new();

        // Primary sun/moon light
        if self.celestial.is_sun_visible() || self.celestial.is_moon_visible() {
            lights.push(GlobalLight {
                direction: [-sun_dir[0], -sun_dir[1], -sun_dir[2]], // Negate for light direction
                color: sun_color,
                intensity: sun_intensity,
                ambient: if !terrain_lights.is_empty() {
                    terrain_lights[0].ambient
                } else {
                    [0.3, 0.3, 0.3]
                },
            });
        }

        // Additional lights from terrain lighting config
        for (i, tl) in terrain_lights
            .iter()
            .enumerate()
            .skip(if lights.is_empty() { 0 } else { 1 })
        {
            if i >= 3 {
                break;
            } // Max 3 lights

            lights.push(GlobalLight {
                direction: tl.get_direction(),
                color: tl.diffuse,
                intensity: 1.0,
                ambient: tl.ambient,
            });
        }

        lights
    }

    /// Get current fog configuration for rendering
    pub fn get_fog_config(&self) -> &FogConfig {
        self.atmosphere.get_current_fog()
    }

    /// Get fog uniforms for shaders
    pub fn get_fog_uniforms(&self) -> super::atmospheric_fog::FogUniforms {
        self.atmosphere.get_fog_uniforms()
    }

    /// Set time of day progress [0,1]
    pub fn set_time_of_day(&mut self, progress: f32) {
        self.time_of_day_progress = progress.clamp(0.0, 1.0);
        self.sync_to_time_of_day();
    }

    /// Get current time of day progress
    pub fn get_time_of_day(&self) -> f32 {
        self.time_of_day_progress
    }

    /// Enable/disable automatic day/night cycling
    pub fn set_auto_cycle(&mut self, enabled: bool, cycle_duration_seconds: f32) {
        self.auto_advance = enabled;
        self.cycle_speed = cycle_duration_seconds;
        self.sky_renderer
            .set_auto_cycle(enabled, cycle_duration_seconds);
    }

    /// Configure skybox parameters
    pub fn configure_skybox(&mut self, scale: f32, position_z: f32, draw: bool) {
        self.sky_renderer.set_skybox_config(scale, position_z, draw);
    }

    /// Get sun direction for shadow rendering
    pub fn get_sun_direction(&self) -> [f32; 3] {
        self.sky_renderer.get_sun_direction()
    }

    /// Set custom fog for specific time of day
    pub fn set_fog_for_time(&mut self, time: TimeOfDay, fog: FogConfig) {
        self.atmosphere.set_fog_for_time(time, fog);
    }

    /// Set custom sun configuration
    pub fn set_sun_config(&mut self, sun: SunConfig) {
        self.celestial.set_sun(sun);
    }

    /// Set custom moon configuration
    pub fn set_moon_config(&mut self, moon: MoonConfig) {
        self.celestial.set_moon(moon);
    }
}

/// Global directional light parameters
#[derive(Debug, Clone, Copy)]
pub struct GlobalLight {
    pub direction: [f32; 3], // Normalized direction TO light source
    pub color: [f32; 3],     // RGB color
    pub intensity: f32,      // Intensity multiplier
    pub ambient: [f32; 3],   // Ambient contribution
}

impl GlobalLight {
    /// Convert to shader uniform format
    pub fn to_uniform(&self) -> GlobalLightUniforms {
        GlobalLightUniforms {
            direction: [self.direction[0], self.direction[1], self.direction[2], 0.0],
            color: [
                self.color[0] * self.intensity,
                self.color[1] * self.intensity,
                self.color[2] * self.intensity,
                1.0,
            ],
            ambient: [self.ambient[0], self.ambient[1], self.ambient[2], 1.0],
        }
    }
}

/// Global light uniform data for shaders
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GlobalLightUniforms {
    pub direction: [f32; 4], // xyz = direction, w = padding
    pub color: [f32; 4],     // RGB + alpha
    pub ambient: [f32; 4],   // RGB + alpha
}

/// Example usage and initialization
pub fn create_default_lighting_config() -> GlobalLightingConfig {
    let mut config = GlobalLightingConfig::default();

    // Morning lighting (sunrise, warm tones)
    config.terrain_lighting[0][0] = TerrainLighting {
        ambient: [0.6, 0.55, 0.5],
        diffuse: [1.0, 0.9, 0.7],
        light_pos: [-0.7, -0.5, 0.3], // East-ish, low angle
    };

    // Afternoon lighting (overhead, bright)
    config.terrain_lighting[1][0] = TerrainLighting {
        ambient: [0.7, 0.7, 0.7],
        diffuse: [1.0, 1.0, 0.95],
        light_pos: [0.0, -1.0, 0.1], // Nearly overhead
    };

    // Evening lighting (sunset, warm orange)
    config.terrain_lighting[2][0] = TerrainLighting {
        ambient: [0.7, 0.5, 0.4],
        diffuse: [1.0, 0.7, 0.5],
        light_pos: [0.7, -0.5, -0.3], // West-ish, low angle
    };

    // Night lighting (moon, cool blue)
    config.terrain_lighting[3][0] = TerrainLighting {
        ambient: [0.15, 0.18, 0.25],
        diffuse: [0.3, 0.35, 0.5],
        light_pos: [0.3, -0.7, -0.5], // Moon position
    };

    // Copy terrain lighting to object lighting (can be modified differently)
    config.terrain_objects_lighting = config.terrain_lighting;

    // Infantry light scales (makes infantry more visible)
    config.infantry_light_scale = [1.2, 1.0, 1.3, 1.5];

    config.num_global_lights = 1;

    config
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_time_sync() {
        // This would require WGPU context, so just test logic
        let mut celestial = CelestialSystem::new();
        let mut atmosphere = AtmosphericSystem::new();

        let time = 0.5; // Noon
        celestial.update_for_time(time);
        atmosphere.update_for_time(time);

        // Both should be synchronized
        assert!(celestial.is_sun_visible());
    }

    #[test]
    fn test_global_light_creation() {
        let light = GlobalLight {
            direction: [0.0, -1.0, 0.0],
            color: [1.0, 0.9, 0.8],
            intensity: 1.5,
            ambient: [0.3, 0.3, 0.3],
        };

        let uniforms = light.to_uniform();

        // Color should be multiplied by intensity
        assert!((uniforms.color[0] - 1.5).abs() < 0.01);
        assert_eq!(uniforms.ambient[0], 0.3);
    }

    #[test]
    fn test_default_lighting_config() {
        let config = create_default_lighting_config();

        assert_eq!(config.num_global_lights, 1);

        // Verify all time periods have lighting
        for tod in 0..4 {
            let light = &config.terrain_lighting[tod][0];
            // Should have valid colors
            assert!(light.ambient[0] > 0.0);
            assert!(light.diffuse[0] > 0.0);
        }
    }
}
