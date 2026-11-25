//! Host `.sav` companion for the C++ ParticleSystem continuation.
//!
//! The typed archive preserves the original system order, identifiers, random
//! streams, and native-load side effects without exposing the legacy positional
//! byte codec to the production save container.

use crate::save_load::{SaveLoadError, SaveLoadResult};

pub const CHUNK_PARTICLE_SYSTEM: &str = "CHUNK_ParticleSystem";

pub use game_client::effects::particle_archive::ParticleSystemManagerArchive;

pub fn capture_particle_system_archive(
    manager: &game_client::effects::particle_manager::ParticleSystemManager,
) -> ParticleSystemManagerArchive {
    manager.capture_archive()
}

pub fn restore_particle_system_from_archive(
    manager: &mut game_client::effects::particle_manager::ParticleSystemManager,
    archive: &ParticleSystemManagerArchive,
) -> SaveLoadResult<game_client::effects::particle_manager::ParticleSystemRestoreReceipt> {
    manager
        .restore_archive(archive)
        .map_err(SaveLoadError::Serialization)
}
