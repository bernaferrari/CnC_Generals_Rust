//! Session-owned C++ PartitionData / W3DGhostObject lifetime and fog transitions.
//! The renderer supplies completed native W3D state; this owner never guesses
//! a RenderObj from an INI name or consults the Core object/ghost registries.
use super::super::*;
use crate::assets::LiveDrawPlaybackIdentity;
use gamelogic::common::types::ObjectShroudStatus;
use gamelogic::object::w3d_ghost_object::{
    ParentGeometrySnapshot, RenderObjectState, W3DGhostSceneId, W3DGhostSnapshotCapture,
};
use std::collections::HashMap;

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

struct HostGhostLink {
    identity: LiveDrawPlaybackIdentity,
    scene_id: W3DGhostSceneId,
}

#[derive(Default)]
pub(crate) struct HostGhostState {
    links: HashMap<ObjectId, HostGhostLink>,
    draws: HashMap<ObjectId, CommittedHostGhostDraw>,
}

fn geometry(object: &Object) -> ParentGeometrySnapshot {
    let info = object.get_template().geometry_info;
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
                || !object.is_kind_of(KindOf::Immobile)
                || object.get_template().default_draw_without_fog_memory
            {
                continue;
            }
            if draw.render_objects.is_empty() {
                self.host_ghosts.draws.remove(&id);
            } else {
                self.host_ghosts.draws.insert(id, draw);
            }
        }
    }

    /// C++ attachToObject runs during actual admission, inserting at the
    /// manager list head. Template module metadata is already instance-owned.
    pub(crate) fn attach_host_fog_ghost(&mut self, id: ObjectId) {
        if self.host_ghosts.links.contains_key(&id) {
            return;
        }
        let Some(object) = self.objects.get(&id) else {
            return;
        };
        if !object.is_kind_of(KindOf::Immobile)
            || object.get_template().default_draw_without_fog_memory
        {
            return;
        }
        let identity = self.live_draw_playback_identity(object);
        if let Some(scene_id) = self.w3d_ghosts.add_linked_ghost_object(Some(id.0), true) {
            self.host_ghosts
                .links
                .insert(id, HostGhostLink { identity, scene_id });
        }
    }

    /// C++ unregisterObject detaches the parent but leaves its frozen ghost
    /// in the partition system until the old location becomes visible again.
    pub(crate) fn detach_host_fog_ghost(&mut self, id: ObjectId) {
        self.host_ghosts.draws.remove(&id);
        if let Some(link) = self.host_ghosts.links.remove(&id) {
            self.w3d_ghosts.orphan_linked_ghost(link.scene_id);
            if !self.w3d_ghosts.linked_ghost_has_any_snapshot(link.scene_id) {
                self.w3d_ghosts.remove_linked_ghost(link.scene_id);
            }
        }
    }

    /// C++ manager Xfer resolves each parent against the receiving GameLogic.
    /// Missing parents are standalone partition ghosts, never foreign objects
    /// with the same numeric ID. No render observation survives a load.
    pub(crate) fn restore_host_fog_ghost_links(&mut self) {
        self.host_ghosts = Default::default();
        let links: Vec<_> = self
            .w3d_ghosts
            .used()
            .iter()
            .filter_map(|ghost| {
                ghost
                    .parent_object_id()
                    .map(|id| (ObjectId(id), ghost.scene_id()))
            })
            .collect();
        for (id, scene_id) in links {
            if let Some(object) = self.objects.get(&id) {
                self.host_ghosts.links.insert(
                    id,
                    HostGhostLink {
                        identity: self.live_draw_playback_identity(object),
                        scene_id,
                    },
                );
            } else {
                self.w3d_ghosts.orphan_linked_ghost(scene_id);
            }
        }
    }

    /// C++ saved fog memory also restores the partition's previously seen
    /// premise. Main's shroud tail stores cells, not derived object caches;
    /// rebuild this fact after installing that tail, before the next COI mix.
    pub(crate) fn restore_host_fog_ghost_shroud_history(&self) -> Result<(), String> {
        let shroud = std::sync::Arc::clone(self.world_services.shroud());
        let mut shroud = shroud
            .lock()
            .map_err(|_| "session shroud poisoned during ghost restore".to_string())?;
        for ghost in self.w3d_ghosts.used() {
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
        self.apply_host_fog_ghost_transitions(shroud)
    }

    fn select_host_fog_ghost_viewer(&mut self, local: u32) {
        if self.w3d_ghosts.local_player_index() != local as usize {
            self.w3d_ghosts.set_local_player_index(local as usize);
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
        self.apply_host_fog_ghost_transitions(&shroud)
    }

    fn apply_host_fog_ghost_transitions(
        &mut self,
        shroud: &gamelogic::system::shroud_manager::ShroudManager,
    ) -> Result<(), String> {
        let stale: Vec<_> = self
            .w3d_ghosts
            .used()
            .iter()
            .filter_map(|ghost| {
                let id = ghost.parent_object_id().map(ObjectId)?;
                let link = self.host_ghosts.links.get(&id)?;
                let matching = self.objects.get(&id).is_some_and(|object| {
                    self.live_draw_playback_identity(object) == link.identity
                });
                (!matching).then_some(id)
            })
            .collect();
        for id in stale {
            self.detach_host_fog_ghost(id);
        }
        // Production admission attaches immediately in source order. This
        // canonical host ID roster also covers direct diagnostic insertions;
        // it never changes an already saved/installed manager list order.
        let mut eligible: Vec<_> = self.objects.keys().copied().collect();
        eligible.sort_unstable();
        for id in eligible {
            self.attach_host_fog_ghost(id);
        }

        let local = self.w3d_ghosts.local_player_index();
        // CPP getShroudedStatus returns INVALID before a partition grid
        // exists. A staged/cold world cannot release saved fog memory.
        let Some(origin) = shroud.grid_world_origin() else {
            return Ok(());
        };
        let Some((width, height, cell_size)) = shroud.grid_dimensions() else {
            return Ok(());
        };
        let dimensions = [width, height];
        let mut transitions = Vec::new();
        for ghost in self.w3d_ghosts.used() {
            let Some(id) = ghost.parent_object_id().map(ObjectId) else {
                continue;
            };
            if self
                .host_ghosts
                .links
                .get(&id)
                .is_some_and(|link| link.scene_id == ghost.scene_id())
            {
                if let Some(status) = shroud.get_host_object_shroud_status(local as u32, id.0) {
                    let Some(object) = self.objects.get(&id) else {
                        continue;
                    };
                    let position = object.get_position();
                    let cells =
                        crate::game_logic::partition_coi::cells_touched_for_footprint_on_grid(
                            origin,
                            dimensions,
                            cell_size,
                            position.x,
                            position.z,
                            crate::game_logic::game_logic::host_object_footprint(object),
                        )
                        .map_err(|reason| {
                            format!("host object {} fog footprint: {reason}", id.0)
                        })?;
                    let counts = shroud.count_footprint_cells(local as u32, &cells);
                    // CPP no-COI branch sets Shrouded but neither frees a
                    // snapshot nor advances previous (:1616, :1680).
                    if counts.total == 0 {
                        continue;
                    }
                    transitions.push((
                        ghost.scene_id(),
                        Some(id),
                        status,
                        counts.shrouded == counts.total,
                    ));
                }
            }
        }
        // Parentless ghosts retain their captured geometry, not the location
        // or visibility of a newly admitted actor with the same ObjectID.
        for ghost in self
            .w3d_ghosts
            .used()
            .iter()
            .filter(|ghost| ghost.parent_object_id().is_none())
        {
            let Some(info) = ghost.parent_geometry() else {
                continue;
            };
            let cells = crate::game_logic::partition_coi::cells_touched_for_footprint_on_grid(
                origin,
                dimensions,
                cell_size,
                info.position[0],
                info.position[1],
                crate::game_logic::partition_coi::HostPartitionFootprint {
                    major_radius: info.major_radius,
                    minor_radius: info.minor_radius,
                    angle: info.angle,
                    is_small: info.is_small,
                    is_box: info.geometry_type == 2,
                },
            )
            .map_err(|reason| format!("saved ghost fog footprint: {reason}"))?;
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
                status,
                counts.shrouded == counts.total,
            ));
        }
        // Preserve C++ manager head-to-tail order (including saved order).
        for (scene_id, parent, current, all_cells_shrouded) in transitions {
            let previous = self
                .w3d_ghosts
                .used()
                .iter()
                .find(|ghost| ghost.scene_id() == scene_id)
                .and_then(|ghost| ghost.previous_shroudedness(local));
            if current == ObjectShroudStatus::Fogged
                && parent.is_some()
                && previous.unwrap_or(ObjectShroudStatus::Invalid as u8)
                    < ObjectShroudStatus::Fogged as u8
            {
                let id = parent.unwrap();
                let Some(draw) = self.host_ghosts.draws.get(&id) else {
                    continue;
                };
                let Some(object) = self.objects.get(&id) else {
                    continue;
                };
                if draw.identity != self.live_draw_playback_identity(object) {
                    continue;
                }
                self.w3d_ghosts.snapshot_linked_ghost(
                    scene_id,
                    local,
                    &W3DGhostSnapshotCapture {
                        capture_window_generation: Some(draw.generation),
                        drawable_effectively_hidden: object.contained_by.is_some()
                            || (object.status.stealthed
                                && !object.status.detected
                                && !object.status.disguised),
                        render_objects: draw.render_objects.clone(),
                        geometry: geometry(object),
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
                self.w3d_ghosts.free_linked_ghost_snapshot(scene_id, local);
            }
            if !matches!(
                current,
                ObjectShroudStatus::Invalid | ObjectShroudStatus::InvalidButPreviousValid
            ) {
                if let Some(ghost) = self
                    .w3d_ghosts
                    .used_mut()
                    .iter_mut()
                    .find(|ghost| ghost.scene_id() == scene_id)
                {
                    ghost.set_previous_shroudedness(local, current as u8);
                }
            }
            if parent.is_none() && !self.w3d_ghosts.linked_ghost_has_any_snapshot(scene_id) {
                self.w3d_ghosts.remove_linked_ghost(scene_id);
            }
        }
        Ok(())
    }
}
