use super::*;
use glam::Vec3;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap, HashSet, VecDeque};

use gamelogic::ai::pathfind_astar::{
    AStarPathfinder, GridCoord, PathfindCellType, PathfindLayerEnum, COST_DIAGONAL,
};
use gamelogic::ai::pathfind_complete::{
    LAYER_Z_CLOSE_ENOUGH_F, MAX_PATH_ITERATIONS, PATHFIND_QUEUE_LEN, SURFACE_AIR, SURFACE_CLIFF,
    SURFACE_GROUND, SURFACE_RUBBLE, SURFACE_WATER,
};

/// Host wrapper so `PathfindingSystem` can stay `Debug` while holding crate A*.
struct HostCrateAStar {
    finder: AStarPathfinder,
    stamp: u64,
}

impl std::fmt::Debug for HostCrateAStar {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HostCrateAStar")
            .field("stamp", &self.stamp)
            .finish_non_exhaustive()
    }
}

/// Queued live path request — C++ `Pathfinder::queueForPath` residual.
#[derive(Debug, Clone)]
pub struct PendingHostPath {
    pub unit_id: ObjectId,
    pub start: Vec3,
    pub destination: Vec3,
    pub waypoints: Vec<Vec3>,
    pub aircraft: bool,
    pub surfaces: u32,
    /// C++ `obj->getCrusherLevel() > 0` (AIPathfind.cpp:8170).
    pub is_crusher: bool,
    /// C++ `AIUpdateInterface::ignoreObstacle` (DozerAIUpdate.cpp:210).
    pub ignore_obstacle: Option<ObjectId>,
}

/// Grid-based pathfinding node
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct GridPos {
    pub x: i32,
    pub y: i32,
}

impl GridPos {
    pub fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }

    pub fn to_world_pos(&self, grid_size: f32) -> Vec3 {
        Vec3::new(self.x as f32 * grid_size, 0.0, self.y as f32 * grid_size)
    }

    pub fn from_world_pos(world_pos: Vec3, grid_size: f32) -> Self {
        // C++ Pathfinder::worldToGrid: REAL_TO_INT(pos/PATHFIND_CELL_SIZE)
        // (AIPathfind.h:856-858). REAL_TO_INT truncates toward zero
        // (BaseType.h:213), not round and not floor-toward--inf.
        Self {
            x: (world_pos.x / grid_size) as i32,
            y: (world_pos.z / grid_size) as i32,
        }
    }

    pub fn distance(&self, other: GridPos) -> f32 {
        let dx = (self.x - other.x) as f32;
        let dy = (self.y - other.y) as f32;
        (dx * dx + dy * dy).sqrt()
    }

    pub fn manhattan_distance(&self, other: GridPos) -> i32 {
        (self.x - other.x).abs() + (self.y - other.y).abs()
    }

    pub fn neighbors(&self) -> Vec<GridPos> {
        vec![
            GridPos::new(self.x + 1, self.y),
            GridPos::new(self.x - 1, self.y),
            GridPos::new(self.x, self.y + 1),
            GridPos::new(self.x, self.y - 1),
            // Diagonal neighbors
            GridPos::new(self.x + 1, self.y + 1),
            GridPos::new(self.x + 1, self.y - 1),
            GridPos::new(self.x - 1, self.y + 1),
            GridPos::new(self.x - 1, self.y - 1),
        ]
    }
}

/// A* pathfinding node
#[derive(Debug, Clone)]
struct PathNode {
    pos: GridPos,
    g_cost: f32, // Cost from start
    h_cost: f32, // Heuristic cost to goal
    parent: Option<GridPos>,
}

impl PathNode {
    fn new(pos: GridPos, g_cost: f32, h_cost: f32, parent: Option<GridPos>) -> Self {
        Self {
            pos,
            g_cost,
            h_cost,
            parent,
        }
    }

    fn f_cost(&self) -> f32 {
        self.g_cost + self.h_cost
    }
}

impl PartialEq for PathNode {
    fn eq(&self, other: &Self) -> bool {
        self.pos == other.pos
    }
}

impl Eq for PathNode {}

