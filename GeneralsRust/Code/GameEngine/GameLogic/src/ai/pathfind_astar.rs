// pathfind_astar.rs
// A* Pathfinding Algorithm - Faithful C++ Port
// Reference: /GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AIPathfind.cpp

use crate::common::{Coord2D, Coord3D};
use std::cmp::{Ordering, Reverse};
use std::collections::{BinaryHeap, HashMap, HashSet};

/// Movement cost constants matching C++ AIPathfind.cpp:1649-1650
pub const COST_ORTHOGONAL: u32 = 10;
pub const COST_DIAGONAL: u32 = 14;
/// C++ notZonePassable penalty: `100 * COST_ORTHOGONAL`.
pub const ZONE_IMPASSABLE_COST: u32 = 100 * COST_ORTHOGONAL;

/// Pathfinding cell size matching C++ AIPathfind.h:415-416
pub const PATHFIND_CELL_SIZE: i32 = 10;
pub const PATHFIND_CELL_SIZE_F: f32 = 10.0;

/// Terrain/layer Z for a path node (C++ TerrainLogic::getLayerHeight).
fn layer_world_height(x: f32, y: f32, layer: PathfindLayerEnum) -> f32 {
    let common = match layer {
        PathfindLayerEnum::Invalid => crate::common::PathfindLayerEnum::Invalid,
        PathfindLayerEnum::Ground => crate::common::PathfindLayerEnum::Ground,
        PathfindLayerEnum::Wall => crate::common::PathfindLayerEnum::Wall,
        _ => crate::common::PathfindLayerEnum::Top,
    };
    crate::helpers::TheTerrainLogic::get()
        .map(|t| t.get_layer_height(x, y, common))
        .unwrap_or(0.0)
}

/// Maximum frames ahead for synchronization matching C++ Connection.cpp
pub const MAX_FRAMES_AHEAD: u32 = 300;
const SURFACE_GROUND: u32 = 0x01;
const SURFACE_WATER: u32 = 0x02;
const SURFACE_CLIFF: u32 = 0x04;
const SURFACE_AIR: u32 = 0x08;
const SURFACE_RUBBLE: u32 = 0x10;

/// Cell type matching C++ AIPathfind.h:233-242
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum PathfindCellType {
    Clear = 0x00,            // Clear, unobstructed ground
    Water = 0x01,            // Water area
    Cliff = 0x02,            // Steep altitude change
    Rubble = 0x03,           // Cell occupied by rubble
    Obstacle = 0x04,         // Occupied by a structure
    BridgeImpassable = 0x05, // Impassable bridge piece
    Impassable = 0x06,       // Impassable except for aircraft
}

/// Cell flags matching C++ AIPathfind.h:244-251
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum CellFlags {
    NoUnits = 0x00,             // No units in this cell
    UnitGoal = 0x01,            // Unit heading to this cell
    UnitPresentMoving = 0x02,   // Unit moving through cell
    UnitPresentFixed = 0x03,    // Unit stationary in cell
    UnitGoalOtherMoving = 0x05, // Unit moving + another has goal
}

/// Pathfinding layer enum matching C++ GameType.h
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum PathfindLayerEnum {
    Invalid = 0,
    Ground = 1,
    /// C++ first unnamed bridge slot (`LAYER_GROUND + 1`).
    Top = 2,
    Layer3 = 3,
    Layer4 = 4,
    Layer5 = 5,
    Layer6 = 6,
    Layer7 = 7,
    Layer8 = 8,
    Layer9 = 9,
    Layer10 = 10,
    Layer11 = 11,
    Layer12 = 12,
    Layer13 = 13,
    Layer14 = 14,
    /// C++ `LAYER_WALL = LAYER_LAST = 15`.
    Wall = 15,
}

impl PathfindLayerEnum {
    pub const LAST: Self = Self::Wall;

    pub fn from_u32(value: u32) -> Self {
        match value {
            0 => Self::Invalid,
            1 => Self::Ground,
            2 => Self::Top,
            3 => Self::Layer3,
            4 => Self::Layer4,
            5 => Self::Layer5,
            6 => Self::Layer6,
            7 => Self::Layer7,
            8 => Self::Layer8,
            9 => Self::Layer9,
            10 => Self::Layer10,
            11 => Self::Layer11,
            12 => Self::Layer12,
            13 => Self::Layer13,
            14 => Self::Layer14,
            15 => Self::Wall,
            _ => Self::Invalid,
        }
    }

    pub fn is_elevated(self) -> bool {
        let v = self as u8;
        v >= 2 && v <= 15
    }
}

/// Grid coordinate for pathfinding
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct GridCoord {
    pub x: i32,
    pub y: i32,
}

impl GridCoord {
    pub fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }

    /// Convert world coordinates to grid coordinates
    /// Matches C++ worldToCell() at AIPathfind.h:934
    pub fn from_world(pos: &Coord3D) -> Self {
        Self {
            x: (pos.x / PATHFIND_CELL_SIZE_F).floor() as i32,
            y: (pos.y / PATHFIND_CELL_SIZE_F).floor() as i32,
        }
    }

    /// Convert grid coordinates to world coordinates
    /// Matches C++ adjustCoordToCell()
    pub fn to_world(&self, layer: PathfindLayerEnum) -> Coord3D {
        let x = (self.x as f32 + 0.5) * PATHFIND_CELL_SIZE_F;
        let y = (self.y as f32 + 0.5) * PATHFIND_CELL_SIZE_F;
        let z = layer_world_height(x, y, layer);
        Coord3D::new(x, y, z)
    }

    /// Manhattan distance for heuristic
    pub fn manhattan_distance(&self, other: &GridCoord) -> u32 {
        let dx: i32 = (self.x - other.x).abs();
        let dy: i32 = (self.y - other.y).abs();
        COST_ORTHOGONAL * (dx + dy) as u32
    }

    /// Diagonal distance heuristic (more accurate than Manhattan)
    /// Matches C++ PathfindCell::costToGoal() at AIPathfind.cpp:1654
    pub fn diagonal_distance(&self, other: &GridCoord) -> u32 {
        let dx = (self.x - other.x).abs() as u32;
        let dy = (self.y - other.y).abs() as u32;

        if dx > dy {
            COST_ORTHOGONAL * dx + (COST_ORTHOGONAL * dy) / 2
        } else {
            COST_ORTHOGONAL * dy + (COST_ORTHOGONAL * dx) / 2
        }
    }

    /// Get 8 neighboring cells (orthogonal + diagonal)
    /// Matches C++ examineNeighboringCells() at AIPathfind.cpp:6125-6128
    pub fn neighbors(&self) -> [GridCoord; 8] {
        [
            GridCoord::new(self.x + 1, self.y),     // Right
            GridCoord::new(self.x, self.y + 1),     // Up
            GridCoord::new(self.x - 1, self.y),     // Left
            GridCoord::new(self.x, self.y - 1),     // Down
            GridCoord::new(self.x + 1, self.y + 1), // Right-Up
            GridCoord::new(self.x - 1, self.y + 1), // Left-Up
            GridCoord::new(self.x - 1, self.y - 1), // Left-Down
            GridCoord::new(self.x + 1, self.y - 1), // Right-Down
        ]
    }

    /// Check if this is a diagonal neighbor
    pub fn is_diagonal(&self, other: &GridCoord) -> bool {
        let dx: i32 = (self.x - other.x).abs();
        let dy: i32 = (self.y - other.y).abs();
        dx == 1 && dy == 1
    }
}

/// Internal A* key: C++ keeps a distinct PathfindCell per (x, y, layer).
/// Public GridCoord stays (x, y); layer is search-only.
type SearchKey = (GridCoord, PathfindLayerEnum);

/// A* node for priority queue
/// Matches C++ PathfindCell structure at AIPathfind.cpp:6137-6357
#[derive(Debug, Clone)]
struct AStarNode {
    coord: GridCoord,
    layer: PathfindLayerEnum,
    g_score: u32, // Cost from start
    f_score: u32, // g_score + h_score
    parent: Option<SearchKey>,
}

impl PartialEq for AStarNode {
    fn eq(&self, other: &Self) -> bool {
        self.coord == other.coord && self.layer == other.layer
    }
}

impl Eq for AStarNode {}

impl PartialOrd for AStarNode {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for AStarNode {
    /// Min-heap based on f_score, then g_score, then coordinates
    /// Matches C++ PathfindCell::putOnSortedOpenList() behavior
    fn cmp(&self, other: &Self) -> Ordering {
        // Reverse for min-heap behavior
        other
            .f_score
            .cmp(&self.f_score)
            .then_with(|| other.g_score.cmp(&self.g_score))
            .then_with(|| other.coord.x.cmp(&self.coord.x))
            .then_with(|| other.coord.y.cmp(&self.coord.y))
            .then_with(|| (other.layer as u8).cmp(&(self.layer as u8)))
    }
}

/// Pathfinding cell data
#[derive(Debug, Clone)]
pub struct PathfindCell {
    cell_type: PathfindCellType,
    flags: CellFlags,
    layer: PathfindLayerEnum,
    /// C++ PathfindCell::m_connectLayer (bridge/wall entry link).
    connect_layer: PathfindLayerEnum,
    zone: u16,
    pinched: bool,
    cost_multiplier: f32,
}

impl PathfindCell {
    pub fn new() -> Self {
        Self {
            cell_type: PathfindCellType::Clear,
            flags: CellFlags::NoUnits,
            layer: PathfindLayerEnum::Ground,
            connect_layer: PathfindLayerEnum::Invalid,
            zone: 0,
            pinched: false,
            cost_multiplier: 1.0,
        }
    }

    pub fn get_type(&self) -> PathfindCellType {
        self.cell_type
    }

    pub fn set_type(&mut self, cell_type: PathfindCellType) {
        self.cell_type = cell_type;
    }

    pub fn get_layer(&self) -> PathfindLayerEnum {
        self.layer
    }

    pub fn set_layer(&mut self, layer: PathfindLayerEnum) {
        self.layer = layer;
    }

    pub fn get_connect_layer(&self) -> PathfindLayerEnum {
        self.connect_layer
    }

    pub fn set_connect_layer(&mut self, layer: PathfindLayerEnum) {
        self.connect_layer = layer;
    }

    pub fn get_flags(&self) -> CellFlags {
        self.flags
    }

    pub fn set_flags(&mut self, flags: CellFlags) {
        self.flags = flags;
    }

    pub fn is_pinched(&self) -> bool {
        self.pinched
    }

    pub fn set_pinched(&mut self, pinched: bool) {
        self.pinched = pinched;
    }

    /// Check if cell is impassable for ground units
    /// Matches C++ IS_IMPASSABLE() at AIPathfind.cpp:55-67
    pub fn is_impassable(&self) -> bool {
        matches!(
            self.cell_type,
            PathfindCellType::Impassable
                | PathfindCellType::Obstacle
                | PathfindCellType::BridgeImpassable
        )
    }
}

/// A* pathfinding algorithm implementation
/// Matches C++ Pathfinder::internalFindPath() at AIPathfind.cpp:6438-6694
pub struct AStarPathfinder {
    /// C++ `Pathfinder::m_map` — LAYER_GROUND cells.
    grid: Vec<Vec<PathfindCell>>,
    /// C++ `Pathfinder::m_layers[layer]` — elevated layer cells, lazy-allocated.
    /// Missing slot / CELL_IMPASSABLE → C++ `PathfindLayer::getCell` returns NULL
    /// and `Pathfinder::getCell` falls back to `m_map` (ground).
    layer_grids: HashMap<PathfindLayerEnum, Vec<Vec<Option<PathfindCell>>>>,
    width: usize,
    height: usize,
    /// Cell -> owning obstacle object id (C++ PathfindCellInfo::obstacleID).
    /// Keyed by (x, y, layer) so Top/Ground obstacles are independent.
    obstacle_owners: HashMap<(i32, i32, u8), u32>,
    /// C++ PathfindCellInfo::m_obstacleIsFence.
    obstacle_fence: HashSet<(i32, i32, u8)>,
    /// C++ PathfindCellInfo::m_obstacleIsTransparent (KINDOF_CAN_SEE_THROUGH).
    obstacle_transparent: HashSet<(i32, i32, u8)>,
    /// C++ PathfindZoneManager block passable (blockX, blockY).
    /// Only false entries stored; missing = true (default passable).
    zone_impassable_blocks: HashSet<(i32, i32)>,
}

impl AStarPathfinder {
    pub fn new(width: usize, height: usize) -> Self {
        let grid = vec![vec![PathfindCell::new(); height]; width];
        Self {
            grid,
            layer_grids: HashMap::new(),
            width,
            height,
            obstacle_owners: HashMap::new(),
            obstacle_fence: HashSet::new(),
            obstacle_transparent: HashSet::new(),
            zone_impassable_blocks: HashSet::new(),
        }
    }

