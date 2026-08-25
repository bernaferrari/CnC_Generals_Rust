//! Complete Pathfinding System — faithful C++ port.
//!
//! Reference: `GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AIPathfind.cpp`
//!
//! Split from the former monolithic `pathfind_complete.rs` into focused
//! submodules. Public API is re-exported from this module root so existing
//! `pathfind_complete::{PathfindingSystem, PathRequest, PathResult, ...}`
//! imports keep working.

#![allow(unused_imports)]

pub(crate) use super::object_footprint_positions;
pub(crate) use super::path_optimization::PathOptimizer;
pub use super::pathfind_astar::{
    AStarPathfinder, COST_DIAGONAL, COST_ORTHOGONAL, GridCoord, PATHFIND_CELL_SIZE,
    PATHFIND_CELL_SIZE_F, PathfindCellType, PathfindLayerEnum,
};

pub(crate) use crate::common::KindOf;
pub(crate) use crate::common::xfer::{Xfer, XferExt};
pub(crate) use crate::common::{
    Coord2D, Coord3D, ICoord2D, INVALID_ID, ObjectID, ObjectStatusTypes,
    PathfindLayerEnum as CommonPathfindLayerEnum, Relationship,
};
pub(crate) use crate::helpers::{ThePartitionManager, TheTerrainLogic};
pub(crate) use crate::object::CrushSquishTestType;
pub(crate) use crate::object::registry::OBJECT_REGISTRY;

pub(crate) use std::collections::{HashMap, HashSet, VecDeque};
pub(crate) use std::sync::atomic::{AtomicI32, Ordering};
pub(crate) use std::sync::{Arc, Mutex};

mod attack_path;
mod block_zones;
mod check_movement;
mod classify;
mod construct;
mod find_path;
mod hierarchical;
mod line_passable;
mod occupancy;
mod snap;
mod system;
mod tall_buildings;
mod types;

#[cfg(test)]
mod tests;

pub use system::PathfindingSystem;
pub use types::{
    BridgeLayer, CheckMovementInfo, LAYER_Z_CLOSE_ENOUGH_F, LocomotorSurfaceTypeMask,
    MAX_PATH_ITERATIONS, MAX_WALL_PIECES, PATHFIND_CELLS_PER_FRAME, PATHFIND_QUEUE_LEN,
    PathRequest, PathResult, SURFACE_AIR, SURFACE_CLIFF, SURFACE_GROUND, SURFACE_RUBBLE,
    SURFACE_WATER, UNINITIALIZED_ZONE, ZONE_BLOCK_SIZE, compute_point_on_path_from_waypoints,
    peek_point_on_path_from_waypoints,
};

pub(crate) use block_zones::{BlockCombiner, ZoneManager};
pub(crate) use types::{
    GoalCell, ObjectPathQueue, clip_line_cells, dual_world_registry_unavailable,
    ignored_obstacle_cells,
};
