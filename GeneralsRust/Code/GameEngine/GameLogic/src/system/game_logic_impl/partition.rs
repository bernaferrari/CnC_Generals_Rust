/// Partition manager for spatial partitioning (matches C++ PartitionManager grid behavior)
#[derive(Debug, Clone)]
struct PartitionGhostLink {
    scene_id: crate::object::w3d_ghost_object::W3DGhostSceneId,
    parent_alive: bool,
    shroudedness_previous:
        [crate::common::ObjectShroudStatus; game_engine::common::game_common::MAX_PLAYER_COUNT],
    /// Frozen parent pose used after `unRegisterObject` orphans the ghost.
    /// C++ keeps partition cells filled from this geometry (W3DGhostObject.cpp:363-368).
    frozen_position: Option<Coord3D>,
}

#[derive(Debug)]
pub struct PartitionManager {
    grid: HashMap<(i32, i32), Vec<ObjectID>>,
    object_cells: HashMap<ObjectID, (i32, i32)>,
    object_positions: HashMap<ObjectID, Coord3D>,
    ghost_links: HashMap<ObjectID, PartitionGhostLink>,
    cell_size: Real,
    /// C++ PartitionCell shroud levels (40wu).
    shroud: crate::object::collide::partition_shroud::PartitionShroudGrid,
    /// C++ `storeFoggedCells` / `restoreFoggedCells` snapshot, keyed by
    /// `(player_index, store_to_fog)`.
    fogged_cells: HashMap<(usize, bool), HashSet<(i32, i32)>>,
}

impl PartitionManager {
    pub fn new() -> Self {
        Self {
            grid: HashMap::new(),
            object_cells: HashMap::new(),
            object_positions: HashMap::new(),
            ghost_links: HashMap::new(),
            cell_size: 40.0,
            shroud: crate::object::collide::partition_shroud::PartitionShroudGrid::new(),
            fogged_cells: HashMap::new(),
        }
    }

    /// C++ PartitionManager::crc walks every cell. Host grid is sparse: dump
    /// cell size plus each occupied cell's sorted occupant IDs.
    fn crc_into(&self, xfer: &mut dyn Xfer) {
        let mut cell_size = self.cell_size;
        let _ = xfer.xfer_real(&mut cell_size);
        let mut total = self.object_cells.len() as i32;
        let _ = xfer.xfer_int(&mut total);
        let mut cells: Vec<(i32, i32, ObjectID)> = self
            .object_cells
            .iter()
            .map(|(&id, &(x, y))| (x, y, id))
            .collect();
        cells.sort_unstable();
        for (mut x, mut y, mut id) in cells {
            let _ = xfer.xfer_int(&mut x);
            let _ = xfer.xfer_int(&mut y);
            let _ = xfer.xfer_unsigned_int(&mut id);
        }
    }

    /// Find objects within radius of position (2D X/Y distance).
    pub fn find_objects_in_radius(&self, center: Coord3D, radius: Real) -> Vec<ObjectID> {
        let mut result = Vec::new();
        let radius_squared = radius * radius;

        let min_cell =
            self.position_to_cell([center.x - radius, center.y - radius, center.z].into());
        let max_cell =
            self.position_to_cell([center.x + radius, center.y + radius, center.z].into());

        for x in min_cell.0..=max_cell.0 {
            for y in min_cell.1..=max_cell.1 {
                if let Some(objects) = self.grid.get(&(x, y)) {
                    for &object_id in objects {
                        let Some(pos) = self.object_positions.get(&object_id) else {
                            continue;
                        };
                        let dx = pos.x - center.x;
                        let dy = pos.y - center.y;
                        if dx * dx + dy * dy <= radius_squared {
                            result.push(object_id);
                        }
                    }
                }
            }
        }

        result
    }

