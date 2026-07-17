use std::collections::{HashMap, HashSet};

/// C++ PartitionManager PartitionCellSize residual (world units).
pub const PARTITION_CELL_SIZE_RESIDUAL: f32 = 40.0;

/// Minimal partition manager mirroring WW3D map reveal + collide broadphase residual.
#[derive(Debug, Default)]
pub struct PartitionManager {
    revealed_players: HashSet<u32>,
    /// Cell key (cx, cz) → object ids currently registered for collide residual.
    cells: HashMap<(i32, i32), Vec<u32>>,
    /// Object id → cell key.
    object_cells: HashMap<u32, (i32, i32)>,
}

impl PartitionManager {
    pub fn new() -> Self {
        Self {
            revealed_players: HashSet::new(),
            cells: HashMap::new(),
            object_cells: HashMap::new(),
        }
    }

    /// Permanently reveal the map for the specified player (observer mode).
    pub fn reveal_map_for_player(&mut self, player_id: u32) {
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

    /// C++ registerObject residual for collide broadphase.
    pub fn register_object_at(&mut self, id: u32, x: f32, z: f32) {
        let key = Self::cell_coords(x, z);
        if let Some(old) = self.object_cells.get(&id).copied() {
            if old == key {
                return;
            }
            if let Some(list) = self.cells.get_mut(&old) {
                list.retain(|&i| i != id);
                if list.is_empty() {
                    self.cells.remove(&old);
                }
            }
        }
        self.object_cells.insert(id, key);
        self.cells.entry(key).or_default().push(id);
    }

    /// C++ unRegisterObject residual.
    pub fn unregister_object(&mut self, id: u32) {
        if let Some(old) = self.object_cells.remove(&id) {
            if let Some(list) = self.cells.get_mut(&old) {
                list.retain(|&i| i != id);
                if list.is_empty() {
                    self.cells.remove(&old);
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

    pub fn registered_count(&self) -> usize {
        self.object_cells.len()
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
    }
}
