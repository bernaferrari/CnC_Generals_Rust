//! W3D fog memory follows PartitionData registration. Retained modules keep
//! their footprint after the live Object has left the collision index.

use super::{PartitionManager, PartitionObject};
use crate::game_logic::partition_coi::HostPartitionFootprint;
use crate::game_logic::{HostGeometryType, KindOf, Object};
use gamelogic::object::w3d_ghost_object::{W3DGhostObjectManager, W3DGhostSceneId};

impl PartitionManager {
    pub(in crate::game_logic) fn fog_scene(&self) -> &W3DGhostObjectManager {
        &self.ghost_scene
    }

    pub(in crate::game_logic) fn fog_scene_mut(&mut self) -> &mut W3DGhostObjectManager {
        &mut self.ghost_scene
    }

    pub(in crate::game_logic) fn object_ghost(&self, id: u32) -> Option<W3DGhostSceneId> {
        self.object_cells.get(&id)?.ghost
    }

    /// PartitionData::attachToObject creates its ghost before the first update.
    pub(super) fn attach_object_ghost(&mut self, object: &Object) {
        if object.is_kind_of(KindOf::Immobile)
            && !object.get_template().default_draw_without_fog_memory
        {
            let id = object.id.0;
            let ghost = self.ghost_scene.add_linked_ghost_object(Some(id), true);
            self.object_cells.get_mut(&id).expect("fresh PartitionData").ghost = ghost;
        }
    }

    pub(super) fn release_object_ghost(&mut self, id: u32, mut module: PartitionObject) {
        let retain = self.retire_object_shroud_history(id, module.ghost.is_some());
        if let Some(ghost) = module.ghost {
            if retain {
                self.ghost_scene.orphan_linked_ghost(ghost);
                module.birth = None;
                self.ghost_cells.insert(ghost, module);
            } else {
                self.ghost_scene.remove_linked_ghost(ghost);
            }
        }
    }

    pub(in crate::game_logic) fn ghost_partition_cells(
        &self,
        ghost: W3DGhostSceneId,
    ) -> Option<&[(i32, i32)]> {
        self.ghost_cells.get(&ghost).map(|module| module.cells.as_slice())
    }

    pub(in crate::game_logic) fn remove_orphan_ghost(&mut self, ghost: W3DGhostSceneId) {
        self.ghost_cells.remove(&ghost);
        self.ghost_scene.remove_linked_ghost(ghost);
    }

    /// W3DGhostObjectManager::releasePartitionData preserves the scene across
    /// a boundary change while freeing both kinds of grid-relative linkage.
    pub(in crate::game_logic) fn release_ghost_partition_links(&mut self) {
        for ghost in self.ghost_scene.used() {
            if let Some(parent) = ghost.parent_object_id() {
                if let Some(module) = self.object_cells.get_mut(&parent) {
                    module.ghost = None;
                }
            } else {
                self.ghost_cells.remove(&ghost.scene_id());
            }
        }
        self.ghost_scene.release_partition_data();
    }

    /// After boundary or save restore, link existing scene IDs to the
    /// receiving modules and rebuild orphan COIs from their frozen geometry.
    pub(in crate::game_logic) fn restore_ghost_partition_links(
        &mut self,
        live_parent: impl Fn(u32) -> bool,
    ) {
        for module in self.object_cells.values_mut() {
            module.ghost = None;
        }
        self.ghost_cells.clear();
        let scene: Vec<_> = self.ghost_scene.used().iter().map(|ghost| {
            (ghost.scene_id(), ghost.parent_object_id(), ghost.parent_geometry())
        }).collect();
        for (scene_id, parent, geometry) in scene {
            if let Some(module) = parent
                .filter(|&id| live_parent(id))
                .and_then(|id| self.object_cells.get_mut(&id))
            {
                module.ghost = Some(scene_id);
                continue;
            }
            if parent.is_some() {
                self.ghost_scene.orphan_linked_ghost(scene_id);
            }
            if let Some(info) = geometry {
                let footprint = HostPartitionFootprint {
                    major_radius: info.major_radius,
                    minor_radius: info.minor_radius,
                    angle: info.angle,
                    is_small: info.is_small,
                    is_box: info.geometry_type == HostGeometryType::Box as u32,
                };
                let cells = self.footprint_cells(info.position[0], info.position[1], footprint);
                self.ghost_cells.insert(scene_id, PartitionObject { cells, ..Default::default() });
            }
        }
        self.ghost_scene.restore_partition_data();
        self.ghost_scene.set_lock_ghost_objects(false);
    }
}
