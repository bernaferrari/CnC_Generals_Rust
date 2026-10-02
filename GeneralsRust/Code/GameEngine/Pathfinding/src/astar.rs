// pathfind_astar.rs
// A* Pathfinding Algorithm - Faithful C++ Port
// Reference: /GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AIPathfind.cpp
//
// The A* search itself: grid storage, the C++ internalFindPath() entry-point
// ladder and its main expansion loop, the examineCellsCallback line seed,
// path reconstruction and checkChangeLayers.

use std::collections::{HashMap, HashSet};

use crate::cell::{
    GridCoord, PathfindCell, PathfindCellType, PathfindLayerEnum, COST_DIAGONAL, COST_ORTHOGONAL,
    PATHFIND_CELL_SIZE_F, SURFACE_AIR, ZONE_IMPASSABLE_COST,
};
use crate::open_set::{AStarNode, OpenSet, SearchKey};

/// A* pathfinding algorithm implementation
/// Matches C++ Pathfinder::internalFindPath() at AIPathfind.cpp:6438-6694
pub struct AStarPathfinder {
    /// C++ `Pathfinder::m_map` — LAYER_GROUND cells.
    pub(crate) grid: Vec<Vec<PathfindCell>>,
    /// C++ `Pathfinder::m_layers[layer]` — elevated layer cells, lazy-allocated.
    /// Missing slot / CELL_IMPASSABLE → C++ `PathfindLayer::getCell` returns NULL
    /// and `Pathfinder::getCell` falls back to `m_map` (ground).
    layer_grids: HashMap<PathfindLayerEnum, Vec<Vec<Option<PathfindCell>>>>,
    pub(crate) width: usize,
    pub(crate) height: usize,
    /// Cell -> owning obstacle object id (C++ PathfindCellInfo::obstacleID).
    /// Keyed by (x, y, layer) so Top/Ground obstacles are independent.
    pub(crate) obstacle_owners: HashMap<(i32, i32, u8), u32>,
    /// C++ PathfindCellInfo::m_obstacleIsFence.
    pub(crate) obstacle_fence: HashSet<(i32, i32, u8)>,
    /// C++ PathfindCellInfo::m_obstacleIsTransparent (KINDOF_CAN_SEE_THROUGH).
    pub(crate) obstacle_transparent: HashSet<(i32, i32, u8)>,
    /// C++ PathfindZoneManager block passable (blockX, blockY).
    /// Only false entries stored; missing = true (default passable).
    pub(crate) zone_impassable_blocks: HashSet<(i32, i32)>,
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
    pub(crate) fn in_bounds(&self, coord: GridCoord) -> bool {
        coord.x >= 0 && coord.x < self.width as i32 && coord.y >= 0 && coord.y < self.height as i32
    }

    #[inline]
    pub(crate) fn is_elevated_layer(layer: PathfindLayerEnum) -> bool {
        (layer as u8) > (PathfindLayerEnum::Ground as u8)
    }

