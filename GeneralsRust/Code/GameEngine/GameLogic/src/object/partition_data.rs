//! C++ `PartitionData` (`PartitionManager.cpp:1582-1688`).
//!
//! Walks the object's COI cells on the 40wu partition shroud grid and mixes
//! SHROUDED / FOGGED / CLEAR into object shroud, including fogged-enemy,
//! mine, neutral-mobile, and PARTIAL_CLEAR rules.

use std::sync::MutexGuard;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::common::{Coord3D, KindOf, MAX_PLAYER_COUNT, ObjectShroudStatus, Relationship};
use crate::object::Object;
use crate::object::collide::partition_coi::{do_circle_fill, do_rect_fill, do_small_fill};
use crate::object::collide::partition_manager::{CellCoord, PARTITION_MANAGER, PartitionManager};
use crate::object::collide::partition_shroud::PartitionCellShroudCounts;
use crate::player::player_list;
use game_engine::common::system::radar::CellShroudStatus;

/// Per-object seen history and C++ COI visibility rules.
///
/// Visibility is recomputed until cell invalidation is fully wired. Seen history
/// is retained through shared Object queries; the outer Object handle currently
/// requires Sync, so this history remains atomic. It is not a concurrency design
/// for the simulation. No write-only status cache is maintained here.
#[derive(Debug)]
pub struct PartitionData {
    ever_seen_by_player: [AtomicBool; MAX_PLAYER_COUNT],
}

impl Default for PartitionData {
    fn default() -> Self {
        Self::new()
    }
}

impl PartitionData {
    pub fn new() -> Self {
        Self {
            ever_seen_by_player: std::array::from_fn(|_| AtomicBool::new(false)),
        }
    }

    /// C++ `PartitionData::getShroudedStatus`.
    pub fn get_shrouded_status(&self, player_index: i32, object: &Object) -> ObjectShroudStatus {
        if player_index < 0 || player_index as usize >= MAX_PLAYER_COUNT {
            return ObjectShroudStatus::Clear;
        }
        let Some(counts) = object_cell_shroud_counts(player_index, object) else {
            // C++: shroud remains invalid until update. We recompute on every
            // query, so no cached status can survive reset.
            return ObjectShroudStatus::Invalid;
        };

        self.status_from_counts(
            player_index as usize,
            counts,
            object.is_kind_of(KindOf::Immobile),
            object.is_kind_of(KindOf::Mine),
            || viewer_relationship_to_object(player_index, object),
        )
    }

    /// Live-parent query from the exact driving match. The compatibility
    /// method above remains for callers that have not received their owner.
    pub(crate) fn get_shrouded_status_with_partition(
        &self,
        player_index: i32,
        object: &Object,
        partition: &crate::system::game_logic::PartitionManager,
    ) -> ObjectShroudStatus {
        if player_index < 0 || player_index as usize >= MAX_PLAYER_COUNT {
            return ObjectShroudStatus::Clear;
        }
        if !partition.updated_since_last_reset() {
            return ObjectShroudStatus::Invalid;
        }
        let cells = geometry_cells_for_object(object);
        let counts =
            partition.count_shroud_cells(player_index, cells.iter().map(|cell| (cell.x, cell.y)));
        self.status_from_counts(
            player_index as usize,
            counts,
            object.is_kind_of(KindOf::Immobile),
            object.is_kind_of(KindOf::Mine),
            || viewer_relationship_to_object(player_index, object),
        )
    }

    fn status_from_counts(
        &self,
        idx: usize,
        counts: PartitionCellShroudCounts,
        immobile: bool,
        mine: bool,
        relationship: impl FnOnce() -> Option<Relationship>,
    ) -> ObjectShroudStatus {
        let coi_count = counts.total;
        let shrouded_cells = counts.shrouded;
        let fogged_cells = counts.fogged;
        if coi_count == 0 || shrouded_cells == coi_count {
            self.ever_seen_by_player[idx].store(false, Ordering::Relaxed);
            ObjectShroudStatus::Shrouded
        } else if shrouded_cells + fogged_cells == coi_count {
            let mut fogged = ObjectShroudStatus::Fogged;
            match relationship() {
                Some(Relationship::Neutral) => {
                    if !immobile {
                        fogged = ObjectShroudStatus::Shrouded;
                    }
                }
                _ => {
                    if !(immobile && self.ever_seen_by_player[idx].load(Ordering::Relaxed)) || mine
                    {
                        fogged = ObjectShroudStatus::Shrouded;
                    }
                }
            }
            fogged
        } else if shrouded_cells == 0 && fogged_cells == 0 {
            self.ever_seen_by_player[idx].store(true, Ordering::Relaxed);
            ObjectShroudStatus::Clear
        } else {
            self.ever_seen_by_player[idx].store(true, Ordering::Relaxed);
            ObjectShroudStatus::PartialClear
        }
    }
}

