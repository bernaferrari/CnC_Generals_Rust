//! Ray Effect System
//!
//! Provides laser beams, particle beams, and other ray-based visual effects.
//! Used for weapon effects like particle cannons, lasers, and energy beams.

use nalgebra::{Point3, Vector3};
use std::collections::HashMap;
use std::time::{Duration, Instant};

use super::{EffectsError, EffectsLOD};

/// Unique identifier for ray effects
pub type RayEffectId = u64;

/// Types of ray effects
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RayType {
    /// Solid laser beam
    Laser,
    /// Particle cannon beam with particles
    ParticleBeam,
    /// Lightning effect with branching
    Lightning,
    /// Microwave beam effect
    Microwave,
    /// Generic energy beam
    EnergyBeam,
}

/// Ray beam configuration
#[derive(Debug, Clone)]
pub struct RayEffectConfig {
    /// Type of ray
    pub ray_type: RayType,

    /// Start position
    pub start: Point3<f32>,

    /// End position
    pub end: Point3<f32>,

    /// Primary color (RGBA)
    pub color: [f32; 4],

    /// Secondary color for gradients (RGBA)
    pub color_secondary: Option<[f32; 4]>,

    /// Beam width/thickness
    pub width: f32,

    /// Intensity/brightness
    pub intensity: f32,

    /// Pulse frequency (Hz, 0 = no pulse)
    pub pulse_frequency: f32,

    /// Scroll speed along beam axis
    pub scroll_speed: f32,

    /// Noise/distortion amount
    pub noise_amount: f32,

    /// Number of segments for rendering
    pub segments: u32,

    /// Lifetime (None = infinite)
    pub lifetime: Option<Duration>,

    /// Glow factor
    pub glow: f32,

    /// Fade in duration
    pub fade_in: Duration,

    /// Fade out duration
    pub fade_out: Duration,
}

impl Default for RayEffectConfig {
    fn default() -> Self {
        Self {
            ray_type: RayType::Laser,
            start: Point3::origin(),
            end: Point3::new(0.0, 0.0, 10.0),
            color: [1.0, 0.0, 0.0, 1.0], // Red
            color_secondary: None,
            width: 0.2,
            intensity: 1.0,
            pulse_frequency: 0.0,
            scroll_speed: 0.0,
            noise_amount: 0.0,
            segments: 16,
            lifetime: Some(Duration::from_millis(500)),
            glow: 0.5,
            fade_in: Duration::from_millis(50),
            fade_out: Duration::from_millis(100),
        }
    }
}

impl RayEffectConfig {
    /// Create a particle cannon beam configuration
    pub fn particle_cannon() -> Self {
        Self {
            ray_type: RayType::ParticleBeam,
            color: [0.3, 0.8, 1.0, 1.0], // Cyan
            color_secondary: Some([1.0, 1.0, 1.0, 0.8]),
            width: 0.5,
            intensity: 1.5,
            pulse_frequency: 20.0,
            scroll_speed: 50.0,
            noise_amount: 0.1,
            segments: 24,
            glow: 1.0,
            ..Default::default()
        }
    }

    /// Create a laser beam configuration
    pub fn laser() -> Self {
        Self {
            ray_type: RayType::Laser,
            color: [1.0, 0.0, 0.0, 1.0], // Red
            width: 0.15,
            intensity: 1.0,
            pulse_frequency: 0.0,
            scroll_speed: 0.0,
            noise_amount: 0.02,
            segments: 8,
            glow: 0.8,
            ..Default::default()
        }
    }

    /// Create a lightning effect configuration
    pub fn lightning() -> Self {
        Self {
            ray_type: RayType::Lightning,
            color: [0.8, 0.9, 1.0, 1.0], // Blue-white
            color_secondary: Some([0.5, 0.6, 1.0, 0.6]),
            width: 0.3,
            intensity: 2.0,
            pulse_frequency: 60.0,
            scroll_speed: 0.0,
            noise_amount: 0.8,
            segments: 32,
            lifetime: Some(Duration::from_millis(150)),
            glow: 1.5,
            ..Default::default()
        }
    }

