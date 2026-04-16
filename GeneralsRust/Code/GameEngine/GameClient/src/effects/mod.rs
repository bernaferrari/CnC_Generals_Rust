//! # Visual Effects Module
//!
//! Comprehensive visual effects system including particle effects, weather,
//! decals, and environmental effects for Command & Conquer Generals Zero Hour.
//!
//! ## Features
//!
//! - High-performance particle systems with GPU acceleration
//! - Weather effects (rain, snow, dust storms)
//! - Ground decals for explosions and impacts
//! - Environmental effects (fire, smoke, sparks)
//! - Lighting effects and dynamic shadows
//! - Performance monitoring and optimization
//!
//! ## Architecture
//!
//! The effects system is built around several main components:
//! - [`ParticleSystem`] - Core particle rendering and simulation
//! - [`WeatherSystem`] - Environmental weather effects
//! - [`DecalManager`] - Ground and surface decals
//! - [`EffectsManager`] - Central coordinator for all visual effects
//!
//! ## Example Usage
//!
//! ```rust,no_run
//! use game_client_rust::effects::{EffectsManager, ParticleSystemDesc, WeatherType};
//!
//! let mut effects_manager = EffectsManager::new();
//! effects_manager.init().unwrap();
//!
//! // Create explosion effect
//! let explosion = ParticleSystemDesc::explosion()
//!     .at_position(100.0, 200.0, 0.0)
//!     .with_scale(2.0);
//! effects_manager.spawn_particle_system(explosion);
//!
//! // Enable weather
//! effects_manager.set_weather(WeatherType::Snow, 0.7);
//!
//! // Update effects in game loop
//! effects_manager.update(delta_time);
//! effects_manager.render();
//! ```

pub mod debug_draw;
pub mod decals;
pub mod fxlist_integration;
pub mod manager;
pub mod particle_manager;
pub mod particle_presets;
pub mod particle_renderer;
pub mod particle_system;
pub mod particle_system_manager;
pub mod particles;
pub mod ray_effects;
pub mod shadow_system;
pub mod weather;
pub mod weather_complete;

use nalgebra::{Point3, Vector3};
use std::time::{Duration, Instant};
use thiserror::Error;

use crate::display::image::GameImageError;
use crate::system::SubsystemInterface;

// Re-export main types for convenience
pub use debug_draw::{DebugDraw, DebugDrawCommand, DebugShape};
pub use decals::{Decal, DecalId, DecalManager, DecalSettings, DecalType, RadiusDecal};
pub use manager::EffectsManager;

// Generic effects particle types (non-C++ parity, used by EffectsManager for runtime effects)
pub use particles::{
    Particle as GenericParticle, ParticleEmitter, ParticleForce,
    ParticleRenderer as GenericParticleRenderer, ParticleStats,
    ParticleSystem as GenericParticleSystem, ParticleSystemDesc,
    ParticleSystemId as GenericParticleSystemId, ParticleType as GenericParticleType,
};

// C++-parity particle system types (matches C++ ParticleSys.h/.cpp behavior exactly)
// These are the authoritative types used by the game logic and rendering pipeline.
pub use particle_manager::{
    EmissionVelocity, EmissionVelocityType, EmissionVolume, EmissionVolumeType,
    GameClientRandomVariable, Keyframe, ObjectId as ParticleObjectId, ParticlePriorityType,
    ParticleShaderType, ParticleSystemId, ParticleSystemManager, ParticleSystemTemplate,
    ParticleType as CppParticleTypeEnum, RGBColorKeyframe, RandomKeyframe, WindMotion,
    INVALID_PARTICLE_SYSTEM_ID, MAX_KEYFRAMES,
};
pub use particle_presets::{destruction, environment, explosions, weapons};
pub use particle_renderer::{
    ParticleBatch, ParticleRenderStats, ParticleRenderer, ParticleUniforms, ParticleVertex,
};
pub use particle_system::{Particle, ParticleInfo, ParticleSystem};

pub use ray_effects::{RayEffect, RayEffectConfig, RayEffectId, RayEffectManager, RayType};
pub use shadow_system::{
    ShadowCaster, ShadowMapArray, ShadowMapResolution, ShadowQuality, ShadowSystem,
};
pub use weather_complete::{
    DustStormSystem, RainSystem, SnowSystem, WeatherParticle, WeatherSettings, WeatherSystem,
    WeatherType,
};

