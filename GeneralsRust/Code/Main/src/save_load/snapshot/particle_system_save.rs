//! Host `.sav` companion for leftover `CHUNK_ParticleSystem`.
//!
//! C++ `ParticleSystemManager::xfer` (`ParticleSys.cpp:3232-3323`) writes
//! uniqueSystemID, systemCount, then each saveable ParticleSystem. Live save
//! used a NullSnapshot placeholder, so mid-flight explosions vanished.

use crate::save_load::{SaveLoadError, SaveLoadResult};

pub const CHUNK_PARTICLE_SYSTEM: &str = "CHUNK_ParticleSystem";

pub fn capture_particle_system_xfer_bytes(
    manager: &mut game_client::effects::particle_manager::ParticleSystemManager,
) -> SaveLoadResult<Vec<u8>> {
    manager
        .capture_xfer_bytes()
        .map_err(SaveLoadError::Serialization)
}

pub fn restore_particle_system_from_xfer_bytes(
    manager: &mut game_client::effects::particle_manager::ParticleSystemManager,
    bytes: &[u8],
) -> SaveLoadResult<Option<game_client::effects::particle_manager::ParticleSystemRestoreReceipt>> {
    manager
        .restore_xfer_bytes(bytes)
        .map_err(SaveLoadError::Serialization)
}