impl PartialOrd for PathNode {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for PathNode {
    fn cmp(&self, other: &Self) -> Ordering {
        // Reverse ordering for min-heap behavior
        other
            .f_cost()
            .partial_cmp(&self.f_cost())
            .unwrap_or(Ordering::Equal)
    }
}

/// C++ `PathfindLayer` slot 2..=14 — classified cells, not flattened onto `m_map`.
#[derive(Debug, Clone)]
struct HostBridgeLayer {
    id: u8,
    from_left: Vec3,
    from_right: Vec3,
    to_left: Vec3,
    to_right: Vec3,
    /// Non-IMPASSABLE layer cells: (x,y) → (type, connectLayer).
    cells: HashMap<(i32, i32), (PathfindCellType, u8)>,
    /// C++ `PathfindLayer::m_destroyed`.
    destroyed: bool,
    /// C++ `PathfindLayer::getBridgeID`.
    object_id: u32,
    /// C++ layer cells with `getConnectLayer()==LAYER_GROUND`.
    ground_connect_cells: Vec<(i32, i32)>,
}

/// C++ PathfindLayer pos/goal occupancy (`updatePos` / `updateGoal`).
#[derive(Debug, Clone, Default)]
struct LayerOccupancy {
    occ_fixed_mask: HashMap<(i32, i32), u16>,
    occ_moving_mask: HashMap<(i32, i32), u16>,
    occ_goal_mask: HashMap<(i32, i32), u16>,
    occ_infantry_mask: HashMap<(i32, i32), u16>,
    occ_fixed_max_crushable: HashMap<(i32, i32), u8>,
    occ_goal_unit: HashMap<(i32, i32), u32>,
}

#[derive(Clone, Copy, Default)]
struct OccBits {
    fixed: u16,
    moving: u16,
    goal: u16,
    infantry: u16,
    crushable: u8,
    goal_unit: u32,
}

/// C++ `LAYER_WALL = LAYER_LAST = 15` (GameType.h:167).
const LAYER_WALL_ID: u8 = 15;
/// C++ `MAX_WALL_PIECES` (AIPathfind.h:181).
const MAX_WALL_PIECES: usize = 128;
/// C++ `PATHFIND_CELLS_PER_FRAME` (AIPathfind.cpp:1079).
const PATHFIND_CELLS_PER_FRAME: i32 = 5000;

/// Registered `KINDOF_WALK_ON_TOP_OF_WALL` piece (C++ `m_wallPieces`).
#[derive(Debug, Clone)]
struct HostWallPiece {
    id: u32,
    pos_x: f32,
    pos_z: f32,
    orientation: f32,
    major: f32,
    minor: f32,
}

/// Pathfinding grid
#[derive(Debug, Clone)]
pub struct PathfindingGrid {
    width: i32,
    height: i32,
    grid_size: f32,
    origin: Vec3,
    /// Static obstacle bits (row-major y*width+x). A* hot path — no HashSet.
    blocked_bits: Vec<u64>,
    /// Dynamic vehicle/structure occupancy bits (cleared each rebuild).
    dynamic_bits: Vec<u64>,
    /// C++ PathfindCell::CellType residual (AIPathfind.h:233-242).
    /// Parallel to blocked_bits so water/cliff stay distinct from IMPASSABLE.
    cell_types: Vec<u8>,
    /// C++ PathfindCell::m_pinched (AIPathfind.h:347).
    pinched_bits: Vec<u64>,
    /// Terrain-only zone ids (structures ignored). 0 = uninitialized.
    terrain_zones: Vec<u16>,
    /// Players with a stopped/fixed occupant in this cell (bit i = player i).
    occ_fixed_mask: Vec<u16>,
    /// Players with a moving occupant in this cell.
    occ_moving_mask: Vec<u16>,
    /// C++ PathfindCellInfo::m_obstacleIsFence.
    fence_bits: Vec<u64>,
    /// C++ PathfindCellInfo::m_obstacleIsTransparent (KINDOF_CAN_SEE_THROUGH).
    transparent_bits: Vec<u64>,
    /// Players with a UNIT_GOAL reservation in this cell (bit i = player i).
    occ_goal_mask: Vec<u16>,
    /// Last UNIT_GOAL writer (C++ `PathfindCell::getGoalUnit`). 0 = none.
    occ_goal_unit: Vec<u32>,
    /// C++ `PathfindCell::getGoalAircraft` (LAYER_GROUND). 0 = none.
    occ_goal_aircraft: Vec<u32>,
    /// Players with infantry occupying this cell.
    occ_infantry_mask: Vec<u16>,
    /// Max CrushableLevel among fixed occupants (canCrushOrSquish).
    occ_fixed_max_crushable: Vec<u8>,
    /// Structure-aware zone ids (obstacles split zones). 0 = uninitialized.
    path_zones: Vec<u16>,
    /// C++ `m_groundWaterZones` — leftover `build_surface_combiners`.
    ground_water_zones: Vec<u16>,
    /// C++ `m_groundCliffZones` — leftover `build_surface_combiners`.
    ground_cliff_zones: Vec<u16>,
    /// C++ `m_groundRubbleZones` — leftover `build_surface_combiners`.
    ground_rubble_zones: Vec<u16>,
    /// C++ `m_crusherZones` — leftover `build_surface_combiners` fence↔clear.
    crusher_zones: Vec<u16>,
    /// Per-player ALLIES occupancy bits (bit j set if player i considers j an ally).
    /// C++ checkForMovement getRelationship == ALLIES (AIPathfind.cpp:5037).
    player_ally_masks: [u16; 16],
    /// C++ `m_map[i][j].connectLayer` (0 = LAYER_INVALID).
    ground_connect: Vec<u8>,
    /// C++ `Pathfinder::m_layers[2..=14]`.
    bridge_layers: Vec<HostBridgeLayer>,
    /// C++ PathfindLayer pos/goal occupancy (updatePos/updateGoal).
    layer_occ: HashMap<u8, LayerOccupancy>,
    /// C++ `m_wallPieces` geometry for `allocateCellsForWallLayer`.
    wall_pieces: Vec<HostWallPiece>,
    /// C++ `m_layers[LAYER_WALL]` classified non-IMPASSABLE cells.
    wall_cells: HashMap<(i32, i32), PathfindCellType>,
    /// C++ `m_wallHeight`.
    wall_height: f32,

