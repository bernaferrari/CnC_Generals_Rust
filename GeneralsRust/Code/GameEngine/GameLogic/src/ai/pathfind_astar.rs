// pathfind_astar.rs
// A* Pathfinding Algorithm - Faithful C++ Port
// Reference: /GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AIPathfind.cpp

use crate::common::{Coord2D, Coord3D};
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap, HashSet};

/// Movement cost constants matching C++ AIPathfind.cpp:1649-1650
pub const COST_ORTHOGONAL: u32 = 10;
pub const COST_DIAGONAL: u32 = 14;
/// C++ notZonePassable penalty: `100 * COST_ORTHOGONAL`.
pub const ZONE_IMPASSABLE_COST: u32 = 100 * COST_ORTHOGONAL;

/// Pathfinding cell size matching C++ AIPathfind.h:415-416
pub const PATHFIND_CELL_SIZE: i32 = 10;
pub const PATHFIND_CELL_SIZE_F: f32 = 10.0;

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
    Top = 2,
    // Additional layers can be added as needed
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
    pub fn to_world(&self, _layer: PathfindLayerEnum) -> Coord3D {
        Coord3D::new(
            (self.x as f32 + 0.5) * PATHFIND_CELL_SIZE_F,
            (self.y as f32 + 0.5) * PATHFIND_CELL_SIZE_F,
            0.0, // Z will be set by terrain logic
        )
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

/// A* node for priority queue
/// Matches C++ PathfindCell structure at AIPathfind.cpp:6137-6357
#[derive(Debug, Clone)]
struct AStarNode {
    coord: GridCoord,
    g_score: u32, // Cost from start
    f_score: u32, // g_score + h_score
    parent: Option<GridCoord>,
}

impl PartialEq for AStarNode {
    fn eq(&self, other: &Self) -> bool {
        self.coord == other.coord
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
    grid: Vec<Vec<PathfindCell>>,
    width: usize,
    height: usize,
    /// Cell -> owning obstacle object id (C++ PathfindCellInfo::obstacleID).
    obstacle_owners: HashMap<(i32, i32), u32>,
    /// C++ PathfindCellInfo::m_obstacleIsFence.
    obstacle_fence: HashSet<(i32, i32)>,
    /// C++ PathfindCellInfo::m_obstacleIsTransparent (KINDOF_CAN_SEE_THROUGH).
    obstacle_transparent: HashSet<(i32, i32)>,
    /// C++ PathfindZoneManager block passable (blockX, blockY).
    /// Only false entries stored; missing = true (default passable).
    zone_impassable_blocks: HashSet<(i32, i32)>,
}

impl AStarPathfinder {
    pub fn new(width: usize, height: usize) -> Self {
        let grid = vec![vec![PathfindCell::new(); height]; width];
        Self {
            grid,
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
        self.obstacle_owners.clear();
        self.obstacle_fence.clear();
        self.obstacle_transparent.clear();
        self.zone_impassable_blocks.clear();
    }

    /// Get cell at grid coordinates
    fn get_cell(&self, coord: GridCoord) -> Option<&PathfindCell> {
        if coord.x >= 0
            && coord.x < self.width as i32
            && coord.y >= 0
            && coord.y < self.height as i32
        {
            Some(&self.grid[coord.x as usize][coord.y as usize])
        } else {
            None
        }
    }

    /// Get mutable cell at grid coordinates
    fn get_cell_mut(&mut self, coord: GridCoord) -> Option<&mut PathfindCell> {
        if coord.x >= 0
            && coord.x < self.width as i32
            && coord.y >= 0
            && coord.y < self.height as i32
        {
            Some(&mut self.grid[coord.x as usize][coord.y as usize])
        } else {
            None
        }
    }

    fn is_ignored_obstacle(
        &self,
        coord: GridCoord,
        ignore_cells: Option<&HashSet<GridCoord>>,
    ) -> bool {
        let Some(ignore_cells) = ignore_cells else {
            return false;
        };
        if !ignore_cells.contains(&coord) {
            return false;
        }
        matches!(
            self.get_cell(coord).map(|cell| cell.get_type()),
            Some(PathfindCellType::Obstacle)
        )
    }

    /// Check if a cell is passable for the given movement type
    /// Matches C++ validMovementPosition() logic
    pub fn is_passable(&self, coord: GridCoord, surfaces: u32, is_crusher: bool) -> bool {
        self.is_passable_with_ignore(coord, surfaces, is_crusher, None)
    }

    pub fn is_passable_with_ignore(
        &self,
        coord: GridCoord,
        surfaces: u32,
        is_crusher: bool,
        ignore_cells: Option<&HashSet<GridCoord>>,
    ) -> bool {
        let Some(cell) = self.get_cell(coord) else {
            return false;
        };

        if self.is_ignored_obstacle(coord, ignore_cells) {
            return true;
        }

        // Impassable cells
        if cell.is_impassable() {
            if cell.get_type() == PathfindCellType::Obstacle && is_crusher {
                // Crushers can go through obstacles
                return true;
            }
            return false;
        }

        // Note: Pinched cells are passable but have higher cost in movement_cost_with_ignore
        // This matches C++ behavior where pinched cells add COST_DIAGONAL but are not blocked

        // Check surface compatibility
        match cell.get_type() {
            // C++ validLocomotorSurfacesForCellType: clear -> ground|air.
            PathfindCellType::Clear => (surfaces & (SURFACE_GROUND | SURFACE_AIR)) != 0,
            PathfindCellType::Water => (surfaces & SURFACE_WATER) != 0,
            PathfindCellType::Cliff => (surfaces & SURFACE_CLIFF) != 0,
            PathfindCellType::Rubble => (surfaces & SURFACE_RUBBLE) != 0 || is_crusher,
            _ => false,
        }
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
        is_crusher: bool,
        ignore_cells: Option<&HashSet<GridCoord>>,
        came_from: &HashMap<GridCoord, GridCoord>,
    ) -> u32 {
        let Some(to_cell) = self.get_cell(to) else {
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
                if self.is_ignored_obstacle(to, ignore_cells) {
                    // Treat ignored obstacles as clear.
                } else if is_crusher {
                    // Crushers can go through but it's expensive
                    cost += 100 * COST_ORTHOGONAL;
                } else {
                    return u32::MAX; // Impassable
                }
            }
            PathfindCellType::BridgeImpassable | PathfindCellType::Impassable => {
                return u32::MAX; // Impassable
            }
        }

        // Apply pinched cell penalty (AIPathfind.cpp:1701-1703)
        // C++ adds COST_DIAGONAL (14) for pinched cells
        if to_cell.is_pinched() {
            cost += COST_DIAGONAL;
        }

        // Apply turn cost penalty (AIPathfind.cpp:1705-1720)
        // This adds extra cost for turns in the path
        if let Some(&parent_coord) = came_from.get(&from) {
            // Calculate direction vectors
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
        self.find_path_ex2(
            start,
            goal,
            surfaces,
            is_crusher,
            max_iterations,
            allow_partial,
            ignore_cells,
            extra_cost,
            false,
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
        // Initialize open and closed sets
        // Matches C++ at AIPathfind.cpp:6575-6581
        let mut open_set = BinaryHeap::new();
        let mut closed_set = HashSet::new();
        let mut came_from: HashMap<GridCoord, GridCoord> = HashMap::new();
        let mut g_scores: HashMap<GridCoord, u32> = HashMap::new();

        let mut best_coord = start;
        let mut best_dist = start.diagonal_distance(&goal);

        // Validate start and goal (tunneling may force start passable).
        let pass = |c: GridCoord| -> bool {
            if self.is_passable_with_ignore(c, surfaces, is_crusher, ignore_cells) {
                return true;
            }
            force_passable.map(|f| f(c)).unwrap_or(false)
        };
        if !pass(start) || !pass(goal) {
            // Goal may still be obstacle when ignore obstacle / tunneling into building.
            if !pass(start) {
                return None;
            }
            if !pass(goal) && force_passable.is_none() {
                return None;
            }
            if !pass(goal) {
                // Allow goal when force_passable says so, else fail.
                if !force_passable.map(|f| f(goal)).unwrap_or(false) {
                    return None;
                }
            }
        }

        // Initialize start node
        // Matches C++ PathfindCell::startPathfind() at AIPathfind.cpp:1216-1219
        let h_score = start.diagonal_distance(&goal);
        let start_node = AStarNode {
            coord: start,
            g_score: 0,
            f_score: h_score,
            parent: None,
        };

        open_set.push(start_node);
        g_scores.insert(start, 0);

        // C++ m_isTunneling — clear once we expand into valid non-pinched cell.
        let mut is_tunneling = starts_tunneling;

        let mut iterations = 0;

        // Main A* loop
        // Matches C++ while loop at AIPathfind.cpp:6589-6633
        while let Some(current) = open_set.pop() {
            iterations += 1;
            if iterations > max_iterations {
                // Prevent infinite loops
                if allow_partial {
                    return Some((self.reconstruct_path(&came_from, best_coord), iterations));
                }
                return None;
            }

            // Goal reached!
            // Matches C++ at AIPathfind.cpp:6595-6622
            if current.coord == goal {
                return Some((self.reconstruct_path(&came_from, current.coord), iterations));
            }

            let current_dist = current.coord.diagonal_distance(&goal);
            if current_dist < best_dist {
                best_dist = current_dist;
                best_coord = current.coord;
            }

            // Move current to closed set
            // Matches C++ at AIPathfind.cpp:6626
            closed_set.insert(current.coord);

            // C++ examineNeighboringCells: examineCellsCallback along parent→goal
            // when NO_ATTACK && !tunneling && !downhillOnly && goalCell.
            // Prefer explicit seed flag; skip when downhill-only / tunneling (C++ guard).
            if seed_line_to_goal && !downhill_only && !is_tunneling {
                self.examine_cells_toward_goal(
                    current.coord,
                    current.g_score,
                    goal,
                    surfaces,
                    is_crusher,
                    ignore_cells,
                    force_passable,
                    line_cell_ok,
                    cell_allowed,
                    &mut open_set,
                    &mut closed_set,
                    &mut came_from,
                    &mut g_scores,
                );
            }

            // Examine all neighbors
            // Matches C++ examineNeighboringCells() at AIPathfind.cpp:6631
            // C++ checkChangeLayers(parent): also examine connect-layer link at same x,y.
            let mut neighbor_iter: Vec<GridCoord> = current.coord.neighbors().to_vec();
            if let Some(link) = self.connect_layer_transition_coord(current.coord) {
                if !neighbor_iter.contains(&link) {
                    neighbor_iter.push(link);
                }
            }
            for neighbor_coord in neighbor_iter {
                // Skip if already evaluated
                if closed_set.contains(&neighbor_coord) {
                    continue;
                }

                // C++ isHuman logical extent clamp (examineNeighboringCells).
                if let Some(ok) = cell_allowed {
                    if !ok(neighbor_coord) {
                        continue;
                    }
                }

                // Prevent diagonal corner-cutting through blocked orthogonal neighbors.
                if current.coord.is_diagonal(&neighbor_coord) {
                    let step_x = neighbor_coord.x - current.coord.x;
                    let step_y = neighbor_coord.y - current.coord.y;
                    let ortho_a = GridCoord::new(current.coord.x + step_x, current.coord.y);
                    let ortho_b = GridCoord::new(current.coord.x, current.coord.y + step_y);
                    let ortho_ok = |c: GridCoord| {
                        self.is_passable_with_ignore(c, surfaces, is_crusher, ignore_cells)
                            || force_passable.map(|f| f(c)).unwrap_or(false)
                    };
                    if !ortho_ok(ortho_a) || !ortho_ok(ortho_b) {
                        continue;
                    }
                }

                let naturally_passable = self.is_passable_with_ignore(
                    neighbor_coord,
                    surfaces,
                    is_crusher,
                    ignore_cells,
                );
                let force_ok = force_passable.map(|f| f(neighbor_coord)).unwrap_or(false);
                // C++: invalid movement only expands while m_isTunneling (or dozerHack).
                if !naturally_passable && !force_ok && !is_tunneling {
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

                // Calculate tentative g_score
                // Matches C++ at AIPathfind.cpp:6259 + 6277-6333
                let mut movement_cost = self.movement_cost_with_ignore(
                    current.coord,
                    neighbor_coord,
                    is_crusher,
                    ignore_cells,
                    &came_from,
                );
                if movement_cost == u32::MAX {
                    // Tunneling / force: still expand with base ortho/diag step.
                    if is_tunneling || force_ok {
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
                if self.is_pinched(neighbor_coord).unwrap_or(false) {
                    movement_cost = movement_cost.saturating_add(COST_ORTHOGONAL);
                }
                // C++ CELL_OBSTACLE: +100*COST_ORTHOGONAL when expanding through obstacle.
                if let Some(cell) = self.get_cell(neighbor_coord) {
                    if cell.get_type() == PathfindCellType::Obstacle
                        && !self.is_ignored_obstacle(neighbor_coord, ignore_cells)
                    {
                        // Crusher already paid 100*ORTHO in movement_cost_with_ignore.
                        if !(naturally_passable && is_crusher) {
                            if !naturally_passable || is_tunneling || force_ok {
                                movement_cost = movement_cost.saturating_add(100 * COST_ORTHOGONAL);
                            }
                        }
                    }
                }
                // C++ notZonePassable: ground hierarchical block not yet expanded →
                // heavy cost (100 * COST_ORTHOGONAL), not hard reject in this path.
                if !self.is_zone_passable(neighbor_coord) {
                    movement_cost = movement_cost.saturating_add(ZONE_IMPASSABLE_COST);
                }
                // C++ allyFixedCount > 0 → +3*COST_DIAGONAL (and setBlockedByAlly).
                if let Some(extra) = extra_cost {
                    movement_cost = movement_cost.saturating_add(extra(neighbor_coord));
                }
                // C++ cliff: if !pinched && |dz| < PATHFIND_CELL_SIZE_F → already has
                // base cliff cost in movement_cost; when |dz| >= cell size, remove the
                // flat-cliff surcharge (movement_cost always adds 7*DIAG for cliffs).
                if let Some(h) = ground_height {
                    if let Some(cell) = self.get_cell(neighbor_coord) {
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
                let neighbor_pinched = self.is_pinched(neighbor_coord).unwrap_or(false);
                if naturally_passable && !neighbor_pinched {
                    is_tunneling = false;
                }

                let tentative_g = current.g_score.saturating_add(movement_cost);

                // Check if this path is better
                // Matches C++ at AIPathfind.cpp:6321-6327
                if let Some(&existing_g) = g_scores.get(&neighbor_coord) {
                    if tentative_g >= existing_g {
                        continue; // Not a better path
                    }
                }

                // This is the best path so far
                came_from.insert(neighbor_coord, current.coord);
                g_scores.insert(neighbor_coord, tentative_g);

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
                    g_score: tentative_g,
                    f_score,
                    parent: Some(current.coord),
                };

                open_set.push(neighbor_node);
            }
        }

        // No path found
        // Matches C++ at AIPathfind.cpp:6635-6693
        if allow_partial {
            Some((self.reconstruct_path(&came_from, best_coord), iterations))
        } else {
            None
        }
    }

    /// C++ Pathfinder::examineCellsCallback line seed (AIPathfind.cpp:5996-6093).
    /// Walks Bresenham from parent toward goal; inserts clear cells at half ortho cost.
    fn examine_cells_toward_goal(
        &self,
        parent: GridCoord,
        parent_g: u32,
        goal: GridCoord,
        surfaces: u32,
        is_crusher: bool,
        ignore_cells: Option<&HashSet<GridCoord>>,
        force_passable: Option<&dyn Fn(GridCoord) -> bool>,
        line_cell_ok: Option<&dyn Fn(GridCoord) -> bool>,
        cell_allowed: Option<&dyn Fn(GridCoord) -> bool>,
        open_set: &mut BinaryHeap<AStarNode>,
        closed_set: &mut HashSet<GridCoord>,
        came_from: &mut HashMap<GridCoord, GridCoord>,
        g_scores: &mut HashMap<GridCoord, u32>,
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
            if self.get_cell(to).is_none() {
                break;
            }
            if let Some(ok) = cell_allowed {
                if !ok(to) {
                    break;
                }
            }

            // Abort line (return 1) conditions from examineCellsCallback.
            if !self.is_passable_with_ignore(to, surfaces, is_crusher, ignore_cells)
                && !force_passable.map(|f| f(to)).unwrap_or(false)
            {
                break;
            }
            if !self.is_zone_passable(to) {
                break;
            }
            if self.is_pinched(to).unwrap_or(false) {
                break;
            }
            if let Some(cell) = self.get_cell(to) {
                if cell.get_type() == PathfindCellType::Cliff {
                    break;
                }
            }
            if let Some(ok) = line_cell_ok {
                if !ok(to) {
                    break;
                }
            }

            // newCostSoFar = from->getCostSoFar() + 0.5f*COST_ORTHOGONAL
            let new_g = from_g.saturating_add(COST_ORTHOGONAL / 2);
            if let Some(&existing_g) = g_scores.get(&to) {
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

            // Better path — reopen if closed.
            closed_set.remove(&to);
            came_from.insert(to, from);
            g_scores.insert(to, new_g);
            let h_score = to.diagonal_distance(&goal);
            open_set.push(AStarNode {
                coord: to,
                g_score: new_g,
                f_score: new_g.saturating_add(h_score),
                parent: Some(from),
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
    fn reconstruct_path(
        &self,
        came_from: &HashMap<GridCoord, GridCoord>,
        mut current: GridCoord,
    ) -> Vec<GridCoord> {
        let mut path = vec![current];

        while let Some(&parent) = came_from.get(&current) {
            path.push(parent);
            current = parent;
        }

        path.reverse();
        path
    }

    /// Set cell type at coordinates
    pub fn set_cell_type(&mut self, coord: GridCoord, cell_type: PathfindCellType) {
        if let Some(cell) = self.get_cell_mut(coord) {
            cell.set_type(cell_type);
        }
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

    pub fn set_cell_connect_layer(&mut self, coord: GridCoord, layer: PathfindLayerEnum) {
        if let Some(cell) = self.get_cell_mut(coord) {
            cell.set_connect_layer(layer);
        }
    }

    pub fn get_cell_connect_layer(&self, coord: GridCoord) -> Option<PathfindLayerEnum> {
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
        self.obstacle_owners.get(&(coord.x, coord.y)).copied()
    }

    pub fn set_cell_obstacle_id(
        &mut self,
        coord: GridCoord,
        obj_id: u32,
        is_fence: bool,
        is_transparent: bool,
    ) {
        if let Some(cell) = self.get_cell_mut(coord) {
            cell.set_type(PathfindCellType::Obstacle);
        }
        self.obstacle_owners.insert((coord.x, coord.y), obj_id);
        if is_fence {
            self.obstacle_fence.insert((coord.x, coord.y));
        } else {
            self.obstacle_fence.remove(&(coord.x, coord.y));
        }
        if is_transparent {
            self.obstacle_transparent.insert((coord.x, coord.y));
        } else {
            self.obstacle_transparent.remove(&(coord.x, coord.y));
        }
    }

    /// C++ PathfindCell::isObstacleTransparent.
    pub fn is_obstacle_transparent(&self, coord: GridCoord) -> bool {
        self.obstacle_transparent.contains(&(coord.x, coord.y))
    }

    pub fn is_obstacle_fence(&self, coord: GridCoord) -> bool {
        self.obstacle_fence.contains(&(coord.x, coord.y))
    }

    /// Clear obstacle if it matches obj_id (C++ removeObstacle).
    pub fn clear_cell_obstacle_id(&mut self, coord: GridCoord, obj_id: u32) -> bool {
        let key = (coord.x, coord.y);
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
        if let Some(cell) = self.get_cell_mut(coord) {
            cell.set_pinched(pinched);
        }
    }

    /// Get whether a cell is pinched.
    pub fn is_pinched(&self, coord: GridCoord) -> Option<bool> {
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
        // Path should go around the wall
        assert!(path.len() > 10); // More than straight line
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
        let mut pathfinder = AStarPathfinder::new(10, 10);

        // Create obstacles
        pathfinder.set_cell_type(GridCoord::new(5, 5), PathfindCellType::Obstacle);

        let start = GridCoord::new(0, 5);
        let goal = GridCoord::new(9, 5);

        // Non-crusher should path around
        let path_normal = pathfinder
            .find_path(start, goal, 0x01, false, 1000, false, None)
            .map(|(p, _)| p);
        assert!(path_normal.is_some());

        // Crusher should be able to go through
        let path_crusher = pathfinder
            .find_path(start, goal, 0x01, true, 1000, false, None)
            .map(|(p, _)| p);
        assert!(path_crusher.is_some());

        // Crusher path might be shorter (going through obstacles)
        assert!(path_crusher.unwrap().len() <= path_normal.unwrap().len());
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
        let force = |c: GridCoord| c == start;
        assert!(pf
            .find_path_ex5(
                start,
                goal,
                0xFFFF,
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
            .is_none());
        let path = pf
            .find_path_ex5(
                start,
                goal,
                0xFFFF,
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
}
