/// Partition manager for spatial partitioning (matches C++ PartitionManager grid behavior)
#[derive(Debug, Clone)]
struct PartitionGhostLink {
    scene_id: crate::object::w3d_ghost_object::W3DGhostSceneId,
    shroudedness_previous:
        [crate::common::ObjectShroudStatus; game_engine::common::game_common::MAX_PLAYER_COUNT],
    /// Frozen parent pose used after `unRegisterObject` orphans the ghost.
    /// C++ keeps partition cells filled from this geometry (W3DGhostObject.cpp:363-368).
    frozen_position: Option<Coord3D>,
}

/// Host-side object index and ghost-link state of THE world partition manager.
///
/// C++ has exactly one `ThePartitionManager`; its object index, ghost
/// (`PartitionData` fog memory) links, cell shroud and CRC all live in it. The
/// Rust port used to keep this half in a second per-`GameLogic` manager. It is
/// now a field (`PartitionManager::host`) of the single world manager, which also
/// owns the shroud grid, fogged-cell snapshots and `m_updatedSinceLastReset`.
/// The centre-point index below still feeds `get_objects_in_range`; it is
/// replaced by the COI-cell index once that query migrates.
#[derive(Debug)]
pub struct HostObjectIndex {
    grid: HashMap<(i32, i32), Vec<ObjectID>>,
    object_cells: HashMap<ObjectID, (i32, i32)>,
    object_positions: HashMap<ObjectID, Coord3D>,
    /// Live `PartitionData` ghosts, owned by their registered object.
    ghost_links: HashMap<ObjectID, PartitionGhostLink>,
    /// C++ orphaned `PartitionData` (PartitionManager.cpp:2836-2851): once
    /// unregistered the module belongs to the ghost, not the object ID, so a
    /// later object reusing that ID gets its own link.
    orphan_links: HashMap<crate::object::w3d_ghost_object::W3DGhostSceneId, PartitionGhostLink>,
    pub(crate) cell_size: Real,
}

impl HostObjectIndex {
    pub fn new() -> Self {
        Self {
            grid: HashMap::new(),
            object_cells: HashMap::new(),
            object_positions: HashMap::new(),
            ghost_links: HashMap::new(),
            orphan_links: HashMap::new(),
            cell_size: 40.0,
        }
    }

    /// C++ PartitionManager::crc walks every cell. Host grid is sparse: dump
    /// cell size plus each occupied cell's sorted occupant IDs.
    pub(crate) fn crc_into(&self, xfer: &mut dyn Xfer) {
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
        self.sync_positions(
            OBJECT_REGISTRY.get_all_object_ids().into_iter().filter_map(|id| {
                OBJECT_REGISTRY.with_object(id, |obj| (obj.get_id(), *obj.get_position()))
            }),
        );
        Ok(())
    }

    fn sync_positions(&mut self, positions: impl Iterator<Item = (ObjectID, Coord3D)>) {
        let (minimum, maximum) = positions.size_hint();
        let mut seen = HashSet::with_capacity(maximum.unwrap_or(minimum));
        for (id, pos) in positions {
            self.add_object(id, (pos.x, pos.y, pos.z));
            seen.insert(id);
        }
        let stale: Vec<_> = self
            .object_positions
            .keys()
            .copied()
            .filter(|id| !seen.contains(id))
            .collect();
        for id in stale {
            self.remove_object(id);
        }
    }

    fn replace_positions(&mut self, positions: impl Iterator<Item = (ObjectID, Coord3D)>) {
        self.grid.clear();
        self.object_cells.clear();
        self.object_positions.clear();
        for (id, pos) in positions {
            self.add_object(id, (pos.x, pos.y, pos.z));
        }
    }

