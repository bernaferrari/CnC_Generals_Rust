// pathfind_astar.rs
// A* Pathfinding Algorithm - Faithful C++ Port
// Reference: /GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AIPathfind.cpp
//
// Passability and movement cost: C++ validMovementPosition(),
// validLocomotorSurfacesForCellType() and PathfindCell::costSoFar().

use std::collections::{HashMap, HashSet};

use crate::astar::AStarPathfinder;
use crate::cell::{
    CellFlags, GridCoord, PathfindCellType, PathfindLayerEnum, COST_DIAGONAL, COST_ORTHOGONAL,
    SURFACE_AIR, SURFACE_CLIFF, SURFACE_GROUND, SURFACE_RUBBLE, SURFACE_WATER,
};
use crate::open_set::SearchKey;

impl AStarPathfinder {
    pub(crate) fn is_ignored_obstacle(
        &self,
        coord: GridCoord,
        layer: PathfindLayerEnum,
        ignore_cells: Option<&HashSet<GridCoord>>,
    ) -> bool {
        let Some(ignore_cells) = ignore_cells else {
            return false;
        };
        if !ignore_cells.contains(&coord) {
            return false;
        }
        matches!(
            self.get_cell_on_layer(coord, layer)
                .map(|cell| cell.get_type()),
            Some(PathfindCellType::Obstacle)
        )
    }

    /// C++ `Pathfinder::validLocomotorSurfacesForCellType` (AIPathfind.cpp:4734-4758).
    ///
    /// OBSTACLE / IMPASSABLE / BRIDGE_IMPASSABLE are AIR-only; every other type
    /// includes AIR as well so aircraft can overfly terrain.
    pub fn valid_locomotor_surfaces_for_cell_type(cell_type: PathfindCellType) -> u32 {
        match cell_type {
            PathfindCellType::Obstacle
            | PathfindCellType::Impassable
            | PathfindCellType::BridgeImpassable => SURFACE_AIR,
            PathfindCellType::Clear => SURFACE_GROUND | SURFACE_AIR,
            PathfindCellType::Water => SURFACE_WATER | SURFACE_AIR,
            PathfindCellType::Rubble => SURFACE_RUBBLE | SURFACE_AIR,
            PathfindCellType::Cliff => SURFACE_CLIFF | SURFACE_AIR,
        }
    }

    /// Check if a cell is passable for the given movement type
    /// Matches C++ validMovementPosition() logic
    pub fn is_passable(&self, coord: GridCoord, surfaces: u32, is_crusher: bool) -> bool {
        self.is_passable_on_layer(coord, PathfindLayerEnum::Ground, surfaces, is_crusher)
    }

    pub fn is_passable_with_ignore(
        &self,
        coord: GridCoord,
        surfaces: u32,
        is_crusher: bool,
        ignore_cells: Option<&HashSet<GridCoord>>,
    ) -> bool {
        self.is_passable_on_layer_with_ignore(
            coord,
            PathfindLayerEnum::Ground,
            surfaces,
            is_crusher,
            ignore_cells,
        )
    }

    /// Layered passability — C++ `validMovementPosition(..., layer, ...)`.
    pub fn is_passable_on_layer(
        &self,
        coord: GridCoord,
        layer: PathfindLayerEnum,
        surfaces: u32,
        is_crusher: bool,
    ) -> bool {
        self.is_passable_on_layer_with_ignore(coord, layer, surfaces, is_crusher, None)
    }

    pub fn is_passable_on_layer_with_ignore(
        &self,
        coord: GridCoord,
        layer: PathfindLayerEnum,
        surfaces: u32,
        is_crusher: bool,
        ignore_cells: Option<&HashSet<GridCoord>>,
    ) -> bool {
        let Some(cell) = self.get_cell_on_layer(coord, layer) else {
            return false;
        };

        if self.is_ignored_obstacle(coord, layer, ignore_cells) {
            return true;
        }

        // C++ Pathfinder::validMovementPosition (AIPathfind.cpp:4840-4842):
        //   if (isCrusher && toCell->isObstacleFence()) return true;
        // Solid CELL_OBSTACLE buildings stay blocked for ground crushers.
        // AIR locomotor still passes via validLocomotorSurfacesForCellType.
        if cell.get_type() == PathfindCellType::Obstacle
            && self.crusher_may_cross_obstacle(coord, cell.get_layer(), is_crusher)
        {
            return true;
        }

        // Note: Pinched cells are passable but have higher cost in movement_cost_with_ignore
        // This matches C++ behavior where pinched cells add COST_DIAGONAL but are not blocked

        let cell_surfaces = Self::valid_locomotor_surfaces_for_cell_type(cell.get_type());
        if (cell_surfaces & surfaces) != 0 {
            return true;
        }

        // Crushers may still enter rubble without a RUBBLE locomotor bit.
        cell.get_type() == PathfindCellType::Rubble && is_crusher
    }

