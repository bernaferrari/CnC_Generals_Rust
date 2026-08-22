//! Host `.sav` companion for leftover `CHUNK_TerrainVisual`.
//!
//! C++ `W3DTerrainVisual::xfer` v3 (`W3DTerrainVisual.cpp:1174-1274`) writes
//! water-grid enable, logic height-map bytes, and the terrain render object.
//! Live host also persists the client scorch overlay so napalm / Particle
//! Cannon / map scorches survive save/load.

use crate::save_load::{SaveLoadError, SaveLoadResult};
use std::sync::Mutex;

pub const CHUNK_TERRAIN_VISUAL: &str = "CHUNK_TerrainVisual";

static PENDING_TERRAIN_VISUAL_XFER: Mutex<Option<Vec<u8>>> = Mutex::new(None);

pub fn capture_terrain_visual_xfer_bytes() -> SaveLoadResult<Vec<u8>> {
    game_client::terrain::capture_live_terrain_visual_xfer_bytes()
        .map_err(SaveLoadError::Serialization)
}

pub fn stash_loaded_terrain_visual_xfer(bytes: Vec<u8>) {
    if let Ok(mut slot) = PENDING_TERRAIN_VISUAL_XFER.lock() {
        *slot = Some(bytes);
    }
}

pub fn take_loaded_terrain_visual_xfer() -> Option<Vec<u8>> {
    PENDING_TERRAIN_VISUAL_XFER
        .lock()
        .ok()
        .and_then(|mut slot| slot.take())
}

pub fn restore_terrain_visual_from_xfer_bytes(bytes: &[u8]) -> SaveLoadResult<()> {
    game_client::terrain::restore_live_terrain_visual_from_xfer_bytes(bytes)
        .map_err(SaveLoadError::Serialization)
}
