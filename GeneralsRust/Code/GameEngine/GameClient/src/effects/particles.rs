//! # Particle System Implementation
//!
//! High-performance particle system with GPU acceleration for Command & Conquer
//! Generals Zero Hour visual effects including explosions, fire, smoke, and debris.

use nalgebra::{Point3, Vector3};
use rand::prelude::*;
use std::collections::VecDeque;
use std::time::{Duration, Instant};

use super::{utils, EffectsConfig, EffectsError, EffectsLOD};

/// Unique identifier for particle systems
pub type ParticleSystemId = u32;

/// Types of particle systems
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParticleType {
    /// Explosion with fire and debris
    Explosion,
    /// Smoke trails and plumes
    Smoke,
    /// Fire effects
    Fire,
    /// Sparks from impacts
    Sparks,
    /// Dust clouds
    Dust,
    /// Muzzle flash
    MuzzleFlash,
    /// Water splash
    Water,
    /// Blood effects
    Blood,
    /// Generic particle system
    Generic,
}

/// Individual particle data
#[derive(Debug, Clone)]
pub struct Particle {
    /// Current position
    pub position: Point3<f32>,
    /// Current velocity
    pub velocity: Vector3<f32>,
    /// Current size/scale
    pub size: f32,
    /// Current color (RGBA)
    pub color: [f32; 4],
    /// Current rotation (radians)
    pub rotation: f32,
    /// Angular velocity (radians per second)
    pub angular_velocity: f32,
    /// Age of this particle (seconds)
    pub age: f32,
    /// Maximum lifetime (seconds)
    pub lifetime: f32,
    /// Mass for physics calculations
    pub mass: f32,
    /// Whether particle is active
    pub active: bool,
    /// Texture UV coordinates
    pub uv_rect: [f32; 4], // [u_min, v_min, u_max, v_max]
}

impl Particle {
    /// Create a new particle
    pub fn new(
        position: Point3<f32>,
        velocity: Vector3<f32>,
        size: f32,
        color: [f32; 4],
        lifetime: f32,
    ) -> Self {
        Self {
            position,
            velocity,
            size,
            color,
            rotation: 0.0,
            angular_velocity: 0.0,
            age: 0.0,
            lifetime,
            mass: 1.0,
            active: true,
            uv_rect: [0.0, 0.0, 1.0, 1.0],
        }
    }

    /// Update particle physics
    pub fn update(&mut self, delta_time: f32, forces: &[ParticleForce]) {
        if !self.active {
            return;
        }

        // Update age
        self.age += delta_time;
        if self.age >= self.lifetime {
            self.active = false;
            return;
        }

        // Apply forces
        let mut acceleration = Vector3::new(0.0, 0.0, 0.0);
        for force in forces {
            acceleration += force.apply(self);
        }

        // Integrate physics (Verlet integration for stability)
        self.velocity += acceleration * delta_time;
        self.position += self.velocity * delta_time;
        self.rotation += self.angular_velocity * delta_time;

        // Apply drag
        let drag_factor = 0.95_f32.powf(delta_time);
        self.velocity *= drag_factor;
        self.angular_velocity *= drag_factor;
    }

    /// Get normalized age (0.0 to 1.0)
    pub fn normalized_age(&self) -> f32 {
        if self.lifetime <= 0.0 {
            1.0
        } else {
            (self.age / self.lifetime).clamp(0.0, 1.0)
        }
    }

    /// Check if particle is alive
    pub fn is_alive(&self) -> bool {
        self.active && self.age < self.lifetime
    }
}