    /// Bump when static cells change so crate A* can resync.
    terrain_gen: u64,
    /// C++ `pathDiameter` for the active query (AIPathfind.cpp:6700). 1 = infantry.
    query_path_diameter: i32,
    /// Crusher flag paired with `query_path_diameter`.
    query_is_crusher: bool,
    /// C++ `adjustDestination` / `internalFindPath` layer for this query.
    query_layer: u8,
    /// Seeker ObjectID for `checkDestination` own-goal skip.
    query_seeker_id: u32,
    /// C++ `checkDestination` `checkForAircraft` (HOVER/WINGS).
    query_check_for_aircraft: bool,
    /// C++ `checkForAdjust` unit position (leftover `check_for_adjust_ex` from).
    query_from: Option<GridPos>,
    /// Original click cell for leftover dest→adjust zone fallback.
    query_orig_dest: Option<GridPos>,
    /// C++ `m_logicalExtent` — playable terrain cells (human path clamp).
    logical_extent_lo: GridPos,
    logical_extent_hi: GridPos,
    /// Constructor world size used when leftover terrain extent is empty.
    world_extent_w: f32,
    world_extent_h: f32,
    /// C++ `checkForAdjust` / `examineNeighboringCells` `isHuman`.
    query_is_human: bool,
    /// C++ `PathfindCell::getObstacleID`.
    occ_obstacle_id: Vec<u32>,
    /// Obstacle controlling player (`0xFF` = unknown).
    occ_obstacle_owner: Vec<u8>,
    /// Obstacle `Team` discriminant (`0xFF` = unknown).
    occ_obstacle_team: Vec<u8>,
    /// C++ `KINDOF_BLAST_CRATER` cells stay classified after the object dies
    /// (`classifyObjectFootprint` never-remove, AIPathfind.cpp:4121-4122).
    permanent_blast_crater_cells: HashSet<(i32, i32)>,
}

mod grid_core;
mod grid_layers;
mod grid_routing;
mod system_attack;
mod system_requests;
mod system_routes;

fn point_in_bridge_quad(px: f32, pz: f32, corners: &[Vec3; 4]) -> bool {
    // Convex quad test in XZ (host ground). C++ classifyLayerMapCell deck fill.
    let mut sign = 0.0f32;
    for i in 0..4 {
        let a = corners[i];
        let b = corners[(i + 1) % 4];
        let cross = (b.x - a.x) * (pz - a.z) - (b.z - a.z) * (px - a.x);
        if cross.abs() < f32::EPSILON {
            continue;
        }
        if sign == 0.0 {
            sign = cross;
        } else if sign * cross < 0.0 {
            return false;
        }
    }
    true
}

struct CellXz {
    lo_x: f32,
    lo_z: f32,
    hi_x: f32,
    hi_z: f32,
}

fn span_xz_eq(a: Vec3, b: Vec3) -> bool {
    (a.x - b.x).abs() < 0.5 && (a.z - b.z).abs() < 0.5
}

/// C++ `LineInRegion` on the host XZ ground plane.
fn line_in_region_xz(x0: f32, z0: f32, x1: f32, z1: f32, cell: &CellXz) -> bool {
    let dx = x1 - x0;
    let dz = z1 - z0;
    let mut t0 = 0.0f32;
    let mut t1 = 1.0f32;
    let clip = |p: f32, q: f32, t0: &mut f32, t1: &mut f32| -> bool {
        if p.abs() <= f32::EPSILON {
            return q >= 0.0;
        }
        let r = q / p;
        if p < 0.0 {
            if r > *t1 {
                return false;
            }
            if r > *t0 {
                *t0 = r;
            }
        } else {
            if r < *t0 {
                return false;
            }
            if r < *t1 {
                *t1 = r;
            }
        }
        true
    };
    if !clip(-dx, x0 - cell.lo_x, &mut t0, &mut t1) {
        return false;
    }
    if !clip(dx, cell.hi_x - x0, &mut t0, &mut t1) {
        return false;
    }
    if !clip(-dz, z0 - cell.lo_z, &mut t0, &mut t1) {
        return false;
    }
    if !clip(dz, cell.hi_z - z0, &mut t0, &mut t1) {
        return false;
    }
    t0 <= t1
}

fn xz_len(x: f32, z: f32) -> f32 {
    (x * x + z * z).sqrt()
}

/// C++ `Bridge::isCellOnEnd` (TerrainLogic.cpp:624-671), host XZ.
fn cell_on_bridge_end(
    cell: &CellXz,
    from_left: Vec3,
    from_right: Vec3,
    to_left: Vec3,
    to_right: Vec3,
    cell_size: f32,
) -> bool {
    let mut ex = from_right.x - from_left.x;
    let mut ez = from_right.z - from_left.z;
    let len = xz_len(ex, ez);
    if len <= f32::EPSILON {
        return false;
    }
    ex = ex / len * cell_size;
    ez = ez / len * cell_size;
    let from_l = (from_left.x + ex, from_left.z + ez);
    let from_r = (from_right.x - ex, from_right.z - ez);
    let to_l = (to_left.x + ex, to_left.z + ez);
    let to_r = (to_right.x - ex, to_right.z - ez);
    line_in_region_xz(from_l.0, from_l.1, from_r.0, from_r.1, cell)
        || line_in_region_xz(to_l.0, to_l.1, to_r.0, to_r.1, cell)
}

/// C++ `Bridge::isCellOnSide` (TerrainLogic.cpp:676-745), host XZ.
fn cell_on_bridge_side(
    cell: &CellXz,
    from_left: Vec3,
    from_right: Vec3,
    to_left: Vec3,
    to_right: Vec3,
    cell_size: f32,
) -> bool {
    let mut ex = from_right.x - from_left.x;
    let mut ez = from_right.z - from_left.z;
    let len = xz_len(ex, ez);
    if len <= f32::EPSILON {
        return false;
    }
    ex = ex / len * cell_size * 0.51;
    ez = ez / len * cell_size * 0.51;
    let mut from_l = (from_left.x - ex, from_left.z - ez);
    let mut from_r = (from_right.x + ex, from_right.z + ez);
    let mut to_l = (to_left.x - ex, to_left.z - ez);
    let mut to_r = (to_right.x + ex, to_right.z + ez);
    if line_in_region_xz(from_l.0, from_l.1, to_l.0, to_l.1, cell)
        || line_in_region_xz(from_r.0, from_r.1, to_r.0, to_r.1, cell)
    {
        return true;
    }
    from_l = (from_l.0 - ex, from_l.1 - ez);
    from_r = (from_r.0 + ex, from_r.1 + ez);
    to_l = (to_l.0 - ex, to_l.1 - ez);
    to_r = (to_r.0 + ex, to_r.1 + ez);
    line_in_region_xz(from_l.0, from_l.1, to_l.0, to_l.1, cell)
        || line_in_region_xz(from_r.0, from_r.1, to_r.0, to_r.1, cell)
}

/// C++ `Bridge::isCellEntryPoint` (TerrainLogic.cpp:750-814), host XZ.
fn cell_is_bridge_entry(
    cell: &CellXz,
    from_left: Vec3,
    from_right: Vec3,
    to_left: Vec3,
    to_right: Vec3,
    cell_size: f32,
) -> bool {
    let mut ex = from_right.x - from_left.x;
    let mut ez = from_right.z - from_left.z;
    let elen = xz_len(ex, ez);
    if elen <= f32::EPSILON {
        return false;
    }
    ex = ex / elen * cell_size;
    ez = ez / elen * cell_size;
    let from = (
        (from_left.x + from_right.x) * 0.5,
        (from_left.z + from_right.z) * 0.5,
    );
    let to = (
        (to_left.x + to_right.x) * 0.5,
        (to_left.z + to_right.z) * 0.5,
    );
    let mut bx = to.0 - from.0;
    let mut bz = to.1 - from.1;
    let blen = xz_len(bx, bz);
    if blen <= f32::EPSILON {
        return false;
    }
    bx = bx / blen * (cell_size * 0.5);
    bz = bz / blen * (cell_size * 0.5);
    let from_l = (from_left.x - bx + ex, from_left.z - bz + ez);
    let from_r = (from_right.x - bx - ex, from_right.z - bz - ez);
    let to_l = (to_left.x + bx + ex, to_left.z + bz + ez);
    let to_r = (to_right.x + bx - ex, to_right.z + bz - ez);
    line_in_region_xz(from_l.0, from_l.1, from_r.0, from_r.1, cell)
        || line_in_region_xz(to_l.0, to_l.1, to_r.0, to_r.1, cell)
}

/// Host-Y plane through fromLeft/fromRight/toLeft (C++ `Bridge::getBridgeHeight`).
fn bridge_deck_height(corners: &[Vec3; 4], x: f32, z: f32) -> f32 {
    let p1 = corners[0];
    let p2 = corners[1];
    let p3 = corners[3];
    let v1 = p2 - p1;
    let v2 = p3 - p1;
    let n = v1.cross(v2);
    if n.y.abs() <= f32::EPSILON {
        return p1.y;
    }
    p1.y - (n.x * (x - p1.x) + n.z * (z - p1.z)) / n.y
}

fn sample_host_ground_height(x: f32, z: f32) -> f32 {
    gamelogic::terrain::get_terrain_logic()
        .read()
        .ok()
        .map(|t| t.get_ground_height(x, z, None))
        .unwrap_or(0.0)
}

/// Flow field pathfinding for RTS-style unit movement
#[derive(Debug, Clone)]
pub struct FlowField {
    width: i32,
    height: i32,
    grid_size: f32,
    origin: Vec3,
    integration_field: HashMap<GridPos, f32>,
    flow_field: HashMap<GridPos, Vec3>,
}

impl FlowField {
    pub fn new(world_width: f32, world_height: f32, grid_size: f32) -> Self {
        Self::new_with_origin(Vec3::ZERO, world_width, world_height, grid_size)
    }