/// Visual effects errors
#[derive(Error, Debug)]
pub enum EffectsError {
    #[error("Effects system initialization failed: {0}")]
    InitializationError(String),

    #[error("Particle system error: {0}")]
    ParticleError(String),

    #[error("Weather system error: {0}")]
    WeatherError(String),

    #[error("Decal system error: {0}")]
    DecalError(String),

    #[error("Shadow system error: {0}")]
    ShadowError(String),

    #[error("Resource loading error: {0}")]
    ResourceError(#[from] GameImageError),

    #[error("Rendering error: {0}")]
    RenderingError(String),

    #[error("GPU resource error: {0}")]
    GPUError(String),
}

/// Effects quality settings
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EffectsQuality {
    /// Minimal effects for low-end hardware
    Low,
    /// Balanced effects for mid-range hardware
    Medium,
    /// Full effects for high-end hardware
    High,
    /// Ultra effects with all features enabled
    Ultra,
}

impl EffectsQuality {
    /// Get maximum number of particles for this quality level
    pub fn max_particles(self) -> usize {
        match self {
            EffectsQuality::Low => 500,
            EffectsQuality::Medium => 1500,
            EffectsQuality::High => 3000,
            EffectsQuality::Ultra => 6000,
        }
    }

    /// Check if weather effects are enabled
    pub fn weather_enabled(self) -> bool {
        matches!(
            self,
            EffectsQuality::Medium | EffectsQuality::High | EffectsQuality::Ultra
        )
    }

    /// Check if dynamic lighting is enabled
    pub fn dynamic_lighting(self) -> bool {
        matches!(self, EffectsQuality::High | EffectsQuality::Ultra)
    }

    /// Get particle update frequency (lower = better quality, more CPU)
    pub fn update_frequency_hz(self) -> f32 {
        match self {
            EffectsQuality::Low => 30.0,
            EffectsQuality::Medium => 45.0,
            EffectsQuality::High => 60.0,
            EffectsQuality::Ultra => 120.0,
        }
    }
}

impl Default for EffectsQuality {
    fn default() -> Self {
        EffectsQuality::Medium
    }
}

/// Effects configuration
#[derive(Debug, Clone)]
pub struct EffectsConfig {
    /// Quality level
    pub quality: EffectsQuality,

    /// Enable/disable particle effects
    pub particles_enabled: bool,

    /// Enable/disable weather effects
    pub weather_enabled: bool,

    /// Enable/disable decals
    pub decals_enabled: bool,

    /// Maximum number of active particle systems
    pub max_particle_systems: usize,

    /// Maximum lifetime for decals (seconds)
    pub decal_lifetime: f32,

    /// Enable performance monitoring
    pub performance_monitoring: bool,

    /// LOD (Level of Detail) distance thresholds
    pub lod_near_distance: f32,
    pub lod_medium_distance: f32,
    pub lod_far_distance: f32,
}

impl Default for EffectsConfig {
    fn default() -> Self {
        let quality = EffectsQuality::default();

        Self {
            quality,
            particles_enabled: true,
            weather_enabled: quality.weather_enabled(),
            decals_enabled: true,
            max_particle_systems: 50,
            decal_lifetime: 30.0,
            performance_monitoring: true,
            lod_near_distance: 100.0,
            lod_medium_distance: 300.0,
            lod_far_distance: 600.0,
        }
    }
}

/// Performance metrics for effects system
#[derive(Debug, Default)]
pub struct EffectsStats {
    /// Number of active particle systems
    pub active_particle_systems: usize,

    /// Total number of active particles
    pub active_particles: usize,

    /// Number of active decals
    pub active_decals: usize,

    /// GPU memory used by effects (bytes)
    pub gpu_memory_used: usize,

    /// CPU time spent updating effects (milliseconds)
    pub update_time_ms: f64,

    /// GPU time spent rendering effects (milliseconds)
    pub render_time_ms: f64,

    /// Effects rendered this frame
    pub effects_rendered: usize,

    /// Effects culled this frame (outside view)
    pub effects_culled: usize,