/// Forces that can be applied to particles
#[derive(Debug, Clone)]
pub enum ParticleForce {
    /// Constant gravity force
    Gravity { strength: Vector3<f32> },
    /// Wind force with turbulence
    Wind {
        direction: Vector3<f32>,
        strength: f32,
        turbulence: f32,
    },
    /// Attraction to a point
    PointAttractor {
        position: Point3<f32>,
        strength: f32,
    },
    /// Repulsion from a point
    PointRepulsor {
        position: Point3<f32>,
        strength: f32,
    },
    /// Vortex/swirl force
    Vortex {
        center: Point3<f32>,
        axis: Vector3<f32>,
        strength: f32,
    },
    /// Drag/air resistance
    Drag { coefficient: f32 },
}

impl ParticleForce {
    /// Apply this force to a particle
    pub fn apply(&self, particle: &Particle) -> Vector3<f32> {
        match self {
            ParticleForce::Gravity { strength } => *strength,

            ParticleForce::Wind {
                direction,
                strength,
                turbulence,
            } => {
                let wind_force = direction.normalize() * *strength;

                // Add turbulence based on position and time
                let turb_x = (particle.position.x * 0.1 + particle.age).sin() * *turbulence;
                let turb_y = (particle.position.y * 0.1 + particle.age * 1.1).sin() * *turbulence;
                let turb_z = (particle.position.z * 0.1 + particle.age * 0.9).sin() * *turbulence;

                wind_force + Vector3::new(turb_x, turb_y, turb_z)
            }

            ParticleForce::PointAttractor { position, strength } => {
                let to_attractor = *position - particle.position;
                let distance_sq = to_attractor.norm_squared().max(0.1); // Avoid division by zero
                to_attractor.normalize() * (*strength / distance_sq)
            }

            ParticleForce::PointRepulsor { position, strength } => {
                let from_repulsor = particle.position - *position;
                let distance_sq = from_repulsor.norm_squared().max(0.1);
                from_repulsor.normalize() * (*strength / distance_sq)
            }

            ParticleForce::Vortex {
                center,
                axis,
                strength,
            } => {
                let to_center = particle.position - *center;
                let radius_vector = to_center - to_center.dot(axis) * *axis;
                let tangent = axis.cross(&radius_vector).normalize();
                tangent * *strength / (radius_vector.norm() + 0.1)
            }

            ParticleForce::Drag { coefficient } => {
                -particle.velocity * *coefficient / particle.mass
            }
        }
    }
}

/// Particle emitter configuration
#[derive(Debug, Clone)]
pub struct ParticleEmitter {
    /// Position of the emitter
    pub position: Point3<f32>,
    /// Emission rate (particles per second)
    pub emission_rate: f32,
    /// Emission direction (will add spread)
    pub direction: Vector3<f32>,
    /// Emission cone angle in radians
    pub spread_angle: f32,
    /// Initial speed range
    pub speed_range: (f32, f32),
    /// Initial size range
    pub size_range: (f32, f32),
    /// Initial color
    pub color: [f32; 4],
    /// Color variation (0.0 = no variation, 1.0 = full variation)
    pub color_variation: f32,
    /// Particle lifetime range
    pub lifetime_range: (f32, f32),
    /// Whether the emitter is active
    pub active: bool,
    /// Emitter lifetime (None = infinite)
    pub emitter_lifetime: Option<f32>,
    /// Time since emitter was created
    pub age: f32,
    /// Accumulator for fractional particles
    emission_accumulator: f32,
}

impl ParticleEmitter {
    /// Create a new particle emitter
    pub fn new(position: Point3<f32>) -> Self {
        Self {
            position,
            emission_rate: 100.0,
            direction: Vector3::new(0.0, 1.0, 0.0),
            spread_angle: 0.5,
            speed_range: (1.0, 5.0),
            size_range: (0.1, 0.3),
            color: [1.0, 1.0, 1.0, 1.0],
            color_variation: 0.0,
            lifetime_range: (1.0, 3.0),
            active: true,
            emitter_lifetime: None,
            age: 0.0,
            emission_accumulator: 0.0,
        }
    }

