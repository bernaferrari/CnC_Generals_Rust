//! Exact named `CHUNK_GhostObject` companion owned by its decoded save.
//!
//! C++ GameState.cpp:305 writes this independent Snapshotable block and
//! :661-683 protects creation while loading it. Main captures and restores the
//! driving session's plain manager; parsing never publishes a pending payload.

use crate::save_load::{SaveLoadError, SaveLoadResult};
use game_engine::common::game_common::MAX_PLAYER_COUNT;
use gamelogic::object::w3d_ghost_object::{
    Matrix3x4, ParentGeometrySnapshot, RenderObjectClass, RenderObjectState,
    RenderSubObjectSnapshot, W3DDrawableInfo, W3DGhostObjectManager, W3DRenderObjectSnapshot,
};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

pub const CHUNK_GHOST_OBJECT: &str = "CHUNK_GhostObject";

/// Typed form of the exact state persisted by the legacy GhostObject xfer.
/// Derived render flags are normalized during admission instead of trusted.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct W3DGhostObjectManagerArchive {
    pub local_player: usize,
    pub ghosts: Vec<W3DGhostObjectArchive>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct W3DGhostObjectArchive {
    pub parent_object_id: Option<u32>,
    pub parent_geometry: ParentGeometrySnapshot,
    pub drawable_info: W3DDrawableInfo,
    pub player_snapshots: Vec<Vec<W3DRenderObjectSnapshotArchive>>,
    pub previous_shroudedness: Vec<(usize, u8)>,
}

/// Exact render snapshot fields present in the original save stream. Runtime-
/// derived class and animation flags are recomputed during admission.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct W3DRenderObjectSnapshotArchive {
    pub name: String,
    pub scale: f32,
    pub color: u32,
    pub transform: Matrix3x4,
    pub sub_objects: Vec<RenderSubObjectSnapshot>,
}

pub fn capture_w3d_ghost_archive(
    manager: &W3DGhostObjectManager,
) -> SaveLoadResult<W3DGhostObjectManagerArchive> {
    if manager.used_count() > u16::MAX as usize {
        return Err(SaveLoadError::Serialization(
            "too many ghost objects for save archive".into(),
        ));
    }
    let mut ghosts = Vec::with_capacity(manager.used_count());
    for ghost in manager.used() {
        let mut player_snapshots = Vec::with_capacity(MAX_PLAYER_COUNT);
        let mut previous_shroudedness = Vec::new();
        for player in 0..MAX_PLAYER_COUNT {
            let snapshots = ghost.snapshots(player);
            if snapshots.len() > u8::MAX as usize {
                return Err(SaveLoadError::Serialization(
                    "too many per-player ghost snapshots for save archive".into(),
                ));
            }
            player_snapshots.push(
                snapshots
                    .iter()
                    .cloned()
                    .map(|snapshot| W3DRenderObjectSnapshotArchive {
                        name: snapshot.render_object.name.clone(),
                        scale: snapshot.render_object.scale,
                        color: snapshot.render_object.color,
                        transform: snapshot.render_object.transform,
                        sub_objects: snapshot.render_object.sub_objects.clone(),
                    })
                    .collect(),
            );
            if ghost.has_snapshot(player) {
                previous_shroudedness
                    .push((player, ghost.previous_shroudedness(player).unwrap_or(0)));
            }
        }
        ghosts.push(W3DGhostObjectArchive {
            parent_object_id: ghost.parent_object_id(),
            parent_geometry: ghost.parent_geometry().unwrap_or(ParentGeometrySnapshot {
                geometry_type: 0,
                is_small: false,
                major_radius: 0.0,
                minor_radius: 0.0,
                position: [0.0; 3],
                angle: 0.0,
            }),
            drawable_info: ghost.drawable_info(),
            player_snapshots,
            previous_shroudedness,
        });
    }
    Ok(W3DGhostObjectManagerArchive {
        local_player: manager.local_player_index(),
        ghosts,
    })
}