    /// Last update time
    pub last_update: Option<Instant>,
}

impl EffectsStats {
    /// Reset all statistics
    pub fn reset(&mut self) {
        *self = Self {
            last_update: Some(Instant::now()),
            ..*self
        };
    }

    /// Get particles per second being processed
    pub fn particles_per_second(&self) -> f64 {
        if let Some(last_update) = self.last_update {
            let elapsed = last_update.elapsed().as_secs_f64();
            if elapsed > 0.0 {
                return self.active_particles as f64 / elapsed;
            }
        }
        0.0
    }

    /// Get total performance impact (0.0 to 1.0)
    pub fn performance_impact(&self) -> f32 {
        let cpu_factor = (self.update_time_ms / 16.67) as f32; // Relative to 60 FPS frame
        let gpu_factor = (self.render_time_ms / 16.67) as f32;
        let particle_factor = (self.active_particles as f32 / 3000.0).min(1.0);

        (cpu_factor + gpu_factor + particle_factor).min(1.0)
    }
}

/// LOD (Level of Detail) calculation for effects
pub fn calculate_effects_lod(distance: f32, config: &EffectsConfig) -> EffectsLOD {
    if distance <= config.lod_near_distance {
        EffectsLOD::High
    } else if distance <= config.lod_medium_distance {
        EffectsLOD::Medium
    } else if distance <= config.lod_far_distance {
        EffectsLOD::Low
    } else {
        EffectsLOD::None
    }
}

/// Level of detail for effects rendering
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EffectsLOD {
    /// Full detail effects
    High,
    /// Reduced particle count and detail
    Medium,
    /// Minimal effects
    Low,
    /// No effects (too far)
    None,
}

impl EffectsLOD {
    /// Get particle count multiplier for this LOD level
    pub fn particle_multiplier(self) -> f32 {
        match self {
            EffectsLOD::High => 1.0,
            EffectsLOD::Medium => 0.6,
            EffectsLOD::Low => 0.3,
            EffectsLOD::None => 0.0,
        }
    }

    /// Get update frequency multiplier for this LOD level
    pub fn update_multiplier(self) -> f32 {
        match self {
            EffectsLOD::High => 1.0,
            EffectsLOD::Medium => 0.8,
            EffectsLOD::Low => 0.5,
            EffectsLOD::None => 0.0,
        }
    }
}

/// Common effects utility functions
pub mod utils {
    use super::*;
    use rand::prelude::*;

    /// Generate random position within a sphere
    pub fn random_sphere_position(center: Point3<f32>, radius: f32) -> Point3<f32> {
        let mut rng = thread_rng();
        let theta = rng.gen::<f32>() * 2.0 * std::f32::consts::PI;
        let phi = rng.gen::<f32>() * std::f32::consts::PI;
        let r = rng.gen::<f32>().powf(1.0 / 3.0) * radius; // Uniform distribution in sphere

        let x = r * phi.sin() * theta.cos();
        let y = r * phi.sin() * theta.sin();
        let z = r * phi.cos();

        Point3::new(center.x + x, center.y + y, center.z + z)
    }

    /// Generate random velocity within a cone
    pub fn random_cone_velocity(
        direction: Vector3<f32>,
        angle_radians: f32,
        min_speed: f32,
        max_speed: f32,
    ) -> Vector3<f32> {
        let mut rng = thread_rng();

        // Generate random direction within cone
        let theta = rng.gen::<f32>() * 2.0 * std::f32::consts::PI;
        let phi = rng.gen::<f32>() * angle_radians;

        // Create rotation matrix to align with desired direction
        let up = if direction.y.abs() < 0.9 {
            Vector3::new(0.0, 1.0, 0.0)
        } else {
            Vector3::new(1.0, 0.0, 0.0)
        };

        let right = direction.cross(&up).normalize();
        let actual_up = right.cross(&direction);

        // Generate random direction in cone
        let local_dir = Vector3::new(phi.sin() * theta.cos(), phi.sin() * theta.sin(), phi.cos());

        // Transform to world space
        let world_dir = direction * local_dir.z + right * local_dir.x + actual_up * local_dir.y;

        // Apply random speed
        let speed = rng.gen_range(min_speed..=max_speed);
        world_dir.normalize() * speed
    }

