//! Exact named `CHUNK_GhostObject` companion owned by its decoded save.
//!
//! C++ GameState.cpp:305 writes this independent Snapshotable block and
//! :661-683 protects creation while loading it. Main captures and restores the
//! driving session's plain manager; parsing never publishes a pending payload.

use crate::save_load::{SaveLoadError, SaveLoadResult};
use game_engine::common::system::Snapshotable;
use game_engine::common::system::xfer_load::XferLoad;
use game_engine::common::system::xfer_save::XferSave;
use gamelogic::object::w3d_ghost_object::W3DGhostObjectManager;
use std::io::Cursor;

pub const CHUNK_GHOST_OBJECT: &str = "CHUNK_GhostObject";

pub fn capture_w3d_ghost_xfer_bytes(manager: &W3DGhostObjectManager) -> SaveLoadResult<Vec<u8>> {
    let mut manager = manager.clone();
    let mut bytes = Vec::new();
    {
        let mut xfer = XferSave::new(Cursor::new(&mut bytes), 1);
        manager
            .xfer(&mut xfer)
            .map_err(SaveLoadError::Serialization)?;
    }
    Ok(bytes)
}

/// Decode into a detached candidate. An error cannot change another session's
/// creation lock, snapshots, or pending scene delivery.
pub fn decode_w3d_ghost_xfer_bytes(bytes: &[u8]) -> SaveLoadResult<W3DGhostObjectManager> {
    let mut loaded = W3DGhostObjectManager::new();
    loaded.set_save_lock_ghost_objects(true);
    let mut cursor = Cursor::new(bytes);
    {
        let mut xfer = XferLoad::new(&mut cursor, 1);
        loaded
            .xfer(&mut xfer)
            .map_err(SaveLoadError::Serialization)?;
    }
    // The reader's accounting counter excludes raw string bytes; actual
    // cursor consumption covers every original field and string payload.
    if cursor.position() != bytes.len() as u64 {
        return Err(SaveLoadError::Corrupted(
            "Trailing bytes in CHUNK_GhostObject manager".into(),
        ));
    }
    // The wire is unchanged, but its geometry can subsequently drive actual
    // orphan partition queries. Validate before the candidate becomes live.
    let mut footprint_samples = 0u64;
    for ghost in loaded.used() {
        let Some(geometry) = ghost.parent_geometry() else {
            continue;
        };
        if geometry.geometry_type > 2 || !geometry.position.iter().all(|value| value.is_finite()) {
            return Err(SaveLoadError::Corrupted(
                "Invalid saved ghost geometry or position".into(),
            ));
        }
        let samples = crate::game_logic::partition_coi::footprint_sample_cost(
            crate::game_logic::partition_coi::HostPartitionFootprint {
                major_radius: geometry.major_radius,
                minor_radius: geometry.minor_radius,
                angle: geometry.angle,
                is_small: geometry.is_small,
                is_box: geometry.geometry_type == 2,
            },
        )
        .map_err(|reason| SaveLoadError::Corrupted(reason.into()))?;
        footprint_samples = footprint_samples.checked_add(samples).ok_or_else(|| {
            SaveLoadError::Corrupted("Saved ghost footprint budget overflow".into())
        })?;
        // A manager-sized budget also prevents many individually admitted
        // large footprints from imposing unbounded work on its next tick.
        if footprint_samples > (1 << 24) {
            return Err(SaveLoadError::Corrupted(
                "Saved ghost manager exceeds the supported footprint sample budget".into(),
            ));
        }
    }
    // Manager Xfer unlocks to allocate its saved modules (W3DGhostObject.cpp:1172).
    // GameState.cpp:683 also unlocks after the named blocks have loaded.
    loaded.set_save_lock_ghost_objects(false);
    Ok(loaded)
}

#[cfg(test)]
#[path = "w3d_ghost_owner_tests.rs"]
mod owner_tests;