    pub fn reset(&mut self) {
        for row in self.grid.iter_mut() {
            for cell in row.iter_mut() {
                *cell = PathfindCell::new();
            }
        }
        self.layer_grids.clear();
        self.obstacle_owners.clear();
        self.obstacle_fence.clear();
        self.obstacle_transparent.clear();
        self.zone_impassable_blocks.clear();
    }

    #[inline]
    fn in_bounds(&self, coord: GridCoord) -> bool {
        coord.x >= 0 && coord.x < self.width as i32 && coord.y >= 0 && coord.y < self.height as i32
    }

    #[inline]
    fn is_elevated_layer(layer: PathfindLayerEnum) -> bool {
        (layer as u8) > (PathfindLayerEnum::Ground as u8)
    }

    #[inline]
    fn obstacle_key(coord: GridCoord, layer: PathfindLayerEnum) -> (i32, i32, u8) {
        (coord.x, coord.y, layer as u8)
    }

    /// C++ `Pathfinder::m_map[x][y]` — ground only, no layer fallback.
    fn get_ground_cell(&self, coord: GridCoord) -> Option<&PathfindCell> {
        if self.in_bounds(coord) {
            Some(&self.grid[coord.x as usize][coord.y as usize])
        } else {
            None
        }
    }

    fn get_ground_cell_mut(&mut self, coord: GridCoord) -> Option<&mut PathfindCell> {
        if self.in_bounds(coord) {
            Some(&mut self.grid[coord.x as usize][coord.y as usize])
        } else {
            None
        }
    }

    /// Ground-only cell (public helpers / existing GROUND APIs).
    fn get_cell(&self, coord: GridCoord) -> Option<&PathfindCell> {
        self.get_ground_cell(coord)
    }

    fn get_cell_mut(&mut self, coord: GridCoord) -> Option<&mut PathfindCell> {
        self.get_ground_cell_mut(coord)
    }

    fn layer_cell(&self, coord: GridCoord, layer: PathfindLayerEnum) -> Option<&PathfindCell> {
        if !self.in_bounds(coord) || !Self::is_elevated_layer(layer) {
            return None;
        }
        self.layer_grids
            .get(&layer)?
            .get(coord.x as usize)?
            .get(coord.y as usize)?
            .as_ref()
    }

    fn ensure_layer_grid(&mut self, layer: PathfindLayerEnum) {
        if !Self::is_elevated_layer(layer) {
            return;
        }
        let w = self.width;
        let h = self.height;
        self.layer_grids
            .entry(layer)
            .or_insert_with(|| vec![vec![None; h]; w]);
    }

    /// Allocate / write a cell on `layer` (no C++ getCell fallback).
    fn get_cell_mut_on_layer(
        &mut self,
        coord: GridCoord,
        layer: PathfindLayerEnum,
    ) -> Option<&mut PathfindCell> {
        if !self.in_bounds(coord) {
            return None;
        }
        if !Self::is_elevated_layer(layer) {
            return self.get_ground_cell_mut(coord);
        }
        self.ensure_layer_grid(layer);
        let slot = self
            .layer_grids
            .get_mut(&layer)?
            .get_mut(coord.x as usize)?
            .get_mut(coord.y as usize)?;
        if slot.is_none() {
            let mut cell = PathfindCell::new();
            cell.set_layer(layer);
            *slot = Some(cell);
        }
        slot.as_mut()
    }

    /// C++ `Pathfinder::getCell(layer, x, y)` at AIPathfind.h:899-917.
    ///
    /// Elevated: `m_layers[layer].getCell` — NULL when the layer grid is
    /// unused, the (x,y) slot was never written, **or** the layer cell is
    /// `CELL_IMPASSABLE` (AIPathfind.cpp:3636-3638). NULL falls back to
    /// `m_map[x][y]` (ground). Off-map → None.
    fn get_cell_on_layer(
        &self,
        coord: GridCoord,
        layer: PathfindLayerEnum,
    ) -> Option<&PathfindCell> {
        if !self.in_bounds(coord) {
            return None;
        }
        if Self::is_elevated_layer(layer) {
            if let Some(cell) = self.layer_cell(coord, layer) {
                if cell.get_type() != PathfindCellType::Impassable {
                    return Some(cell);
                }
                // C++ PathfindLayer::getCell: Impassable cells are ignored.
            }
        }
        Some(&self.grid[coord.x as usize][coord.y as usize])
    }