    /// Interpolate color over time
    pub fn interpolate_color(start_color: [f32; 4], end_color: [f32; 4], t: f32) -> [f32; 4] {
        let t = t.clamp(0.0, 1.0);
        [
            start_color[0] + (end_color[0] - start_color[0]) * t,
            start_color[1] + (end_color[1] - start_color[1]) * t,
            start_color[2] + (end_color[2] - start_color[2]) * t,
            start_color[3] + (end_color[3] - start_color[3]) * t,
        ]
    }

    /// Calculate wind effect on position
    pub fn apply_wind_force(
        position: Point3<f32>,
        wind_direction: Vector3<f32>,
        wind_strength: f32,
        delta_time: f32,
    ) -> Vector3<f32> {
        // Add some turbulence based on position
        let turbulence_x = (position.x * 0.1).sin() * 0.3;
        let turbulence_y = (position.y * 0.1 + 1.0).sin() * 0.2;
        let turbulence_z = (position.z * 0.1 + 2.0).sin() * 0.3;

        let turbulence = Vector3::new(turbulence_x, turbulence_y, turbulence_z);
        let total_wind = wind_direction * wind_strength + turbulence;

        total_wind * delta_time
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_effects_quality_settings() {
        assert_eq!(EffectsQuality::Low.max_particles(), 500);
        assert_eq!(EffectsQuality::Ultra.max_particles(), 6000);

        assert!(!EffectsQuality::Low.weather_enabled());
        assert!(EffectsQuality::High.weather_enabled());

        assert!(!EffectsQuality::Low.dynamic_lighting());
        assert!(EffectsQuality::Ultra.dynamic_lighting());
    }

    #[test]
    fn test_effects_lod() {
        let config = EffectsConfig::default();

        assert_eq!(calculate_effects_lod(50.0, &config), EffectsLOD::High);
        assert_eq!(calculate_effects_lod(200.0, &config), EffectsLOD::Medium);
        assert_eq!(calculate_effects_lod(500.0, &config), EffectsLOD::Low);
        assert_eq!(calculate_effects_lod(800.0, &config), EffectsLOD::None);

        assert_eq!(EffectsLOD::High.particle_multiplier(), 1.0);
        assert_eq!(EffectsLOD::Medium.particle_multiplier(), 0.6);
        assert_eq!(EffectsLOD::None.particle_multiplier(), 0.0);
    }

    #[test]
    fn test_effects_config_defaults() {
        let config = EffectsConfig::default();

        assert!(config.particles_enabled);
        assert_eq!(config.quality, EffectsQuality::Medium);
        assert_eq!(config.max_particle_systems, 50);
        assert_eq!(config.decal_lifetime, 30.0);
    }

    #[test]
    fn test_effects_stats() {
        let mut stats = EffectsStats::default();
        stats.active_particles = 1000;

        stats.reset();
        assert!(stats.last_update.is_some());

        // Test performance impact calculation
        stats.update_time_ms = 8.0; // Half frame time
        stats.render_time_ms = 8.0; // Half frame time
        stats.active_particles = 1500; // Half max for medium quality

        let impact = stats.performance_impact();
        assert!(impact > 0.0 && impact <= 1.0);
    }

    #[test]
    fn test_utils_random_sphere() {
        use crate::effects::utils::*;

        let center = Point3::new(0.0, 0.0, 0.0);
        let radius = 10.0;

        for _ in 0..100 {
            let pos = random_sphere_position(center, radius);
            let distance = (pos - center).norm();
            assert!(distance <= radius);
        }
    }

    #[test]
    fn test_utils_color_interpolation() {
        use crate::effects::utils::*;

        let start = [1.0, 0.0, 0.0, 1.0]; // Red
        let end = [0.0, 1.0, 0.0, 1.0]; // Green

        let mid = interpolate_color(start, end, 0.5);
        assert_eq!(mid[0], 0.5); // R
        assert_eq!(mid[1], 0.5); // G
        assert_eq!(mid[2], 0.0); // B
        assert_eq!(mid[3], 1.0); // A

        let start_color = interpolate_color(start, end, 0.0);
        assert_eq!(start_color, start);

        let end_color = interpolate_color(start, end, 1.0);
        assert_eq!(end_color, end);
    }
}
