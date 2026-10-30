use super::partition_coi::{HostPartitionFootprint, cells_touched_for_footprint};

#[path = "partition_radius_query.rs"]
mod radius_query;

use std::collections::{HashMap, HashSet};
mod partition_contacts;
mod partition_containment;
mod partition_continuation;
mod partition_dirty;
mod partition_scalars;
pub(in crate::game_logic) use partition_contacts::PartitionContact;
pub(crate) use partition_continuation::PartitionContinuation;
use partition_dirty::{PartitionDirty, PartitionObject};

/// C++ PartitionManager PartitionCellSize residual (world units).
pub const PARTITION_CELL_SIZE_RESIDUAL: f32 = 40.0;

/// Last value/threat stamp for a live host object (C++ SightingInfo residual).
#[derive(Debug, Clone, Copy, Default, serde::Serialize, serde::Deserialize)]
pub struct HostPartitionAffectStamp {
    pub x: f32,
    pub z: f32,
    pub range: f32,
    pub value: u32,
    pub threat: u32,
    pub mask: u32,
}

/// Last doShroudReveal looker for a live host object (C++ SightingInfo).
#[derive(Debug, Clone, Copy, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct HostPartitionLookStamp {
    pub x: f32,
    pub z: f32,
    pub range: f32,
    pub mask: u32,
}

/// Minimal partition manager mirroring WW3D map reveal + collide broadphase residual.
#[derive(Debug, Default)]
pub struct PartitionManager {
    /// Immutable receiving map geometry; mapless programmatic values retain the
    /// retail fixture grid until a real map is admitted.
    grid: Option<PartitionGrid>,
    revealed_players: HashSet<u32>,
    /// Cell key (cx, cz) → object ids currently registered for collide residual.
    cells: HashMap<(i32, i32), Vec<u32>>,
    /// Object id → every overlapping cell (C++ COI list).
    object_cells: HashMap<u32, PartitionObject>,
    dirty: Vec<PartitionDirty>,
    namespace: std::sync::Arc<()>,
    /// Same receiving WorldServices grid; this handle adds no scalar storage.
    scalar_owner: Option<partition_scalars::ScalarOwner>,
}

#[derive(Clone, Copy, Debug)]
struct PartitionGrid {
    origin: [f32; 2],
    dimensions: [usize; 2],
    cell_size: f32,
}

impl PartitionManager {
    pub fn new() -> Self {
        Self {
            grid: None,
            revealed_players: HashSet::new(),
            cells: HashMap::new(),
            object_cells: HashMap::new(),
            dirty: Vec::new(),
            namespace: std::sync::Arc::new(()),
            scalar_owner: None,
        }
    }

    /// Both collision and shroud sample the grid admitted by this map. There
    /// is no config poll or independent cell-size selection in a query.
    pub(crate) fn configure_grid_from_shroud(
        &mut self,
        shroud: &gamelogic::system::shroud_manager::ShroudManager,
    ) {
        self.clear_registered_objects();
        self.revealed_players.clear();
        self.grid = shroud
            .grid_dimensions()
            .zip(shroud.grid_world_origin())
            .map(|((width, height, cell_size), origin)| PartitionGrid {
                origin,
                dimensions: [width, height],
                cell_size,
            });
    }

    fn receiving_cell_coords(&self, x: f32, z: f32) -> (i32, i32) {
        match self.grid {
            Some(grid) => (
                ((x - grid.origin[0]) * ((1.0_f64 / f64::from(grid.cell_size)) as f32)).floor()
                    as i32,
                ((z - grid.origin[1]) * ((1.0_f64 / f64::from(grid.cell_size)) as f32)).floor()
                    as i32,
            ),
            None => Self::cell_coords(x, z),
        }
    }

    fn footprint_cells(&self, x: f32, z: f32, fp: HostPartitionFootprint) -> Vec<(i32, i32)> {
        match self.grid {
            Some(grid) => super::partition_coi::cells_touched_for_footprint_on_grid(grid.origin,grid.dimensions,grid.cell_size,x,z,fp)
                .expect("receiving Object partition footprint must be valid and within the admitted sample budget"),
            None => cells_touched_for_footprint(x,z,fp),
        }
    }

    /// C++ PartitionManager::revealMapForPlayer (non-permanent).
    /// Shroud crate / RevealMapForPlayer script: addLooker+removeLooker → FOGGED.
    pub fn reveal_map_for_player(
        &mut self,
        shroud: &mut gamelogic::system::shroud_manager::ShroudManager,
        player_id: u32,
    ) {
        crate::fow_rendering::reveal_entire_map_explored_for_player(shroud, player_id);
    }