fn decode_render_snapshot(saved: &W3DRenderObjectSnapshotArchive) -> W3DRenderObjectSnapshot {
    W3DRenderObjectSnapshot::new(RenderObjectState {
        name: saved.name.clone(),
        scale: saved.scale,
        geometry_scale: saved.scale,
        color: saved.color,
        transform: saved.transform,
        class_id: if saved.sub_objects.is_empty() {
            RenderObjectClass::Mesh
        } else {
            RenderObjectClass::HLod
        },
        sub_objects: saved.sub_objects.clone(),
    })
}

/// Decode typed archive data into a detached manager using the same allocation,
/// ordering, normalization, and scene-event rules as the original xfer reader.
pub fn decode_w3d_ghost_archive(
    archive: &W3DGhostObjectManagerArchive,
) -> SaveLoadResult<W3DGhostObjectManager> {
    if archive.local_player >= MAX_PLAYER_COUNT {
        return Err(SaveLoadError::Corrupted(
            "Invalid saved ghost local player".into(),
        ));
    }
    if archive.ghosts.len() > u16::MAX as usize {
        return Err(SaveLoadError::Corrupted(
            "Saved ghost object count exceeds supported domain".into(),
        ));
    }
    let mut loaded = W3DGhostObjectManager::new();
    loaded.set_save_lock_ghost_objects(false);
    loaded.set_local_player_index(archive.local_player);
    for saved in &archive.ghosts {
        if saved.player_snapshots.len() != MAX_PLAYER_COUNT {
            return Err(SaveLoadError::Corrupted(
                "Saved ghost player snapshot table has wrong width".into(),
            ));
        }
        if saved
            .player_snapshots
            .iter()
            .any(|snapshots| snapshots.len() > u8::MAX as usize)
        {
            return Err(SaveLoadError::Corrupted(
                "Saved ghost player snapshot count exceeds supported domain".into(),
            ));
        }
        let mut shroud_players = HashSet::new();
        for &(player, _) in &saved.previous_shroudedness {
            if player >= MAX_PLAYER_COUNT
                || !shroud_players.insert(player)
                || saved.player_snapshots[player].is_empty()
            {
                return Err(SaveLoadError::Corrupted(
                    "Invalid saved ghost shroudedness entry".into(),
                ));
            }
        }
        loaded
            .add_ghost_object(saved.parent_object_id, saved.parent_object_id.is_some())
            .ok_or_else(|| {
                SaveLoadError::Corrupted("could not create saved ghost object".into())
            })?;
        let ghost = &mut loaded.used_mut()[0];
        ghost.update_parent_object(saved.parent_object_id, true);
        ghost.set_parent_geometry(saved.parent_geometry);
        ghost.set_drawable_info(saved.drawable_info);
        for (player, snapshots) in saved.player_snapshots.iter().enumerate() {
            ghost.replace_player_snapshots(
                player,
                snapshots
                    .iter()
                    .map(decode_render_snapshot)
                    .collect(),
            );
        }
        for &(player, status) in &saved.previous_shroudedness {
            ghost.set_previous_shroudedness(player, status);
        }
        ghost.emit_xfer_load_scene_events(archive.local_player);
    }
    validate_w3d_ghost_manager(&loaded)?;
    loaded.set_save_lock_ghost_objects(false);
    Ok(loaded)
}

fn validate_w3d_ghost_manager(loaded: &W3DGhostObjectManager) -> SaveLoadResult<()> {
    // Geometry can subsequently drive actual orphan partition queries. Validate
    // before the candidate becomes live.
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
        if footprint_samples > (1 << 24) {
            return Err(SaveLoadError::Corrupted(
                "Saved ghost manager exceeds the supported footprint sample budget".into(),
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "w3d_ghost_owner_tests.rs"]
mod owner_tests;
