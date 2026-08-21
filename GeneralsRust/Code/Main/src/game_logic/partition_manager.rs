use std::collections::{HashMap, HashSet};

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

impl HostPartitionAffectStamp {
    pub fn apply(&self, add: bool) {
        let Some(pm) = gamelogic::helpers::ThePartitionManager::get() else {
            return;
        };
        let center = gamelogic::common::Coord3D::new(self.x, self.z, 0.0);
        let mask = gamelogic::common::PlayerMaskType::from_bits_truncate(self.mask);
        if add {
            if self.value > 0 {
                pm.do_value_affect(&center, self.range, self.value, mask);
            }
            if self.threat > 0 {
                pm.do_threat_affect(&center, self.range, self.threat, mask);
            }
        } else {
            if self.value > 0 {
                pm.undo_value_affect(&center, self.range, self.value, mask);
            }
            if self.threat > 0 {
                pm.undo_threat_affect(&center, self.range, self.threat, mask);
            }
        }
    }
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
    revealed_players: HashSet<u32>,
    /// Cell key (cx, cz) → object ids currently registered for collide residual.
    cells: HashMap<(i32, i32), Vec<u32>>,
    /// Object id → every overlapping cell (C++ COI list).
    object_cells: HashMap<u32, Vec<(i32, i32)>>,
}

impl PartitionManager {
    pub fn new() -> Self {
        Self {
            revealed_players: HashSet::new(),
            cells: HashMap::new(),
            object_cells: HashMap::new(),
        }
    }

    /// C++ PartitionManager::revealMapForPlayer (non-permanent).
    /// Shroud crate / RevealMapForPlayer script: addLooker+removeLooker → FOGGED.
    pub fn reveal_map_for_player(&mut self, player_id: u32) {
        crate::fow_rendering::reveal_entire_map_explored_for_player(player_id);
    }

    /// C++ PartitionManager::revealMapForPlayerPermanently — observer/defeat only.
    pub fn reveal_map_for_player_permanently(&mut self, player_id: u32) {
        if self.revealed_players.insert(player_id) {
            crate::fow_rendering::reveal_entire_map_for_player(player_id);
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

    /// C++ registerObject residual for collide broadphase (center cell).
    pub fn register_object_at(&mut self, id: u32, x: f32, z: f32) {
        self.register_object_footprint(id, x, z, 0.0);
    }

    /// Stamp every partition cell overlapping a circle of `radius`.
    pub fn register_object_footprint(&mut self, id: u32, x: f32, z: f32, radius: f32) {
        self.unregister_object(id);
        let keys = Self::cells_for_circle(x, z, radius);
        for key in &keys {
            self.cells.entry(*key).or_default().push(id);
        }
        self.object_cells.insert(id, keys);
    }

    fn cells_for_circle(x: f32, z: f32, radius: f32) -> Vec<(i32, i32)> {
        let s = PARTITION_CELL_SIZE_RESIDUAL;
        let r = radius.max(0.0);
        let min_cx = ((x - r) / s).floor() as i32;
        let max_cx = ((x + r) / s).floor() as i32;
        let min_cz = ((z - r) / s).floor() as i32;
        let max_cz = ((z + r) / s).floor() as i32;
        let mut keys = Vec::new();
        for cz in min_cz..=max_cz {
            for cx in min_cx..=max_cx {
                keys.push((cx, cz));
            }
        }
        if keys.is_empty() {
            keys.push(Self::cell_coords(x, z));
        }
        keys
    }

    /// C++ unRegisterObject residual.
    pub fn unregister_object(&mut self, id: u32) {
        if let Some(old) = self.object_cells.remove(&id) {
            for key in old {
                if let Some(list) = self.cells.get_mut(&key) {
                    list.retain(|&i| i != id);
                    if list.is_empty() {
                        self.cells.remove(&key);
                    }
                }
            }
        }
    }

    /// Candidate neighbor object ids for collide (self cell + 8 neighbors).
    pub fn neighbor_object_ids(&self, x: f32, z: f32) -> Vec<u32> {
        let (cx, cz) = Self::cell_coords(x, z);
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

    /// Object ids registered in cells overlapping a world-space XZ radius.
    ///
    /// Cell ring is ceil(radius / cell_size) + 1 (inclusive margin). Used by AI
    /// acquire scans so Lone Eagle (~900 objs) does not full-table scan every unit.
    pub fn ids_in_radius(&self, x: f32, z: f32, radius: f32) -> Vec<u32> {
        if self.object_cells.is_empty() {
            return Vec::new();
        }
        let r = radius.max(0.0);
        let (cx, cz) = Self::cell_coords(x, z);
        // Callers still distance-filter; +0 keeps small radii from spanning half the map.
        // Partial-cell margin: include one extra ring only when radius > 0.
        let ring = if r <= 0.0 {
            0
        } else {
            (r / PARTITION_CELL_SIZE_RESIDUAL).ceil() as i32
        };
        let mut out = Vec::new();
        for dz in -ring..=ring {
            for dx in -ring..=ring {
                if let Some(list) = self.cells.get(&(cx + dx, cz + dz)) {
                    out.extend(list.iter().copied());
                }
            }
        }
        out.sort_unstable();
        out.dedup();
        out
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
    fn reveal_map_for_player_is_not_permanent() {
        let mut pm = PartitionManager::new();
        pm.reveal_map_for_player(0);
        assert!(
            !pm.has_revealed_map(0),
            "crate/script reveal must not latch permanent lookers"
        );
        pm.reveal_map_for_player_permanently(1);
        assert!(pm.has_revealed_map(1));
    }


}