    pub fn new_with_origin(
        origin: Vec3,
        world_width: f32,
        world_height: f32,
        grid_size: f32,
    ) -> Self {
        Self {
            width: (world_width / grid_size).ceil() as i32,
            height: (world_height / grid_size).ceil() as i32,
            grid_size,
            origin,
            integration_field: HashMap::new(),
            flow_field: HashMap::new(),
        }
    }

    /// Generate flow field toward a goal
    pub fn generate_flow_field(&mut self, goal: GridPos, pathfinding_grid: &PathfindingGrid) {
        self.integration_field.clear();
        self.flow_field.clear();

        // Initialize integration field
        let mut open_set = BinaryHeap::new();
        self.integration_field.insert(goal, 0.0);
        open_set.push((0, goal)); // (negative cost, position) for min-heap

        // Dijkstra's algorithm to fill integration field
        while let Some((neg_cost, current)) = open_set.pop() {
            let current_cost = (-neg_cost) as f32;

            if current_cost
                > *self
                    .integration_field
                    .get(&current)
                    .unwrap_or(&f32::INFINITY)
            {
                continue;
            }

            for neighbor in current.neighbors() {
                if !pathfinding_grid.is_valid_pos(neighbor) || pathfinding_grid.is_blocked(neighbor)
                {
                    continue;
                }

                let movement_cost =
                    if (neighbor.x - current.x).abs() == 1 && (neighbor.y - current.y).abs() == 1 {
                        1.414_213_5
                    } else {
                        1.0
                    };

                let new_cost = current_cost + movement_cost;

                if new_cost
                    < *self
                        .integration_field
                        .get(&neighbor)
                        .unwrap_or(&f32::INFINITY)
                {
                    self.integration_field.insert(neighbor, new_cost);
                    open_set.push((-((new_cost * 1000.0) as i32), neighbor));
                }
            }
        }

        // Generate flow vectors
        for (&pos, &cost) in &self.integration_field {
            let mut best_neighbor = pos;
            let mut best_cost = cost;

            for neighbor in pos.neighbors() {
                if let Some(&neighbor_cost) = self.integration_field.get(&neighbor) {
                    if neighbor_cost < best_cost {
                        best_cost = neighbor_cost;
                        best_neighbor = neighbor;
                    }
                }
            }

            if best_neighbor != pos {
                let direction = Vec3::new(
                    (best_neighbor.x - pos.x) as f32,
                    0.0,
                    (best_neighbor.y - pos.y) as f32,
                )
                .normalize_or_zero();

                self.flow_field.insert(pos, direction);
            }
        }
    }