    /// Whether this partition already holds the Object (C++
    /// `Object::friend_getPartitionData() != NULL`).
    pub fn contains_object(&self, object_id: ObjectID) -> bool {
        self.object_cells.contains_key(&object_id)
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

    pub(crate) fn rekey_restored_birth(
        &mut self,
        provisional: ObjectID,
        saved: ObjectID,
        ghosts: &mut crate::object::W3DGhostObjectManager,
    ) {
        if let Some(cell) = self.object_cells.remove(&provisional) {
            if let Some(bucket) = self.grid.get_mut(&cell) {
                if let Some(id) = bucket.iter_mut().find(|id| **id == provisional) {
                    *id = saved;
                }
            }
            self.object_cells.insert(saved, cell);
        }
        if let Some(position) = self.object_positions.remove(&provisional) {
            self.object_positions.insert(saved, position);
        }
        if let Some(link) = self.ghost_links.remove(&provisional) {
            ghosts.rekey_restored_parent(link.scene_id, saved);
            self.ghost_links.insert(saved, link);
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
        transition_ghost_link(link, true, player_index, current, capture, manager);
        true
    }

    /// C++ `PartitionManager::unRegisterObject`: retain a fog-memory module as
    /// an orphan, otherwise return it to the manager free store immediately.
    pub fn detach_object_ghost(
        &mut self,
        object_id: ObjectID,
        manager: &mut crate::object::W3DGhostObjectManager,
    ) -> bool {
        let Some(link) = self.ghost_links.remove(&object_id) else {
            return false;
        };
        if manager.linked_ghost_has_any_snapshot(link.scene_id) {
            manager.orphan_linked_ghost(link.scene_id);
            self.orphan_links.insert(link.scene_id, link);
        } else {
            manager.remove_linked_ghost(link.scene_id);
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
            player_index < game_engine::common::game_common::MAX_PLAYER_COUNT
                && current == crate::common::ObjectShroudStatus::Fogged
                && (link.shroudedness_previous[player_index] as u8)
                    < (crate::common::ObjectShroudStatus::Fogged as u8)
        })
    }

    pub fn ghost_link_object_ids(&self) -> Vec<ObjectID> {
        self.ghost_links.keys().copied().collect()
    }

    pub fn orphan_ghost_scene_ids(&self) -> Vec<crate::object::w3d_ghost_object::W3DGhostSceneId> {
        self.orphan_links.keys().copied().collect()
    }

    pub fn orphan_ghost_frozen_position(
        &self,
        scene_id: crate::object::w3d_ghost_object::W3DGhostSceneId,
    ) -> Option<Coord3D> {
        self.orphan_links
            .get(&scene_id)
            .and_then(|link| link.frozen_position)
    }

    pub fn ghost_shroudedness_previous(
        &self,
        object_id: ObjectID,
        player_index: usize,
    ) -> Option<crate::common::ObjectShroudStatus> {
        self.ghost_links
            .get(&object_id)
            .and_then(|link| link.shroudedness_previous.get(player_index).copied())
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
        current_status: impl Fn(
            crate::object::w3d_ghost_object::W3DGhostSceneId,
        ) -> crate::common::ObjectShroudStatus,
        manager: &mut crate::object::W3DGhostObjectManager,
    ) {
        if player_index < game_engine::common::game_common::MAX_PLAYER_COUNT {
            self.orphan_links.retain(|&scene_id, link| {
                transition_ghost_link(
                    link,
                    false,
                    player_index,
                    current_status(scene_id),
                    None,
                    manager,
                );
                if manager.linked_ghost_has_any_snapshot(scene_id) {
                    return true;
                }
                manager.remove_linked_ghost(scene_id);
                false
            });
        }
        manager.update_orphaned_objects(&[]);
    }

    pub fn clear_ghost_objects(&mut self, manager: &mut crate::object::W3DGhostObjectManager) {
        for link in self.ghost_links.values().chain(self.orphan_links.values()) {
            manager.remove_linked_ghost(link.scene_id);
        }
        self.ghost_links.clear();
        self.orphan_links.clear();
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
            self.replace_positions(snapshot.into_iter());
            return;
        }

        self.replace_positions(
            OBJECT_REGISTRY.get_all_object_ids().into_iter().filter_map(|id| {
                OBJECT_REGISTRY.with_object(id, |obj| (obj.get_id(), *obj.get_position()))
            }),
        );
    }

    /// Register an object at a specific position
    /// Used during save game restoration
    pub fn register_object(&mut self, object_id: ObjectID, x: f32, y: f32) {
        self.add_object(object_id, (x, y, 0.0));
    }

    /// C++ `worldToCell` (PartitionManager.h:1506-1510, cpp:2571).
    fn position_to_cell(&self, position: Coord3D) -> (i32, i32) {
        let inv = (1.0_f64 / f64::from(self.cell_size)) as f32;
        (
            game_engine::base_type::real_to_int_floor(position.x * inv),
            game_engine::base_type::real_to_int_floor(position.y * inv),
        )
    }

    pub fn cell_size(&self) -> Real {
        self.cell_size
    }

    /// C++ `PartitionManager::clear` — drop all registered occupants so a
    /// map-boundary change can re-register objects on the new grid.
    pub fn clear(&mut self) {
        self.grid.clear();
        self.object_cells.clear();
        self.object_positions.clear();
        self.ghost_links.clear();
        self.orphan_links.clear();
    }
}