    /// C++ PartitionManager::revealMapForPlayerPermanently — observer/defeat only.
    pub fn reveal_map_for_player_permanently(
        &mut self,
        shroud: &mut gamelogic::system::shroud_manager::ShroudManager,
        player_id: u32,
    ) {
        if self.revealed_players.insert(player_id) {
            crate::fow_rendering::reveal_entire_map_for_player(shroud, player_id);
        }
    }

    pub fn has_revealed_map(&self, player_id: u32) -> bool {
        self.revealed_players.contains(&player_id)
    }

    /// World XZ → cell indices residual.
    pub fn cell_coords(x: f32, z: f32) -> (i32, i32) {
        let s = PARTITION_CELL_SIZE_RESIDUAL;
        ((x / s).floor() as i32, (z / s).floor() as i32)
    }

    /// C++ `registerObject` residual — stamp the object's authored geometry COI
    /// (`doSmallFill` / `doCircleFill` / `doRectFill`), not a selection-radius AABB.
    pub fn register_object_at(&mut self, id: u32, x: f32, z: f32) {
        self.register_object_geometry(id, x, z, HostPartitionFootprint::small_circle(0.0));
    }

    /// Stamp every partition cell overlapping a circle of `radius`.
    /// Small radii use `doSmallFill`; larger ones use `doCircleFill`.
    pub fn register_object_footprint(&mut self, id: u32, x: f32, z: f32, radius: f32) {
        let r = radius.max(0.0);
        let fp = if r
            <= self
                .grid
                .map_or(PARTITION_CELL_SIZE_RESIDUAL, |grid| grid.cell_size)
                * 0.5
        {
            HostPartitionFootprint::small_circle(r)
        } else {
            HostPartitionFootprint {
                major_radius: r,
                minor_radius: r,
                angle: 0.0,
                is_small: false,
                is_box: false,
            }
        };
        self.register_object_geometry(id, x, z, fp);
    }

    /// C++ `PartitionData::updateCellsTouched` — stamp every overlapping cell.
    pub fn register_object_geometry(
        &mut self,
        id: u32,
        x: f32,
        z: f32,
        fp: HostPartitionFootprint,
    ) {
        self.clear_object_coverage(id);
        let keys = self.footprint_cells(x, z, fp);
        for key in &keys {
            self.cells.entry(*key).or_default().push(id);
        }
        self.object_cells.entry(id).or_default().cells = keys;
    }

    /// C++ unRegisterObject residual.
    pub fn unregister_object(&mut self, id: u32) {
        self.clear_object_coverage(id);
        self.object_cells.remove(&id);
        self.dirty.retain(|dirty| dirty.object_id != id);
    }

    /// Drop coverage while retaining the original attached module/dirty identity.
    fn clear_object_coverage(&mut self, id: u32) {
        if let Some(module) = self.object_cells.get_mut(&id) {
            for key in std::mem::take(&mut module.cells) {
                if let Some(list) = self.cells.get_mut(&key) {
                    list.retain(|&i| i != id);
                    if list.is_empty() {
                        self.cells.remove(&key);
                    }
                }
            }
        }
    }

    /// Candidate neighbor object ids for collide (center cell + 8 neighbors).
    /// Point queries stay 9-cell; object collide must use [`Self::neighbor_object_ids_of`].
    pub fn neighbor_object_ids(&self, x: f32, z: f32) -> Vec<u32> {
        let (cx, cz) = self.receiving_cell_coords(x, z);
        let mut out = Vec::new();
        for dz in -1..=1 {
            for dx in -1..=1 {
                if let Some(list) = self.cells.get(&(cx + dx, cz + dz)) {
                    out.extend(list.iter().copied());
                }
            }
        }
        out.sort_unstable();
        out.dedup();
        out
    }

    /// C++ `PartitionData::addPossibleCollisions` — every other module in
    /// **every** COI cell of `id`, not the 9-cell ring around its center.
    pub fn neighbor_object_ids_of(&self, id: u32) -> Vec<u32> {
        let Some(keys) = self.object_cells.get(&id) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for key in &keys.cells {
            if let Some(list) = self.cells.get(key) {
                out.extend(list.iter().copied());
            }
        }
        out.sort_unstable();
        out.dedup();
        out
    }