    /// Update emitter and emit particles
    pub fn update(&mut self, delta_time: f32) -> Vec<Particle> {
        if !self.active {
            return Vec::new();
        }

        // Update emitter age
        self.age += delta_time;

        // Check if emitter has expired
        if let Some(lifetime) = self.emitter_lifetime {
            if self.age >= lifetime {
                self.active = false;
                return Vec::new();
            }
        }

        // Calculate particles to emit this frame
        self.emission_accumulator += self.emission_rate * delta_time;
        let particles_to_emit = self.emission_accumulator as u32;
        self.emission_accumulator -= particles_to_emit as f32;

        let mut new_particles = Vec::with_capacity(particles_to_emit as usize);
        let mut rng = thread_rng();

        for _ in 0..particles_to_emit {
            // Generate random properties
            let velocity = utils::random_cone_velocity(
                self.direction,
                self.spread_angle,
                self.speed_range.0,
                self.speed_range.1,
            );

            let size = rng.gen_range(self.size_range.0..=self.size_range.1);
            let lifetime = rng.gen_range(self.lifetime_range.0..=self.lifetime_range.1);

            // Generate color with variation
            let mut color = self.color;
            if self.color_variation > 0.0 {
                for i in 0..3 {
                    // Don't vary alpha
                    let variation = (rng.gen::<f32>() - 0.5) * 2.0 * self.color_variation;
                    color[i] = (color[i] + variation).clamp(0.0, 1.0);
                }
            }

            let mut particle = Particle::new(self.position, velocity, size, color, lifetime);

            // Add some random rotation
            particle.angular_velocity = rng.gen_range(-2.0..=2.0);

            new_particles.push(particle);
        }

        new_particles
    }
}

/// Particle system implementation
pub struct ParticleSystem {
    /// Unique identifier
    pub id: ParticleSystemId,
    /// System type
    pub particle_type: ParticleType,
    /// All particles in this system
    pub particles: Vec<Particle>,
    /// Emitters for this system
    pub emitters: Vec<ParticleEmitter>,
    /// Forces acting on particles
    pub forces: Vec<ParticleForce>,
    /// System age
    pub age: f32,
    /// Maximum lifetime (None = infinite)
    pub lifetime: Option<f32>,
    /// Whether system is active
    pub active: bool,
    /// Position of the entire system
    pub position: Point3<f32>,
    /// System scale multiplier
    pub scale: f32,
    /// LOD level for this system
    pub lod: EffectsLOD,
    /// Performance statistics
    pub stats: ParticleStats,
}

impl ParticleSystem {
    /// Create a new particle system
    pub fn new(id: ParticleSystemId, particle_type: ParticleType, position: Point3<f32>) -> Self {
        Self {
            id,
            particle_type,
            particles: Vec::new(),
            emitters: Vec::new(),
            forces: Vec::new(),
            age: 0.0,
            lifetime: None,
            active: true,
            position,
            scale: 1.0,
            lod: EffectsLOD::High,
            stats: ParticleStats::default(),
        }
    }

    /// Add an emitter to the system
    pub fn add_emitter(&mut self, emitter: ParticleEmitter) {
        self.emitters.push(emitter);
    }

    /// Add a force to the system
    pub fn add_force(&mut self, force: ParticleForce) {
        self.forces.push(force);
    }