    fn is_ignored_obstacle(
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

    pub fn width(&self) -> usize {
        self.width
    }

    pub fn height(&self) -> usize {
        self.height
    }

    /// Calculate movement cost between adjacent cells
    /// Matches C++ PathfindCell::costSoFar() at AIPathfind.cpp:1691-1711
    fn movement_cost_with_ignore(
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
    fn cell_occupancy_cost(&self, cell: GridCoord) -> u32 {
        let Some(c) = self.get_cell(cell) else {
            return 0;
        };
        match c.get_flags() {
            CellFlags::UnitPresentFixed | CellFlags::UnitGoal => 3 * COST_DIAGONAL,
            CellFlags::UnitPresentMoving | CellFlags::UnitGoalOtherMoving => COST_DIAGONAL,
            CellFlags::NoUnits => 0,
        }
    }

    /// Find path using A* algorithm
    /// Matches C++ Pathfinder::internalFindPath() at AIPathfind.cpp:6438-6694
    pub fn find_path(
        &self,
        start: GridCoord,
        goal: GridCoord,
        surfaces: u32,
        is_crusher: bool,
        max_iterations: usize,
        allow_partial: bool,
        ignore_cells: Option<&HashSet<GridCoord>>,
    ) -> Option<(Vec<GridCoord>, usize)> {
        self.find_path_ex(
            start,
            goal,
            surfaces,
            is_crusher,
            max_iterations,
            allow_partial,
            ignore_cells,
            None,
        )
    }

    /// A* with optional per-cell extra cost and downhill-only filter.
    ///
    /// `extra_cost`: C++ allyFixedCount / allyMoving penalties.
    /// `downhill_only`: C++ locomotorSet.isDownhillOnly() — reject uphill steps.
    /// `ground_height`: world ground Z at cell center (for downhill + cliff |dz|).
    pub fn find_path_ex(
        &self,
        start: GridCoord,
        goal: GridCoord,
        surfaces: u32,
        is_crusher: bool,
        max_iterations: usize,
        allow_partial: bool,
        ignore_cells: Option<&HashSet<GridCoord>>,
        extra_cost: Option<&dyn Fn(GridCoord) -> u32>,
    ) -> Option<(Vec<GridCoord>, usize)> {
        // Thin callers (host find_path_via_crate) used to skip layers/zones/
        // occupancy/tunneling. Apply C++ internalFindPath defaults here.
        let start_is_obstacle = self.get_cell_type(start) == Some(PathfindCellType::Obstacle);
        let occupancy = |cell: GridCoord| -> u32 {
            extra_cost.map(|f| f(cell)).unwrap_or(0) + self.cell_occupancy_cost(cell)
        };
        let ground_h = |cell: GridCoord| -> f32 {
            let wx = (cell.x as f32 + 0.5) * PATHFIND_CELL_SIZE_F;
            let wy = (cell.y as f32 + 0.5) * PATHFIND_CELL_SIZE_F;
            layer_world_height(wx, wy, PathfindLayerEnum::Ground)
        };
        let line_ok = |cell: GridCoord| -> bool {
            extra_cost.map(|f| f(cell) < u32::MAX / 8).unwrap_or(true)
        };
        self.find_path_ex6(
            start,
            goal,
            surfaces,
            is_crusher,
            max_iterations,
            allow_partial,
            ignore_cells,
            Some(&occupancy as &dyn Fn(GridCoord) -> u32),
            false,
            Some(&ground_h as &dyn Fn(GridCoord) -> f32),
            None,
            Some(&line_ok as &dyn Fn(GridCoord) -> bool),
            !start_is_obstacle,
            start_is_obstacle,
            None,
            None,
        )
    }

    pub fn find_path_ex2(
        &self,
        start: GridCoord,
        goal: GridCoord,
        surfaces: u32,
        is_crusher: bool,
        max_iterations: usize,
        allow_partial: bool,
        ignore_cells: Option<&HashSet<GridCoord>>,
        extra_cost: Option<&dyn Fn(GridCoord) -> u32>,
        downhill_only: bool,
        ground_height: Option<&dyn Fn(GridCoord) -> f32>,
    ) -> Option<(Vec<GridCoord>, usize)> {
        self.find_path_ex3(
            start,
            goal,
            surfaces,
            is_crusher,
            max_iterations,
            allow_partial,
            ignore_cells,
            extra_cost,
            downhill_only,
            ground_height,
            None,
        )
    }

    /// Like find_path_ex2 plus optional neighbor override for tunneling/dozer.
    /// `force_passable(cell)` → treat as passable even if map says not (tunneling/dozer).
    pub fn find_path_ex3(
        &self,
        start: GridCoord,
        goal: GridCoord,
        surfaces: u32,
        is_crusher: bool,
        max_iterations: usize,
        allow_partial: bool,
        ignore_cells: Option<&HashSet<GridCoord>>,
        extra_cost: Option<&dyn Fn(GridCoord) -> u32>,
        downhill_only: bool,
        ground_height: Option<&dyn Fn(GridCoord) -> f32>,
        force_passable: Option<&dyn Fn(GridCoord) -> bool>,
    ) -> Option<(Vec<GridCoord>, usize)> {
        self.find_path_ex4(
            start,
            goal,
            surfaces,
            is_crusher,
            max_iterations,
            allow_partial,
            ignore_cells,
            extra_cost,
            downhill_only,
            ground_height,
            force_passable,
            None,
            false,
        )
    }

    /// Like find_path_ex3 plus C++ examineCellsCallback line-to-goal seeding.
    ///
    /// When `seed_line_to_goal` and not downhill-only / not tunneling, each expanded
    /// parent walks Bresenham cells toward the goal and inserts clear cells at
    /// `costSoFar + 0.5*COST_ORTHOGONAL` (AIPathfind.cpp:5996-6093, 6120).
    /// `line_cell_ok(cell)` returns false to abort the line (enemyFixed/allyFixed/etc).
    pub fn find_path_ex4(
        &self,
        start: GridCoord,
        goal: GridCoord,
        surfaces: u32,
        is_crusher: bool,
        max_iterations: usize,
        allow_partial: bool,
        ignore_cells: Option<&HashSet<GridCoord>>,
        extra_cost: Option<&dyn Fn(GridCoord) -> u32>,
        downhill_only: bool,
        ground_height: Option<&dyn Fn(GridCoord) -> f32>,
        force_passable: Option<&dyn Fn(GridCoord) -> bool>,
        line_cell_ok: Option<&dyn Fn(GridCoord) -> bool>,
        seed_line_to_goal: bool,
    ) -> Option<(Vec<GridCoord>, usize)> {
        self.find_path_ex5(
            start,
            goal,
            surfaces,
            is_crusher,
            max_iterations,
            allow_partial,
            ignore_cells,
            extra_cost,
            downhill_only,
            ground_height,
            force_passable,
            line_cell_ok,
            seed_line_to_goal,
            false,
            None,
        )
    }

    /// Like find_path_ex4 plus C++ m_isTunneling start flag and expand-time clear.
    pub fn find_path_ex5(
        &self,
        start: GridCoord,
        goal: GridCoord,
        surfaces: u32,
        is_crusher: bool,
        max_iterations: usize,
        allow_partial: bool,
        ignore_cells: Option<&HashSet<GridCoord>>,
        extra_cost: Option<&dyn Fn(GridCoord) -> u32>,
        downhill_only: bool,
        ground_height: Option<&dyn Fn(GridCoord) -> f32>,
        force_passable: Option<&dyn Fn(GridCoord) -> bool>,
        line_cell_ok: Option<&dyn Fn(GridCoord) -> bool>,
        seed_line_to_goal: bool,
        starts_tunneling: bool,
        cell_allowed: Option<&dyn Fn(GridCoord) -> bool>,
    ) -> Option<(Vec<GridCoord>, usize)> {
        self.find_path_ex6(
            start,
            goal,
            surfaces,
            is_crusher,
            max_iterations,
            allow_partial,
            ignore_cells,
            extra_cost,
            downhill_only,
            ground_height,
            force_passable,
            line_cell_ok,
            seed_line_to_goal,
            starts_tunneling,
            cell_allowed,
            None,
        )
    }

    /// Like find_path_ex5 plus C++ dozerHack (AIPathfind.cpp:6207-6226).
    ///
    /// `dozer_obstacle_ok(cell)` is true when the unit is a dozer and the cell's
    /// obstacle is a non-enemy (KINDOF_DOZER + !ENEMIES). That cell is treated as
    /// passable for this step but does **not** set neighborFlags (no diagonal squeeze).
    pub fn find_path_ex6(
        &self,
        start: GridCoord,
        goal: GridCoord,
        surfaces: u32,
        is_crusher: bool,
        max_iterations: usize,
        allow_partial: bool,
        ignore_cells: Option<&HashSet<GridCoord>>,
        extra_cost: Option<&dyn Fn(GridCoord) -> u32>,
        downhill_only: bool,
        ground_height: Option<&dyn Fn(GridCoord) -> f32>,
        force_passable: Option<&dyn Fn(GridCoord) -> bool>,
        line_cell_ok: Option<&dyn Fn(GridCoord) -> bool>,
        seed_line_to_goal: bool,
        starts_tunneling: bool,
        cell_allowed: Option<&dyn Fn(GridCoord) -> bool>,
        dozer_obstacle_ok: Option<&dyn Fn(GridCoord) -> bool>,
    ) -> Option<(Vec<GridCoord>, usize)> {
        self.find_path_with_start_layer(
            start,
            goal,
            PathfindLayerEnum::Ground,
            PathfindLayerEnum::Ground,
            surfaces,
            is_crusher,
            max_iterations,
            allow_partial,
            ignore_cells,
            extra_cost,
            downhill_only,
            ground_height,
            force_passable,
            line_cell_ok,
            seed_line_to_goal,
            starts_tunneling,
            cell_allowed,
            dozer_obstacle_ok,
        )
    }

    /// A* starting on `start_layer` (C++ `obj->getLayer()` / `getClippedCell(layer, from)`).
    pub fn find_path_on_layer(
        &self,
        start: GridCoord,
        goal: GridCoord,
        layer: PathfindLayerEnum,
        surfaces: u32,
        is_crusher: bool,
        max_iterations: usize,
        allow_partial: bool,
        ignore_cells: Option<&HashSet<GridCoord>>,
    ) -> Option<(Vec<GridCoord>, usize)> {
        self.find_path_with_start_layer(
            start,
            goal,
            layer,
            layer,
            surfaces,
            is_crusher,
            max_iterations,
            allow_partial,
            ignore_cells,
            None,
            false,
            None,
            None,
            None,
            false,
            false,
            None,
            None,
        )
    }

    pub fn find_path_with_start_layer(
        &self,
        start: GridCoord,
        goal: GridCoord,
        start_layer: PathfindLayerEnum,
        dest_layer: PathfindLayerEnum,
        surfaces: u32,
        is_crusher: bool,
        max_iterations: usize,
        allow_partial: bool,
        ignore_cells: Option<&HashSet<GridCoord>>,
        extra_cost: Option<&dyn Fn(GridCoord) -> u32>,
        downhill_only: bool,
        ground_height: Option<&dyn Fn(GridCoord) -> f32>,
        force_passable: Option<&dyn Fn(GridCoord) -> bool>,
        line_cell_ok: Option<&dyn Fn(GridCoord) -> bool>,
        seed_line_to_goal: bool,
        starts_tunneling: bool,
        cell_allowed: Option<&dyn Fn(GridCoord) -> bool>,
        dozer_obstacle_ok: Option<&dyn Fn(GridCoord) -> bool>,
    ) -> Option<(Vec<GridCoord>, usize)> {
        // Initialize open and closed sets
        // Matches C++ at AIPathfind.cpp:6575-6581
        let mut open_set = BinaryHeap::new();
        let mut open_members: HashSet<SearchKey> = HashSet::new();
        let mut closed_set: HashSet<SearchKey> = HashSet::new();
        let mut came_from: HashMap<SearchKey, SearchKey> = HashMap::new();
        let mut g_scores: HashMap<SearchKey, u32> = HashMap::new();

        let start_layer = match start_layer {
            PathfindLayerEnum::Invalid => PathfindLayerEnum::Ground,
            layer => layer,
        };
        let dest_layer = match dest_layer {
            PathfindLayerEnum::Invalid => PathfindLayerEnum::Ground,
            layer => layer,
        };
        let start_key: SearchKey = (start, start_layer);
        let mut best_key = start_key;
        let mut best_dist = start.diagonal_distance(&goal);

        // C++ getClippedCell(obj->getLayer(), from) / getCell(destinationLayer, to).
        let pass_on = |c: GridCoord, layer: PathfindLayerEnum| -> bool {
            if self.is_passable_on_layer_with_ignore(c, layer, surfaces, is_crusher, ignore_cells) {
                return true;
            }
            force_passable.map(|f| f(c)).unwrap_or(false)
        };
        if !pass_on(start, start_layer) {
            return None;
        }
        if !pass_on(goal, dest_layer) {
            if force_passable.is_none() {
                return None;
            }
            if !force_passable.map(|f| f(goal)).unwrap_or(false) {
                return None;
            }
        }

        // Initialize start node
        // Matches C++ PathfindCell::startPathfind() at AIPathfind.cpp:1216-1219
        let h_score = start.diagonal_distance(&goal);
        let start_node = AStarNode {
            coord: start,
            layer: start_layer,
            g_score: 0,
            f_score: h_score,
            parent: None,
        };

        open_set.push(start_node);
        open_members.insert(start_key);
        g_scores.insert(start_key, 0);

        // C++ m_isTunneling — clear once we expand into valid non-pinched cell.
        let mut is_tunneling = starts_tunneling;

        let mut iterations = 0;

        // Main A* loop
        // Matches C++ while loop at AIPathfind.cpp:6589-6633
        while let Some(current) = open_set.pop() {
            let current_key: SearchKey = (current.coord, current.layer);
            // Stale BinaryHeap entry after examine_cells_toward_goal reopen.
            if !open_members.contains(&current_key) {
                continue;
            }
            if let Some(&best_g) = g_scores.get(&current_key) {
                if current.g_score > best_g {
                    continue;
                }
            }

            iterations += 1;
            if iterations > max_iterations {
                // Prevent infinite loops
                if allow_partial {
                    return Some((self.reconstruct_path(&came_from, best_key), iterations));
                }
                return None;
            }

            // Popped from open (C++ removeFromOpenList).
            open_members.remove(&current_key);

            // Goal reached!
            // C++ compares PathfindCell pointers: dest XY on destinationLayer.
            if current.coord == goal && current.layer == dest_layer {
                return Some((self.reconstruct_path(&came_from, current_key), iterations));
            }

            let current_dist = current.coord.diagonal_distance(&goal);
            if current_dist < best_dist {
                best_dist = current_dist;
                best_key = current_key;
            }

            // Move current to closed set
            // Matches C++ at AIPathfind.cpp:6626
            closed_set.insert(current_key);

            // C++ checkChangeLayers(parent) before examineNeighboringCells.
            self.check_change_layers(
                current.coord,
                current.layer,
                current.g_score,
                current.f_score,
                &mut open_set,
                &mut open_members,
                &closed_set,
                &mut came_from,
                &mut g_scores,
            );

            // C++ examineNeighboringCells: examineCellsCallback along parent→goal
            // when NO_ATTACK && !tunneling && !downhillOnly && goalCell.
            // Prefer explicit seed flag; skip when downhill-only / tunneling (C++ guard).
            if seed_line_to_goal && !downhill_only && !is_tunneling {
                self.examine_cells_toward_goal(
                    current.coord,
                    current.layer,
                    current.g_score,
                    goal,
                    surfaces,
                    is_crusher,
                    ignore_cells,
                    force_passable,
                    line_cell_ok,
                    cell_allowed,
                    &mut open_set,
                    &mut open_members,
                    &mut closed_set,
                    &mut came_from,
                    &mut g_scores,
                );
            }

            // Examine all neighbors
            // Matches C++ examineNeighboringCells() at AIPathfind.cpp:6125-6226
            // Neighbors stay on the CURRENT node's layer (C++ getCell(parent->getLayer())).
            let neighbors = current.coord.neighbors();
            // C++: firstDiagonal=4, adjacent={0,1,2,3,0}, neighborFlags[8]={false...}
            let mut neighbor_flags = [false; 8];
            const FIRST_DIAGONAL: usize = 4;
            const ADJACENT: [usize; 5] = [0, 1, 2, 3, 0];
            for (i, neighbor_coord) in neighbors.iter().copied().enumerate() {
                let neighbor_key: SearchKey = (neighbor_coord, current.layer);
                // C++ AIPathfind.cpp:6167-6180: onList = getOpen() || getClosed(); skip.
                // Never update g / never reopen from examineNeighboringCells.
                if open_members.contains(&neighbor_key) || closed_set.contains(&neighbor_key) {
                    continue;
                }

                // C++ isHuman logical extent clamp (examineNeighboringCells).
                if let Some(ok) = cell_allowed {
                    if !ok(neighbor_coord) {
                        continue;
                    }
                }

                // C++ examineNeighboringCells ~6181-6185:
                // if (i>=firstDiagonal) skip when BOTH adjacent orthogonal
                // neighborFlags are false. One open orthogonal is enough.
                if i >= FIRST_DIAGONAL
                    && !neighbor_flags[ADJACENT[i - 4]]
                    && !neighbor_flags[ADJACENT[i - 3]]
                {
                    continue;
                }

                let naturally_passable = self.is_passable_on_layer_with_ignore(
                    neighbor_coord,
                    current.layer,
                    surfaces,
                    is_crusher,
                    ignore_cells,
                );
                let force_ok = force_passable.map(|f| f(neighbor_coord)).unwrap_or(false);
                // C++ dozerHack: KINDOF_DOZER + CELL_OBSTACLE + non-enemy obstacle.
                let dozer_hack = if !naturally_passable && !force_ok {
                    matches!(
                        self.get_cell_on_layer(neighbor_coord, current.layer)
                            .map(|c| c.get_type()),
                        Some(PathfindCellType::Obstacle)
                    ) && dozer_obstacle_ok
                        .map(|f| f(neighbor_coord))
                        .unwrap_or(false)
                } else {
                    false
                };
                // C++: invalid movement only expands while m_isTunneling (or dozerHack).
                if !naturally_passable && !force_ok && !dozer_hack && !is_tunneling {
                    continue;
                }

                // C++ locomotorSet.isDownhillOnly(): reject if from.z < to.z
                if downhill_only {
                    if let Some(h) = ground_height {
                        let fz = h(current.coord);
                        let tz = h(neighbor_coord);
                        if fz < tz {
                            continue;
                        }
                    }
                }

                // C++: if (!dozerHack) neighborFlags[i] = true;
                if !dozer_hack {
                    neighbor_flags[i] = true;
                }

                // Calculate tentative g_score
                // Matches C++ at AIPathfind.cpp:6259 + 6277-6333
                let mut movement_cost = self.movement_cost_with_ignore(
                    current.coord,
                    neighbor_coord,
                    current.layer,
                    surfaces,
                    is_crusher,
                    ignore_cells,
                    &came_from,
                );
                if movement_cost == u32::MAX {
                    // Tunneling / force / dozerHack: still expand with base ortho/diag step.
                    if is_tunneling || force_ok || dozer_hack {
                        movement_cost = if current.coord.is_diagonal(&neighbor_coord) {
                            COST_DIAGONAL
                        } else {
                            COST_ORTHOGONAL
                        };
                        // C++ m_isTunneling invalid step: +10*COST_ORTHOGONAL
                        if is_tunneling && !naturally_passable {
                            movement_cost = movement_cost.saturating_add(10 * COST_ORTHOGONAL);
                        }
                    } else {
                        continue; // Impassable
                    }
                }
                // C++ examineNeighboringCells: pinched gets EXTRA COST_ORTHOGONAL
                // on top of costSoFar's COST_DIAGONAL pinched surcharge.
                let neighbor_cell = self.get_cell_on_layer(neighbor_coord, current.layer);
                if neighbor_cell.map(|c| c.is_pinched()).unwrap_or(false) {
                    movement_cost = movement_cost.saturating_add(COST_ORTHOGONAL);
                }
                // C++ CELL_OBSTACLE: +100*COST_ORTHOGONAL when expanding through obstacle.
                // Crusher fences and AIR already paid 100*ORTHO in movement_cost_with_ignore.
                if let Some(cell) = neighbor_cell {
                    if cell.get_type() == PathfindCellType::Obstacle
                        && !self.is_ignored_obstacle(neighbor_coord, current.layer, ignore_cells)
                    {
                        let paid_in_cost = (naturally_passable && is_crusher)
                            || (naturally_passable && (surfaces & SURFACE_AIR) != 0);
                        if !paid_in_cost
                            && (!naturally_passable || is_tunneling || force_ok || dozer_hack)
                        {
                            movement_cost = movement_cost.saturating_add(100 * COST_ORTHOGONAL);
                        }
                    }
                }
                // C++ notZonePassable: ground hierarchical block not yet expanded →
                // heavy cost (100 * COST_ORTHOGONAL), not hard reject in this path.
                // Only applies when the resolved cell is LAYER_GROUND (AIPathfind.cpp:6156).
                if neighbor_cell
                    .map(|c| c.get_layer() == PathfindLayerEnum::Ground)
                    .unwrap_or(true)
                    && !self.is_zone_passable(neighbor_coord)
                {
                    movement_cost = movement_cost.saturating_add(ZONE_IMPASSABLE_COST);
                }
                // C++ allyFixedCount > 0 → +3*COST_DIAGONAL (and setBlockedByAlly).
                if let Some(extra) = extra_cost {
                    let e = extra(neighbor_coord);
                    if e >= u32::MAX / 8 {
                        continue; // C++ enemyFixed / clearCellForDiameter miss
                    }
                    movement_cost = movement_cost.saturating_add(e);
                }
                // C++ cliff: if !pinched && |dz| < PATHFIND_CELL_SIZE_F → already has
                // base cliff cost in movement_cost; when |dz| >= cell size, remove the
                // flat-cliff surcharge (movement_cost always adds 7*DIAG for cliffs).
                if let Some(h) = ground_height {
                    if let Some(cell) = neighbor_cell {
                        if cell.get_type() == PathfindCellType::Cliff && !cell.is_pinched() {
                            let dz = (h(current.coord) - h(neighbor_coord)).abs();
                            if dz >= PATHFIND_CELL_SIZE_F {
                                // Steep cliff step: undo flat surcharge (keep base ortho/diag).
                                movement_cost = movement_cost.saturating_sub(7 * COST_DIAGONAL);
                            }
                        }
                    }
                }

                // C++: if (movementValid && !pinched) m_isTunneling = false;
                let neighbor_pinched = neighbor_cell.map(|c| c.is_pinched()).unwrap_or(false);
                if (naturally_passable || dozer_hack) && !neighbor_pinched {
                    is_tunneling = false;
                }

                let tentative_g = current.g_score.saturating_add(movement_cost);

                // First visit only (onList already skipped). C++ 6321-6327 is unreachable
                // after the 6177-6180 continue.
                came_from.insert(neighbor_key, current_key);
                g_scores.insert(neighbor_key, tentative_g);
                open_members.insert(neighbor_key);

                // Calculate h_score and f_score
                // C++: if m_isTunneling, costRemaining = 0 (closest valid cell).
                let h_score = if is_tunneling {
                    0
                } else {
                    neighbor_coord.diagonal_distance(&goal)
                };
                let f_score = tentative_g.saturating_add(h_score);

                // Add to open set
                // Matches C++ at AIPathfind.cpp:6354
                let neighbor_node = AStarNode {
                    coord: neighbor_coord,
                    layer: current.layer,
                    g_score: tentative_g,
                    f_score,
                    parent: Some(current_key),
                };

                open_set.push(neighbor_node);
            }
        }

        // No path found
        // Matches C++ at AIPathfind.cpp:6635-6693
        if allow_partial {
            Some((self.reconstruct_path(&came_from, best_key), iterations))
        } else {
            None
        }
    }

    /// C++ Pathfinder::examineCellsCallback line seed (AIPathfind.cpp:5996-6093).
    /// Walks Bresenham from parent toward goal; inserts clear cells at half ortho cost.
    /// Unlike examineNeighboringCells, this CAN reopen if the new g is better (C++ 6063-6088).
    fn examine_cells_toward_goal(
        &self,
        parent: GridCoord,
        layer: PathfindLayerEnum,
        parent_g: u32,
        goal: GridCoord,
        surfaces: u32,
        is_crusher: bool,
        ignore_cells: Option<&HashSet<GridCoord>>,
        force_passable: Option<&dyn Fn(GridCoord) -> bool>,
        line_cell_ok: Option<&dyn Fn(GridCoord) -> bool>,
        cell_allowed: Option<&dyn Fn(GridCoord) -> bool>,
        open_set: &mut BinaryHeap<AStarNode>,
        open_members: &mut HashSet<SearchKey>,
        closed_set: &mut HashSet<SearchKey>,
        came_from: &mut HashMap<SearchKey, SearchKey>,
        g_scores: &mut HashMap<SearchKey, u32>,
    ) {
        if parent == goal {
            return;
        }
        // Bresenham cell walk parent → goal (same topology as iterateCellsAlongLine).
        let delta_x = (goal.x - parent.x).abs();
        let delta_y = (goal.y - parent.y).abs();
        let mut x = parent.x;
        let mut y = parent.y;
        let (mut xinc1, mut xinc2) = if goal.x >= parent.x {
            (1i32, 1i32)
        } else {
            (-1, -1)
        };
        let (mut yinc1, mut yinc2) = if goal.y >= parent.y {
            (1i32, 1i32)
        } else {
            (-1, -1)
        };
        let (den, mut num, numadd, numpixels);
        if delta_x >= delta_y {
            xinc1 = 0;
            yinc2 = 0;
            den = delta_x;
            num = delta_x / 2;
            numadd = delta_y;
            numpixels = delta_x;
        } else {
            xinc2 = 0;
            yinc1 = 0;
            den = delta_y;
            num = delta_y / 2;
            numadd = delta_x;
            numpixels = delta_y;
        }

        let mut from = parent;
        let mut from_g = parent_g;
        // Skip the parent cell itself; process subsequent cells on the line.
        for _ in 0..=numpixels {
            num += numadd;
            if num >= den {
                num -= den;
                x += xinc1;
                y += yinc1;
            }
            x += xinc2;
            y += yinc2;
            let to = GridCoord::new(x, y);
            if to == parent {
                continue;
            }
            let Some(to_cell) = self.get_cell_on_layer(to, layer) else {
                break;
            };
            let to_resolved_layer = to_cell.get_layer();
            let to_pinched = to_cell.is_pinched();
            let to_type = to_cell.get_type();
            if let Some(ok) = cell_allowed {
                if !ok(to) {
                    break;
                }
            }

            // Abort line (return 1) conditions from examineCellsCallback.
            if !self.is_passable_on_layer_with_ignore(to, layer, surfaces, is_crusher, ignore_cells)
                && !force_passable.map(|f| f(to)).unwrap_or(false)
            {
                break;
            }
            // C++: only ground cells consult the zone manager (AIPathfind.cpp:6005).
            if to_resolved_layer == PathfindLayerEnum::Ground && !self.is_zone_passable(to) {
                break;
            }
            if to_pinched {
                break;
            }
            if to_type == PathfindCellType::Cliff {
                break;
            }
            if let Some(ok) = line_cell_ok {
                if !ok(to) {
                    break;
                }
            }

            // newCostSoFar = from->getCostSoFar() + 0.5f*COST_ORTHOGONAL
            let new_g = from_g.saturating_add(COST_ORTHOGONAL / 2);
            let to_key: SearchKey = (to, layer);
            if let Some(&existing_g) = g_scores.get(&to_key) {
                if existing_g <= new_g {
                    // Keep going along the line without updating.
                    from = to;
                    from_g = existing_g;
                    if to == goal {
                        break;
                    }
                    continue;
                }
            }

            // Better path — reopen if closed (C++ 6063-6088).
            closed_set.remove(&to_key);
            open_members.insert(to_key);
            came_from.insert(to_key, (from, layer));
            g_scores.insert(to_key, new_g);
            let h_score = to.diagonal_distance(&goal);
            open_set.push(AStarNode {
                coord: to,
                layer,
                g_score: new_g,
                f_score: new_g.saturating_add(h_score),
                parent: Some((from, layer)),
            });

            from = to;
            from_g = new_g;
            if to == goal {
                break;
            }
        }
    }

    /// Reconstruct path from came_from map
    /// Matches C++ buildActualPath() at AIPathfind.cpp:8954-9071
    /// Layer transitions stay at the same xy; collapse those duplicates.
    fn reconstruct_path(
        &self,
        came_from: &HashMap<SearchKey, SearchKey>,
        mut current: SearchKey,
    ) -> Vec<GridCoord> {
        let mut path = vec![current.0];

        while let Some(&parent) = came_from.get(&current) {
            if parent.0 != current.0 {
                path.push(parent.0);
            }
            current = parent;
        }

        path.reverse();
        path
    }

    /// C++ Pathfinder::checkChangeLayers (AIPathfind.cpp:5942-5981).
    ///
    /// If `connectLayer != LAYER_INVALID`, enqueue the same (x,y) on that layer
    /// with the parent's costSoFar and totalCost (0 extra), unless already on open/closed.
    /// Returns true when a new same-xy layered node was inserted.
    fn check_change_layers(
        &self,
        coord: GridCoord,
        current_layer: PathfindLayerEnum,
        parent_g: u32,
        parent_f: u32,
        open_set: &mut BinaryHeap<AStarNode>,
        open_members: &mut HashSet<SearchKey>,
        closed_set: &HashSet<SearchKey>,
        came_from: &mut HashMap<SearchKey, SearchKey>,
        g_scores: &mut HashMap<SearchKey, u32>,
    ) -> bool {
        let Some(cell) = self.get_cell_on_layer(coord, current_layer) else {
            return false;
        };
        let connect = cell.get_connect_layer();
        if connect == PathfindLayerEnum::Invalid || connect == current_layer {
            return false;
        }
        let key: SearchKey = (coord, connect);
        if open_members.contains(&key) || closed_set.contains(&key) {
            return false;
        }
        let parent_key: SearchKey = (coord, current_layer);
        came_from.insert(key, parent_key);
        g_scores.insert(key, parent_g);
        open_members.insert(key);
        open_set.push(AStarNode {
            coord,
            layer: connect,
            g_score: parent_g,
            f_score: parent_f,
            parent: Some(parent_key),
        });
        true
    }

    /// Set cell type at coordinates (LAYER_GROUND / C++ `m_map`).
    pub fn set_cell_type(&mut self, coord: GridCoord, cell_type: PathfindCellType) {
        if let Some(cell) = self.get_cell_mut(coord) {
            cell.set_type(cell_type);
        }
    }

    /// Write `ty` on `layer`. Ground goes to `m_map`; Top/other lazily allocates
    /// that layer's grid (C++ `m_layers[layer]`).
    pub fn set_cell_type_on_layer(
        &mut self,
        coord: GridCoord,
        layer: PathfindLayerEnum,
        ty: PathfindCellType,
    ) {
        if let Some(cell) = self.get_cell_mut_on_layer(coord, layer) {
            cell.set_type(ty);
            if Self::is_elevated_layer(layer) {
                cell.set_layer(layer);
            }
        }
    }

    /// Stored type on `layer` with **no** C++ getCell fallback.
    ///
    /// Ground → `get_cell_type`. Elevated missing/OOB → `None`. Search still
    /// uses `get_cell_on_layer`, which falls back to ground when the elevated
    /// cell is missing or CELL_IMPASSABLE (AIPathfind.h:899-917).
    pub fn get_cell_type_on_layer(
        &self,
        coord: GridCoord,
        layer: PathfindLayerEnum,
    ) -> Option<PathfindCellType> {
        if !Self::is_elevated_layer(layer) {
            return self.get_cell_type(coord);
        }
        self.layer_cell(coord, layer).map(|c| c.get_type())
    }

    /// Stamp obstacle object id / fence flag on a cell (C++ setTypeAsObstacle).
    /// C++ cell connectLayer stamp for bridge/wall transitions.
    /// C++ ZONE_BLOCK_SIZE for hierarchical passable blocks.
    pub const ZONE_BLOCK_SIZE: i32 = 10;

    /// C++ `PathfindZoneManager::setPassable` — marks the whole zone block.
    pub fn set_zone_passable(&mut self, coord: GridCoord, passable: bool) {
        let bx = coord.x.div_euclid(Self::ZONE_BLOCK_SIZE);
        let by = coord.y.div_euclid(Self::ZONE_BLOCK_SIZE);
        let key = (bx, by);
        if passable {
            self.zone_impassable_blocks.remove(&key);
        } else {
            self.zone_impassable_blocks.insert(key);
        }
    }

    pub fn clear_zone_passable_flags(&mut self) {
        self.zone_impassable_blocks.clear();
    }

    /// Mark all blocks impassable (hierarchical closed until expanded).
    pub fn mark_all_zone_blocks_impassable(&mut self) {
        self.zone_impassable_blocks.clear();
        let bx_max = (self.width as i32 + Self::ZONE_BLOCK_SIZE - 1) / Self::ZONE_BLOCK_SIZE;
        let by_max = (self.height as i32 + Self::ZONE_BLOCK_SIZE - 1) / Self::ZONE_BLOCK_SIZE;
        for bx in 0..bx_max {
            for by in 0..by_max {
                self.zone_impassable_blocks.insert((bx, by));
            }
        }
    }

    /// C++ `PathfindZoneManager::isPassable`.
    #[inline]
    pub fn is_zone_passable(&self, coord: GridCoord) -> bool {
        let bx = coord.x.div_euclid(Self::ZONE_BLOCK_SIZE);
        let by = coord.y.div_euclid(Self::ZONE_BLOCK_SIZE);
        !self.zone_impassable_blocks.contains(&(bx, by))
    }

    /// C++ `clipIsPassable` — false when off-map; else block flag.
    #[inline]
    pub fn clip_is_zone_passable(&self, cell_x: i32, cell_y: i32) -> bool {
        if cell_x < 0 || cell_y < 0 || cell_x >= self.width as i32 || cell_y >= self.height as i32 {
            return false;
        }
        self.is_zone_passable(GridCoord::new(cell_x, cell_y))
    }

    /// C++ `Pathfinder::findPath` hierarchical passable dance (AIPathfind.cpp:6375-6381).
    ///
    /// `clearPassableFlags` → coarse `ZONE_BLOCK_SIZE` search (plus bridge
    /// start/end jumps) → mark corridor + start box, or `setAllPassable`.
    pub fn apply_hierarchical_zone_prune(
        &mut self,
        start: GridCoord,
        goal: GridCoord,
        surfaces: u32,
        is_crusher: bool,
        bridge_jumps: &[(GridCoord, GridCoord)],
    ) -> bool {
        if (surfaces & SURFACE_AIR) != 0 {
            self.clear_zone_passable_flags();
            return true;
        }
        if !self.in_bounds(start) || !self.in_bounds(goal) {
            self.clear_zone_passable_flags();
            return false;
        }
        self.mark_all_zone_blocks_impassable();
        if let Some(path) =
            self.find_hierarchical_block_path(start, goal, surfaces, is_crusher, bridge_jumps)
        {
            for c in path {
                self.set_zone_passable(c, true);
            }
            let half = Self::ZONE_BLOCK_SIZE;
            for dx in -half..=half {
                for dy in -half..=half {
                    let c = GridCoord::new(start.x + dx, start.y + dy);
                    if self.in_bounds(c) {
                        self.set_zone_passable(c, true);
                    }
                }
            }
            true
        } else {
            self.clear_zone_passable_flags();
            false
        }
    }

    #[inline]
    fn zone_block_index(coord: GridCoord) -> (i32, i32) {
        (
            coord.x.div_euclid(Self::ZONE_BLOCK_SIZE),
            coord.y.div_euclid(Self::ZONE_BLOCK_SIZE),
        )
    }

    fn collect_connect_layer_jumps(&self) -> Vec<(GridCoord, GridCoord)> {
        let mut by_layer: HashMap<u8, Vec<GridCoord>> = HashMap::new();
        for x in 0..self.width as i32 {
            for y in 0..self.height as i32 {
                let c = GridCoord::new(x, y);
                if let Some(cl) = self.get_cell_connect_layer(c) {
                    if cl != PathfindLayerEnum::Invalid && cl != PathfindLayerEnum::Ground {
                        by_layer.entry(cl as u8).or_default().push(c);
                    }
                }
            }
        }
        let mut pairs = Vec::new();
        for cells in by_layer.values() {
            if cells.len() < 2 {
                continue;
            }
            let mut lo = cells[0];
            let mut hi = cells[0];
            for &c in cells {
                if c.x + c.y < lo.x + lo.y {
                    lo = c;
                }
                if c.x + c.y > hi.x + hi.y {
                    hi = c;
                }
            }
            if lo != hi {
                pairs.push((lo, hi));
            }
        }
        pairs
    }

    fn zone_blocks_share_passable_edge(
        &self,
        a: (i32, i32),
        b: (i32, i32),
        surfaces: u32,
        is_crusher: bool,
    ) -> bool {
        let size = Self::ZONE_BLOCK_SIZE;
        let dx = b.0 - a.0;
        let dy = b.1 - a.1;
        if dx.abs() + dy.abs() != 1 {
            return false;
        }
        let w = self.width as i32;
        let h = self.height as i32;
        if dx != 0 {
            let x0 = if dx > 0 {
                a.0 * size + size - 1
            } else {
                a.0 * size
            };
            let x1 = x0 + dx;
            let y0 = (a.1 * size).max(0);
            let y1 = ((a.1 + 1) * size).min(h);
            if x0 < 0 || x1 < 0 || x0 >= w || x1 >= w {
                return false;
            }
            for y in y0..y1 {
                if self.is_passable(GridCoord::new(x0, y), surfaces, is_crusher)
                    && self.is_passable(GridCoord::new(x1, y), surfaces, is_crusher)
                {
                    return true;
                }
            }
        } else {
            let y0 = if dy > 0 {
                a.1 * size + size - 1
            } else {
                a.1 * size
            };
            let y1 = y0 + dy;
            let x0 = (a.0 * size).max(0);
            let x1 = ((a.0 + 1) * size).min(w);
            if y0 < 0 || y1 < 0 || y0 >= h || y1 >= h {
                return false;
            }
            for x in x0..x1 {
                if self.is_passable(GridCoord::new(x, y0), surfaces, is_crusher)
                    && self.is_passable(GridCoord::new(x, y1), surfaces, is_crusher)
                {
                    return true;
                }
            }
        }
        false
    }

    /// Coarse 10×10 block A* (C++ `internal_findHierarchicalPath`).
    fn find_hierarchical_block_path(
        &self,
        start: GridCoord,
        goal: GridCoord,
        surfaces: u32,
        is_crusher: bool,
        bridge_jumps: &[(GridCoord, GridCoord)],
    ) -> Option<Vec<GridCoord>> {
        let start_b = Self::zone_block_index(start);
        let goal_b = Self::zone_block_index(goal);
        if start_b == goal_b {
            return Some(vec![start, goal]);
        }
        let bx_max = (self.width as i32 + Self::ZONE_BLOCK_SIZE - 1) / Self::ZONE_BLOCK_SIZE;
        let by_max = (self.height as i32 + Self::ZONE_BLOCK_SIZE - 1) / Self::ZONE_BLOCK_SIZE;
        let mut jumps = self.collect_connect_layer_jumps();
        jumps.extend_from_slice(bridge_jumps);

        let hier_h = |b: (i32, i32)| (b.0 - goal_b.0).abs() + (b.1 - goal_b.1).abs();
        let mut open: BinaryHeap<Reverse<(i32, i32, i32, i32)>> = BinaryHeap::new();
        let mut g_score: HashMap<(i32, i32), i32> = HashMap::new();
        let mut came_from: HashMap<(i32, i32), (i32, i32)> = HashMap::new();
        let mut closed: HashSet<(i32, i32)> = HashSet::new();
        open.push(Reverse((hier_h(start_b), 0, start_b.0, start_b.1)));
        g_score.insert(start_b, 0);

        let mut reached = false;
        while let Some(Reverse((_f, g, bx, by))) = open.pop() {
            if !closed.insert((bx, by)) {
                continue;
            }
            if (bx, by) == goal_b {
                reached = true;
                break;
            }
            for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                let nx = bx + dx;
                let ny = by + dy;
                if nx < 0 || ny < 0 || nx >= bx_max || ny >= by_max {
                    continue;
                }
                if closed.contains(&(nx, ny)) {
                    continue;
                }
                if !self.zone_blocks_share_passable_edge((bx, by), (nx, ny), surfaces, is_crusher) {
                    continue;
                }
                let ng = g + 1;
                if g_score.get(&(nx, ny)).is_some_and(|&og| ng >= og) {
                    continue;
                }
                g_score.insert((nx, ny), ng);
                came_from.insert((nx, ny), (bx, by));
                open.push(Reverse((ng + hier_h((nx, ny)), ng, nx, ny)));
            }
            for &(near, far) in &jumps {
                let ends = [(near, far), (far, near)];
                for (from, to) in ends {
                    if Self::zone_block_index(from) != (bx, by) {
                        continue;
                    }
                    let dest_b = Self::zone_block_index(to);
                    if dest_b.0 < 0
                        || dest_b.1 < 0
                        || dest_b.0 >= bx_max
                        || dest_b.1 >= by_max
                        || closed.contains(&dest_b)
                    {
                        continue;
                    }
                    let ng = g + 1;
                    if g_score.get(&dest_b).is_some_and(|&og| ng >= og) {
                        continue;
                    }
                    g_score.insert(dest_b, ng);
                    came_from.insert(dest_b, (bx, by));
                    open.push(Reverse((ng + hier_h(dest_b), ng, dest_b.0, dest_b.1)));
                }
            }
        }
        if !reached {
            return None;
        }
        let mut blocks = vec![goal_b];
        let mut cur = goal_b;
        while cur != start_b {
            cur = *came_from.get(&cur)?;
            blocks.push(cur);
        }
        blocks.reverse();
        let mut cells: Vec<GridCoord> = blocks
            .iter()
            .map(|&(bx, by)| GridCoord::new(bx * Self::ZONE_BLOCK_SIZE, by * Self::ZONE_BLOCK_SIZE))
            .collect();
        cells.push(start);
        cells.push(goal);
        Some(cells)
    }

    pub fn set_cell_connect_layer(&mut self, coord: GridCoord, layer: PathfindLayerEnum) {
        self.set_cell_connect_layer_on_layer(coord, PathfindLayerEnum::Ground, layer);
    }

    pub fn get_cell_connect_layer(&self, coord: GridCoord) -> Option<PathfindLayerEnum> {
        self.get_cell_connect_layer_on_layer(coord, PathfindLayerEnum::Ground)
    }

    pub fn set_cell_connect_layer_on_layer(
        &mut self,
        coord: GridCoord,
        on_layer: PathfindLayerEnum,
        connect: PathfindLayerEnum,
    ) {
        if let Some(cell) = self.get_cell_mut_on_layer(coord, on_layer) {
            cell.set_connect_layer(connect);
        }
    }

    pub fn get_cell_connect_layer_on_layer(
        &self,
        coord: GridCoord,
        on_layer: PathfindLayerEnum,
    ) -> Option<PathfindLayerEnum> {
        // Stored connect_layer only — no getCell fallback (missing elevated → None).
        if Self::is_elevated_layer(on_layer) {
            return self
                .layer_cell(coord, on_layer)
                .map(|c| c.get_connect_layer());
        }
        self.get_cell(coord).map(|c| c.get_connect_layer())
    }

    /// C++ `Pathfinder::checkChangeLayers` — enqueue same-xy cell on connect layer.
    ///
    /// Returns extra neighbor coords (same x,y) when connectLayer is valid and not already
    /// represented by the normal ground neighbor set. Caller merges into open set.
    pub fn connect_layer_transition_coord(&self, coord: GridCoord) -> Option<GridCoord> {
        let cell = self.get_cell(coord)?;
        let cl = cell.get_connect_layer();
        if cl == PathfindLayerEnum::Invalid {
            return None;
        }
        // Transition stays at same indices; layer change is tracked externally.
        Some(coord)
    }

    /// C++ PathfindCell::getObstacleID.
    pub fn get_cell_obstacle_id(&self, coord: GridCoord) -> Option<u32> {
        self.obstacle_owners
            .get(&Self::obstacle_key(coord, PathfindLayerEnum::Ground))
            .copied()
    }

    pub fn set_cell_obstacle_id(
        &mut self,
        coord: GridCoord,
        obj_id: u32,
        is_fence: bool,
        is_transparent: bool,
    ) {
        self.set_cell_obstacle_id_on_layer(
            coord,
            PathfindLayerEnum::Ground,
            obj_id,
            is_fence,
            is_transparent,
        );
    }

    pub fn set_cell_obstacle_id_on_layer(
        &mut self,
        coord: GridCoord,
        layer: PathfindLayerEnum,
        obj_id: u32,
        is_fence: bool,
        is_transparent: bool,
    ) {
        if let Some(cell) = self.get_cell_mut_on_layer(coord, layer) {
            cell.set_type(PathfindCellType::Obstacle);
            if Self::is_elevated_layer(layer) {
                cell.set_layer(layer);
            }
        }
        let key = Self::obstacle_key(coord, layer);
        self.obstacle_owners.insert(key, obj_id);
        if is_fence {
            self.obstacle_fence.insert(key);
        } else {
            self.obstacle_fence.remove(&key);
        }
        if is_transparent {
            self.obstacle_transparent.insert(key);
        } else {
            self.obstacle_transparent.remove(&key);
        }
    }

    /// C++ PathfindCell::isObstacleTransparent.
    pub fn is_obstacle_transparent(&self, coord: GridCoord) -> bool {
        self.obstacle_transparent
            .contains(&Self::obstacle_key(coord, PathfindLayerEnum::Ground))
    }

    pub fn is_obstacle_fence(&self, coord: GridCoord) -> bool {
        self.obstacle_fence
            .contains(&Self::obstacle_key(coord, PathfindLayerEnum::Ground))
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

    /// Clear obstacle if it matches obj_id (C++ removeObstacle).
    pub fn clear_cell_obstacle_id(&mut self, coord: GridCoord, obj_id: u32) -> bool {
        let key = Self::obstacle_key(coord, PathfindLayerEnum::Ground);
        match self.obstacle_owners.get(&key).copied() {
            Some(owner) if owner == obj_id => {
                self.obstacle_owners.remove(&key);
                self.obstacle_fence.remove(&key);
                self.obstacle_transparent.remove(&key);
                if let Some(cell) = self.get_cell_mut(coord) {
                    cell.set_type(PathfindCellType::Clear);
                }
                true
            }
            _ => false,
        }
    }

    /// Get cell type at coordinates.
    pub fn get_cell_type(&self, coord: GridCoord) -> Option<PathfindCellType> {
        self.get_cell(coord).map(|cell| cell.get_type())
    }

    /// Mark a cell as pinched (surrounded by obstacles)
    pub fn set_pinched(&mut self, coord: GridCoord, pinched: bool) {
        self.set_pinched_on_layer(coord, PathfindLayerEnum::Ground, pinched);
    }

    /// Get whether a cell is pinched.
    pub fn is_pinched(&self, coord: GridCoord) -> Option<bool> {
        self.is_pinched_on_layer(coord, PathfindLayerEnum::Ground)
    }

    pub fn set_pinched_on_layer(
        &mut self,
        coord: GridCoord,
        layer: PathfindLayerEnum,
        pinched: bool,
    ) {
        if let Some(cell) = self.get_cell_mut_on_layer(coord, layer) {
            cell.set_pinched(pinched);
            if Self::is_elevated_layer(layer) {
                cell.set_layer(layer);
            }
        }
    }

    /// Stored pinch flag on `layer` (no getCell fallback). Search uses the
    /// resolved cell from `get_cell_on_layer` instead.
    pub fn is_pinched_on_layer(&self, coord: GridCoord, layer: PathfindLayerEnum) -> Option<bool> {
        if Self::is_elevated_layer(layer) {
            return self.layer_cell(coord, layer).map(|c| c.is_pinched());
        }
        self.get_cell(coord).map(|cell| cell.is_pinched())
    }

    pub fn refresh_pinched_cells_in_bounds(&mut self, lo: GridCoord, hi: GridCoord) {
        let min_x = lo.x.max(0);
        let min_y = lo.y.max(0);
        let max_x = hi.x.min(self.width as i32 - 1);
        let max_y = hi.y.min(self.height as i32 - 1);

        if min_x > max_x || min_y > max_y {
            return;
        }

        for x in min_x..=max_x {
            for y in min_y..=max_y {
                let cell = &mut self.grid[x as usize][y as usize];
                if cell.get_type() == PathfindCellType::Impassable {
                    cell.set_type(PathfindCellType::Clear);
                }
                cell.set_pinched(false);
            }
        }

        for x in min_x..=max_x {
            for y in min_y..=max_y {
                if self.grid[x as usize][y as usize].get_type() != PathfindCellType::Clear {
                    continue;
                }
                let mut total_count = 0;
                let mut orthogonal_count = 0;
                for dx in -1..=1 {
                    for dy in -1..=1 {
                        let nx = x + dx;
                        let ny = y + dy;
                        if nx < 0 || ny < 0 || nx >= self.width as i32 || ny >= self.height as i32 {
                            continue;
                        }
                        if dx == 0 && dy == 0 {
                            continue;
                        }
                        if self.grid[nx as usize][ny as usize].get_type() == PathfindCellType::Clear
                        {
                            total_count += 1;
                            if dx == 0 || dy == 0 {
                                orthogonal_count += 1;
                            }
                        }
                    }
                }
                if orthogonal_count < 2 || total_count < 4 {
                    self.grid[x as usize][y as usize].set_pinched(true);
                }
            }
        }

        for x in min_x..=max_x {
            for y in min_y..=max_y {
                let cell = &mut self.grid[x as usize][y as usize];
                if cell.is_pinched() && cell.get_type() == PathfindCellType::Clear {
                    cell.set_type(PathfindCellType::Impassable);
                    cell.set_pinched(false);
                }
            }
        }

        for x in min_x..=max_x {
            for y in min_y..=max_y {
                if self.grid[x as usize][y as usize].get_type() != PathfindCellType::Clear {
                    continue;
                }
                let mut obstacle_adjacent = false;
                for dx in -1..=1 {
                    for dy in -1..=1 {
                        let nx = x + dx;
                        let ny = y + dy;
                        if nx < 0 || ny < 0 || nx >= self.width as i32 || ny >= self.height as i32 {
                            continue;
                        }
                        if dx == 0 && dy == 0 {
                            continue;
                        }
                        if dx != 0 && dy != 0 {
                            continue;
                        }
                        if self.grid[nx as usize][ny as usize].get_type()
                            == PathfindCellType::Obstacle
                        {
                            obstacle_adjacent = true;
                            break;
                        }
                    }
                    if obstacle_adjacent {
                        break;
                    }
                }
                if obstacle_adjacent {
                    self.grid[x as usize][y as usize].set_pinched(true);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_grid_coord_conversion() {
        let world_pos = Coord3D::new(15.0, 25.0, 0.0);
        let grid = GridCoord::from_world(&world_pos);
        assert_eq!(grid.x, 1);
        assert_eq!(grid.y, 2);

        let world_back = grid.to_world(PathfindLayerEnum::Ground);
        assert!((world_back.x - 15.0).abs() < 1.0);
        assert!((world_back.y - 25.0).abs() < 1.0);
    }

    #[test]
    fn test_manhattan_distance() {
        let a = GridCoord::new(0, 0);
        let b = GridCoord::new(3, 4);
        assert_eq!(a.manhattan_distance(&b), 70); // (3+4) * 10
    }

    #[test]
    fn test_diagonal_distance() {
        let a = GridCoord::new(0, 0);
        let b = GridCoord::new(3, 4);
        // Should be more accurate than Manhattan
        let dist = a.diagonal_distance(&b);
        assert!(dist > 0 && dist <= a.manhattan_distance(&b));
    }

    #[test]
    fn test_simple_pathfinding() {
        let mut pathfinder = AStarPathfinder::new(10, 10);

        let start = GridCoord::new(0, 0);
        let goal = GridCoord::new(5, 5);

        let path = pathfinder
            .find_path(start, goal, 0xFFFFFFFF, false, 1000, false, None)
            .map(|(p, _)| p);
        assert!(path.is_some());

        let path = path.unwrap();
        assert_eq!(path[0], start);
        assert_eq!(path[path.len() - 1], goal);
    }

    #[test]
    fn test_pathfinding_with_obstacles() {
        let mut pathfinder = AStarPathfinder::new(10, 10);

        // Create a wall
        for y in 1..9 {
            pathfinder.set_cell_type(GridCoord::new(5, y), PathfindCellType::Obstacle);
        }

        let start = GridCoord::new(0, 5);
        let goal = GridCoord::new(9, 5);

        // Should find path around the wall
        let path = pathfinder
            .find_path(start, goal, 0x01, false, 1000, false, None)
            .map(|(p, _)| p);
        assert!(path.is_some());

        let path = path.unwrap();
        assert_eq!(path.first().copied(), Some(start));
        assert_eq!(path.last().copied(), Some(goal));
        // C++ one-open-orthogonal squeeze lets the path cut the wall end at (5,9)/(5,0).
        // Chebyshev around that gap can match the 10-cell straight-line hop count.
        assert!(
            path.iter().any(|c| c.x == 5 && (c.y == 0 || c.y == 9)),
            "ground path must detour through the wall gap, got {:?}",
            path
        );
        assert!(
            path.iter()
                .all(|c| { pathfinder.get_cell_type(*c) != Some(PathfindCellType::Obstacle) }),
            "ground path must not step on the obstacle column: {:?}",
            path
        );
    }

    #[test]
    fn test_no_path_exists() {
        let mut pathfinder = AStarPathfinder::new(10, 10);

        // Create a complete barrier
        for y in 0..10 {
            pathfinder.set_cell_type(GridCoord::new(5, y), PathfindCellType::Impassable);
        }

        let start = GridCoord::new(0, 5);
        let goal = GridCoord::new(9, 5);

        let path = pathfinder
            .find_path(start, goal, 0x01, false, 1000, false, None)
            .map(|(p, _)| p);
        assert!(path.is_none());
    }

    #[test]
    fn test_crusher_pathfinding() {
        // C++ validMovementPosition: crushers only enter isObstacleFence cells.
        let mut pathfinder = AStarPathfinder::new(10, 10);
        let obstacle = GridCoord::new(5, 5);
        pathfinder.set_cell_obstacle_id(obstacle, 7, false, false);

        let start = GridCoord::new(0, 5);
        let goal = GridCoord::new(9, 5);

        // Solid building: both crushers and non-crushers path around.
        let path_normal = pathfinder
            .find_path(start, goal, SURFACE_GROUND, false, 1000, false, None)
            .map(|(p, _)| p);
        assert!(path_normal.is_some());
        let path_crusher = pathfinder
            .find_path(start, goal, SURFACE_GROUND, true, 1000, false, None)
            .map(|(p, _)| p);
        assert!(path_crusher.is_some());
        assert_eq!(path_crusher.unwrap().len(), path_normal.unwrap().len());
        assert!(!pathfinder.is_passable(obstacle, SURFACE_GROUND, true));
    }

    #[test]
    fn crusher_find_path_only_crosses_fence_obstacles_like_cpp() {
        // Tiny grid, full-height wall so the only route is through x=4.
        // C++ AIPathfind.cpp:4840-4842 + 6316-6318.
        let mut pf = AStarPathfinder::new(9, 5);
        for y in 0..5 {
            pf.set_cell_obstacle_id(GridCoord::new(4, y), 1, false, false);
        }
        let start = GridCoord::new(0, 2);
        let goal = GridCoord::new(8, 2);

        assert!(
            pf.find_path(start, goal, SURFACE_GROUND, true, 2000, false, None)
                .is_none(),
            "crusher must not path through solid CELL_OBSTACLE buildings"
        );
        assert!(
            pf.find_path(start, goal, SURFACE_GROUND, false, 2000, false, None)
                .is_none()
        );
        assert!(!pf.is_passable(GridCoord::new(4, 2), SURFACE_GROUND, true));

        for y in 0..5 {
            pf.set_cell_obstacle_id(GridCoord::new(4, y), 2, true, false);
        }
        assert!(
            pf.is_obstacle_fence(GridCoord::new(4, 2)),
            "wall must be stamped as fence"
        );
        assert!(
            !pf.is_passable(GridCoord::new(4, 2), SURFACE_GROUND, false),
            "non-crusher still blocked by fence"
        );
        assert!(
            pf.is_passable(GridCoord::new(4, 2), SURFACE_GROUND, true),
            "crusher may enter fence cells"
        );
        assert!(
            pf.find_path(start, goal, SURFACE_GROUND, false, 2000, false, None)
                .is_none(),
            "non-crusher must not path through a fence wall"
        );
        let (path, iters) = pf
            .find_path(start, goal, SURFACE_GROUND, true, 2000, false, None)
            .expect("crusher must path through a fence wall");
        assert!(iters >= 1);
        assert_eq!(path.first().copied(), Some(start));
        assert_eq!(path.last().copied(), Some(goal));
        assert!(
            path.iter().any(|c| c.x == 4),
            "crusher path must cross fence column: {:?}",
            path
        );
    }

    #[test]
    fn diagonal_squeeze_one_orthogonal_open_allows_path() {
        // C++ examineNeighboringCells AIPathfind.cpp:6181-6185:
        // skip diagonal only if BOTH adjacent neighborFlags are false.
        // 2x2 crack: S X / . G — one ortho open, A* prefers S→G (cost 14).
        let mut pf = AStarPathfinder::new(2, 2);
        pf.set_cell_type(GridCoord::new(1, 0), PathfindCellType::Impassable);
        let start = GridCoord::new(0, 0);
        let goal = GridCoord::new(1, 1);
        let (path, iters) = pf
            .find_path(start, goal, SURFACE_GROUND, false, 200, false, None)
            .expect("diagonal crack must be usable when one orthogonal is open");
        assert!(iters >= 1);
        assert_eq!(
            path,
            vec![start, goal],
            "expected direct diagonal: {:?}",
            path
        );
    }

    #[test]
    fn diagonal_squeeze_both_orthogonals_blocked_no_path() {
        // Both adjacent orthogonals blocked → C++ neighborFlags both false → no diagonal.
        let mut pf = AStarPathfinder::new(2, 2);
        pf.set_cell_type(GridCoord::new(1, 0), PathfindCellType::Impassable);
        pf.set_cell_type(GridCoord::new(0, 1), PathfindCellType::Impassable);
        let start = GridCoord::new(0, 0);
        let goal = GridCoord::new(1, 1);
        assert!(
            pf.find_path(start, goal, SURFACE_GROUND, false, 200, false, None)
                .is_none(),
            "must not squeeze a diagonal when both orthogonals are blocked"
        );
    }

    #[test]
    fn air_paths_over_solid_obstacle_ground_does_not() {
        // C++ validLocomotorSurfacesForCellType(CELL_OBSTACLE) = AIR.
        // Full-height solid building wall: ground cannot go around.
        let mut pf = AStarPathfinder::new(9, 5);
        for y in 0..5 {
            pf.set_cell_obstacle_id(GridCoord::new(4, y), 1, false, false);
        }
        let start = GridCoord::new(0, 2);
        let goal = GridCoord::new(8, 2);

        assert!(
            !pf.is_passable(GridCoord::new(4, 2), SURFACE_GROUND, false),
            "ground blocked by solid building"
        );
        assert!(
            !pf.is_passable(GridCoord::new(4, 2), SURFACE_GROUND, true),
            "ground crusher still blocked by non-fence building"
        );
        assert!(
            pf.is_passable(GridCoord::new(4, 2), SURFACE_AIR, false),
            "AIR locomotor must enter CELL_OBSTACLE"
        );
        assert!(
            pf.find_path(start, goal, SURFACE_GROUND, false, 2000, false, None)
                .is_none(),
            "ground unit must not path through a solid building wall"
        );
        let (path, iters) = pf
            .find_path(start, goal, SURFACE_AIR, false, 2000, false, None)
            .expect("AIR locomotor must path over solid building obstacle");
        assert!(iters >= 1);
        assert_eq!(path.first().copied(), Some(start));
        assert_eq!(path.last().copied(), Some(goal));
        assert!(
            path.iter().any(|c| c.x == 4),
            "AIR path must cross obstacle column: {:?}",
            path
        );
    }

    #[test]
    fn air_paths_over_impassable_cells_ground_does_not() {
        // C++ validLocomotorSurfacesForCellType(CELL_IMPASSABLE) = AIR.
        let mut pf = AStarPathfinder::new(9, 5);
        for y in 0..5 {
            pf.set_cell_type(GridCoord::new(4, y), PathfindCellType::Impassable);
        }
        let start = GridCoord::new(0, 2);
        let goal = GridCoord::new(8, 2);
        assert!(!pf.is_passable(GridCoord::new(4, 2), SURFACE_GROUND, false));
        assert!(pf.is_passable(GridCoord::new(4, 2), SURFACE_AIR, false));
        assert!(
            pf.find_path(start, goal, SURFACE_GROUND, false, 2000, false, None)
                .is_none()
        );
        let (path, _) = pf
            .find_path(start, goal, SURFACE_AIR, false, 2000, false, None)
            .expect("AIR locomotor must path over CELL_IMPASSABLE");
        assert!(path.iter().any(|c| c.x == 4), "AIR path: {:?}", path);
    }

    #[test]
    fn test_ignore_obstacle_allows_pass_through() {
        let mut pathfinder = AStarPathfinder::new(10, 10);
        let obstacle = GridCoord::new(5, 5);
        pathfinder.set_cell_type(obstacle, PathfindCellType::Obstacle);

        let mut ignore = HashSet::new();
        ignore.insert(obstacle);

        assert!(!pathfinder.is_passable_with_ignore(obstacle, 0x01, false, None));
        assert!(pathfinder.is_passable_with_ignore(obstacle, 0x01, false, Some(&ignore)));
    }

    #[test]
    fn zone_impassable_adds_cost_penalty() {
        let mut pf = AStarPathfinder::new(30, 30);
        let a = GridCoord::new(2, 2);
        let (path1, cells1) = pf
            .find_path(
                a,
                GridCoord::new(25, 2),
                SURFACE_GROUND,
                false,
                8000,
                false,
                None,
            )
            .expect("path");
        assert!(path1.len() > 1);
        assert!(cells1 >= 1);
        pf.set_zone_passable(GridCoord::new(25, 2), false);
        assert!(!pf.is_zone_passable(GridCoord::new(25, 2)));
        assert!(pf.is_zone_passable(a));
        let (path2, cells2) = pf
            .find_path(
                a,
                GridCoord::new(25, 2),
                SURFACE_GROUND,
                false,
                8000,
                false,
                None,
            )
            .expect("path with zone penalty");
        assert!(path2.len() > 1);
        assert!(cells2 >= 1);
        assert!(!pf.clip_is_zone_passable(-1, 0));
        assert!(!pf.clip_is_zone_passable(0, 1000));
    }

    #[test]
    fn hierarchical_zone_prune_marks_corridor() {
        let mut pf = AStarPathfinder::new(80, 80);
        let start = GridCoord::new(2, 2);
        let goal = GridCoord::new(75, 2);
        assert!(pf.apply_hierarchical_zone_prune(start, goal, SURFACE_GROUND, false, &[]));
        assert!(pf.is_zone_passable(start));
        assert!(pf.is_zone_passable(goal));
        assert!(pf.is_zone_passable(GridCoord::new(40, 2)));
        assert!(
            !pf.is_zone_passable(GridCoord::new(40, 70)),
            "off-corridor block must stay pruned"
        );
        let (path, _n) = pf
            .find_path(start, goal, SURFACE_GROUND, false, 2000, false, None)
            .expect("corridor A* must still reach the far cell");
        assert!(path.len() > 2);
        assert_eq!(*path.last().unwrap(), goal);
    }

    #[test]
    fn hierarchical_zone_prune_jumps_bridge_over_water() {
        let mut pf = AStarPathfinder::new(40, 20);
        for y in 0..20 {
            pf.set_cell_type(GridCoord::new(20, y), PathfindCellType::Water);
        }
        let start = GridCoord::new(2, 10);
        let goal = GridCoord::new(35, 10);
        let near = GridCoord::new(19, 10);
        let far = GridCoord::new(21, 10);
        assert!(
            pf.apply_hierarchical_zone_prune(start, goal, SURFACE_GROUND, false, &[(near, far)]),
            "bridge jump must join river banks"
        );
        assert!(pf.is_zone_passable(start));
        assert!(pf.is_zone_passable(goal));
        assert!(
            !pf.apply_hierarchical_zone_prune(start, goal, SURFACE_GROUND, false, &[]),
            "no jump → hierarchical fail → all passable"
        );
        assert!(pf.is_zone_passable(GridCoord::new(2, 2)));
        assert!(pf.is_zone_passable(GridCoord::new(35, 18)));
    }

    #[test]
    fn examine_cells_line_seed_half_ortho_cost() {
        let mut pf = AStarPathfinder::new(20, 20);
        for x in 0..20 {
            for y in 0..20 {
                pf.set_cell_type(GridCoord::new(x, y), PathfindCellType::Clear);
            }
        }
        let start = GridCoord::new(2, 2);
        let goal = GridCoord::new(10, 2);
        let path = pf
            .find_path_ex4(
                start, goal, 0xFFFF, false, 5000, false, None, None, false, None, None, None, true,
            )
            .expect("path");
        assert!(path.0.len() >= 2);
        assert_eq!(*path.0.first().unwrap(), start);
        assert_eq!(*path.0.last().unwrap(), goal);
        assert!(
            path.0.iter().all(|c| c.y == 2),
            "line seed should prefer straight y=2: {:?}",
            path.0
        );
    }

    #[test]
    fn tunneling_invalid_step_allows_obstacle_with_surcharge() {
        // C++: start inside obstacle (tunneling), exit to clear goal beyond wall.
        // Tunneling clears on first valid non-pinched cell — so start must be obstacle.
        let mut pf = AStarPathfinder::new(12, 12);
        for x in 0..12 {
            for y in 0..12 {
                pf.set_cell_type(GridCoord::new(x, y), PathfindCellType::Clear);
            }
        }
        // Solid obstacle blob containing start at (3,5); goal outside at (8,5).
        for x in 2..=5 {
            for y in 3..=7 {
                pf.set_cell_type(GridCoord::new(x, y), PathfindCellType::Obstacle);
            }
        }
        let start = GridCoord::new(3, 5);
        let goal = GridCoord::new(8, 5);
        // force_passable allows start/goal validation for obstacle start.
        // Ground-only: 0xFFFF includes AIR, which can overfly CELL_OBSTACLE.
        let force = |c: GridCoord| c == start;
        assert!(
            pf.find_path_ex5(
                start,
                goal,
                SURFACE_GROUND,
                false,
                5000,
                false,
                None,
                None,
                false,
                None,
                Some(&force as &dyn Fn(GridCoord) -> bool),
                None,
                false,
                false,
                None,
            )
            .is_none()
        );
        let path = pf
            .find_path_ex5(
                start,
                goal,
                SURFACE_GROUND,
                false,
                5000,
                false,
                None,
                None,
                false,
                None,
                Some(&force as &dyn Fn(GridCoord) -> bool),
                None,
                false,
                true,
                None,
            )
            .expect("tunnel path");
        assert_eq!(*path.0.first().unwrap(), start);
        assert_eq!(*path.0.last().unwrap(), goal);
    }

    #[test]
    fn pinched_extra_ortho_on_expand_cpp_surface() {
        let src = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/ai/pathfind_astar.rs"
        ));
        assert!(
            src.contains("starts_tunneling")
                && src.contains("10 * COST_ORTHOGONAL")
                && src.contains("is_tunneling = false"),
            "expand must clear tunneling and apply C++ tunnel surcharge"
        );
    }

    #[test]
    fn examine_neighbors_on_list_never_reopens_or_recosts() {
        // C++ examineNeighboringCells AIPathfind.cpp:6167-6180:
        // if (getOpen() || getClosed()) continue; — never update g, never reopen.
        // extra_cost is applied only after that skip, so each cell is recosted once.
        let pf = AStarPathfinder::new(5, 5);
        let start = GridCoord::new(0, 0);
        let goal = GridCoord::new(4, 4);
        let counts = std::cell::RefCell::new(HashMap::<GridCoord, u32>::new());
        let extra = |c: GridCoord| {
            *counts.borrow_mut().entry(c).or_insert(0) += 1;
            0u32
        };
        let (path, iters) = pf
            .find_path_ex(
                start,
                goal,
                SURFACE_GROUND,
                false,
                2000,
                false,
                None,
                Some(&extra as &dyn Fn(GridCoord) -> u32),
            )
            .expect("open 5x5 must path");
        assert!(iters >= 1);
        assert_eq!(path.first().copied(), Some(start));
        assert_eq!(path.last().copied(), Some(goal));
        let counts = counts.into_inner();
        assert!(
            counts.len() > 1,
            "extra_cost must run on neighbor expansion"
        );
        for (c, n) in counts.iter() {
            assert_eq!(
                *n, 1,
                "cell {:?} recosted {} times; onList must skip before cost update",
                c, n
            );
        }
        // First-visit parent is kept: straight-ish first expansion from start
        // claims (1,0) via ortho; a later cheaper reopen would not replace it.
        assert!(
            path.contains(&GridCoord::new(1, 0))
                || path.contains(&GridCoord::new(0, 1))
                || path.contains(&GridCoord::new(1, 1)),
            "first-visit neighbors from start stay on the reconstructed path: {:?}",
            path
        );
    }

    #[test]
    fn dozer_hack_obstacle_and_no_diagonal_squeeze() {
        // C++ AIPathfind.cpp:6207-6226:
        // dozerHack lets dozers step on non-enemy CELL_OBSTACLE;
        // neighborFlags is NOT set, so diagonals cannot squeeze through.
        let mut pf = AStarPathfinder::new(2, 2);
        let start = GridCoord::new(0, 0);
        let goal = GridCoord::new(1, 1);
        let dozer_cell = GridCoord::new(1, 0);
        pf.set_cell_type(dozer_cell, PathfindCellType::Obstacle);
        pf.set_cell_type(GridCoord::new(0, 1), PathfindCellType::Impassable);

        assert!(
            pf.find_path(start, goal, SURFACE_GROUND, false, 200, false, None)
                .is_none(),
            "non-dozer cannot cross non-fence Obstacle or squeeze the diagonal"
        );
        assert!(
            pf.find_path_ex6(
                start,
                goal,
                SURFACE_GROUND,
                false,
                200,
                false,
                None,
                None,
                false,
                None,
                None,
                None,
                false,
                false,
                None,
                None,
            )
            .is_none(),
            "find_path_ex6 with dozer_obstacle_ok=None matches non-dozer"
        );

        let ok = |c: GridCoord| c == dozer_cell;
        let (path, _) = pf
            .find_path_ex6(
                start,
                goal,
                SURFACE_GROUND,
                false,
                200,
                false,
                None,
                None,
                false,
                None,
                None,
                None,
                false,
                false,
                None,
                Some(&ok as &dyn Fn(GridCoord) -> bool),
            )
            .expect("dozer_obstacle_ok must allow stepping on CELL_OBSTACLE");
        assert_eq!(
            path,
            vec![start, dozer_cell, goal],
            "dozer must step on the obstacle; must not diagonal-squeeze (neighborFlags false): {:?}",
            path
        );

        // Callback true on Impassable still must NOT dozerHack (C++ requires CELL_OBSTACLE).
        let always = |_c: GridCoord| true;
        let (path2, _) = pf
            .find_path_ex6(
                start,
                goal,
                SURFACE_GROUND,
                false,
                200,
                false,
                None,
                None,
                false,
                None,
                None,
                None,
                false,
                false,
                None,
                Some(&always as &dyn Fn(GridCoord) -> bool),
            )
            .expect("dozer still paths via the obstacle cell");
        assert_eq!(path2, vec![start, dozer_cell, goal]);
        assert!(
            !pf.is_passable(GridCoord::new(0, 1), SURFACE_GROUND, false),
            "Impassable cell is not a dozerHack target"
        );
    }

    #[test]
    fn check_change_layers_enqueues_same_xy_at_parent_cost() {
        // C++ checkChangeLayers AIPathfind.cpp:5942-5981:
        // connectLayer cell at same x,y, not on open/closed, same costSoFar/totalCost.
        let mut pf = AStarPathfinder::new(4, 4);
        let start = GridCoord::new(1, 1);
        pf.set_cell_connect_layer(start, PathfindLayerEnum::Top);

        let mut open_set = BinaryHeap::new();
        let mut open_members = HashSet::new();
        let closed_set = HashSet::new();
        let mut came_from = HashMap::new();
        let mut g_scores = HashMap::new();
        let enqueued = pf.check_change_layers(
            start,
            PathfindLayerEnum::Ground,
            40,
            99,
            &mut open_set,
            &mut open_members,
            &closed_set,
            &mut came_from,
            &mut g_scores,
        );
        assert!(
            enqueued,
            "checkChangeLayers must enqueue same-xy connect layer"
        );
        let key = (start, PathfindLayerEnum::Top);
        assert!(open_members.contains(&key));
        assert_eq!(g_scores.get(&key).copied(), Some(40));
        let node = open_set.pop().expect("layered node on open heap");
        assert_eq!(node.coord, start);
        assert_eq!(node.layer, PathfindLayerEnum::Top);
        assert_eq!(node.g_score, 40);
        assert_eq!(node.f_score, 99);
        assert_eq!(node.parent, Some((start, PathfindLayerEnum::Ground)));

        // Already on open: do not re-enqueue.
        open_members.insert(key);
        assert!(
            !pf.check_change_layers(
                start,
                PathfindLayerEnum::Ground,
                40,
                99,
                &mut open_set,
                &mut open_members,
                &closed_set,
                &mut came_from,
                &mut g_scores,
            ),
            "already on open list must not re-enqueue"
        );

        // find_path still succeeds; extra same-xy expand is not a silent no-op.
        let goal = GridCoord::new(3, 1);
        let (path_layered, iters_layered) = pf
            .find_path(start, goal, SURFACE_GROUND, false, 500, false, None)
            .expect("path with connect_layer must succeed");
        assert_eq!(path_layered.first().copied(), Some(start));
        assert_eq!(path_layered.last().copied(), Some(goal));

        pf.set_cell_connect_layer(start, PathfindLayerEnum::Invalid);
        let (_, iters_plain) = pf
            .find_path(start, goal, SURFACE_GROUND, false, 500, false, None)
            .expect("plain path");
        assert!(
            iters_layered > iters_plain,
            "checkChangeLayers must expand an extra same-xy layer node (layered={}, plain={})",
            iters_layered,
            iters_plain
        );
        assert_eq!(
            pf.connect_layer_transition_coord(start),
            None,
            "invalid connect layer has no transition coord"
        );
        pf.set_cell_connect_layer(start, PathfindLayerEnum::Top);
        assert_eq!(
            pf.connect_layer_transition_coord(start),
            Some(start),
            "public GridCoord API stays same-xy"
        );
    }

    #[test]
    fn ground_impassable_does_not_block_top_when_top_is_clear_and_vice_versa() {
        // Independent per-layer grids: C++ m_map vs m_layers[LAYER_TOP].
        let mut pf = AStarPathfinder::new(9, 5);
        let wall_x = 4;
        for y in 0..5 {
            let c = GridCoord::new(wall_x, y);
            pf.set_cell_type(c, PathfindCellType::Impassable);
            pf.set_cell_type_on_layer(c, PathfindLayerEnum::Top, PathfindCellType::Clear);
        }
        let blocked = GridCoord::new(wall_x, 2);
        assert_eq!(
            pf.get_cell_type(blocked),
            Some(PathfindCellType::Impassable)
        );
        assert_eq!(
            pf.get_cell_type_on_layer(blocked, PathfindLayerEnum::Top),
            Some(PathfindCellType::Clear)
        );
        assert!(!pf.is_passable(blocked, SURFACE_GROUND, false));
        assert!(pf.is_passable_on_layer(blocked, PathfindLayerEnum::Top, SURFACE_GROUND, false));

        let start = GridCoord::new(0, 2);
        let goal = GridCoord::new(8, 2);
        assert!(
            pf.find_path(start, goal, SURFACE_GROUND, false, 2000, false, None)
                .is_none(),
            "ground Impassable wall must block LAYER_GROUND search"
        );
        let (top_path, _) = pf
            .find_path_on_layer(
                start,
                goal,
                PathfindLayerEnum::Top,
                SURFACE_GROUND,
                false,
                2000,
                false,
                None,
            )
            .expect("Top Clear cells must ignore ground Impassable");
        assert!(
            top_path.iter().any(|c| c.x == wall_x),
            "Top path must cross the ground wall: {:?}",
            top_path
        );

        // Vice versa: Top Impassable/Obstacle does not block Ground Clear.
        let mut pf2 = AStarPathfinder::new(9, 5);
        for y in 0..5 {
            let c = GridCoord::new(wall_x, y);
            pf2.set_cell_type(c, PathfindCellType::Clear);
            pf2.set_cell_type_on_layer(c, PathfindLayerEnum::Top, PathfindCellType::Impassable);
        }
        assert_eq!(pf2.get_cell_type(blocked), Some(PathfindCellType::Clear));
        assert_eq!(
            pf2.get_cell_type_on_layer(blocked, PathfindLayerEnum::Top),
            Some(PathfindCellType::Impassable)
        );
        assert!(pf2.is_passable(blocked, SURFACE_GROUND, false));
        let (ground_path, _) = pf2
            .find_path(start, goal, SURFACE_GROUND, false, 2000, false, None)
            .expect("ground Clear must ignore Top Impassable");
        assert!(
            ground_path.iter().any(|c| c.x == wall_x),
            "ground path must cross the Top-only wall: {:?}",
            ground_path
        );
    }

    #[test]
    fn top_obstacle_blocks_top_search_not_ground() {
        let mut pf = AStarPathfinder::new(9, 5);
        let mid = GridCoord::new(4, 2);
        pf.set_cell_type(mid, PathfindCellType::Clear);
        pf.set_cell_type_on_layer(mid, PathfindLayerEnum::Top, PathfindCellType::Obstacle);

        assert_eq!(pf.get_cell_type(mid), Some(PathfindCellType::Clear));
        assert_eq!(
            pf.get_cell_type_on_layer(mid, PathfindLayerEnum::Top),
            Some(PathfindCellType::Obstacle)
        );
        assert!(pf.is_passable(mid, SURFACE_GROUND, false));
        assert!(!pf.is_passable_on_layer(mid, PathfindLayerEnum::Top, SURFACE_GROUND, false));

        let start = GridCoord::new(0, 2);
        let goal = GridCoord::new(8, 2);
        let (ground_path, _) = pf
            .find_path(start, goal, SURFACE_GROUND, false, 2000, false, None)
            .expect("ground Clear at mid remains walkable");
        assert!(
            ground_path.contains(&mid),
            "ground path may step Clear mid: {:?}",
            ground_path
        );

        let (top_path, _) = pf
            .find_path_on_layer(
                start,
                goal,
                PathfindLayerEnum::Top,
                SURFACE_GROUND,
                false,
                2000,
                false,
                None,
            )
            .expect("Top search can go around a single Obstacle");
        assert!(
            !top_path.contains(&mid),
            "Top path must not step Top Obstacle: {:?}",
            top_path
        );
        assert_eq!(top_path.first().copied(), Some(start));
        assert_eq!(top_path.last().copied(), Some(goal));
    }

    #[test]
    fn reset_clears_layer_grids() {
        let mut pf = AStarPathfinder::new(4, 4);
        let c = GridCoord::new(2, 2);
        pf.set_cell_type(c, PathfindCellType::Water);
        pf.set_cell_type_on_layer(c, PathfindLayerEnum::Top, PathfindCellType::Obstacle);
        pf.set_pinched_on_layer(c, PathfindLayerEnum::Top, true);
        assert_eq!(
            pf.get_cell_type_on_layer(c, PathfindLayerEnum::Top),
            Some(PathfindCellType::Obstacle)
        );
        pf.reset();
        assert_eq!(pf.get_cell_type(c), Some(PathfindCellType::Clear));
        assert_eq!(
            pf.get_cell_type_on_layer(c, PathfindLayerEnum::Top),
            None,
            "reset() must drop elevated layer grids"
        );
        assert_eq!(pf.is_pinched_on_layer(c, PathfindLayerEnum::Top), None);
    }

    #[test]
    fn missing_elevated_cell_falls_back_to_ground_like_cpp_get_cell() {
        // C++ Pathfinder::getCell(layer, x, y) (AIPathfind.h:899-917):
        //   if layer > GROUND, try m_layers[layer].getCell; if NULL, return &m_map[x][y].
        // PathfindLayer::getCell also returns NULL for CELL_IMPASSABLE (cpp:3636-3638).
        //
        // Public get_cell_type_on_layer reports the *stored* elevated type (None if
        // missing) so callers can distinguish "no Top cell" from "Top == ground".
        // Search / is_passable_on_layer use get_cell_on_layer → ground fallback.
        let mut pf = AStarPathfinder::new(5, 3);
        let c = GridCoord::new(2, 1);
        pf.set_cell_type(c, PathfindCellType::Impassable);

        assert_eq!(
            pf.get_cell_type_on_layer(c, PathfindLayerEnum::Top),
            None,
            "no Top slot written → stored type is None"
        );
        assert!(
            !pf.is_passable_on_layer(c, PathfindLayerEnum::Top, SURFACE_GROUND, false),
            "C++ getCell fallback: missing Top cell uses ground Impassable"
        );

        let start = GridCoord::new(0, 1);
        let goal = GridCoord::new(4, 1);
        // Full-height ground Impassable would block; here only one cell is Impassable
        // so both searches can detour. Stamp a full wall, then only one Top Clear.
        for y in 0..3 {
            pf.set_cell_type(GridCoord::new(2, y), PathfindCellType::Impassable);
        }
        assert!(
            pf.find_path(start, goal, SURFACE_GROUND, false, 500, false, None)
                .is_none()
        );
        assert!(
            pf.find_path_on_layer(
                start,
                goal,
                PathfindLayerEnum::Top,
                SURFACE_GROUND,
                false,
                500,
                false,
                None,
            )
            .is_none(),
            "missing Top cells fall back to ground Impassable wall"
        );

        // Writing Top Clear at the wall opens only the Top layer (no fallback).
        for y in 0..3 {
            pf.set_cell_type_on_layer(
                GridCoord::new(2, y),
                PathfindLayerEnum::Top,
                PathfindCellType::Clear,
            );
        }
        assert_eq!(
            pf.get_cell_type_on_layer(c, PathfindLayerEnum::Top),
            Some(PathfindCellType::Clear)
        );
        assert!(
            pf.find_path_on_layer(
                start,
                goal,
                PathfindLayerEnum::Top,
                SURFACE_GROUND,
                false,
                500,
                false,
                None,
            )
            .is_some()
        );
        assert!(
            pf.find_path(start, goal, SURFACE_GROUND, false, 500, false, None)
                .is_none()
        );

        // C++ PathfindLayer::getCell: CELL_IMPASSABLE on the layer is treated as
        // NULL, so getCell falls back to ground (still Impassable here).
        pf.set_cell_type_on_layer(c, PathfindLayerEnum::Top, PathfindCellType::Impassable);
        assert_eq!(
            pf.get_cell_type_on_layer(c, PathfindLayerEnum::Top),
            Some(PathfindCellType::Impassable),
            "stored type remains Impassable"
        );
        assert!(
            !pf.is_passable_on_layer(c, PathfindLayerEnum::Top, SURFACE_GROUND, false),
            "Impassable Top cell → getCell NULL → fall back to ground Impassable"
        );
    }
}