    /// Create a microwave beam configuration
    pub fn microwave() -> Self {
        Self {
            ray_type: RayType::Microwave,
            color: [1.0, 0.8, 0.2, 0.7], // Orange-yellow
            width: 0.4,
            intensity: 1.2,
            pulse_frequency: 15.0,
            scroll_speed: 30.0,
            noise_amount: 0.3,
            segments: 20,
            glow: 0.9,
            ..Default::default()
        }
    }

    /// Set start and end positions
    pub fn between(mut self, start: Point3<f32>, end: Point3<f32>) -> Self {
        self.start = start;
        self.end = end;
        self
    }

    /// Set color
    pub fn with_color(mut self, color: [f32; 4]) -> Self {
        self.color = color;
        self
    }

    /// Set width
    pub fn with_width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    /// Set lifetime
    pub fn with_lifetime(mut self, lifetime: Duration) -> Self {
        self.lifetime = Some(lifetime);
        self
    }
}

/// Ray effect instance
pub struct RayEffect {
    id: RayEffectId,
    config: RayEffectConfig,
    created_at: Instant,
    active: bool,
    current_alpha: f32,
}

impl RayEffect {
    /// Create a new ray effect
    pub fn new(id: RayEffectId, config: RayEffectConfig) -> Self {
        Self {
            id,
            config,
            created_at: Instant::now(),
            active: true,
            current_alpha: 0.0,
        }
    }

    /// Get effect ID
    pub fn id(&self) -> RayEffectId {
        self.id
    }

    /// Check if effect is active
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Get start position
    pub fn start(&self) -> Point3<f32> {
        self.config.start
    }

    /// Get end position
    pub fn end(&self) -> Point3<f32> {
        self.config.end
    }

    /// Update effect state
    pub fn update(&mut self, delta_time: f32) {
        let elapsed = self.created_at.elapsed();

        // Check lifetime
        if let Some(lifetime) = self.config.lifetime {
            if elapsed > lifetime {
                self.active = false;
                return;
            }
        }

        // Calculate alpha based on fade in/out
        let total_lifetime = self.config.lifetime.unwrap_or(Duration::from_secs(3600));
        let fade_in_secs = self.config.fade_in.as_secs_f32();
        let fade_out_secs = self.config.fade_out.as_secs_f32();
        let elapsed_secs = elapsed.as_secs_f32();
        let total_secs = total_lifetime.as_secs_f32();

        self.current_alpha = if elapsed_secs < fade_in_secs {
            // Fading in
            elapsed_secs / fade_in_secs
        } else if elapsed_secs > total_secs - fade_out_secs {
            // Fading out
            (total_secs - elapsed_secs) / fade_out_secs
        } else {
            // Full intensity
            1.0
        };

        self.current_alpha = self.current_alpha.clamp(0.0, 1.0);
    }

    /// Get current rendering alpha
    pub fn alpha(&self) -> f32 {
        self.current_alpha
    }

    /// Get current color with alpha applied
    pub fn current_color(&self) -> [f32; 4] {
        let mut color = self.config.color;
        color[3] *= self.current_alpha;
        color
    }

    /// Get effect age
    pub fn age(&self) -> Duration {
        self.created_at.elapsed()
    }

    /// Get configuration
    pub fn config(&self) -> &RayEffectConfig {
        &self.config
    }

    /// Update positions (for tracking moving targets)
    pub fn update_positions(&mut self, start: Point3<f32>, end: Point3<f32>) {
        self.config.start = start;
        self.config.end = end;
    }