    /// Update the particle system
    pub fn update(&mut self, delta_time: f32, config: &EffectsConfig) {
        if !self.active {
            return;
        }

        let update_start = Instant::now();

        // Update system age
        self.age += delta_time;

        // Check if system has expired
        if let Some(lifetime) = self.lifetime {
            if self.age >= lifetime {
                // Stop emitters but let particles finish
                for emitter in &mut self.emitters {
                    emitter.active = false;
                }

                // If no particles left, deactivate system
                if self.particles.iter().all(|p| !p.is_alive()) {
                    self.active = false;
                    return;
                }
            }
        }

        // Apply LOD scaling
        let lod_multiplier = self.lod.particle_multiplier();
        let effective_delta_time = delta_time * self.lod.update_multiplier();

        // Update emitters and add new particles
        let mut new_particles = Vec::new();
        for emitter in &mut self.emitters {
            let mut emitted = emitter.update(effective_delta_time);

            // Apply LOD by potentially discarding some particles
            if lod_multiplier < 1.0 {
                let keep_count = (emitted.len() as f32 * lod_multiplier) as usize;
                emitted.truncate(keep_count);
            }

            new_particles.extend(emitted);
        }

        // Add new particles up to maximum
        let max_particles = (config.quality.max_particles() as f32 * lod_multiplier) as usize;
        let space_available = max_particles.saturating_sub(self.particles.len());

        if new_particles.len() > space_available {
            new_particles.truncate(space_available);
        }

        self.particles.extend(new_particles);

        // Update existing particles
        let mut alive_count = 0;
        for particle in &mut self.particles {
            if particle.is_alive() {
                particle.update(effective_delta_time, &self.forces);
                if particle.is_alive() {
                    alive_count += 1;
                }
            }
        }

        // Remove dead particles periodically for performance
        if self.age % 1.0 < delta_time {
            // Roughly once per second
            self.particles.retain(|p| p.is_alive());
        }

        // Update statistics
        self.stats.active_particles = alive_count;
        self.stats.update_time_ms = update_start.elapsed().as_secs_f64() * 1000.0;
    }

    /// Set LOD level for this system
    pub fn set_lod(&mut self, lod: EffectsLOD) {
        self.lod = lod;
    }

    /// Get active particle count
    pub fn active_particle_count(&self) -> usize {
        self.particles.iter().filter(|p| p.is_alive()).count()
    }

    /// Check if system is finished (no emitters active and no particles alive)
    pub fn is_finished(&self) -> bool {
        !self.active
            || (self.emitters.iter().all(|e| !e.active)
                && self.particles.iter().all(|p| !p.is_alive()))
    }
}

/// Performance statistics for particle systems
#[derive(Debug, Default)]
pub struct ParticleStats {
    /// Number of active particles
    pub active_particles: usize,
    /// Update time in milliseconds
    pub update_time_ms: f64,
    /// Render time in milliseconds
    pub render_time_ms: f64,
    /// GPU memory usage in bytes
    pub gpu_memory_bytes: usize,
}

/// Particle system renderer bridge.
pub struct ParticleRenderer {
    /// Whether rendering is enabled
    enabled: bool,
}

impl ParticleRenderer {
    /// Create new particle renderer
    pub fn new() -> Self {
        Self { enabled: true }
    }

    /// Render a particle system
    pub fn render(&self, _system: &ParticleSystem) -> Result<(), EffectsError> {
        if !self.enabled {
            return Ok(());
        }

        // Rendering is handled by the GPU particle renderer wired into the display pipeline.

        Ok(())
    }

    /// Enable/disable rendering
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }
}

impl Default for ParticleRenderer {
    fn default() -> Self {
        Self::new()
    }
}

/// Particle system description for easy creation
#[derive(Debug, Clone)]
pub struct ParticleSystemDesc {
    /// Type of particle system
    pub particle_type: ParticleType,
    /// Position
    pub position: Point3<f32>,
    /// Scale multiplier
    pub scale: f32,
    /// Duration (None = infinite)
    pub duration: Option<f32>,
    /// Custom emitter settings
    pub emitter: Option<ParticleEmitter>,
    /// Custom forces
    pub forces: Vec<ParticleForce>,
}

impl ParticleSystemDesc {
    /// Create explosion particle system
    pub fn explosion() -> Self {
        Self {
            particle_type: ParticleType::Explosion,
            position: Point3::new(0.0, 0.0, 0.0),
            scale: 1.0,
            duration: Some(5.0),
            emitter: None,
            forces: vec![
                ParticleForce::Gravity {
                    strength: Vector3::new(0.0, 0.0, -9.8),
                },
                ParticleForce::Drag { coefficient: 0.1 },
            ],
        }
    }

