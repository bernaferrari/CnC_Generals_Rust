//! Host `.sav` companion for leftover `CHUNK_TerrainVisual`.
//!
//! C++ `W3DTerrainVisual::xfer` v3 (`W3DTerrainVisual.cpp:1174-1274`) writes
//! water-grid enable, logic height-map bytes, and the terrain render object.
//! Live host also persists the client scorch overlay so napalm / Particle
//! Cannon / map scorches survive save/load.

use crate::save_load::{SaveLoadError, SaveLoadResult};
pub const CHUNK_TERRAIN_VISUAL: &str = "CHUNK_TerrainVisual";
pub use game_client::terrain::terrain_visual_xfer::PreparedTerrainVisual;

/// Explicit logical owner, with an optional native observation from the caller.
pub fn capture_receiving_terrain_visual_xfer_bytes(
    world: &crate::game_logic::GameLogic,
    scorches: Option<&[game_client::terrain::scorch_mesh::ScorchMark]>,
) -> SaveLoadResult<Vec<u8>> {
    capture_receiving_terrain_visual_with_water(world, None, scorches)
}

pub fn capture_receiving_terrain_visual_with_water(
    world: &crate::game_logic::GameLogic,
    native_water: Option<&gamelogic::terrain_water::WaterGridState>,
    scorches: Option<&[game_client::terrain::scorch_mesh::ScorchMark]>,
) -> SaveLoadResult<Vec<u8>> {
    let terrain = world
        .world_services
        .terrain()
        .read()
        .map_err(|_| SaveLoadError::Serialization("receiving terrain lock poisoned".into()))?;
    game_client::terrain::terrain_visual_xfer::capture_terrain_visual_with_water(
        &terrain,
        native_water,
        scorches,
    )
    .map_err(SaveLoadError::Serialization)
}

/// No native effects during candidate preparation or logical admission.
pub fn prepare_receiving_terrain_visual(
    world: &crate::game_logic::GameLogic,
    bytes: Option<&[u8]>,
) -> SaveLoadResult<Option<PreparedTerrainVisual>> {
    let Some(bytes) = bytes else {
        return Ok(None);
    };
    let terrain = world
        .world_services
        .terrain()
        .read()
        .map_err(|_| SaveLoadError::Corrupted("receiving terrain lock poisoned".into()))?;
    game_client::terrain::terrain_visual_xfer::prepare_terrain_visual_for_terrain(&terrain, bytes)
        .map_err(SaveLoadError::Corrupted)
}
pub fn admit_receiving_terrain_visual(
    world: &mut crate::game_logic::GameLogic,
    prepared: &PreparedTerrainVisual,
) -> SaveLoadResult<()> {
    let mut terrain = world
        .world_services
        .terrain()
        .write()
        .map_err(|_| SaveLoadError::Corrupted("candidate terrain lock poisoned".into()))?;
    prepared
        .admit_logical(&mut terrain)
        .map_err(SaveLoadError::Corrupted)
}

// Standalone compatibility test/service APIs; actual archive paths never use
// these ambient owner adapters. Native renderer ownership remains x3l7v work.
pub fn capture_terrain_visual_xfer_bytes() -> SaveLoadResult<Vec<u8>> {
    game_client::terrain::capture_live_terrain_visual_xfer_bytes()
        .map_err(SaveLoadError::Serialization)
}
pub fn restore_terrain_visual_from_xfer_bytes(bytes: &[u8]) -> SaveLoadResult<()> {
    game_client::terrain::restore_live_terrain_visual_from_xfer_bytes(bytes)
        .map_err(SaveLoadError::Serialization)
}