    pub fn update(&mut self) -> Result<(), GameLogicError> {
        // Host path: OBJECT_REGISTRY is empty; cells were filled via register_object.
        // Keep cached positions — C++ PartitionManager still tracks live objects.
        if dual_world_registry_unavailable() {
            return Ok(());
        }
        let object_ids = OBJECT_REGISTRY.get_all_object_ids();
        let mut seen = HashSet::with_capacity(object_ids.len());

        for obj_id in &object_ids {
            let Some(obj_arc) = OBJECT_REGISTRY.get_object(*obj_id) else {
                continue;
            };
            let Ok(obj) = obj_arc.read() else {
                continue;
            };
            let id = obj.get_id();
            let pos = obj.get_position();
            self.add_object(id, (pos.x, pos.y, pos.z));
            seen.insert(id);
        }

        let stale: Vec<ObjectID> = self
            .object_positions
            .keys()
            .copied()
            .filter(|id| !seen.contains(id))
            .collect();
        for id in stale {
            self.remove_object(id);
        }
        Ok(())
    }

    pub fn add_object(&mut self, object_id: ObjectID, position: (f32, f32, f32)) {
        let pos = Coord3D::new(position.0, position.1, position.2);
        let cell = self.position_to_cell(pos);

        if let Some(old_cell) = self.object_cells.get(&object_id) {
            if let Some(objects) = self.grid.get_mut(old_cell) {
                objects.retain(|&id| id != object_id);
            }
        }

        self.grid
            .entry(cell)
            .or_insert_with(Vec::new)
            .push(object_id);
        self.object_cells.insert(object_id, cell);
        self.object_positions.insert(object_id, pos);
    }

    pub fn remove_object(&mut self, object_id: ObjectID) {
        self.object_positions.remove(&object_id);
        if let Some(cell) = self.object_cells.remove(&object_id) {
            if let Some(objects) = self.grid.get_mut(&cell) {
                objects.retain(|&id| id != object_id);
                if objects.is_empty() {
                    self.grid.remove(&cell);
                }
            }
        }
    }

    /// C++ `PartitionData::attachToObject` ghost allocation. Eligibility is
    /// computed by the caller from the immutable ThingTemplate contract.
    pub fn attach_object_ghost(
        &mut self,
        object_id: ObjectID,
        eligible: bool,
        manager: &mut crate::object::W3DGhostObjectManager,
    ) -> Option<crate::object::w3d_ghost_object::W3DGhostSceneId> {
        if !eligible {
            return None;
        }
        if let Some(link) = self.ghost_links.get(&object_id) {
            return Some(link.scene_id);
        }
        let scene_id = manager.add_linked_ghost_object(Some(object_id), true)?;
        self.ghost_links.insert(
            object_id,
            PartitionGhostLink {
                scene_id,
                parent_alive: true,
                shroudedness_previous: [crate::common::ObjectShroudStatus::Invalid;
                    game_engine::common::game_common::MAX_PLAYER_COUNT],
                frozen_position: self.object_positions.get(&object_id).copied(),
            },
        );
        Some(scene_id)
    }

    /// C++ `PartitionData::getShroudedStatus` transition side effects. The
    /// caller supplies the already-resolved object status and exact optional
    /// W3D capture; this layer never reconstructs render state.
    pub fn apply_object_ghost_shroud_status(
        &mut self,
        object_id: ObjectID,
        player_index: usize,
        current: crate::common::ObjectShroudStatus,
        capture: Option<&crate::object::w3d_ghost_object::W3DGhostSnapshotCapture>,
        manager: &mut crate::object::W3DGhostObjectManager,
    ) -> bool {
        if player_index >= game_engine::common::game_common::MAX_PLAYER_COUNT {
            return false;
        }
        let Some(link) = self.ghost_links.get_mut(&object_id) else {
            return false;
        };
        let previous = link.shroudedness_previous[player_index];

        match current {
            crate::common::ObjectShroudStatus::Fogged
                if link.parent_alive
                    && (previous as u8) < (crate::common::ObjectShroudStatus::Fogged as u8) =>
            {
                // Fail-closed: a missing capture must not advance previous status.
                // C++ snapShot is the only transition that freezes fog memory;
                // retry next frame until the renderer-owned hook succeeds.
                let Some(capture) = capture else {
                    return true;
                };
                link.frozen_position = Some(Coord3D::new(
                    capture.geometry.position[0],
                    capture.geometry.position[1],
                    capture.geometry.position[2],
                ));
                manager.snapshot_linked_ghost(link.scene_id, player_index, capture);
            }
            crate::common::ObjectShroudStatus::Clear
            | crate::common::ObjectShroudStatus::PartialClear
            | crate::common::ObjectShroudStatus::Shrouded
                if previous == crate::common::ObjectShroudStatus::Fogged =>
            {
                manager.free_linked_ghost_snapshot(link.scene_id, player_index);
            }
            _ => {}
        }

        if !matches!(
            current,
            crate::common::ObjectShroudStatus::Invalid
                | crate::common::ObjectShroudStatus::InvalidButPreviousValid
        ) {
            link.shroudedness_previous[player_index] = current;
        }

        if !link.parent_alive && !manager.linked_ghost_has_any_snapshot(link.scene_id) {
            let scene_id = link.scene_id;
            manager.remove_linked_ghost(scene_id);
            self.ghost_links.remove(&object_id);
        }
        true
    }