    /// Create fire particle system
    pub fn fire() -> Self {
        Self {
            particle_type: ParticleType::Fire,
            position: Point3::new(0.0, 0.0, 0.0),
            scale: 1.0,
            duration: None, // Infinite
            emitter: None,
            forces: vec![
                ParticleForce::Wind {
                    direction: Vector3::new(0.0, 0.0, 1.0),
                    strength: 2.0,
                    turbulence: 1.0,
                },
                ParticleForce::Drag { coefficient: 0.05 },
            ],
        }
    }

    /// Create smoke particle system
    pub fn smoke() -> Self {
        Self {
            particle_type: ParticleType::Smoke,
            position: Point3::new(0.0, 0.0, 0.0),
            scale: 1.0,
            duration: Some(10.0),
            emitter: None,
            forces: vec![
                ParticleForce::Wind {
                    direction: Vector3::new(0.0, 0.0, 1.0),
                    strength: 1.0,
                    turbulence: 0.5,
                },
                ParticleForce::Drag { coefficient: 0.02 },
            ],
        }
    }

    /// Set position
    pub fn at_position(mut self, x: f32, y: f32, z: f32) -> Self {
        self.position = Point3::new(x, y, z);
        self
    }

    /// Set scale
    pub fn with_scale(mut self, scale: f32) -> Self {
        self.scale = scale;
        self
    }

    /// Set duration
    pub fn with_duration(mut self, duration: f32) -> Self {
        self.duration = Some(duration);
        self
    }