    /// Get flow direction at world position
    pub fn get_flow_direction(&self, world_pos: Vec3) -> Vec3 {
        let grid_pos = GridPos {
            x: ((world_pos.x - self.origin.x) / self.grid_size).round() as i32,
            y: ((world_pos.z - self.origin.z) / self.grid_size).round() as i32,
        };
        self.flow_field
            .get(&grid_pos)
            .copied()
            .unwrap_or(Vec3::ZERO)
    }
}

/// C++ `Pathfinder::clearCellForDiameter` (AIPathfind.cpp:6700-6759).
fn clear_cell_for_diameter_impl(
    width: i32,
    height: i32,
    cell_types: &[u8],
    fence_bits: &[u64],
    occ_fixed: &[u16],
    occ_crush: &[u8],
    crusher: bool,
    cell: GridPos,
    path_diameter: i32,
) -> i32 {
    if path_diameter <= 0 {
        return 0;
    }
    let radius = path_diameter / 2;
    let mut num_cells_above = radius;
    if radius == 0 {
        num_cells_above += 1;
    }
    let cut_corners = radius > 1;
    let mut clear = true;
    'outer: for i in (cell.x - radius)..(cell.x + num_cells_above) {
        let x_min_or_max = i == cell.x - radius || i == cell.x + num_cells_above - 1;
        for j in (cell.y - radius)..(cell.y + num_cells_above) {
            let y_min_or_max = j == cell.y - radius || j == cell.y + num_cells_above - 1;
            if x_min_or_max && y_min_or_max && cut_corners {
                continue;
            }
            if i < 0 || j < 0 || i >= width || j >= height {
                return 0;
            }
            let idx = j as usize * width as usize + i as usize;
            let ty = match cell_types.get(idx).copied().unwrap_or(0) {
                0x01 => PathfindCellType::Water,
                0x02 => PathfindCellType::Cliff,
                0x03 => PathfindCellType::Rubble,
                0x04 => PathfindCellType::Obstacle,
                0x05 => PathfindCellType::BridgeImpassable,
                0x06 => PathfindCellType::Impassable,
                _ => PathfindCellType::Clear,
            };
            if ty != PathfindCellType::Clear {
                if ty == PathfindCellType::Obstacle {
                    if PathfindingGrid::bit_test(fence_bits, idx) {
                        if !crusher {
                            clear = false;
                        }
                    } else {
                        clear = false;
                    }
                } else {
                    clear = false;
                }
            }
            if path_diameter >= 2 {
                let fixed = occ_fixed.get(idx).copied().unwrap_or(0);
                if fixed != 0 {
                    let crushable = occ_crush.get(idx).copied().unwrap_or(0);
                    if crusher {
                        if crushable > 1 {
                            clear = false;
                        }
                    } else if crushable > 0 {
                        clear = false;
                    }
                }
            }
            if !clear {
                break 'outer;
            }
        }
    }
    if clear {
        if radius == 0 {
            return 1;
        }
        return 2 * radius;
    }
    if path_diameter < 2 {
        return 0;
    }
    clear_cell_for_diameter_impl(
        width,
        height,
        cell_types,
        fence_bits,
        occ_fixed,
        occ_crush,
        crusher,
        cell,
        path_diameter - 2,
    )
}

