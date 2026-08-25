//! C++ `PartitionData` (`PartitionManager.cpp:1582-1688`).
//!
//! Walks the object's COI cells on the 40wu partition shroud grid and mixes
//! SHROUDED / FOGGED / CLEAR into object shroud, including fogged-enemy,
//! mine, neutral-mobile, and PARTIAL_CLEAR rules.

use crate::common::{Coord3D, KindOf, MAX_PLAYER_COUNT, ObjectShroudStatus, Relationship};
use crate::object::Object;
use crate::object::collide::partition_coi::{do_circle_fill, do_rect_fill, do_small_fill};
use crate::object::collide::partition_manager::{CellCoord, PARTITION_MANAGER};
use crate::player::player_list;
use game_engine::common::system::radar::CellShroudStatus;

/// C++ `PartitionData` per-object shroud cache + COI mix.
#[derive(Debug, Clone)]
pub struct PartitionData {
    shroudedness: [ObjectShroudStatus; MAX_PLAYER_COUNT],
    ever_seen_by_player: [bool; MAX_PLAYER_COUNT],
}

impl Default for PartitionData {
    fn default() -> Self {
        Self::new()
    }
}

impl PartitionData {
    pub fn new() -> Self {
        Self {
            shroudedness: [ObjectShroudStatus::Invalid; MAX_PLAYER_COUNT],
            ever_seen_by_player: [false; MAX_PLAYER_COUNT],
        }
    }

    /// C++ `PartitionData::invalidateShroudedStatusForPlayer`.
    pub fn invalidate_shrouded_status_for_player(&mut self, player_index: i32) {
        if let Some(slot) = self.shroudedness.get_mut(player_index as usize) {
            *slot = ObjectShroudStatus::Invalid;
        }
    }

    /// C++ `PartitionData::getShroudedStatus`.
    pub fn get_shrouded_status(
        &mut self,
        player_index: i32,
        object: &Object,
    ) -> ObjectShroudStatus {
        if player_index < 0 || player_index as usize >= MAX_PLAYER_COUNT {
            return ObjectShroudStatus::Clear;
        }
        let idx = player_index as usize;

        if !partition_updated_since_last_reset() {
            // C++: fog must not persist across reset; force recompute next ask.
            self.invalidate_shrouded_status_for_player(player_index);
            return ObjectShroudStatus::Invalid;
        }

        let cells = coi_cells_for_object(object);
        let mut shrouded_cells = 0usize;
        let mut fogged_cells = 0usize;
        for cell in &cells {
            match cell_shroud_status(player_index, cell.x, cell.y) {
                CellShroudStatus::Shrouded => shrouded_cells += 1,
                CellShroudStatus::Fogged => fogged_cells += 1,
                CellShroudStatus::Clear => {}
            }
        }

        let coi_count = cells.len();
        let status = if coi_count == 0 {
            self.ever_seen_by_player[idx] = false;
            ObjectShroudStatus::Shrouded
        } else if shrouded_cells == coi_count {
            self.ever_seen_by_player[idx] = false;
            ObjectShroudStatus::Shrouded
        } else if shrouded_cells + fogged_cells == coi_count {
            let mut fogged = ObjectShroudStatus::Fogged;
            let relationship = viewer_relationship_to_object(player_index, object);
            let immobile = object.is_kind_of(KindOf::Immobile);
            let mine = object.is_kind_of(KindOf::Mine);
            match relationship {
                Some(Relationship::Neutral) => {
                    if !immobile {
                        fogged = ObjectShroudStatus::Shrouded;
                    }
                }
                _ => {
                    if !(immobile && self.ever_seen_by_player[idx]) || mine {
                        fogged = ObjectShroudStatus::Shrouded;
                    }
                }
            }
            fogged
        } else if shrouded_cells == 0 && fogged_cells == 0 {
            self.ever_seen_by_player[idx] = true;
            ObjectShroudStatus::Clear
        } else {
            self.ever_seen_by_player[idx] = true;
            ObjectShroudStatus::PartialClear
        };

        self.shroudedness[idx] = status;
        status
    }
}

fn partition_updated_since_last_reset() -> bool {
    let leftover = PARTITION_MANAGER
        .read()
        .ok()
        .is_some_and(|pm| pm.updated_since_last_reset());
    if leftover {
        return true;
    }
    crate::system::game_logic::get_game_logic()
        .try_lock()
        .ok()
        .is_some_and(|logic| logic.partition_manager().updated_since_last_reset())
}

fn coi_cells_for_object(object: &Object) -> Vec<CellCoord> {
    if let Ok(pm) = PARTITION_MANAGER.read() {
        if let Some(cells) = pm.object_coi_cells(object.get_id()) {
            return cells.to_vec();
        }
    }
    let pos = *object.get_position();
    let geom = object.get_geometry_info();
    if geom.get_is_small() {
        do_small_fill(pos.x, pos.y, geom.get_major_radius())
    } else if geom.get_minor_radius() + 0.5 < geom.get_major_radius() {
        do_rect_fill(
            pos.x,
            pos.y,
            geom.get_major_radius(),
            geom.get_minor_radius(),
            object.get_orientation(),
        )
    } else {
        do_circle_fill(pos.x, pos.y, geom.get_major_radius())
    }
}

fn cell_shroud_status(player_index: i32, x: i32, y: i32) -> CellShroudStatus {
    if let Ok(pm) = PARTITION_MANAGER.read() {
        let status = pm.get_shroud_status_for_player_cell(player_index, x, y);
        if status != CellShroudStatus::Shrouded || pm.has_known_shroud_cell(x, y) {
            return status;
        }
    }
    crate::system::game_logic::get_game_logic()
        .try_lock()
        .ok()
        .map(|logic| {
            logic
                .partition_manager()
                .get_shroud_status_for_player_cell(player_index, x, y)
        })
        .unwrap_or(CellShroudStatus::Shrouded)
}

/// Host FOW: cell shroud on the 40wu partition grid stamped by lookers.
pub fn partition_cell_shroud_status(player_index: i32, x: i32, y: i32) -> CellShroudStatus {
    cell_shroud_status(player_index, x, y)
}

fn viewer_relationship_to_object(player_index: i32, object: &Object) -> Option<Relationship> {
    let list = player_list().read().ok()?;
    let player_arc = list.get_player(player_index)?.clone();
    drop(list);
    let player = player_arc.read().ok()?;
    let team_arc = object.get_team()?;
    let team = team_arc.read().ok()?;
    Some(player.get_relationship_with_team(&team))
}

/// Stamp looker circles onto both 40wu partition grids (leftover + live).
pub fn stamp_partition_cell_lookers(center: &Coord3D, radius: f32, player_mask: u32, add: bool) {
    if let Ok(mut pm) = PARTITION_MANAGER.write() {
        if add {
            pm.do_shroud_reveal_cells(center, radius, player_mask);
        } else {
            pm.undo_shroud_reveal_cells(center, radius, player_mask);
        }
    }
    if let Ok(mut logic) = crate::system::game_logic::get_game_logic().try_lock() {
        let pm = logic.partition_manager_mut();
        if add {
            pm.do_shroud_reveal(center, radius, player_mask);
        } else {
            pm.undo_shroud_reveal(center, radius, player_mask);
        }
    }
}