    /// Calculate points along the beam for rendering
    pub fn generate_beam_points(&self, time: f32) -> Vec<Point3<f32>> {
        let direction = self.config.end - self.config.start;
        let length = direction.norm();
        let normalized_dir = direction.normalize();

        let mut points = Vec::with_capacity(self.config.segments as usize + 1);

        for i in 0..=self.config.segments {
            let t = i as f32 / self.config.segments as f32;
            let mut point = self.config.start + normalized_dir * (length * t);

            // Apply noise/distortion
            if self.config.noise_amount > 0.0 {
                let noise_offset = match self.config.ray_type {
                    RayType::Lightning => {
                        // More chaotic for lightning
                        let noise_x = ((t * 10.0 + time * 5.0).sin() * (t * 7.0).cos())
                            * self.config.noise_amount;
                        let noise_y = ((t * 8.0 + time * 3.0).cos() * (t * 9.0).sin())
                            * self.config.noise_amount;
                        let noise_z = ((t * 6.0 + time * 4.0).sin() * (t * 11.0).cos())
                            * self.config.noise_amount;
                        Vector3::new(noise_x, noise_y, noise_z)
                    }
                    _ => {
                        // Subtle wave for other beams
                        let noise = (t * 20.0 + time * 10.0).sin() * self.config.noise_amount * 0.1;
                        // Perpendicular offset
                        let perpendicular = if normalized_dir.y.abs() < 0.9 {
                            Vector3::new(0.0, 1.0, 0.0)
                                .cross(&normalized_dir)
                                .normalize()
                        } else {
                            Vector3::new(1.0, 0.0, 0.0)
                                .cross(&normalized_dir)
                                .normalize()
                        };
                        perpendicular * noise
                    }
                };

                point += noise_offset;
            }

            points.push(point);
        }

        points
    }

    /// Get pulse intensity at current time
    pub fn pulse_intensity(&self, time: f32) -> f32 {
        if self.config.pulse_frequency > 0.0 {
            let pulse = (time * self.config.pulse_frequency * std::f32::consts::TAU).sin();
            0.5 + pulse * 0.5 // Oscillate between 0.5 and 1.0
        } else {
            1.0
        }
    }
}

/// Ray effect manager
pub struct RayEffectManager {
    effects: HashMap<RayEffectId, RayEffect>,
    next_id: RayEffectId,
    time: f32,
}

impl RayEffectManager {
    /// Create a new ray effect manager
    pub fn new() -> Self {
        Self {
            effects: HashMap::new(),
            next_id: 1,
            time: 0.0,
        }
    }

    /// Spawn a new ray effect
    pub fn spawn(&mut self, config: RayEffectConfig) -> RayEffectId {
        let id = self.next_id;
        self.next_id += 1;

        let effect = RayEffect::new(id, config);
        self.effects.insert(id, effect);

        id
    }

    /// Remove a ray effect
    pub fn remove(&mut self, id: RayEffectId) -> bool {
        self.effects.remove(&id).is_some()
    }

    /// Get ray effect
    pub fn get(&self, id: RayEffectId) -> Option<&RayEffect> {
        self.effects.get(&id)
    }

    /// Get mutable ray effect
    pub fn get_mut(&mut self, id: RayEffectId) -> Option<&mut RayEffect> {
        self.effects.get_mut(&id)
    }

    /// Update all ray effects
    pub fn update(&mut self, delta_time: f32) {
        self.time += delta_time;

        // Update all effects
        for effect in self.effects.values_mut() {
            effect.update(delta_time);
        }

        // Remove inactive effects
        self.effects.retain(|_, effect| effect.is_active());
    }

    /// Get all active effects
    pub fn active_effects(&self) -> impl Iterator<Item = &RayEffect> {
        self.effects.values().filter(|e| e.is_active())
    }

    /// Get current time
    pub fn time(&self) -> f32 {
        self.time
    }

    /// Get number of active effects
    pub fn count(&self) -> usize {
        self.effects.len()
    }

    /// Clear all effects
    pub fn clear(&mut self) {
        self.effects.clear();
    }

    /// Apply LOD (Level of Detail) to effects based on distance
    pub fn apply_lod(
        &self,
        camera_position: Point3<f32>,
        lod_config: &super::EffectsConfig,
    ) -> Vec<(RayEffectId, EffectsLOD)> {
        self.effects
            .values()
            .map(|effect| {
                let mid_point = effect.start() + (effect.end() - effect.start()) * 0.5;
                let distance = (mid_point - camera_position).norm();
                let lod = super::calculate_effects_lod(distance, lod_config);
                (effect.id(), lod)
            })
            .collect()
    }
}

impl Default for RayEffectManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Ray effect rendering data for GPU
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct RayRenderData {
    pub start: [f32; 3],
    pub width: f32,
    pub end: [f32; 3],
    pub intensity: f32,
    pub color: [f32; 4],
    pub glow: f32,
    pub pulse: f32,
    pub alpha: f32,
    pub _padding: f32,
}

