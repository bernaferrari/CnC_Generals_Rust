//! Exact W3D ghost scene frozen from the driving Main session.
use crate::game_logic::GameLogic;
use game_client::render_bridge::FrozenGhostSceneFrame;
use gamelogic::object::w3d_ghost_object::{FrozenW3DGhostSnapshot, W3DGhostSnapshotKey};

pub(super) fn freeze_owned_scene(
    logic: &mut GameLogic,
    local_player: u32,
) -> FrozenGhostSceneFrame {
    if logic.w3d_ghosts.local_player_index() != local_player as usize {
        // C++ setLocalPlayerIndex removes the old scene, restores its parent,
        // then installs the selected local player's retained snapshots.
        logic
            .w3d_ghosts
            .set_local_player_index(local_player as usize);
    }
    // Query transitions after selecting the actual viewer. No renderer call
    // here: only this session's completed native observation is consumed.
    if let Err(error) = logic.update_host_fog_ghosts_for_viewer(local_player) {
        log::error!("Cannot update this session's fog ghosts: {error}");
    }
    let manager = &mut logic.w3d_ghosts;
    let mut snapshots = Vec::new();
    let mut hidden_parent_object_ids = Vec::new();
    for ghost in manager.used() {
        // C++ snapShot removes normal render objects for local snapshots
        // (:340-348) without setting FullyObscured. removeParentObject
        // (:375+) also obscures the Drawable; both hide the normal parent.
        if ghost.has_snapshot(local_player as usize) || ghost.parent_fully_obscured() {
            if let Some(parent) = ghost.parent_object_id() {
                hidden_parent_object_ids.push(parent);
            }
        }
        for (snapshot_index, render_object) in
            ghost.snapshots(local_player as usize).iter().enumerate()
        {
            snapshots.push(FrozenW3DGhostSnapshot {
                key: W3DGhostSnapshotKey {
                    ghost_id: ghost.scene_id(),
                    player_index: local_player as usize,
                    snapshot_index,
                },
                parent_object_id: ghost.parent_object_id(),
                drawable_info: ghost.drawable_info(),
                parent_geometry: ghost.parent_geometry(),
                render_object: render_object.clone(),
            });
        }
    }
    snapshots.sort_by_key(|snapshot| {
        (
            snapshot.key.ghost_id,
            snapshot.key.player_index,
            snapshot.key.snapshot_index,
        )
    });
    hidden_parent_object_ids.sort_unstable();
    hidden_parent_object_ids.dedup();
    // This whole-scene boundary carries the final result of those same events;
    // clearing delivered messages prevents an unbounded queue in the owner.
    manager.drain_scene_events();
    FrozenGhostSceneFrame {
        revision: logic.get_frame() as u64,
        snapshots,
        hidden_parent_object_ids,
    }
}