/// C++ `PartitionData::getShroudedStatus` transition side effects shared by
/// live links and orphaned modules (an orphan has no parent to snapshot).
fn transition_ghost_link(
    link: &mut PartitionGhostLink,
    has_parent: bool,
    player_index: usize,
    current: crate::common::ObjectShroudStatus,
    capture: Option<&crate::object::w3d_ghost_object::W3DGhostSnapshotCapture>,
    manager: &mut crate::object::W3DGhostObjectManager,
) {
    let previous = link.shroudedness_previous[player_index];
    match current {
        crate::common::ObjectShroudStatus::Fogged
            if has_parent
                && (previous as u8) < (crate::common::ObjectShroudStatus::Fogged as u8) =>
        {
            // Fail-closed: a missing capture must not advance previous status.
            // C++ snapShot is the only transition that freezes fog memory;
            // retry next frame until the renderer-owned hook succeeds.
            let Some(capture) = capture else {
                return;
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
}

impl Default for HostObjectIndex {
    fn default() -> Self {
        Self::new()
    }
}

pub use crate::object::collide::partition_manager::PartitionManager;

macro_rules! forward_host {
    (ref; $( fn $name:ident($($arg:ident: $ty:ty),*) $(-> $ret:ty)?; )*) => {
        impl PartitionManager {
            $(
                pub fn $name(&self $(, $arg: $ty)*) $(-> $ret)? {
                    self.host.$name($($arg),*)
                }
            )*
        }
    };
    (mutable; $( fn $name:ident($($arg:ident: $ty:ty),*) $(-> $ret:ty)?; )*) => {
        impl PartitionManager {
            $(
                pub fn $name(&mut self $(, $arg: $ty)*) $(-> $ret)? {
                    self.host.$name($($arg),*)
                }
            )*
        }
    };
}

forward_host! { ref;
    fn contains_object(object_id: ObjectID) -> bool;
    fn cell_size() -> Real;
    fn ghost_link_scene_id(object_id: ObjectID) -> Option<crate::object::w3d_ghost_object::W3DGhostSceneId>;
    fn object_ghost_needs_capture(object_id: ObjectID, player_index: usize, current: crate::common::ObjectShroudStatus) -> bool;
    fn ghost_link_object_ids() -> Vec<ObjectID>;
    fn orphan_ghost_scene_ids() -> Vec<crate::object::w3d_ghost_object::W3DGhostSceneId>;
    fn orphan_ghost_frozen_position(scene_id: crate::object::w3d_ghost_object::W3DGhostSceneId) -> Option<Coord3D>;
    fn ghost_shroudedness_previous(object_id: ObjectID, player_index: usize) -> Option<crate::common::ObjectShroudStatus>;
    fn ghost_frozen_position(object_id: ObjectID) -> Option<Coord3D>;
}

forward_host! { mutable;
    fn add_object(object_id: ObjectID, position: (f32, f32, f32));
    fn remove_object(object_id: ObjectID);
    fn rebuild();
    fn attach_object_ghost(object_id: ObjectID, eligible: bool, manager: &mut crate::object::W3DGhostObjectManager) -> Option<crate::object::w3d_ghost_object::W3DGhostSceneId>;
    fn apply_object_ghost_shroud_status(object_id: ObjectID, player_index: usize, current: crate::common::ObjectShroudStatus, capture: Option<&crate::object::w3d_ghost_object::W3DGhostSnapshotCapture>, manager: &mut crate::object::W3DGhostObjectManager) -> bool;
    fn detach_object_ghost(object_id: ObjectID, manager: &mut crate::object::W3DGhostObjectManager) -> bool;
    fn clear_ghost_objects(manager: &mut crate::object::W3DGhostObjectManager);
}

impl PartitionManager {
    /// Registers an object's position in the centre-point index
    /// (save-game restore path; the COI registration is `register_object`).
    pub fn register_object_at(&mut self, object_id: ObjectID, x: f32, y: f32) {
        self.host.register_object(object_id, x, y);
    }

    /// Centre-point radius query that feeds `get_objects_in_range`.
    pub fn find_objects_in_radius_by_center(&self, center: Coord3D, radius: Real) -> Vec<ObjectID> {
        self.host.find_objects_in_radius(center, radius)
    }

    /// C++ `ThePartitionManager->update()`: always sets `m_updatedSinceLastReset`.
    pub fn update_host_index(&mut self) -> Result<(), GameLogicError> {
        self.mark_updated_since_last_reset();
        self.host.update()
    }

    fn refresh_host_positions(&mut self, positions: impl Iterator<Item = (ObjectID, Coord3D)>) {
        self.mark_updated_since_last_reset();
        self.host.sync_positions(positions);
    }

    fn rebuild_host_positions(&mut self, positions: impl Iterator<Item = (ObjectID, Coord3D)>) {
        self.host.replace_positions(positions);
    }

    pub fn update_orphaned_ghosts(
        &mut self,
        player_index: usize,
        current_status: impl Fn(crate::object::w3d_ghost_object::W3DGhostSceneId) -> crate::common::ObjectShroudStatus,
        manager: &mut crate::object::W3DGhostObjectManager,
    ) {
        self.host.update_orphaned_ghosts(player_index, current_status, manager);
    }

    /// Host-state reset used by `GameLogic::reset` and the map-boundary
    /// adjust: drops the centre index, ghost links and cell shroud and clears
    /// `m_updatedSinceLastReset`, leaving the COI registrations untouched
    /// (they are re-registered by their owners).
    pub fn reset_host_state(&mut self) {
        self.host.clear();
        self.shroud.clear();
        self.reset_updated_since_last_reset();
    }

    /// Boundary resize releases and restores the existing ghost partitions.
    /// Keep their links while rebuilding the center index and shroud cells.
    pub(crate) fn reset_host_boundary_index(&mut self) {
        self.host.grid.clear();
        self.host.object_cells.clear();
        self.host.object_positions.clear();
        self.shroud.clear();
        self.reset_updated_since_last_reset();
    }

    /// C++ `PartitionManager::crc` host-side dump.
    pub(crate) fn host_crc_into(&self, xfer: &mut dyn Xfer) {
        self.host.crc_into(xfer);
    }

    pub(crate) fn host_cell_size_mut(&mut self) -> &mut Real {
        &mut self.host.cell_size
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
                geometry_scale: 1.0,
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
        assert_eq!(partition.ghost_link_scene_id(8), None);
        assert_eq!(partition.orphan_ghost_scene_ids(), vec![scene_id]);
        assert_eq!(manager.used_count(), 1);

        partition.update_orphaned_ghosts(0, |_| ObjectShroudStatus::Clear, &mut manager);

        assert!(partition.orphan_ghost_scene_ids().is_empty());
        assert_eq!(manager.used_count(), 0);
    }

    #[test]
    fn reused_object_id_gets_new_live_link_while_orphan_survives() {
        let mut partition = PartitionManager::new();
        let mut manager = W3DGhostObjectManager::new();
        partition.add_object(9, (40.0, 50.0, 0.0));
        let orphan_scene = partition
            .attach_object_ghost(9, true, &mut manager)
            .expect("ghost link");
        partition.apply_object_ghost_shroud_status(
            9,
            0,
            ObjectShroudStatus::Fogged,
            Some(&capture("OldBarracks")),
            &mut manager,
        );
        assert!(partition.detach_object_ghost(9, &mut manager));
        partition.remove_object(9);

        partition.add_object(9, (400.0, 400.0, 0.0));
        let live_scene = partition
            .attach_object_ghost(9, true, &mut manager)
            .expect("reused ID gets a fresh ghost");

        assert_ne!(live_scene, orphan_scene);
        assert_eq!(partition.ghost_link_scene_id(9), Some(live_scene));
        assert_eq!(partition.orphan_ghost_scene_ids(), vec![orphan_scene]);
        assert!(manager.linked_ghost_has_any_snapshot(orphan_scene));
        assert_eq!(
            partition.ghost_shroudedness_previous(9, 0),
            Some(ObjectShroudStatus::Invalid)
        );
        assert_eq!(
            partition.orphan_ghost_frozen_position(orphan_scene),
            Some(super::Coord3D::new(40.0, 50.0, 0.0))
        );
        assert_eq!(manager.used_count(), 2);
    }
}

#[cfg(test)]
mod world_to_cell_tests {
    use super::HostObjectIndex as PartitionManager;

    #[test]
    fn object_cells_use_cpp_real_to_int_floor_at_integral_boundaries() {
        // PartitionManager.h:1506-1510 REAL_TO_INT_FLOOR(w * m_cellSizeInv):
        // the almost-one subtract (BaseType.h:195-201) puts an exact negative
        // boundary one cell lower than f32::floor, and tiny negatives in cell 0.
        let mut partition = PartitionManager::new();
        let size = partition.cell_size();
        partition.add_object(1, (-size, 0.0, 0.0));
        partition.add_object(2, (-1.0e-6, 3.0 * size, 0.0));
        partition.add_object(3, (2.5 * size, size, 0.0));
        assert_eq!(partition.object_cells[&1], (-2, 0));
        assert_eq!(partition.object_cells[&2], (0, 3));
        assert_eq!(partition.object_cells[&3], (2, 1));
    }
}
