//! Session-owned C++ PartitionData / W3DGhostObject lifetime and fog transitions.
//! The renderer supplies completed native W3D state; this owner never guesses
//! a RenderObj from an INI name or consults the Core object/ghost registries.
use super::super::*;
use crate::assets::LiveDrawPlaybackIdentity;
use gamelogic::common::types::ObjectShroudStatus;
use gamelogic::object::w3d_ghost_object::{
    ParentGeometrySnapshot, RenderObjectState, W3DGhostObjectManager, W3DGhostSnapshotCapture,
};

/// Immutable native ModelDraw output. The identity is the existing admitted
/// world/actor lease, so equal ObjectIDs and a reset cannot admit foreign art.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CommittedHostGhostDraw {
    pub identity: LiveDrawPlaybackIdentity,
    pub generation: u64,
    pub render_objects: Vec<RenderObjectState>,
}

/// Explicit immutable source components used by native ModelDraw creation.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct HostGhostScaleSource {
    pub identity: LiveDrawPlaybackIdentity,
    pub asset_scale: f32,
    pub instance_scale: f32,
}

/// W3DGhostObject::snapShot saves the current Object shape and pose.
fn snapshot_parent_geometry(object: &Object) -> ParentGeometrySnapshot {
    let info = object.geometry_info();
    let position = object.get_position();
    ParentGeometrySnapshot {
        geometry_type: info.geom_type as u32,
        is_small: info.is_small,
        major_radius: info.major_radius,
        minor_radius: info.minor_radius,
        // Saved Geometry is C++ XY / Z-up, while Main Objects use XZ / Y-up.
        position: [position.x, position.z, position.y],
        angle: object.get_orientation(),
    }
}

impl GameLogic {
    /// The engine calls this after the actual native draw completes. Partial
    /// or unavailable assets are absent; they never create fabricated ghosts.
    pub(crate) fn admit_host_ghost_draws(&mut self, draws: Vec<CommittedHostGhostDraw>) {
        for draw in draws {
            let id = ObjectId(draw.identity.object_id());
            let Some(object) = self.objects.get(&id) else {
                continue;
            };
            if self.live_draw_playback_identity(object) != draw.identity
                || self.partition_manager.object_ghost(id.0).is_none()
            {
                continue;
            }
            if draw.render_objects.is_empty() {
                self.host_ghost_draws.remove(&id);
            } else {
                self.host_ghost_draws.insert(id, draw);
            }
        }
    }

    pub(crate) fn fog_ghost_scene(&self) -> &W3DGhostObjectManager {
        self.partition_manager.fog_scene()
    }

    pub(crate) fn drain_host_fog_scene_events(&mut self) {
        self.partition_manager.fog_scene_mut().drain_scene_events();
    }

    #[cfg(test)]
    pub(crate) fn fixture_fog_ghost_scene_mut(&mut self) -> &mut W3DGhostObjectManager {
        self.partition_manager.fog_scene_mut()
    }

    /// Save restoration owns scene and actor reconstruction independently.
    /// Suppress new ghost creation until the saved links can be assigned.
    pub(crate) fn install_saved_fog_ghost_scene(&mut self, mut scene: W3DGhostObjectManager) {
        self.host_ghost_draws.clear();
        self.partition_manager.release_ghost_partition_links();
        scene.set_lock_ghost_objects(true);
        *self.partition_manager.fog_scene_mut() = scene;
    }

    /// Xfer and boundary replacement link fog memory after the receiving
    /// modules and their grid have been installed.
    pub(crate) fn restore_host_fog_ghost_links(&mut self) -> Result<(), String> {
        self.partition_manager.restore_ghost_partition_links(|id| {
            self.objects.contains_key(&ObjectId(id))
        });
        self.restore_host_fog_ghost_shroud_history()
    }

    /// C++ saved fog memory also restores the partition's previously seen
    /// premise. Main's shroud tail stores cells, not derived object caches;
    /// rebuild this fact after installing that tail, before the next COI mix.
    fn restore_host_fog_ghost_shroud_history(&self) -> Result<(), String> {
        let shroud = std::sync::Arc::clone(self.world_services.shroud());
        let mut shroud = shroud
            .lock()
            .map_err(|_| "session shroud poisoned during ghost restore".to_string())?;
        for ghost in self.fog_ghost_scene().used() {
            let Some(parent) = ghost.parent_object_id() else {
                continue;
            };
            for player in 0..game_engine::common::game_common::MAX_PLAYER_COUNT {
                if ghost.has_snapshot(player) {
                    shroud.set_host_object_ever_seen(player as u32, parent, true);
                }
            }
        }
        Ok(())
    }

