//! C++ `PartitionManager` shroud looker grid + `getPropShroudStatusForPlayer`.
//!
//! Shroud/looker/shrouder circles share the object partition cell size
//! (`GameData.ini PartitionCellSize = 40`). Missing cells are shrouded.

use super::partition_manager::{CellCoord, PARTITION_CELL_SIZE, PartitionManager};
use crate::common::{Coord3D, ObjectShroudStatus};
use game_engine::common::system::radar::CellShroudStatus;
use std::collections::HashMap;

const MAX_PLAYER_COUNT: usize = 16;

/// C++ `PartitionCell::ShroudLevel::m_currentShroud`:
/// `1` = shrouded, `0` = fogged, `<0` = lookers present (clear).
#[derive(Debug, Clone)]
pub struct PartitionShroudGrid {
    cell_size: f32,
    levels: HashMap<(i32, i32), [i16; MAX_PLAYER_COUNT]>,
}

impl Default for PartitionShroudGrid {
    fn default() -> Self {
        Self::new()
    }
}

impl PartitionShroudGrid {
    pub fn new() -> Self {
        Self {
            cell_size: PARTITION_CELL_SIZE,
            levels: HashMap::new(),
        }
    }

    pub fn cell_size(&self) -> f32 {
        self.cell_size
    }

    pub fn clear(&mut self) {
        self.levels.clear();
    }

    pub fn iter_known_cells(&self) -> impl Iterator<Item = (i32, i32)> + '_ {
        self.levels.keys().copied()
    }

    pub fn has_known_cell(&self, x: i32, y: i32) -> bool {
        self.levels.contains_key(&(x, y))
    }

    /// C++ `worldToCell` — floor(wx / cellSize). Extents origin is 0 on host maps.
    pub fn world_to_cell(&self, wx: f32, wy: f32) -> (i32, i32) {
        (
            (wx / self.cell_size).floor() as i32,
            (wy / self.cell_size).floor() as i32,
        )
    }

    fn level_at(&self, x: i32, y: i32, player_index: usize) -> i16 {
        self.levels
            .get(&(x, y))
            .and_then(|row| row.get(player_index).copied())
            .unwrap_or(1)
    }

    /// C++ `PartitionCell::getShroudStatusForPlayer`.
    pub fn cell_status(&self, player_index: i32, x: i32, y: i32) -> CellShroudStatus {
        if player_index < 0 {
            return CellShroudStatus::Shrouded;
        }
        let idx = player_index as usize;
        if idx >= MAX_PLAYER_COUNT {
            return CellShroudStatus::Shrouded;
        }
        match self.level_at(x, y, idx) {
            1 => CellShroudStatus::Shrouded,
            0 => CellShroudStatus::Fogged,
            _ => CellShroudStatus::Clear,
        }
    }

    pub fn status_at_world(&self, player_index: i32, loc: &Coord3D) -> CellShroudStatus {
        let (x, y) = self.world_to_cell(loc.x, loc.y);
        self.cell_status(player_index, x, y)
    }

    /// C++ `PartitionManager::getPropShroudStatusForPlayer`.
    /// Samples four cells around `loc - halfCell`.
    pub fn prop_status(&self, player_index: i32, loc: &Coord3D) -> ObjectShroudStatus {
        let half = self.cell_size * 0.5;
        let (x, y) = self.world_to_cell(loc.x - half, loc.y - half);
        let a = self.cell_status(player_index, x, y);
        if a != self.cell_status(player_index, x + 1, y)
            || a != self.cell_status(player_index, x + 1, y + 1)
            || a != self.cell_status(player_index, x, y + 1)
        {
            return ObjectShroudStatus::PartialClear;
        }
        match a {
            CellShroudStatus::Shrouded => ObjectShroudStatus::Shrouded,
            CellShroudStatus::Clear => ObjectShroudStatus::Clear,
            CellShroudStatus::Fogged => ObjectShroudStatus::Fogged,
        }
    }

    /// C++ `addLooker` current-shroud algorithm: `min(current - 1, -1)`.
    pub fn add_looker(&mut self, player_index: i32, x: i32, y: i32) {
        if player_index < 0 {
            return;
        }
        let idx = player_index as usize;
        if idx >= MAX_PLAYER_COUNT {
            return;
        }
        let row = self.levels.entry((x, y)).or_insert([1; MAX_PLAYER_COUNT]);
        row[idx] = (row[idx] - 1).min(-1);
    }

    /// C++ `removeLooker`: `-1` → fogged (`0`) when no active shrouders.
    pub fn remove_looker(&mut self, player_index: i32, x: i32, y: i32) {
        if player_index < 0 {
            return;
        }
        let idx = player_index as usize;
        if idx >= MAX_PLAYER_COUNT {
            return;
        }
        let row = self.levels.entry((x, y)).or_insert([1; MAX_PLAYER_COUNT]);
        if row[idx] == -1 {
            row[idx] = 0;
        } else if row[idx] < -1 {
            row[idx] += 1;
        }
    }

    /// Paint a looker circle on the 40wu grid (C++ `doShroudReveal` cell walk).
    pub fn reveal_circle(&mut self, center: &Coord3D, radius: f32, player_mask: u32) {
        if radius <= 0.0 {
            return;
        }
        let (cx, cy) = self.world_to_cell(center.x, center.y);
        let cell_r = (radius / self.cell_size).ceil() as i32;
        let r2 = radius * radius;
        for dx in -cell_r..=cell_r {
            for dy in -cell_r..=cell_r {
                let x = cx + dx;
                let y = cy + dy;
                let wx = x as f32 * self.cell_size + self.cell_size * 0.5;
                let wy = y as f32 * self.cell_size + self.cell_size * 0.5;
                let ddx = wx - center.x;
                let ddy = wy - center.y;
                if ddx * ddx + ddy * ddy > r2 {
                    continue;
                }
                for p in 0..MAX_PLAYER_COUNT {
                    if (player_mask & (1u32 << p)) != 0 {
                        self.add_looker(p as i32, x, y);
                    }
                }
            }
        }
    }

    pub fn undo_reveal_circle(&mut self, center: &Coord3D, radius: f32, player_mask: u32) {
        if radius <= 0.0 {
            return;
        }
        let (cx, cy) = self.world_to_cell(center.x, center.y);
        let cell_r = (radius / self.cell_size).ceil() as i32;
        let r2 = radius * radius;
        for dx in -cell_r..=cell_r {
            for dy in -cell_r..=cell_r {
                let x = cx + dx;
                let y = cy + dy;
                let wx = x as f32 * self.cell_size + self.cell_size * 0.5;
                let wy = y as f32 * self.cell_size + self.cell_size * 0.5;
                let ddx = wx - center.x;
                let ddy = wy - center.y;
                if ddx * ddx + ddy * ddy > r2 {
                    continue;
                }
                for p in 0..MAX_PLAYER_COUNT {
                    if (player_mask & (1u32 << p)) != 0 {
                        self.remove_looker(p as i32, x, y);
                    }
                }
            }
        }
    }
}