    /// Create the actual particle system
    pub fn build(self, id: ParticleSystemId) -> ParticleSystem {
        let mut system = ParticleSystem::new(id, self.particle_type, self.position);
        system.scale = self.scale;
        system.lifetime = self.duration;

        // Add forces
        system.forces = self.forces;

        // Add default emitter if none specified
        if let Some(emitter) = self.emitter {
            system.add_emitter(emitter);
        } else {
            let mut emitter = ParticleEmitter::new(self.position);

            // Configure emitter based on particle type
            match self.particle_type {
                ParticleType::Explosion => {
                    emitter.emission_rate = 500.0;
                    emitter.speed_range = (5.0, 15.0);
                    emitter.size_range = (0.2, 0.8);
                    emitter.lifetime_range = (1.0, 3.0);
                    emitter.color = [1.0, 0.5, 0.2, 1.0]; // Orange
                    emitter.color_variation = 0.3;
                    emitter.spread_angle = std::f32::consts::PI; // Full sphere
                    emitter.emitter_lifetime = Some(0.5); // Quick burst
                }

                ParticleType::Fire => {
                    emitter.emission_rate = 100.0;
                    emitter.speed_range = (0.5, 2.0);
                    emitter.size_range = (0.1, 0.4);
                    emitter.lifetime_range = (2.0, 5.0);
                    emitter.color = [1.0, 0.3, 0.1, 0.8]; // Red-orange
                    emitter.color_variation = 0.2;
                    emitter.spread_angle = 0.5;
                    emitter.direction = Vector3::new(0.0, 0.0, 1.0); // Upward
                }

                ParticleType::Smoke => {
                    emitter.emission_rate = 50.0;
                    emitter.speed_range = (0.2, 1.0);
                    emitter.size_range = (0.3, 1.0);
                    emitter.lifetime_range = (5.0, 10.0);
                    emitter.color = [0.3, 0.3, 0.3, 0.6]; // Gray
                    emitter.color_variation = 0.1;
                    emitter.spread_angle = 0.3;
                    emitter.direction = Vector3::new(0.0, 0.0, 1.0); // Upward
                }

                _ => {} // Use defaults for other types
            }

            system.add_emitter(emitter);
        }

        system
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_particle_creation() {
        let position = Point3::new(1.0, 2.0, 3.0);
        let velocity = Vector3::new(0.5, 1.0, 0.0);
        let particle = Particle::new(position, velocity, 1.0, [1.0, 1.0, 1.0, 1.0], 5.0);

        assert_eq!(particle.position, position);
        assert_eq!(particle.velocity, velocity);
        assert_eq!(particle.lifetime, 5.0);
        assert!(particle.is_alive());
        assert_eq!(particle.normalized_age(), 0.0);
    }

    #[test]
    fn test_particle_aging() {
        let mut particle = Particle::new(
            Point3::new(0.0, 0.0, 0.0),
            Vector3::new(0.0, 0.0, 0.0),
            1.0,
            [1.0, 1.0, 1.0, 1.0],
            2.0,
        );

        // Age particle halfway
        particle.age = 1.0;
        assert_eq!(particle.normalized_age(), 0.5);
        assert!(particle.is_alive());

        // Age particle to death
        particle.age = 2.5;
        assert!(particle.normalized_age() >= 1.0);
        assert!(!particle.is_alive());
    }

    #[test]
    fn test_particle_forces() {
        let particle = Particle::new(
            Point3::new(0.0, 0.0, 0.0),
            Vector3::new(1.0, 0.0, 0.0),
            1.0,
            [1.0, 1.0, 1.0, 1.0],
            5.0,
        );

        let gravity = ParticleForce::Gravity {
            strength: Vector3::new(0.0, 0.0, -9.8),
        };
        let force = gravity.apply(&particle);
        assert_eq!(force, Vector3::new(0.0, 0.0, -9.8));

        let drag = ParticleForce::Drag { coefficient: 0.1 };
        let drag_force = drag.apply(&particle);
        assert_eq!(drag_force, Vector3::new(-0.1, 0.0, 0.0)); // Opposite to velocity
    }

    #[test]
    fn test_particle_emitter() {
        let mut emitter = ParticleEmitter::new(Point3::new(0.0, 0.0, 0.0));
        emitter.emission_rate = 100.0; // 100 particles per second

        let particles = emitter.update(0.1); // 0.1 second
                                             // Should emit approximately 10 particles (100 * 0.1)
        assert!(particles.len() >= 8 && particles.len() <= 12); // Some variance due to accumulator
    }

    #[test]
    fn test_particle_system() {
        let mut system =
            ParticleSystem::new(1, ParticleType::Explosion, Point3::new(0.0, 0.0, 0.0));

        let emitter = ParticleEmitter::new(Point3::new(0.0, 0.0, 0.0));
        system.add_emitter(emitter);

        let config = EffectsConfig::default();

        // Initially no particles
        assert_eq!(system.active_particle_count(), 0);

        // Update should create particles
        system.update(0.1, &config);
        assert!(system.active_particle_count() > 0);

        // System should not be finished while emitters are active
        assert!(!system.is_finished());
    }

    #[test]
    fn test_particle_system_desc() {
        let desc = ParticleSystemDesc::explosion()
            .at_position(10.0, 20.0, 30.0)
            .with_scale(2.0)
            .with_duration(3.0);

        let system = desc.build(42);

        assert_eq!(system.id, 42);
        assert_eq!(system.particle_type, ParticleType::Explosion);
        assert_eq!(system.position, Point3::new(10.0, 20.0, 30.0));
        assert_eq!(system.scale, 2.0);
        assert_eq!(system.lifetime, Some(3.0));
        assert!(!system.emitters.is_empty());
    }

    #[test]
    fn test_particle_renderer() {
        let renderer = ParticleRenderer::new();
        let system = ParticleSystem::new(1, ParticleType::Fire, Point3::new(0.0, 0.0, 0.0));

        // Renderer should accept basic systems without error.
        assert!(renderer.render(&system).is_ok());
    }
}
