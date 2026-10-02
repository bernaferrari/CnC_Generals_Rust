//! pathfind_astar — A* Pathfinding Algorithm, a faithful C++ port.
//!
//! Reference: `/GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AIPathfind.cpp`
//! (every `AIPathfind.*` line citation throughout this crate points there).
//!
//! The former single-file implementation is split by component; each module
//! owns one responsibility of the C++ `Pathfinder`:
//!
//! | Module | Responsibility (C++ anchor) |
//! |---|---|
//! | [`cell`] | cell / coordinate / layer model, terrain surfaces, cost constants |
//! | [`open_set`] | `AStarNode` comparator + open list (`putOnSortedOpenList`) |
//! | [`astar`] | `AStarPathfinder` storage and the `internalFindPath` search loop |
//! | [`passability`] | `validMovementPosition` / `costSoFar` movement cost |
//! | [`grid_edit`] | stored-grid mutation (cell types, connect layers, pinch) |
//! | [`obstacles`] | obstacle owner / fence / transparent annotation |
//! | [`zones`] | `PathfindZoneManager` coarse passable blocks |
//! | [`hierarchical`] | `internal_findHierarchicalPath` coarse block A* |
//!
//! Tunneling contract (C++ `m_isTunneling`, AIPathfind.cpp:6259+): a search may
//! begin with `starts_tunneling` set; the main loop in [`astar`] applies the
//! `10 * COST_ORTHOGONAL` invalid-step surcharge while tunneling and clears the
//! flag with `is_tunneling = false` as soon as it expands into a valid
//! non-pinched cell. See
//! [`AStarPathfinder::find_path_with_start_layer_and_layers`]
//! ([`astar::AStarPathfinder`]).
//!
//! Comparator semantics (equal-cost FIFO tie-break) are pinned by the
//! PERMANENT DIFFERENTIAL FIXTURE tests in `src/tests.rs`, which cite the C++
//! lines they defend; none of those assertions may be relaxed.

mod astar;
mod cell;
mod grid_edit;
mod hierarchical;
mod obstacles;
mod open_set;
mod passability;
mod zones;

pub use astar::AStarPathfinder;
pub use cell::{
    CellFlags, GridCoord, PathfindCell, PathfindCellType, PathfindLayerEnum, COST_DIAGONAL,
    COST_ORTHOGONAL, MAX_FRAMES_AHEAD, PATHFIND_CELL_SIZE, PATHFIND_CELL_SIZE_F,
    ZONE_IMPASSABLE_COST,
};

/// Crate-internal names reached by sibling modules and by `src/tests.rs`
/// through `use super::*;` (the comparator fixtures construct `AStarNode` /
/// `OpenSet` directly).
#[cfg(test)]
pub(crate) use cell::{SURFACE_AIR, SURFACE_CLIFF, SURFACE_GROUND};
#[cfg(test)]
pub(crate) use open_set::{AStarNode, OpenSet};
#[cfg(test)]
use glam::Vec3 as Coord3D;
#[cfg(test)]
use std::collections::{HashMap, HashSet};

#[cfg(test)]
mod tests;
