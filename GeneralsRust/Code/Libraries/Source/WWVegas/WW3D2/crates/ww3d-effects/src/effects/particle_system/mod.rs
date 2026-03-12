//! # Particle System Module
//!
//! This module provides comprehensive particle effects functionality for the WW3D engine,
//! converted from C++ to Rust with WGPU integration.
//!
//! ## Components
//!
//! - `particle_emitter`: Core particle emitter class
//! - `particle_buffer`: Particle storage and management
//! - `particle_system`: Complete particle system
//! - `particle_properties`: Particle property definitions

pub mod particle_buffer;
pub mod particle_emitter;
pub mod particle_properties;
pub mod particle_system;

// Re-export specific types to avoid naming conflicts
pub use particle_buffer::{Particle, ParticleBuffer};
pub use particle_emitter::{ParticleEmitterClass, ParticleEmitterUtils, ParticleProperty};
pub use particle_properties::{ParticleProperties, ParticlePropertyLoader, ParticlePropertySets};
pub use particle_system::{ParticleSystem, ParticleSystemConfig, ParticleSystemManager};
