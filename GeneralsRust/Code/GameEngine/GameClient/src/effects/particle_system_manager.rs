// Lightweight compatibility shim for the ParticleSystemManager API.
// This file intentionally re-exports the existing, fully-ported
// ParticleSystemManager from the C++ parity port implemented in
// particle_manager.rs. The goal is to expose the same public surface
// (types and constants) under the new module path
// `effects::particle_system_manager` so existing call sites can reference
// the new path without requiring a complete rework of the surrounding code.

// Public re-exports to preserve parity with the original C++ API.
pub use crate::effects::particle_manager::{
    EmissionVelocity, EmissionVelocityType, EmissionVolume, EmissionVolumeType,
    GameClientRandomVariable, INVALID_PARTICLE_SYSTEM_ID, Keyframe, MAX_KEYFRAMES,
    ObjectId as ParticleObjectId, ParticlePriorityType, ParticleShaderType, ParticleSystemId,
    ParticleSystemManager, ParticleSystemTemplate, ParticleType as CppParticleTypeEnum,
    RGBColorKeyframe, RandomKeyframe, WindMotion,
};

// Also re-export the core ParticleSystem type used by the renderer from the
// dedicated particle_system module to keep downstream code happy.
pub use crate::effects::particle_system::ParticleSystem;