    #[inline]
    pub(crate) fn obstacle_key(coord: GridCoord, layer: PathfindLayerEnum) -> (i32, i32, u8) {
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
    pub(crate) fn get_cell(&self, coord: GridCoord) -> Option<&PathfindCell> {
        self.get_ground_cell(coord)
    }

    pub(crate) fn get_cell_mut(&mut self, coord: GridCoord) -> Option<&mut PathfindCell> {
        self.get_ground_cell_mut(coord)
    }

    pub(crate) fn layer_cell(&self, coord: GridCoord, layer: PathfindLayerEnum) -> Option<&PathfindCell> {
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
    pub(crate) fn get_cell_mut_on_layer(
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
    pub(crate) fn get_cell_on_layer(
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

    pub fn width(&self) -> usize {
        self.width
    }

    pub fn height(&self) -> usize {
        self.height
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
        self.find_path_ex_with_ground_height(
            start,
            goal,
            surfaces,
            is_crusher,
            max_iterations,
            allow_partial,
            ignore_cells,
            extra_cost,
            None,
        )
    }

    /// A* with explicit terrain heights for downhill and cliff cost decisions.
    ///
    /// The callback is evaluated at each cell center in pathfinding-cell coordinates.
    pub(crate) fn find_path_ex_with_ground_height(
        &self,
        start: GridCoord,
        goal: GridCoord,
        surfaces: u32,
        is_crusher: bool,
        max_iterations: usize,
        allow_partial: bool,
        ignore_cells: Option<&HashSet<GridCoord>>,
        extra_cost: Option<&dyn Fn(GridCoord) -> u32>,
        ground_height: Option<&dyn Fn(GridCoord) -> f32>,
    ) -> Option<(Vec<GridCoord>, usize)> {
        // Thin callers (host find_path_via_crate) used to skip layers/zones/
        // occupancy/tunneling. Apply C++ internalFindPath defaults here.
        let start_is_obstacle = self.get_cell_type(start) == Some(PathfindCellType::Obstacle);
        let occupancy = |cell: GridCoord| -> u32 {
            extra_cost.map(|f| f(cell)).unwrap_or(0) + self.cell_occupancy_cost(cell)
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
            ground_height,
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

    /// Layer-preserving `findPathEx6`. The C++ path nodes retain both the cell
    /// pointer and its layer; callers building PathNodes must not infer layers
    /// later from bridge bounds.
    #[allow(clippy::too_many_arguments)]
    pub fn find_path_ex6_with_layers(
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
    ) -> Option<(Vec<(GridCoord, PathfindLayerEnum)>, usize)> {
        self.find_path_with_start_layer_and_layers(
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
        self.find_path_with_start_layer_and_layers(
            start,
            goal,
            start_layer,
            dest_layer,
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
        .map(|(path, examined)| (path.into_iter().map(|(coord, _)| coord).collect(), examined))
    }

    #[allow(clippy::too_many_arguments)]
    pub fn find_path_with_start_layer_and_layers(
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
    ) -> Option<(Vec<(GridCoord, PathfindLayerEnum)>, usize)> {
        // Initialize open and closed sets
        // Matches C++ at AIPathfind.cpp:6575-6581
        let mut open_set = OpenSet::new();
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
            enqueue_order: 0,
        };

        open_set.push(start_node);
        open_members.insert(start_key);
        g_scores.insert(start_key, 0);

        // C++ m_isTunneling — clear once we expand into valid non-pinched cell.
        let mut is_tunneling = starts_tunneling;

        let mut iterations = 0;

        // Main A* loop
        // Matches C++ while loop at AIPathfind.cpp:6589-6633
        while let Some(current) = open_set.pop_live(|node| {
            let key = (node.coord, node.layer);
            open_members.contains(&key)
                && g_scores
                    .get(&key)
                    .map(|&best_g| node.g_score <= best_g)
                    .unwrap_or(true)
        }) {
            let current_key: SearchKey = (current.coord, current.layer);
            iterations += 1;
            if iterations > max_iterations {
                // Prevent infinite loops
                if allow_partial {
                    return Some((
                        self.reconstruct_path_with_layers(&came_from, best_key),
                        iterations,
                    ));
                }
                return None;
            }

            // Popped from open (C++ removeFromOpenList).
            open_members.remove(&current_key);

            // Goal reached!
            // C++ compares PathfindCell pointers: dest XY on destinationLayer.
            if current.coord == goal && current.layer == dest_layer {
                return Some((
                    self.reconstruct_path_with_layers(&came_from, current_key),
                    iterations,
                ));
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
                    enqueue_order: 0,
                };

                open_set.push(neighbor_node);
            }
        }

        // No path found
        // Matches C++ at AIPathfind.cpp:6635-6693
        if allow_partial {
            Some((
                self.reconstruct_path_with_layers(&came_from, best_key),
                iterations,
            ))
        } else {
            None
        }
    }

    /// C++ Pathfinder::examineCellsCallback line seed (AIPathfind.cpp:5996-6093).
    /// Walks Bresenham from parent toward goal; inserts clear cells at half ortho cost.
    /// Unlike examineNeighboringCells, this CAN reopen if the new g is better (C++ 6063-6088).
    pub(crate) fn examine_cells_toward_goal(
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
        open_set: &mut OpenSet,
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
                enqueue_order: 0,
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

    fn reconstruct_path_with_layers(
        &self,
        came_from: &HashMap<SearchKey, SearchKey>,
        mut current: SearchKey,
    ) -> Vec<(GridCoord, PathfindLayerEnum)> {
        let mut path = vec![current];
        while let Some(&parent) = came_from.get(&current) {
            path.push(parent);
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
    pub(crate) fn check_change_layers(
        &self,
        coord: GridCoord,
        current_layer: PathfindLayerEnum,
        parent_g: u32,
        parent_f: u32,
        open_set: &mut OpenSet,
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
        // C++ getCell(connect, x, y) falls back to m_map when an elevated
        // layer has no cell (or its cell is Impassable). Preserve that cell
        // identity here: an absent Top slot resolves to the Ground parent,
        // which is already on the closed list during normal search.
        let resolved_connect = if Self::is_elevated_layer(connect)
            && self
                .layer_cell(coord, connect)
                .is_some_and(|cell| cell.get_type() != PathfindCellType::Impassable)
        {
            connect
        } else {
            PathfindLayerEnum::Ground
        };
        let key: SearchKey = (coord, resolved_connect);
        if open_members.contains(&key) || closed_set.contains(&key) {
            return false;
        }
        let parent_key: SearchKey = (coord, current_layer);
        came_from.insert(key, parent_key);
        g_scores.insert(key, parent_g);
        open_members.insert(key);
        open_set.push(AStarNode {
            coord,
            layer: resolved_connect,
            g_score: parent_g,
            f_score: parent_f,
            parent: Some(parent_key),
            enqueue_order: 0,
        });
        true
    }
}