    /// Same phase as the C++ PartitionData status mix. The shroud query is
    /// supplied by this session; native rendered pose remains immutable.
    pub(crate) fn update_host_fog_ghosts(&mut self) -> Result<(), String> {
        let shroud = std::sync::Arc::clone(self.world_services.shroud());
        let shroud = shroud
            .lock()
            .map_err(|_| "session shroud poisoned during fog transition".to_string())?;
        self.update_host_fog_ghosts_with_shroud(&shroud)
    }

    pub(crate) fn update_host_fog_ghosts_with_shroud(
        &mut self,
        shroud: &gamelogic::system::shroud_manager::ShroudManager,
    ) -> Result<(), String> {
        // This is the same canonical host player selection used by Main's
        // start/load/replay/UI path, not the manager constructor's default0.
        if let Some(local) = self.local_player_id() {
            self.select_host_fog_ghost_viewer(local);
        }
        self.apply_host_fog_ghost_transitions(shroud, false)
    }

    fn select_host_fog_ghost_viewer(&mut self, local: u32) {
        if self.fog_ghost_scene().local_player_index() != local as usize {
            self.partition_manager.fog_scene_mut().set_local_player_index(local as usize);
        }
    }

    /// Presentation supplies its exact viewer explicitly, including observer
    /// frames. It uses the same owner/algorithm with no ambient selection.
    pub(crate) fn update_host_fog_ghosts_for_viewer(&mut self, local: u32) -> Result<(), String> {
        self.select_host_fog_ghost_viewer(local);
        let shroud = std::sync::Arc::clone(self.world_services.shroud());
        let shroud = shroud
            .lock()
            .map_err(|_| "session shroud poisoned during fog freeze".to_string())?;
        self.apply_host_fog_ghost_transitions(&shroud, false)
    }

    /// GameClient::update visits parentless ghosts only while time is unfrozen.
    pub(crate) fn update_orphaned_host_fog_ghosts(&mut self) -> Result<(), String> {
        let shroud = std::sync::Arc::clone(self.world_services.shroud());
        let shroud = shroud
            .lock()
            .map_err(|_| "session shroud poisoned during orphan update".to_string())?;
        self.apply_host_fog_ghost_transitions(&shroud, true)
    }

    fn remove_snapshotless_host_orphans(&mut self, local: usize) {
        let empty: Vec<_> = self.fog_ghost_scene().used().iter()
            .filter(|ghost| ghost.parent_object_id().is_none() && !ghost.has_snapshot(local))
            .map(|ghost| ghost.scene_id())
            .collect();
        for scene_id in empty {
            self.partition_manager.remove_orphan_ghost(scene_id);
        }
    }