    /// C++ `PartitionManager::unRegisterObject`: retain a fog-memory module as
    /// an orphan, otherwise return it to the manager free store immediately.
    pub fn detach_object_ghost(
        &mut self,
        object_id: ObjectID,
        manager: &mut crate::object::W3DGhostObjectManager,
    ) -> bool {
        let Some(link) = self.ghost_links.get_mut(&object_id) else {
            return false;
        };
        if manager.linked_ghost_has_any_snapshot(link.scene_id) {
            link.parent_alive = false;
            manager.orphan_linked_ghost(link.scene_id);
        } else {
            let scene_id = link.scene_id;
            manager.remove_linked_ghost(scene_id);
            self.ghost_links.remove(&object_id);
        }
        true
    }

    pub fn ghost_link_scene_id(
        &self,
        object_id: ObjectID,
    ) -> Option<crate::object::w3d_ghost_object::W3DGhostSceneId> {
        self.ghost_links.get(&object_id).map(|link| link.scene_id)
    }

    pub fn object_ghost_needs_capture(
        &self,
        object_id: ObjectID,
        player_index: usize,
        current: crate::common::ObjectShroudStatus,
    ) -> bool {
        self.ghost_links.get(&object_id).is_some_and(|link| {
            link.parent_alive
                && player_index < game_engine::common::game_common::MAX_PLAYER_COUNT
                && current == crate::common::ObjectShroudStatus::Fogged
                && (link.shroudedness_previous[player_index] as u8)
                    < (crate::common::ObjectShroudStatus::Fogged as u8)
        })
    }

    pub fn ghost_link_object_ids(&self) -> Vec<ObjectID> {
        self.ghost_links.keys().copied().collect()
    }

    pub fn orphan_ghost_object_ids(&self) -> Vec<ObjectID> {
        self.ghost_links
            .iter()
            .filter(|(_, link)| !link.parent_alive)
            .map(|(&id, _)| id)
            .collect()
    }

    pub fn ghost_shroudedness_previous(
        &self,
        object_id: ObjectID,
        player_index: usize,
    ) -> Option<crate::common::ObjectShroudStatus> {
        self.ghost_links.get(&object_id).and_then(|link| {
            link.shroudedness_previous
                .get(player_index)
                .copied()
        })
    }

    pub fn ghost_frozen_position(&self, object_id: ObjectID) -> Option<Coord3D> {
        self.ghost_links
            .get(&object_id)
            .and_then(|link| link.frozen_position)
            .or_else(|| self.object_positions.get(&object_id).copied())
    }

    /// C++ `W3DGhostObject::getShroudStatus` for parentless modules, then
    /// `W3DGhostObjectManager::updateOrphanedObjects`.
    pub fn update_orphaned_ghosts(
        &mut self,
        player_index: usize,
        current_status: impl Fn(ObjectID) -> crate::common::ObjectShroudStatus,
        manager: &mut crate::object::W3DGhostObjectManager,
    ) {
        let orphans = self.orphan_ghost_object_ids();
        for object_id in orphans {
            let status = current_status(object_id);
            self.apply_object_ghost_shroud_status(
                object_id,
                player_index,
                status,
                None,
                manager,
            );
        }
        manager.update_orphaned_objects(&[]);
    }

