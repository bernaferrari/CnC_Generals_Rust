//! Particle System Core
//!
//! This module provides the core particle system functionality.

use crate::effects::particle_system::particle_buffer::{Particle, ParticleBuffer};
use glam::{Vec3, Vec4};
use ww3d_core::errors::W3DResult;

/// Particle system configuration
#[derive(Clone, Debug)]
pub struct ParticleSystemConfig {
    pub max_particles: usize,
    pub emission_rate: f32,
    pub lifetime: f32,
    pub start_speed: f32,
    pub start_size: f32,
    pub start_color: Vec4,
    pub gravity: Vec3,
}

impl Default for ParticleSystemConfig {
    fn default() -> Self {
        Self {
            max_particles: 1000,
            emission_rate: 100.0,
            lifetime: 2.0,
            start_speed: 10.0,
            start_size: 1.0,
            start_color: Vec4::ONE,
            gravity: Vec3::new(0.0, -9.81, 0.0),
        }
    }
}

/// Particle system class
pub struct ParticleSystem {
    buffer: ParticleBuffer,
    config: ParticleSystemConfig,
    accumulator: f32,
    position: Vec3,
    is_emitting: bool,
}

impl ParticleSystem {
    /// Create a new particle system
    pub fn new(config: ParticleSystemConfig) -> Self {
        Self {
            buffer: ParticleBuffer::new(config.max_particles),
            config,
            accumulator: 0.0,
            position: Vec3::ZERO,
            is_emitting: false,
        }
    }

    /// Start emitting particles
    pub fn start_emitting(&mut self) {
        self.is_emitting = true;
    }

    /// Stop emitting particles
    pub fn stop_emitting(&mut self) {
        self.is_emitting = false;
    }

    /// Set emission position
    pub fn set_position(&mut self, position: Vec3) {
        self.position = position;
    }

    /// Update particle system
    pub fn update(&mut self, delta_time: f32) {
        // Update existing particles
        self.buffer.update(delta_time);

        // Emit new particles
        if self.is_emitting {
            self.accumulator += self.config.emission_rate * delta_time;
            let particles_to_emit = self.accumulator as usize;
            self.accumulator -= particles_to_emit as f32;

            for _ in 0..particles_to_emit {
                if let Ok(particle) = self.create_particle() {
                    let _ = self.buffer.add_particle(particle);
                }
            }
        }
    }

    /// Create a new particle
    fn create_particle(&self) -> W3DResult<Particle> {
        let mut particle = Particle::new();
        particle.position = self.position;
        particle.velocity = Vec3::new(
            (rand::random::<f32>() - 0.5) * 2.0,
            (rand::random::<f32>() - 0.5) * 2.0,
            (rand::random::<f32>() - 0.5) * 2.0,
        )
        .normalize()
            * self.config.start_speed;
        particle.color = self.config.start_color;
        particle.size = self.config.start_size;
        particle.lifetime = self.config.lifetime;
        Ok(particle)
    }

    /// Get active particles
    pub fn active_particles(&self) -> &[Particle] {
        self.buffer.active_particles()
    }

    /// Get active particle count
    pub fn active_count(&self) -> usize {
        self.buffer.active_count()
    }

    /// Clear all particles
    pub fn clear(&mut self) {
        self.buffer.clear();
    }

    /// Check if system is emitting
    pub fn is_emitting(&self) -> bool {
        self.is_emitting
    }
}

/// Particle system manager
pub struct ParticleSystemManager {
    systems: Vec<ParticleSystem>,
}

impl ParticleSystemManager {
    /// Create a new particle system manager
    pub fn new() -> Self {
        Self {
            systems: Vec::new(),
        }
    }

    /// Add particle system
    pub fn add_system(&mut self, system: ParticleSystem) {
        self.systems.push(system);
    }

    /// Update all particle systems
    pub fn update_all(&mut self, delta_time: f32) {
        for system in &mut self.systems {
            system.update(delta_time);
        }
    }

