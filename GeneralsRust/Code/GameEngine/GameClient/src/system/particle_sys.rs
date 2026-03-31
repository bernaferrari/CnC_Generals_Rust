//! Particle system wrapper (matches System/ParticleSys.cpp).
//!
//! Re-exports the C++-matching particle system implementation from the effects module.

// C++-matching particle system types (particle_manager.rs)
pub use crate::effects::particle_manager::{
    EmissionVelocity, EmissionVelocityType, EmissionVolume, EmissionVolumeType,
    GameClientRandomVariable, Keyframe, ObjectId as ParticleObjectId, ParticlePriorityType,
    ParticleShaderType, ParticleSystemId, ParticleSystemManager, ParticleSystemTemplate,
    ParticleType as ParticleTypeEnum, RGBColorKeyframe, RandomKeyframe, WindMotion, MAX_KEYFRAMES,
};

// C++-matching particle and system types (particle_system.rs)
pub use crate::effects::particle_system::{Particle, ParticleInfo, ParticleSystem};

// Renderer
pub use crate::effects::particle_renderer::ParticleRenderer;