impl RayRenderData {
    /// Create render data from ray effect
    pub fn from_effect(effect: &RayEffect, time: f32) -> Self {
        let color = effect.current_color();
        let pulse = effect.pulse_intensity(time);

        Self {
            start: [
                effect.config.start.x,
                effect.config.start.y,
                effect.config.start.z,
            ],
            width: effect.config.width,
            end: [
                effect.config.end.x,
                effect.config.end.y,
                effect.config.end.z,
            ],
            intensity: effect.config.intensity * pulse,
            color,
            glow: effect.config.glow,
            pulse,
            alpha: effect.alpha(),
            _padding: 0.0,
        }
    }
}

// Ensure proper alignment for GPU buffers
unsafe impl bytemuck::Pod for RayRenderData {}
unsafe impl bytemuck::Zeroable for RayRenderData {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ray_effect_creation() {
        let config = RayEffectConfig::laser();
        let effect = RayEffect::new(1, config);

        assert_eq!(effect.id(), 1);
        assert!(effect.is_active());
        assert_eq!(effect.alpha(), 0.0); // Starts at 0 before first update
    }

    #[test]
    fn test_ray_effect_update() {
        let config = RayEffectConfig::default();
        let mut effect = RayEffect::new(1, config);

        effect.update(0.1);
        assert!(effect.is_active());
        assert!(effect.alpha() > 0.0);
    }

    #[test]
    fn test_ray_effect_lifetime() {
        let mut config = RayEffectConfig::default();
        config.lifetime = Some(Duration::from_millis(100));
        config.fade_in = Duration::from_millis(10);
        config.fade_out = Duration::from_millis(10);

        let mut effect = RayEffect::new(1, config);

        // Simulate time passing
        for _ in 0..20 {
            effect.update(0.01);
        }

        assert!(!effect.is_active());
    }

    #[test]
    fn test_ray_manager() {
        let mut manager = RayEffectManager::new();

        let id1 = manager.spawn(RayEffectConfig::laser());
        let id2 = manager.spawn(RayEffectConfig::particle_cannon());

        assert_eq!(manager.count(), 2);

        manager.remove(id1);
        assert_eq!(manager.count(), 1);

        manager.clear();
        assert_eq!(manager.count(), 0);
    }

    #[test]
    fn test_beam_point_generation() {
        let config =
            RayEffectConfig::default().between(Point3::origin(), Point3::new(0.0, 0.0, 10.0));
        let effect = RayEffect::new(1, config);

        let points = effect.generate_beam_points(0.0);
        assert_eq!(points.len(), 17); // segments + 1

        // First and last points should match start/end (approximately with noise)
        let first = points.first().unwrap();
        let last = points.last().unwrap();

        assert!((first - effect.start()).norm() < 0.1);
        assert!((last - effect.end()).norm() < 0.1);
    }

    #[test]
    fn test_pulse_intensity() {
        let mut config = RayEffectConfig::laser();
        config.pulse_frequency = 1.0; // 1 Hz
        let effect = RayEffect::new(1, config);

        let intensity_0 = effect.pulse_intensity(0.0);
        let intensity_quarter = effect.pulse_intensity(0.25);
        let intensity_half = effect.pulse_intensity(0.5);

        assert!(intensity_0 >= 0.5 && intensity_0 <= 1.0);
        assert!(intensity_quarter >= 0.5 && intensity_quarter <= 1.0);
        assert!(intensity_half >= 0.5 && intensity_half <= 1.0);
    }

    #[test]
    fn test_ray_presets() {
        let laser = RayEffectConfig::laser();
        assert_eq!(laser.ray_type, RayType::Laser);

        let particle = RayEffectConfig::particle_cannon();
        assert_eq!(particle.ray_type, RayType::ParticleBeam);

        let lightning = RayEffectConfig::lightning();
        assert_eq!(lightning.ray_type, RayType::Lightning);

        let microwave = RayEffectConfig::microwave();
        assert_eq!(microwave.ray_type, RayType::Microwave);
    }
}
