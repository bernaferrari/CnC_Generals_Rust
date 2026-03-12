//! Particle Buffer System
//!
//! This module handles particle data storage and management.

use glam::{Vec3, Vec4};
use ww3d_core::errors::{W3DError, W3DResult};

/// Particle data structure
#[derive(Clone, Debug)]
pub struct Particle {
    pub position: Vec3,
    pub velocity: Vec3,
    pub color: Vec4,
    pub size: f32,
    pub lifetime: f32,
    pub age: f32,
}

impl Particle {
    /// Create a new particle
    pub fn new() -> Self {
        Self {
            position: Vec3::ZERO,
            velocity: Vec3::ZERO,
            color: Vec4::ONE,
            size: 1.0,
            lifetime: 1.0,
            age: 0.0,
        }
    }

    /// Update particle
    pub fn update(&mut self, delta_time: f32) {
        self.position += self.velocity * delta_time;
        self.age += delta_time;
    }

    /// Check if particle is alive
    pub fn is_alive(&self) -> bool {
        self.age < self.lifetime
    }

    /// Get normalized age (0.0 to 1.0)
    pub fn normalized_age(&self) -> f32 {
        if self.lifetime > 0.0 {
            self.age / self.lifetime
        } else {
            0.0
        }
    }
}

/// Particle buffer for managing multiple particles
pub struct ParticleBuffer {
    particles: Vec<Particle>,
    max_particles: usize,
    active_count: usize,
}

impl ParticleBuffer {
    /// Create a new particle buffer
    pub fn new(max_particles: usize) -> Self {
        Self {
            particles: vec![Particle::new(); max_particles],
            max_particles,
            active_count: 0,
        }
    }

    /// Add a particle to the buffer
    pub fn add_particle(&mut self, particle: Particle) -> W3DResult<()> {
        if self.active_count >= self.max_particles {
            return Err(W3DError::InvalidParameter(
                "Particle buffer is full".to_string(),
            ));
        }

        self.particles[self.active_count] = particle;
        self.active_count += 1;
        Ok(())
    }

    /// Update all particles
    pub fn update(&mut self, delta_time: f32) {
        let mut new_active_count = 0;

        for i in 0..self.active_count {
            self.particles[i].update(delta_time);
            if self.particles[i].is_alive() {
                // Move alive particle to the front
                if new_active_count != i {
                    self.particles.swap(new_active_count, i);
                }
                new_active_count += 1;
            }
        }

        self.active_count = new_active_count;
    }

    /// Get active particles
    pub fn active_particles(&self) -> &[Particle] {
        &self.particles[..self.active_count]
    }

    /// Get active particle count
    pub fn active_count(&self) -> usize {
        self.active_count
    }

    /// Get maximum particle count
    pub fn max_particles(&self) -> usize {
        self.max_particles
    }

    /// Clear all particles
    pub fn clear(&mut self) {
        self.active_count = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec3;

    #[test]
    fn test_particle_creation() {
        let particle = Particle::new();
        assert_eq!(particle.position, Vec3::ZERO);
        assert_eq!(particle.velocity, Vec3::ZERO);
        assert_eq!(particle.color, Vec4::ONE);
        assert_eq!(particle.size, 1.0);
        assert_eq!(particle.lifetime, 1.0);
        assert_eq!(particle.age, 0.0);
    }

    #[test]
    fn test_particle_update() {
        let mut particle = Particle {
            position: Vec3::ZERO,
            velocity: Vec3::new(1.0, 2.0, 3.0),
            color: Vec4::ONE,
            size: 1.0,
            lifetime: 2.0,
            age: 0.0,
        };

        particle.update(0.5);

        assert_eq!(particle.position, Vec3::new(0.5, 1.0, 1.5));
        assert_eq!(particle.age, 0.5);
        assert!(particle.is_alive());
    }

    #[test]
    fn test_particle_lifetime() {
        let mut particle = Particle {
            position: Vec3::ZERO,
            velocity: Vec3::ZERO,
            color: Vec4::ONE,
            size: 1.0,
            lifetime: 1.0,
            age: 0.5,
        };

        assert!(particle.is_alive());

        particle.update(0.6);
        assert!(!particle.is_alive());
    }

    #[test]
    fn test_particle_buffer_creation() {
        let buffer = ParticleBuffer::new(100);
        assert_eq!(buffer.active_count(), 0);
        assert_eq!(buffer.max_particles(), 100);
    }

    #[test]
    fn test_particle_buffer_add_particle() {
        let mut buffer = ParticleBuffer::new(10);
        let particle = Particle::new();

        assert!(buffer.add_particle(particle).is_ok());
        assert_eq!(buffer.active_count(), 1);
    }

    #[test]
    fn test_particle_buffer_full() {
        let mut buffer = ParticleBuffer::new(1);
        let particle1 = Particle::new();
        let particle2 = Particle::new();

        assert!(buffer.add_particle(particle1).is_ok());
        assert!(buffer.add_particle(particle2).is_err());
        assert_eq!(buffer.active_count(), 1);
    }

    #[test]
    fn test_particle_buffer_update() {
        let mut buffer = ParticleBuffer::new(10);
        let particle = Particle {
            position: Vec3::ZERO,
            velocity: Vec3::ONE,
            color: Vec4::ONE,
            size: 1.0,
            lifetime: 1.0,
            age: 0.0,
        };

        buffer.add_particle(particle).unwrap();
        buffer.update(0.5);

        let active_particles = buffer.active_particles();
        assert_eq!(active_particles.len(), 1);
        assert_eq!(active_particles[0].position, Vec3::new(0.5, 0.5, 0.5));
        assert_eq!(active_particles[0].age, 0.5);
    }

    #[test]
    fn test_particle_buffer_clear() {
        let mut buffer = ParticleBuffer::new(10);
        let particle = Particle::new();

        buffer.add_particle(particle).unwrap();
        assert_eq!(buffer.active_count(), 1);

        buffer.clear();
        assert_eq!(buffer.active_count(), 0);
    }
}