    fn apply_host_fog_ghost_transitions(
        &mut self,
        shroud: &gamelogic::system::shroud_manager::ShroudManager,
        update_orphans: bool,
    ) -> Result<(), String> {

        let local = self.fog_ghost_scene().local_player_index();
        // CPP getShroudedStatus returns INVALID before a partition grid
        // exists. A staged/cold world cannot release saved fog memory.
        let Some(origin) = shroud.grid_world_origin() else {
            if update_orphans {
                self.remove_snapshotless_host_orphans(local);
            }
            return Ok(());
        };
        let Some((width, height, cell_size)) = shroud.grid_dimensions() else {
            if update_orphans {
                self.remove_snapshotless_host_orphans(local);
            }
            return Ok(());
        };
        let dimensions = [width, height];
        let partition = self.partition_manager.matching_shroud_grid(origin, dimensions, cell_size);
        let mut transitions = Vec::new();
        for ghost in self.fog_ghost_scene().used().iter().filter(|_| !update_orphans) {
            let Some(id) = ghost.parent_object_id().map(ObjectId) else {
                continue;
            };
            if self.partition_manager.object_ghost(id.0) != Some(ghost.scene_id()) {
                continue;
            }
            let Some(status) = shroud.get_host_object_shroud_status(local as u32, id.0) else {
                continue;
            };
            let Some(object) = self.objects.get(&id) else {
                continue;
            };
            let position = object.get_position();
            let footprint = crate::game_logic::game_logic::host_object_footprint(object);
            let computed;
            let cells = if let Some(cells) = partition.and_then(|partition| {
                partition.stamped_footprint_cells(id.0, position.x, position.z, footprint)
            }) {
                cells
            } else {
                computed = crate::game_logic::partition_coi::cells_touched_for_footprint_on_grid(
                    origin, dimensions, cell_size, position.x, position.z, footprint,
                )
                .map_err(|reason| format!("host object {} fog footprint: {reason}", id.0))?;
                &computed
            };
            let previous = ghost.previous_shroudedness(local);
            // No COIs neither release a snapshot nor advance the saved status.
            // An unchanged status also needs no second count of the cells.
            if cells.is_empty() || previous == Some(status as u8) {
                continue;
            }
            // Only the Fogged -> Shrouded transition depends on how the COIs
            // produced Shrouded. Other transitions read the published status.
            let all_cells_shrouded = if previous == Some(ObjectShroudStatus::Fogged as u8)
                && status == ObjectShroudStatus::Shrouded
            {
                let counts = shroud.count_footprint_cells(local as u32, cells);
                counts.shrouded == counts.total
            } else {
                false
            };
            transitions.push((ghost.scene_id(), Some(id), previous, status, all_cells_shrouded));
        }
        // Parentless ghosts retain their captured geometry, not the location
        // or visibility of a newly admitted actor with the same ObjectID.
        for ghost in self
            .fog_ghost_scene()
            .used()
            .iter()
            .filter(|ghost| update_orphans && ghost.parent_object_id().is_none())
        {
            let Some(cells) = self.partition_manager.ghost_partition_cells(ghost.scene_id()) else {
                continue;
            };
            let counts = shroud.count_footprint_cells(local as u32, &cells);
            if counts.total == 0 {
                continue;
            }
            // C++ null-parent PartitionData mixes the same cells without
            // Object relationship/mobile/mine exclusions.
            let status = if counts.total == 0 || counts.shrouded == counts.total {
                ObjectShroudStatus::Shrouded
            } else if counts.shrouded + counts.fogged == counts.total {
                ObjectShroudStatus::Fogged
            } else if counts.shrouded + counts.fogged > 0 {
                ObjectShroudStatus::PartialClear
            } else {
                ObjectShroudStatus::Clear
            };
            transitions.push((
                ghost.scene_id(),
                None,
                ghost.previous_shroudedness(local),
                status,
                counts.shrouded == counts.total,
            ));
        }
        // Preserve C++ manager head-to-tail order (including saved order).
        for (scene_id, parent, previous, current, all_cells_shrouded) in transitions {
            if current == ObjectShroudStatus::Fogged
                && parent.is_some()
                && previous.unwrap_or(ObjectShroudStatus::Invalid as u8)
                    < ObjectShroudStatus::Fogged as u8
            {
                let id = parent.unwrap();
                let Some(draw) = self.host_ghost_draws.get(&id) else {
                    continue;
                };
                let Some(object) = self.objects.get(&id) else {
                    continue;
                };
                if draw.identity != self.live_draw_playback_identity(object) {
                    continue;
                }
                self.partition_manager.fog_scene_mut().snapshot_linked_ghost(
                    scene_id,
                    local,
                    &W3DGhostSnapshotCapture {
                        capture_window_generation: Some(draw.generation),
                        drawable_effectively_hidden: object.drawable_is_effectively_hidden(),
                        render_objects: draw.render_objects.clone(),
                        geometry: snapshot_parent_geometry(object),
                    },
                );
            } else if previous == Some(ObjectShroudStatus::Fogged as u8)
                && (current != ObjectShroudStatus::Shrouded || all_cells_shrouded)
                && matches!(
                    current,
                    ObjectShroudStatus::Clear
                        | ObjectShroudStatus::PartialClear
                        | ObjectShroudStatus::Shrouded
                )
            {
                self.partition_manager.fog_scene_mut().free_linked_ghost_snapshot(scene_id, local);
            }
            if !matches!(
                current,
                ObjectShroudStatus::Invalid | ObjectShroudStatus::InvalidButPreviousValid
            ) {
                if let Some(ghost) = self
                    .partition_manager
                    .fog_scene_mut()
                    .used_mut()
                    .iter_mut()
                    .find(|ghost| ghost.scene_id() == scene_id)
                {
                    ghost.set_previous_shroudedness(local, current as u8);
                }
            }
            if parent.is_none() && !self.fog_ghost_scene().linked_ghost_has_any_snapshot(scene_id) {
                self.partition_manager.remove_orphan_ghost(scene_id);
            }
        }
        if update_orphans {
            self.remove_snapshotless_host_orphans(local);
        }
        Ok(())
    }
}