    /// Get total active particles across all systems
    pub fn total_active_particles(&self) -> usize {
        self.systems.iter().map(|s| s.active_count()).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_particle_system_creation() {
        let config = ParticleSystemConfig::default();
        let system = ParticleSystem::new(config);

        assert_eq!(system.active_count(), 0);
        assert!(!system.is_emitting());
    }

    #[test]
    fn test_particle_system_start_stop_emission() {
        let config = ParticleSystemConfig::default();
        let mut system = ParticleSystem::new(config);

        assert!(!system.is_emitting());

        system.start_emitting();
        assert!(system.is_emitting());

        system.stop_emitting();
        assert!(!system.is_emitting());
    }

    #[test]
    fn test_particle_system_emission() {
        let mut config = ParticleSystemConfig::default();
        config.emission_rate = 100.0; // Emit 100 particles per second

        let mut system = ParticleSystem::new(config);
        system.start_emitting();

        // Update for 0.1 seconds (should emit ~10 particles)
        system.update(0.1);

        assert!(system.active_count() > 0);
        assert!(system.active_count() <= 15); // Allow some variation due to accumulator
    }

    #[test]
    fn test_particle_system_position() {
        let config = ParticleSystemConfig::default();
        let mut system = ParticleSystem::new(config);

        let position = Vec3::new(5.0, 10.0, 15.0);
        system.set_position(position);

        system.start_emitting();
        system.update(0.01); // Small update to emit one particle

        if system.active_count() > 0 {
            let particles = system.active_particles();
            assert_eq!(particles[0].position, position);
        }
    }

    #[test]
    fn test_particle_system_max_particles() {
        let mut config = ParticleSystemConfig::default();
        config.max_particles = 10;
        config.emission_rate = 1000.0; // Very high emission rate

        let mut system = ParticleSystem::new(config);
        system.start_emitting();

        // Multiple updates to exceed max
        system.update(0.1);
        system.update(0.1);
        system.update(0.1);

        // Should never exceed max_particles
        assert!(system.active_count() <= 10);
    }

    #[test]
    fn test_particle_system_clear() {
        let mut config = ParticleSystemConfig::default();
        config.emission_rate = 100.0;

        let mut system = ParticleSystem::new(config);
        system.start_emitting();
        system.update(0.1); // Emit particles

        assert!(system.active_count() > 0);

        system.clear();
        assert_eq!(system.active_count(), 0);
    }

    #[test]
    fn test_particle_system_config_default() {
        let config = ParticleSystemConfig::default();

        assert_eq!(config.max_particles, 1000);
        assert_eq!(config.emission_rate, 100.0);
        assert_eq!(config.lifetime, 2.0);
        assert_eq!(config.start_speed, 10.0);
        assert_eq!(config.start_size, 1.0);
        assert_eq!(config.gravity, Vec3::new(0.0, -9.81, 0.0));
    }

    #[test]
    fn test_particle_system_custom_config() {
        let config = ParticleSystemConfig {
            max_particles: 500,
            emission_rate: 50.0,
            lifetime: 5.0,
            start_speed: 20.0,
            start_size: 2.0,
            start_color: Vec4::new(1.0, 0.0, 0.0, 1.0),
            gravity: Vec3::new(0.0, -5.0, 0.0),
        };

        let system = ParticleSystem::new(config.clone());
        assert_eq!(system.active_count(), 0);
    }

    #[test]
    fn test_particle_system_manager_creation() {
        let manager = ParticleSystemManager::new();
        assert_eq!(manager.total_active_particles(), 0);
    }

    #[test]
    fn test_particle_system_manager_add_system() {
        let mut manager = ParticleSystemManager::new();
        let config = ParticleSystemConfig::default();
        let system = ParticleSystem::new(config);

        manager.add_system(system);
        assert_eq!(manager.total_active_particles(), 0);
    }

    #[test]
    fn test_particle_system_manager_update() {
        let mut manager = ParticleSystemManager::new();

        let mut config = ParticleSystemConfig::default();
        config.emission_rate = 100.0;

        let mut system = ParticleSystem::new(config);
        system.start_emitting();

        manager.add_system(system);
        manager.update_all(0.1);

        assert!(manager.total_active_particles() > 0);
    }

    #[test]
    fn test_particle_system_manager_multiple_systems() {
        let mut manager = ParticleSystemManager::new();

        let mut config1 = ParticleSystemConfig::default();
        config1.emission_rate = 50.0;
        let mut system1 = ParticleSystem::new(config1);
        system1.start_emitting();

        let mut config2 = ParticleSystemConfig::default();
        config2.emission_rate = 30.0;
        let mut system2 = ParticleSystem::new(config2);
        system2.start_emitting();

        manager.add_system(system1);
        manager.add_system(system2);
        manager.update_all(0.1);

        assert!(manager.total_active_particles() > 0);
    }

    #[test]
    fn test_particle_system_lifetime() {
        let mut config = ParticleSystemConfig::default();
        config.lifetime = 1.0; // 1 second lifetime
        config.emission_rate = 100.0; // High emission rate to ensure particles

        let mut system = ParticleSystem::new(config);
        system.start_emitting();

        // Emit particles
        system.update(0.1);
        let particles_emitted = system.active_count();
        assert!(particles_emitted > 0, "No particles were emitted");

        // Wait for them to die
        system.stop_emitting();
        for _ in 0..15 {
            system.update(0.1);
        }

        // All particles should be dead (after 1.5 seconds of aging)
        assert_eq!(system.active_count(), 0);
    }

    #[test]
    fn test_particle_system_stop_emission_preserves_particles() {
        let mut config = ParticleSystemConfig::default();
        config.emission_rate = 100.0;
        config.lifetime = 2.0;

        let mut system = ParticleSystem::new(config);
        system.start_emitting();
        system.update(0.1); // Emit particles

        let count_before = system.active_count();
        assert!(count_before > 0);

        system.stop_emitting();
        system.update(0.05); // Update without emitting

        // Particles should still be there, just fewer (due to aging)
        assert!(system.active_count() <= count_before);
    }
}