    /// Broadphase IDs from C++ FASTER_GCO radius buckets over stored COIs.
    /// Current callers retain the explicit sorted-ID set policy and must apply
    /// their actual Object distance/geometry/relation filters themselves.
    /// This does not implement source COI visitation or SimpleObjectIterator order.
    pub fn ids_in_radius(&self, x: f32, z: f32, radius: f32) -> Vec<u32> {
        if self.object_cells.is_empty() {
            return Vec::new();
        }
        let (cx, cz) = self.receiving_cell_coords(x, z);
        let query = radius_query::RadiusCellQuery::new(
            [cx, cz],
            self.grid
                .map_or(PARTITION_CELL_SIZE_RESIDUAL, |grid| grid.cell_size),
            self.grid.map(|grid| grid.dimensions),
            radius.max(0.0),
        );
        let mut out = Vec::new();
        for (&(cell_x, cell_z), objects) in &self.cells {
            if query.accepts([cell_x, cell_z]) {
                out.extend(objects.iter().copied());
            }
        }
        out.sort_unstable();
        out.dedup();
        out
    }

    /// C++ getClosestObjects -> SimpleObjectIterator(ITER_FASTEST).
    /// Distance and object filters run while the iterator is built, before any
    /// caller callback. Preserve first-COI dedup even when a filter rejects it.
    /// Cells own chronological insertion vectors; reverse them for CPP prepend
    /// order. SimpleObjectIterator::insert prepends again, so reverse the final
    /// accepted vector. No parallel index, persistent cursor or ambient flags.
    pub(in crate::game_logic) fn ids_in_radius_iterator_order(
        &self,
        x: f32,
        z: f32,
        radius: f32,
        mut accept: impl FnMut(u32) -> bool,
    ) -> Vec<u32> {
        let (cx, cz) = self.receiving_cell_coords(x, z);
        let query = radius_query::RadiusCellQuery::new(
            [cx, cz],
            self.grid
                .map_or(PARTITION_CELL_SIZE_RESIDUAL, |grid| grid.cell_size),
            self.grid.map(|grid| grid.dimensions),
            radius,
        );
        let mut cells: Vec<_> = self
            .cells
            .iter()
            .filter(|(cell, _)| query.accepts([cell.0, cell.1]))
            .collect();
        cells.sort_by_key(|(cell, _)| query.traversal_key([cell.0, cell.1]));
        let mut seen = HashSet::new();
        let mut result = Vec::new();
        for (_, occupants) in cells {
            for id in occupants.iter().rev().copied() {
                if seen.insert(id) && accept(id) {
                    result.push(id);
                }
            }
        }
        result.reverse();
        result
    }

    /// CPP getClosestObject scans the closest occupied radius first, visits
    /// the whole radius, and keeps the first strictly closest allowed object.
    pub(in crate::game_logic) fn closest_in_radius(
        &self,
        x: f32,
        z: f32,
        radius: f32,
        mut distance_if_allowed: impl FnMut(u32) -> Option<f32>,
    ) -> Option<u32> {
        let (cx, cz) = self.receiving_cell_coords(x, z);
        let query = radius_query::RadiusCellQuery::new(
            [cx, cz],
            self.grid
                .map_or(PARTITION_CELL_SIZE_RESIDUAL, |grid| grid.cell_size),
            self.grid.map(|grid| grid.dimensions),
            radius,
        );
        let mut cells: Vec<_> = self
            .cells
            .iter()
            .filter(|(cell, _)| query.accepts([cell.0, cell.1]))
            .collect();
        cells.sort_by_key(|(cell, _)| query.traversal_key([cell.0, cell.1]));
        let mut seen = HashSet::new();
        let mut closest = None;
        let mut best_distance = radius * radius;
        let mut found_radius = None;
        for (cell, occupants) in cells {
            let ring = query.traversal_key([cell.0, cell.1]).0;
            if found_radius.is_some_and(|found| ring > found) {
                break;
            }
            for id in occupants.iter().rev().copied() {
                if !seen.insert(id) {
                    continue;
                }
                if let Some(distance) = distance_if_allowed(id) {
                    if distance < best_distance {
                        best_distance = distance;
                        closest = Some(id);
                        found_radius = Some(ring);
                    }
                }
            }
        }
        closest
    }

    pub fn registered_count(&self) -> usize {
        self.object_cells.len()
    }

    pub fn is_registered(&self, id: u32) -> bool {
        self.object_cells.contains_key(&id)
    }

    pub fn cell_count(&self) -> usize {
        self.cells.len()
    }