/// Main pathfinding system
#[derive(Debug)]
pub struct PathfindingSystem {
    pub grid: PathfindingGrid,
    flow_fields: HashMap<ObjectId, FlowField>, // Flow fields for different goals
    /// Active host logic frame (set via note_logic_frame).
    logic_frame: u64,
    /// Frame stamp of last dynamic obstacle rebuild.
    dynamic_obstacle_frame: u64,
    /// Live crate A* (AIPathfind.cpp internalFindPath).
    crate_astar: Option<HostCrateAStar>,
    /// C++ Pathfinder::queueForPath residual (AIPathfind.h:418).
    pending_paths: VecDeque<PendingHostPath>,
    /// Seeker controlling player for occupancy costs this query.
    seeker_player: Option<u32>,
    /// Seeker is infantry (INFANTRY_MOVES_THROUGH_INFANTRY).
    seeker_is_infantry: bool,
    /// Seeker uses LOCO_WINGS (circleClips only for circling aircraft).
    seeker_wings: bool,
    /// Seeker object id for moveAllies / UNIT_GOAL.
    seeker_id: Option<ObjectId>,
    /// Seeker team for ally checks.
    seeker_team: Option<Team>,
    /// Seeker CrusherLevel for canCrushOrSquish occupancy (AIPathfind.cpp:5063).
    seeker_crusher_level: u8,
    /// C++ `pathDiameter` from getRadiusAndCenter (vehicles, MAX_RADIUS=2).
    seeker_path_diameter: i32,
    /// C++ `centerInCell` from getRadiusAndCenter (odd-diameter / infantry).
    seeker_center_in_cell: bool,
    /// C++ `m_ignoreObstacleID` for this path query (DozerAIUpdate.cpp:210).
    ignore_obstacle_id: Option<ObjectId>,
    /// C++ `getPlayerType() == PLAYER_HUMAN` bits (bit i = player i).
    human_player_mask: u16,
    /// Seeker is a human player (m_logicalExtent clamp).
    seeker_is_human: bool,
    /// C++ `KINDOF_DOZER` for dozerHack.
    seeker_is_dozer: bool,
    /// C++ `locomotorSet.isDownhillOnly()`.
    seeker_downhill_only: bool,
    /// C++ `AIUpdateInterface::canPathThroughUnits` for findClosestPath goal accept.
    seeker_can_path_through_units: bool,
    /// Coarse pathfind-cell heights for LOS_TERRAIN (findAttackPath A*).
    terrain_height_samples: Option<(i32, i32, Vec<f32>)>,
    /// C++ `m_cumulativeCellsAllocated` this processPathfindQueue call.
    cumulative_cells_allocated: i32,
    /// C++ `PATHFIND_CELLS_PER_FRAME` (test-overridable).
    pathfind_cells_per_frame: i32,
}

#[cfg(test)]
mod tests;