    pub fn clear_ghost_objects(&mut self, manager: &mut crate::object::W3DGhostObjectManager) {
        for link in self.ghost_links.values() {
            manager.remove_linked_ghost(link.scene_id);
        }
        self.ghost_links.clear();
    }

    /// Rebuild the spatial partition index
    /// Used after loading a saved game to reconstruct spatial data
    pub fn rebuild(&mut self) {
        if dual_world_registry_unavailable() {
            // Re-grid from cached positions instead of wiping the host-path index.
            let snapshot: Vec<(ObjectID, Coord3D)> = self
                .object_positions
                .iter()
                .map(|(id, pos)| (*id, *pos))
                .collect();
            self.grid.clear();
            self.object_cells.clear();
            self.object_positions.clear();
            for (id, pos) in snapshot {
                self.add_object(id, (pos.x, pos.y, pos.z));
            }
            return;
        }

        self.grid.clear();
        self.object_cells.clear();
        self.object_positions.clear();

        for obj_id in OBJECT_REGISTRY.get_all_object_ids() {
            let Some((id, xyz)) = OBJECT_REGISTRY.with_object(obj_id, |obj| {
                let pos = obj.get_position();
                (obj.get_id(), (pos.x, pos.y, pos.z))
            }) else {
                continue;
            };
            self.add_object(id, xyz);
        }
    }

    /// Register an object at a specific position
    /// Used during save game restoration
    pub fn register_object(&mut self, object_id: ObjectID, x: f32, y: f32) {
        self.add_object(object_id, (x, y, 0.0));
    }

    fn position_to_cell(&self, position: Coord3D) -> (i32, i32) {
        let x = (position.x / self.cell_size).floor() as i32;
        let y = (position.y / self.cell_size).floor() as i32;
        (x, y)
    }

    pub fn cell_size(&self) -> Real {
        self.cell_size
    }

    /// C++ `getShroudStatusForPlayer` on the 40wu closest-object grid.
    pub fn get_shroud_status_for_player(
        &self,
        player_index: i32,
        loc: &Coord3D,
    ) -> game_engine::common::system::radar::CellShroudStatus {
        self.shroud.status_at_world(player_index, loc)
    }

    /// C++ `getShroudStatusForPlayer(player, x, y)`.
    pub fn get_shroud_status_for_player_cell(
        &self,
        player_index: i32,
        x: i32,
        y: i32,
    ) -> game_engine::common::system::radar::CellShroudStatus {
        self.shroud.cell_status(player_index, x, y)
    }

    /// C++ `getPropShroudStatusForPlayer`.
    pub fn get_prop_shroud_status_for_player(
        &self,
        player_index: i32,
        loc: &Coord3D,
    ) -> crate::common::ObjectShroudStatus {
        self.shroud.prop_status(player_index, loc)
    }

    pub fn do_shroud_reveal(&mut self, center: &Coord3D, radius: Real, player_mask: u32) {
        self.shroud.reveal_circle(center, radius, player_mask);
    }

    pub fn undo_shroud_reveal(&mut self, center: &Coord3D, radius: Real, player_mask: u32) {
        self.shroud.undo_reveal_circle(center, radius, player_mask);
    }

    pub fn add_looker(&mut self, player_index: i32, x: i32, y: i32) {
        self.shroud.add_looker(player_index, x, y);
    }

    pub fn remove_looker(&mut self, player_index: i32, x: i32, y: i32) {
        self.shroud.remove_looker(player_index, x, y);
    }

    /// C++ `PartitionManager::storeFoggedCells`.
    pub fn store_fogged_cells(&mut self, player_index: usize, store_to_fog: bool) {
        let player = player_index as i32;
        let mut cells = HashSet::new();
        for &(x, y) in self.grid.keys() {
            match self.shroud.cell_status(player, x, y) {
                game_engine::common::system::radar::CellShroudStatus::Fogged if store_to_fog => {
                    cells.insert((x, y));
                }
                game_engine::common::system::radar::CellShroudStatus::Clear if !store_to_fog => {
                    cells.insert((x, y));
                }
                _ => {}
            }
        }
        self.fogged_cells
            .insert((player_index, store_to_fog), cells);
    }

