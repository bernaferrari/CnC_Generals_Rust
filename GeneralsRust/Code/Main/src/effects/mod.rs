//! Complete particle effects, animations, and audio integration
//!
//! This module provides the authentic C&C Generals visual and audio experience,
//! matching the original C++ implementation exactly.

pub mod animation_system;
pub mod audio_integration;
pub mod integration;
pub mod lighting_system;
pub mod particle_system;
pub mod performance;
pub mod visual_effects;

pub use animation_system::{Animation, AnimationManager, AnimationType};
pub use audio_integration::{AudioEvent, AudioEventType, EnhancedAudioManager};
pub use integration::EffectsIntegration;
pub use lighting_system::{DynamicLighting, LightSource, LightType};
pub use particle_system::{Particle, ParticlePriority, ParticleSystem, ParticleSystemManager};
pub use performance::{EffectLODManager, EffectPool, QualityLevel};
pub use visual_effects::{ExplosionEffect, VisualEffectsManager, WeaponEffect};
