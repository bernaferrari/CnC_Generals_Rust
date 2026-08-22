//! Host `.sav` companion for leftover `CHUNK_ParticleSystem`.
//!
//! C++ `ParticleSystemManager::xfer` (`ParticleSys.cpp:3232-3323`) writes
//! uniqueSystemID, systemCount, then each saveable ParticleSystem. Live save
//! used a NullSnapshot placeholder, so mid-flight explosions vanished.

use crate::save_load::{SaveLoadError, SaveLoadResult};
use std::sync::Mutex;

pub const CHUNK_PARTICLE_SYSTEM: &str = "CHUNK_ParticleSystem";

static PENDING_PARTICLE_XFER: Mutex<Option<Vec<u8>>> = Mutex::new(None);

pub fn capture_particle_system_xfer_bytes() -> SaveLoadResult<Vec<u8>> {
    game_client::effects::capture_live_particle_system_xfer_bytes()
        .map_err(SaveLoadError::Serialization)
}

pub fn stash_loaded_particle_system_xfer(bytes: Vec<u8>) {
    if let Ok(mut slot) = PENDING_PARTICLE_XFER.lock() {
        *slot = Some(bytes);
    }
}

pub fn take_loaded_particle_system_xfer() -> Option<Vec<u8>> {
    PENDING_PARTICLE_XFER
        .lock()
        .ok()
        .and_then(|mut slot| slot.take())
}

pub fn restore_particle_system_from_xfer_bytes(bytes: &[u8]) -> SaveLoadResult<()> {
    game_client::effects::restore_live_particle_system_from_xfer_bytes(bytes)
        .map_err(SaveLoadError::Serialization)
}