    /// Drop collide registration without clearing FOW reveal residual.
    pub fn clear_registered_objects(&mut self) {
        self.cells.clear();
        self.object_cells.clear();
        self.dirty.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn partition_cell_register_and_neighbors() {
        let mut pm = PartitionManager::new();
        assert_eq!(PARTITION_CELL_SIZE_RESIDUAL, 40.0);
        pm.register_object_at(1, 10.0, 10.0);
        pm.register_object_at(2, 15.0, 12.0); // same cell
        pm.register_object_at(3, 100.0, 100.0); // far cell
        assert_eq!(pm.registered_count(), 3);
        let n = pm.neighbor_object_ids(10.0, 10.0);
        assert!(n.contains(&1) && n.contains(&2));
        assert!(!n.contains(&3));
        pm.unregister_object(1);
        assert_eq!(pm.registered_count(), 2);
        // Move object 2 into far cell.
        pm.register_object_at(2, 100.0, 100.0);
        let n2 = pm.neighbor_object_ids(100.0, 100.0);
        assert!(n2.contains(&2) && n2.contains(&3));
        // Radius query includes far cell when radius is large enough.
        let wide = pm.ids_in_radius(10.0, 10.0, 200.0);
        assert!(wide.contains(&2) && wide.contains(&3));
        let tight = pm.ids_in_radius(10.0, 10.0, 5.0);
        assert!(!tight.contains(&3));
    }

    #[test]
    fn large_box_coi_shares_cells_beyond_nine_cell_ring() {
        let mut pm = PartitionManager::new();
        let building = HostPartitionFootprint {
            major_radius: 80.0,
            minor_radius: 80.0,
            angle: 0.0,
            is_small: false,
            is_box: true,
        };
        pm.register_object_geometry(1, 0.0, 0.0, building);
        pm.register_object_geometry(2, 80.0, 0.0, HostPartitionFootprint::small_circle(5.0));
        // Centers are two 40wu cells apart — 9-cell from the tank center misses
        // a selection-radius stamp of the building, but COI cells overlap.
        let n = pm.neighbor_object_ids_of(2);
        assert!(
            n.contains(&1),
            "hull-sharing pair must be a collide candidate"
        );
        let near = pm.ids_in_radius(80.0, 0.0, 15.0);
        assert!(
            near.contains(&1),
            "iterate must see a structure whose hull is in range"
        );
    }

    #[test]
    fn reveal_map_for_player_is_not_permanent() {
        let mut pm = PartitionManager::new();
        let mut shroud = gamelogic::system::shroud_manager::ShroudManager::new();
        pm.reveal_map_for_player(&mut shroud, 0);
        assert!(
            !pm.has_revealed_map(0),
            "crate/script reveal must not latch permanent lookers"
        );
        pm.reveal_map_for_player_permanently(&mut shroud, 1);
        assert!(pm.has_revealed_map(1));
    }
    #[test]
    fn cpp_fastest_iterator_preserves_cell_prepend_and_reinsert_order() {
        let mut partition = PartitionManager::new();
        partition.register_object_at(90, 10., 10.);
        partition.register_object_at(2, 10., 10.);
        partition.register_object_at(70, 10., 10.);
        assert_eq!(
            partition.ids_in_radius_iterator_order(10., 10., 0., |_| true),
            vec![90, 2, 70]
        );
        // updateCellsTouched removes every COI and then prepends new coverage,
        // including when the actor remains in the same cell.
        partition.register_object_at(2, 11., 11.);
        assert_eq!(
            partition.ids_in_radius_iterator_order(10., 10., 0., |_| true),
            vec![90, 70, 2]
        );
        assert_eq!(partition.ids_in_radius(10., 10., 0.), vec![2, 70, 90]);
    }

    #[test]
    fn cpp_fastest_iterator_reverses_bucket_then_y_x_and_filters_once() {
        let mut partition = PartitionManager::new();
        partition.register_object_at(8, -30., -30.);
        partition.register_object_at(7, 10., 10.);
        partition.register_object_at(6, 50., 50.);
        // All adjacent cells belong to radius bucket zero. Source offsets
        // visit y then x; SimpleObjectIterator prepend reverses accepted rows.
        assert_eq!(
            partition.ids_in_radius_iterator_order(10., 10., 0., |_| true),
            vec![6, 7, 8]
        );
        partition.register_object_footprint(4, 10., 10., 50.);
        let mut calls = std::collections::HashMap::new();
        let accepted = partition.ids_in_radius_iterator_order(10., 10., 80., |id| {
            *calls.entry(id).or_insert(0) += 1;
            id != 4
        });
        assert_eq!(calls[&4], 1);
        assert!(!accepted.contains(&4));
        assert_eq!(accepted, vec![6, 7, 8]);
    }
}
