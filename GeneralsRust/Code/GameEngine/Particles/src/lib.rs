//! Renderer-independent authored particle primitives and explicit-input kernels.
//! Authority: GameClient/ParticleSys.h and System/ParticleSys.cpp.
#![forbid(unsafe_code)]

mod definition;
mod enums;
mod motion;
mod sampling;

pub use definition::*;
pub use enums::*;
pub use motion::{apply_wind_motion, integrate_translation};
pub use sampling::{point_on_unit_hemisphere, point_on_unit_sphere};

/// Maximum number of keyframes for particle animation
pub const MAX_KEYFRAMES: usize = 8;

/// Maximum volume particle depth
pub const MAX_VOLUME_PARTICLE_DEPTH: u32 = 16;
pub const DEFAULT_VOLUME_PARTICLE_DEPTH: u32 = 0;
pub const OPTIMUM_VOLUME_PARTICLE_DEPTH: u32 = 6;

/// Unique identifier for particle systems
pub type ParticleSystemId = u32;
pub const INVALID_PARTICLE_SYSTEM_ID: ParticleSystemId = 0;

/// Unique identifier for game objects
pub type ObjectId = u32;

#[cfg(test)]
mod tests;