impl PartitionManager {
    /// C++ `getShroudStatusForPlayer(player, loc)` on the 40wu partition grid.
    pub fn get_shroud_status_for_player(
        &self,
        player_index: i32,
        loc: &Coord3D,
    ) -> CellShroudStatus {
        self.shroud.status_at_world(player_index, loc)
    }

    /// C++ `getShroudStatusForPlayer(player, x, y)`.
    pub fn get_shroud_status_for_player_cell(
        &self,
        player_index: i32,
        x: i32,
        y: i32,
    ) -> CellShroudStatus {
        self.shroud.cell_status(player_index, x, y)
    }

    /// C++ `getPropShroudStatusForPlayer` — trees/props sample four cells.
    pub fn get_prop_shroud_status_for_player(
        &self,
        player_index: i32,
        loc: &Coord3D,
    ) -> ObjectShroudStatus {
        self.shroud.prop_status(player_index, loc)
    }

    pub fn do_shroud_reveal_cells(&mut self, center: &Coord3D, radius: f32, player_mask: u32) {
        self.shroud.reveal_circle(center, radius, player_mask);
        self.mark_updated_since_last_reset();
    }

    pub fn undo_shroud_reveal_cells(&mut self, center: &Coord3D, radius: f32, player_mask: u32) {
        self.shroud.undo_reveal_circle(center, radius, player_mask);
        self.mark_updated_since_last_reset();
    }

    pub fn partition_cell_size() -> f32 {
        PARTITION_CELL_SIZE
    }

    pub fn world_to_partition_cell(pos: &Coord3D) -> CellCoord {
        CellCoord::from_world_pos(&super::Coord3D {
            x: pos.x,
            y: pos.y,
            z: pos.z,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cell_size_is_retail_40() {
        assert!((PARTITION_CELL_SIZE - 40.0).abs() < f32::EPSILON);
        let g = PartitionShroudGrid::new();
        assert!((g.cell_size() - 40.0).abs() < f32::EPSILON);
    }

    #[test]
    fn unknown_cell_is_shrouded() {
        let g = PartitionShroudGrid::new();
        assert_eq!(
            g.status_at_world(0, &Coord3D::new(10.0, 10.0, 0.0)),
            CellShroudStatus::Shrouded
        );
        assert_eq!(
            g.prop_status(0, &Coord3D::new(10.0, 10.0, 0.0)),
            ObjectShroudStatus::Shrouded
        );
    }

    #[test]
    fn reveal_clears_and_mixed_prop_is_partial() {
        let mut g = PartitionShroudGrid::new();
        g.reveal_circle(&Coord3D::new(20.0, 20.0, 0.0), 5.0, 1);
        assert_eq!(
            g.status_at_world(0, &Coord3D::new(20.0, 20.0, 0.0)),
            CellShroudStatus::Clear
        );
        // Half-cell offset samples (x,y) plus neighbors — neighbor still shrouded.
        let mixed = g.prop_status(0, &Coord3D::new(20.0, 20.0, 0.0));
        assert!(
            mixed == ObjectShroudStatus::PartialClear || mixed == ObjectShroudStatus::Clear,
            "revealed prop should not stay fully shrouded, got {mixed:?}"
        );
    }

    #[test]
    fn invalid_player_is_shrouded() {
        let g = PartitionShroudGrid::new();
        assert_eq!(g.cell_status(-1, 0, 0), CellShroudStatus::Shrouded);
        assert_eq!(
            g.prop_status(-1, &Coord3D::new(0.0, 0.0, 0.0)),
            ObjectShroudStatus::Shrouded
        );
    }
}