    pub fn is_impassable_cell(&self, coord: GridCoord) -> bool {
        let Some(cell) = self.get_cell(coord) else {
            return true;
        };
        cell.is_impassable()
    }

    /// Calculate movement cost between adjacent cells
    /// Matches C++ PathfindCell::costSoFar() at AIPathfind.cpp:1691-1711
    pub(crate) fn movement_cost_with_ignore(
        &self,
        from: GridCoord,
        to: GridCoord,
        from_layer: PathfindLayerEnum,
        surfaces: u32,
        is_crusher: bool,
        ignore_cells: Option<&HashSet<GridCoord>>,
        came_from: &HashMap<SearchKey, SearchKey>,
    ) -> u32 {
        let Some(to_cell) = self.get_cell_on_layer(to, from_layer) else {
            return u32::MAX;
        };

        // Base cost: orthogonal or diagonal
        let mut cost = if from.is_diagonal(&to) {
            COST_DIAGONAL
        } else {
            COST_ORTHOGONAL
        };

        // Terrain cost modifiers matching C++ logic at AIPathfind.cpp:6263-6318
        match to_cell.get_type() {
            PathfindCellType::Clear => {}
            PathfindCellType::Water => {
                cost = (cost as f32 * 1.5) as u32; // Slower in water
            }
            PathfindCellType::Cliff => {
                // Base cliff surcharge applied when height unavailable; find_path_ex2
                // adjusts via ground_height when |dz| is known (C++ AIPathfind.cpp:6263-6276).
                cost += 7 * COST_DIAGONAL;
            }
            PathfindCellType::Rubble => {
                if is_crusher {
                    cost = (cost as f32 * 1.2) as u32;
                } else {
                    cost = (cost as f32 * 1.8) as u32;
                }
            }
            PathfindCellType::Obstacle => {
                if self.is_ignored_obstacle(to, from_layer, ignore_cells) {
                    // Treat ignored obstacles as clear.
                } else if self.crusher_may_cross_obstacle(to, to_cell.get_layer(), is_crusher)
                    || (surfaces & SURFACE_AIR) != 0
                {
                    // C++ examineNeighboringCells: CELL_OBSTACLE += 100*COST_ORTHOGONAL
                    // for crushers through fences and AIR over solid buildings.
                    cost += 100 * COST_ORTHOGONAL;
                } else {
                    return u32::MAX; // Impassable solid building
                }
            }
            PathfindCellType::BridgeImpassable | PathfindCellType::Impassable => {
                // C++ validLocomotorSurfacesForCellType: AIR only.
                if (surfaces & SURFACE_AIR) == 0 {
                    return u32::MAX;
                }
            }
        }

        // Apply pinched cell penalty (AIPathfind.cpp:1701-1703)
        // C++ adds COST_DIAGONAL (14) for pinched cells
        if to_cell.is_pinched() {
            cost += COST_DIAGONAL;
        }

        // Apply turn cost penalty (AIPathfind.cpp:1705-1720)
        // This adds extra cost for turns in the path
        if let Some(&parent_key) = came_from.get(&(from, from_layer)) {
            // Calculate direction vectors
            let parent_coord = parent_key.0;
            let prev_dir_x = from.x - parent_coord.x;
            let prev_dir_y = from.y - parent_coord.y;
            let curr_dir_x = to.x - from.x;
            let curr_dir_y = to.y - from.y;

            // If direction changed, add turn cost
            if prev_dir_x != curr_dir_x || prev_dir_y != curr_dir_y {
                // Dot product determines turn angle
                let dot = prev_dir_x * curr_dir_x + prev_dir_y * curr_dir_y;
                if dot > 0 {
                    cost += 4; // 45 degree turn
                } else if dot == 0 {
                    cost += 8; // 90 degree turn
                } else {
                    cost += 16; // 135 degree turn
                }
            }
        }

        // Apply custom cost multiplier
        cost = (cost as f32 * to_cell.cost_multiplier) as u32;

        cost
    }

    /// C++ UNIT_PRESENT_FIXED / UNIT_GOAL occupancy surcharge.
    pub(crate) fn cell_occupancy_cost(&self, cell: GridCoord) -> u32 {
        let Some(c) = self.get_cell(cell) else {
            return 0;
        };
        match c.get_flags() {
            CellFlags::UnitPresentFixed | CellFlags::UnitGoal => 3 * COST_DIAGONAL,
            CellFlags::UnitPresentMoving | CellFlags::UnitGoalOtherMoving => COST_DIAGONAL,
            CellFlags::NoUnits => 0,
        }
    }

    /// C++ `isCrusher && toCell->isObstacleFence()` in validMovementPosition.
    #[inline]
    fn crusher_may_cross_obstacle(
        &self,
        coord: GridCoord,
        layer: PathfindLayerEnum,
        is_crusher: bool,
    ) -> bool {
        is_crusher
            && self
                .obstacle_fence
                .contains(&Self::obstacle_key(coord, layer))
    }
}