    /// C++ `PartitionManager::restoreFoggedCells`.
    pub fn restore_fogged_cells(&mut self, player_index: usize, restore_to_fog: bool) {
        let Some(cells) = self.fogged_cells.get(&(player_index, restore_to_fog)).cloned() else {
            return;
        };
        let player = player_index as i32;
        for (x, y) in cells {
            self.shroud.add_looker(player, x, y);
            if restore_to_fog {
                self.shroud.remove_looker(player, x, y);
            }
        }
    }

    /// C++ `PartitionManager::clear` — drop all registered occupants so a
    /// map-boundary change can re-register objects on the new grid.
    pub fn clear(&mut self) {
        self.grid.clear();
        self.object_cells.clear();
        self.object_positions.clear();
        self.ghost_links.clear();
        self.shroud.clear();
    }
}

impl Default for PartitionManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod ghost_transition_tests {
    use super::PartitionManager;
    use crate::common::ObjectShroudStatus;
    use crate::object::w3d_ghost_object::{
        Matrix3x4, ParentGeometrySnapshot, RenderObjectClass, RenderObjectState,
        W3DGhostObjectManager, W3DGhostSnapshotCapture,
    };

    fn capture(name: &str) -> W3DGhostSnapshotCapture {
        W3DGhostSnapshotCapture {
            capture_window_generation: None,
            drawable_effectively_hidden: false,
            render_objects: vec![RenderObjectState {
                name: name.to_string(),
                scale: 1.0,
                color: 0xffff_ffff,
                transform: Matrix3x4::IDENTITY,
                sub_objects: Vec::new(),
                class_id: RenderObjectClass::Mesh,
            }],
            geometry: ParentGeometrySnapshot {
                geometry_type: 2,
                is_small: false,
                major_radius: 10.0,
                minor_radius: 5.0,
                position: [40.0, 50.0, 0.0],
                angle: 0.0,
            },
        }
    }

    #[test]
    fn capture_failure_does_not_advance_previous_status() {
        let mut partition = PartitionManager::new();
        let mut manager = W3DGhostObjectManager::new();
        partition.add_object(7, (40.0, 50.0, 0.0));
        let scene_id = partition
            .attach_object_ghost(7, true, &mut manager)
            .expect("ghost link");

        partition.apply_object_ghost_shroud_status(
            7,
            0,
            ObjectShroudStatus::Fogged,
            None,
            &mut manager,
        );

        assert_eq!(
            partition.ghost_shroudedness_previous(7, 0),
            Some(ObjectShroudStatus::Invalid)
        );
        assert!(!manager.linked_ghost_has_any_snapshot(scene_id));
        assert!(partition.object_ghost_needs_capture(7, 0, ObjectShroudStatus::Fogged));

        partition.apply_object_ghost_shroud_status(
            7,
            0,
            ObjectShroudStatus::Fogged,
            Some(&capture("Tower")),
            &mut manager,
        );
        assert_eq!(
            partition.ghost_shroudedness_previous(7, 0),
            Some(ObjectShroudStatus::Fogged)
        );
        assert!(manager.linked_ghost_has_any_snapshot(scene_id));
    }

    #[test]
    fn orphan_is_removed_when_shroud_clears() {
        let mut partition = PartitionManager::new();
        let mut manager = W3DGhostObjectManager::new();
        partition.add_object(8, (40.0, 50.0, 0.0));
        let scene_id = partition
            .attach_object_ghost(8, true, &mut manager)
            .expect("ghost link");
        partition.apply_object_ghost_shroud_status(
            8,
            0,
            ObjectShroudStatus::Fogged,
            Some(&capture("Ruins")),
            &mut manager,
        );
        assert!(partition.detach_object_ghost(8, &mut manager));
        assert_eq!(partition.ghost_link_scene_id(8), Some(scene_id));
        assert_eq!(manager.used_count(), 1);

        partition.update_orphaned_ghosts(0, |_| ObjectShroudStatus::Clear, &mut manager);

        assert_eq!(partition.ghost_link_scene_id(8), None);
        assert_eq!(manager.used_count(), 0);
    }
}