// Temporary adapter for the existing two partition representations. All guards
// are local to a synchronous cell walk; no object/player callbacks run under them.
// The fallback is attempted once, only when the primary cannot answer a query.
enum LivePartitionRead {
    Unqueried,
    Unavailable,
    Borrowed(MutexGuard<'static, crate::system::game_logic::GameLogic>),
}

impl LivePartitionRead {
    fn partition(&mut self) -> Option<&crate::system::game_logic::PartitionManager> {
        if matches!(self, Self::Unqueried) {
            *self = match crate::system::game_logic::get_game_logic().try_lock() {
                Ok(logic) => Self::Borrowed(logic),
                Err(_) => Self::Unavailable,
            };
        }
        match self {
            Self::Borrowed(logic) => Some(logic.partition_manager()),
            Self::Unqueried | Self::Unavailable => None,
        }
    }
}

fn sample_partition_cells(
    primary: Option<&PartitionManager>,
    live: &mut LivePartitionRead,
    player_index: i32,
    cells: impl IntoIterator<Item = (i32, i32)>,
) -> PartitionCellShroudCounts {
    let fallback = |x, y| {
        live.partition()
            .map(|pm| pm.get_shroud_status_for_player_cell(player_index, x, y))
            .unwrap_or(CellShroudStatus::Shrouded)
    };
    match primary {
        Some(pm) => pm
            .shroud
            .count_cells_with_fallback(player_index, cells, fallback),
        None => PartitionCellShroudCounts::sample(cells, fallback),
    }
}

fn object_cell_shroud_counts(
    player_index: i32,
    object: &Object,
) -> Option<PartitionCellShroudCounts> {
    let primary = PARTITION_MANAGER.read().ok();
    let mut live = LivePartitionRead::Unqueried;
    if !primary
        .as_ref()
        .is_some_and(|pm| pm.updated_since_last_reset())
        && !live
            .partition()
            .is_some_and(|pm| pm.updated_since_last_reset())
    {
        return None;
    }
    // Borrow registered COIs instead of cloning the array on every query.
    let generated;
    let cells = match primary
        .as_ref()
        .and_then(|pm| pm.object_coi_cells(object.get_id()))
    {
        Some(cells) => cells,
        None => {
            generated = geometry_cells_for_object(object);
            &generated
        }
    };
    Some(sample_partition_cells(
        primary.as_deref(),
        &mut live,
        player_index,
        cells.iter().map(|cell| (cell.x, cell.y)),
    ))
}

fn geometry_cells_for_object(object: &Object) -> Vec<CellCoord> {
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
        if let Some(status) = pm.shroud.known_cell_status(player_index, x, y) {
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

/// Compatibility adapter for a whole footprint. Prefer
/// `PartitionShroudGrid::count_cells` when the driving instance is available.
/// Acquires the primary guard once and lazily tries the fallback at most once.
pub fn partition_cell_shroud_counts(
    player_index: i32,
    cells: &[(i32, i32)],
) -> PartitionCellShroudCounts {
    let primary = PARTITION_MANAGER.read().ok();
    sample_partition_cells(
        primary.as_deref(),
        &mut LivePartitionRead::Unqueried,
        player_index,
        cells.iter().copied(),
    )
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

/// Stamp shroud-cover circles onto both 40wu partition grids.
pub fn stamp_partition_cell_covers(center: &Coord3D, radius: f32, player_mask: u32, add: bool) {
    if let Ok(mut pm) = PARTITION_MANAGER.write() {
        if add {
            pm.do_shroud_cover_cells(center, radius, player_mask);
        } else {
            pm.undo_shroud_cover_cells(center, radius, player_mask);
        }
    }
    if let Ok(mut logic) = crate::system::game_logic::get_game_logic().try_lock() {
        let pm = logic.partition_manager_mut();
        if add {
            pm.do_shroud_cover(center, radius, player_mask);
        } else {
            pm.undo_shroud_cover(center, radius, player_mask);
        }
    }
}

#[cfg(test)]
#[path = "partition_data_tests.rs"]
mod tests;
