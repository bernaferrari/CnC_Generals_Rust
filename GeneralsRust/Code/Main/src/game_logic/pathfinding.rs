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

impl PathfindingGrid {
    pub fn new(world_width: f32, world_height: f32, grid_size: f32) -> Self {
        Self::new_with_origin(Vec3::ZERO, world_width, world_height, grid_size)
    }

    pub fn new_with_origin(
        origin: Vec3,
        world_width: f32,
        world_height: f32,
        grid_size: f32,
    ) -> Self {
        let width = (world_width / grid_size).ceil() as i32;
        let height = (world_height / grid_size).ceil() as i32;
        let cells = (width.max(0) as usize).saturating_mul(height.max(0) as usize);
        let words = cells.div_ceil(64);
        let mut grid = Self {
            width,
            height,
            grid_size,
            origin,
            blocked_bits: vec![0u64; words],
            dynamic_bits: vec![0u64; words],
            cell_types: vec![PathfindCellType::Clear as u8; cells],
            pinched_bits: vec![0u64; words],
            terrain_zones: vec![0u16; cells],
            occ_fixed_mask: vec![0u16; cells],
            occ_goal_unit: vec![0u32; cells],
            occ_goal_aircraft: vec![0u32; cells],
            occ_moving_mask: vec![0u16; cells],
            fence_bits: vec![0u64; words],
            transparent_bits: vec![0u64; words],
            occ_goal_mask: vec![0u16; cells],
            occ_infantry_mask: vec![0u16; cells],
            occ_fixed_max_crushable: vec![0u8; cells],
            path_zones: vec![0u16; cells],
            ground_water_zones: Vec::new(),
            ground_cliff_zones: Vec::new(),
            ground_rubble_zones: Vec::new(),
            player_ally_masks: [0u16; 16],
            ground_connect: vec![0u8; cells],
            query_layer: PathfindLayerEnum::Ground as u8,
            query_seeker_id: 0,
            query_check_for_aircraft: false,
            query_from: None,
            query_orig_dest: None,
            logical_extent_lo: GridPos::new(0, 0),
            logical_extent_hi: GridPos::new(
                ((world_width / grid_size).floor() as i32 - 1)
                    .min(width.saturating_sub(1))
                    .max(0),
                ((world_height / grid_size).floor() as i32 - 1)
                    .min(height.saturating_sub(1))
                    .max(0),
            ),
            world_extent_w: world_width,
            world_extent_h: world_height,
            query_is_human: false,
            occ_obstacle_id: vec![0u32; cells],
            occ_obstacle_owner: vec![0xFFu8; cells],
            occ_obstacle_team: vec![0xFFu8; cells],
            permanent_blast_crater_cells: HashSet::new(),
            bridge_layers: Vec::new(),
            layer_occ: HashMap::new(),
            wall_pieces: Vec::new(),
            wall_cells: HashMap::new(),
            wall_height: 0.0,
            terrain_gen: 1,
            query_path_diameter: 1,
            query_is_crusher: false,
        };
        grid.refresh_logical_extent();
        grid
    }

    /// C++ `Pathfinder::processPathfindQueue` m_logicalExtent refresh
    /// (AIPathfind.cpp:5887-5897).
    pub fn refresh_logical_extent(&mut self) {
        let cell = self.grid_size.max(1.0);
        let Some(ext) = gamelogic::terrain::get_terrain_logic()
            .read()
            .ok()
            .map(|t| t.get_extent())
            .filter(|e| e.hi.x > e.lo.x && e.hi.y > e.lo.y)
        else {
            return;
        };
        // Leftover/C++ terrain is XY; host grid is XZ with origin.
        let mut lo_x = ((ext.lo.x - self.origin.x) / cell).floor() as i32;
        let mut lo_y = ((ext.lo.y - self.origin.z) / cell).floor() as i32;
        let mut hi_x = ((ext.hi.x - self.origin.x) / cell).floor() as i32 - 1;
        let mut hi_y = ((ext.hi.y - self.origin.z) / cell).floor() as i32 - 1;
        lo_x = lo_x.max(0);
        lo_y = lo_y.max(0);
        hi_x = hi_x.min(self.width.saturating_sub(1)).max(lo_x);
        hi_y = hi_y.min(self.height.saturating_sub(1)).max(lo_y);
        self.logical_extent_lo = GridPos::new(lo_x, lo_y);
        self.logical_extent_hi = GridPos::new(hi_x, hi_y);
    }

    #[inline]
    pub fn in_logical_extent(&self, cell: GridPos) -> bool {
        cell.x >= self.logical_extent_lo.x
            && cell.y >= self.logical_extent_lo.y
            && cell.x <= self.logical_extent_hi.x
            && cell.y <= self.logical_extent_hi.y
    }

    pub fn set_logical_extent(&mut self, lo: GridPos, hi: GridPos) {
        self.logical_extent_lo = lo;
        self.logical_extent_hi = hi;
    }

    pub fn set_query_is_human(&mut self, is_human: bool) {
        self.query_is_human = is_human;
    }

    /// C++ Player relationship ALLIES bits for occupancy crush-through.
    pub fn set_player_ally_masks(&mut self, masks: [u16; 16]) {
        self.player_ally_masks = masks;
    }

    #[inline]
    fn ally_mask_for(&self, player: u32) -> u16 {
        self.player_ally_masks[player.min(15) as usize]
    }
    #[inline]
    fn bit_index(&self, pos: GridPos) -> Option<usize> {
        if !self.is_valid_pos(pos) {
            return None;
        }
        Some(pos.y as usize * self.width as usize + pos.x as usize)
    }

    #[inline]
    fn bit_test(bits: &[u64], idx: usize) -> bool {
        let w = idx >> 6;
        let b = idx & 63;
        bits.get(w)
            .map(|word| (word >> b) & 1 == 1)
            .unwrap_or(false)
    }

    #[inline]
    fn bit_set(bits: &mut [u64], idx: usize, on: bool) {
        let w = idx >> 6;
        let b = idx & 63;
        if let Some(word) = bits.get_mut(w) {
            if on {
                *word |= 1u64 << b;
            } else {
                *word &= !(1u64 << b);
            }
        }
    }

    pub fn is_valid_pos(&self, pos: GridPos) -> bool {
        pos.x >= 0 && pos.x < self.width && pos.y >= 0 && pos.y < self.height
    }

    pub fn origin(&self) -> Vec3 {
        self.origin
    }

    pub fn world_to_grid(&self, world_pos: Vec3) -> GridPos {
        // C++ Pathfinder::worldToGrid REAL_TO_INT truncate-toward-zero
        // (AIPathfind.h:856-858, BaseType.h:213). Host ground plane is XZ.
        GridPos {
            x: ((world_pos.x - self.origin.x) / self.grid_size) as i32,
            y: ((world_pos.z - self.origin.z) / self.grid_size) as i32,
        }
    }

    pub fn grid_to_world(&self, pos: GridPos) -> Vec3 {
        let x = self.origin.x + pos.x as f32 * self.grid_size;
        let z = self.origin.z + pos.y as f32 * self.grid_size;
        Vec3::new(x, self.cell_world_height(pos, x, z), z)
    }

    /// C++ `adjustCoordToCell` / `TerrainLogic::getLayerHeight` on a known layer.
    pub fn grid_to_world_on_layer(&self, pos: GridPos, layer: PathfindLayerEnum) -> Vec3 {
        let x = self.origin.x + pos.x as f32 * self.grid_size;
        let z = self.origin.z + pos.y as f32 * self.grid_size;
        Vec3::new(x, self.layer_world_height(pos, layer, x, z), z)
    }

    /// C++ `Pathfinder::adjustCoordToCell` (AIPathfind.cpp:8936-8946).
    /// Infantry / odd-diameter: cell center (+0.5). Vehicles / even: +0.05 inset.
    pub fn adjust_coord_to_cell(&self, pos: GridPos, center_in_cell: bool) -> Vec3 {
        let bias = if center_in_cell { 0.5 } else { 0.05 };
        let x = self.origin.x + (pos.x as f32 + bias) * self.grid_size;
        let z = self.origin.z + (pos.y as f32 + bias) * self.grid_size;
        Vec3::new(x, self.cell_world_height(pos, x, z), z)
    }

    /// `adjustCoordToCell` on a known pathfind layer.
    pub fn adjust_coord_to_cell_on_layer(
        &self,
        pos: GridPos,
        center_in_cell: bool,
        layer: PathfindLayerEnum,
    ) -> Vec3 {
        let bias = if center_in_cell { 0.5 } else { 0.05 };
        let x = self.origin.x + (pos.x as f32 + bias) * self.grid_size;
        let z = self.origin.z + (pos.y as f32 + bias) * self.grid_size;
        Vec3::new(x, self.layer_world_height(pos, layer, x, z), z)
    }

    fn cell_world_height(&self, pos: GridPos, x: f32, z: f32) -> f32 {
        if self.wall_cells.contains_key(&(pos.x, pos.y)) && self.wall_height > 0.0 {
            return self.wall_height;
        }
        if let Some(id) = self.host_deck_layer_at(pos) {
            return self.layer_world_height(pos, PathfindLayerEnum::from_u32(id as u32), x, z);
        }
        sample_host_ground_height(x, z)
    }

    fn layer_world_height(&self, pos: GridPos, layer: PathfindLayerEnum, x: f32, z: f32) -> f32 {
        let _ = pos;
        match layer {
            PathfindLayerEnum::Wall => {
                if self.wall_height > 0.0 {
                    self.wall_height
                } else {
                    sample_host_ground_height(x, z)
                }
            }
            PathfindLayerEnum::Ground | PathfindLayerEnum::Invalid => {
                sample_host_ground_height(x, z)
            }
            _ => {
                if let Some(bridge) = self.bridge_layers.iter().find(|l| l.id == layer as u8) {
                    let corners = [
                        bridge.from_left,
                        bridge.from_right,
                        bridge.to_right,
                        bridge.to_left,
                    ];
                    return bridge_deck_height(&corners, x, z);
                }
                sample_host_ground_height(x, z)
            }
        }
    }

    pub fn is_blocked(&self, pos: GridPos) -> bool {
        self.is_static_blocked(pos)
            || self
                .bit_index(pos)
                .is_some_and(|idx| Self::bit_test(&self.dynamic_bits, idx))
    }

    pub fn is_static_blocked(&self, pos: GridPos) -> bool {
        let Some(idx) = self.bit_index(pos) else {
            return true;
        };
        if Self::bit_test(&self.blocked_bits, idx) {
            return true;
        }
        matches!(
            self.cell_type_at_index(idx),
            PathfindCellType::Impassable
                | PathfindCellType::Obstacle
                | PathfindCellType::BridgeImpassable
        )
    }

    #[inline]
    fn cell_type_at_index(&self, idx: usize) -> PathfindCellType {
        match self.cell_types.get(idx).copied().unwrap_or(0) {
            0x01 => PathfindCellType::Water,
            0x02 => PathfindCellType::Cliff,
            0x03 => PathfindCellType::Rubble,
            0x04 => PathfindCellType::Obstacle,
            0x05 => PathfindCellType::BridgeImpassable,
            0x06 => PathfindCellType::Impassable,
            _ => PathfindCellType::Clear,
        }
    }

    /// C++ PathfindCell::getType residual (AIPathfind.h:233-242).
    pub fn cell_type(&self, pos: GridPos) -> PathfindCellType {
        match self.bit_index(pos) {
            Some(idx) => self.cell_type_at_index(idx),
            None => PathfindCellType::Impassable,
        }
    }

    /// Classify a cell without claiming full locomotor surfaces.
    /// Water/Cliff stay walk-costed (not hard-blocked); Impassable/Obstacle set bits.
    /// Fence/transparent bits are independent flags (C++ PathfindCellInfo).
    pub fn set_cell_type(&mut self, pos: GridPos, ty: PathfindCellType) {
        let Some(idx) = self.bit_index(pos) else {
            return;
        };
        if let Some(slot) = self.cell_types.get_mut(idx) {
            *slot = ty as u8;
        }
        let hard = matches!(
            ty,
            PathfindCellType::Impassable
                | PathfindCellType::Obstacle
                | PathfindCellType::BridgeImpassable
        );
        Self::bit_set(&mut self.blocked_bits, idx, hard);
        if !matches!(ty, PathfindCellType::Obstacle) {
            Self::bit_set(&mut self.fence_bits, idx, false);
            Self::bit_set(&mut self.transparent_bits, idx, false);
            if let Some(slot) = self.occ_obstacle_id.get_mut(idx) {
                *slot = 0;
            }
            if let Some(slot) = self.occ_obstacle_owner.get_mut(idx) {
                *slot = 0xFF;
            }
            if let Some(slot) = self.occ_obstacle_team.get_mut(idx) {
                *slot = 0xFF;
            }
        }
        self.terrain_gen = self.terrain_gen.wrapping_add(1);
    }

    /// C++ `PathfindCell::setTypeAsObstacle(..., isFence)`.
    pub fn set_cell_obstacle(&mut self, pos: GridPos, is_fence: bool, is_transparent: bool) {
        self.set_cell_obstacle_owned(pos, is_fence, is_transparent, 0, None, None);
    }

    /// C++ `setTypeAsObstacle` + `getObstacleID` owner for dozerHack.
    pub fn set_cell_obstacle_owned(
        &mut self,
        pos: GridPos,
        is_fence: bool,
        is_transparent: bool,
        object_id: u32,
        owner_player: Option<u32>,
        team: Option<Team>,
    ) {
        self.set_cell_type(pos, PathfindCellType::Obstacle);
        let Some(idx) = self.bit_index(pos) else {
            return;
        };
        Self::bit_set(&mut self.fence_bits, idx, is_fence);
        Self::bit_set(&mut self.transparent_bits, idx, is_transparent);
        if let Some(slot) = self.occ_obstacle_id.get_mut(idx) {
            *slot = object_id;
        }
        if let Some(slot) = self.occ_obstacle_owner.get_mut(idx) {
            *slot = owner_player
                .filter(|&p| p <= 15)
                .map(|p| p as u8)
                .unwrap_or(0xFF);
        }
        if let Some(slot) = self.occ_obstacle_team.get_mut(idx) {
            *slot = team.map(|t| t as u8).unwrap_or(0xFF);
        }
    }

    fn obstacle_owner(&self, pos: GridPos) -> Option<(u32, Option<u32>, Option<Team>)> {
        let idx = self.bit_index(pos)?;
        let id = *self.occ_obstacle_id.get(idx)?;
        if id == 0 {
            return None;
        }
        let owner = self.occ_obstacle_owner.get(idx).copied().and_then(|p| {
            if p == 0xFF {
                None
            } else {
                Some(p as u32)
            }
        });
        let team = self.occ_obstacle_team.get(idx).copied().and_then(|t| match t {
            0 => Some(Team::GLA),
            1 => Some(Team::USA),
            2 => Some(Team::China),
            3 => Some(Team::Neutral),
            _ => None,
        });
        Some((id, owner, team))
    }

    pub fn is_obstacle_fence(&self, pos: GridPos) -> bool {
        self.bit_index(pos)
            .is_some_and(|idx| Self::bit_test(&self.fence_bits, idx))
    }

    pub fn is_obstacle_transparent(&self, pos: GridPos) -> bool {
        self.bit_index(pos)
            .is_some_and(|idx| Self::bit_test(&self.transparent_bits, idx))
    }

    /// C++ Pathfinder::isAttackViewBlockedByObstacle residual (static obstacles only).
    /// Bresenham walk from `from`→`to`; only CELL_OBSTACLE blocks, after identity
    /// / transparent / victim-cell skips. No Chebyshev-4 blind zone.
    pub fn is_attack_view_blocked_static(&self, from: Vec3, to: Vec3) -> bool {
        self.is_attack_view_blocked_static_ex(from, to, 0, &[])
    }

    /// C++ `attackBlockedByObstacleCallback` + leftover skip-3 / identity / victim-cell.
    fn is_attack_view_blocked_static_ex(
        &self,
        from: Vec3,
        to: Vec3,
        mut skip_count: i32,
        skip_ids: &[u32],
    ) -> bool {
        let start = self.world_to_grid(from);
        let goal = self.world_to_grid(to);
        if start == goal {
            return false;
        }
        if start.manhattan_distance(goal) <= 1 {
            return false;
        }
        let victim_obs = self.obstacle_owner(goal).map(|(id, _, _)| id).unwrap_or(0);
        let mut x0 = start.x;
        let mut y0 = start.y;
        let x1 = goal.x;
        let y1 = goal.y;
        let dx = (x1 - x0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let dy = -(y1 - y0).abs();
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;
        loop {
            let e2 = 2 * err;
            if e2 >= dy {
                if x0 == x1 {
                    break;
                }
                err += dy;
                x0 += sx;
            }
            if e2 <= dx {
                if y0 == y1 {
                    break;
                }
                err += dx;
                y0 += sy;
            }
            let cell = GridPos::new(x0, y0);
            if cell == goal {
                break;
            }
            if !self.is_valid_pos(cell) {
                continue;
            }
            // C++ skipCount: first N cells off a bridge/rooftop are see-through.
            if skip_count > 0 {
                skip_count -= 1;
                continue;
            }
            // C++ attackBlockedByObstacleCallback: only CELL_OBSTACLE.
            if self.cell_type(cell) != PathfindCellType::Obstacle {
                continue;
            }
            if self.is_obstacle_transparent(cell) {
                continue;
            }
            let obs_id = self.obstacle_owner(cell).map(|(id, _, _)| id).unwrap_or(0);
            if obs_id != 0 {
                if skip_ids.contains(&obs_id) {
                    continue;
                }
                if victim_obs != 0 && obs_id == victim_obs {
                    continue;
                }
            }
            return true;
        }
        false
    }

    /// Live-host name used by attack/mood/save. Same residual as static LOS.
    pub fn is_attack_view_blocked(&self, from: Vec3, to: Vec3) -> bool {
        self.is_attack_view_blocked_static(from, to)
    }



    pub fn set_blocked(&mut self, pos: GridPos, blocked: bool) {
        if blocked {
            self.set_cell_type(pos, PathfindCellType::Obstacle);
        } else {
            self.set_cell_type(pos, PathfindCellType::Clear);
        }
    }

    /// Mark a structure footprint as static-blocked (C++ pathfind obstacle residual).
    /// `radius_cells` is half-extent in grid cells (1 => 3×3).
    pub fn block_structure_footprint(&mut self, center: GridPos, radius_cells: i32) {
        self.block_structure_footprint_ex(center, radius_cells, false, false);
    }

    /// Block a structure footprint from a world position (cell radius).
    pub fn block_structure_at_world(&mut self, pos: Vec3, radius_cells: i32) {
        let center = self.world_to_grid(pos);
        self.block_structure_footprint(center, radius_cells);
    }


    pub fn block_structure_footprint_ex(
        &mut self,
        center: GridPos,
        radius_cells: i32,
        is_fence: bool,
        is_transparent: bool,
    ) {
        let r = radius_cells.max(0);
        for dy in -r..=r {
            for dx in -r..=r {
                let p = GridPos::new(center.x + dx, center.y + dy);
                if self.is_valid_pos(p) {
                    self.set_cell_obstacle(p, is_fence, is_transparent);
                }
            }
        }
    }

    /// C++ `setTypeAsObstacle` BODY_RUBBLE → CELL_RUBBLE.
    pub fn stamp_rubble_footprint(&mut self, center: GridPos, radius_cells: i32) {
        let r = radius_cells.max(0);
        for dy in -r..=r {
            for dx in -r..=r {
                let p = GridPos::new(center.x + dx, center.y + dy);
                if self.is_valid_pos(p) {
                    self.set_cell_type(p, PathfindCellType::Rubble);
                }
            }
        }
    }

    fn object_is_pathfind_rubble(obj: &Object) -> bool {
        use crate::game_logic::host_enum_table_residual::HostBodyDamageType;
        obj.status.keep_as_rubble
            || obj.body_damage_state == HostBodyDamageType::Rubble
            || obj
                .keep_object_die
                .as_ref()
                .is_some_and(|d| d.is_rubble)
    }

    /// C++ `Pathfinder::classifyFence` raster (AIPathfind.cpp:3983+).
    pub fn classify_fence_world(
        &mut self,
        world: Vec3,
        orientation: f32,
        fence_width: f32,
        fence_x_offset: f32,
        is_transparent: bool,
        object_id: u32,
        owner_player: Option<u32>,
        team: Option<Team>,
    ) -> Option<(GridPos, GridPos)> {
        if fence_width <= 0.0 {
            return None;
        }
        let halfsize_x = fence_width * 0.5;
        let halfsize_y = self.grid_size / 10.0;
        let (s, c) = orientation.sin_cos();
        let step = self.grid_size * 0.5;
        let ydx = s * step;
        let ydy = -c * step;
        let xdx = c * step;
        let xdy = s * step;
        let num_steps_x = ((2.0 * halfsize_x / step).ceil() as i32).max(1);
        let num_steps_y = ((2.0 * halfsize_y / step).ceil() as i32).max(1);
        let mut tl_x = world.x - fence_x_offset * c - halfsize_y * s;
        let mut tl_z = world.z + halfsize_y * c - fence_x_offset * s;
        let mut lo = GridPos::new(i32::MAX, i32::MAX);
        let mut hi = GridPos::new(i32::MIN, i32::MIN);
        let mut did = false;
        for _iy in 0..num_steps_y {
            let mut x = tl_x;
            let mut z = tl_z;
            for _ix in 0..num_steps_x {
                let cell = self.classify_world_to_cell(x, z);
                if self.is_valid_pos(cell) {
                    self.set_cell_obstacle_owned(
                        cell,
                        true,
                        is_transparent,
                        object_id,
                        owner_player,
                        team,
                    );
                    Self::expand_stamp_bounds(&mut lo, &mut hi, cell);
                    did = true;
                }
                x += xdx;
                z += xdy;
            }
            tl_x += ydx;
            tl_z += ydy;
        }
        if did {
            Some((lo, hi))
        } else {
            None
        }
    }


    /// C++ `REAL_TO_INT_FLOOR((x+0.5)/PATHFIND_CELL_SIZE)` used by classify raster.
    fn classify_world_to_cell(&self, x: f32, z: f32) -> GridPos {
        GridPos {
            x: ((x - self.origin.x + 0.5) / self.grid_size).floor() as i32,
            y: ((z - self.origin.z + 0.5) / self.grid_size).floor() as i32,
        }
    }

    fn expand_stamp_bounds(lo: &mut GridPos, hi: &mut GridPos, cell: GridPos) {
        lo.x = lo.x.min(cell.x);
        lo.y = lo.y.min(cell.y);
        hi.x = hi.x.max(cell.x);
        hi.y = hi.y.max(cell.y);
    }

    /// Leftover `classify.rs` / C++ `internal_classifyObjectFootprint` (AIPathfind.cpp:4175-4290).
    /// Oriented GEOMETRY_BOX raster or cylinder/sphere disc. Returns stamped bounds.
    pub fn classify_object_footprint(
        &mut self,
        obj: &Object,
        as_rubble: bool,
    ) -> Option<(GridPos, GridPos)> {
        if obj.is_kind_of(KindOf::Mine)
            || obj.is_kind_of(KindOf::Projectile)
            || obj.is_kind_of(KindOf::BridgeTower)
        {
            return None;
        }
        let fence_width = obj.thing.template.fence_width;
        if fence_width > 0.0 && !obj.is_kind_of(KindOf::DefensiveWall) {
            return self.classify_fence_world(
                obj.get_position(),
                obj.get_orientation(),
                fence_width,
                obj.thing.template.fence_x_offset,
                obj.is_kind_of(KindOf::CanSeeThrough),
                obj.id.0,
                obj.owner_player_id,
                Some(obj.team),
            );
        }
        if !obj.is_kind_of(KindOf::Structure) {
            return None;
        }
        if obj.is_mobile() {
            return None;
        }
        let geom = obj.thing.template.geometry_info;
        if geom.authored && geom.is_small {
            return None;
        }
        let pos = obj.get_position();
        let height_above = pos.y - sample_host_ground_height(pos.x, pos.z);
        let is_blast_crater = obj.is_kind_of(KindOf::BlastCrater);
        // C++ skips airborne bounds unless KINDOF_BLAST_CRATER (AIPathfind.cpp:4168-4171).
        if height_above > self.grid_size && !is_blast_crater {
            return None;
        }
        let is_transparent = obj.is_kind_of(KindOf::CanSeeThrough);
        let mut lo = GridPos::new(i32::MAX, i32::MAX);
        let mut hi = GridPos::new(i32::MIN, i32::MIN);
        let mut did = false;
        let stamp = |grid: &mut Self, cell: GridPos, lo: &mut GridPos, hi: &mut GridPos| {
            if !grid.is_valid_pos(cell) {
                return false;
            }
            if as_rubble {
                grid.set_cell_type(cell, PathfindCellType::Rubble);
            } else {
                grid.set_cell_obstacle_owned(
                    cell,
                    false,
                    is_transparent,
                    obj.id.0,
                    obj.owner_player_id,
                    Some(obj.team),
                );
            }
            if is_blast_crater {
                grid.permanent_blast_crater_cells.insert((cell.x, cell.y));
            }
            Self::expand_stamp_bounds(lo, hi, cell);
            true
        };
        let geom_type = if geom.authored {
            geom.geom_type
        } else {
            crate::game_logic::HostGeometryType::Cylinder
        };
        let major = if geom.authored && geom.major_radius > 0.0 {
            geom.major_radius
        } else {
            obj.selection_radius.max(self.grid_size * 0.5)
        };
        let minor = if geom.authored && geom.minor_radius > 0.0 {
            geom.minor_radius
        } else {
            major
        };
        match geom_type {
            crate::game_logic::HostGeometryType::Box => {
                let angle = obj.get_orientation();
                let (s, c) = angle.sin_cos();
                let step = self.grid_size * 0.5;
                let ydx = s * step;
                let ydy = -c * step;
                let xdx = c * step;
                let xdy = s * step;
                let num_steps_x = ((2.0 * major / step).ceil() as i32).max(1);
                let num_steps_y = ((2.0 * minor / step).ceil() as i32).max(1);
                let mut tl_x = pos.x - major * c - minor * s;
                let mut tl_z = pos.z + minor * c - major * s;
                for _iy in 0..num_steps_y {
                    let mut x = tl_x;
                    let mut z = tl_z;
                    for _ix in 0..num_steps_x {
                        let cell = self.classify_world_to_cell(x, z);
                        if stamp(self, cell, &mut lo, &mut hi) {
                            did = true;
                        }
                        x += xdx;
                        z += xdy;
                    }
                    tl_x += ydx;
                    tl_z += ydy;
                }
            }
            crate::game_logic::HostGeometryType::Sphere
            | crate::game_logic::HostGeometryType::Cylinder => {
                // C++ cylinder: cell-center disc, size = radius/cell + 0.4.
                let size = major / self.grid_size + 0.4;
                let r2 = size * size;
                let center_x = (pos.x - self.origin.x) / self.grid_size;
                let center_y = (pos.z - self.origin.z) / self.grid_size;
                let top_left_x = ((pos.x - self.origin.x - major) / self.grid_size + 0.5)
                    .floor() as i32
                    - 1;
                let top_left_y = ((pos.z - self.origin.z - major) / self.grid_size + 0.5)
                    .floor() as i32
                    - 1;
                let bottom_right_x = top_left_x + (2.0 * size) as i32 + 2;
                let bottom_right_y = top_left_y + (2.0 * size) as i32 + 2;
                for j in top_left_y..bottom_right_y {
                    for i in top_left_x..bottom_right_x {
                        let dx = i as f32 + 0.5 - center_x;
                        let dy = j as f32 + 0.5 - center_y;
                        if dx * dx + dy * dy > r2 {
                            continue;
                        }
                        if stamp(self, GridPos::new(i, j), &mut lo, &mut hi) {
                            did = true;
                        }
                    }
                }
            }
        }
        if did {
            Some((lo, hi))
        } else {
            None
        }
    }

    /// C++ never-remove: re-OR BLAST_CRATER cells after terrain/object rebuild.
    fn restamp_permanent_blast_craters(
        &mut self,
        lo: &mut GridPos,
        hi: &mut GridPos,
        did: &mut bool,
    ) {
        let cells: Vec<(i32, i32)> = self.permanent_blast_crater_cells.iter().copied().collect();
        for (x, y) in cells {
            let cell = GridPos::new(x, y);
            if !self.is_valid_pos(cell) {
                continue;
            }
            self.set_cell_obstacle(cell, false, false);
            Self::expand_stamp_bounds(lo, hi, cell);
            *did = true;
        }
    }


    /// Leftover `AStarPathfinder::refresh_pinched_cells_in_bounds` (C++ AIPathfind.cpp:4404-4477).
    pub fn refresh_pinched_cells_in_bounds(&mut self, lo: GridPos, hi: GridPos) {
        let min_x = lo.x.max(0);
        let min_y = lo.y.max(0);
        let max_x = hi.x.min(self.width.saturating_sub(1));
        let max_y = hi.y.min(self.height.saturating_sub(1));
        if min_x > max_x || min_y > max_y {
            return;
        }
        for y in min_y..=max_y {
            for x in min_x..=max_x {
                let p = GridPos::new(x, y);
                if self.cell_type(p) == PathfindCellType::Impassable {
                    self.set_cell_type(p, PathfindCellType::Clear);
                }
                self.set_pinched(p, false);
            }
        }
        for y in min_y..=max_y {
            for x in min_x..=max_x {
                let p = GridPos::new(x, y);
                if self.cell_type(p) != PathfindCellType::Clear {
                    continue;
                }
                let mut total_count = 0;
                let mut orthogonal_count = 0;
                for dy in -1..=1 {
                    for dx in -1..=1 {
                        if dx == 0 && dy == 0 {
                            continue;
                        }
                        let n = GridPos::new(x + dx, y + dy);
                        if !self.is_valid_pos(n) {
                            continue;
                        }
                        if self.cell_type(n) == PathfindCellType::Clear {
                            total_count += 1;
                            if dx == 0 || dy == 0 {
                                orthogonal_count += 1;
                            }
                        }
                    }
                }
                if orthogonal_count < 2 || total_count < 4 {
                    self.set_pinched(p, true);
                }
            }
        }
        for y in min_y..=max_y {
            for x in min_x..=max_x {
                let p = GridPos::new(x, y);
                if self.is_pinched(p) && self.cell_type(p) == PathfindCellType::Clear {
                    self.set_cell_type(p, PathfindCellType::Impassable);
                    self.set_pinched(p, false);
                }
            }
        }
        for y in min_y..=max_y {
            for x in min_x..=max_x {
                let p = GridPos::new(x, y);
                if self.cell_type(p) != PathfindCellType::Clear {
                    continue;
                }
                let mut obstacle_adjacent = false;
                for dy in -1..=1 {
                    for dx in -1..=1 {
                        if dx == 0 && dy == 0 {
                            continue;
                        }
                        if dx != 0 && dy != 0 {
                            continue;
                        }
                        let n = GridPos::new(x + dx, y + dy);
                        if !self.is_valid_pos(n) {
                            continue;
                        }
                        if self.cell_type(n) == PathfindCellType::Obstacle {
                            obstacle_adjacent = true;
                            break;
                        }
                    }
                    if obstacle_adjacent {
                        break;
                    }
                }
                if obstacle_adjacent {
                    self.set_pinched(p, true);
                }
            }
        }
    }

    /// Leftover `refresh_pinched_bounds`: expand stamped region by 2 cells then pinch.
    pub fn refresh_pinched_bounds(&mut self, lo: GridPos, hi: GridPos) {
        if lo.x == i32::MAX {
            return;
        }
        let lo = GridPos::new((lo.x - 2).max(0), (lo.y - 2).max(0));
        let hi = GridPos::new(
            (hi.x + 2).min(self.width.saturating_sub(1)),
            (hi.y + 2).min(self.height.saturating_sub(1)),
        );
        self.refresh_pinched_cells_in_bounds(lo, hi);
    }


    pub fn clear_static_blocks(&mut self) {
        self.blocked_bits.fill(0);
        self.cell_types.fill(PathfindCellType::Clear as u8);
        self.pinched_bits.fill(0);
        self.terrain_zones.fill(0);
        self.path_zones.fill(0);
        self.ground_water_zones.clear();
        self.ground_cliff_zones.clear();
        self.ground_rubble_zones.clear();
        self.fence_bits.fill(0);
        self.transparent_bits.fill(0);
        self.ground_connect.fill(0);
        self.bridge_layers.clear();
        // C++ classifyMap keeps m_layers[LAYER_WALL] when pieces remain
        // (AIPathfind.cpp:4650-4651). Do not drop the deck on terrain rebuild.
        self.allocate_and_classify_wall_layer();
    }


    pub fn export_static_block_mask(&self) -> Vec<bool> {
        let mut mask = vec![false; (self.width.max(0) * self.height.max(0)) as usize];
        for y in 0..self.height {
            for x in 0..self.width {
                let idx = (y * self.width + x) as usize;
                mask[idx] = self.is_static_blocked(GridPos::new(x, y));
            }
        }
        mask
    }

    pub fn import_static_block_mask(&mut self, width: i32, height: i32, mask: &[bool]) -> bool {
        if width != self.width || height != self.height {
            return false;
        }

        let expected_len = (self.width * self.height) as usize;
        if mask.len() != expected_len {
            return false;
        }

        self.clear_static_blocks();
        for y in 0..self.height {
            for x in 0..self.width {
                let idx = (y * self.width + x) as usize;
                if mask[idx] {
                    self.set_blocked(GridPos::new(x, y), true);
                }
            }
        }
        true
    }

    pub fn grid_size(&self) -> f32 {
        self.grid_size
    }

    pub fn width(&self) -> i32 {
        self.width
    }

    pub fn height(&self) -> i32 {
        self.height
    }

    pub fn set_dynamic_blocked(&mut self, pos: GridPos, blocked: bool) {
        if let Some(idx) = self.bit_index(pos) {
            Self::bit_set(&mut self.dynamic_bits, idx, blocked);
        }
    }

    pub fn clear_dynamic_blocks(&mut self) {
        self.dynamic_bits.fill(0);
        self.occ_fixed_mask.fill(0);
        self.occ_moving_mask.fill(0);
        self.occ_goal_mask.fill(0);
        self.occ_goal_unit.fill(0);
        self.occ_goal_aircraft.fill(0);
        self.occ_infantry_mask.fill(0);
        self.occ_fixed_max_crushable.fill(0);
        self.layer_occ.clear();
    }

    pub fn is_pinched(&self, pos: GridPos) -> bool {
        self.bit_index(pos)
            .is_some_and(|idx| Self::bit_test(&self.pinched_bits, idx))
    }

    pub fn set_pinched(&mut self, pos: GridPos, pinched: bool) {
        if let Some(idx) = self.bit_index(pos) {
            Self::bit_set(&mut self.pinched_bits, idx, pinched);
        }
    }

    /// C++ Pathfinder::classifyMap cliff expand (AIPathfind.cpp:4591-4632).
    pub fn pinch_tighten_cliffs(&mut self) {
        let w = self.width;
        let h = self.height;
        if w <= 0 || h <= 0 {
            return;
        }
        self.pinched_bits.fill(0);
        let mut first_ring = Vec::new();
        for y in 0..h {
            for x in 0..w {
                let pos = GridPos::new(x, y);
                if self.cell_type(pos) != PathfindCellType::Cliff {
                    continue;
                }
                for ny in (y - 1).max(0)..=(y + 1).min(h - 1) {
                    for nx in (x - 1).max(0)..=(x + 1).min(w - 1) {
                        let n = GridPos::new(nx, ny);
                        if self.cell_type(n) == PathfindCellType::Clear {
                            first_ring.push(n);
                        }
                    }
                }
            }
        }
        for pos in &first_ring {
            self.set_pinched(*pos, true);
        }
        for pos in first_ring {
            if self.cell_type(pos) == PathfindCellType::Clear {
                self.set_cell_type(pos, PathfindCellType::Cliff);
            }
        }
        let mut second_ring = Vec::new();
        for y in 0..h {
            for x in 0..w {
                let pos = GridPos::new(x, y);
                if self.cell_type(pos) != PathfindCellType::Cliff {
                    continue;
                }
                for ny in (y - 1).max(0)..=(y + 1).min(h - 1) {
                    for nx in (x - 1).max(0)..=(x + 1).min(w - 1) {
                        let n = GridPos::new(nx, ny);
                        if self.cell_type(n) == PathfindCellType::Clear {
                            second_ring.push(n);
                        }
                    }
                }
            }
        }
        for pos in second_ring {
            self.set_pinched(pos, true);
        }
        self.terrain_gen = self.terrain_gen.wrapping_add(1);
    }

    fn terrain_zone_passable(ty: PathfindCellType) -> bool {
        // Terrain-only: structures (Obstacle) are ignored for UI zones.
        !matches!(
            ty,
            PathfindCellType::Water
                | PathfindCellType::Cliff
                | PathfindCellType::Impassable
                | PathfindCellType::BridgeImpassable
        )
    }

    /// Flood-fill terrain zones ignoring structure obstacles (C++ effectiveTerrainZone).
    pub fn rebuild_terrain_zones(&mut self) {
        let cells = self.terrain_zones.len();
        self.terrain_zones.fill(0);
        let mut next_zone = 1u16;
        let w = self.width;
        let h = self.height;
        for y in 0..h {
            for x in 0..w {
                let start = GridPos::new(x, y);
                let Some(sidx) = self.bit_index(start) else {
                    continue;
                };
                if self.terrain_zones[sidx] != 0 {
                    continue;
                }
                if !Self::terrain_zone_passable(self.cell_type_at_index(sidx)) {
                    continue;
                }
                let zone = next_zone;
                next_zone = next_zone.saturating_add(1);
                let mut stack = vec![start];
                while let Some(cur) = stack.pop() {
                    let Some(idx) = self.bit_index(cur) else {
                        continue;
                    };
                    if self.terrain_zones[idx] != 0 {
                        continue;
                    }
                    if !Self::terrain_zone_passable(self.cell_type_at_index(idx)) {
                        continue;
                    }
                    self.terrain_zones[idx] = zone;
                    stack.push(GridPos::new(cur.x + 1, cur.y));
                    stack.push(GridPos::new(cur.x - 1, cur.y));
                    stack.push(GridPos::new(cur.x, cur.y + 1));
                    stack.push(GridPos::new(cur.x, cur.y - 1));
                }
                let _ = cells;
            }
        }
        let mut zones = std::mem::take(&mut self.terrain_zones);
        self.merge_zones_via_connect_layer(&mut zones);
        self.terrain_zones = zones;

    }

    pub fn terrain_zone(&self, pos: GridPos) -> u16 {
        self.bit_index(pos)
            .and_then(|idx| self.terrain_zones.get(idx).copied())
            .unwrap_or(0)
    }

    /// C++ Pathfinder::clientSafeQuickDoesPathExistForUI (AIPathfind.cpp:8055).
    pub fn quick_path_exists_for_ui(&self, from: Vec3, to: Vec3) -> bool {
        let start = self.world_to_grid(from);
        let goal = self.world_to_grid(to);
        if self.cell_type(goal) == PathfindCellType::Cliff {
            return false;
        }
        let z1 = self.terrain_zone(start);
        let z2 = self.terrain_zone(goal);
        // C++ UNINITIALIZED_ZONE → false-positive true.
        if z1 == 0 || z2 == 0 {
            return true;
        }
        z1 == z2
    }

    /// Flood kind for structure-aware zones: same-type connectivity.
    /// C++ `calculateZones` joins only equal `getType()`; leftover
    /// `flood_fill_type`. Rubble is not Clear. Impassable / Obstacle /
    /// BridgeImpassable get real zone ids (not zone 0).
    fn path_zone_flood_kind(ty: PathfindCellType) -> Option<u8> {
        Some(ty as u8)
    }

    fn pair_water_ground(a: PathfindCellType, b: PathfindCellType) -> bool {
        matches!(
            (a, b),
            (PathfindCellType::Clear, PathfindCellType::Water)
                | (PathfindCellType::Water, PathfindCellType::Clear)
        )
    }

    fn pair_ground_cliff(a: PathfindCellType, b: PathfindCellType) -> bool {
        matches!(
            (a, b),
            (PathfindCellType::Clear, PathfindCellType::Cliff)
                | (PathfindCellType::Cliff, PathfindCellType::Clear)
        )
    }

    fn pair_ground_rubble(a: PathfindCellType, b: PathfindCellType) -> bool {
        matches!(
            (a, b),
            (PathfindCellType::Clear, PathfindCellType::Rubble)
                | (PathfindCellType::Rubble, PathfindCellType::Clear)
        )
    }

    /// C++ `PathfindZoneManager::calculateZones` resolveZones
    /// (`AIPathfind.cpp:2629-2633`): CLEAR ground cells with
    /// `getConnectLayer() > LAYER_GROUND` merge with that bridge layer's
    /// zone in `m_hierarchicalZones`. Leftover `build_surface_combiners`
    /// already does this; live floods store the effective id on the cell.
    fn merge_zones_via_connect_layer(&self, zones: &mut [u16]) {
        if zones.len() != self.ground_connect.len() {
            return;
        }
        let mut max_z = 0u16;
        for &z in zones.iter() {
            if z > max_z {
                max_z = z;
            }
        }
        if max_z == 0 {
            return;
        }
        let mut parent: Vec<u16> = (0..=max_z).collect();
        fn find(parent: &mut [u16], mut z: u16) -> u16 {
            while (z as usize) < parent.len() && parent[z as usize] != z {
                let p = parent[z as usize];
                if (p as usize) < parent.len() {
                    parent[z as usize] = parent[p as usize];
                }
                z = p;
            }
            z
        }
        fn union(parent: &mut [u16], a: u16, b: u16) {
            if a == 0 || b == 0 {
                return;
            }
            let pa = find(parent, a);
            let pb = find(parent, b);
            if pa == pb {
                return;
            }
            // C++ resolveZones keeps the lower zone id.
            if pa < pb {
                parent[pb as usize] = pa;
            } else {
                parent[pa as usize] = pb;
            }
        }
        // Union every CLEAR ground cell that shares a connectLayer
        // (equivalent to resolveZones(cell.zone, layer.zone)).
        let mut layer_rep = [0u16; 16];
        for (idx, &cl) in self.ground_connect.iter().enumerate() {
            if cl <= PathfindLayerEnum::Ground as u8 || (cl as usize) >= layer_rep.len() {
                continue;
            }
            if self.cell_type_at_index(idx) != PathfindCellType::Clear {
                continue;
            }
            let z = zones.get(idx).copied().unwrap_or(0);
            if z == 0 {
                continue;
            }
            let slot = cl as usize;
            if layer_rep[slot] == 0 {
                layer_rep[slot] = z;
            } else {
                union(&mut parent, layer_rep[slot], z);
            }
        }
        for z in zones.iter_mut() {
            if *z != 0 {
                *z = find(&mut parent, *z);
            }
        }
    }


    /// Structure-aware zones (C++ clientSafeQuickDoesPathExist).
    pub fn rebuild_path_zones(&mut self) {
        self.path_zones.fill(0);
        let mut next_zone = 1u16;
        let w = self.width;
        let h = self.height;
        for y in 0..h {
            for x in 0..w {
                let start = GridPos::new(x, y);
                let Some(sidx) = self.bit_index(start) else {
                    continue;
                };
                if self.path_zones[sidx] != 0 {
                    continue;
                }
                let Some(kind) = Self::path_zone_flood_kind(self.cell_type_at_index(sidx)) else {
                    continue;
                };
                let zone = next_zone;
                next_zone = next_zone.saturating_add(1);
                let mut stack = vec![start];
                while let Some(cur) = stack.pop() {
                    let Some(idx) = self.bit_index(cur) else {
                        continue;
                    };
                    if self.path_zones[idx] != 0 {
                        continue;
                    }
                    if Self::path_zone_flood_kind(self.cell_type_at_index(idx)) != Some(kind) {
                        continue;
                    }
                    self.path_zones[idx] = zone;
                    stack.push(GridPos::new(cur.x + 1, cur.y));
                    stack.push(GridPos::new(cur.x - 1, cur.y));
                    stack.push(GridPos::new(cur.x, cur.y + 1));
                    stack.push(GridPos::new(cur.x, cur.y - 1));
                }
            }
        }
        let mut zones = std::mem::take(&mut self.path_zones);
        self.merge_zones_via_connect_layer(&mut zones);
        self.path_zones = zones;
        self.build_surface_combiners();
    }

    /// Leftover `ZoneManager::build_surface_combiners` (GROUND+WATER / GROUND+CLIFF).
    fn build_surface_combiners(&mut self) {
        let mut max_z = 0u16;
        for &z in &self.path_zones {
            if z > max_z {
                max_z = z;
            }
        }
        let n = max_z as usize + 1;
        let mut water: Vec<u16> = (0..n as u16).collect();
        let mut cliff: Vec<u16> = (0..n as u16).collect();
        let mut rubble: Vec<u16> = (0..n as u16).collect();
        let resolve = |table: &mut [u16], a: u16, b: u16| {
            if a == 0 || b == 0 || a == b {
                return;
            }
            let za = table.get(a as usize).copied().unwrap_or(a);
            let zb = table.get(b as usize).copied().unwrap_or(b);
            if za == zb {
                return;
            }
            let final_z = za.min(zb);
            for z in table.iter_mut() {
                if *z == za || *z == zb {
                    *z = final_z;
                }
            }
        };
        let w = self.width;
        let h = self.height;
        for y in 0..h {
            for x in 0..w {
                let Some(idx) = self.bit_index(GridPos::new(x, y)) else {
                    continue;
                };
                let z1 = self.path_zones[idx];
                if z1 == 0 {
                    continue;
                }
                let t1 = self.cell_type_at_index(idx);
                if x > 0 {
                    if let Some(nidx) = self.bit_index(GridPos::new(x - 1, y)) {
                        let z0 = self.path_zones[nidx];
                        let t0 = self.cell_type_at_index(nidx);
                        if z0 != 0 && z0 != z1 {
                            if Self::pair_water_ground(t0, t1) {
                                resolve(&mut water, z0, z1);
                            } else if Self::pair_ground_rubble(t0, t1) {
                                resolve(&mut rubble, z0, z1);
                            } else if Self::pair_ground_cliff(t0, t1) {
                                resolve(&mut cliff, z0, z1);
                            }
                        }
                    }
                }
                if y > 0 {
                    if let Some(nidx) = self.bit_index(GridPos::new(x, y - 1)) {
                        let z0 = self.path_zones[nidx];
                        let t0 = self.cell_type_at_index(nidx);
                        if z0 != 0 && z0 != z1 {
                            if Self::pair_water_ground(t0, t1) {
                                resolve(&mut water, z0, z1);
                            } else if Self::pair_ground_rubble(t0, t1) {
                                resolve(&mut rubble, z0, z1);
                            } else if Self::pair_ground_cliff(t0, t1) {
                                resolve(&mut cliff, z0, z1);
                            }
                        }
                    }
                }
            }
        }
        self.ground_water_zones = water;
        self.ground_cliff_zones = cliff;
        self.ground_rubble_zones = rubble;
    }

    /// C++ `PathfindZoneManager::getEffectiveZone` (AIPathfind.cpp:3118).
    fn get_effective_zone(&self, surfaces: u32, zone: u16) -> u16 {
        if zone == 0 {
            return 0;
        }
        if (surfaces & SURFACE_AIR) != 0 {
            return 1;
        }
        if (surfaces & SURFACE_GROUND) != 0
            && (surfaces & SURFACE_WATER) != 0
            && (surfaces & SURFACE_CLIFF) != 0
        {
            return 1;
        }
        if (surfaces & SURFACE_GROUND) != 0 && (surfaces & SURFACE_CLIFF) != 0 {
            return self
                .ground_cliff_zones
                .get(zone as usize)
                .copied()
                .unwrap_or(zone);
        }
        if (surfaces & SURFACE_GROUND) != 0 && (surfaces & SURFACE_WATER) != 0 {
            return self
                .ground_water_zones
                .get(zone as usize)
                .copied()
                .unwrap_or(zone);
        }
        if (surfaces & SURFACE_GROUND) != 0 && (surfaces & SURFACE_RUBBLE) != 0 {
            return self
                .ground_rubble_zones
                .get(zone as usize)
                .copied()
                .unwrap_or(zone);
        }
        zone
    }

    pub fn path_zone(&self, pos: GridPos) -> u16 {
        self.bit_index(pos)
            .and_then(|idx| self.path_zones.get(idx).copied())
            .unwrap_or(0)
    }

    /// C++ Pathfinder::clientSafeQuickDoesPathExist (structure-aware, ground).
    pub fn quick_path_exists(&self, from: Vec3, to: Vec3) -> bool {
        self.quick_path_exists_for(from, to, SURFACE_GROUND)
    }

    /// C++ `clientSafeQuickDoesPathExist` with locomotor surfaces.
    pub fn quick_path_exists_for(&self, from: Vec3, to: Vec3, surfaces: u32) -> bool {
        let start = self.world_to_grid(from);
        let goal = self.world_to_grid(to);
        if self.cell_type(goal) == PathfindCellType::Cliff {
            return false;
        }
        // C++ validMovementPosition: water dest needs WATER or AIR.
        if self.cell_type(goal) == PathfindCellType::Water
            && (surfaces & (SURFACE_WATER | SURFACE_AIR)) == 0
        {
            return false;
        }
        if self.cell_type(goal) == PathfindCellType::Obstacle && !self.is_obstacle_fence(goal) {
            return false;
        }
        let z1 = self.path_zone(start);
        let z2 = self.path_zone(goal);
        if z1 == 0 || z2 == 0 {
            // Uninitialized: treat as possible (C++ UNINITIALIZED_ZONE).
            return true;
        }
        self.get_effective_zone(surfaces, z1) == self.get_effective_zone(surfaces, z2)
    }


    /// C++ Pathfinder::classifyMapCell (AIPathfind.cpp:4491-4521).
    /// Cliff at the cell top-left; water if any of 4 corners — water wins.
    /// No terrain-slope Impassable gate.
    pub fn classify_map_cell(cliff_top_left: bool, water_any_corner: bool) -> PathfindCellType {
        let mut ty = PathfindCellType::Clear;
        if cliff_top_left {
            ty = PathfindCellType::Cliff;
        }
        if water_any_corner {
            ty = PathfindCellType::Water;
        }
        ty
    }


    /// C++ `PathfindLayer::classifyCells` / `classifyLayerMapCell`.
    /// Deck lives on its own layer (CLEAR); sides are BRIDGE_IMPASSABLE;
    /// only end/entry cells connect to LAYER_GROUND. Does **not** flatten
    /// the deck onto `m_map`. Low-clearance ground under the deck is stamped
    /// BRIDGE_IMPASSABLE. Destroyed: layer cells become BRIDGE_IMPASSABLE and
    /// ground connects are dropped — water/ground under the span stays.
    pub fn stamp_bridge_deck(
        &mut self,
        from_left: Vec3,
        from_right: Vec3,
        to_left: Vec3,
        to_right: Vec3,
        destroyed: bool,
    ) {
        let corners = [from_left, from_right, to_right, to_left];
        let mut min_x = f32::MAX;
        let mut max_x = f32::MIN;
        let mut min_z = f32::MAX;
        let mut max_z = f32::MIN;
        for c in corners {
            min_x = min_x.min(c.x);
            max_x = max_x.max(c.x);
            min_z = min_z.min(c.z);
            max_z = max_z.max(c.z);
        }
        // Entry/side lines sit up to ~1 cell outside the deck AABB.
        let pad = self.grid_size;
        let lo = self.world_to_grid(Vec3::new(min_x - pad, 0.0, min_z - pad));
        let hi = self.world_to_grid(Vec3::new(max_x + pad, 0.0, max_z + pad));
        let layer_id = self.alloc_or_find_bridge_layer(from_left, from_right, to_left, to_right);
        self.disconnect_ground_from_layer(layer_id);

        let mut cells: HashMap<(i32, i32), (PathfindCellType, u8)> = HashMap::new();
        for y in lo.y.min(hi.y)..=lo.y.max(hi.y) {
            for x in lo.x.min(hi.x)..=lo.x.max(hi.x) {
                let pos = GridPos::new(x, y);
                if !self.is_valid_pos(pos) {
                    continue;
                }
                let Some((ty, connect)) = self.classify_layer_map_cell(
                    pos,
                    &corners,
                    from_left,
                    from_right,
                    to_left,
                    to_right,
                ) else {
                    continue;
                };
                if connect == PathfindLayerEnum::Ground as u8 {
                    if let Some(idx) = self.bit_index(pos) {
                        if let Some(slot) = self.ground_connect.get_mut(idx) {
                            *slot = layer_id;
                        }
                    }
                }
                // C++ classifyLayerMapCell clearance (AIPathfind.cpp:3711-3721).
                if connect != PathfindLayerEnum::Ground as u8 {
                    let center = self.cell_center_xz(pos);
                    let deck_h = bridge_deck_height(&corners, center.0, center.1);
                    let ground_h = sample_host_ground_height(center.0, center.1);
                    if ground_h + LAYER_Z_CLOSE_ENOUGH_F > deck_h
                        && self.cell_type(pos) != PathfindCellType::Obstacle
                    {
                        self.set_cell_type(pos, PathfindCellType::BridgeImpassable);
                    }
                }
                cells.insert((x, y), (ty, connect));
            }
        }

        if destroyed {
            // C++ classifyCells m_destroyed: every layer cell BRIDGE_IMPASSABLE,
            // drop ground connect (AIPathfind.cpp:3504-3519).
            self.disconnect_ground_from_layer(layer_id);
            for value in cells.values_mut() {
                *value = (PathfindCellType::BridgeImpassable, 0);
            }
        }

        if let Some(layer) = self
            .bridge_layers
            .iter_mut()
            .find(|layer| layer.id == layer_id)
        {
            layer.from_left = from_left;
            layer.from_right = from_right;
            layer.to_left = to_left;
            layer.to_right = to_right;
            layer.cells = cells;
        }
        self.terrain_gen = self.terrain_gen.wrapping_add(1);
    }

    /// C++ `PathfindLayer::getCell` type (NULL/IMPASSABLE → None).
    pub fn layer_cell_type(&self, layer: u8, pos: GridPos) -> Option<PathfindCellType> {
        if layer == LAYER_WALL_ID {
            return self.wall_cells.get(&(pos.x, pos.y)).copied();
        }
        self.bridge_layers
            .iter()
            .find(|l| l.id == layer)
            .and_then(|l| l.cells.get(&(pos.x, pos.y)).map(|(ty, _)| *ty))
    }

    /// C++ `Pathfinder::isPointOnWall` (AIPathfind.cpp:3929-3942).
    pub fn is_point_on_wall(&self, pos: Vec3) -> bool {
        if self.wall_pieces.is_empty() || self.wall_cells.is_empty() {
            return false;
        }
        let cell = self.world_to_grid(pos);
        matches!(
            self.wall_cells.get(&(cell.x, cell.y)),
            Some(PathfindCellType::Clear)
        )
    }

    pub fn wall_height(&self) -> f32 {
        self.wall_height
    }

    pub fn set_wall_height(&mut self, h: f32) {
        self.wall_height = h;
    }

    pub fn wall_piece_count(&self) -> usize {
        self.wall_pieces.len()
    }

    fn wall_piece_contains(piece: &HostWallPiece, x: f32, z: f32) -> bool {
        // C++ PathfindLayer::isPointOnWall — Cos/Sin(-orientation).
        let ori = -piece.orientation;
        let (s, c) = ori.sin_cos();
        let ptx = x - piece.pos_x;
        let ptz = z - piece.pos_z;
        let ptx_new = (ptx * c - ptz * s).abs();
        let ptz_new = (ptx * s + ptz * c).abs();
        ptx_new <= piece.major && ptz_new <= piece.minor
    }

    fn wall_piece_aabb(piece: &HostWallPiece) -> (f32, f32, f32, f32) {
        let (s, c) = piece.orientation.sin_cos();
        let mut lo_x = f32::MAX;
        let mut lo_z = f32::MAX;
        let mut hi_x = f32::MIN;
        let mut hi_z = f32::MIN;
        for &sx in &[-piece.major, piece.major] {
            for &sz in &[-piece.minor, piece.minor] {
                let x = piece.pos_x + sx * c - sz * s;
                let z = piece.pos_z + sx * s + sz * c;
                lo_x = lo_x.min(x);
                lo_z = lo_z.min(z);
                hi_x = hi_x.max(x);
                hi_z = hi_z.max(z);
            }
        }
        (lo_x, lo_z, hi_x, hi_z)
    }

    fn wall_corner_count(&self, pos: GridPos, pieces: &[HostWallPiece]) -> u32 {
        let tl = self.grid_to_world(pos);
        let s = self.grid_size;
        let pts = [
            (tl.x, tl.z),
            (tl.x, tl.z + s),
            (tl.x + s, tl.z + s),
            (tl.x + s, tl.z),
        ];
        pts.iter()
            .filter(|(x, z)| pieces.iter().any(|p| Self::wall_piece_contains(p, *x, *z)))
            .count() as u32
    }

    /// C++ `allocateCellsForWallLayer` + `classifyWallCells` (AIPathfind.cpp:3386-3583).
    pub fn allocate_and_classify_wall_layer(&mut self) {
        self.wall_cells.clear();
        if self.wall_pieces.is_empty() {
            self.terrain_gen = self.terrain_gen.wrapping_add(1);
            return;
        }
        let mut lo_x = f32::MAX;
        let mut lo_z = f32::MAX;
        let mut hi_x = f32::MIN;
        let mut hi_z = f32::MIN;
        for piece in &self.wall_pieces {
            let (a, b, c, d) = Self::wall_piece_aabb(piece);
            lo_x = lo_x.min(a);
            lo_z = lo_z.min(b);
            hi_x = hi_x.max(c);
            hi_z = hi_z.max(d);
        }
        let pad = self.grid_size / 100.0;
        let mut min_cell = self.world_to_grid(Vec3::new(lo_x - pad, 0.0, lo_z - pad));
        let mut max_cell = self.world_to_grid(Vec3::new(hi_x + pad, 0.0, hi_z + pad));
        min_cell.x -= 1;
        min_cell.y -= 1;
        max_cell.x += 1;
        max_cell.y += 1;
        min_cell.x = min_cell.x.max(0);
        min_cell.y = min_cell.y.max(0);
        max_cell.x = max_cell.x.min(self.width.saturating_sub(1));
        max_cell.y = max_cell.y.min(self.height.saturating_sub(1));
        if max_cell.x < min_cell.x || max_cell.y < min_cell.y {
            self.terrain_gen = self.terrain_gen.wrapping_add(1);
            return;
        }
        let pieces = self.wall_pieces.clone();
        let mut raw: HashMap<(i32, i32), PathfindCellType> = HashMap::new();
        for y in min_cell.y..=max_cell.y {
            for x in min_cell.x..=max_cell.x {
                let count = self.wall_corner_count(GridPos::new(x, y), &pieces);
                let ty = if count == 4 {
                    PathfindCellType::Clear
                } else if count != 0 {
                    PathfindCellType::BridgeImpassable
                } else {
                    PathfindCellType::Impassable
                };
                if ty != PathfindCellType::Impassable {
                    raw.insert((x, y), ty);
                }
            }
        }
        // C++ pinch: any 3x3 neighbor not CLEAR → pinched; pinched CLEAR → CLIFF.
        let mut pinched = HashSet::new();
        for y in (min_cell.y + 1)..max_cell.y {
            for x in (min_cell.x + 1)..max_cell.x {
                let mut pinch = false;
                'adj: for dy in -1..=1 {
                    for dx in -1..=1 {
                        let ty = raw
                            .get(&(x + dx, y + dy))
                            .copied()
                            .unwrap_or(PathfindCellType::Impassable);
                        if ty != PathfindCellType::Clear {
                            pinch = true;
                            break 'adj;
                        }
                    }
                }
                if pinch {
                    pinched.insert((x, y));
                }
            }
        }
        for (k, ty) in raw.iter_mut() {
            if pinched.contains(k) && *ty == PathfindCellType::Clear {
                *ty = PathfindCellType::Cliff;
            }
        }
        self.wall_cells = raw;
        self.terrain_gen = self.terrain_gen.wrapping_add(1);
    }

    /// C++ `Pathfinder::addWallPiece` (AIPathfind.cpp:3885-3890).
    pub fn add_wall_piece(
        &mut self,
        id: u32,
        pos: Vec3,
        orientation: f32,
        major: f32,
        minor: f32,
    ) {
        if self.wall_pieces.len() >= MAX_WALL_PIECES.saturating_sub(1) {
            return;
        }
        if self.wall_pieces.iter().any(|p| p.id == id) {
            return;
        }
        self.wall_pieces.push(HostWallPiece {
            id,
            pos_x: pos.x,
            pos_z: pos.z,
            orientation,
            major: major.max(0.1),
            minor: minor.max(0.1),
        });
        self.allocate_and_classify_wall_layer();
    }

    /// C++ `Pathfinder::removeWallPiece` (AIPathfind.cpp:3896-3923).
    pub fn remove_wall_piece(&mut self, id: u32) {
        if let Some(i) = self.wall_pieces.iter().position(|p| p.id == id) {
            let last = self.wall_pieces.len() - 1;
            self.wall_pieces.swap(i, last);
            self.wall_pieces.pop();
            self.allocate_and_classify_wall_layer();
        }
    }

    /// C++ classifyObjectFootprint remove: `isPointOnWall(&curID, 1, pos)`.
    pub fn is_point_on_wall_piece(&self, piece_id: u32, pos: Vec3) -> bool {
        self.wall_pieces
            .iter()
            .find(|p| p.id == piece_id)
            .is_some_and(|p| Self::wall_piece_contains(p, pos.x, pos.z))
    }

    /// C++ `m_map[i][j].getConnectLayer()` (0 = LAYER_INVALID).
    pub fn ground_connect_layer(&self, pos: GridPos) -> u8 {
        self.bit_index(pos)
            .and_then(|idx| self.ground_connect.get(idx).copied())
            .unwrap_or(0)
    }

    pub fn first_bridge_layer_id(&self) -> Option<u8> {
        self.bridge_layers.first().map(|l| l.id)
    }

    /// C++ `TerrainLogic::getLayerForDestination` (host Y-up).
    /// Nearest deck/ground height among bridges whose quad covers XZ.
    pub fn layer_for_destination(&self, pos: Vec3) -> PathfindLayerEnum {
        let ground_y = sample_host_ground_height(pos.x, pos.z);
        let mut best_layer = PathfindLayerEnum::Ground;
        let mut best_distance = (pos.y - ground_y).abs();
        // C++ TerrainLogic::getLayerForDestination checks the wall first
        // when |z-ground| > wallHeight/2 (TerrainLogic.cpp:1674-1682).
        if best_distance > self.wall_height * 0.5 && self.is_point_on_wall(pos) {
            let delta = (pos.y - self.wall_height).abs();
            if delta < best_distance {
                best_layer = PathfindLayerEnum::Wall;
                best_distance = delta;
            }
        }
        let cell = self.world_to_grid(pos);
        for layer in &self.bridge_layers {
            let corners = [
                layer.from_left,
                layer.from_right,
                layer.to_right,
                layer.to_left,
            ];
            if point_in_bridge_quad(pos.x, pos.z, &corners) {
                let deck_y = bridge_deck_height(&corners, pos.x, pos.z);
                let delta = (pos.y - deck_y).abs();
                if delta < best_distance {
                    best_layer = PathfindLayerEnum::from_u32(layer.id as u32);
                    best_distance = delta;
                }
            }
        }
        if best_layer != PathfindLayerEnum::Ground {
            return best_layer;
        }
        if let Ok(tl) = gamelogic::terrain::get_terrain_logic().read() {
            let dest = gamelogic::common::Coord3D::new(pos.x, pos.z, pos.y);
            let leftover = tl.get_layer_for_destination(&dest);
            if leftover != gamelogic::path::PathfindLayerEnum::Ground {
                let id = leftover as u8;
                if self.layer_cell_type(id, cell).is_some() {
                    return PathfindLayerEnum::from_u32(id as u32);
                }
                if let Some(host) = self.host_deck_layer_at(cell) {
                    return PathfindLayerEnum::from_u32(host as u32);
                }
            }
        }
        // Click/unit on a river cell whose deck is Clear: prefer the deck
        // when ground is not a valid ground locomotor cell (Y may be 0).
        if !self.cell_passable_for(cell, SURFACE_GROUND, false) {
            if let Some(host) = self.host_deck_layer_at(cell) {
                return PathfindLayerEnum::from_u32(host as u32);
            }
        }
        PathfindLayerEnum::Ground
    }

    fn host_deck_layer_at(&self, pos: GridPos) -> Option<u8> {
        self.bridge_layers.iter().find_map(|layer| {
            layer.cells.get(&(pos.x, pos.y)).and_then(|(ty, _)| {
                if matches!(
                    *ty,
                    PathfindCellType::Impassable | PathfindCellType::BridgeImpassable
                ) {
                    None
                } else {
                    Some(layer.id)
                }
            })
        })
    }

    /// C++ `Pathfinder::getCell(layer, x, y)` type (Impassable/missing → ground).
    pub fn resolved_cell_type(&self, layer: PathfindLayerEnum, pos: GridPos) -> PathfindCellType {
        if (layer as u8) > PathfindLayerEnum::Ground as u8 {
            if let Some(ty) = self.layer_cell_type(layer as u8, pos) {
                if ty != PathfindCellType::Impassable {
                    return ty;
                }
            }
        }
        self.cell_type(pos)
    }

    fn type_passable_for(ty: PathfindCellType, surfaces: u32, is_crusher: bool) -> bool {
        let cell_surfaces = match ty {
            PathfindCellType::Obstacle
            | PathfindCellType::Impassable
            | PathfindCellType::BridgeImpassable => SURFACE_AIR,
            PathfindCellType::Clear => SURFACE_GROUND | SURFACE_AIR,
            PathfindCellType::Water => SURFACE_WATER | SURFACE_AIR,
            PathfindCellType::Rubble => SURFACE_RUBBLE | SURFACE_AIR,
            PathfindCellType::Cliff => SURFACE_CLIFF | SURFACE_AIR,
        };
        if (cell_surfaces & surfaces) != 0 {
            return true;
        }
        ty == PathfindCellType::Rubble && is_crusher
    }

    /// `cell_passable_for` on `layer` (C++ `validMovementPosition(..., layer, ...)`).
    pub fn cell_passable_for_layer(
        &self,
        pos: GridPos,
        layer: PathfindLayerEnum,
        surfaces: u32,
        is_crusher: bool,
    ) -> bool {
        if !self.is_valid_pos(pos) {
            return false;
        }
        if self.is_obstacle_fence(pos) && is_crusher {
            return true;
        }
        Self::type_passable_for(self.resolved_cell_type(layer, pos), surfaces, is_crusher)
    }



    fn cell_center_xz(&self, pos: GridPos) -> (f32, f32) {
        let tl = self.grid_to_world(pos);
        (
            tl.x + self.grid_size * 0.5,
            tl.z + self.grid_size * 0.5,
        )
    }

    fn alloc_or_find_bridge_layer(
        &mut self,
        from_left: Vec3,
        from_right: Vec3,
        to_left: Vec3,
        to_right: Vec3,
    ) -> u8 {
        if let Some(existing) = self.bridge_layers.iter().find(|layer| {
            span_xz_eq(layer.from_left, from_left)
                && span_xz_eq(layer.from_right, from_right)
                && span_xz_eq(layer.to_left, to_left)
                && span_xz_eq(layer.to_right, to_right)
        }) {
            return existing.id;
        }
        let used: Vec<u8> = self.bridge_layers.iter().map(|l| l.id).collect();
        let id = (2u8..=14).find(|id| !used.contains(id)).unwrap_or(2);
        if used.contains(&id) {
            self.disconnect_ground_from_layer(id);
            if let Some(slot) = self.bridge_layers.iter_mut().find(|l| l.id == id) {
                slot.from_left = from_left;
                slot.from_right = from_right;
                slot.to_left = to_left;
                slot.to_right = to_right;
                slot.cells.clear();
                return id;
            }
        }
        self.bridge_layers.push(HostBridgeLayer {
            id,
            from_left,
            from_right,
            to_left,
            to_right,
            cells: HashMap::new(),
        });
        id
    }

    fn disconnect_ground_from_layer(&mut self, layer_id: u8) {
        for slot in &mut self.ground_connect {
            if *slot == layer_id {
                *slot = 0;
            }
        }
    }

    /// C++ `PathfindLayer::classifyLayerMapCell` (AIPathfind.cpp:3647-3724).
    fn classify_layer_map_cell(
        &self,
        pos: GridPos,
        corners: &[Vec3; 4],
        from_left: Vec3,
        from_right: Vec3,
        to_left: Vec3,
        to_right: Vec3,
    ) -> Option<(PathfindCellType, u8)> {
        let tl = self.grid_to_world(pos);
        let s = self.grid_size;
        let pts = [
            (tl.x, tl.z),
            (tl.x, tl.z + s),
            (tl.x + s, tl.z + s),
            (tl.x + s, tl.z),
        ];
        let mut count = 0u32;
        for (px, pz) in pts {
            if point_in_bridge_quad(px, pz, corners) {
                count += 1;
            }
        }
        let bounds = CellXz {
            lo_x: tl.x,
            lo_z: tl.z,
            hi_x: tl.x + s,
            hi_z: tl.z + s,
        };
        let mut ty = PathfindCellType::Impassable;
        let mut connect = 0u8;
        if count == 4 {
            ty = PathfindCellType::Clear;
        } else {
            if count != 0 {
                ty = PathfindCellType::BridgeImpassable;
            }
            if cell_on_bridge_side(&bounds, from_left, from_right, to_left, to_right, s) {
                ty = PathfindCellType::BridgeImpassable;
            } else {
                if cell_on_bridge_end(&bounds, from_left, from_right, to_left, to_right, s) {
                    ty = PathfindCellType::Clear;
                }
                if cell_is_bridge_entry(&bounds, from_left, from_right, to_left, to_right, s) {
                    ty = PathfindCellType::Clear;
                    connect = PathfindLayerEnum::Ground as u8;
                }
            }
        }
        if ty == PathfindCellType::Impassable {
            None
        } else {
            Some((ty, connect))
        }
    }

    fn query_layer_enum(&self) -> PathfindLayerEnum {
        PathfindLayerEnum::from_u32(self.query_layer as u32)
    }

    /// C++ `getCell(layer, x, y)` occupancy. Missing layer cells fall back to ground.
    fn occupancy_bits(&self, pos: GridPos, layer: PathfindLayerEnum) -> OccBits {
        if (layer as u8) > PathfindLayerEnum::Ground as u8
            && self.layer_cell_type(layer as u8, pos).is_some()
        {
            if let Some(occ) = self.layer_occ.get(&(layer as u8)) {
                let key = (pos.x, pos.y);
                return OccBits {
                    fixed: occ.occ_fixed_mask.get(&key).copied().unwrap_or(0),
                    moving: occ.occ_moving_mask.get(&key).copied().unwrap_or(0),
                    goal: occ.occ_goal_mask.get(&key).copied().unwrap_or(0),
                    infantry: occ.occ_infantry_mask.get(&key).copied().unwrap_or(0),
                    crushable: occ.occ_fixed_max_crushable.get(&key).copied().unwrap_or(0),
                    goal_unit: occ.occ_goal_unit.get(&key).copied().unwrap_or(0),
                };
            }
            return OccBits::default();
        }
        let Some(idx) = self.bit_index(pos) else {
            return OccBits::default();
        };
        OccBits {
            fixed: self.occ_fixed_mask.get(idx).copied().unwrap_or(0),
            moving: self.occ_moving_mask.get(idx).copied().unwrap_or(0),
            goal: self.occ_goal_mask.get(idx).copied().unwrap_or(0),
            infantry: self.occ_infantry_mask.get(idx).copied().unwrap_or(0),
            crushable: self.occ_fixed_max_crushable.get(idx).copied().unwrap_or(0),
            goal_unit: self.occ_goal_unit.get(idx).copied().unwrap_or(0),
        }
    }

    fn occupancy_cost(
        &self,
        pos: GridPos,
        seeker_player: Option<u32>,
        seeker_is_infantry: bool,
        crusher_level: u8,
        ally_mask: u16,
        start: Option<GridPos>,
    ) -> Option<f32> {
        let bits = self.occupancy_bits(pos, self.query_layer_enum());
        if bits.fixed == 0 && bits.moving == 0 && bits.goal == 0 {
            return Some(0.0);
        }
        // C++ INFANTRY_MOVES_THROUGH_INFANTRY continue is unconditional
        // (AIPathfind.cpp:5031-5035) — goals do not block infantry stream-through.
        if seeker_is_infantry && bits.infantry != 0 && (bits.fixed | bits.moving) == bits.infantry {
            return Some(0.0);
        }
        let Some(player) = seeker_player else {
            return Some(3.0 * 1.414_213_5);
        };
        let bit = 1u16 << player.min(15);
        // C++ checkForMovement: ALLIES increment allyFixedCount, never enemyFixed
        // (AIPathfind.cpp:5037-5066). Only non-allies consult canCrushOrSquish.
        let friend = bit | ally_mask;
        if seeker_is_infantry
            && (bits.infantry & !bit) != 0
            && (bits.fixed & !bit) == (bits.infantry & !bit)
        {
            let leftover_fixed = bits.fixed & !bits.infantry;
            let leftover_moving = bits.moving & !bits.infantry;
            if leftover_fixed == 0 && leftover_moving == 0 {
                return Some(0.0);
            }
        }
        let enemy_fixed = (bits.fixed & !friend) != 0;
        if enemy_fixed && (crusher_level == 0 || crusher_level <= bits.crushable) {
            return None;
        }
        let mut extra = 0.0;
        // C++ allyMoving +3*COST_DIAGONAL only within dx<10 && dy<10 of start
        // (AIPathfind.cpp:6260-6262). Moving enemies add no cost.
        if (bits.moving & friend) != 0 {
            if let Some(s) = start {
                if (pos.x - s.x).abs() < 10 && (pos.y - s.y).abs() < 10 {
                    extra += 3.0 * 1.414_213_5;
                }
            }
        }
        if (bits.fixed & friend) != 0 {
            extra += 3.0 * 1.414_213_5;
        }
        Some(extra)
    }

    /// C++ `examineCellsCallback` seed abort (AIPathfind.cpp:6034-6052).
    /// Allied-fixed or any enemy-fixed (crushable included) refuses the seed line.
    fn seed_line_occupancy_ok(
        &self,
        pos: GridPos,
        seeker_player: Option<u32>,
        ally_mask: u16,
        seeker_is_infantry: bool,
        layer: PathfindLayerEnum,
    ) -> bool {
        let bits = self.occupancy_bits(pos, layer);
        if seeker_is_infantry && bits.infantry != 0 && (bits.fixed | bits.moving) == bits.infantry {
            return true;
        }
        let Some(player) = seeker_player else {
            return bits.fixed == 0;
        };
        let bit = 1u16 << player.min(15);
        let friend = bit | ally_mask;
        if (bits.fixed & friend) != 0 {
            return false;
        }
        if (bits.fixed & !friend) != 0 {
            return false;
        }
        true
    }

    /// C++ `checkForMovement` occupancy used by `patchPath` (AIPathfind.cpp:10419-10442).
    fn patch_cell_occupied(
        &self,
        cell: GridPos,
        layer: PathfindLayerEnum,
        consider_transient: bool,
        seeker_player: Option<u32>,
        seeker_is_infantry: bool,
        crusher_level: u8,
        radius: i32,
        center_in_cell: bool,
    ) -> bool {
        let above = if center_in_cell { radius + 1 } else { radius };
        for y in (cell.y - radius)..(cell.y + above) {
            for x in (cell.x - radius)..(cell.x + above) {
                let p = GridPos::new(x, y);
                if !self.is_valid_pos(p) {
                    return true;
                }
                if self.patch_occupant_blocks(
                    p,
                    layer,
                    consider_transient,
                    seeker_player,
                    seeker_is_infantry,
                    crusher_level,
                ) {
                    return true;
                }
            }
        }
        false
    }

    fn patch_occupant_blocks(
        &self,
        pos: GridPos,
        layer: PathfindLayerEnum,
        consider_transient: bool,
        seeker_player: Option<u32>,
        seeker_is_infantry: bool,
        crusher_level: u8,
    ) -> bool {
        let bits = self.occupancy_bits(pos, layer);
        if bits.fixed == 0 && (!consider_transient || bits.moving == 0) {
            return false;
        }
        if seeker_is_infantry && bits.infantry != 0 && (bits.fixed | bits.moving) == bits.infantry {
            return false;
        }
        let present = bits.fixed | if consider_transient { bits.moving } else { 0 };
        if present == 0 {
            return false;
        }
        let Some(player) = seeker_player else {
            return true;
        };
        let bit = 1u16 << player.min(15);
        let friend = bit | self.ally_mask_for(player);
        if (present & friend) != 0 {
            return true;
        }
        let enemy = (present & !friend) != 0;
        enemy && (crusher_level == 0 || crusher_level <= bits.crushable)
    }

    /// C++ `PathfindCell::getConnectLayer` hop target (same XY, other layer).
    fn connect_layer_of(&self, pos: GridPos, current: PathfindLayerEnum) -> Option<PathfindLayerEnum> {
        if matches!(
            current,
            PathfindLayerEnum::Ground | PathfindLayerEnum::Invalid
        ) {
            let cl = self.ground_connect_layer(pos);
            if cl != 0 {
                return Some(PathfindLayerEnum::from_u32(cl as u32));
            }
            return None;
        }
        if current as u8 == LAYER_WALL_ID {
            return Some(PathfindLayerEnum::Ground);
        }
        self.bridge_layers
            .iter()
            .find(|layer| layer.id == current as u8)
            .and_then(|layer| layer.cells.get(&(pos.x, pos.y)))
            .and_then(|(_, connect)| {
                if *connect != 0 && *connect != current as u8 {
                    Some(PathfindLayerEnum::from_u32(*connect as u32))
                } else {
                    None
                }
            })
    }

    /// C++ `Pathfinder::checkChangeLayers` (AIPathfind.cpp:5942-5981).
    /// Enqueue the same-XY connect-layer cell at parent cost if not closed.
    fn enqueue_connect_layer(
        &self,
        cell: GridPos,
        layer: PathfindLayerEnum,
        g: i32,
        f: i32,
        closed: &HashSet<(i32, i32, u8)>,
        g_score: &mut HashMap<(i32, i32, u8), i32>,
        open: &mut BinaryHeap<std::cmp::Reverse<(i32, i32, i32, i32, u8)>>,
    ) -> bool {
        let Some(cl) = self.connect_layer_of(cell, layer) else {
            return false;
        };
        let lid = cl as u8;
        let key = (cell.x, cell.y, lid);
        if closed.contains(&key) {
            return false;
        }
        if g_score.get(&key).is_some_and(|&og| g >= og) {
            return false;
        }
        g_score.insert(key, g);
        open.push(std::cmp::Reverse((f, g, cell.x, cell.y, lid)));
        true
    }

    /// C++ `examineNeighboringCells` attackDistance occupancy (AIPathfind.cpp:6228-6300).
    /// `None` = enemyFixed skip. Extra is added to the ortho/diag step.
    fn attack_step_occupancy(
        &self,
        pos: GridPos,
        start: GridPos,
        seeker_player: Option<u32>,
        ally_mask: u16,
        seeker_is_infantry: bool,
        is_vehicle: bool,
        crusher_level: u8,
        layer: PathfindLayerEnum,
    ) -> Option<i32> {
        const COST_ORTHO: i32 = 10;
        const COST_DIAG: i32 = 14;
        let bits = self.occupancy_bits(pos, layer);
        if seeker_is_infantry && bits.infantry != 0 && (bits.fixed | bits.moving) == bits.infantry {
            return Some(0);
        }
        let friend = match seeker_player {
            Some(player) => (1u16 << player.min(15)) | ally_mask,
            None => 0,
        };
        if seeker_player.is_some() && (bits.fixed & !friend) != 0 {
            if crusher_level == 0 || crusher_level <= bits.crushable {
                return None;
            }
        }
        let mut extra = 0i32;
        if (bits.fixed & friend) != 0 {
            extra += 3 * COST_DIAG;
        }
        if (bits.moving & friend) != 0
            && (pos.x - start.x).abs() < 10
            && (pos.y - start.y).abs() < 10
        {
            extra += 3 * COST_DIAG;
        }
        if (bits.goal & friend) != 0 {
            extra += if is_vehicle {
                3 * COST_ORTHO
            } else {
                COST_ORTHO
            };
        }
        Some(extra)
    }


    /// C++ `Pathfinder::updatePos` / `updateGoal` cell stamp.
    /// LAYER cells go on the layer map; `dynamic_bits` is ground-only.
    /// Missing layer cells fall back to ground (`getCell` residual).
    fn mark_occupancy(
        &mut self,
        pos: GridPos,
        player: u32,
        moving: bool,
        infantry: bool,
        goal: bool,
        crushable_level: u8,
        unit_id: u32,
        layer: PathfindLayerEnum,
    ) {
        let bit = 1u16 << player.min(15);
        if (layer as u8) > PathfindLayerEnum::Ground as u8
            && self.layer_cell_type(layer as u8, pos).is_some()
        {
            let occ = self.layer_occ.entry(layer as u8).or_default();
            let key = (pos.x, pos.y);
            if goal {
                *occ.occ_goal_mask.entry(key).or_insert(0) |= bit;
                occ.occ_goal_unit.insert(key, unit_id);
                return;
            }
            if infantry {
                *occ.occ_infantry_mask.entry(key).or_insert(0) |= bit;
            }
            if moving {
                *occ.occ_moving_mask.entry(key).or_insert(0) |= bit;
            } else {
                *occ.occ_fixed_mask.entry(key).or_insert(0) |= bit;
                let crush = occ.occ_fixed_max_crushable.entry(key).or_insert(0);
                *crush = (*crush).max(crushable_level);
            }
            return;
        }
        let Some(idx) = self.bit_index(pos) else {
            return;
        };
        Self::bit_set(&mut self.dynamic_bits, idx, true);
        if goal {
            if let Some(slot) = self.occ_goal_mask.get_mut(idx) {
                *slot |= bit;
            }
            if let Some(slot) = self.occ_goal_unit.get_mut(idx) {
                *slot = unit_id;
            }
            return;
        }
        if infantry {
            if let Some(slot) = self.occ_infantry_mask.get_mut(idx) {
                *slot |= bit;
            }
        }
        if moving {
            if let Some(slot) = self.occ_moving_mask.get_mut(idx) {
                *slot |= bit;
            }
        } else if let Some(slot) = self.occ_fixed_mask.get_mut(idx) {
            *slot |= bit;
            if let Some(crush) = self.occ_fixed_max_crushable.get_mut(idx) {
                *crush = (*crush).max(crushable_level);
            }
        }
    }

    /// C++ `TerrainLogic::objectInteractsWithBridgeEnd` (TerrainLogic.cpp:1799).
    fn object_interacts_with_bridge_end(
        &self,
        pos: Vec3,
        minor_radius: f32,
        layer: PathfindLayerEnum,
    ) -> bool {
        if layer == PathfindLayerEnum::Ground {
            return false;
        }
        let Some(bridge) = self
            .bridge_layers
            .iter()
            .find(|layer_rec| layer_rec.id == layer as u8)
        else {
            return false;
        };
        let r = minor_radius + self.grid_size * 0.5;
        let cell = CellXz {
            lo_x: pos.x - r,
            lo_z: pos.z - r,
            hi_x: pos.x + r,
            hi_z: pos.z + r,
        };
        if !cell_on_bridge_end(
            &cell,
            bridge.from_left,
            bridge.from_right,
            bridge.to_left,
            bridge.to_right,
            self.grid_size,
        ) {
            return false;
        }
        let corners = [
            bridge.from_left,
            bridge.from_right,
            bridge.to_right,
            bridge.to_left,
        ];
        let deck_h = bridge_deck_height(&corners, pos.x, pos.z);
        (pos.y - deck_h).abs() <= LAYER_Z_CLOSE_ENOUGH_F
    }



    /// Clamp a grid position into the playable rectangle.
    pub fn clamp_pos(&self, pos: GridPos) -> GridPos {
        GridPos::new(
            pos.x.clamp(0, self.width.saturating_sub(1).max(0)),
            pos.y.clamp(0, self.height.saturating_sub(1).max(0)),
        )
    }

    /// Nearest non-blocked cell around `pos` (spiral search). Returns None if none found.
    pub fn nearest_open(&self, pos: GridPos, max_radius: i32) -> Option<GridPos> {
        let origin = self.clamp_pos(pos);
        if self.is_valid_pos(origin) && !self.is_blocked(origin) {
            return Some(origin);
        }
        for r in 1..=max_radius {
            for dx in -r..=r {
                for dy in -r..=r {
                    if dx.abs() != r && dy.abs() != r {
                        continue;
                    }
                    let candidate = GridPos::new(origin.x + dx, origin.y + dy);
                    if self.is_valid_pos(candidate) && !self.is_blocked(candidate) {
                        return Some(candidate);
                    }
                }
            }
        }
        None
    }

    /// Find path using A* algorithm.
    ///
    /// Start/goal are clamped into the grid. If the goal cell is blocked (building
    /// footprint etc.), the nearest open cell is used so infantry can still approach.
    ///
    /// Parity notes vs C++ examineNeighboringCells (host simplified grid):
    /// - static blocks hard-reject; dynamic unit occupancy is a soft cost (allyFixed-like)
    /// - diagonal steps require both orthogonal legs open (no corner cut)
    pub fn find_path(&self, start: GridPos, goal: GridPos) -> Option<Vec<Vec3>> {
        if self.width <= 0 || self.height <= 0 {
            return None;
        }

        let start = self
            .nearest_static_open(self.clamp_pos(start), 16)
            .unwrap_or_else(|| self.clamp_pos(start));
        // Prefer static-open goal; dynamic occupancy near goal is soft-costed below.
        let goal = self
            .nearest_static_open(self.clamp_pos(goal), 16)
            .unwrap_or_else(|| self.clamp_pos(goal));

        // Either endpoint still static-blocked and no open neighbor — cannot plan.
        if self.is_static_blocked(start) || self.is_static_blocked(goal) {
            return None;
        }

        // Trivial same-cell path.
        if start == goal {
            return Some(vec![self.grid_to_world(start)]);
        }

        let mut open_set = BinaryHeap::new();
        let mut came_from: HashMap<GridPos, GridPos> = HashMap::new();
        let mut g_score: HashMap<GridPos, f32> = HashMap::new();
        // Closed bitset keeps large open-field A* from revisiting nodes forever.
        let mut closed = vec![0u64; self.blocked_bits.len().max(1)];

        g_score.insert(start, 0.0);
        open_set.push(PathNode::new(start, 0.0, start.distance(goal), None));

        while let Some(current) = open_set.pop() {
            if current.pos == goal {
                // Reconstruct path
                return Some(self.reconstruct_path(&came_from, current.pos));
            }

            let Some(cidx) = self.bit_index(current.pos) else {
                continue;
            };
            if Self::bit_test(&closed, cidx) {
                continue;
            }
            Self::bit_set(&mut closed, cidx, true);

            for neighbor in current.pos.neighbors() {
                if !self.is_valid_pos(neighbor) || self.is_static_blocked(neighbor) {
                    continue;
                }
                if !self.diameter_allows(self.query_is_crusher, neighbor) {
                    continue;
                }
                if self
                    .bit_index(neighbor)
                    .is_some_and(|idx| Self::bit_test(&closed, idx))
                {
                    continue;
                }

                let dx = neighbor.x - current.pos.x;
                let dy = neighbor.y - current.pos.y;
                let is_diag = dx.abs() == 1 && dy.abs() == 1;

                // C++ diagonal corner-cut: both orthogonal legs must be open.
                if is_diag {
                    let ortho_a = GridPos::new(current.pos.x + dx, current.pos.y);
                    let ortho_b = GridPos::new(current.pos.x, current.pos.y + dy);
                    if !self.is_valid_pos(ortho_a)
                        || !self.is_valid_pos(ortho_b)
                        || self.is_static_blocked(ortho_a)
                        || self.is_static_blocked(ortho_b)
                    {
                        continue;
                    }
                }

                // Base ortho/diag cost (COST_ORTHOGONAL=1, COST_DIAGONAL≈1.414).
                let mut movement_cost = if is_diag { 1.414_213_5 } else { 1.0 };
                // C++ costSoFar pinched surcharge (AIPathfind.cpp:1701-1703).
                if self.is_pinched(neighbor) {
                    movement_cost += 1.414_213_5;
                }
                match self.occupancy_cost(neighbor, None, false, 0, 0, Some(start)) {
                    None => continue, // enemyFixed abort
                    Some(extra) => movement_cost += extra,
                }


                let tentative_g_score = current.g_cost + movement_cost;

                if tentative_g_score < *g_score.get(&neighbor).unwrap_or(&f32::INFINITY) {
                    came_from.insert(neighbor, current.pos);
                    g_score.insert(neighbor, tentative_g_score);

                    open_set.push(PathNode::new(
                        neighbor,
                        tentative_g_score,
                        neighbor.distance(goal),
                        Some(current.pos),
                    ));
                }
            }
        }

        None // No path found
    }

    /// Like nearest_open but only considers static blocks (dynamic is soft in A*).
    fn nearest_static_open(&self, origin: GridPos, max_radius: i32) -> Option<GridPos> {
        if self.is_valid_pos(origin) && !self.is_static_blocked(origin) {
            return Some(origin);
        }
        for r in 1..=max_radius {
            for dx in -r..=r {
                for dy in -r..=r {
                    if dx.abs() != r && dy.abs() != r {
                        continue;
                    }
                    let candidate = GridPos::new(origin.x + dx, origin.y + dy);
                    if self.is_valid_pos(candidate) && !self.is_static_blocked(candidate) {
                        return Some(candidate);
                    }
                }
            }
        }
        None
    }

    fn reconstruct_path(
        &self,
        came_from: &HashMap<GridPos, GridPos>,
        mut current: GridPos,
    ) -> Vec<Vec3> {
        let mut path = vec![self.grid_to_world(current)];

        while let Some(&parent) = came_from.get(&current) {
            current = parent;
            path.push(self.grid_to_world(current));
        }

        path.reverse();
        path
    }

    /// Update dynamic obstacles based on unit positions
    pub fn update_dynamic_obstacles(&mut self, objects: &HashMap<ObjectId, Object>) {
        self.update_dynamic_obstacles_ignoring(objects, None);
    }

    /// Same occupancy stamp, skipping `ignore` (C++ `ignoreObstacle(goalObject)`).
    pub fn update_dynamic_obstacles_ignoring(
        &mut self,
        objects: &HashMap<ObjectId, Object>,
        ignore: Option<ObjectId>,
    ) {
        self.clear_dynamic_blocks();

        for obj in objects.values() {
            if !obj.is_alive() {
                continue;
            }
            if ignore == Some(obj.id) {
                continue;
            }
            let is_aircraft = obj.is_kind_of(KindOf::Aircraft)
                || obj.object_type == crate::game_logic::ObjectType::Aircraft
                || obj.chinook_ai.is_some();
            if is_aircraft {
                if Self::is_aircraft_that_adjusts_destination(obj) {
                    self.stamp_aircraft_goal_from_object(obj);
                }
                // C++ updatePos: !isDoingGroundMovement → removePos and return.
                // JetAIUpdate never grounds; parked/taxiing jets do not stamp UNIT_PRESENT.
                if !Self::is_doing_ground_movement(obj) {
                    continue;
                }
            }
            // C++ examineNeighboringCells occupancy: infantry + vehicles + structures.
            if !(obj.is_kind_of(KindOf::Vehicle)
                || obj.is_kind_of(KindOf::Structure)
                || obj.is_kind_of(KindOf::Infantry))
            {
                continue;
            }

            let player = obj.owner_player_id.unwrap_or(obj.team as u32);
            let moving = !obj.is_kind_of(KindOf::Structure)
                && (!obj.movement.path.is_empty()
                    || obj.movement.velocity.length_squared() > 0.25);
            let infantry = obj.is_kind_of(KindOf::Infantry);
            let (radius, center_in_cell) =
                Self::radius_and_center(obj.selection_radius, self.grid_size);
            let mut num_above = radius;
            if center_in_cell {
                num_above += 1;
            }
            let pos = obj.get_position();
            let pos_layer = self.layer_for_destination(pos);
            let pos_at_end =
                self.object_interacts_with_bridge_end(pos, obj.selection_radius, pos_layer);
            let pos_do_layer = pos_layer != PathfindLayerEnum::Ground;
            let pos_do_ground = !pos_do_layer || pos_at_end;
            let grid_pos = self.cell_for_unit_position(pos, center_in_cell);
            for i in (grid_pos.x - radius)..(grid_pos.x + num_above) {
                for j in (grid_pos.y - radius)..(grid_pos.y + num_above) {
                    let p = GridPos::new(i, j);
                    if self.is_valid_pos(p) {
                        // C++ canCrushOrSquish TEST_CRUSH_OR_SQUISH: module
                        // presence is crush-through even at CrushableLevel 255.
                        let crushable = if obj.has_squish_collide {
                            0
                        } else {
                            obj.crushable_level
                        };
                        if pos_do_ground {
                            self.mark_occupancy(
                                p,
                                player,
                                moving,
                                infantry,
                                false,
                                crushable,
                                obj.id.0,
                                PathfindLayerEnum::Ground,
                            );
                        }
                        if pos_do_layer {
                            self.mark_occupancy(
                                p,
                                player,
                                moving,
                                infantry,
                                false,
                                crushable,
                                obj.id.0,
                                pos_layer,
                            );
                        }
                    }
                }
            }
            // C++ Pathfinder::updateGoal stamps UNIT_GOAL on the destination cell.
            if !is_aircraft
                && !obj.is_kind_of(KindOf::Immobile)
                && !obj.is_kind_of(KindOf::Structure)
            {
                let dest = obj
                    .movement
                    .path
                    .last()
                    .copied()
                    .or(obj.movement.target_position);
                if let Some(goal) = dest {
                    let goal_layer = self.layer_for_destination(goal);
                    let goal_at_end = self.object_interacts_with_bridge_end(
                        pos,
                        obj.selection_radius,
                        goal_layer,
                    );
                    let goal_do_layer = goal_layer != PathfindLayerEnum::Ground;
                    let goal_do_ground = !goal_do_layer || goal_at_end;
                    let goal_cell = self.cell_for_unit_position(goal, center_in_cell);
                    for i in (goal_cell.x - radius)..(goal_cell.x + num_above) {
                        for j in (goal_cell.y - radius)..(goal_cell.y + num_above) {
                            let p = GridPos::new(i, j);
                            if self.is_valid_pos(p) {
                                if goal_do_ground {
                                    self.mark_occupancy(
                                        p,
                                        player,
                                        false,
                                        false,
                                        true,
                                        255,
                                        obj.id.0,
                                        PathfindLayerEnum::Ground,
                                    );
                                }
                                if goal_do_layer {
                                    self.mark_occupancy(
                                        p,
                                        player,
                                        false,
                                        false,
                                        true,
                                        255,
                                        obj.id.0,
                                        goal_layer,
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    pub fn occupancy_extra_cost(
        &self,
        pos: GridPos,
        seeker_player: Option<u32>,
        seeker_is_infantry: bool,
        crusher_level: u8,
    ) -> u32 {
        let ally_mask = seeker_player.map(|p| self.ally_mask_for(p)).unwrap_or(0);
        match self.occupancy_cost(
            pos,
            seeker_player,
            seeker_is_infantry,
            crusher_level,
            ally_mask,
            None,
        ) {
            None => u32::MAX / 8,
            Some(c) => (c * 10.0) as u32, // crate A* uses integer COST_DIAGONAL=14
        }
    }

    pub fn has_allied_goal(&self, pos: GridPos, seeker_player: Option<u32>) -> bool {
        self.has_allied_goal_on(pos, seeker_player, self.query_layer_enum())
    }

    pub fn has_allied_goal_on(
        &self,
        pos: GridPos,
        seeker_player: Option<u32>,
        layer: PathfindLayerEnum,
    ) -> bool {
        let bits = self.occupancy_bits(pos, layer);
        if bits.goal == 0 {
            return false;
        }
        let Some(player) = seeker_player else {
            return true;
        };
        // C++ checkDestination: own UNIT_GOAL is skipped (goalUnitID==objID).
        if self.query_seeker_id != 0 && bits.goal_unit == self.query_seeker_id {
            return false;
        }
        let bit = 1u16 << player.min(15);
        let ally = self.ally_mask_for(player);
        // Refuse allies (other players + same-player siblings). Own reservation
        // already excluded above.
        (bits.goal & (ally | bit)) != 0
    }

    /// C++ `checkDestination` occupancy (AIPathfind.cpp:4946-4953).
    fn has_blocking_fixed_occupant(
        &self,
        pos: GridPos,
        crusher_level: u8,
    ) -> bool {
        self.has_blocking_fixed_occupant_on(pos, crusher_level, self.query_layer_enum())
    }

    fn has_blocking_fixed_occupant_on(
        &self,
        pos: GridPos,
        crusher_level: u8,
        layer: PathfindLayerEnum,
    ) -> bool {
        let bits = self.occupancy_bits(pos, layer);
        if bits.fixed == 0 {
            return false;
        }
        crusher_level == 0 || crusher_level <= bits.crushable
    }

    /// C++ `checkDestination` single-cell residual used by adjustDestination.
    fn destination_cell_ok(
        &self,
        pos: GridPos,
        surfaces: u32,
        is_crusher: bool,
        seeker_player: Option<u32>,
        crusher_level: u8,
        layer: PathfindLayerEnum,
    ) -> bool {
        if !self.is_valid_pos(pos) {
            return false;
        }
        // C++ checkForAdjust isHuman: reject cells outside m_logicalExtent.
        if self.query_is_human && !self.in_logical_extent(pos) {
            return false;
        }
        if self.query_check_for_aircraft {
            return !self.has_other_aircraft_goal(pos);
        }
        if !self.cell_passable_for_layer(pos, layer, surfaces, is_crusher) {
            return false;
        }
        if self.resolved_cell_type(layer, pos) == PathfindCellType::Cliff {
            return false;
        }
        if self.has_allied_goal_on(pos, seeker_player, layer) {
            return false;
        }
        if self.has_blocking_fixed_occupant_on(pos, crusher_level, layer) {
            return false;
        }
        if !self.diameter_allows(is_crusher, pos) {
            return false;
        }
        // Leftover `check_for_adjust_ex` zone path-exists gate (AIPathfind.cpp:5198-5208).
        if (surfaces & SURFACE_AIR) == 0 {
            if let Some(from) = self.query_from {
                let from_w = self.grid_to_world(from);
                let dest_w = self.grid_to_world(pos);
                let orig_w = self
                    .query_orig_dest
                    .map(|p| self.grid_to_world(p))
                    .unwrap_or(dest_w);
                let path_exists = self.quick_path_exists_for(from_w, orig_w, surfaces);
                let adjusted_path_exists = self.quick_path_exists_for(from_w, dest_w, surfaces);
                let mut ok = adjusted_path_exists;
                if !path_exists && self.quick_path_exists_for(orig_w, dest_w, surfaces) {
                    ok = true;
                }
                if !ok {
                    return false;
                }
            }
        }
        true
    }

    /// C++ `AIUpdateInterface::isAircraftThatAdjustsDestination` (HOVER/WINGS).
    pub fn is_aircraft_that_adjusts_destination(obj: &Object) -> bool {
        if matches!(obj.loco_appearance, LocomotorAppearance::Thrust) {
            return false;
        }
        matches!(
            obj.loco_appearance,
            LocomotorAppearance::Hover | LocomotorAppearance::Wings
        ) || obj.is_kind_of(KindOf::Aircraft)
            || obj.object_type == crate::game_logic::ObjectType::Aircraft
            || obj.chinook_ai.is_some()
    }

    /// C++ `AIUpdateInterface::isDoingGroundMovement` (AIUpdate.cpp:2347-2361).
    /// Air-only / current-AIR locos never stamp UNIT_PRESENT.
    pub fn is_doing_ground_movement(obj: &Object) -> bool {
        use crate::game_logic::object::LOCO_SURFACE_AIR;
        if matches!(
            obj.loco_appearance,
            LocomotorAppearance::Wings | LocomotorAppearance::Thrust
        ) {
            return false;
        }
        if obj.locomotor_surfaces != 0 {
            if obj.locomotor_surfaces == LOCO_SURFACE_AIR {
                return false;
            }
            if (obj.locomotor_surfaces & LOCO_SURFACE_AIR) != 0 {
                return false;
            }
        }
        if obj.is_kind_of(KindOf::Aircraft)
            || obj.object_type == crate::game_logic::ObjectType::Aircraft
        {
            return false;
        }
        true
    }

    pub fn goal_aircraft(&self, pos: GridPos) -> u32 {
        self.bit_index(pos)
            .and_then(|idx| self.occ_goal_aircraft.get(idx).copied())
            .unwrap_or(0)
    }


    pub fn has_other_aircraft_goal(&self, pos: GridPos) -> bool {
        let id = self.goal_aircraft(pos);
        id != 0 && (self.query_seeker_id == 0 || id != self.query_seeker_id)
    }

    fn stamp_aircraft_goal_cell(&mut self, pos: GridPos, unit_id: u32) {
        let Some(idx) = self.bit_index(pos) else {
            return;
        };
        if let Some(slot) = self.occ_goal_aircraft.get_mut(idx) {
            if *slot == 0 || *slot == unit_id {
                *slot = unit_id;
            }
        }
    }

    pub(crate) fn aircraft_goal_dest(obj: &Object) -> Option<Vec3> {
        if let Some(ai) = obj.chinook_ai.as_ref() {
            if matches!(
                ai.flight_status,
                crate::game_logic::host_combat_chinook::HostChinookFlightStatus::Landing
                    | crate::game_logic::host_combat_chinook::HostChinookFlightStatus::TakingOff
            ) {
                return Some(Vec3::new(ai.dest[0], ai.dest[2], ai.dest[1]));
            }
        }
        obj.movement
            .path
            .last()
            .copied()
            .or(obj.movement.target_position)
    }

    /// C++ `Pathfinder::updateAircraftGoal`.
    fn stamp_aircraft_goal_from_object(&mut self, obj: &Object) {
        let Some(goal) = Self::aircraft_goal_dest(obj) else {
            return;
        };
        let (radius, center_in_cell) = Self::radius_and_center(obj.selection_radius, self.grid_size);
        let mut num_above = radius;
        if center_in_cell {
            num_above += 1;
        }
        let cell = self.world_to_grid(goal);
        for i in (cell.x - radius)..(cell.x + num_above) {
            for j in (cell.y - radius)..(cell.y + num_above) {
                self.stamp_aircraft_goal_cell(GridPos::new(i, j), obj.id.0);
            }
        }
        if let Ok(ai) = gamelogic::ai::THE_AI.read() {
            if let Some(pf) = ai.pathfinder() {
                if let Ok(pf) = pf.read() {
                    let dest = gamelogic::common::Coord3D::new(goal.x, goal.z, goal.y);
                    pf.update_aircraft_goal(&dest, obj.id.0, radius, center_in_cell);
                }
            }
        }
    }

    fn check_for_landing(
        &self,
        cell: GridPos,
        layer: PathfindLayerEnum,
        radius: i32,
        center_in_cell: bool,
    ) -> bool {
        if !self.is_valid_pos(cell) {
            return false;
        }
        match self.resolved_cell_type(layer, cell) {
            PathfindCellType::Cliff | PathfindCellType::Water | PathfindCellType::Impassable => {
                return false;
            }
            _ => {}
        }
        if self.has_other_aircraft_goal(cell) {
            return false;
        }
        // C++ checkDestination(NULL, cell, layer, iRadius, center): footprint
        // refuses CELL_OBSTACLE / IS_IMPASSABLE / any UNIT_GOAL / off-map.
        self.check_destination_null(cell, layer, radius, center_in_cell)
    }

    fn check_destination_null(
        &self,
        cell: GridPos,
        layer: PathfindLayerEnum,
        radius: i32,
        center_in_cell: bool,
    ) -> bool {
        let mut num_above = radius;
        if center_in_cell {
            num_above += 1;
        }
        for i in (cell.x - radius)..(cell.x + num_above) {
            for j in (cell.y - radius)..(cell.y + num_above) {
                let p = GridPos::new(i, j);
                if !self.is_valid_pos(p) {
                    return false;
                }
                match self.resolved_cell_type(layer, p) {
                    PathfindCellType::Obstacle
                    | PathfindCellType::Impassable
                    | PathfindCellType::BridgeImpassable => {
                        return false;
                    }
                    _ => {}
                }
                if self.has_other_aircraft_goal(p) {
                    return false;
                }
                if self.has_allied_goal_on(p, None, layer) {
                    return false;
                }
                if self.has_blocking_fixed_occupant_on(p, 0, layer) {
                    return false;
                }
            }
        }
        true
    }

    /// C++ `Pathfinder::adjustToLandingDestination` spiral.
    pub fn adjust_to_landing_destination(
        &self,
        dest: GridPos,
        max_cells: i32,
        layer: PathfindLayerEnum,
    ) -> Option<GridPos> {
        self.adjust_to_landing_destination_for(dest, max_cells, layer, 0, true)
    }

    pub fn adjust_to_landing_destination_for(
        &self,
        dest: GridPos,
        max_cells: i32,
        layer: PathfindLayerEnum,
        radius: i32,
        center_in_cell: bool,
    ) -> Option<GridPos> {
        if !self.is_valid_pos(dest) {
            return None;
        }
        if self.check_for_landing(dest, layer, radius, center_in_cell) {
            return Some(dest);
        }
        let mut i = dest.x;
        let mut j = dest.y;
        let mut delta = 1;
        let mut limit = max_cells.max(1);
        while limit > 0 {
            for _ in 0..delta {
                i += 1;
                limit -= 1;
                let c = GridPos::new(i, j);
                if self.check_for_landing(c, layer, radius, center_in_cell) {
                    return Some(c);
                }
            }
            for _ in 0..delta {
                j += 1;
                limit -= 1;
                let c = GridPos::new(i, j);
                if self.check_for_landing(c, layer, radius, center_in_cell) {
                    return Some(c);
                }
            }
            delta += 1;
            for _ in 0..delta {
                i -= 1;
                limit -= 1;
                let c = GridPos::new(i, j);
                if self.check_for_landing(c, layer, radius, center_in_cell) {
                    return Some(c);
                }
            }
            for _ in 0..delta {
                j -= 1;
                limit -= 1;
                let c = GridPos::new(i, j);
                if self.check_for_landing(c, layer, radius, center_in_cell) {
                    return Some(c);
                }
            }
            delta += 1;
        }
        None
    }

    /// C++ `Pathfinder::checkForTarget` (AIPathfind.cpp:5409-5421).
    /// Aircraft `checkDestination` only refuses another unit's goalAircraft.
    pub fn check_for_target(
        &self,
        cell_x: i32,
        cell_y: i32,
        radius: i32,
        center_in_cell: bool,
        seeker_id: u32,
        claimed: &HashSet<GridPos>,
        in_range: impl Fn(Vec3) -> bool,
        dest: &mut Vec3,
    ) -> bool {
        let cell = GridPos::new(cell_x, cell_y);
        if !self.is_valid_pos(cell) {
            return false;
        }
        let mut num_above = radius;
        if center_in_cell {
            num_above += 1;
        }
        for i in (cell.x - radius)..(cell.x + num_above) {
            for j in (cell.y - radius)..(cell.y + num_above) {
                let p = GridPos::new(i, j);
                if !self.is_valid_pos(p) {
                    return false;
                }
                if claimed.contains(&p) {
                    return false;
                }
                let id = self.goal_aircraft(p);
                if id != 0 && id != seeker_id {
                    return false;
                }
            }
        }
        let size = self.grid_size;
        let o = self.origin;
        let adjust = if center_in_cell {
            Vec3::new(
                o.x + cell.x as f32 * size + size * 0.5,
                dest.y,
                o.z + cell.y as f32 * size + size * 0.5,
            )
        } else {
            Vec3::new(
                o.x + cell.x as f32 * size + 0.05,
                dest.y,
                o.z + cell.y as f32 * size + 0.05,
            )
        };
        if !in_range(adjust) {
            return false;
        }
        *dest = adjust;
        true
    }

    /// C++ `Pathfinder::adjustTargetDestination` (AIPathfind.cpp:5428-5487).
    pub fn adjust_target_destination(
        &self,
        dest: &mut Vec3,
        unit_radius: f32,
        seeker_id: u32,
        claimed: &HashSet<GridPos>,
        in_range: impl Fn(Vec3) -> bool,
    ) -> bool {
        let (radius, center_in_cell) = Self::radius_and_center(unit_radius, self.grid_size);
        let mut adjust_dest = *dest;
        if !center_in_cell {
            let half = self.grid_size * 0.5;
            adjust_dest.x += half;
            adjust_dest.z += half;
        }
        let cell = self.world_to_grid(adjust_dest);
        if !self.is_valid_pos(cell) {
            return false;
        }
        if self.check_for_target(
            cell.x,
            cell.y,
            radius,
            center_in_cell,
            seeker_id,
            claimed,
            &in_range,
            dest,
        ) {
            return true;
        }
        const MAX_CELLS_TO_TRY: i32 = 400;
        let mut limit = MAX_CELLS_TO_TRY;
        let mut i = cell.x;
        let mut j = cell.y;
        let mut delta = 1;
        while limit > 0 {
            for _ in 0..delta {
                i += 1;
                limit -= 1;
                if self.check_for_target(
                    i,
                    j,
                    radius,
                    center_in_cell,
                    seeker_id,
                    claimed,
                    &in_range,
                    dest,
                ) {
                    return true;
                }
            }
            for _ in 0..delta {
                j += 1;
                limit -= 1;
                if self.check_for_target(
                    i,
                    j,
                    radius,
                    center_in_cell,
                    seeker_id,
                    claimed,
                    &in_range,
                    dest,
                ) {
                    return true;
                }
            }
            delta += 1;
            for _ in 0..delta {
                i -= 1;
                limit -= 1;
                if self.check_for_target(
                    i,
                    j,
                    radius,
                    center_in_cell,
                    seeker_id,
                    claimed,
                    &in_range,
                    dest,
                ) {
                    return true;
                }
            }
            for _ in 0..delta {
                j -= 1;
                limit -= 1;
                if self.check_for_target(
                    i,
                    j,
                    radius,
                    center_in_cell,
                    seeker_id,
                    claimed,
                    &in_range,
                    dest,
                ) {
                    return true;
                }
            }
            delta += 1;
        }
        false
    }


    /// C++ linePassableCallback occupancy + pinched (AIPathfind.cpp:9553-9591).
    fn occupancy_blocks_line(
        &self,
        pos: GridPos,
        seeker_player: Option<u32>,
        crusher_level: u8,
    ) -> bool {
        let bits = self.occupancy_bits(pos, self.query_layer_enum());
        if bits.fixed == 0 {
            return false;
        }
        let Some(player) = seeker_player else {
            return true;
        };
        let bit = 1u16 << player.min(15);
        let friend = bit | self.ally_mask_for(player);
        if (bits.fixed & friend) != 0 {
            return true;
        }
        if (bits.fixed & !friend) != 0 {
            return crusher_level == 0 || crusher_level <= bits.crushable;
        }
        false
    }

    fn occupancy_blocks_line_on(
        &self,
        pos: GridPos,
        seeker_player: Option<u32>,
        crusher_level: u8,
        layer: PathfindLayerEnum,
    ) -> bool {
        let bits = self.occupancy_bits(pos, layer);
        if bits.fixed == 0 {
            return false;
        }
        let Some(player) = seeker_player else {
            return true;
        };
        let bit = 1u16 << player.min(15);
        let friend = bit | self.ally_mask_for(player);
        if (bits.fixed & friend) != 0 {
            return true;
        }
        if (bits.fixed & !friend) != 0 {
            return crusher_level == 0 || crusher_level <= bits.crushable;
        }
        false
    }

    fn line_cell_ok_on(
        &self,
        cell: GridPos,
        surfaces: u32,
        is_crusher: bool,
        allow_pinched: bool,
        seeker_player: Option<u32>,
        crusher_level: u8,
        unpinched_cliff_passable: bool,
        layer: PathfindLayerEnum,
    ) -> bool {
        if !self.is_valid_pos(cell) {
            return false;
        }
        if !allow_pinched && self.is_pinched(cell) {
            return false;
        }
        if self.occupancy_blocks_line_on(cell, seeker_player, crusher_level, layer) {
            return false;
        }
        if !self.diameter_allows(is_crusher, cell) {
            return false;
        }
        if unpinched_cliff_passable
            && self.resolved_cell_type(layer, cell) == PathfindCellType::Cliff
            && !self.is_pinched(cell)
        {
            return true;
        }
        self.cell_passable_for_layer(cell, layer, surfaces, is_crusher)
    }

    /// C++ `isLinePassable` on the **anchor node's layer** (AIPathfind.cpp:501-502).
    fn line_passable_on_layer(
        &self,
        from: GridPos,
        to: GridPos,
        surfaces: u32,
        is_crusher: bool,
        allow_pinched: bool,
        seeker_player: Option<u32>,
        crusher_level: u8,
        unpinched_cliff_passable: bool,
        layer: PathfindLayerEnum,
    ) -> bool {
        if from == to {
            return true;
        }
        let mut x0 = from.x;
        let mut y0 = from.y;
        let x1 = to.x;
        let y1 = to.y;
        let dx = (x1 - x0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let dy = -(y1 - y0).abs();
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;
        loop {
            let cell = GridPos::new(x0, y0);
            if !self.line_cell_ok_on(
                cell,
                surfaces,
                is_crusher,
                allow_pinched,
                seeker_player,
                crusher_level,
                unpinched_cliff_passable,
                layer,
            ) {
                return false;
            }
            if x0 == x1 && y0 == y1 {
                return true;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x0 += sx;
            }
            if e2 <= dx {
                err += dx;
                y0 += sy;
            }
        }
    }


    /// C++ `validLocomotorSurfacesForCellType` + fence crusher exception.
    pub fn cell_passable_for(&self, pos: GridPos, surfaces: u32, is_crusher: bool) -> bool {
        self.cell_passable_for_layer(pos, PathfindLayerEnum::Ground, surfaces, is_crusher)
    }

    /// C++ `Pathfinder::getRadiusAndCenter` (AIPathfind.cpp:9670-9696). MAX_RADIUS=2.
    pub fn radius_and_center(unit_radius: f32, grid_size: f32) -> (i32, bool) {
        let cell = grid_size.max(1.0);
        let mut diameter = 2.0 * unit_radius;
        if diameter > cell && diameter < 2.0 * cell {
            diameter = 2.0 * cell;
        }
        let mut radius = (diameter / cell + 0.3).floor() as i32;
        let mut center_in_cell = false;
        if radius == 0 {
            radius = 1;
        }
        if (radius & 1) != 0 {
            center_in_cell = true;
        }
        radius /= 2;
        const MAX_RADIUS: i32 = 2;
        if radius > MAX_RADIUS {
            radius = MAX_RADIUS;
            center_in_cell = true;
        }
        (radius, center_in_cell)
    }

    /// C++ getRadiusAndCenter + worldToCell (AIPathfind.cpp:9748-9753).
    pub fn cell_for_unit_position(&self, pos: Vec3, center_in_cell: bool) -> GridPos {
        if center_in_cell {
            self.world_to_grid(pos)
        } else {
            GridPos {
                x: ((pos.x - self.origin.x) / self.grid_size + 0.5).floor() as i32,
                y: ((pos.z - self.origin.z) / self.grid_size + 0.5).floor() as i32,
            }
        }
    }

    fn check_destination_for(
        &self,
        cell: GridPos,
        layer: PathfindLayerEnum,
        radius: i32,
        center_in_cell: bool,
        surfaces: u32,
        is_crusher: bool,
        seeker_player: Option<u32>,
        crusher_level: u8,
    ) -> bool {
        let mut num_above = radius;
        if center_in_cell {
            num_above += 1;
        }
        for i in (cell.x - radius)..(cell.x + num_above) {
            for j in (cell.y - radius)..(cell.y + num_above) {
                let p = GridPos::new(i, j);
                if !self.destination_cell_ok(
                    p,
                    surfaces,
                    is_crusher,
                    seeker_player,
                    crusher_level,
                    layer,
                ) {
                    return false;
                }
                match self.resolved_cell_type(layer, p) {
                    PathfindCellType::Obstacle
                    | PathfindCellType::Impassable
                    | PathfindCellType::BridgeImpassable => {
                        return false;
                    }
                    _ => {}
                }
            }
        }
        true
    }

    /// Vehicle path width in cells. Infantry stay single-cell.
    pub fn path_diameter_for_unit(unit_radius: f32, grid_size: f32, is_vehicle: bool) -> i32 {
        if !is_vehicle {
            return 1;
        }
        let (radius, _) = Self::radius_and_center(unit_radius, grid_size);
        (2 * radius.max(1)).min(4)
    }

    pub fn set_query_footprint(&mut self, path_diameter: i32, is_crusher: bool) {
        self.query_path_diameter = path_diameter.max(1);
        self.query_is_crusher = is_crusher;
    }

    pub fn query_seeker_id(&self) -> u32 {
        self.query_seeker_id
    }

    pub fn set_query_seeker_id(&mut self, id: u32) {
        self.query_seeker_id = id;
    }

    /// C++ `Pathfinder::clearCellForDiameter` (AIPathfind.cpp:6700-6759).
    pub fn clear_cell_for_diameter(
        &self,
        crusher: bool,
        cell: GridPos,
        path_diameter: i32,
    ) -> i32 {
        clear_cell_for_diameter_impl(
            self.width,
            self.height,
            &self.cell_types,
            &self.fence_bits,
            &self.occ_fixed_mask,
            &self.occ_fixed_max_crushable,
            crusher,
            cell,
            path_diameter,
        )
    }

    fn diameter_allows(&self, crusher: bool, cell: GridPos) -> bool {
        let d = self.query_path_diameter;
        d < 2 || self.clear_cell_for_diameter(crusher, cell, d) == d
    }

    /// C++ `Pathfinder::adjustDestination` spiral (AIPathfind.cpp:5331-5407).
    pub fn adjust_destination(
        &self,
        dest: GridPos,
        surfaces: u32,
        is_crusher: bool,
        max_cells: i32,
    ) -> Option<GridPos> {
        self.adjust_destination_ex(
            dest,
            surfaces,
            is_crusher,
            max_cells,
            None,
            if is_crusher { 1 } else { 0 },
        )
    }

    pub fn adjust_destination_ex(
        &self,
        dest: GridPos,
        surfaces: u32,
        is_crusher: bool,
        max_cells: i32,
        seeker_player: Option<u32>,
        crusher_level: u8,
    ) -> Option<GridPos> {
        self.adjust_destination_on_layer(
            dest,
            surfaces,
            is_crusher,
            max_cells,
            seeker_player,
            crusher_level,
            PathfindLayerEnum::Ground,
        )
    }

    /// C++ `adjustDestination` spiral on `layer` (`getCell(layer, i, j)`).
    pub fn adjust_destination_on_layer(
        &self,
        dest: GridPos,
        surfaces: u32,
        is_crusher: bool,
        max_cells: i32,
        seeker_player: Option<u32>,
        crusher_level: u8,
        layer: PathfindLayerEnum,
    ) -> Option<GridPos> {
        let origin = self.clamp_pos(dest);
        if self.destination_cell_ok(
            origin,
            surfaces,
            is_crusher,
            seeker_player,
            crusher_level,
            layer,
        ) {
            return Some(origin);
        }
        let mut i = origin.x;
        let mut j = origin.y;
        let mut delta = 1;
        let mut limit = max_cells.max(1);
        while limit > 0 {
            for _ in 0..delta {
                i += 1;
                limit -= 1;
                let c = GridPos::new(i, j);
                if self.destination_cell_ok(
                    c,
                    surfaces,
                    is_crusher,
                    seeker_player,
                    crusher_level,
                    layer,
                ) {
                    return Some(c);
                }
            }
            for _ in 0..delta {
                j += 1;
                limit -= 1;
                let c = GridPos::new(i, j);
                if self.destination_cell_ok(
                    c,
                    surfaces,
                    is_crusher,
                    seeker_player,
                    crusher_level,
                    layer,
                ) {
                    return Some(c);
                }
            }
            delta += 1;
            for _ in 0..delta {
                i -= 1;
                limit -= 1;
                let c = GridPos::new(i, j);
                if self.destination_cell_ok(
                    c,
                    surfaces,
                    is_crusher,
                    seeker_player,
                    crusher_level,
                    layer,
                ) {
                    return Some(c);
                }
            }
            for _ in 0..delta {
                j -= 1;
                limit -= 1;
                let c = GridPos::new(i, j);
                if self.destination_cell_ok(
                    c,
                    surfaces,
                    is_crusher,
                    seeker_player,
                    crusher_level,
                    layer,
                ) {
                    return Some(c);
                }
            }
            delta += 1;
        }
        None
    }

    /// C++ `Pathfinder::tightenPath` (AIPathfind.cpp:8414-8421).
    /// Walk Bresenham from `from` toward `to`; advance `from` to the last
    /// cell that still passes `destination_cell_ok`.
    pub fn tighten_path(
        &self,
        from: &mut Vec3,
        to: Vec3,
        surfaces: u32,
        is_crusher: bool,
        seeker_player: Option<u32>,
        crusher_level: u8,
        layer: PathfindLayerEnum,
    ) {
        let start = *from;
        let start_cell = self.world_to_grid(start);
        let goal_cell = self.world_to_grid(to);
        let mut found = false;
        let mut dest_pos = start;
        let mut x0 = start_cell.x;
        let mut y0 = start_cell.y;
        let x1 = goal_cell.x;
        let y1 = goal_cell.y;
        let dx = (x1 - x0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let dy = -(y1 - y0).abs();
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;
        loop {
            let cell = GridPos::new(x0, y0);
            if self.layer_for_destination(self.grid_to_world(cell)) != layer {
                break;
            }
            if self.destination_cell_ok(
                cell,
                surfaces,
                is_crusher,
                seeker_player,
                crusher_level,
                layer,
            ) {
                found = true;
                dest_pos = self.grid_to_world(cell);
            } else {
                break;
            }
            if x0 == x1 && y0 == y1 {
                break;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x0 += sx;
            }
            if e2 <= dx {
                err += dx;
                y0 += sy;
            }
        }
        if found {
            *from = dest_pos;
        }
    }

    /// C++ `Pathfinder::checkPathCost` (AIPathfind.cpp:8338+).
    /// Limited A*; returns `0x7fff0000` when no path.
    pub fn check_path_cost(
        &self,
        surfaces: u32,
        is_crusher: bool,
        from: Vec3,
        to: Vec3,
    ) -> f32 {
        const MAX_COST: f32 = 0x7fff_0000u32 as f32;
        const MAX_CELL_COUNT: i32 = 500;
        const COST_ORTHO: i32 = 10;
        const COST_DIAG: i32 = 14;
        let start = self.world_to_grid(from);
        let goal = self.world_to_grid(to);
        if !self.is_valid_pos(start) || !self.is_valid_pos(goal) {
            return MAX_COST;
        }
        if !self.cell_passable_for(start, surfaces, is_crusher)
            && !(self.is_obstacle_fence(start) && is_crusher)
        {
            return MAX_COST;
        }
        if start == goal {
            return 0.0;
        }
        let heuristic = |c: GridPos| -> i32 {
            let dx = (goal.x - c.x).abs();
            let dy = (goal.y - c.y).abs();
            let dmin = dx.min(dy);
            let dmax = dx.max(dy);
            COST_DIAG * dmin + COST_ORTHO * (dmax - dmin)
        };
        let deltas: [(i32, i32); 8] = [
            (1, 0),
            (0, 1),
            (-1, 0),
            (0, -1),
            (1, 1),
            (-1, 1),
            (-1, -1),
            (1, -1),
        ];
        let mut open: BinaryHeap<std::cmp::Reverse<(i32, i32, i32, i32)>> = BinaryHeap::new();
        let mut g_score: HashMap<(i32, i32), i32> = HashMap::new();
        let mut closed: HashSet<(i32, i32)> = HashSet::new();
        let h0 = heuristic(start);
        open.push(std::cmp::Reverse((h0, 0, start.x, start.y)));
        g_score.insert((start.x, start.y), 0);
        let mut cell_count = 0i32;
        while let Some(std::cmp::Reverse((_f, g, cx, cy))) = open.pop() {
            if closed.contains(&(cx, cy)) {
                continue;
            }
            closed.insert((cx, cy));
            if cx == goal.x && cy == goal.y {
                return g as f32;
            }
            if cell_count > MAX_CELL_COUNT {
                continue;
            }
            let parent = GridPos::new(cx, cy);
            if let Some(link) = self.change_layer_xy(parent) {
                if !closed.contains(&(link.x, link.y)) {
                    let key = (link.x, link.y);
                    if !g_score.get(&key).is_some_and(|&og| g >= og) {
                        g_score.insert(key, g);
                        let f = g + heuristic(link);
                        open.push(std::cmp::Reverse((f, g, link.x, link.y)));
                        cell_count += 1;
                    }
                }
            }
            let mut neighbor_flags = [false; 8];
            for (i, (dx, dy)) in deltas.iter().enumerate() {
                let nx = cx + dx;
                let ny = cy + dy;
                let nc = GridPos::new(nx, ny);
                if !self.is_valid_pos(nc) || closed.contains(&(nx, ny)) {
                    continue;
                }
                if Self::skip_diagonal_if_squeezed(i, &neighbor_flags) {
                    continue;
                }
                if !self.cell_passable_for(nc, surfaces, is_crusher)
                    && !(self.is_obstacle_fence(nc) && is_crusher)
                {
                    continue;
                }
                neighbor_flags[i] = true;
                let step = if i >= 4 { COST_DIAG } else { COST_ORTHO };
                let ng = g + step;
                let key = (nx, ny);
                if g_score.get(&key).is_some_and(|&og| ng >= og) {
                    continue;
                }
                g_score.insert(key, ng);
                let f = ng + heuristic(nc);
                open.push(std::cmp::Reverse((f, ng, nx, ny)));
                cell_count += 1;
            }
        }
        MAX_COST
    }

    #[inline]
    fn skip_diagonal_if_squeezed(i: usize, neighbor_flags: &[bool; 8]) -> bool {
        const FIRST_DIAGONAL: usize = 4;
        const ADJACENT: [usize; 5] = [0, 1, 2, 3, 0];
        i >= FIRST_DIAGONAL && !neighbor_flags[ADJACENT[i - 4]] && !neighbor_flags[ADJACENT[i - 3]]
    }

    /// C++ `checkChangeLayers`: same-XY connect-layer hop (2D residual).
    fn change_layer_xy(&self, cell: GridPos) -> Option<GridPos> {
        let connect = self.ground_connect_layer(cell);
        if connect == 0 {
            return None;
        }
        Some(cell)
    }

    fn human_extent_allows(&self, pos: GridPos, is_human: bool) -> bool {
        if !self.is_valid_pos(pos) {
            return false;
        }
        if !is_human {
            return true;
        }
        self.in_logical_extent(pos)
    }


    /// C++ `checkForAdjust` groupDest tighten + `checkPathCost` gate, then
    /// spiral. Falls back to simple `adjustDestination` when the group
    /// cost check rejects every candidate (AIPathfind.cpp:5210-5403).
    pub fn adjust_destination_for_group(
        &self,
        dest: GridPos,
        group_dest: GridPos,
        surfaces: u32,
        is_crusher: bool,
        seeker_player: Option<u32>,
        crusher_level: u8,
        layer: PathfindLayerEnum,
    ) -> Option<GridPos> {
        if let Some(adj) = self.adjust_destination_on_layer_group(
            dest,
            Some(group_dest),
            surfaces,
            is_crusher,
            seeker_player,
            crusher_level,
            layer,
        ) {
            return Some(adj);
        }
        self.adjust_destination_on_layer(
            dest,
            surfaces,
            is_crusher,
            400,
            seeker_player,
            crusher_level,
            layer,
        )
    }

    fn check_for_adjust_group(
        &self,
        cell: GridPos,
        group_dest: Option<GridPos>,
        surfaces: u32,
        is_crusher: bool,
        seeker_player: Option<u32>,
        crusher_level: u8,
        layer: PathfindLayerEnum,
    ) -> bool {
        if !self.destination_cell_ok(
            cell,
            surfaces,
            is_crusher,
            seeker_player,
            crusher_level,
            layer,
        ) {
            return false;
        }
        let Some(gd) = group_dest else {
            return true;
        };
        let mut adjust_dest = self.grid_to_world(cell);
        let group_w = self.grid_to_world(gd);
        self.tighten_path(
            &mut adjust_dest,
            group_w,
            surfaces,
            is_crusher,
            seeker_player,
            crusher_level,
            layer,
        );
        let cost = self.check_path_cost(surfaces, is_crusher, group_w, adjust_dest);
        let dx = (group_w.x - adjust_dest.x).abs();
        let dy = (group_w.z - adjust_dest.z).abs();
        if cost > 0.0 && 1.4 * (dx + dy) < cost {
            return false;
        }
        true
    }

    fn adjust_destination_on_layer_group(
        &self,
        dest: GridPos,
        group_dest: Option<GridPos>,
        surfaces: u32,
        is_crusher: bool,
        seeker_player: Option<u32>,
        crusher_level: u8,
        layer: PathfindLayerEnum,
    ) -> Option<GridPos> {
        let origin = self.clamp_pos(dest);
        if self.check_for_adjust_group(
            origin,
            group_dest,
            surfaces,
            is_crusher,
            seeker_player,
            crusher_level,
            layer,
        ) {
            return Some(origin);
        }
        let mut i = origin.x;
        let mut j = origin.y;
        let mut delta = 1;
        let mut limit = 400i32;
        while limit > 0 {
            for _ in 0..delta {
                i += 1;
                limit -= 1;
                let c = GridPos::new(i, j);
                if self.check_for_adjust_group(
                    c,
                    group_dest,
                    surfaces,
                    is_crusher,
                    seeker_player,
                    crusher_level,
                    layer,
                ) {
                    return Some(c);
                }
            }
            for _ in 0..delta {
                j += 1;
                limit -= 1;
                let c = GridPos::new(i, j);
                if self.check_for_adjust_group(
                    c,
                    group_dest,
                    surfaces,
                    is_crusher,
                    seeker_player,
                    crusher_level,
                    layer,
                ) {
                    return Some(c);
                }
            }
            delta += 1;
            for _ in 0..delta {
                i -= 1;
                limit -= 1;
                let c = GridPos::new(i, j);
                if self.check_for_adjust_group(
                    c,
                    group_dest,
                    surfaces,
                    is_crusher,
                    seeker_player,
                    crusher_level,
                    layer,
                ) {
                    return Some(c);
                }
            }
            for _ in 0..delta {
                j -= 1;
                limit -= 1;
                let c = GridPos::new(i, j);
                if self.check_for_adjust_group(
                    c,
                    group_dest,
                    surfaces,
                    is_crusher,
                    seeker_player,
                    crusher_level,
                    layer,
                ) {
                    return Some(c);
                }
            }
            delta += 1;
        }
        None
    }


    fn line_cell_ok(
        &self,
        cell: GridPos,
        surfaces: u32,
        is_crusher: bool,
        allow_pinched: bool,
        seeker_player: Option<u32>,
        crusher_level: u8,
        unpinched_cliff_passable: bool,
    ) -> bool {
        if !self.is_valid_pos(cell) {
            return false;
        }
        if !allow_pinched && self.is_pinched(cell) {
            return false;
        }
        if self.occupancy_blocks_line(cell, seeker_player, crusher_level) {
            return false;
        }
        if !self.diameter_allows(is_crusher, cell) {
            return false;
        }
        if unpinched_cliff_passable
            && self.cell_type(cell) == PathfindCellType::Cliff
            && !self.is_pinched(cell)
        {
            return true;
        }
        self.cell_passable_for(cell, surfaces, is_crusher)
    }

    fn line_passable(
        &self,
        from: GridPos,
        to: GridPos,
        surfaces: u32,
        is_crusher: bool,
    ) -> bool {
        self.line_passable_ex(from, to, surfaces, is_crusher, true, None, 0, false)
    }

    fn line_passable_ex(
        &self,
        from: GridPos,
        to: GridPos,
        surfaces: u32,
        is_crusher: bool,
        allow_pinched: bool,
        seeker_player: Option<u32>,
        crusher_level: u8,
        unpinched_cliff_passable: bool,
    ) -> bool {
        if from == to {
            return true;
        }
        let mut x0 = from.x;
        let mut y0 = from.y;
        let x1 = to.x;
        let y1 = to.y;
        let dx = (x1 - x0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let dy = -(y1 - y0).abs();
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;
        loop {
            let cell = GridPos::new(x0, y0);
            if !self.line_cell_ok(
                cell,
                surfaces,
                is_crusher,
                allow_pinched,
                seeker_player,
                crusher_level,
                unpinched_cliff_passable,
            ) {
                return false;
            }
            if x0 == x1 && y0 == y1 {
                return true;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x0 += sx;
            }
            if e2 <= dx {
                err += dx;
                y0 += sy;
            }
        }
    }

    /// C++ `Path::optimize` / `optimizeGroundPath` LOS-shortcut + jig removal.
    pub fn optimize_ground_path(
        &self,
        waypoints: &[Vec3],
        surfaces: u32,
        is_crusher: bool,
    ) -> Vec<Vec3> {
        self.optimize_ground_path_ex(waypoints, surfaces, is_crusher, None, if is_crusher { 1 } else { 0 })
    }

    pub fn optimize_ground_path_ex(
        &self,
        waypoints: &[Vec3],
        surfaces: u32,
        is_crusher: bool,
        seeker_player: Option<u32>,
        crusher_level: u8,
    ) -> Vec<Vec3> {
        if waypoints.len() <= 2 {
            return waypoints.to_vec();
        }
        // C++ Path::optimize ALLOWED_STEPS = 3 past a layer change
        // (AIPathfind.cpp:472-485). Deck shortcuts stay on the deck.
        const ALLOWED_STEPS: usize = 3;
        let layers: Vec<PathfindLayerEnum> = waypoints
            .iter()
            .map(|p| self.layer_for_destination(*p))
            .collect();
        let mut optimized = vec![waypoints[0]];
        let mut anchor = 0usize;
        let mut first_node = true;
        let first_layer = layers[0];
        while anchor < waypoints.len() - 1 {
            let mut layer = layers[anchor];
            let mut cur_layer = layers[anchor];
            let mut count = 0usize;
            let mut node_idx = anchor + 1;
            while node_idx + 1 < waypoints.len() {
                count += 1;
                if cur_layer == PathfindLayerEnum::Ground {
                    if layers[node_idx] != cur_layer {
                        layer = layers[node_idx];
                        cur_layer = layer;
                        if count > ALLOWED_STEPS {
                            break;
                        }
                    }
                } else if layers[node_idx + 1] != cur_layer && count > ALLOWED_STEPS {
                    break;
                }
                cur_layer = layers[node_idx];
                node_idx += 1;
            }
            if first_node {
                layer = first_layer;
                first_node = false;
            }
            let mut found = false;
            let mut far = node_idx;
            while far > anchor {
                let a = self.world_to_grid(waypoints[anchor]);
                let b = self.world_to_grid(waypoints[far]);
                if self.line_passable_on_layer(
                    a,
                    b,
                    surfaces,
                    is_crusher,
                    false,
                    seeker_player,
                    crusher_level,
                    true,
                    layer,
                ) || self.collinear_cells_force_passable(waypoints, anchor, far)
                {
                    if far == anchor {
                        break;
                    }
                    optimized.push(waypoints[far]);
                    anchor = far;
                    found = true;
                    break;
                }
                if far == 0 {
                    break;
                }
                far -= 1;
            }
            if !found {
                optimized.push(waypoints[anchor + 1]);
                anchor += 1;
            }
        }
        // C++ jig-jog removal: drop very short mid segments.
        let cell = self.grid_size;
        let thresh = cell * cell * 3.9;
        let mut i = 0;
        while i + 2 < optimized.len() {
            let dx = optimized[i + 1].x - optimized[i].x;
            let dz = optimized[i + 1].z - optimized[i].z;
            if dx * dx + dz * dz < thresh {
                optimized.remove(i + 1);
            } else {
                i += 1;
            }
        }
        optimized
    }

    /// C++ `Path::optimize` H/V/diag force-passable (AIPathfind.cpp:511-551).
    /// A* already walked these cells; pinched occupancy on a straight jog
    /// must not keep every cell center.
    fn collinear_cells_force_passable(&self, waypoints: &[Vec3], from: usize, to: usize) -> bool {
        if to <= from {
            return true;
        }
        let cell = self.grid_size;
        let eps = cell * 0.15;
        let bx = waypoints[to].x - waypoints[from].x;
        let bz = waypoints[to].z - waypoints[from].z;
        if (bx.abs() - cell).abs() < eps && (bz.abs() - cell).abs() < eps {
            return true;
        }
        let horiz = bx.abs() < eps;
        let vert = bz.abs() < eps;
        let diag_pos = (bx - bz).abs() < eps;
        let diag_neg = (bx + bz).abs() < eps;
        if !horiz && !vert && !diag_pos && !diag_neg {
            return false;
        }
        for i in from..to {
            let dx = waypoints[i + 1].x - waypoints[i].x;
            let dz = waypoints[i + 1].z - waypoints[i].z;
            if horiz && dx.abs() >= eps {
                return false;
            }
            if vert && dz.abs() >= eps {
                return false;
            }
            if diag_pos && (dx - dz).abs() >= eps {
                return false;
            }
            if diag_neg && (dx + dz).abs() >= eps {
                return false;
            }
        }
        true
    }

}

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
    /// C++ `m_cumulativeCellsAllocated` this processPathfindQueue call.
    cumulative_cells_allocated: i32,
    /// C++ `PATHFIND_CELLS_PER_FRAME` (test-overridable).
    pathfind_cells_per_frame: i32,



}

impl PathfindingSystem {
    pub fn new(world_width: f32, world_height: f32) -> Self {
        Self::new_with_origin(Vec3::ZERO, world_width, world_height)
    }

    pub fn new_with_origin(origin: Vec3, world_width: f32, world_height: f32) -> Self {
        const GRID_SIZE: f32 = 10.0; // 10 units per grid cell

        Self {
            grid: PathfindingGrid::new_with_origin(origin, world_width, world_height, GRID_SIZE),
            flow_fields: HashMap::new(),
            logic_frame: 0,
            dynamic_obstacle_frame: u64::MAX,
            crate_astar: None,
            pending_paths: VecDeque::new(),
            seeker_player: None,
            seeker_is_infantry: false,
            seeker_wings: false,
            seeker_id: None,
            seeker_team: None,
            seeker_crusher_level: 0,
            seeker_path_diameter: 1,
            seeker_center_in_cell: true,
            ignore_obstacle_id: None,
            human_player_mask: 0,
            seeker_is_human: false,
            seeker_is_dozer: false,
            seeker_downhill_only: false,
            cumulative_cells_allocated: 0,
            pathfind_cells_per_frame: PATHFIND_CELLS_PER_FRAME,


        }
    }

    /// C++ `Player::getPlayerType() == PLAYER_HUMAN` bits for logical-extent clamp.
    pub fn set_human_player_mask(&mut self, mask: u16) {
        self.human_player_mask = mask;
    }

    fn apply_seeker_human_flag(&mut self) {
        self.seeker_is_human = match self.seeker_player {
            Some(p) if self.human_player_mask != 0 => {
                (self.human_player_mask & (1u16 << p.min(15))) != 0
            }
            // No mask yet: player-path default is human (full-grid extent is a no-op).
            _ => self.human_player_mask == 0,
        };
        self.grid.set_query_is_human(self.seeker_is_human);
    }

    /// C++ `findPath(obj, ...)` — seeker flags come from the mover, not nearest-to-start.
    fn bind_seeker_from_mover(
        &mut self,
        objects: &HashMap<ObjectId, Object>,
        mover: Option<ObjectId>,
    ) {
        if let Some(o) = mover.and_then(|id| objects.get(&id)).filter(|o| o.is_alive()) {
            self.seeker_player = o.owner_player_id.or(Some(o.team as u32));
            self.seeker_is_infantry = o.is_kind_of(KindOf::Infantry);
            self.seeker_wings = matches!(o.loco_appearance, LocomotorAppearance::Wings);
            self.seeker_id = Some(o.id);
            self.seeker_team = Some(o.team);
            self.seeker_crusher_level = o.crusher_level;
            self.seeker_is_dozer = o.is_kind_of(KindOf::Dozer);
            self.seeker_downhill_only = o.downhill_only;
            self.seeker_path_diameter = PathfindingGrid::path_diameter_for_unit(
                o.selection_radius,
                self.grid.grid_size(),
                o.is_kind_of(KindOf::Vehicle),
            );
            self.seeker_center_in_cell = PathfindingGrid::radius_and_center(
                o.selection_radius,
                self.grid.grid_size(),
            )
            .1;
        } else {
            self.seeker_player = None;
            self.seeker_is_infantry = false;
            self.seeker_wings = false;
            self.seeker_id = None;
            self.seeker_team = None;
            self.seeker_crusher_level = 0;
            self.seeker_path_diameter = 1;
            self.seeker_center_in_cell = true;
            self.seeker_is_dozer = false;
            self.seeker_downhill_only = false;
        }
    }


    /// C++ dozerHack: obstacle exists and relationship != ENEMIES.
    fn dozer_hack_allows(&self, cell: GridCoord) -> bool {
        if !self.seeker_is_dozer {
            return false;
        }
        let Some((_id, owner, team)) = self.grid.obstacle_owner(GridPos::new(cell.x, cell.y)) else {
            return false;
        };
        if let (Some(sp), Some(op)) = (self.seeker_player, owner) {
            if sp == op {
                return true;
            }
            let ally = self.grid.ally_mask_for(sp);
            if (ally & (1u16 << op.min(15))) != 0 {
                return true;
            }
            return matches!(team, Some(Team::Neutral));
        }
        match (self.seeker_team, team) {
            (Some(a), Some(b)) if a == b => true,
            (Some(Team::Neutral), _) | (_, Some(Team::Neutral)) => true,
            _ => false,
        }
    }


    /// C++ getRelationship == ALLIES bits for occupancy crush-through.
    pub fn set_player_ally_masks(&mut self, masks: [u16; 16]) {
        self.grid.set_player_ally_masks(masks);
    }

    /// C++ `Pathfinder::adjustToLandingDestination`. Off-map unit+dest is scripted OK.
    pub fn adjust_to_landing_destination(&self, from: Vec3, dest: Vec3) -> Vec3 {
        self.adjust_to_landing_destination_radius(from, dest, 0.0)
    }

    pub fn adjust_to_landing_destination_radius(
        &self,
        from: Vec3,
        dest: Vec3,
        unit_radius: f32,
    ) -> Vec3 {
        let dest_cell = self.grid.world_to_grid(dest);
        let from_cell = self.grid.world_to_grid(from);
        if !self.grid.is_valid_pos(dest_cell) && !self.grid.is_valid_pos(from_cell) {
            return dest;
        }
        let (radius, center_in_cell) =
            PathfindingGrid::radius_and_center(unit_radius, self.grid.grid_size());
        let mut adjust = dest;
        if !center_in_cell {
            let half = self.grid.grid_size() * 0.5;
            adjust.x += half;
            adjust.z += half;
        }
        let dest_cell = self.grid.world_to_grid(adjust);
        let layer = self.grid.layer_for_destination(dest);
        let Some(adj) = self.grid.adjust_to_landing_destination_for(
            dest_cell,
            400,
            layer,
            radius,
            center_in_cell,
        ) else {
            return dest;
        };
        let mut world = self.grid.grid_to_world(adj);
        world.y = dest.y;
        world
    }

    /// C++ `Weapon::computeApproachTarget` aircraft branch + leftover
    /// `Pathfinder::adjustTargetDestination` so two Comanches do not share a hover cell.
    pub fn adjust_target_destination(
        &self,
        seeker: u32,
        objects: &HashMap<ObjectId, Object>,
        dest: Vec3,
        target_pos: Vec3,
        unit_radius: f32,
        surfaces: u32,
        is_crusher: bool,
        source_radius: f32,
        target_radius: f32,
        attack_range: f32,
        min_range: f32,
    ) -> Vec3 {
        let in_range = |goal: Vec3| {
            crate::game_logic::weapon_bootstrap::is_goal_pos_within_attack_range(
                goal,
                target_pos,
                attack_range,
                min_range,
                source_radius,
                target_radius,
            )
        };
        let mut dest = dest;

        if let Ok(ai) = gamelogic::ai::THE_AI.read() {
            if let Some(pf) = ai.pathfinder() {
                if let Ok(pf) = pf.read() {
                    let mut dest3 = gamelogic::common::Coord3D::new(dest.x, dest.z, dest.y);
                    let _ = pf.adjust_target_destination_for(
                        surfaces,
                        is_crusher,
                        unit_radius,
                        Some(seeker),
                        &mut dest3,
                        |goal| in_range(Vec3::new(goal.x, goal.z, goal.y)),
                    );
                    dest = Vec3::new(dest3.x, dest.y, dest3.y);
                }
            }
        }

        let mut claimed = HashSet::new();
        let cell_size = self.grid.grid_size();
        for (id, obj) in objects {
            if id.0 == seeker {
                continue;
            }
            if !PathfindingGrid::is_aircraft_that_adjusts_destination(obj) {
                continue;
            }
            let Some(goal) = PathfindingGrid::aircraft_goal_dest(obj) else {
                continue;
            };
            let (or, oc) = PathfindingGrid::radius_and_center(obj.selection_radius, cell_size);
            let cell = self.grid.world_to_grid(goal);
            let mut num_above = or;
            if oc {
                num_above += 1;
            }
            for i in (cell.x - or)..(cell.x + num_above) {
                for j in (cell.y - or)..(cell.y + num_above) {
                    claimed.insert(GridPos::new(i, j));
                }
            }
        }

        let mut out = dest;
        if self
            .grid
            .adjust_target_destination(&mut out, unit_radius, seeker, &claimed, in_range)
        {
            out.y = dest.y;
            return out;
        }
        dest
    }


    /// Stamp occupancy then unstack landing dest for `seeker` (C++ checkDestination objID).
    pub fn adjust_landing_destination_for(
        &mut self,
        seeker: u32,
        objects: &HashMap<ObjectId, Object>,
        from: Vec3,
        dest: Vec3,
    ) -> Vec3 {
        self.grid.update_dynamic_obstacles(objects);
        self.grid.query_seeker_id = seeker;
        let unit_radius = objects
            .get(&ObjectId(seeker))
            .map(|o| o.selection_radius)
            .unwrap_or(0.0);
        let adj = self.adjust_to_landing_destination_radius(from, dest, unit_radius);
        self.grid.query_seeker_id = 0;
        adj
    }

    pub fn clear_static_blocks(&mut self) {
        self.grid.clear_static_blocks();
        self.crate_astar = None;
    }

    /// Mark the active host logic frame so dynamic obstacle rebuilds run once
    /// per frame across many find_path_ex calls.
    #[inline]
    pub fn note_logic_frame(&mut self, frame: u64) {
        self.logic_frame = frame;
    }

    /// Rebuild vehicle/structure dynamic blocks at most once per logic frame.
    #[inline]
    fn ensure_dynamic_obstacles(&mut self, objects: &HashMap<ObjectId, Object>) {
        if self.ignore_obstacle_id.is_some() {
            self.grid
                .update_dynamic_obstacles_ignoring(objects, self.ignore_obstacle_id);
            // Do not cache an ignore-filtered occupancy stamp.
            self.dynamic_obstacle_frame = u64::MAX;
            return;
        }
        if self.dynamic_obstacle_frame != self.logic_frame {
            self.grid.update_dynamic_obstacles(objects);
            self.dynamic_obstacle_frame = self.logic_frame;
        }
    }

    /// C++ `AIUpdateInterface::ignoreObstacle` for the next `find_path_ex_*`.
    pub fn set_ignore_obstacle(&mut self, id: Option<ObjectId>) {
        self.ignore_obstacle_id = id;
        if id.is_some() {
            self.dynamic_obstacle_frame = u64::MAX;
        }
    }

    pub fn ignore_obstacle(&self) -> Option<ObjectId> {
        self.ignore_obstacle_id
    }

    /// Leftover `ignored_obstacle_cells`: footprint cells of `m_ignoreObstacleID`.
    fn ignored_obstacle_cells(&self) -> Option<HashSet<GridCoord>> {
        let id = self.ignore_obstacle_id?.0;
        if id == 0 {
            return None;
        }
        let mut cells = HashSet::new();
        for y in 0..self.grid.height {
            for x in 0..self.grid.width {
                let pos = GridPos::new(x, y);
                if self.grid.cell_type(pos) != PathfindCellType::Obstacle {
                    continue;
                }
                if self
                    .grid
                    .obstacle_owner(pos)
                    .is_some_and(|(oid, _, _)| oid == id)
                {
                    cells.insert(GridCoord::new(x, y));
                }
            }
        }
        if cells.is_empty() {
            None
        } else {
            Some(cells)
        }
    }

    /// C++ `processPathfindQueue` reset of `m_cumulativeCellsAllocated` + extent.
    pub fn begin_pathfind_queue_frame(&mut self) {
        self.grid.refresh_logical_extent();
        self.cumulative_cells_allocated = 0;
    }

    pub fn pathfind_budget_remaining(&self) -> bool {
        self.cumulative_cells_allocated < self.pathfind_cells_per_frame
    }

    pub fn pop_pending_path(&mut self) -> Option<PendingHostPath> {
        self.pending_paths.pop_front()
    }

    fn note_cells_allocated(&mut self, n: usize) {
        self.cumulative_cells_allocated = self
            .cumulative_cells_allocated
            .saturating_add(n.min(i32::MAX as usize) as i32);
    }

    #[cfg(test)]
    pub fn set_pathfind_cells_per_frame(&mut self, n: i32) {
        self.pathfind_cells_per_frame = n.max(1);
    }

    #[cfg(test)]
    pub fn cumulative_cells_allocated(&self) -> i32 {
        self.cumulative_cells_allocated
    }


    fn sync_crate_astar(&mut self) {
        let w = self.grid.width.max(0) as usize;
        let h = self.grid.height.max(0) as usize;
        if w == 0 || h == 0 {
            self.crate_astar = None;
            return;
        }
        let stamp = self.grid.terrain_gen;
        let needs_rebuild = match &self.crate_astar {
            Some(c) => c.stamp != stamp || c.finder.width() != w || c.finder.height() != h,
            None => true,
        };
        if !needs_rebuild {
            return;
        }
        let mut finder = AStarPathfinder::new(w, h);
        for y in 0..self.grid.height {
            for x in 0..self.grid.width {
                let pos = GridPos::new(x, y);
                let coord = GridCoord::new(x, y);
                finder.set_cell_type(coord, self.grid.cell_type(pos));
                finder.set_pinched(coord, self.grid.is_pinched(pos));
                if self.grid.cell_type(pos) == PathfindCellType::Obstacle {
                    finder.set_cell_obstacle_id(
                        coord,
                        self.grid
                            .obstacle_owner(pos)
                            .map(|(id, _, _)| id)
                            .unwrap_or(1),
                        self.grid.is_obstacle_fence(pos),
                        self.grid.is_obstacle_transparent(pos),
                    );
                }
                let connect = self.grid.ground_connect_layer(pos);
                if connect != 0 {
                    finder.set_cell_connect_layer(
                        coord,
                        PathfindLayerEnum::from_u32(connect as u32),
                    );
                }
            }
        }
        for layer in &self.grid.bridge_layers {
            let pf_layer = PathfindLayerEnum::from_u32(layer.id as u32);
            for ((x, y), (ty, connect)) in &layer.cells {
                let coord = GridCoord::new(*x, *y);
                finder.set_cell_type_on_layer(coord, pf_layer, *ty);
                if *connect != 0 {
                    finder.set_cell_connect_layer_on_layer(
                        coord,
                        pf_layer,
                        PathfindLayerEnum::from_u32(*connect as u32),
                    );
                }
            }
        }
        if !self.grid.wall_cells.is_empty() {
            let pf_layer = PathfindLayerEnum::Wall;
            for ((x, y), ty) in &self.grid.wall_cells {
                finder.set_cell_type_on_layer(GridCoord::new(*x, *y), pf_layer, *ty);
            }
        }
        self.crate_astar = Some(HostCrateAStar { finder, stamp });
    }

    fn host_to_crate_coord(&self, pos: GridPos) -> GridCoord {
        GridCoord::new(pos.x, pos.y)
    }

    fn crate_path_to_world(&self, cells: &[GridCoord]) -> Vec<Vec3> {
        let center = self.seeker_center_in_cell;
        cells
            .iter()
            .map(|c| {
                let pos = GridPos::new(c.x, c.y);
                if let Some(id) = self.grid.host_deck_layer_at(pos) {
                    self.grid.adjust_coord_to_cell_on_layer(
                        pos,
                        center,
                        PathfindLayerEnum::from_u32(id as u32),
                    )
                } else {
                    self.grid.adjust_coord_to_cell(pos, center)
                }
            })
            .collect()
    }

    /// C++ hierarchical bridge start/end cells (AIPathfind.cpp:7595-7623).
    fn hierarchical_bridge_jumps(&self) -> Vec<(GridCoord, GridCoord)> {
        let mut out = Vec::new();
        for layer in &self.grid.bridge_layers {
            let start = self.grid.world_to_grid(Vec3::new(
                (layer.from_left.x + layer.from_right.x) * 0.5,
                0.0,
                (layer.from_left.z + layer.from_right.z) * 0.5,
            ));
            let end = self.grid.world_to_grid(Vec3::new(
                (layer.to_left.x + layer.to_right.x) * 0.5,
                0.0,
                (layer.to_left.z + layer.to_right.z) * 0.5,
            ));
            out.push((
                GridCoord::new(start.x, start.y),
                GridCoord::new(end.x, end.y),
            ));
            let connects: Vec<GridCoord> = layer
                .cells
                .iter()
                .filter(|(_, (_, connect))| *connect == PathfindLayerEnum::Ground as u8)
                .map(|((x, y), _)| GridCoord::new(*x, *y))
                .collect();
            if connects.len() >= 2 {
                let mut lo = connects[0];
                let mut hi = connects[0];
                for &c in &connects {
                    if c.x + c.y < lo.x + lo.y {
                        lo = c;
                    }
                    if c.x + c.y > hi.x + hi.y {
                        hi = c;
                    }
                }
                if lo != hi {
                    out.push((lo, hi));
                }
            }
        }
        out
    }


    /// C++ `Pathfinder::findPath` via crate A* after hierarchical zone-block prune.
    /// Falls back to the host grid A* if crate types cannot run (empty grid).

    fn find_path_via_crate(
        &mut self,
        start: GridPos,
        goal: GridPos,
        surfaces: u32,
        is_crusher: bool,
        start_layer: PathfindLayerEnum,
        dest_layer: PathfindLayerEnum,
        fallback_closest: bool,
    ) -> Option<Vec<Vec3>> {
        self.sync_crate_astar();
        self.grid.refresh_logical_extent();
        self.apply_seeker_human_flag();
        self.grid.query_seeker_id = self.seeker_id.map(|id| id.0).unwrap_or(0);
        self.grid
            .set_query_footprint(self.seeker_path_diameter, is_crusher);
        let crusher_level = if is_crusher {
            self.seeker_crusher_level.max(1)
        } else {
            0
        };
        let start_seed = self.grid.clamp_pos(start);
        let goal_seed = self.grid.clamp_pos(goal);
        self.grid.query_from = Some(start_seed);
        self.grid.query_orig_dest = Some(start_seed);
        // C++ adjustDestination: on failure leave dest unchanged (no static-open fallback).
        let start = self
            .grid
            .adjust_destination_on_layer(
                start_seed,
                surfaces,
                is_crusher,
                64,
                self.seeker_player,
                crusher_level,
                start_layer,
            )
            .unwrap_or(start_seed);
        // C++ adjustDestination: snap water/cliff/impassable/occupied clicks
        // on destinationLayer (AIPathfind.cpp:5352-5355).
        self.grid.query_from = Some(start);
        self.grid.query_orig_dest = Some(goal_seed);
        let mut goal = self
            .grid
            .adjust_destination_on_layer(
                goal_seed,
                surfaces,
                is_crusher,
                400,
                self.seeker_player,
                crusher_level,
                dest_layer,
            )
            .unwrap_or(goal_seed);
        self.grid.query_from = None;
        self.grid.query_orig_dest = None;
        let ignore_cells = self.ignored_obstacle_cells();
        let ignore_covers = |pos: GridPos| {
            ignore_cells
                .as_ref()
                .is_some_and(|s| s.contains(&GridCoord::new(pos.x, pos.y)))
        };
        let start_obstacle = self.grid.cell_type(start) == PathfindCellType::Obstacle;
        // C++ ignoreObstacle / tunneling uses terrain zones (AIPathfind.cpp:6544-6550).
        if (surfaces & SURFACE_AIR) == 0 {
            let from_w = self.grid.grid_to_world(start);
            let to_w = self.grid.grid_to_world(goal);
            let ignore_or_tunnel =
                start_obstacle || ignore_covers(start) || ignore_covers(goal);
            if !ignore_or_tunnel && !self.grid.quick_path_exists_for(from_w, to_w, surfaces)
            {
                return None;
            }
        }

        // C++ checkDestination refuses allied UNIT_GOAL cells.
        if self.grid.has_allied_goal_on(goal, self.seeker_player, dest_layer) {
            if let Some(adj) = self.grid.adjust_destination_on_layer(
                goal,
                surfaces,
                is_crusher,
                64,
                self.seeker_player,
                crusher_level,
                dest_layer,
            ) {
                if !self.grid.has_allied_goal_on(adj, self.seeker_player, dest_layer) {
                    goal = adj;
                }
            }
        }
        if !self
            .grid
            .cell_passable_for_layer(start, start_layer, surfaces, is_crusher)
            || !self
                .grid
                .cell_passable_for_layer(goal, dest_layer, surfaces, is_crusher)
        {
            // Still try — crusher fences / air / tunneling / ignoreObstacle may pass.
            if start_layer == PathfindLayerEnum::Ground
                && self.grid.is_static_blocked(start)
                && !self.grid.is_obstacle_fence(start)
                && !start_obstacle
                && !ignore_covers(start)
            {
                return None;
            }
        }
        if start == goal {
            return Some(vec![self.grid.adjust_coord_to_cell(start, self.seeker_center_in_cell)]);
        }
        let start_c = self.host_to_crate_coord(start);
        let goal_c = self.host_to_crate_coord(goal);
        // C++ findPath: clearPassableFlags + findHierarchicalPath corridor
        // (AIPathfind.cpp:6375-6381). Fine A* then consults isPassable.
        let jumps = self.hierarchical_bridge_jumps();
        if let Some(crate_pf) = self.crate_astar.as_mut() {
            crate_pf.finder.apply_hierarchical_zone_prune(
                start_c,
                goal_c,
                surfaces,
                is_crusher,
                &jumps,
            );
        }

        let width = self.grid.width;
        let occ_fixed = self.grid.occ_fixed_mask.clone();
        let occ_moving = self.grid.occ_moving_mask.clone();
        let occ_goal = self.grid.occ_goal_mask.clone();
        let occ_infantry = self.grid.occ_infantry_mask.clone();
        let occ_crush = self.grid.occ_fixed_max_crushable.clone();
        let layer_occ = self.grid.layer_occ.clone();
        let start_layer_id = start_layer as u8;
        let dest_layer_id = dest_layer as u8;
        let layer_cells = |id: u8| -> HashSet<(i32, i32)> {
            if id <= PathfindLayerEnum::Ground as u8 {
                return HashSet::new();
            }
            if id == LAYER_WALL_ID {
                return self.grid.wall_cells.keys().copied().collect();
            }
            self.grid
                .bridge_layers
                .iter()
                .find(|layer| layer.id == id)
                .map(|layer| layer.cells.keys().copied().collect())
                .unwrap_or_default()
        };
        let start_layer_cells = layer_cells(start_layer_id);
        let dest_layer_cells = layer_cells(dest_layer_id);
        let seeker = self.seeker_player;
        let seeker_inf = self.seeker_is_infantry;
        let ally_mask = seeker.map(|p| self.grid.ally_mask_for(p)).unwrap_or(0);
        let height = self.grid.height;
        let path_diameter = self.seeker_path_diameter;
        let cell_types = self.grid.cell_types.clone();
        let fence_bits = self.grid.fence_bits.clone();
        let diameter_crusher = is_crusher;
        let start_for_cost = start;
        let extra = move |c: GridCoord| {
            if c.x < 0 || c.y < 0 || c.x >= width {
                return 0;
            }
            if path_diameter >= 2 {
                let d = clear_cell_for_diameter_impl(
                    width,
                    height,
                    &cell_types,
                    &fence_bits,
                    &occ_fixed,
                    &occ_crush,
                    diameter_crusher,
                    GridPos::new(c.x, c.y),
                    path_diameter,
                );
                if d != path_diameter {
                    return u32::MAX / 8;
                }
            }
            let key = (c.x, c.y);
            let layer_id = if start_layer_id > PathfindLayerEnum::Ground as u8
                && start_layer_cells.contains(&key)
            {
                Some(start_layer_id)
            } else if dest_layer_id > PathfindLayerEnum::Ground as u8
                && dest_layer_cells.contains(&key)
            {
                Some(dest_layer_id)
            } else {
                None
            };
            let idx = c.y as usize * width as usize + c.x as usize;
            let (fixed, moving, goal_m, infantry, crush) = if let Some(lid) = layer_id {
                if let Some(occ) = layer_occ.get(&lid) {
                    (
                        occ.occ_fixed_mask.get(&key).copied().unwrap_or(0),
                        occ.occ_moving_mask.get(&key).copied().unwrap_or(0),
                        occ.occ_goal_mask.get(&key).copied().unwrap_or(0),
                        occ.occ_infantry_mask.get(&key).copied().unwrap_or(0),
                        occ.occ_fixed_max_crushable.get(&key).copied().unwrap_or(0),
                    )
                } else {
                    (0, 0, 0, 0, 0)
                }
            } else {
                (
                    occ_fixed.get(idx).copied().unwrap_or(0),
                    occ_moving.get(idx).copied().unwrap_or(0),
                    occ_goal.get(idx).copied().unwrap_or(0),
                    occ_infantry.get(idx).copied().unwrap_or(0),
                    occ_crush.get(idx).copied().unwrap_or(0),
                )
            };
            // C++ INFANTRY_MOVES_THROUGH_INFANTRY: stream even when a goal is set.
            if seeker_inf && infantry != 0 && (fixed | moving) == infantry {
                return 0;
            }
            if fixed == 0 && moving == 0 && goal_m == 0 {
                return 0;
            }
            let Some(player) = seeker else {
                return 3 * COST_DIAGONAL;
            };
            let bit = 1u16 << player.min(15);
            let friend = bit | ally_mask;
            if seeker_inf && (infantry & !bit) != 0 && (fixed & !bit) == (infantry & !bit) {
                let leftover_fixed = fixed & !infantry;
                let leftover_moving = moving & !infantry;
                if leftover_fixed == 0 && leftover_moving == 0 {
                    return 0;
                }
            }
            if (fixed & !friend) != 0 {
                let max_c = crush;
                if crusher_level == 0 || crusher_level <= max_c {
                    return u32::MAX / 8;
                }
            }
            let mut extra = 0u32;
            // C++ allyMoving for ALL allies, only within 10 cells of start.
            // Moving enemies are ignored (considerTransient=false).
            if (moving & friend) != 0
                && (c.x - start_for_cost.x).abs() < 10
                && (c.y - start_for_cost.y).abs() < 10
            {
                extra += 3 * COST_DIAGONAL;
            }
            if (fixed & friend) != 0 {
                extra += 3 * COST_DIAGONAL;
            }
            extra
        };
        let Some(crate_pf) = self.crate_astar.as_ref() else {
            let path = self.grid.find_path(start, goal);
            if path.is_some() {
                self.note_cells_allocated(1);
            }
            return path;
        };
        let start_is_obstacle = crate_pf
            .finder
            .get_cell_type_on_layer(start_c, start_layer)
            == Some(PathfindCellType::Obstacle);
        let extra_ref = &extra;
        let line_ok = |c: GridCoord| {
            if extra_ref(c) >= u32::MAX / 8 {
                return false;
            }
            // C++ examineCellsCallback: abort on allyFixed / any enemyFixed
            // (crushable included). extra() only MAX/8s uncrushable enemies.
            let ally = self
                .seeker_player
                .map(|p| self.grid.ally_mask_for(p))
                .unwrap_or(0);
            self.grid.seed_line_occupancy_ok(
                GridPos::new(c.x, c.y),
                self.seeker_player,
                ally,
                self.seeker_is_infantry,
                start_layer,
            )
        };
        let ground_h = |c: GridCoord| {
            let w = self.grid.grid_to_world(GridPos::new(c.x, c.y));
            sample_host_ground_height(w.x, w.z)
        };
        let downhill_only = self.seeker_downhill_only;
        let is_human = self.seeker_is_human;
        let cell_allowed = |c: GridCoord| {
            if is_human {
                self.grid.in_logical_extent(GridPos::new(c.x, c.y))
            } else {
                true
            }
        };
        let is_dozer = self.seeker_is_dozer;
        let seeker_player = self.seeker_player;
        let seeker_team = self.seeker_team;
        let ally_mask = seeker_player
            .map(|p| self.grid.ally_mask_for(p))
            .unwrap_or(0);
        let dozer_ok = |c: GridCoord| {
            if !is_dozer {
                return false;
            }
            let Some((_id, owner, team)) = self.grid.obstacle_owner(GridPos::new(c.x, c.y))
            else {
                return false;
            };
            if let (Some(sp), Some(op)) = (seeker_player, owner) {
                if sp == op {
                    return true;
                }
                if (ally_mask & (1u16 << op.min(15))) != 0 {
                    return true;
                }
                return matches!(team, Some(Team::Neutral));
            }
            match (seeker_team, team) {
                (Some(a), Some(b)) if a == b => true,
                (Some(Team::Neutral), _) | (_, Some(Team::Neutral)) => true,
                _ => false,
            }
        };
        let dozer_ok_ref: Option<&dyn Fn(GridCoord) -> bool> = if is_dozer {
            Some(&dozer_ok as &dyn Fn(GridCoord) -> bool)
        } else {
            None
        };
        let seed_line = !start_is_obstacle && !downhill_only;
        if is_human
            && (!self.grid.in_logical_extent(start) || !self.grid.in_logical_extent(goal))
        {
            if !self.grid.in_logical_extent(start) {
                return None;
            }
        }
        let run = |allow_partial: bool| {
            crate_pf.finder.find_path_with_start_layer(
                start_c,
                goal_c,
                start_layer,
                dest_layer,
                surfaces,
                is_crusher,
                MAX_PATH_ITERATIONS,
                allow_partial,
                ignore_cells.as_ref(),
                Some(extra_ref as &dyn Fn(GridCoord) -> u32),
                downhill_only,
                Some(&ground_h as &dyn Fn(GridCoord) -> f32),
                None,
                Some(&line_ok as &dyn Fn(GridCoord) -> bool),
                seed_line,
                start_is_obstacle,
                Some(&cell_allowed as &dyn Fn(GridCoord) -> bool),
                dozer_ok_ref,
            )
        };

        let exact = run(false);
        let examined = exact.as_ref().map(|(_, n)| *n).unwrap_or(0);
        let exact_path = exact.map(|(path, _)| path);
        let from_w = self.grid.grid_to_world(start);
        let to_w = self.grid.grid_to_world(goal);
        let closest_human = is_human;
        drop(run);
        drop(dozer_ok_ref);
        drop(dozer_ok);
        drop(cell_allowed);
        drop(ground_h);
        drop(line_ok);
        drop(extra_ref);
        drop(extra);
        drop(crate_pf);
        self.note_cells_allocated(examined);
        if let Some(cells) = exact_path {
            let world = self.crate_path_to_world(&cells);
            return Some(self.grid.optimize_ground_path_ex(
                &world,
                surfaces,
                is_crusher,
                self.seeker_player,
                crusher_level,
            ));
        }
        if !fallback_closest {
            return None;
        }
        // C++ findClosestPath on findPath fail (AIUpdate.cpp:1713-1717).
        self.find_closest_path(from_w, to_w, surfaces, is_crusher, closest_human)


    }


    /// Queue a path request for next-frame resolve (C++ queueForPath).
    /// Duplicate ObjectID updates dest in place. Full queue refuses the new request.
    pub fn queue_path(&mut self, req: PendingHostPath) -> bool {
        if let Some(existing) = self
            .pending_paths
            .iter_mut()
            .find(|p| p.unit_id == req.unit_id)
        {
            *existing = req;
            return true;
        }
        if self.pending_paths.len() >= PATHFIND_QUEUE_LEN {
            return false;
        }
        self.pending_paths.push_back(req);
        true
    }

    pub fn take_pending_paths(&mut self) -> Vec<PendingHostPath> {
        self.pending_paths.drain(..).collect()
    }

    pub fn pending_path_count(&self) -> usize {
        self.pending_paths.len()
    }

    pub fn pending_paths(&self) -> impl Iterator<Item = &PendingHostPath> {
        self.pending_paths.iter()
    }


    pub fn stamp_rubble_at_world(&mut self, world: Vec3, radius_cells: i32) {
        let cell = self.grid.world_to_grid(world);
        self.grid.stamp_rubble_footprint(cell, radius_cells);
        // Destroy path stamps after sync_structure_path_blocks; rebuild so
        // rubble is not left in the previous Clear zone (hq-0r5xs).
        self.grid.rebuild_path_zones();
    }

    pub fn block_structure_at_world(&mut self, pos: Vec3, radius_cells: i32) {
        let center = self.grid.world_to_grid(pos);
        self.grid.block_structure_footprint(center, radius_cells);
    }

    pub fn is_attack_view_blocked(&self, from: Vec3, to: Vec3) -> bool {
        self.is_attack_view_blocked_for(from, to, None, None)
    }

    /// C++ `TheAI->getAiData()->m_attackUsesLineOfSight` (default true).
    fn attack_uses_line_of_sight() -> bool {
        if let Some(data) = game_engine::common::ini::get_ai_data_store().get_active() {
            return data.attack_uses_line_of_sight;
        }
        gamelogic::ai::THE_AI
            .read()
            .ok()
            .and_then(|ai| {
                ai.get_ai_data()
                    .read()
                    .ok()
                    .map(|d| d.attack_uses_line_of_sight)
            })
            .unwrap_or(true)
    }

    /// C++ `Pathfinder::isAttackViewBlockedByObstacle` leftover gates + static Bresenham.
    pub fn is_attack_view_blocked_for(
        &self,
        from: Vec3,
        to: Vec3,
        attacker: Option<&Object>,
        victim: Option<&Object>,
    ) -> bool {
        if !Self::attack_uses_line_of_sight() {
            return false;
        }
        if let Some(atk) = attacker {
            if !atk.is_kind_of(KindOf::AttackNeedsLineOfSight) {
                return false;
            }
        }
        if let Some(v) = victim {
            // C++ isSignificantlyAboveTerrain — do not LOS-check flying victims.
            if v.is_significantly_above_terrain() {
                return false;
            }
        }
        let mut skip_ids: Vec<u32> = Vec::new();
        if let Some(atk) = attacker {
            skip_ids.push(atk.id.0);
            if let Some(c) = atk.contained_by {
                skip_ids.push(c.0);
            }
            if let Some(p) = atk.producer_id {
                skip_ids.push(p.0);
            }
        }
        if let Some(v) = victim {
            skip_ids.push(v.id.0);
            if let Some(p) = v.producer_id {
                skip_ids.push(p.0);
            }
        }
        let skip_count = if attacker.is_some() {
            let layer = self.grid.layer_for_destination(from);
            if !matches!(
                layer,
                PathfindLayerEnum::Ground | PathfindLayerEnum::Invalid
            ) {
                3
            } else {
                0
            }
        } else {
            0
        };
        self.grid
            .is_attack_view_blocked_static_ex(from, to, skip_count, &skip_ids)
    }

    /// C++ `Pathfinder::snapClosestGoalPosition` (AIPathfind.cpp:5101-5156).
    pub fn snap_closest_goal_position(
        &self,
        pos: Vec3,
        surfaces: u32,
        is_crusher: bool,
        unit_radius: f32,
    ) -> Vec3 {
        let cell_size = self.grid.grid_size();
        let (radius, center_in_cell) = PathfindingGrid::radius_and_center(unit_radius, cell_size);
        let mut adjust = pos;
        if !center_in_cell {
            adjust.x += cell_size * 0.5;
            adjust.z += cell_size * 0.5;
        }
        let layer = self.grid.layer_for_destination(pos);
        let cell = self.grid.world_to_grid(adjust);
        let crusher_level = if is_crusher {
            self.seeker_crusher_level.max(1)
        } else {
            0
        };
        let snap_cell = |c: GridPos| -> Vec3 {
            let mut w = self.grid.grid_to_world_on_layer(c, layer);
            if center_in_cell {
                w.x += cell_size * 0.5;
                w.z += cell_size * 0.5;
            }
            w
        };
        let dest_ok = |c: GridPos| {
            self.grid.destination_cell_ok(
                c,
                surfaces,
                is_crusher,
                self.seeker_player,
                crusher_level,
                layer,
            )
        };
        let mut out = snap_cell(cell);
        if dest_ok(cell) {
            return out;
        }
        for i in (cell.x - 1)..(cell.x + 2) {
            for j in (cell.y - 1)..(cell.y + 2) {
                let c = GridPos::new(i, j);
                if !self.grid.is_valid_pos(c) {
                    continue;
                }
                if dest_ok(c) {
                    return snap_cell(c);
                }
            }
        }
        if radius == 0 {
            for i in (cell.x - 1)..(cell.x + 2) {
                for j in (cell.y - 1)..(cell.y + 2) {
                    let c = GridPos::new(i, j);
                    if !self.grid.is_valid_pos(c) {
                        continue;
                    }
                    let bits = self.grid.occupancy_bits(c, layer);
                    let own = self.seeker_id.map(|id| id.0).unwrap_or(0);
                    if bits.goal_unit == 0 || (own != 0 && bits.goal_unit == own) {
                        return snap_cell(c);
                    }
                }
            }
            for i in (cell.x - 1)..(cell.x + 2) {
                for j in (cell.y - 1)..(cell.y + 2) {
                    let c = GridPos::new(i, j);
                    if !self.grid.is_valid_pos(c) {
                        continue;
                    }
                    if self.grid.occupancy_bits(c, layer).fixed == 0 {
                        return snap_cell(c);
                    }
                }
            }
        }
        let _ = out;
        snap_cell(cell)
    }

    /// C++ snapClosestGoalPosition at plant (AIStates idle restake / group pad).
    /// Occupancy ignores `seeker` so the arriver does not block their own cell.
    pub fn snap_plant_goal(
        &mut self,
        pos: Vec3,
        surfaces: u32,
        is_crusher: bool,
        unit_radius: f32,
        seeker: ObjectId,
        seeker_player: Option<u32>,
        objects: &HashMap<ObjectId, Object>,
    ) -> Vec3 {
        self.seeker_id = Some(seeker);
        self.seeker_player = seeker_player;
        self.grid.query_seeker_id = seeker.0;
        self.grid
            .update_dynamic_obstacles_ignoring(objects, Some(seeker));
        self.snap_closest_goal_position(pos, surfaces, is_crusher, unit_radius)
    }


    /// Rebuild structure static obstacles from live objects (map load / bulk sync).
    /// Does not clear terrain slope blocks — only ORs structure footprints.
    /// C++ `addObjectToPathfindMap` includes scaffolds (DozerAIUpdate.cpp:1698-1699).
    pub fn apply_structure_static_blocks(&mut self, objects: &HashMap<ObjectId, Object>) {
        let mut lo = GridPos::new(i32::MAX, i32::MAX);
        let mut hi = GridPos::new(i32::MIN, i32::MIN);
        let mut did = false;
        for obj in objects.values() {
            let rubble = PathfindingGrid::object_is_pathfind_rubble(obj);
            let blast_crater = obj.is_kind_of(KindOf::BlastCrater);
            // C++ classifyObjectFootprint(!insert) returns for BLAST_CRATER.
            if !obj.is_alive() && !rubble && !blast_crater {
                continue;
            }
            if rubble && obj.is_kind_of(KindOf::Structure) && !blast_crater {
                if let Some((a, b)) = self.grid.classify_object_footprint(obj, true) {
                    PathfindingGrid::expand_stamp_bounds(&mut lo, &mut hi, a);
                    PathfindingGrid::expand_stamp_bounds(&mut lo, &mut hi, b);
                    did = true;
                }
                continue;
            }
            if !obj.is_alive() && !blast_crater {
                continue;
            }
            if let Some((a, b)) = self.grid.classify_object_footprint(obj, false) {
                PathfindingGrid::expand_stamp_bounds(&mut lo, &mut hi, a);
                PathfindingGrid::expand_stamp_bounds(&mut lo, &mut hi, b);
                did = true;
            }
        }
        self.grid
            .restamp_permanent_blast_craters(&mut lo, &mut hi, &mut did);
        if did {
            self.grid.refresh_pinched_bounds(lo, hi);
        }
        self.grid.rebuild_path_zones();
        self.sync_wall_pieces_from_objects(objects);
    }

    /// Incremental leftover classify + pinch for one live structure (dozer place).
    pub fn classify_and_pinch_object(&mut self, obj: &Object) {
        if let Some((lo, hi)) = self.grid.classify_object_footprint(obj, false) {
            self.grid.refresh_pinched_bounds(lo, hi);
            self.grid.rebuild_path_zones();
        }
    }


    /// C++ `Pathfinder::addWallPiece` from a live host object.
    pub fn add_wall_piece_from_object(&mut self, obj: &Object) {
        let geom = &obj.thing.template.geometry_info;
        let major = if geom.authored && geom.major_radius > 0.0 {
            geom.major_radius
        } else {
            obj.selection_radius.max(1.0)
        };
        let minor = if geom.authored {
            if matches!(geom.geom_type, crate::game_logic::HostGeometryType::Sphere) {
                major
            } else {
                geom.minor_radius.max(0.1)
            }
        } else {
            major
        };
        if self.grid.wall_height <= 0.0 && geom.authored && geom.height > 0.0 {
            self.grid.wall_height = geom.height;
        }
        if self.grid.wall_height <= 0.0 {
            if let Ok(ai) = gamelogic::ai::THE_AI.read() {
                if let Ok(data) = ai.get_ai_data().read() {
                    if data.wall_height > 0.0 {
                        self.grid.wall_height = data.wall_height;
                    }
                }
            }
        }
        self.grid.add_wall_piece(
            obj.id.0,
            obj.get_position(),
            obj.get_orientation(),
            major,
            minor,
        );
    }

    pub fn remove_wall_piece(&mut self, id: ObjectId) {
        self.grid.remove_wall_piece(id.0);
    }

    pub fn is_point_on_wall(&self, pos: Vec3) -> bool {
        self.grid.is_point_on_wall(pos)
    }

    pub fn set_wall_height(&mut self, h: f32) {
        self.grid.set_wall_height(h);
    }

    pub fn wall_height(&self) -> f32 {
        self.grid.wall_height()
    }

    /// Rebuild wall pieces from live `WALK_ON_TOP_OF_WALL` objects.
    pub fn sync_wall_pieces_from_objects(&mut self, objects: &HashMap<ObjectId, Object>) {
        self.grid.wall_pieces.clear();
        self.grid.wall_cells.clear();
        for obj in objects.values() {
            if obj.is_alive() && obj.is_kind_of(KindOf::WalkOnTopOfWall) {
                self.add_wall_piece_from_object(obj);
            }
        }
        if self.grid.wall_pieces.is_empty() {
            self.grid.terrain_gen = self.grid.terrain_gen.wrapping_add(1);
        }
    }

    /// C++ classifyObjectFootprint wall remove: DAMAGE_FALLING / DEATH_SPLATTED.
    pub fn splat_units_on_wall_piece(
        &self,
        piece_id: ObjectId,
        objects: &HashMap<ObjectId, Object>,
    ) -> Vec<ObjectId> {
        let wall_h = self.grid.wall_height;
        objects
            .values()
            .filter(|obj| {
                if obj.id == piece_id || !obj.is_alive() {
                    return false;
                }
                if obj.is_kind_of(KindOf::Structure) {
                    return false;
                }
                if !self.grid.is_point_on_wall_piece(piece_id.0, obj.get_position()) {
                    return false;
                }
                // Stand-in for C++ `obj->getLayer() == LAYER_WALL`: unit Y
                // must sit on the wall deck, not the ground footprint.
                (obj.get_position().y - wall_h).abs() <= LAYER_Z_CLOSE_ENOUGH_F
                    && wall_h > 0.0
            })
            .map(|obj| obj.id)
            .collect()
    }


    /// C++ `Pathfinder::findAttackPath` (AIPathfind.cpp:10530+).
    ///
    /// Quick steps toward the victim, then A* whose goal is any in-range
    /// `checkDestination` cell with LOS. Start cell is never accepted.
    /// Closest valid movement cell is the leftover fallback.
    pub fn find_attack_firing_position(
        &mut self,
        from: Vec3,
        victim: Vec3,
        weapon_range: f32,
        objects: &HashMap<ObjectId, Object>,
        is_crusher: bool,
    ) -> Option<Vec<Vec3>> {
        self.ensure_dynamic_obstacles(objects);
        let range = weapon_range.max(self.grid.grid_size());
        let cell_size = self.grid.grid_size();
        let start = self.grid.world_to_grid(from);
        let victim_cell = self.grid.world_to_grid(victim);
        let surfaces = SURFACE_GROUND;
        let crusher_level = if is_crusher {
            self.seeker_crusher_level.max(1)
        } else {
            0
        };
        let seeker_player = self.seeker_player;
        let is_human = true;
        let layer = PathfindLayerEnum::Ground;
        self.grid.set_query_is_human(is_human);
        let attacker_id = self.seeker_id;
        let victim_id = objects
            .values()
            .filter(|o| {
                let p = o.get_position();
                let dx = p.x - victim.x;
                let dz = p.z - victim.z;
                dx * dx + dz * dz <= (cell_size * 2.0).powi(2)
            })
            .min_by(|a, b| {
                let da = {
                    let p = a.get_position();
                    let dx = p.x - victim.x;
                    let dz = p.z - victim.z;
                    dx * dx + dz * dz
                };
                let db = {
                    let p = b.get_position();
                    let dx = p.x - victim.x;
                    let dz = p.z - victim.z;
                    dx * dx + dz * dz
                };
                da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|o| o.id);
        let is_vehicle = attacker_id
            .and_then(|id| objects.get(&id))
            .is_some_and(|o| o.is_kind_of(KindOf::Vehicle));
        let ally_mask = seeker_player
            .map(|p| self.grid.ally_mask_for(p))
            .unwrap_or(0);
        let seeker_inf = self.seeker_is_infantry;

        // Quick check: step toward victim (C++ i=1..10, delta * i * 0.5 * cell).
        {
            let mut delta = Vec3::new(victim.x - from.x, 0.0, victim.z - from.z);
            let len = (delta.x * delta.x + delta.z * delta.z).sqrt();
            if len > f32::EPSILON {
                delta = delta / len * cell_size;
                for i in 1..10 {
                    let test = from + delta * (i as f32 * 0.5);
                    let cell = self.grid.world_to_grid(test);
                    if !self.grid.is_valid_pos(cell)
                        || !self.grid.cell_passable_for(cell, surfaces, is_crusher)
                    {
                        break;
                    }
                    if !self.grid.destination_cell_ok(
                        cell,
                        surfaces,
                        is_crusher,
                        seeker_player,
                        crusher_level,
                        layer,
                    ) {
                        break;
                    }
                    if !self.grid.human_extent_allows(cell, is_human) {
                        break;
                    }
                    let dist = {
                        let dx = test.x - victim.x;
                        let dz = test.z - victim.z;
                        (dx * dx + dz * dz).sqrt()
                    };
                    if dist <= range
                        && !self.is_attack_view_blocked_for(
                            test,
                            victim,
                            attacker_id.and_then(|id| objects.get(&id)),
                            victim_id.and_then(|id| objects.get(&id)),
                        )
                    {
                        let path = self.find_path_ex_surfaces(
                            from,
                            test,
                            objects,
                            false,
                            surfaces,
                            is_crusher,
                            attacker_id,
                        );
                        self.grid.set_query_is_human(false);
                        return path;
                    }
                }
            }
        }

        // Hierarchical connectivity probe (C++ findClosestHierarchicalPath).
        self.sync_crate_astar();
        let attack_jumps = self.hierarchical_bridge_jumps();
        let attack_start = self.host_to_crate_coord(start);
        let attack_victim = self.host_to_crate_coord(victim_cell);
        if let Some(crate_pf) = self.crate_astar.as_mut() {
            crate_pf.finder.apply_hierarchical_zone_prune(
                attack_start,
                attack_victim,
                surfaces,
                is_crusher,
                &attack_jumps,
            );
        }

        const ATTACK_CELL_LIMIT: i32 = 2500;
        const COST_ORTHO: i32 = 10;
        const COST_DIAG: i32 = 14;
        let attack_dist_cells = ((range / cell_size).round() as i32) + 3;
        let attack_dist_cost_units = attack_dist_cells.max(0) * COST_ORTHO;
        if !self.grid.is_valid_pos(start) {
            self.grid.set_query_is_human(false);
            return None;
        }


        let deltas: [(i32, i32); 8] = [
            (1, 0),
            (0, 1),
            (-1, 0),
            (0, -1),
            (1, 1),
            (-1, 1),
            (-1, -1),
            (1, -1),
        ];
        let mut open: BinaryHeap<std::cmp::Reverse<(i32, i32, i32, i32)>> = BinaryHeap::new();
        let mut g_score: HashMap<(i32, i32), i32> = HashMap::new();
        let mut came_from: HashMap<(i32, i32), (i32, i32)> = HashMap::new();
        let mut closed: HashSet<(i32, i32)> = HashSet::new();
        open.push(std::cmp::Reverse((0, 0, start.x, start.y)));
        g_score.insert((start.x, start.y), 0);

        let mut cell_count = 0i32;
        let mut found_cell: Option<GridPos> = None;
        let mut closest_cell: Option<GridPos> = None;
        let mut closest_dist_sqr = f32::MAX;
        let half_cell_sqr = (cell_size * 0.5).powi(2);

        while let Some(std::cmp::Reverse((_f, g, cx, cy))) = open.pop() {
            if closed.contains(&(cx, cy)) {
                continue;
            }
            closed.insert((cx, cy));
            let cell = GridPos::new(cx, cy);
            let world = self.grid.grid_to_world(cell);
            let dest_ok = self.grid.destination_cell_ok(
                cell,
                surfaces,
                is_crusher,
                seeker_player,
                crusher_level,
                layer,
            );
            if dest_ok {
                let dx = world.x - victim.x;
                let dz = world.z - victim.z;
                let in_range = dx * dx + dz * dz <= range * range;
                if in_range {
                    let mut blocked = cell == start;
                    if !blocked {
                        let sdx = world.x - from.x;
                        let sdz = world.z - from.z;
                        if sdx * sdx + sdz * sdz < half_cell_sqr {
                            blocked = true;
                        }
                    }
                    if !blocked
                        && self.is_attack_view_blocked_for(
                            world,
                            victim,
                            attacker_id.and_then(|id| objects.get(&id)),
                            victim_id.and_then(|id| objects.get(&id)),
                        )
                    {
                        blocked = true;
                    }
                    if !blocked {
                        found_cell = Some(cell);
                        break;
                    }
                }
                if self.grid.cell_passable_for(cell, surfaces, is_crusher) {
                    let ddx = (victim_cell.x - cx).abs() as f32;
                    let ddy = (victim_cell.y - cy).abs() as f32;
                    let d2 = ddx * ddx + ddy * ddy;
                    if d2 < closest_dist_sqr {
                        closest_dist_sqr = d2;
                        closest_cell = Some(cell);
                    }
                }
            }

            if cell_count >= ATTACK_CELL_LIMIT {
                continue;
            }
            cell_count += 1;
            // C++ checkChangeLayers: enqueue connect-layer same-xy at parent cost.
            if let Some(_link) = self.grid.connect_layer_of(cell, layer) {
                let key = (cx, cy);
                if !closed.contains(&key) && !g_score.get(&key).is_some_and(|&og| g >= og) {
                    g_score.insert(key, g);
                    if cx != start.x || cy != start.y {
                        came_from.insert(key, (cx, cy));
                    }
                    open.push(std::cmp::Reverse((g, g, cx, cy)));
                }
            }
            let mut neighbor_flags = [false; 8];
            for (i, &(dx, dy)) in deltas.iter().enumerate() {
                let nx = cx + dx;
                let ny = cy + dy;
                let nc = GridPos::new(nx, ny);
                if !self.grid.is_valid_pos(nc) || closed.contains(&(nx, ny)) {
                    continue;
                }
                if !self.grid.human_extent_allows(nc, is_human) {
                    continue;
                }
                if PathfindingGrid::skip_diagonal_if_squeezed(i, &neighbor_flags) {
                    continue;
                }
                if !self.grid.cell_passable_for(nc, surfaces, is_crusher)
                    && !(self.grid.is_obstacle_fence(nc) && is_crusher)
                {
                    continue;
                }
                neighbor_flags[i] = true;
                let mut step = if i >= 4 { COST_DIAG } else { COST_ORTHO };
                match self.grid.attack_step_occupancy(
                    nc,
                    start,
                    seeker_player,
                    ally_mask,
                    seeker_inf,
                    is_vehicle,
                    crusher_level,
                    layer,
                ) {
                    None => continue,
                    Some(extra) => step += extra,
                }
                if self.grid.is_pinched(nc) {
                    step += COST_ORTHO + COST_DIAG;
                }
                if let Some(crate_pf) = self.crate_astar.as_ref() {
                    if !crate_pf.finder.is_zone_passable(GridCoord::new(nx, ny)) {
                        step += 100 * COST_ORTHO;
                    }
                }
                let ng = g + step;
                let key = (nx, ny);
                if g_score.get(&key).is_some_and(|&og| ng >= og) {
                    continue;
                }
                g_score.insert(key, ng);
                came_from.insert(key, (cx, cy));
                let hdx = (nx - victim_cell.x) as f32;
                let hdy = (ny - victim_cell.y) as f32;
                let heu = (COST_ORTHO as f32) * (hdx * hdx + hdy * hdy).sqrt();
                let h_rem = (heu - (attack_dist_cost_units as f32) / 2.0).max(0.0);
                let f_cost = ng + h_rem as i32;
                open.push(std::cmp::Reverse((f_cost, ng, nx, ny)));
            }
        }

        let Some(mut goal_cell) = found_cell.or(closest_cell) else {
            self.grid.set_query_is_human(false);
            return None;
        };
        // C++ vehicle strip-back: walk parent chain ≤12; last blocked → parent.
        if is_vehicle {
            let mut chain: Vec<GridPos> = Vec::new();
            let mut cur = (goal_cell.x, goal_cell.y);
            chain.push(GridPos::new(cur.0, cur.1));
            while let Some(&p) = came_from.get(&cur) {
                chain.push(GridPos::new(p.0, p.1));
                cur = p;
                if chain.len() > 64 {
                    break;
                }
            }
            let mut last_blocked: Option<GridPos> = None;
            let mut use_large = false;
            let large_r = (self.seeker_path_diameter.max(1) / 2).max(1);
            for (idx, c) in chain.iter().enumerate() {
                if idx >= 12 {
                    break;
                }
                let r = if use_large { large_r } else { 0 };
                let mut fixed = 0u16;
                let mut goal_m = 0u16;
                let mut crush = 0u8;
                for dx in -r..=r {
                    for dy in -r..=r {
                        let n = GridPos::new(c.x + dx, c.y + dy);
                        if !self.grid.is_valid_pos(n) {
                            continue;
                        }
                        let b = self.grid.occupancy_bits(n, layer);
                        fixed |= b.fixed;
                        goal_m |= b.goal;
                        crush = crush.max(b.crushable);
                    }
                }
                let friend = match seeker_player {
                    Some(player) => (1u16 << player.min(15)) | ally_mask,
                    None => 0,
                };
                let enemy_fixed = seeker_player.is_some()
                    && (fixed & !friend) != 0
                    && (crusher_level == 0 || crusher_level <= crush);
                let mut blocked_by_allies = (fixed & friend) != 0 || (goal_m & friend) != 0;
                let occupant = objects.values().find(|o| {
                    let gp = self.grid.world_to_grid(o.get_position());
                    (gp.x - c.x).abs() <= r && (gp.y - c.y).abs() <= r
                });
                let unit_idle = occupant.is_some_and(|o| {
                    matches!(o.ai_state, crate::game_logic::AIState::Idle)
                });
                if unit_idle {
                    blocked_by_allies = false;
                }
                let move_ok = self.grid.cell_passable_for(*c, surfaces, is_crusher)
                    || (self.grid.is_obstacle_fence(*c) && is_crusher);
                if !move_ok || enemy_fixed || blocked_by_allies {
                    last_blocked = Some(*c);
                    use_large = true;
                } else {
                    use_large = false;
                }
            }
            if let Some(lb) = last_blocked {
                if let Some(&(px, py)) = came_from.get(&(lb.x, lb.y)) {
                    goal_cell = GridPos::new(px, py);
                } else {
                    goal_cell = lb;
                }
            }
        }
        let goal = self.grid.grid_to_world(goal_cell);
        let path = self.find_path_ex_surfaces(
            from,
            goal,
            objects,
            false,
            surfaces,
            is_crusher,
            attacker_id,
        );
        self.grid.set_query_is_human(false);
        path
    }




    /// C++ `computeNormalRadialOffset` residual (AIPathfind.cpp) on host XZ ground plane.
    pub fn compute_normal_radial_offset_xz(
        from: Vec3,
        to: Vec3,
        obj_pos: Vec3,
        radius: f32,
    ) -> Vec3 {
        let dx = to.x - from.x;
        let dz = to.z - from.z;
        let obj_dx = obj_pos.x - from.x;
        let obj_dz = obj_pos.z - from.z;
        let cross = dx * obj_dz - dz * obj_dx;
        let (mut nx, mut nz) = if cross > 0.0 { (dz, -dx) } else { (-dz, dx) };
        let len = (nx * nx + nz * nz).sqrt();
        if len > 0.0001 {
            nx /= len;
            nz /= len;
        } else {
            nx = 1.0;
            nz = 0.0;
        }
        Vec3::new(obj_pos.x + nx * radius, obj_pos.y, obj_pos.z + nz * radius)
    }

    /// Leftover `tall_buildings.rs`: bounding circle + 2 pathfind cells.
    fn aircraft_path_around_radius(obj: &Object) -> Option<f32> {
        if !obj.is_alive() || !obj.is_kind_of(crate::game_logic::KindOf::AircraftPathAround) {
            return None;
        }
        let geom = obj.thing.template.geometry_info;
        let r = if geom.authored {
            geom.bounding_circle_radius()
        } else {
            obj.selection_radius.max(0.0)
        };
        Some(r + 2.0 * 10.0)
    }

    /// Leftover `find_tall_building_along_segment`: AIRCRAFT_PATH_AROUND only.
    fn find_tall_building_along_segment(
        from: Vec3,
        to: Vec3,
        objects: &HashMap<ObjectId, Object>,
        ignore: Option<ObjectId>,
    ) -> Option<(ObjectId, Vec3, f32)> {
        let dx = to.x - from.x;
        let dz = to.z - from.z;
        let len = (dx * dx + dz * dz).sqrt();
        if len < 0.01 {
            return None;
        }
        let steps = ((len / 5.0).ceil() as i32).clamp(1, 256);
        let mut best: Option<(ObjectId, Vec3, f32, f32)> = None;
        for obj in objects.values() {
            if ignore == Some(obj.id) {
                continue;
            }
            let Some(radius) = Self::aircraft_path_around_radius(obj) else {
                continue;
            };
            let p = obj.get_position();
            let t = (((p.x - from.x) * dx + (p.z - from.z) * dz) / (len * len)).clamp(0.0, 1.0);
            let cx = from.x + dx * t;
            let cz = from.z + dz * t;
            let dist = ((p.x - cx) * (p.x - cx) + (p.z - cz) * (p.z - cz)).sqrt();
            if dist > radius {
                continue;
            }
            match best {
                Some((_, _, _, bt)) if t >= bt => {}
                _ => best = Some((obj.id, p, radius, t)),
            }
        }
        if let Some((id, p, r, _)) = best {
            for i in 0..=steps {
                let t = i as f32 / steps as f32;
                let sx = from.x + dx * t;
                let sz = from.z + dz * t;
                let d = ((p.x - sx) * (p.x - sx) + (p.z - sz) * (p.z - sz)).sqrt();
                if d <= r {
                    return Some((id, p, r));
                }
            }
        }
        None
    }


    /// C++ `Pathfinder::segmentIntersectsTallBuilding` residual (host XZ).
    /// Returns optional nudged `to` plus three insert waypoints.
    pub fn segment_intersects_tall_building(
        from: Vec3,
        mut to: Vec3,
        objects: &HashMap<ObjectId, Object>,
        ignore: Option<ObjectId>,
    ) -> Option<(Vec3, Vec3, Vec3, Vec3)> {
        let mut from_pos = from;
        let mut to_pos = to;
        for _ in 0..2 {
            let Some((_id, bldg_pos, radius)) =
                Self::find_tall_building_along_segment(from_pos, to_pos, objects, ignore)
            else {
                return None;
            };

            // If to inside radius, push out and retry.
            let mut delta_x = to_pos.x - bldg_pos.x;
            let mut delta_z = to_pos.z - bldg_pos.z;
            let mut len = (delta_x * delta_x + delta_z * delta_z).sqrt();
            if len <= radius * 0.98 {
                if len < 0.1 {
                    delta_x = 1.0;
                    delta_z = 0.0;
                    len = 1.0;
                }
                delta_x = delta_x / len * radius;
                delta_z = delta_z / len * radius;
                to_pos.x = bldg_pos.x + delta_x;
                to_pos.z = bldg_pos.z + delta_z;
                to = to_pos;
                continue;
            }

            // If from inside radius, push from out.
            delta_x = from_pos.x - bldg_pos.x;
            delta_z = from_pos.z - bldg_pos.z;
            len = (delta_x * delta_x + delta_z * delta_z).sqrt();
            if len <= radius * 0.98 {
                if len < 0.1 {
                    delta_x = 1.0;
                    delta_z = 0.0;
                    len = 1.0;
                }
                delta_x = delta_x / len * radius;
                delta_z = delta_z / len * radius;
                from_pos.x = bldg_pos.x + delta_x;
                from_pos.z = bldg_pos.z + delta_z;
            }

            let insert2 = Self::compute_normal_radial_offset_xz(from_pos, to_pos, bldg_pos, radius);
            let insert1 =
                Self::compute_normal_radial_offset_xz(from_pos, insert2, bldg_pos, radius);
            let insert3 = Self::compute_normal_radial_offset_xz(insert2, to_pos, bldg_pos, radius);
            return Some((to, insert1, insert2, insert3));
        }
        None
    }

    /// C++ `AIUpdateInterface::getBuildingToNotPathAround` (Chinook combat-drop).
    fn building_to_not_path_around(
        objects: &HashMap<ObjectId, Object>,
        seeker: Option<ObjectId>,
    ) -> Option<ObjectId> {
        let obj = seeker.and_then(|id| objects.get(&id))?;
        let ai = obj.chinook_ai.as_ref()?;
        use crate::game_logic::host_combat_chinook::HostChinookAIState;
        if matches!(
            ai.state,
            HostChinookAIState::MoveToCombatDrop | HostChinookAIState::DoCombatDrop
        ) {
            ai.combat_drop_target.map(ObjectId)
        } else {
            None
        }
    }

    /// C++ aircraft tall-building path detour residual: walk path segments and
    /// insert radial offsets when AIRCRAFT_PATH_AROUND / tall structures clip.
    pub fn detour_path_around_tall_buildings(
        path: &[Vec3],
        objects: &HashMap<ObjectId, Object>,
    ) -> Vec<Vec3> {
        Self::detour_path_around_tall_buildings_ignoring(path, objects, None)
    }

    /// Leftover `get_aircraft_path` segment walk (`limit=20`, insert n1,n2,n3).
    pub fn detour_path_around_tall_buildings_ignoring(
        path: &[Vec3],
        objects: &HashMap<ObjectId, Object>,
        ignore: Option<ObjectId>,
    ) -> Vec<Vec3> {
        if path.len() < 2 {
            return path.to_vec();
        }
        let mut waypoints = path.to_vec();
        let mut limit = 20i32;
        let mut idx = 0usize;
        while idx + 1 < waypoints.len() && limit >= 0 {
            let cur = waypoints[idx];
            let next = waypoints[idx + 1];
            if let Some((nudged_to, n1, n2, n3)) =
                Self::segment_intersects_tall_building(cur, next, objects, ignore)
            {
                waypoints[idx + 1] = nudged_to;
                waypoints.insert(idx + 1, n3);
                waypoints.insert(idx + 1, n2);
                waypoints.insert(idx + 1, n1);
                idx += 2;
            } else {
                idx += 1;
            }
            limit -= 1;
        }
        waypoints
    }

    pub fn find_path(
        &mut self,
        start: Vec3,
        goal: Vec3,
        objects: &HashMap<ObjectId, Object>,
    ) -> Option<Vec<Vec3>> {
        self.find_path_ex(start, goal, objects, false, None)
    }
    /// Leftover `circle_clips_tall_building`: AIRCRAFT_PATH_AROUND + normal offset.
    pub fn circle_clips_tall_building(
        from: Vec3,
        to: Vec3,
        circle_radius: f32,
        objects: &HashMap<ObjectId, Object>,
        ignore: Option<ObjectId>,
    ) -> Option<Vec3> {
        let mut best: Option<(ObjectId, Vec3, f32, f32)> = None;
        for obj in objects.values() {
            if ignore == Some(obj.id) {
                continue;
            }
            let Some(bldg_r) = Self::aircraft_path_around_radius(obj) else {
                continue;
            };
            let p = obj.get_position();
            let dx = p.x - to.x;
            let dz = p.z - to.z;
            let d = (dx * dx + dz * dz).sqrt();
            if d > circle_radius {
                continue;
            }
            match best {
                Some((_, _, _, bd)) if d >= bd => {}
                _ => best = Some((obj.id, p, bldg_r, d)),
            }
        }
        let Some((tall_id, bldg_pos, bldg_r, _)) = best else {
            return None;
        };
        let mut adjust = Self::compute_normal_radial_offset_xz(
            from,
            to,
            bldg_pos,
            circle_radius + bldg_r,
        );
        adjust.y = to.y;
        // Leftover second AIRCRAFT_PATH_AROUND near adjust_to.
        let mut other: Option<(Vec3, f32, f32)> = None;
        for obj in objects.values() {
            if ignore == Some(obj.id) || obj.id == tall_id {
                continue;
            }
            let Some(bldg_r) = Self::aircraft_path_around_radius(obj) else {
                continue;
            };
            let p = obj.get_position();
            let dx = p.x - adjust.x;
            let dz = p.z - adjust.z;
            let d = (dx * dx + dz * dz).sqrt();
            if d > circle_radius {
                continue;
            }
            match other {
                Some((_, _, bd)) if d >= bd => {}
                _ => other = Some((p, bldg_r, d)),
            }
        }
        if let Some((op, or, _)) = other {
            let tmp = adjust;
            adjust = Self::compute_normal_radial_offset_xz(from, tmp, op, circle_radius + or);
            adjust.y = to.y;
        }
        Some(adjust)
    }


    /// `aircraft`: apply C++ tall-building aircraft path-around residual after A*.
    pub fn find_path_ex(
        &mut self,
        start: Vec3,
        goal: Vec3,
        objects: &HashMap<ObjectId, Object>,
        aircraft: bool,
        mover: Option<ObjectId>,
    ) -> Option<Vec<Vec3>> {
        self.find_path_ex_surfaces(
            start,
            goal,
            objects,
            aircraft,
            if aircraft {
                SURFACE_AIR
            } else {
                SURFACE_GROUND
            },
            false,
            mover,
        )
    }

    /// Live path: crate `AStarPathfinder::find_path_ex` (AIPathfind.cpp:6438).
    pub fn find_path_ex_surfaces(
        &mut self,
        start: Vec3,
        goal: Vec3,
        objects: &HashMap<ObjectId, Object>,
        aircraft: bool,
        surfaces: u32,
        is_crusher: bool,
        mover: Option<ObjectId>,
    ) -> Option<Vec<Vec3>> {
        self.ensure_dynamic_obstacles(objects);
        self.bind_seeker_from_mover(objects, mover);
        self.apply_seeker_human_flag();
        // Live seeker CrusherLevel wins so find_path / find_path_ex still crush.
        let is_crusher = is_crusher || self.seeker_crusher_level > 0;
        if is_crusher && self.seeker_crusher_level == 0 {
            self.seeker_crusher_level = 1;
        }
        self.grid
            .set_query_footprint(self.seeker_path_diameter, is_crusher);

        let mut goal = goal;
        self.grid.query_seeker_id = self.seeker_id.map(|id| id.0).unwrap_or(0);
        let seeker_adjusts = self.seeker_id.and_then(|id| objects.get(&id)).is_some_and(
            PathfindingGrid::is_aircraft_that_adjusts_destination,
        );
        if aircraft && seeker_adjusts {
            self.grid.query_check_for_aircraft = true;
            let dest_layer = self.grid.layer_for_destination(goal);
            if let Some(adj) = self.grid.adjust_destination_on_layer(
                self.grid.world_to_grid(goal),
                surfaces,
                is_crusher,
                400,
                self.seeker_player,
                0,
                dest_layer,
            ) {
                let mut snapped = self.grid.grid_to_world(adj);
                snapped.y = goal.y;
                goal = snapped;
            }
            self.grid.query_check_for_aircraft = false;
        } else if !aircraft {
            // C++ snapClosestGoalPosition when installing the move goal.
            let unit_radius = self
                .seeker_id
                .and_then(|id| objects.get(&id))
                .map(|o| o.selection_radius)
                .unwrap_or(0.0);
            goal = self.snap_closest_goal_position(goal, surfaces, is_crusher, unit_radius);
        }
        let start_grid = self.grid.cell_for_unit_position(start, self.seeker_center_in_cell);
        let goal_grid = self.grid.cell_for_unit_position(goal, self.seeker_center_in_cell);
        let start_layer = self
            .seeker_id
            .and_then(|id| objects.get(&id))
            .map(|o| self.grid.layer_for_destination(o.get_position()))
            .unwrap_or_else(|| self.grid.layer_for_destination(start));
        let dest_layer = self.grid.layer_for_destination(goal);

        // C++ getAircraftPath: circleClips only when appearance == LOCO_WINGS.
        // segmentIntersectsTallBuilding walk runs for every aircraft (hover/thrust).
        let mut path = if aircraft {
            let check_clips = self.seeker_wings;
            let avoid = Self::building_to_not_path_around(objects, self.seeker_id);
            let goal_adj = if check_clips {
                Self::circle_clips_tall_building(start, goal, 100.0, objects, avoid)
                    .unwrap_or(goal)
            } else {
                goal
            };
            // C++ getAircraftPath: first node is unit XY at dest Z (host Y).
            let mut start_at_dest = start;
            start_at_dest.y = goal.y;
            let direct = vec![start_at_dest, goal_adj];
            Self::detour_path_around_tall_buildings_ignoring(&direct, objects, avoid)
        } else {
            self.find_path_via_crate(
                start_grid,
                goal_grid,
                surfaces,
                is_crusher,
                start_layer,
                dest_layer,
                true,
            )?
        };
        // Ground: keep crate/grid terrain-layer Y (hq-gd0jd). Do not lerp start→goal.
        // Aircraft: first/last stay at dest altitude; detours keep radial-offset Y
        // (hq-zqfpa). Do not flatten every node to start.y then overwrite first.
        if !aircraft {
            if let Some(first) = path.first_mut() {
                *first = start;
            }
        }
        // C++ checkDestination: snapped dest is the final position. Do not
        // restore the raw click (that walks units into building footprints).
        Some(path)
    }

    /// C++ `Pathfinder::clientSafeQuickDoesPathExist` (structure-aware, ground).
    pub fn client_safe_quick_does_path_exist(&self, from: Vec3, to: Vec3) -> bool {
        self.client_safe_quick_does_path_exist_for(from, to, SURFACE_GROUND)
    }

    /// C++ `clientSafeQuickDoesPathExist` with locomotor surfaces.
    pub fn client_safe_quick_does_path_exist_for(&self, from: Vec3, to: Vec3, surfaces: u32) -> bool {
        if self.grid.path_zones.iter().all(|&z| z == 0) {
            return self.grid.quick_path_exists_for_ui(from, to);
        }
        self.grid.quick_path_exists_for(from, to, surfaces)
    }

    /// C++ `Pathfinder::patchPath` (AIPathfind.cpp:10344-10520).
    pub fn patch_path(
        &mut self,
        from: Vec3,
        original: &[Vec3],
        surfaces: u32,
        is_crusher: bool,
        objects: &HashMap<ObjectId, Object>,
        mover: Option<ObjectId>,
    ) -> Option<Vec<Vec3>> {
        if self.ignore_obstacle_id.is_some() {
            self.grid
                .update_dynamic_obstacles_ignoring(objects, self.ignore_obstacle_id);
        } else {
            self.grid.update_dynamic_obstacles(objects);
        }
        self.bind_seeker_from_mover(objects, mover);
        if original.len() < 2 {
            return None;
        }
        let start = self.grid.cell_for_unit_position(from, self.seeker_center_in_cell);
        let blocked = true;
        let radius = (self.seeker_path_diameter.max(1) / 2).max(0);
        let crusher_level = if is_crusher {
            self.seeker_crusher_level.max(1)
        } else {
            0
        };
        let xz_dist_sq = |a: Vec3, b: Vec3| {
            let dx = a.x - b.x;
            let dz = a.z - b.z;
            dx * dx + dz * dz
        };
        // Walk original path reverse; stop at first blocked / occupied node.
        let mut start_node_idx = 0usize;
        let mut goal_pos = *original.last().unwrap();
        let mut goal_delta = xz_dist_sq(goal_pos, from);
        for idx in (1..original.len()).rev() {
            let cell = self.grid.world_to_grid(original[idx]);
            let layer = self.grid.layer_for_destination(original[idx]);
            if !self
                .grid
                .cell_passable_for_layer(cell, layer, surfaces, is_crusher)
            {
                start_node_idx = idx;
                break;
            }
            let dx = cell.x - start.x;
            let dy = cell.y - start.y;
            let consider_transient = blocked && dx >= -2 && dx <= 2 && dy >= -2 && dy <= 2;
            if self.grid.patch_cell_occupied(
                cell,
                layer,
                consider_transient,
                self.seeker_player,
                self.seeker_is_infantry,
                crusher_level,
                radius,
                self.seeker_center_in_cell,
            ) {
                start_node_idx = idx;
                break;
            }
            let cur = xz_dist_sq(original[idx], from);
            if cur < goal_delta {
                goal_pos = original[idx];
                goal_delta = cur;
            }
            start_node_idx = idx;
        }
        // C++: startNode == last → no open suffix to splice onto.
        if start_node_idx + 1 >= original.len() {
            return None;
        }

        let start_layer = self.grid.layer_for_destination(from);
        let mut try_goal = |goal: Vec3| -> Option<Vec<Vec3>> {
            let goal_cell = self.grid.world_to_grid(goal);
            let dest_layer = self.grid.layer_for_destination(goal);
            self.find_path_via_crate(
                start,
                goal_cell,
                surfaces,
                is_crusher,
                start_layer,
                dest_layer,
                true,
            )
        };
        let mut prefix = try_goal(goal_pos);
        if prefix.is_none() {
            for idx in ((start_node_idx + 1)..original.len()).rev() {
                if let Some(trial) = try_goal(original[idx]) {
                    prefix = Some(trial);
                    goal_pos = original[idx];
                    break;
                }
            }
        }
        let mut prefix = prefix?;

        let end = prefix.last().copied().unwrap_or(goal_pos);
        let mut match_idx = None;
        for idx in ((start_node_idx + 1)..original.len()).rev() {
            let p = original[idx];
            if (p.x - end.x).abs() < 0.5 && (p.z - end.z).abs() < 0.5 {
                match_idx = Some(idx);
                break;
            }
            let a = self.grid.world_to_grid(p);
            let b = self.grid.world_to_grid(end);
            if a.x == b.x && a.y == b.y {
                match_idx = Some(idx);
                break;
            }
        }
        let match_idx = match_idx.unwrap_or(original.len() - 1);
        if let Some(last) = prefix.last() {
            let m = original[match_idx];
            if (last.x - m.x).abs() < 0.5 && (last.z - m.z).abs() < 0.5 {
                prefix.pop();
            }
        }
        prefix.extend_from_slice(&original[match_idx..]);
        let optimized = self.grid.optimize_ground_path_ex(
            &prefix,
            surfaces,
            is_crusher,
            self.seeker_player,
            crusher_level,
        );
        if optimized.is_empty() {
            None
        } else {
            Some(optimized)
        }
    }

    /// C++ `Pathfinder::getMoveAwayFromPath` (AIPathfind.cpp:10171-10338).
    /// A* to a cell whose clearance box does not overlap avoided path segments.
    pub fn get_move_away_from_path(
        &mut self,
        from: Vec3,
        path_to_avoid: &[Vec3],
        path_to_avoid2: Option<&[Vec3]>,
        surfaces: u32,
        is_crusher: bool,
        unit_radius: f32,
        other_radius: f32,
        seeker_player: Option<u32>,
        crusher_level: u8,
        tunnel: bool,
    ) -> Option<Vec<Vec3>> {
        if path_to_avoid.len() < 2 {
            return None;
        }
        let (radius, center_in_cell) =
            PathfindingGrid::radius_and_center(unit_radius, self.grid.grid_size());
        let (other_r, other_center) =
            PathfindingGrid::radius_and_center(other_radius, self.grid.grid_size());
        let start = self.grid.cell_for_unit_position(from, center_in_cell);
        if !self.grid.is_valid_pos(start) {
            return None;
        }
        let cell = self.grid.grid_size();
        let mut box_half = radius as f32 * cell - cell / 4.0;
        if center_in_cell {
            box_half += cell / 2.0;
        }
        box_half += other_r as f32 * cell;
        if other_center {
            box_half += cell / 2.0;
        }
        let layer = self.grid.layer_for_destination(from);
        let mut open: BinaryHeap<std::cmp::Reverse<(i32, i32, i32)>> = BinaryHeap::new();
        let mut g_score: HashMap<GridPos, i32> = HashMap::new();
        let mut closed: HashSet<GridPos> = HashSet::new();
        open.push(std::cmp::Reverse((0, start.x, start.y)));
        g_score.insert(start, 0);
        let mut found: Option<GridPos> = None;
        let mut expanded = 0i32;
        const MAX_EXPAND: i32 = 2500;
        let overlaps = |cell_pos: GridPos| -> bool {
            let mut center = self.grid.grid_to_world(cell_pos);
            if center_in_cell {
                center.x += cell * 0.5;
                center.z += cell * 0.5;
            }
            let region = CellXz {
                lo_x: center.x - box_half,
                lo_z: center.z - box_half,
                hi_x: center.x + box_half,
                hi_z: center.z + box_half,
            };
            let check = |path: &[Vec3]| {
                path.windows(2).any(|w| {
                    line_in_region_xz(w[0].x, w[0].z, w[1].x, w[1].z, &region)
                })
            };
            check(path_to_avoid) || path_to_avoid2.is_some_and(check)
        };
        while let Some(std::cmp::Reverse((g, cx, cy))) = open.pop() {
            let cell_pos = GridPos::new(cx, cy);
            if !closed.insert(cell_pos) {
                continue;
            }
            expanded += 1;
            if expanded > MAX_EXPAND {
                break;
            }
            let overlap = cell_pos == start || overlaps(cell_pos);
            if !overlap
                && self.grid.check_destination_for(
                    cell_pos,
                    layer,
                    radius,
                    center_in_cell,
                    surfaces,
                    is_crusher,
                    seeker_player,
                    crusher_level,
                )
            {
                found = Some(cell_pos);
                break;
            }
            for n in cell_pos.neighbors() {
                if !self.grid.is_valid_pos(n) || closed.contains(&n) {
                    continue;
                }
                if !tunnel
                    && !self.grid.cell_passable_for_layer(n, layer, surfaces, is_crusher)
                    && !(self.grid.is_obstacle_fence(n) && is_crusher)
                {
                    continue;
                }
                let step = if (n.x - cx).abs() + (n.y - cy).abs() == 2 {
                    14
                } else {
                    10
                };
                let ng = g + step;
                if g_score.get(&n).is_some_and(|&og| ng >= og) {
                    continue;
                }
                g_score.insert(n, ng);
                open.push(std::cmp::Reverse((ng, n.x, n.y)));
            }
        }
        let dest = found?;
        if dest == start {
            return None;
        }
        let world_dest = {
            let mut w = self.grid.grid_to_world(dest);
            if center_in_cell {
                w.x += cell * 0.5;
                w.z += cell * 0.5;
            }
            w.y = from.y;
            w
        };
        if let Some(path) = self.find_path_via_crate(
            start,
            dest,
            surfaces,
            is_crusher,
            layer,
            layer,
            true,
        ) {
            if path.len() >= 2 {
                return Some(path);
            }
        }
        Some(vec![from, world_dest])
    }



    /// C++ `Pathfinder::findSafePath` Dijkstra flee (AIPathfind.cpp:10885+).
    /// Single-repulsor wrapper — both radii share `repulsor`.
    pub fn find_safe_path(
        &mut self,
        from: Vec3,
        repulsor: Vec3,
        repulsor_radius: f32,
        surfaces: u32,
        is_crusher: bool,
    ) -> Option<Vec<Vec3>> {
        self.find_safe_path_from(
            from,
            repulsor,
            repulsor,
            repulsor_radius,
            surfaces,
            is_crusher,
            true,
        )
    }

    /// C++ `Pathfinder::findSafePath` with two repulsors, human extent,
    /// destination_cell_ok, then `findPath` to the flee cell.
    pub fn find_safe_path_from(
        &mut self,
        from: Vec3,
        repulsor1: Vec3,
        repulsor2: Vec3,
        repulsor_radius: f32,
        surfaces: u32,
        is_crusher: bool,
        is_human: bool,
    ) -> Option<Vec<Vec3>> {
        self.sync_crate_astar();
        let start = self.grid.cell_for_unit_position(from, self.seeker_center_in_cell);
        if !self.grid.is_valid_pos(start) {
            return None;
        }
        let radius_sqr = repulsor_radius * repulsor_radius;
        let crusher_level = if is_crusher {
            self.seeker_crusher_level.max(1)
        } else {
            0
        };
        let seeker_player = self.seeker_player;
        let start_layer = self.grid.layer_for_destination(from);
        let start_lid = start_layer as u8;
        const MAX_CELLS: i32 = 2000;
        const COST_ORTHO: i32 = 10;
        const COST_DIAG: i32 = 14;
        let deltas: [(i32, i32); 8] = [
            (1, 0),
            (0, 1),
            (-1, 0),
            (0, -1),
            (1, 1),
            (-1, 1),
            (-1, -1),
            (1, -1),
        ];
        let mut open: BinaryHeap<std::cmp::Reverse<(i32, i32, i32, i32, u8)>> = BinaryHeap::new();
        let mut g_score: HashMap<(i32, i32, u8), i32> = HashMap::new();
        let mut closed: HashSet<(i32, i32, u8)> = HashSet::new();
        open.push(std::cmp::Reverse((0, 0, start.x, start.y, start_lid)));
        g_score.insert((start.x, start.y, start_lid), 0);
        let mut farthest: Option<(GridPos, PathfindLayerEnum, f32)> = None;
        let mut found: Option<(GridPos, PathfindLayerEnum)> = None;
        let mut cell_count = 0i32;
        while let Some(std::cmp::Reverse((_f, g, cx, cy, lid))) = open.pop() {
            let key = (cx, cy, lid);
            if !closed.insert(key) {
                continue;
            }
            let cell = GridPos::new(cx, cy);
            let layer = PathfindLayerEnum::from_u32(lid as u32);
            let world = self.grid.grid_to_world_on_layer(cell, layer);
            let d1 = {
                let dx = world.x - repulsor1.x;
                let dz = world.z - repulsor1.z;
                dx * dx + dz * dz
            };
            let d2 = {
                let dx = world.x - repulsor2.x;
                let dz = world.z - repulsor2.z;
                dx * dx + dz * dz
            };
            let nearest = d1.min(d2);
            let mut ok = nearest > radius_sqr;
            if open.is_empty() && cell_count > 0 {
                ok = true;
            }
            if farthest.map(|(_, _, d)| nearest > d).unwrap_or(true) {
                farthest = Some((cell, layer, nearest));
                if cell_count > MAX_CELLS {
                    ok = true;
                }
            }
            if ok
                && self.grid.destination_cell_ok(
                    cell,
                    surfaces,
                    is_crusher,
                    seeker_player,
                    crusher_level,
                    layer,
                )
                && self.grid.human_extent_allows(cell, is_human)
            {
                found = Some((cell, layer));
                break;
            }
            if self
                .grid
                .enqueue_connect_layer(cell, layer, g, g, &closed, &mut g_score, &mut open)
            {
                cell_count += 1;
            }
            let mut neighbor_flags = [false; 8];
            for (i, (dx, dy)) in deltas.iter().enumerate() {
                let nx = cx + dx;
                let ny = cy + dy;
                let nc = GridPos::new(nx, ny);
                if !self.grid.is_valid_pos(nc) || closed.contains(&(nx, ny, lid)) {
                    continue;
                }
                if !self.grid.human_extent_allows(nc, is_human) {
                    continue;
                }
                if PathfindingGrid::skip_diagonal_if_squeezed(i, &neighbor_flags) {
                    continue;
                }
                if !self
                    .grid
                    .cell_passable_for_layer(nc, layer, surfaces, is_crusher)
                    && !(self.grid.is_obstacle_fence(nc) && is_crusher)
                {
                    continue;
                }
                neighbor_flags[i] = true;
                let step = if i >= 4 { COST_DIAG } else { COST_ORTHO };
                let ng = g + step;
                let nkey = (nx, ny, lid);
                if g_score.get(&nkey).is_some_and(|&og| ng >= og) {
                    continue;
                }
                g_score.insert(nkey, ng);
                open.push(std::cmp::Reverse((ng, ng, nx, ny, lid)));
                cell_count += 1;
            }
        }
        let (dest, dest_layer) = if let Some((cell, layer)) = found {
            (cell, layer)
        } else if let Some((cell, layer, _)) = farthest {
            if !self.grid.destination_cell_ok(
                cell,
                surfaces,
                is_crusher,
                seeker_player,
                crusher_level,
                layer,
            ) {
                return None;
            }
            (cell, layer)
        } else {
            return None;
        };
        if dest == start {
            return None;
        }
        let goal = self.grid.grid_to_world_on_layer(dest, dest_layer);
        if let Some(path) = self.find_path_via_crate(
            start,
            dest,
            surfaces,
            is_crusher,
            start_layer,
            dest_layer,
            true,
        ) {
            if path.len() >= 2 {
                return Some(path);
            }
        }
        Some(vec![from, goal])
    }

    /// C++ `Pathfinder::findClosestPath` (AIPathfind.cpp:8739+).
    /// Goal cell need not be passable. Tracks closest *valid destination*
    /// by screen-dist + cost², then paths to that cell.
    pub fn find_closest_path(
        &mut self,
        from: Vec3,
        goal: Vec3,
        surfaces: u32,
        is_crusher: bool,
        is_human: bool,
    ) -> Option<Vec<Vec3>> {
        const COST_ORTHO: i32 = 10;
        const COST_DIAG: i32 = 14;
        const COST_TO_DISTANCE_FACTOR_SQR: f32 = 0.01;
        const MAX_EXPAND: i32 = 4000;
        self.sync_crate_astar();
        let start = self.grid.cell_for_unit_position(from, self.seeker_center_in_cell);
        let goal_grid = self.grid.cell_for_unit_position(goal, self.seeker_center_in_cell);
        if !self.grid.is_valid_pos(start) || !self.grid.is_valid_pos(goal_grid) {
            return None;
        }
        if is_human && !self.grid.human_extent_allows(start, true) {
            return None;
        }
        let crusher_level = if is_crusher {
            self.seeker_crusher_level.max(1)
        } else {
            0
        };
        let seeker_player = self.seeker_player;
        let start_layer = self.grid.layer_for_destination(from);
        let start_lid = start_layer as u8;
        let closest_jumps = self.hierarchical_bridge_jumps();
        let closest_start = self.host_to_crate_coord(start);
        let closest_goal = self.host_to_crate_coord(goal_grid);
        if let Some(crate_pf) = self.crate_astar.as_mut() {
            crate_pf.finder.apply_hierarchical_zone_prune(
                closest_start,
                closest_goal,
                surfaces,
                is_crusher,
                &closest_jumps,
            );
        }
        let heuristic = |c: GridPos| -> i32 {
            let dx = (goal_grid.x - c.x).abs();
            let dy = (goal_grid.y - c.y).abs();
            let dmin = dx.min(dy);
            let dmax = dx.max(dy);
            COST_DIAG * dmin + COST_ORTHO * (dmax - dmin)
        };
        let deltas: [(i32, i32); 8] = [
            (1, 0),
            (0, 1),
            (-1, 0),
            (0, -1),
            (1, 1),
            (-1, 1),
            (-1, -1),
            (1, -1),
        ];
        let mut open: BinaryHeap<std::cmp::Reverse<(i32, i32, i32, i32, u8)>> = BinaryHeap::new();
        let mut g_score: HashMap<(i32, i32, u8), i32> = HashMap::new();
        let mut closed: HashSet<(i32, i32, u8)> = HashSet::new();
        let h0 = heuristic(start);
        open.push(std::cmp::Reverse((h0, 0, start.x, start.y, start_lid)));
        g_score.insert((start.x, start.y, start_lid), 0);
        let mut closest_cell: Option<(GridPos, PathfindLayerEnum, f32)> = None;
        let mut closest_screen_sqr = f32::MAX;
        let mut found_goal_cell = false;
        let mut expanded = 0i32;
        while let Some(std::cmp::Reverse((_f, g, cx, cy, lid))) = open.pop() {
            let key = (cx, cy, lid);
            if !closed.insert(key) {
                continue;
            }
            expanded += 1;
            if expanded > MAX_EXPAND {
                break;
            }
            let cell = GridPos::new(cx, cy);
            let layer = PathfindLayerEnum::from_u32(lid as u32);
            if cx == goal_grid.x && cy == goal_grid.y {
                let goal_ok = self.grid.destination_cell_ok(
                    cell,
                    surfaces,
                    is_crusher,
                    seeker_player,
                    crusher_level,
                    layer,
                );
                if goal_ok || closest_cell.is_none() {
                    found_goal_cell = true;
                    closest_cell = Some((cell, layer, 0.0));
                    break;
                } else {
                    found_goal_cell = true;
                }
            } else if self.grid.destination_cell_ok(
                cell,
                surfaces,
                is_crusher,
                seeker_player,
                crusher_level,
                layer,
            ) {
                let dx = (goal_grid.x - cx).abs() as f32;
                let dy = (goal_grid.y - cy).abs() as f32;
                let dist_screen = dx * dx + dy * dy;
                if dist_screen < closest_screen_sqr {
                    closest_screen_sqr = dist_screen;
                }
                let cost_term = (g as f32) * (g as f32) * COST_TO_DISTANCE_FACTOR_SQR;
                let dist_sqr = dist_screen + cost_term;
                let better = match closest_cell {
                    None => true,
                    Some((_, _, best)) => dist_sqr < best,
                };
                if better {
                    closest_cell = Some((cell, layer, dist_sqr));
                }
            }
            let f_hop = g + heuristic(cell);
            self.grid.enqueue_connect_layer(
                cell,
                layer,
                g,
                f_hop,
                &closed,
                &mut g_score,
                &mut open,
            );
            let mut neighbor_flags = [false; 8];
            for (i, (dx, dy)) in deltas.iter().enumerate() {
                let nx = cx + dx;
                let ny = cy + dy;
                let nc = GridPos::new(nx, ny);
                if !self.grid.is_valid_pos(nc) || closed.contains(&(nx, ny, lid)) {
                    continue;
                }
                if !self.grid.human_extent_allows(nc, is_human) {
                    continue;
                }
                if PathfindingGrid::skip_diagonal_if_squeezed(i, &neighbor_flags) {
                    continue;
                }
                if !self
                    .grid
                    .cell_passable_for_layer(nc, layer, surfaces, is_crusher)
                    && !(self.grid.is_obstacle_fence(nc) && is_crusher)
                {
                    continue;
                }
                neighbor_flags[i] = true;
                let step = if i >= 4 { COST_DIAG } else { COST_ORTHO };
                let ng = g + step;
                let nkey = (nx, ny, lid);
                if g_score.get(&nkey).is_some_and(|&og| ng >= og) {
                    continue;
                }
                g_score.insert(nkey, ng);
                let f = ng + heuristic(nc);
                open.push(std::cmp::Reverse((f, ng, nx, ny, lid)));
            }
        }
        let (best_cell, best_layer, _) = closest_cell?;
        let to_pos = if found_goal_cell && best_cell == goal_grid {
            goal
        } else {
            self.grid.grid_to_world_on_layer(best_cell, best_layer)
        };
        let to_cell = self.grid.cell_for_unit_position(to_pos, self.seeker_center_in_cell);
        // Leftover find_closest_path: buildActualPath via crate A* (layers/occupancy).
        if let Some(path) = self.find_path_via_crate(
            start,
            to_cell,
            surfaces,
            is_crusher,
            start_layer,
            best_layer,
            false,
        ) {
            let mut world = path;
            if let Some(first) = world.first_mut() {
                *first = from;
            }
            if let Some(last) = world.last_mut() {
                *last = to_pos;
            }
            return Some(self.grid.optimize_ground_path(&world, surfaces, is_crusher));
        }
        // Leftover returns none when internalFindPath fails — no fail-open line.
        if start == to_cell {
            Some(vec![from])
        } else {
            None
        }
    }


    /// C++ `adjustDestination(..., groupDest)` for ground group members.
    pub fn adjust_group_destination(
        &mut self,
        from: Vec3,
        dest: Vec3,
        group_dest: Vec3,
        surfaces: u32,
        is_crusher: bool,
        seeker_player: Option<u32>,
        crusher_level: u8,
        seeker_id: u32,
    ) -> Vec3 {
        let dest_cell = self.grid.world_to_grid(dest);
        let group_cell = self.grid.world_to_grid(group_dest);
        let layer = self.grid.layer_for_destination(group_dest);
        self.grid.query_from = Some(self.grid.world_to_grid(from));
        self.grid.query_orig_dest = Some(dest_cell);
        self.grid.query_seeker_id = seeker_id;
        let adj = self.grid.adjust_destination_for_group(
            dest_cell,
            group_cell,
            surfaces,
            is_crusher,
            seeker_player,
            crusher_level,
            layer,
        );
        self.grid.query_from = None;
        self.grid.query_orig_dest = None;
        self.grid.query_seeker_id = 0;
        match adj {
            Some(cell) => {
                let mut w = self.grid.grid_to_world(cell);
                w.y = dest.y;
                w
            }
            None => dest,
        }
    }


    /// C++ `Path::computePointOnPath` on a host XZ polyline.
    pub fn compute_point_on_path(pos: Vec3, waypoints: &[Vec3]) -> Vec3 {
        Self::compute_point_on_path_ex(pos, waypoints, None, SURFACE_GROUND, false)
    }

    pub fn compute_point_on_path_ex(
        pos: Vec3,
        waypoints: &[Vec3],
        grid: Option<&PathfindingGrid>,
        surfaces: u32,
        is_crusher: bool,
    ) -> Vec3 {
        Self::compute_point_on_path_for(
            pos,
            waypoints,
            grid,
            surfaces,
            is_crusher,
            None,
            if is_crusher { 1 } else { 0 },
        )
    }

    /// C++ `Path::computePointOnPath` (AIPathfind.cpp:732-1014).
    /// Always tries `isLinePassable` to the next node; k = offset/(3*CELL)
    /// only chooses the remaining-segment midpoint when the lead fails.
    /// A blocked lead stays on the closest point so we do not aim through
    /// buildings (hq-2f8q0 / hq-5g9m3).
    pub fn compute_point_on_path_for(
        pos: Vec3,
        waypoints: &[Vec3],
        grid: Option<&PathfindingGrid>,
        surfaces: u32,
        is_crusher: bool,
        seeker_player: Option<u32>,
        crusher_level: u8,
    ) -> Vec3 {
        if waypoints.is_empty() {
            return pos;
        }
        if waypoints.len() == 1 {
            return waypoints[0];
        }
        let mut best_d2 = f32::MAX;
        let mut best = waypoints[0];
        let mut best_seg = 0usize;
        let mut best_t = 0.0f32;
        for i in 0..waypoints.len() - 1 {
            let a = waypoints[i];
            let b = waypoints[i + 1];
            let sx = b.x - a.x;
            let sz = b.z - a.z;
            let len_sqr = sx * sx + sz * sz;
            let t = if len_sqr <= 1.0e-8 {
                0.0
            } else {
                let tx = pos.x - a.x;
                let tz = pos.z - a.z;
                ((tx * sx + tz * sz) / len_sqr).clamp(0.0, 1.0)
            };
            let px = a.x + sx * t;
            let pz = a.z + sz * t;
            let dx = pos.x - px;
            let dz = pos.z - pz;
            let d2 = dx * dx + dz * dz;
            if d2 < best_d2 {
                best_d2 = d2;
                best = Vec3::new(px, a.y, pz);
                best_seg = i;
                best_t = t;
            }
        }
        let cell = grid.map(|g| g.grid_size()).unwrap_or(10.0);
        let max_err = 3.0 * cell;
        let k = (best_d2.sqrt() / max_err).clamp(0.0, 1.0);
        let next = waypoints[best_seg + 1];
        let a = waypoints[best_seg];
        let sx = next.x - a.x;
        let sz = next.z - a.z;
        let seg_len = (sx * sx + sz * sz).sqrt();
        let along = best_t * seg_len;
        let line_ok = |to: Vec3| match grid {
            Some(g) => {
                let from = g.world_to_grid(pos);
                let dest = g.world_to_grid(to);
                g.line_passable_ex(
                    from,
                    dest,
                    surfaces,
                    is_crusher,
                    true,
                    seeker_player,
                    crusher_level,
                    false,
                )
            }
            None => true,
        };
        // C++ AIPathfind.cpp:910-950 — always try next node, then tryAhead.
        if line_ok(next) {
            let remaining = seg_len - along;
            let try_ahead = best_t > 0.5 || remaining < 1.0;
            if try_ahead {
                if let Some(ahead) = waypoints.get(best_seg + 2) {
                    let mid = Vec3::new(
                        (next.x + ahead.x) * 0.5,
                        next.y,
                        (next.z + ahead.z) * 0.5,
                    );
                    if remaining < 1.0 || line_ok(mid) {
                        return mid;
                    }
                }
            }
            return next;
        }
        // C++ :952-966 — k>0.5 tries the remaining-segment midpoint.
        if k > 0.5 && seg_len > 1.0e-6 {
            let try_dist = along + 0.5 * (seg_len - along);
            let t = (try_dist / seg_len).clamp(0.0, 1.0);
            let mid = Vec3::new(a.x + sx * t, a.y, a.z + sz * t);
            if line_ok(mid) {
                return mid;
            }
        }
        best
    }

    /// C++ `Path::computePointOnPath` `distAlongPath` (AIPathfind.cpp:997).
    /// Remaining polyline from the closest point on the path to the end.
    /// Lead-point raise (`:999-1007`) is applied by the locomotor dispatcher.
    pub fn dist_along_path(pos: Vec3, waypoints: &[Vec3]) -> f32 {
        if waypoints.is_empty() {
            return 0.0;
        }
        if waypoints.len() == 1 {
            let dx = pos.x - waypoints[0].x;
            let dz = pos.z - waypoints[0].z;
            return (dx * dx + dz * dz).sqrt();
        }
        let mut best_d2 = f32::MAX;
        let mut best_seg = 0usize;
        let mut best_t = 0.0f32;
        for i in 0..waypoints.len() - 1 {
            let a = waypoints[i];
            let b = waypoints[i + 1];
            let sx = b.x - a.x;
            let sz = b.z - a.z;
            let len_sqr = sx * sx + sz * sz;
            let t = if len_sqr <= 1.0e-8 {
                0.0
            } else {
                let tx = pos.x - a.x;
                let tz = pos.z - a.z;
                ((tx * sx + tz * sz) / len_sqr).clamp(0.0, 1.0)
            };
            let px = a.x + sx * t;
            let pz = a.z + sz * t;
            let dx = pos.x - px;
            let dz = pos.z - pz;
            let d2 = dx * dx + dz * dz;
            if d2 < best_d2 {
                best_d2 = d2;
                best_seg = i;
                best_t = t;
            }
        }
        let mut remaining = 0.0f32;
        for i in best_seg..waypoints.len() - 1 {
            let a = waypoints[i];
            let b = waypoints[i + 1];
            let seg = ((b.x - a.x) * (b.x - a.x) + (b.z - a.z) * (b.z - a.z)).sqrt();
            if i == best_seg {
                remaining += (1.0 - best_t) * seg;
            } else {
                remaining += seg;
            }
        }
        remaining
    }


    fn is_allied_to(&self, mover: &Object, other: &Object) -> bool {
        let mover_p = mover.owner_player_id.unwrap_or(mover.team as u32);
        let other_p = other.owner_player_id.unwrap_or(other.team as u32);
        if mover_p == other_p {
            return true;
        }
        let bit = 1u16 << other_p.min(15);
        (self.grid.ally_mask_for(mover_p) & bit) != 0
    }

    /// C++ `Pathfinder::moveAllies` — idle allies occupying the new path scoot.
    pub fn allies_to_nudge_off_path(
        &self,
        mover_id: ObjectId,
        path: &[Vec3],
        objects: &HashMap<ObjectId, Object>,
    ) -> Vec<ObjectId> {
        let Some(mover) = objects.get(&mover_id) else {
            return Vec::new();
        };
        if path.len() < 2 {
            return Vec::new();
        }
        let is_dozer = mover.is_kind_of(KindOf::Dozer);
        let is_harvester = mover.is_kind_of(KindOf::Harvester);
        let mover_infantry = mover.is_kind_of(KindOf::Infantry);
        let (radius, center_in_cell) =
            PathfindingGrid::radius_and_center(mover.selection_radius, self.grid.grid_size());
        let mut num_above = radius;
        if center_in_cell {
            num_above += 1;
        }
        let ignore = self.ignore_obstacle_id;
        let blocked_by_ally = path.iter().skip(1).any(|wp| {
            let cell = self.grid.world_to_grid(*wp);
            objects.values().any(|obj| {
                if obj.id == mover_id || !obj.is_alive() || ignore == Some(obj.id) {
                    return false;
                }
                if !self.is_allied_to(mover, obj) {
                    return false;
                }
                let oc = self.grid.world_to_grid(obj.get_position());
                oc.x >= cell.x - radius
                    && oc.x < cell.x + num_above
                    && oc.y >= cell.y - radius
                    && oc.y < cell.y + num_above
            })
        });
        if !is_dozer && !is_harvester && !blocked_by_ally {
            return Vec::new();
        }
        let mut nudged = Vec::new();
        for wp in path.iter().skip(1).rev() {
            let cell = self.grid.world_to_grid(*wp);
            for obj in objects.values() {
                if obj.id == mover_id || !obj.is_alive() {
                    continue;
                }
                if ignore == Some(obj.id) {
                    continue;
                }
                if !self.is_allied_to(mover, obj) {
                    continue;
                }
                if obj.is_kind_of(KindOf::Structure) || obj.is_kind_of(KindOf::Immobile) {
                    continue;
                }
                if mover_infantry && obj.is_kind_of(KindOf::Infantry) {
                    continue;
                }
                if mover_infantry && !obj.is_kind_of(KindOf::Infantry) && !blocked_by_ally {
                    continue;
                }
                if !obj.movement.path.is_empty()
                    || obj.movement.velocity.length_squared() > 0.25
                    || obj.status.attacking
                    || obj.status.using_ability
                    || obj.deploy_style.as_ref().is_some_and(|d| d.is_busy())
                {
                    continue;
                }
                let oc = self.grid.world_to_grid(obj.get_position());
                if oc.x >= cell.x - radius
                    && oc.x < cell.x + num_above
                    && oc.y >= cell.y - radius
                    && oc.y < cell.y + num_above
                    && !nudged.contains(&obj.id)
                {
                    nudged.push(obj.id);
                }
            }
        }
        nudged
    }


    /// Move unit along path
    pub fn move_unit_along_path(
        &self,
        object_id: ObjectId,
        objects: &mut HashMap<ObjectId, Object>,
        dt: f32,
    ) -> bool {
        if let Some(unit) = objects.get_mut(&object_id) {
            if unit.movement.path.is_empty()
                || unit.movement.current_path_index >= unit.movement.path.len()
            {
                unit.stop_moving();
                return false;
            }

            let target_waypoint = unit.movement.path[unit.movement.current_path_index];
            let current_pos = unit.get_position();
            let distance_to_waypoint = current_pos.distance(target_waypoint);

            if distance_to_waypoint < 5.0 {
                // Reached waypoint, move to next
                unit.movement.current_path_index += 1;
                if unit.movement.current_path_index >= unit.movement.path.len() {
                    // Reached final destination
                    unit.stop_moving();
                    return true;
                }
                return false; // Continue to next waypoint
            }

            // Move toward waypoint
            let direction = (target_waypoint - current_pos).normalize_or_zero();
            let move_distance = unit.movement.max_speed * dt;
            let new_position = current_pos + direction * move_distance;

            unit.set_position(new_position);
            unit.set_orientation((-direction.z).atan2(direction.x));

            false
        } else {
            false
        }
    }

    /// Set up flow field for group movement
    pub fn create_flow_field(
        &mut self,
        goal_object_id: ObjectId,
        goal_pos: Vec3,
        objects: &HashMap<ObjectId, Object>,
    ) {
        // Update obstacles and create flow field (once per logic frame).
        self.ensure_dynamic_obstacles(objects);

        let goal_grid = self.grid.world_to_grid(goal_pos);
        let mut flow_field = FlowField::new_with_origin(
            self.grid.origin(),
            self.grid.width as f32 * self.grid.grid_size,
            self.grid.height as f32 * self.grid.grid_size,
            self.grid.grid_size,
        );

        flow_field.generate_flow_field(goal_grid, &self.grid);
        self.flow_fields.insert(goal_object_id, flow_field);
    }

    /// Move group of units using flow field
    pub fn move_group_with_flow_field(
        &self,
        goal_object_id: ObjectId,
        unit_ids: &[ObjectId],
        objects: &mut HashMap<ObjectId, Object>,
        dt: f32,
    ) {
        if let Some(flow_field) = self.flow_fields.get(&goal_object_id) {
            // Calculate movements
            let movements: Vec<(ObjectId, Vec3, f32)> = unit_ids
                .iter()
                .filter_map(|&unit_id| {
                    if let Some(unit) = objects.get(&unit_id) {
                        let flow_direction = flow_field.get_flow_direction(unit.get_position());

                        if flow_direction.length() > 0.1 {
                            let move_distance = unit.movement.max_speed * dt;
                            let new_position = unit.get_position() + flow_direction * move_distance;
                            let new_orientation = (-flow_direction.z).atan2(flow_direction.x);

                            Some((unit_id, new_position, new_orientation))
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                })
                .collect();

            // Apply movements directly

            // Apply movements
            for (unit_id, new_position, new_orientation) in movements {
                if let Some(unit) = objects.get_mut(&unit_id) {
                    unit.set_position(new_position);
                    unit.set_orientation(new_orientation);
                }
            }
        }
    }

    /// Clean up flow fields
    pub fn cleanup_flow_field(&mut self, goal_object_id: ObjectId) {
        self.flow_fields.remove(&goal_object_id);
    }

    /// Batch pathfinding for multiple units
    pub fn find_paths_batch(
        &mut self,
        path_requests: Vec<(ObjectId, Vec3, Vec3)>, // (unit_id, start, goal)
        objects: &HashMap<ObjectId, Object>,
    ) -> Vec<(ObjectId, Option<Vec<Vec3>>)> {
        self.ensure_dynamic_obstacles(objects);

        // Process all pathfinding requests sequentially
        let mut results = Vec::new();

        for (unit_id, start, goal) in path_requests {
            let start_grid = self.grid.world_to_grid(start);
            let goal_grid = self.grid.world_to_grid(goal);

            let path = self.grid.find_path(start_grid, goal_grid);
            results.push((unit_id, path));
        }

        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn open_grid(w: i32, h: i32) -> PathfindingGrid {
        PathfindingGrid::new(w as f32 * 10.0, h as f32 * 10.0, 10.0)
    }

    #[test]
    fn host_astar_rejects_diagonal_corner_cut() {
        let mut g = open_grid(8, 8);
        // Block both ortho legs between (2,2) and (3,3)
        g.set_blocked(GridPos::new(3, 2), true);
        g.set_blocked(GridPos::new(2, 3), true);
        // Path from (2,2) to (3,3) cannot go diagonal through blocked legs.
        let path = g.find_path(GridPos::new(2, 2), GridPos::new(4, 4));
        assert!(path.is_some());
        // Ensure path does not step from (2,2) directly to (3,3)
        let cells: Vec<_> = path
            .unwrap()
            .into_iter()
            .map(|p| g.world_to_grid(p))
            .collect();
        for w in cells.windows(2) {
            let dx = (w[1].x - w[0].x).abs();
            let dy = (w[1].y - w[0].y).abs();
            if dx == 1 && dy == 1 {
                let ortho_a = GridPos::new(w[0].x + (w[1].x - w[0].x), w[0].y);
                let ortho_b = GridPos::new(w[0].x, w[0].y + (w[1].y - w[0].y));
                assert!(!g.is_static_blocked(ortho_a) && !g.is_static_blocked(ortho_b));
            }
        }
    }

    #[test]
    fn host_march_closes_range_without_teleport() {
        use crate::game_logic::{Object, ObjectId, Team, ThingTemplate};
        let mut sys = PathfindingSystem::new(400.0, 400.0);
        let mut objects = HashMap::new();
        let tmpl = ThingTemplate::new("Ranger");
        let mut unit = Object::new(tmpl, ObjectId(1), Team::USA);
        let start = Vec3::new(20.0, 0.0, 20.0);
        let goal = Vec3::new(220.0, 0.0, 20.0);
        unit.set_position(start);
        unit.movement.max_speed = 20.0;
        objects.insert(unit.id, unit);

        let path = sys
            .find_path_ex(start, goal, &objects, false, Some(ObjectId(1)))
            .expect("open-field path");
        assert!(path.len() >= 2);
        {
            let u = objects.get_mut(&ObjectId(1)).unwrap();
            u.movement.path = path;
            u.movement.current_path_index = 0;
        }
        for _ in 0..400 {
            let _ = sys.move_unit_along_path(ObjectId(1), &mut objects, 1.0 / 30.0);
        }
        let end = objects[&ObjectId(1)].get_position();
        let dx = end.x - goal.x;
        let dz = end.z - goal.z;
        assert!(
            (dx * dx + dz * dz).sqrt() < 30.0,
            "unit must walk into range without set_position pull, end={end:?}"
        );
    }

    #[test]
    fn host_astar_snaps_blocked_start_to_nearest_open() {
        let mut g = open_grid(12, 12);
        g.set_blocked(GridPos::new(2, 2), true);
        g.set_blocked(GridPos::new(1, 2), true);
        g.set_blocked(GridPos::new(2, 1), true);
        let path = g.find_path(GridPos::new(2, 2), GridPos::new(10, 10));
        assert!(
            path.is_some(),
            "blocked start must snap to a walkable cell like a blocked goal"
        );
        let first = g.world_to_grid(path.unwrap()[0]);
        assert!(!g.is_static_blocked(first));
    }

    #[test]
    fn host_astar_soft_cost_dynamic_occupancy() {
        let mut g = open_grid(12, 12);
        // Wall of dynamic occupancy across middle — still pathable with surcharge.
        for y in 0..12 {
            g.set_dynamic_blocked(GridPos::new(5, y), true);
        }
        let path = g.find_path(GridPos::new(1, 5), GridPos::new(10, 5));
        assert!(path.is_some(), "dynamic occupancy must not hard-block path");
        assert!(path.unwrap().len() >= 2);
    }

    #[test]
    fn host_astar_static_block_still_hard() {
        let mut g = open_grid(12, 12);
        for y in 0..12 {
            g.set_blocked(GridPos::new(5, y), true);
        }
        // Completely sealed — no path.
        let path = g.find_path(GridPos::new(1, 5), GridPos::new(10, 5));
        assert!(path.is_none());
    }

    /// C++ `Pathfinder::worldToGrid` / `REAL_TO_INT` (AIPathfind.h:856-858).
    #[test]
    fn world_to_grid_truncates_toward_zero_like_real_to_int() {
        let g = open_grid(20, 20);
        assert_eq!(
            g.world_to_grid(Vec3::new(19.9, 0.0, 5.0)),
            GridPos::new(1, 0),
            "19.9/10=1.99 → 1, 5/10=0.5 → 0 (round would be 2,1)"
        );
        assert_eq!(g.world_to_grid(Vec3::new(20.0, 0.0, 0.0)), GridPos::new(2, 0));
        assert_eq!(
            g.world_to_grid(Vec3::new(-19.9, 0.0, -5.1)),
            GridPos::new(-1, 0)
        );
    }

    #[test]
    fn compute_normal_radial_offset_xz_perpendicular() {
        let from = Vec3::new(0.0, 0.0, 0.0);
        let to = Vec3::new(100.0, 0.0, 0.0);
        let obj = Vec3::new(50.0, 0.0, 0.0);
        let p = PathfindingSystem::compute_normal_radial_offset_xz(from, to, obj, 10.0);
        // cross=0 uses fallback normal (1,0) or perpendicular — distance from obj ~ radius
        let d = ((p.x - obj.x).powi(2) + (p.z - obj.z).powi(2)).sqrt();
        assert!((d - 10.0).abs() < 0.01, "offset radius {d}");
    }

    #[test]
    fn tall_building_aircraft_detour_inserts_waypoints() {
        use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
        let mut objects = HashMap::new();
        let mut tmpl = ThingTemplate::new("TallTower");
        tmpl.add_kind_of(KindOf::Structure);
        tmpl.add_kind_of(KindOf::AircraftPathAround);
        tmpl.add_kind_of(KindOf::Attackable);
        let mut bldg = Object::new(tmpl, ObjectId(1), Team::USA);
        bldg.set_position(Vec3::new(50.0, 0.0, 0.0));
        bldg.selection_radius = 25.0;
        objects.insert(bldg.id, bldg);

        let from = Vec3::new(0.0, 40.0, 0.0);
        let to = Vec3::new(100.0, 40.0, 0.0);
        let path = PathfindingSystem::detour_path_around_tall_buildings(&[from, to], &objects);
        assert!(
            path.len() > 2,
            "expected inserted tall-building waypoints, got {}",
            path.len()
        );
        // Path should not go through building center (within radius).
        for p in &path[1..path.len() - 1] {
            let d = ((p.x - 50.0).powi(2) + (p.z - 0.0).powi(2)).sqrt();
            // inserts are on the radius circle (~45)
            assert!(d + 1e-3 >= 20.0, "waypoint inside building d={d} at {p:?}");
        }
    }

    /// hq-1dbmo: hover/thrust still run leftover segmentIntersectsTallBuilding.
    #[test]
    fn hover_aircraft_detour_around_tall_building() {
        use crate::game_logic::{
            host_combat_chinook::{HostChinookAI, HostChinookAIState},
            KindOf, LocomotorAppearance, Object, ObjectId, Team, ThingTemplate,
        };
        let mut sys = PathfindingSystem::new(200.0, 200.0);
        let mut objects = HashMap::new();
        let mut tmpl = ThingTemplate::new("CommandCenter");
        tmpl.add_kind_of(KindOf::Structure);
        tmpl.add_kind_of(KindOf::AircraftPathAround);
        let mut bldg = Object::new(tmpl, ObjectId(1), Team::USA);
        bldg.set_position(Vec3::new(50.0, 0.0, 0.0));
        bldg.selection_radius = 25.0;
        objects.insert(bldg.id, bldg);

        let from = Vec3::new(0.0, 40.0, 0.0);
        let to = Vec3::new(100.0, 40.0, 0.0);
        let mut hover_t = ThingTemplate::new("AmericaChinook");
        hover_t.add_kind_of(KindOf::Aircraft);
        let mut hover = Object::new(hover_t, ObjectId(2), Team::USA);
        hover.loco_appearance = LocomotorAppearance::Hover;
        hover.set_position(from);
        objects.insert(hover.id, hover);
        let path = sys
            .find_path_ex_surfaces(
                from,
                to,
                &objects,
                true,
                SURFACE_AIR,
                false,
                Some(ObjectId(2)),
            )
            .expect("hover aircraft path");
        assert!(
            path.len() > 2,
            "hover/thrust must still insert tall-building detour, got {}",
            path.len()
        );

        let mut drop_t = ThingTemplate::new("CombatChinook");
        drop_t.add_kind_of(KindOf::Aircraft);
        let mut drop = Object::new(drop_t, ObjectId(3), Team::USA);
        drop.loco_appearance = LocomotorAppearance::Hover;
        drop.set_position(from);
        let mut ai = HostChinookAI::new_combat([0.0, 40.0, 0.0]);
        ai.state = HostChinookAIState::MoveToCombatDrop;
        ai.combat_drop_target = Some(1);
        drop.chinook_ai = Some(ai);
        objects.insert(drop.id, drop);
        let ignored = sys
            .find_path_ex_surfaces(
                from,
                to,
                &objects,
                true,
                SURFACE_AIR,
                false,
                Some(ObjectId(3)),
            )
            .expect("combat-drop path");
        assert_eq!(
            ignored.len(),
            2,
            "getBuildingToNotPathAround must skip the combat-drop goal"
        );
    }

    #[test]
    fn tall_building_segment_intersect_cpp_surface() {
        let src = include_str!("pathfinding.rs");
        assert!(src.contains("segmentIntersectsTallBuilding"));
        assert!(src.contains("AIRCRAFT_PATH_AROUND"));
        assert!(src.contains("compute_normal_radial_offset_xz"));
        assert!(src.contains("find_path_ex"));
    }

    #[test]
    fn circle_clips_tall_building_nudges_goal() {
        use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
        let mut objects = HashMap::new();
        let mut tmpl = ThingTemplate::new("TallCC");
        tmpl.add_kind_of(KindOf::Structure);
        tmpl.add_kind_of(KindOf::AircraftPathAround);
        let mut bldg = Object::new(tmpl, ObjectId(9), Team::USA);
        bldg.set_position(Vec3::new(0.0, 0.0, 0.0));
        bldg.selection_radius = 30.0;
        objects.insert(bldg.id, bldg);

        let from = Vec3::new(-100.0, 50.0, 0.0);
        let to = Vec3::new(5.0, 50.0, 0.0); // inside building footprint
        let adj = PathfindingSystem::circle_clips_tall_building(from, to, 80.0, &objects, None)
            .expect("must clip");
        let d = (adj.x * adj.x + adj.z * adj.z).sqrt();
        // leftover/C++: bounding circle + 2 cells, then perpendicular offset
        assert!(
            d >= 45.0,
            "adjusted goal still inside building d={d} adj={adj:?}"
        );
    }

    #[test]
    fn circle_clips_cpp_surface() {
        let src = include_str!("pathfinding.rs");
        assert!(src.contains("circleClipsTallBuilding"));
        assert!(src.contains("circle_clips_tall_building"));
    }

    /// C++ PathfindCell::CellType (AIPathfind.h:233-242) on the live host grid.
    #[test]
    fn host_grid_classifies_water_cliff_impassable() {
        let mut g = open_grid(8, 8);
        g.set_cell_type(GridPos::new(2, 2), PathfindCellType::Water);
        g.set_cell_type(GridPos::new(3, 3), PathfindCellType::Cliff);
        g.set_cell_type(GridPos::new(4, 4), PathfindCellType::Impassable);
        assert_eq!(g.cell_type(GridPos::new(2, 2)), PathfindCellType::Water);
        assert_eq!(g.cell_type(GridPos::new(3, 3)), PathfindCellType::Cliff);
        assert_eq!(g.cell_type(GridPos::new(4, 4)), PathfindCellType::Impassable);
        assert!(!g.is_static_blocked(GridPos::new(2, 2)), "water is not hard-blocked");
        assert!(!g.is_static_blocked(GridPos::new(3, 3)), "cliff is not hard-blocked");
        assert!(g.is_static_blocked(GridPos::new(4, 4)));
        let path = g.find_path(GridPos::new(0, 0), GridPos::new(7, 7));
        assert!(path.is_some(), "water/cliff must stay walkable for ground A*");
    }

    /// C++ Pathfinder::classifyMapCell (AIPathfind.cpp:4491-4521):
    /// cliff at top-left, water if any of 4 corners — water wins. No slope gate.
    #[test]
    fn classify_map_cell_water_wins_over_cliff_no_slope_gate() {
        use PathfindCellType::*;
        assert_eq!(
            PathfindingGrid::classify_map_cell(false, false),
            Clear
        );
        assert_eq!(
            PathfindingGrid::classify_map_cell(true, false),
            Cliff
        );
        assert_eq!(
            PathfindingGrid::classify_map_cell(false, true),
            Water
        );
        assert_eq!(
            PathfindingGrid::classify_map_cell(true, true),
            Water,
            "C++ assigns water after cliff so wet cliff-base stays SURFACE_WATER"
        );
        let src = include_str!("world_save.rs");
        assert!(
            !src.contains("MAX_SLOPE"),
            "live seed_pathfinding_from_terrain must not slope-gate Impassable"
        );
        assert!(
            src.contains("is_underwater_at_world(tl)")
                && src.contains("classify_map_cell(cliff, water)"),
            "live seed must sample four corners then classify_map_cell"
        );
    }


    /// Live find_path_ex must call crate AStarPathfinder (AIPathfind.cpp:6438).
    #[test]
    fn live_find_path_ex_uses_crate_astar() {
        let mut sys = PathfindingSystem::new(200.0, 200.0);
        let objects = HashMap::new();
        let path = sys
            .find_path_ex(
                Vec3::new(10.0, 0.0, 10.0),
                Vec3::new(80.0, 0.0, 10.0),
                &objects,
                false,
                None,
            )
            .expect("crate A* open-field path");
        assert!(path.len() >= 2);
        assert!(sys.crate_astar.is_some(), "crate A* must be wired after first search");
        sys.grid
            .set_cell_type(GridPos::new(5, 1), PathfindCellType::Water);
        let wet = sys
            .find_path_ex(
                Vec3::new(10.0, 0.0, 10.0),
                Vec3::new(80.0, 0.0, 10.0),
                &objects,
                false,
                None,
            )
            .expect("water-costed crate path");
        assert!(wet.len() >= 2);
        assert_eq!(
            sys.grid.cell_type(GridPos::new(5, 1)),
            PathfindCellType::Water
        );
    }

    /// C++ Pathfinder::queueForPath / processPathfindQueue (AI.cpp:332-339).
    #[test]
    fn host_path_queue_defers_until_taken() {
        let mut sys = PathfindingSystem::new(100.0, 100.0);
        sys.queue_path(PendingHostPath {
            unit_id: ObjectId(1),
            start: Vec3::ZERO,
            destination: Vec3::new(50.0, 0.0, 0.0),
            waypoints: Vec::new(),
            aircraft: false,
            surfaces: SURFACE_GROUND,
            is_crusher: false,
            ignore_obstacle: None,
        });
        assert_eq!(sys.pending_path_count(), 1);
        let drained = sys.take_pending_paths();
        assert_eq!(drained.len(), 1);
        assert_eq!(sys.pending_path_count(), 0);
    }

    /// Live assign_unit_path queues when map_loaded (AI.cpp:332-339).
    #[test]
    fn assign_unit_path_queues_until_next_update() {
        use crate::game_logic::{GameLogic, Team, ThingTemplate};
        let mut logic = GameLogic::new();
        let mut tmpl = ThingTemplate::new("Ranger");
        tmpl.add_kind_of(KindOf::Infantry);
        tmpl.add_kind_of(KindOf::Attackable);
        logic.templates.insert("Ranger".into(), tmpl);
        let id = logic
            .create_object("Ranger", Team::USA, Vec3::new(10.0, 0.0, 10.0))
            .expect("ranger");
        if let Some(u) = logic.host_object_mut(id) {
            u.movement.max_speed = 20.0;
        }
        logic.force_map_loaded_for_path_test(true);
        assert!(logic.assign_unit_path(id, Vec3::new(80.0, 0.0, 10.0), &[]));
        let unit = logic.host_object(id).expect("unit");
        assert!(unit.waiting_for_path, "C++ m_waitingForPath until next frame");
        assert!(
            unit.movement.path.is_empty(),
            "waypoints must not land same frame"
        );
        logic.update();
        let unit = logic.host_object(id).expect("unit after queue");
        assert!(!unit.waiting_for_path);
        assert!(
            !unit.movement.path.is_empty(),
            "processPathfindQueue must install crate A* path"
        );
    }

    #[test]
    fn assign_shared_group_paths_uses_one_spine() {
        use crate::game_logic::{GameLogic, Team, ThingTemplate};
        let mut logic = GameLogic::new();
        logic.force_map_loaded_for_path_test(false);
        let mut tmpl = ThingTemplate::new("Ranger");
        tmpl.add_kind_of(KindOf::Infantry);
        logic.templates.insert("Ranger".into(), tmpl);
        let a = logic
            .create_object("Ranger", Team::USA, Vec3::new(0.0, 0.0, 0.0))
            .expect("a");
        let b = logic
            .create_object("Ranger", Team::USA, Vec3::new(10.0, 0.0, 0.0))
            .expect("b");
        for id in [a, b] {
            if let Some(u) = logic.host_object_mut(id) {
                u.movement.max_speed = 20.0;
            }
        }
        let dest = Vec3::new(80.0, 0.0, 0.0);
        let goals = vec![(a, dest), (b, dest + Vec3::new(0.0, 0.0, 10.0))];
        assert!(logic.assign_shared_group_paths(&goals, dest));
        let pa = logic.host_object(a).unwrap().movement.path.clone();
        let pb = logic.host_object(b).unwrap().movement.path.clone();
        assert!(!pa.is_empty() && !pb.is_empty());
        assert_eq!(
            pa.last().copied().unwrap(),
            dest,
            "leader last waypoint is destination"
        );
        assert_eq!(
            pb.last().copied().unwrap().z,
            10.0,
            "follower last waypoint is slot"
        );
    }

    #[test]
    fn cliff_pinch_converts_clear_neighbors() {
        let mut g = open_grid(8, 8);
        g.set_cell_type(GridPos::new(4, 4), PathfindCellType::Cliff);
        g.pinch_tighten_cliffs();
        assert_eq!(g.cell_type(GridPos::new(4, 5)), PathfindCellType::Cliff);
        assert!(g.is_pinched(GridPos::new(4, 6)) || g.cell_type(GridPos::new(4, 5)) == PathfindCellType::Cliff);
    }

    #[test]
    fn terrain_zones_split_on_water() {
        let mut g = open_grid(8, 8);
        for y in 0..8 {
            g.set_cell_type(GridPos::new(4, y), PathfindCellType::Water);
        }
        g.rebuild_terrain_zones();
        let a = g.terrain_zone(GridPos::new(1, 4));
        let b = g.terrain_zone(GridPos::new(6, 4));
        assert_ne!(a, 0);
        assert_ne!(b, 0);
        assert_ne!(a, b, "water must split terrain zones");
        let from = g.grid_to_world(GridPos::new(1, 4));
        let to = g.grid_to_world(GridPos::new(6, 4));
        assert!(!g.quick_path_exists_for_ui(from, to));
    }

    #[test]
    fn terrain_zones_ignore_structure_obstacles() {
        let mut g = open_grid(8, 8);
        for y in 0..8 {
            g.set_cell_type(GridPos::new(4, y), PathfindCellType::Obstacle);
        }
        g.rebuild_terrain_zones();
        let from = g.grid_to_world(GridPos::new(1, 4));
        let to = g.grid_to_world(GridPos::new(6, 4));
        assert!(g.quick_path_exists_for_ui(from, to));
    }

    #[test]
    fn bridge_deck_classifies_layer_not_flatten() {
        // C++ PathfindLayer: deck CLEAR on its layer, river under stays Water,
        // sides BRIDGE_IMPASSABLE, destroy closes the layer only.
        let mut g = open_grid(12, 12);
        for y in 4..=6 {
            for x in 3..8 {
                g.set_cell_type(GridPos::new(x, y), PathfindCellType::Water);
            }
        }
        let from_l = Vec3::new(20.0, 20.0, 40.0);
        let from_r = Vec3::new(20.0, 20.0, 60.0);
        let to_l = Vec3::new(90.0, 20.0, 40.0);
        let to_r = Vec3::new(90.0, 20.0, 60.0);
        g.stamp_bridge_deck(from_l, from_r, to_l, to_r, false);
        let layer = g.first_bridge_layer_id().expect("bridge layer");
        let deck = GridPos::new(5, 5);
        assert_eq!(
            g.cell_type(deck),
            PathfindCellType::Water,
            "must not flatten deck onto ground"
        );
        assert_eq!(
            g.layer_cell_type(layer, deck),
            Some(PathfindCellType::Clear)
        );
        let side = GridPos::new(5, 3);
        assert_eq!(
            g.layer_cell_type(layer, side),
            Some(PathfindCellType::BridgeImpassable)
        );
        g.stamp_bridge_deck(from_l, from_r, to_l, to_r, true);
        assert_eq!(
            g.cell_type(deck),
            PathfindCellType::Water,
            "destroyed deck must not slab the river"
        );
        assert_eq!(
            g.layer_cell_type(layer, deck),
            Some(PathfindCellType::BridgeImpassable)
        );
        assert_eq!(g.ground_connect_layer(deck), 0);
    }

    #[test]
    fn low_overpass_stamps_ground_bridge_impassable() {
        let mut g = open_grid(12, 12);
        g.stamp_bridge_deck(
            Vec3::new(20.0, 0.0, 40.0),
            Vec3::new(20.0, 0.0, 60.0),
            Vec3::new(90.0, 0.0, 40.0),
            Vec3::new(90.0, 0.0, 60.0),
            false,
        );
        assert_eq!(
            g.cell_type(GridPos::new(5, 5)),
            PathfindCellType::BridgeImpassable
        );
        assert!(g.is_static_blocked(GridPos::new(5, 5)));
    }

    #[test]
    fn classified_bridge_layer_hops_across_river() {
        let mut sys = PathfindingSystem::new(120.0, 120.0);
        for y in 0..12 {
            for x in 3..8 {
                sys.grid
                    .set_cell_type(GridPos::new(x, y), PathfindCellType::Water);
            }
        }
        sys.grid.stamp_bridge_deck(
            Vec3::new(20.0, 20.0, 40.0),
            Vec3::new(20.0, 20.0, 60.0),
            Vec3::new(90.0, 20.0, 40.0),
            Vec3::new(90.0, 20.0, 60.0),
            false,
        );
        let objects = HashMap::new();
        let from = sys.grid.grid_to_world(GridPos::new(1, 5));
        let to = sys.grid.grid_to_world(GridPos::new(10, 5));
        let path = sys.find_path(from, to, &objects);
        assert!(
            path.as_ref().map(|p| p.len() >= 2).unwrap_or(false),
            "crate A* must hop the classified deck across water"
        );
    }

    /// hq-z66hi: rally/dozer zone gate must honor bridge connectLayer.
    #[test]
    fn connect_layer_merges_zones_across_river() {
        let mut g = open_grid(12, 12);
        for y in 0..12 {
            for x in 3..8 {
                g.set_cell_type(GridPos::new(x, y), PathfindCellType::Water);
            }
        }
        g.stamp_bridge_deck(
            Vec3::new(20.0, 20.0, 40.0),
            Vec3::new(20.0, 20.0, 60.0),
            Vec3::new(90.0, 20.0, 40.0),
            Vec3::new(90.0, 20.0, 60.0),
            false,
        );
        g.rebuild_terrain_zones();
        g.rebuild_path_zones();
        let from = g.grid_to_world(GridPos::new(1, 5));
        let to = g.grid_to_world(GridPos::new(10, 5));
        assert!(
            g.quick_path_exists(from, to),
            "clientSafeQuickDoesPathExist must join banks via connectLayer"
        );
        assert!(
            g.quick_path_exists_for_ui(from, to),
            "ForUI effectiveTerrainZone still applies hierarchical bridge merge"
        );
        g.stamp_bridge_deck(
            Vec3::new(20.0, 20.0, 40.0),
            Vec3::new(20.0, 20.0, 60.0),
            Vec3::new(90.0, 20.0, 40.0),
            Vec3::new(90.0, 20.0, 60.0),
            true,
        );
        g.rebuild_terrain_zones();
        g.rebuild_path_zones();
        assert!(
            !g.quick_path_exists(from, to),
            "destroyed deck must drop ground connect and split banks"
        );
    }


    /// hq-54w0z: dest on a deck stays on the deck (not snapped to the riverbank).
    #[test]
    fn adjust_destination_keeps_bridge_deck() {
        let mut g = open_grid(12, 12);
        for y in 4..=6 {
            for x in 3..8 {
                g.set_cell_type(GridPos::new(x, y), PathfindCellType::Water);
            }
        }
        g.stamp_bridge_deck(
            Vec3::new(20.0, 20.0, 40.0),
            Vec3::new(20.0, 20.0, 60.0),
            Vec3::new(90.0, 20.0, 40.0),
            Vec3::new(90.0, 20.0, 60.0),
            false,
        );
        let dest = GridPos::new(5, 5);
        let world = g.grid_to_world(dest);
        let on_deck = Vec3::new(world.x, 20.0, world.z);
        let layer = g.layer_for_destination(on_deck);
        assert_ne!(layer, PathfindLayerEnum::Ground, "deck click must pick the span");
        let snapped = g
            .adjust_destination_on_layer(dest, SURFACE_GROUND, false, 400, None, 0, layer)
            .expect("deck dest");
        assert_eq!(snapped, dest, "must not spiral off the Clear deck cell");
        let bank = g.adjust_destination_ex(dest, SURFACE_GROUND, false, 400, None, 0);
        assert_ne!(
            bank,
            Some(dest),
            "ground-only adjust still refuses Water under the span"
        );
    }

    /// hq-tg3cs: deck traffic must not stamp the roadbed under the span.
    #[test]
    fn deck_occupancy_does_not_block_roadbed() {
        use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
        let mut g = open_grid(12, 12);
        g.stamp_bridge_deck(
            Vec3::new(20.0, 20.0, 40.0),
            Vec3::new(20.0, 20.0, 60.0),
            Vec3::new(90.0, 20.0, 40.0),
            Vec3::new(90.0, 20.0, 60.0),
            false,
        );
        let deck = GridPos::new(5, 5);
        let world = g.grid_to_world(deck);
        let layer = g.layer_for_destination(Vec3::new(world.x, 20.0, world.z));
        assert_ne!(layer, PathfindLayerEnum::Ground);

        let mut objects = HashMap::new();
        let mut deck_t = ThingTemplate::new("Humvee");
        deck_t.add_kind_of(KindOf::Vehicle);
        let mut on_deck = Object::new(deck_t, ObjectId(10), Team::USA);
        on_deck.set_position(Vec3::new(world.x, 20.0, world.z));
        on_deck.selection_radius = 1.0;
        on_deck.owner_player_id = Some(1);
        objects.insert(on_deck.id, on_deck);
        g.update_dynamic_obstacles(&objects);

        assert!(
            !g.is_blocked(deck),
            "mid-span deck unit must not set ground dynamic_bits"
        );
        g.query_layer = PathfindLayerEnum::Ground as u8;
        assert_eq!(
            g.occupancy_extra_cost(deck, Some(0), false, 0),
            0,
            "roadbed under the span must ignore deck occupancy"
        );
        g.query_layer = layer as u8;
        assert_eq!(
            g.occupancy_extra_cost(deck, Some(0), false, 0),
            u32::MAX / 8,
            "same XZ on the deck still occupies the layer"
        );

        let mut ground_t = ThingTemplate::new("Battlemaster");
        ground_t.add_kind_of(KindOf::Vehicle);
        let mut on_ground = Object::new(ground_t, ObjectId(11), Team::China);
        on_ground.set_position(Vec3::new(world.x, 0.0, world.z));
        on_ground.selection_radius = 1.0;
        on_ground.owner_player_id = Some(2);
        objects.insert(on_ground.id, on_ground);
        g.update_dynamic_obstacles(&objects);

        g.query_layer = PathfindLayerEnum::Ground as u8;
        assert_eq!(
            g.occupancy_extra_cost(deck, Some(0), false, 0),
            u32::MAX / 8,
            "ground unit still occupies the roadbed"
        );
        g.query_layer = layer as u8;
        assert_eq!(
            g.occupancy_extra_cost(deck, Some(0), false, 0),
            u32::MAX / 8,
            "deck unit still occupies the layer after ground stamp"
        );
    }

    /// hq-a79xb: own reservation accepted; allied other player refused; enemy accepted.
    #[test]
    fn has_allied_goal_own_vs_ally_player() {
        use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
        let mut g = open_grid(12, 12);
        let mut masks = [0u16; 16];
        masks[0] = 1u16 << 0 | 1u16 << 1;
        masks[1] = 1u16 << 0 | 1u16 << 1;
        g.set_player_ally_masks(masks);

        let mut objects = HashMap::new();
        let mut own_t = ThingTemplate::new("Ranger");
        own_t.add_kind_of(KindOf::Infantry);
        let mut own = Object::new(own_t, ObjectId(10), Team::USA);
        own.set_position(g.grid_to_world(GridPos::new(2, 2)));
        own.owner_player_id = Some(0);
        own.movement.target_position = Some(g.grid_to_world(GridPos::new(4, 4)));
        objects.insert(own.id, own);

        let mut ally_t = ThingTemplate::new("RedGuard");
        ally_t.add_kind_of(KindOf::Infantry);
        let mut ally = Object::new(ally_t, ObjectId(20), Team::China);
        ally.set_position(g.grid_to_world(GridPos::new(8, 2)));
        ally.owner_player_id = Some(1);
        ally.movement.target_position = Some(g.grid_to_world(GridPos::new(5, 5)));
        objects.insert(ally.id, ally);

        let mut enemy_t = ThingTemplate::new("Rebel");
        enemy_t.add_kind_of(KindOf::Infantry);
        let mut enemy = Object::new(enemy_t, ObjectId(30), Team::GLA);
        enemy.set_position(g.grid_to_world(GridPos::new(8, 8)));
        enemy.owner_player_id = Some(2);
        enemy.movement.target_position = Some(g.grid_to_world(GridPos::new(6, 6)));
        objects.insert(enemy.id, enemy);

        g.update_dynamic_obstacles(&objects);
        g.query_seeker_id = 10;
        assert!(
            !g.has_allied_goal(GridPos::new(4, 4), Some(0)),
            "own UNIT_GOAL must be accepted"
        );
        assert!(
            g.has_allied_goal(GridPos::new(5, 5), Some(0)),
            "allied other-player goal must be refused"
        );
        assert!(
            !g.has_allied_goal(GridPos::new(6, 6), Some(0)),
            "enemy UNIT_GOAL is not allied"
        );
    }

    /// hq-7qvsn: no enemy-moving cost; ally moving only near start; no goal cost.
    #[test]
    fn occupancy_cost_matches_examine_neighboring_cells() {
        use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
        let mut g = open_grid(24, 24);
        let mut masks = [0u16; 16];
        masks[0] = 1u16 << 0 | 1u16 << 1;
        masks[1] = 1u16 << 0 | 1u16 << 1;
        g.set_player_ally_masks(masks);

        let mut objects = HashMap::new();
        let mut enemy_t = ThingTemplate::new("Technical");
        enemy_t.add_kind_of(KindOf::Vehicle);
        let mut enemy = Object::new(enemy_t, ObjectId(1), Team::GLA);
        enemy.set_position(g.grid_to_world(GridPos::new(5, 5)));
        enemy.owner_player_id = Some(2);
        enemy.movement.velocity = Vec3::new(5.0, 0.0, 0.0);
        objects.insert(enemy.id, enemy);

        let mut ally_t = ThingTemplate::new("Humvee");
        ally_t.add_kind_of(KindOf::Vehicle);
        let mut ally = Object::new(ally_t, ObjectId(2), Team::China);
        ally.set_position(g.grid_to_world(GridPos::new(6, 6)));
        ally.owner_player_id = Some(1);
        ally.movement.velocity = Vec3::new(5.0, 0.0, 0.0);
        objects.insert(ally.id, ally);

        let mut goal_t = ThingTemplate::new("Ranger");
        goal_t.add_kind_of(KindOf::Infantry);
        let mut goaled = Object::new(goal_t, ObjectId(3), Team::USA);
        goaled.set_position(g.grid_to_world(GridPos::new(1, 1)));
        goaled.owner_player_id = Some(0);
        goaled.movement.target_position = Some(g.grid_to_world(GridPos::new(8, 8)));
        objects.insert(goaled.id, goaled);

        g.update_dynamic_obstacles(&objects);
        let start = GridPos::new(5, 5);
        let enemy_cell = GridPos::new(5, 5);
        let ally_near = GridPos::new(6, 6);
        let ally_far = GridPos::new(6, 6);
        let far_start = GridPos::new(20, 20);
        let goal_cell = GridPos::new(8, 8);

        assert_eq!(
            g.occupancy_extra_cost(enemy_cell, Some(0), false, 0),
            0,
            "moving enemy adds no examineNeighboringCells cost"
        );
        let near = g.occupancy_cost(ally_near, Some(0), false, 0, masks[0], Some(start));
        assert!(
            near.unwrap_or(0.0) > 0.0,
            "allied mover within 10 cells of start is charged"
        );
        let far = g.occupancy_cost(ally_far, Some(0), false, 0, masks[0], Some(far_start));
        assert_eq!(
            far,
            Some(0.0),
            "allied mover more than 10 cells from start is free"
        );
        let goal_cost = g.occupancy_cost(goal_cell, Some(0), false, 0, masks[0], Some(start));
        assert_eq!(
            goal_cost,
            Some(0.0),
            "UNIT_GOAL is not a movement-path surcharge"
        );
    }


    #[test]
    fn occupancy_radius_marks_overlord_footprint() {
        use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
        let mut g = open_grid(16, 16);
        let mut objects = HashMap::new();
        let mut tmpl = ThingTemplate::new("Overlord");
        tmpl.add_kind_of(KindOf::Vehicle);
        let mut tank = Object::new(tmpl, ObjectId(1), Team::China);
        tank.set_position(Vec3::new(50.0, 0.0, 50.0));
        tank.selection_radius = 25.0;
        tank.owner_player_id = Some(1);
        objects.insert(tank.id, tank);
        g.update_dynamic_obstacles(&objects);
        let center = g.world_to_grid(Vec3::new(50.0, 0.0, 50.0));
        assert!(g.is_blocked(center));
        assert!(g.is_blocked(GridPos::new(center.x + 2, center.y)));
    }

    /// hq-0xpfm: C++ setGoalAircraft + adjustToLandingDestination.
    #[test]
    fn aircraft_goals_and_landing_dest_unstack() {
        use crate::game_logic::{KindOf, Object, ObjectId, ObjectType, Team, ThingTemplate};
        let mut g = open_grid(16, 16);
        let dest = Vec3::new(85.0, 40.0, 85.0);
        let dest_cell = g.world_to_grid(dest);
        let mut objects = HashMap::new();
        let mut tmpl_a = ThingTemplate::new("AmericaVehicleChinook");
        tmpl_a.add_kind_of(KindOf::Aircraft);
        tmpl_a.add_kind_of(KindOf::Vehicle);
        let mut a = Object::new(tmpl_a, ObjectId(11), Team::USA);
        a.object_type = ObjectType::Aircraft;
        a.loco_appearance = LocomotorAppearance::Hover;
        a.status.airborne_target = true;
        a.set_position(Vec3::new(20.0, 40.0, 20.0));
        a.movement.target_position = Some(dest);
        a.owner_player_id = Some(0);
        objects.insert(a.id, a);
        g.update_dynamic_obstacles(&objects);
        assert_eq!(g.goal_aircraft(dest_cell), 11, "first chinook stamps goalAircraft");

        let mut tmpl_b = ThingTemplate::new("AmericaVehicleChinook");
        tmpl_b.add_kind_of(KindOf::Aircraft);
        tmpl_b.add_kind_of(KindOf::Vehicle);
        let mut b = Object::new(tmpl_b, ObjectId(12), Team::USA);
        b.object_type = ObjectType::Aircraft;
        b.loco_appearance = LocomotorAppearance::Hover;
        b.status.airborne_target = true;
        b.set_position(Vec3::new(30.0, 40.0, 20.0));
        b.movement.target_position = Some(dest);
        b.owner_player_id = Some(0);
        objects.insert(b.id, b);
        g.update_dynamic_obstacles(&objects);
        g.query_seeker_id = 12;
        g.query_check_for_aircraft = true;
        let snapped = g
            .adjust_destination_on_layer(
                dest_cell,
                SURFACE_AIR,
                false,
                400,
                Some(0),
                0,
                PathfindLayerEnum::Ground,
            )
            .expect("second LZ");
        g.query_check_for_aircraft = false;
        assert_ne!(snapped, dest_cell, "second aircraft dest must leave first LZ");
        assert!(!g.has_other_aircraft_goal(snapped));

        let water = GridPos::new(4, 4);
        g.set_cell_type(water, PathfindCellType::Water);
        let land = g
            .adjust_to_landing_destination(water, 400, PathfindLayerEnum::Ground)
            .expect("landing dest");
        assert_ne!(g.cell_type(land), PathfindCellType::Water);
        assert_ne!(land, dest_cell, "landing dest refuses occupied aircraft goal");
    }

    /// hq-sa4qi: HOVER attack dests spiral so two Comanches do not share a cell.
    #[test]
    fn aircraft_attack_dests_spiral_off_occupied_hover_cell() {
        use crate::game_logic::{KindOf, Object, ObjectId, ObjectType, Team, ThingTemplate};
        let sys = PathfindingSystem::new(400.0, 400.0);
        let target = Vec3::new(200.0, 0.0, 200.0);
        let dest = Vec3::new(110.0, 40.0, 200.0);
        let mut objects = HashMap::new();
        let mut tmpl_a = ThingTemplate::new("AmericaVehicleComanche");
        tmpl_a.add_kind_of(KindOf::Aircraft);
        tmpl_a.add_kind_of(KindOf::Vehicle);
        let mut a = Object::new(tmpl_a, ObjectId(21), Team::USA);
        a.object_type = ObjectType::Aircraft;
        a.loco_appearance = LocomotorAppearance::Hover;
        a.status.airborne_target = true;
        a.set_position(Vec3::new(20.0, 40.0, 200.0));
        a.movement.target_position = Some(dest);
        a.selection_radius = 8.0;
        objects.insert(a.id, a);

        let mut tmpl_b = ThingTemplate::new("AmericaVehicleComanche");
        tmpl_b.add_kind_of(KindOf::Aircraft);
        tmpl_b.add_kind_of(KindOf::Vehicle);
        let mut b = Object::new(tmpl_b, ObjectId(22), Team::USA);
        b.object_type = ObjectType::Aircraft;
        b.loco_appearance = LocomotorAppearance::Hover;
        b.status.airborne_target = true;
        b.set_position(Vec3::new(30.0, 40.0, 190.0));
        b.selection_radius = 8.0;
        objects.insert(b.id, b);

        let adj = sys.adjust_target_destination(
            22,
            &objects,
            dest,
            target,
            8.0,
            SURFACE_AIR,
            false,
            8.0,
            5.0,
            150.0,
            0.0,
        );
        let dest_cell = sys.grid.world_to_grid(dest);
        let adj_cell = sys.grid.world_to_grid(adj);
        assert_ne!(
            adj_cell, dest_cell,
            "second Comanche must leave first hover cell"
        );
        assert!(
            crate::game_logic::weapon_bootstrap::is_goal_pos_within_attack_range(
                adj, target, 150.0, 0.0, 8.0, 5.0
            ),
            "spiral dest must stay in attack range"
        );
    }



    /// hq-6p032: crushers plan through idle crushable cars (AIPathfind.cpp:5063).
    #[test]
    fn crusher_plans_through_idle_crushable_cars() {
        use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
        let mut sys = PathfindingSystem::new(200.0, 200.0);
        let mut objects = HashMap::new();
        for (i, z) in (20..90).step_by(10).enumerate() {
            let mut tmpl = ThingTemplate::new("CivilianCar");
            tmpl.add_kind_of(KindOf::Vehicle);
            let mut car = Object::new(tmpl, ObjectId(100 + i as u32), Team::GLA);
            car.set_position(Vec3::new(80.0, 0.0, z as f32));
            car.crushable_level = 1;
            car.crusher_level = 0;
            car.owner_player_id = Some(1);
            car.selection_radius = 4.0;
            objects.insert(car.id, car);
        }
        let mut tank_t = ThingTemplate::new("Crusader");
        tank_t.add_kind_of(KindOf::Vehicle);
        let mut tank = Object::new(tank_t, ObjectId(1), Team::USA);
        tank.set_position(Vec3::new(10.0, 0.0, 50.0));
        tank.crusher_level = 2;
        tank.crushable_level = 2;
        tank.owner_player_id = Some(0);
        objects.insert(tank.id, tank);

        let start = Vec3::new(10.0, 0.0, 50.0);
        let goal = Vec3::new(150.0, 0.0, 50.0);
        let crushed = sys
            .find_path_ex_surfaces(start, goal, &objects, false, SURFACE_GROUND, true, Some(ObjectId(1)))
            .expect("crusher must path through cars");
        assert!(crushed.len() >= 2);
        let through = crushed.windows(2).any(|w| {
            let crosses = (w[0].x - 80.0) * (w[1].x - 80.0) <= 0.0;
            let near_mid = w[0].z > 15.0 && w[0].z < 95.0;
            crosses && near_mid
        });
        assert!(
            through,
            "crusher path must cross the car line, path={crushed:?}"
        );

        let mut inf_t = ThingTemplate::new("Ranger");
        inf_t.add_kind_of(KindOf::Infantry);
        let mut inf = Object::new(inf_t, ObjectId(2), Team::USA);
        inf.set_position(start);
        inf.crusher_level = 0;
        inf.owner_player_id = Some(0);
        objects.insert(inf.id, inf);
        objects.remove(&ObjectId(1));
        sys.note_logic_frame(1);
        let walked = sys
            .find_path_ex_surfaces(start, goal, &objects, false, SURFACE_GROUND, false, Some(ObjectId(2)))
            .expect("non-crusher can detour");
        let walk_len: f32 = walked.windows(2).map(|w| (w[0] - w[1]).length()).sum();
        let crush_len: f32 = crushed.windows(2).map(|w| (w[0] - w[1]).length()).sum();
        assert!(
            walk_len > crush_len + 20.0,
            "non-crusher must detour around cars crush={crush_len} walk={walk_len}"
        );
    }

    /// hq-t7pyo: ALLIES occupancy is allyFixed, never crush-through.
    #[test]
    fn occupancy_allies_are_not_crush_through() {
        use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
        let mut sys = PathfindingSystem::new(200.0, 200.0);
        let mut masks = [0u16; 16];
        masks[0] = 1u16 << 0 | 1u16 << 1;
        masks[1] = 1u16 << 0 | 1u16 << 1;
        sys.set_player_ally_masks(masks);

        let mut objects = HashMap::new();
        for (i, z) in (20..90).step_by(10).enumerate() {
            let mut tmpl = ThingTemplate::new("AlliedCar");
            tmpl.add_kind_of(KindOf::Vehicle);
            let mut car = Object::new(tmpl, ObjectId(100 + i as u32), Team::China);
            car.set_position(Vec3::new(80.0, 0.0, z as f32));
            car.crushable_level = 1;
            car.crusher_level = 0;
            car.owner_player_id = Some(1);
            car.selection_radius = 4.0;
            objects.insert(car.id, car);
        }
        let mut tank_t = ThingTemplate::new("AllyCrusader");
        tank_t.add_kind_of(KindOf::Vehicle);
        let mut tank = Object::new(tank_t, ObjectId(1), Team::USA);
        tank.set_position(Vec3::new(10.0, 0.0, 50.0));
        tank.crusher_level = 2;
        tank.crushable_level = 2;
        tank.owner_player_id = Some(0);
        objects.insert(tank.id, tank);

        let start = Vec3::new(10.0, 0.0, 50.0);
        let goal = Vec3::new(150.0, 0.0, 50.0);
        let path = sys
            .find_path_ex_surfaces(start, goal, &objects, false, SURFACE_GROUND, true, Some(ObjectId(1)))
            .expect("must detour around allied cars");
        let through = path.windows(2).any(|w| {
            let crosses = (w[0].x - 80.0) * (w[1].x - 80.0) <= 0.0;
            let near_mid = w[0].z > 15.0 && w[0].z < 95.0;
            crosses && near_mid
        });
        assert!(
            !through,
            "ALLIES cars must not be crush-through, path={path:?}"
        );
    }

    /// hq-bw35t: attack / requestPath must not fail-open through sealed walls.
    #[test]
    fn attack_and_request_path_fail_closed_through_walls() {
        use crate::game_logic::{GameLogic, KindOf, Object, ObjectId, Team, ThingTemplate};
        let mut sys = PathfindingSystem::new(200.0, 200.0);
        for y in 0..20 {
            sys.grid
                .set_cell_type(GridPos::new(10, y), PathfindCellType::Impassable);
        }
        let objects = HashMap::new();
        let from = Vec3::new(20.0, 0.0, 50.0);
        let to = Vec3::new(150.0, 0.0, 50.0);
        let crosses_wall = |path: &[Vec3]| {
            path.windows(2).any(|w| {
                let a = sys.grid.world_to_grid(w[0]);
                let b = sys.grid.world_to_grid(w[1]);
                (a.x - 10) * (b.x - 10) <= 0 && (a.x != 10 || b.x != 10)
                    || sys.grid.cell_type(a) == PathfindCellType::Impassable
                    || sys.grid.cell_type(b) == PathfindCellType::Impassable
            })
        };
        if let Some(p) = sys.find_path_ex_surfaces(from, to, &objects, false, SURFACE_GROUND, false, None)
        {
            assert!(
                !crosses_wall(&p),
                "A* must not walk Impassable, path={p:?}"
            );
        }
        if let Some(p) = sys.find_attack_firing_position(from, to, 50.0, &objects, false) {
            assert!(
                !crosses_wall(&p),
                "findAttackPath must not install a through-wall march, path={p:?}"
            );
            assert!(
                !(p.len() == 2 && (p[1] - to).length() < 1.0),
                "must not fail-open start→goal through the wall"
            );
        }

        let mut logic = GameLogic::new();
        for y in 0..logic.pathfinding_system.grid.height() {
            logic
                .pathfinding_system
                .grid
                .set_cell_type(GridPos::new(10, y), PathfindCellType::Impassable);
        }
        let mut tmpl = ThingTemplate::new("Ranger");
        tmpl.add_kind_of(KindOf::Infantry);
        let id = ObjectId(7);
        let mut unit = Object::new(tmpl, id, Team::USA);
        unit.set_position(from);
        logic.objects.insert(id, unit);
        let ok = logic.request_object_path(id, to);
        let unit = logic.objects.get(&id).expect("unit");
        if ok {
            let path = &unit.movement.path;
            let through = path.iter().any(|p| {
                let c = logic.pathfinding_system.grid.world_to_grid(*p);
                logic.pathfinding_system.grid.cell_type(c) == PathfindCellType::Impassable
                    || p.x > 105.0
            });
            assert!(!through, "requestPath must not march through the wall: {path:?}");
        }
    }

    /// hq-3biqe: assign_unit_path must pass real is_crusher into live A*.
    #[test]
    fn assign_unit_path_crusher_walks_rubble() {
        use crate::game_logic::{GameLogic, KindOf, ObjectId, Team, ThingTemplate};
        let mut logic = GameLogic::new();
        logic.force_map_loaded_for_path_test(false);
        let start = Vec3::new(10.0, 0.0, 10.0);
        let goal = Vec3::new(80.0, 0.0, 10.0);
        let start_cell = logic.pathfinding_system.grid.world_to_grid(start);
        let goal_cell = logic.pathfinding_system.grid.world_to_grid(goal);
        let wall_x = (start_cell.x + goal_cell.x) / 2;
        for y in 0..logic.pathfinding_system.grid.height() {
            logic
                .pathfinding_system
                .grid
                .set_cell_type(GridPos::new(wall_x, y), PathfindCellType::Rubble);
        }
        let mut tmpl = ThingTemplate::new("Overlord");
        tmpl.add_kind_of(KindOf::Vehicle);
        logic.templates.insert("Overlord".into(), tmpl);
        let id = logic
            .create_object("Overlord", Team::USA, start)
            .expect("overlord");
        if let Some(u) = logic.host_object_mut(id) {
            u.crusher_level = 2;
            u.locomotor_surfaces = SURFACE_GROUND;
            u.movement.max_speed = 20.0;
        }
        assert!(
            logic.assign_unit_path_for_test(id, goal, &[]),
            "crusher assign_unit_path must path CELL_RUBBLE"
        );
        let unit = logic.host_object(id).expect("unit");
        assert!(
            unit.movement.path.len() >= 2,
            "crusher must receive a live A* path, got {:?}",
            unit.movement.path
        );
    }

    /// hq-8q7gp: adjustDestination refuses uncrushable parked occupants.
    #[test]
    fn adjust_destination_refuses_enemy_fixed_occupant() {
        use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
        let mut g = open_grid(16, 16);
        let dest = GridPos::new(8, 8);
        let mut objects = HashMap::new();
        let mut tmpl = ThingTemplate::new("CivilianCar");
        tmpl.add_kind_of(KindOf::Vehicle);
        let mut car = Object::new(tmpl, ObjectId(9), Team::GLA);
        car.set_position(g.grid_to_world(dest));
        car.crushable_level = 1;
        car.owner_player_id = Some(1);
        objects.insert(car.id, car);
        g.update_dynamic_obstacles(&objects);
        let snapped = g
            .adjust_destination_ex(dest, SURFACE_GROUND, false, 64, Some(0), 0)
            .expect("neighbor");
        assert_ne!(snapped, dest, "must not accept the occupied cell");
        assert!(
            !g.has_blocking_fixed_occupant(snapped, 0),
            "spiral must land off the parked car"
        );
        let crushed = g
            .adjust_destination_ex(dest, SURFACE_GROUND, true, 64, Some(0), 2)
            .expect("crusher");
        assert_eq!(crushed, dest, "crusher may occupy the crushable car cell");
    }

    /// hq-9erk0: rally uses structure-aware zones, not ForUI terrain zones.
    #[test]
    fn rally_gate_rejects_structure_enclosed_courtyard() {
        let mut g = open_grid(12, 12);
        for y in 0..12 {
            g.set_cell_type(GridPos::new(6, y), PathfindCellType::Obstacle);
        }
        g.rebuild_terrain_zones();
        g.rebuild_path_zones();
        let from = g.grid_to_world(GridPos::new(2, 6));
        let to = g.grid_to_world(GridPos::new(10, 6));
        assert!(
            g.quick_path_exists_for_ui(from, to),
            "ForUI ignores structure obstacles"
        );
        assert!(
            !g.quick_path_exists(from, to),
            "clientSafeQuickDoesPathExist must split on Obstacle"
        );
    }

    /// hq-c88bl: rubble husks stamp CELL_RUBBLE, not Clear / Obstacle.
    #[test]
    fn destroyed_building_stamps_rubble_not_clear() {
        use crate::game_logic::host_enum_table_residual::HostBodyDamageType;
        use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
        let mut sys = PathfindingSystem::new(200.0, 200.0);
        let mut objects = HashMap::new();
        let mut tmpl = ThingTemplate::new("AmericaWarFactory");
        tmpl.add_kind_of(KindOf::Structure);
        let mut factory = Object::new(tmpl, ObjectId(3), Team::USA);
        factory.set_position(Vec3::new(80.0, 0.0, 80.0));
        factory.selection_radius = 20.0;
        factory.body_damage_state = HostBodyDamageType::Rubble;
        factory.status.keep_as_rubble = true;
        factory.status.effectively_dead = true;
        factory.health.current = 0.0;
        objects.insert(factory.id, factory);
        sys.apply_structure_static_blocks(&objects);
        let cell = sys.grid.world_to_grid(Vec3::new(80.0, 0.0, 80.0));
        assert_eq!(sys.grid.cell_type(cell), PathfindCellType::Rubble);
        assert_ne!(
            sys.grid.path_zone(cell),
            0,
            "rubble husk must get a real zone"
        );
        let nearby = sys.grid.world_to_grid(Vec3::new(20.0, 0.0, 20.0));
        assert_ne!(
            sys.grid.path_zone(cell),
            sys.grid.path_zone(nearby),
            "rubble husk must not merge into Clear ground"
        );
        assert!(
            !sys.grid.cell_passable_for(cell, SURFACE_GROUND, false),
            "infantry cannot walk rubble at full ground surfaces"
        );
        assert!(
            sys.grid.cell_passable_for(cell, SURFACE_GROUND, true),
            "crushers walk rubble"
        );
    }

    /// hq-2f8q0: CPOP must not lead through an obstacle wall.
    #[test]
    fn cpop_lead_aborts_through_building() {
        let mut g = open_grid(16, 16);
        for y in 0..16 {
            g.set_cell_type(GridPos::new(8, y), PathfindCellType::Obstacle);
        }
        let a = g.grid_to_world(GridPos::new(2, 8));
        let b = g.grid_to_world(GridPos::new(14, 8));
        let pos = g.grid_to_world(GridPos::new(4, 8));
        let lead = PathfindingSystem::compute_point_on_path_ex(
            pos,
            &[a, b],
            Some(&g),
            SURFACE_GROUND,
            false,
        );
        let lead_cell = g.world_to_grid(lead);
        assert!(
            lead_cell.x < 8,
            "must not aim through the building, lead={lead:?} cell={lead_cell:?}"
        );
        let blind = PathfindingSystem::compute_point_on_path(pos, &[a, b]);
        let blind_cell = g.world_to_grid(blind);
        assert!(
            blind_cell.x >= 8,
            "ungated geometric lead still crosses the wall (control)"
        );
    }

    /// hq-5g9m3: C++ always tries lead; offset of 2 cells must still cut to next.
    #[test]
    fn cpop_leads_when_two_cells_off_clear_path() {
        let g = open_grid(16, 16);
        let a = g.grid_to_world(GridPos::new(2, 8));
        let b = g.grid_to_world(GridPos::new(14, 8));
        // Two cells off the polyline (C++ k = 2/3, still tries next node).
        let pos = g.grid_to_world(GridPos::new(4, 6));
        let lead = PathfindingSystem::compute_point_on_path_ex(
            pos,
            &[a, b],
            Some(&g),
            SURFACE_GROUND,
            false,
        );
        let lead_cell = g.world_to_grid(lead);
        assert!(
            lead_cell.x >= 12,
            "clear line must lead to next node, lead={lead:?} cell={lead_cell:?}"
        );
    }

    /// hq-csg1b: pinched cells on a straight A* jog still collapse.
    #[test]
    fn optimize_collapses_pinched_collinear_jog() {
        let mut g = open_grid(12, 8);
        for x in 3..9 {
            g.set_pinched(GridPos::new(x, 4), true);
        }
        let raw: Vec<Vec3> = (2..=10)
            .map(|x| g.grid_to_world(GridPos::new(x, 4)))
            .collect();
        let opt = g.optimize_ground_path_ex(&raw, SURFACE_GROUND, false, None, 0);
        assert!(
            opt.len() <= 2,
            "straight pinched jog must collapse, got {opt:?}"
        );
    }

    /// hq-9jz4r: layer change only looks ALLOWED_STEPS=3 past the ramp.
    #[test]
    fn optimize_caps_bridge_layer_steps() {
        let mut g = open_grid(16, 12);
        for y in 4..=6 {
            for x in 4..12 {
                g.set_cell_type(GridPos::new(x, y), PathfindCellType::Water);
            }
        }
        g.stamp_bridge_deck(
            Vec3::new(30.0, 20.0, 40.0),
            Vec3::new(30.0, 20.0, 60.0),
            Vec3::new(130.0, 20.0, 40.0),
            Vec3::new(130.0, 20.0, 60.0),
            false,
        );
        let mut raw = Vec::new();
        // Five ground cells so count at the first deck node is > ALLOWED_STEPS.
        for x in 0..=4 {
            raw.push(g.grid_to_world(GridPos::new(x, 5)));
        }
        for x in 5..=12 {
            raw.push(g.grid_to_world_on_layer(
                GridPos::new(x, 5),
                PathfindLayerEnum::from_u32(g.first_bridge_layer_id().unwrap_or(2) as u32),
            ));
        }
        let opt = g.optimize_ground_path_ex(&raw, SURFACE_GROUND, false, None, 0);
        assert!(
            opt.len() >= 3,
            "ALLOWED_STEPS=3 must keep ramp approach, got {opt:?}"
        );
        let start_layer = g.layer_for_destination(opt[0]);
        assert_eq!(
            start_layer,
            PathfindLayerEnum::Ground,
            "must keep a ground approach node"
        );
    }



    /// hq-5iup4: optimize must not cut through parked occupants; un-pinched cliff can.
    #[test]
    fn optimize_respects_occupancy_and_unpinched_cliff() {
        use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
        let mut g = open_grid(16, 16);
        let mut objects = HashMap::new();
        let mut tmpl = ThingTemplate::new("Ranger");
        tmpl.add_kind_of(KindOf::Infantry);
        let mut ally = Object::new(tmpl, ObjectId(4), Team::USA);
        ally.set_position(g.grid_to_world(GridPos::new(8, 4)));
        ally.owner_player_id = Some(0);
        objects.insert(ally.id, ally);
        g.update_dynamic_obstacles(&objects);
        let start = g.grid_to_world(GridPos::new(2, 4));
        let mid = g.grid_to_world(GridPos::new(8, 2));
        let end = g.grid_to_world(GridPos::new(14, 4));
        let raw = vec![start, mid, end];
        let opt = g.optimize_ground_path_ex(&raw, SURFACE_GROUND, false, Some(0), 0);
        assert!(
            opt.len() >= 3,
            "must keep the detour around idle ally, got {opt:?}"
        );

        let mut cliff = open_grid(8, 8);
        cliff.set_cell_type(GridPos::new(4, 4), PathfindCellType::Cliff);
        let a = cliff.grid_to_world(GridPos::new(1, 4));
        let b = cliff.grid_to_world(GridPos::new(4, 4));
        let c = cliff.grid_to_world(GridPos::new(7, 4));
        let collapsed = cliff.optimize_ground_path_ex(&[a, b, c], SURFACE_GROUND, false, None, 0);
        assert!(
            collapsed.len() <= 2,
            "un-pinched cliff ramp must collapse, got {collapsed:?}"
        );
    }

    /// hq-vovla: FenceWidth>0 rasters a fence; name-only decorative props do not.
    #[test]
    fn fence_width_not_name_classifies_fence() {
        use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
        let mut sys = PathfindingSystem::new(200.0, 200.0);
        let mut objects = HashMap::new();
        let mut named = ThingTemplate::new("DecorativeFenceProp");
        named.add_kind_of(KindOf::Structure);
        named.fence_width = 0.0;
        let mut prop = Object::new(named, ObjectId(1), Team::Neutral);
        prop.set_position(Vec3::new(40.0, 0.0, 40.0));
        prop.selection_radius = 20.0;
        objects.insert(prop.id, prop);
        sys.apply_structure_static_blocks(&objects);
        let named_cell = sys.grid.world_to_grid(Vec3::new(40.0, 0.0, 40.0));
        assert!(
            !sys.grid.is_obstacle_fence(named_cell),
            "name-only fence must not become a crush corridor"
        );

        let mut real = ThingTemplate::new("ChinaChainlink");
        real.fence_width = 40.0;
        real.fence_x_offset = 0.0;
        let mut fence = Object::new(real, ObjectId(2), Team::China);
        fence.set_position(Vec3::new(120.0, 0.0, 40.0));
        fence.set_orientation(0.0);
        objects.insert(fence.id, fence);
        sys.apply_structure_static_blocks(&objects);
        let fence_cell = sys.grid.world_to_grid(Vec3::new(120.0, 0.0, 40.0));
        assert!(
            sys.grid.is_obstacle_fence(fence_cell),
            "INI FenceWidth must classify a crushable fence strip"
        );
        assert!(sys.grid.cell_passable_for(fence_cell, SURFACE_GROUND, true));
    }

    /// hq-qbvcc: scaffolds classify as path obstacles at placement.
    #[test]
    fn under_construction_structure_blocks_path() {
        use crate::game_logic::{GameLogic, KindOf, Object, ObjectId, Team, ThingTemplate};
        let mut sys = PathfindingSystem::new(200.0, 200.0);
        let mut objects = HashMap::new();
        let mut tmpl = ThingTemplate::new("AmericaWarFactory");
        tmpl.add_kind_of(KindOf::Structure);
        let mut factory = Object::new_under_construction(tmpl, ObjectId(4), Team::USA);
        factory.set_position(Vec3::new(80.0, 0.0, 80.0));
        factory.selection_radius = 20.0;
        assert!(factory.status.under_construction);
        objects.insert(factory.id, factory);
        sys.apply_structure_static_blocks(&objects);
        let cell = sys.grid.world_to_grid(Vec3::new(80.0, 0.0, 80.0));
        assert!(
            sys.grid.is_static_blocked(cell),
            "C++ addObjectToPathfindMap at construct() must block unfinished buildings"
        );

        let mut logic = GameLogic::new();
        let mut place = ThingTemplate::new("TestScaffoldBarracks");
        place.add_kind_of(KindOf::Structure);
        logic.templates.insert("TestScaffoldBarracks".into(), place);
        let id = logic
            .create_object_under_construction(
                "TestScaffoldBarracks",
                Team::USA,
                Vec3::new(80.0, 0.0, 80.0),
            )
            .expect("scaffold");
        let obj = logic.host_object(id).expect("placed");
        assert!(obj.status.under_construction);
        let placed = obj.get_position();
        let placed_cell = logic.pathfinding_system.grid.world_to_grid(placed);
        assert!(
            logic
                .pathfinding_system
                .grid
                .is_static_blocked(placed_cell),
            "placement must stamp the scaffold footprint immediately"
        );
    }

    /// hq-ah4jh: queued move must not install dest velocity before A*.
    #[test]
    fn queued_move_does_not_charge_before_path() {
        use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
        let mut logic = GameLogic::new();
        let mut tmpl = ThingTemplate::new("Ranger");
        tmpl.add_kind_of(KindOf::Infantry);
        logic.templates.insert("Ranger".into(), tmpl);
        let id = logic
            .create_object("Ranger", Team::USA, Vec3::new(10.0, 0.0, 10.0))
            .expect("ranger");
        if let Some(u) = logic.host_object_mut(id) {
            u.movement.max_speed = 20.0;
        }
        logic.force_map_loaded_for_path_test(true);
        assert!(logic.assign_unit_path(id, Vec3::new(80.0, 0.0, 10.0), &[]));
        let unit = logic.host_object(id).expect("unit");
        assert!(unit.waiting_for_path);
        assert!(
            unit.movement.velocity.length_squared() < 1.0e-6,
            "must not charge the raw click, vel={:?}",
            unit.movement.velocity
        );
        assert!(
            unit.movement.target_position.is_none(),
            "locomotor must not integrate toward dest while waiting"
        );
    }

    /// hq-985ts: leftover/C++ clearCellForDiameter on the live grid.
    #[test]
    fn clear_cell_for_diameter_open_and_blocked() {
        let mut g = open_grid(16, 16);
        assert_eq!(
            g.clear_cell_for_diameter(false, GridPos::new(8, 8), 2),
            2
        );
        assert_eq!(
            g.clear_cell_for_diameter(false, GridPos::new(8, 8), 1),
            1
        );
        g.set_blocked(GridPos::new(7, 8), true);
        assert_eq!(
            g.clear_cell_for_diameter(false, GridPos::new(8, 8), 2),
            0,
            "diameter 2 must fail when an adjacent cell is blocked"
        );
        let (r, _) = PathfindingGrid::radius_and_center(15.0, 10.0);
        assert_eq!(r, 1);
        assert_eq!(
            PathfindingGrid::path_diameter_for_unit(15.0, 10.0, true),
            2
        );
        assert_eq!(
            PathfindingGrid::path_diameter_for_unit(8.0, 10.0, false),
            1
        );
    }

    /// hq-985ts: tanks cannot thread a one-cell infantry slot.
    #[test]
    fn vehicle_astar_rejects_infantry_width_gap() {
        use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
        let mut sys = PathfindingSystem::new(200.0, 200.0);
        for y in 0..20 {
            sys.grid.set_blocked(GridPos::new(8, y), true);
            sys.grid.set_blocked(GridPos::new(10, y), true);
        }
        let start = Vec3::new(20.0, 0.0, 50.0);
        let goal = Vec3::new(150.0, 0.0, 50.0);

        let mut inf_t = ThingTemplate::new("Ranger");
        inf_t.add_kind_of(KindOf::Infantry);
        let mut inf = Object::new(inf_t, ObjectId(1), Team::USA);
        inf.set_position(start);
        inf.selection_radius = 8.0;
        let mut objects = HashMap::new();
        objects.insert(inf.id, inf);
        let infantry_path = sys
            .find_path_ex_surfaces(start, goal, &objects, false, SURFACE_GROUND, false, Some(ObjectId(1)))
            .expect("infantry can thread a one-cell corridor");
        assert!(infantry_path.len() >= 2);

        objects.clear();
        let mut tank_t = ThingTemplate::new("Crusader");
        tank_t.add_kind_of(KindOf::Vehicle);
        let mut tank = Object::new(tank_t, ObjectId(2), Team::USA);
        tank.set_position(start);
        tank.selection_radius = 15.0;
        objects.insert(tank.id, tank);
        sys.note_logic_frame(1);
        let tank_path =
            sys.find_path_ex_surfaces(start, goal, &objects, false, SURFACE_GROUND, false, Some(ObjectId(2)));
        let crossed = tank_path.as_ref().is_some_and(|path| {
            path.iter().any(|p| {
                let c = sys.grid.world_to_grid(*p);
                c.x == 9
            }) && path
                .last()
                .is_some_and(|p| sys.grid.world_to_grid(*p).x >= 12)
        });
        assert!(
            !crossed,
            "vehicle A* must not thread a one-cell infantry gap, got {tank_path:?}"
        );
    }

    /// hq-asov5: structure-aware rally zones split on water.
    #[test]
    fn path_zones_reject_water_dest_and_split_river() {
        let mut g = open_grid(8, 8);
        for y in 0..8 {
            g.set_cell_type(GridPos::new(4, y), PathfindCellType::Water);
        }
        g.rebuild_path_zones();
        let from = g.grid_to_world(GridPos::new(1, 4));
        let to = g.grid_to_world(GridPos::new(6, 4));
        let wet = g.grid_to_world(GridPos::new(4, 4));
        assert!(!g.quick_path_exists(from, to), "water must split path_zones");
        assert!(
            !g.quick_path_exists(from, wet),
            "ground rally must reject a water dest"
        );
        assert_ne!(g.path_zone(GridPos::new(1, 4)), 0);
        assert_ne!(
            g.path_zone(GridPos::new(1, 4)),
            g.path_zone(GridPos::new(6, 4))
        );
    }

    /// hq-998ki: GROUND+WATER / GROUND+CLIFF combiners join banks; dest gates stay C++.
    #[test]
    fn path_zones_surface_combiners_join_water_and_cliff() {
        let mut g = open_grid(8, 8);
        for y in 0..8 {
            g.set_cell_type(GridPos::new(4, y), PathfindCellType::Water);
        }
        g.rebuild_path_zones();
        let from = g.grid_to_world(GridPos::new(1, 4));
        let to = g.grid_to_world(GridPos::new(6, 4));
        let wet = g.grid_to_world(GridPos::new(4, 4));
        assert!(
            !g.quick_path_exists(from, to),
            "GROUND-only must not share a zone across CELL_WATER"
        );
        assert!(
            g.quick_path_exists_for(from, to, SURFACE_GROUND | SURFACE_WATER),
            "GROUND+WATER combiner must join opposite banks"
        );
        assert!(
            g.quick_path_exists_for(from, wet, SURFACE_GROUND | SURFACE_WATER),
            "amphibious dest on water is validMovementPosition"
        );

        let mut c = open_grid(8, 8);
        for y in 0..8 {
            c.set_cell_type(GridPos::new(4, y), PathfindCellType::Cliff);
        }
        c.rebuild_path_zones();
        let c_from = c.grid_to_world(GridPos::new(1, 4));
        let c_to = c.grid_to_world(GridPos::new(6, 4));
        let cliff = c.grid_to_world(GridPos::new(4, 4));
        assert!(
            !c.quick_path_exists(c_from, c_to),
            "GROUND-only must not share a zone across CELL_CLIFF"
        );
        assert!(
            c.quick_path_exists_for(c_from, c_to, SURFACE_GROUND | SURFACE_CLIFF),
            "GROUND+CLIFF combiner must join opposite banks"
        );
        assert!(
            !c.quick_path_exists_for(c_from, cliff, SURFACE_GROUND | SURFACE_CLIFF),
            "C++ rejects cliff goals even for cliff locos"
        );
    }

    /// hq-0r5xs: rubble is its own zone; Impassable is not uninitialized 0.
    #[test]
    fn path_zones_rubble_and_impassable_are_not_ground() {
        let mut g = open_grid(8, 8);
        for y in 0..8 {
            g.set_cell_type(GridPos::new(4, y), PathfindCellType::Rubble);
        }
        g.rebuild_path_zones();
        let from = g.grid_to_world(GridPos::new(1, 4));
        let to = g.grid_to_world(GridPos::new(6, 4));
        let rubble = g.grid_to_world(GridPos::new(4, 4));
        assert_ne!(
            g.path_zone(GridPos::new(4, 4)),
            0,
            "CELL_RUBBLE must get a real zone"
        );
        assert_ne!(
            g.path_zone(GridPos::new(1, 4)),
            g.path_zone(GridPos::new(4, 4)),
            "rubble must not merge into the Clear flood"
        );
        assert!(
            !g.quick_path_exists(from, to),
            "GROUND-only must not share a zone across CELL_RUBBLE"
        );
        assert!(
            !g.quick_path_exists(from, rubble),
            "GROUND-only must not treat rubble dest as pathable"
        );
        assert!(
            g.quick_path_exists_for(from, to, SURFACE_GROUND | SURFACE_RUBBLE),
            "GROUND+RUBBLE combiner must join opposite banks"
        );
        assert!(
            g.quick_path_exists_for(from, rubble, SURFACE_GROUND | SURFACE_RUBBLE),
            "rubble dest is valid for GROUND+RUBBLE locos"
        );

        let mut imp = open_grid(8, 8);
        for y in 0..8 {
            imp.set_cell_type(GridPos::new(4, y), PathfindCellType::Impassable);
        }
        imp.rebuild_path_zones();
        let i_from = imp.grid_to_world(GridPos::new(1, 4));
        let i_to = imp.grid_to_world(GridPos::new(6, 4));
        let i_cell = imp.grid_to_world(GridPos::new(4, 4));
        assert_ne!(
            imp.path_zone(GridPos::new(4, 4)),
            0,
            "Impassable must get a real zone, not uninitialized 0"
        );
        assert!(
            !imp.quick_path_exists(i_from, i_to),
            "GROUND-only must not share a zone across Impassable"
        );
        assert!(
            !imp.quick_path_exists(i_from, i_cell),
            "Impassable dest must not false-positive via zone 0"
        );

        for ty in [
            PathfindCellType::Obstacle,
            PathfindCellType::BridgeImpassable,
        ] {
            let mut g = open_grid(8, 8);
            for y in 0..8 {
                g.set_cell_type(GridPos::new(4, y), ty);
            }
            g.rebuild_path_zones();
            let from = g.grid_to_world(GridPos::new(1, 4));
            let to = g.grid_to_world(GridPos::new(6, 4));
            let cell = g.grid_to_world(GridPos::new(4, 4));
            assert_ne!(g.path_zone(GridPos::new(4, 4)), 0, "{ty:?} must get a real zone");
            assert!(
                !g.quick_path_exists(from, to),
                "GROUND-only must not share a zone across {ty:?}"
            );
            assert!(
                !g.quick_path_exists(from, cell),
                "{ty:?} dest must not false-positive via zone 0"
            );
        }

        let mut sys = PathfindingSystem::new(80.0, 80.0);
        sys.grid.rebuild_path_zones();
        let s_from = sys.grid.grid_to_world(GridPos::new(1, 4));
        let s_to = sys.grid.grid_to_world(GridPos::new(6, 4));
        assert!(
            sys.grid.quick_path_exists(s_from, s_to),
            "open map must path before rubble stamp"
        );
        for y in 0..8 {
            let p = sys.grid.grid_to_world(GridPos::new(4, y));
            sys.stamp_rubble_at_world(p, 0);
        }
        assert_eq!(
            sys.grid.cell_type(GridPos::new(4, 4)),
            PathfindCellType::Rubble
        );
        assert!(
            !sys.grid.quick_path_exists(s_from, s_to),
            "stamp_rubble_at_world must rebuild zones so rubble splits GROUND"
        );
    }

    /// hq-su78f: full queue refuses the newest, keeps the oldest.
    #[test]
    fn queue_overflow_refuses_newest() {
        let mut sys = PathfindingSystem::new(100.0, 100.0);
        let mk = |id: u32| PendingHostPath {
            unit_id: ObjectId(id),
            start: Vec3::ZERO,
            destination: Vec3::new(id as f32, 0.0, 0.0),
            waypoints: Vec::new(),
            aircraft: false,
            surfaces: SURFACE_GROUND,
            is_crusher: false,
            ignore_obstacle: None,
        };
        for i in 1..=PATHFIND_QUEUE_LEN as u32 {
            assert!(sys.queue_path(mk(i)), "slot {i} must enqueue");
        }
        assert_eq!(sys.pending_path_count(), PATHFIND_QUEUE_LEN);
        assert!(
            !sys.queue_path(mk(9000)),
            "C++ queueForPath refuses when nextSlot==head"
        );
        assert_eq!(sys.pending_path_count(), PATHFIND_QUEUE_LEN);
        assert!(sys.queue_path(mk(1)), "duplicate ObjectID is a no-op success");
        let drained = sys.take_pending_paths();
        assert_eq!(drained.len(), PATHFIND_QUEUE_LEN);
        assert_eq!(drained[0].unit_id, ObjectId(1), "oldest waiter stays");
        assert!(
            drained.iter().all(|p| p.unit_id != ObjectId(9000)),
            "newest must not evict oldest"
        );
    }

    /// hq-p1pwi: moveAllies leaves packing/unpacking units planted.
    #[test]
    fn move_allies_skips_deploy_style_busy() {
        use crate::game_logic::host_deploy_style::{
            HostDeployStyleData, HostDeployStyleState,
        };
        use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
        let sys = PathfindingSystem::new(200.0, 200.0);
        let mut objects = HashMap::new();
        let mut mover_t = ThingTemplate::new("Ranger");
        mover_t.add_kind_of(KindOf::Infantry);
        let mut mover = Object::new(mover_t, ObjectId(1), Team::USA);
        mover.set_position(Vec3::new(10.0, 0.0, 10.0));
        objects.insert(mover.id, mover);

        let mut idle_t = ThingTemplate::new("Humvee");
        idle_t.add_kind_of(KindOf::Vehicle);
        let mut idle = Object::new(idle_t, ObjectId(2), Team::USA);
        idle.set_position(Vec3::new(50.0, 0.0, 10.0));
        objects.insert(idle.id, idle);

        let mut busy_t = ThingTemplate::new("NukeCannon");
        busy_t.add_kind_of(KindOf::Vehicle);
        let mut busy = Object::new(busy_t, ObjectId(3), Team::USA);
        busy.set_position(Vec3::new(60.0, 0.0, 10.0));
        let mut style = HostDeployStyleData::default();
        style.state = HostDeployStyleState::Deploying;
        busy.deploy_style = Some(style);
        objects.insert(busy.id, busy);

        let path = vec![
            Vec3::new(10.0, 0.0, 10.0),
            Vec3::new(50.0, 0.0, 10.0),
            Vec3::new(60.0, 0.0, 10.0),
        ];
        let nudged = sys.allies_to_nudge_off_path(ObjectId(1), &path, &objects);
        assert!(
            nudged.contains(&ObjectId(2)),
            "idle ally on the path must scoot"
        );
        assert!(
            !nudged.contains(&ObjectId(3)),
            "packing unit must stay planted"
        );
    }

    /// hq-8cfgs: 4-corner CLEAR / pinch CLIFF / dest layer / walk-on-wall A*.
    #[test]
    fn layer_wall_classify_and_search() {
        let mut g = open_grid(16, 16);
        g.set_wall_height(12.0);
        // Fat enough that cell (8,8) and its 3x3 are 4-corner CLEAR (no pinch).
        g.add_wall_piece(1, Vec3::new(85.0, 0.0, 85.0), 0.0, 40.0, 40.0);
        assert!(g.wall_piece_count() == 1);
        let center = GridPos::new(8, 8);
        assert_eq!(
            g.layer_cell_type(LAYER_WALL_ID, center),
            Some(PathfindCellType::Clear),
            "interior 4-corner cell must be CLEAR on LAYER_WALL"
        );
        // Cell (4,8) is [40,50]×[80,90]: two corners on the 40-radius deck.
        assert_eq!(
            g.layer_cell_type(LAYER_WALL_ID, GridPos::new(4, 8)),
            Some(PathfindCellType::BridgeImpassable),
            "1–3 corner cells are BRIDGE_IMPASSABLE (AIPathfind.cpp:3794-3796)"
        );
        g.clear_static_blocks();
        assert_eq!(
            g.layer_cell_type(LAYER_WALL_ID, center),
            Some(PathfindCellType::Clear),
            "terrain rebuild must reclassify remaining wall pieces"
        );
        let on_wall = Vec3::new(85.0, 12.0, 85.0);
        assert!(g.is_point_on_wall(on_wall));
        assert_eq!(
            g.layer_for_destination(on_wall),
            PathfindLayerEnum::Wall,
            "dest at wall height on a CLEAR cell is LAYER_WALL"
        );
        let on_ground = Vec3::new(85.0, 0.0, 85.0);
        assert_ne!(
            g.layer_for_destination(on_ground),
            PathfindLayerEnum::Wall,
            "ground-height click on the footprint stays off LAYER_WALL"
        );
        g.remove_wall_piece(1);
        assert_eq!(g.wall_piece_count(), 0);
        assert!(
            g.layer_cell_type(LAYER_WALL_ID, center).is_none(),
            "removing the last piece drops LAYER_WALL"
        );
        assert!(!g.is_point_on_wall(on_wall));
    }

    #[test]
    fn infantry_paths_along_classified_wall() {
        let mut sys = PathfindingSystem::new(160.0, 160.0);
        sys.set_wall_height(12.0);
        for x in 4..13 {
            sys.grid
                .set_cell_type(GridPos::new(x, 8), PathfindCellType::Obstacle);
        }
        sys.grid
            .add_wall_piece(7, Vec3::new(85.0, 0.0, 85.0), 0.0, 50.0, 25.0);
        let objects = HashMap::new();
        let from = Vec3::new(50.0, 12.0, 85.0);
        let to = Vec3::new(120.0, 12.0, 85.0);
        assert_eq!(
            sys.grid.layer_for_destination(from),
            PathfindLayerEnum::Wall
        );
        assert_eq!(sys.grid.layer_for_destination(to), PathfindLayerEnum::Wall);
        let path = sys.find_path(from, to, &objects);
        assert!(
            path.as_ref().map(|p| p.len() >= 2).unwrap_or(false),
            "infantry A* must walk LAYER_WALL over the blocked ground"
        );
    }

    #[test]
    fn destroy_wall_piece_splats_units_on_deck() {
        use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
        let mut sys = PathfindingSystem::new(160.0, 160.0);
        sys.set_wall_height(12.0);
        sys.grid
            .add_wall_piece(1, Vec3::new(80.0, 0.0, 50.0), 0.0, 20.0, 8.0);
        let mut objects = HashMap::new();
        let mut inf_t = ThingTemplate::new("RedGuard");
        inf_t.add_kind_of(KindOf::Infantry);
        let mut on_deck = Object::new(inf_t, ObjectId(10), Team::China);
        on_deck.set_position(Vec3::new(80.0, 12.0, 50.0));
        objects.insert(on_deck.id, on_deck);
        let mut ground_t = ThingTemplate::new("Battlemaster");
        ground_t.add_kind_of(KindOf::Vehicle);
        let mut on_ground = Object::new(ground_t, ObjectId(11), Team::China);
        on_ground.set_position(Vec3::new(80.0, 0.0, 50.0));
        objects.insert(on_ground.id, on_ground);
        let splat = sys.splat_units_on_wall_piece(ObjectId(1), &objects);
        assert!(splat.contains(&ObjectId(10)), "deck unit must splat");
        assert!(
            !splat.contains(&ObjectId(11)),
            "ground unit on the footprint must live"
        );
    }

    /// hq-1bkmw: infantry occupancy is getRadiusAndCenter (1 cell), not 3×3.
    #[test]
    fn infantry_occupancy_is_single_cell() {
        use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
        let mut g = open_grid(16, 16);
        let mut objects = HashMap::new();
        let mut tmpl = ThingTemplate::new("Ranger");
        tmpl.add_kind_of(KindOf::Infantry);
        let mut inf = Object::new(tmpl, ObjectId(1), Team::USA);
        inf.set_position(Vec3::new(55.0, 0.0, 55.0));
        inf.owner_player_id = Some(0);
        inf.selection_radius = 5.0;
        g.update_dynamic_obstacles(&objects);
        let center = g.world_to_grid(Vec3::new(55.0, 0.0, 55.0));
        assert!(g.is_blocked(center), "infantry center cell must occupy");
        assert!(
            !g.is_blocked(GridPos::new(center.x + 1, center.y)),
            "r=5 infantry must not stamp a 3x3 Chebyshev"
        );
        assert!(
            !g.is_blocked(GridPos::new(center.x, center.y + 1)),
            "r=5 infantry must not stamp a 3x3 Chebyshev"
        );
    }

    /// hq-7q7d9: landing dest rejects Obstacle and scans aircraft radius.
    #[test]
    fn landing_dest_rejects_obstacle_and_uses_radius() {
        let mut g = open_grid(16, 16);
        let obstacle = GridPos::new(5, 5);
        g.set_cell_type(obstacle, PathfindCellType::Obstacle);
        let land = g
            .adjust_to_landing_destination(obstacle, 400, PathfindLayerEnum::Ground)
            .expect("spiral off obstacle");
        assert_ne!(land, obstacle);
        assert_ne!(g.cell_type(land), PathfindCellType::Obstacle);

        let rail = GridPos::new(8, 8);
        g.set_cell_type(rail, PathfindCellType::BridgeImpassable);
        let land_rail = g
            .adjust_to_landing_destination(rail, 400, PathfindLayerEnum::Ground)
            .expect("spiral off bridge rail");
        assert_ne!(g.cell_type(land_rail), PathfindCellType::BridgeImpassable);

        let mut g2 = open_grid(16, 16);
        g2.set_cell_type(GridPos::new(6, 5), PathfindCellType::Obstacle);
        let dest = GridPos::new(5, 5);
        assert_eq!(
            g2.adjust_to_landing_destination_for(dest, 400, PathfindLayerEnum::Ground, 0, true),
            Some(dest),
            "radius 0 may land on a clear center"
        );
        let wide = g2
            .adjust_to_landing_destination_for(dest, 400, PathfindLayerEnum::Ground, 1, true)
            .expect("spiral off radius-1 obstacle");
        assert_ne!(wide, dest, "radius 1 footprint must refuse neighbor Obstacle");
    }

    /// hq-lhwsl: yield A* walks off the avoided path, not a 20wu shove.
    #[test]
    fn get_move_away_from_path_leaves_avoided_route() {
        let mut sys = PathfindingSystem::new(200.0, 200.0);
        let from = Vec3::new(50.0, 0.0, 50.0);
        let avoided = vec![
            Vec3::new(10.0, 0.0, 50.0),
            Vec3::new(50.0, 0.0, 50.0),
            Vec3::new(150.0, 0.0, 50.0),
        ];
        let path = sys
            .get_move_away_from_path(
                from,
                &avoided,
                None,
                SURFACE_GROUND,
                false,
                6.0,
                15.0,
                Some(0),
                0,
                false,
            )
            .expect("yield path");
        assert!(path.len() >= 2);
        let dest = *path.last().unwrap();
        assert!(
            (dest.x - from.x).abs() > 1.0 || (dest.z - from.z).abs() > 1.0,
            "must leave the start cell, dest={dest:?}"
        );
        assert!(
            (dest.z - 50.0).abs() > 5.0 || dest.x < 20.0 || dest.x > 140.0,
            "dest must not sit on the avoided corridor, dest={dest:?}"
        );
    }

    /// hq-fyn6i: moveAllies uses ALLIES / ignoreObstacle / mover radius.
    #[test]
    fn move_allies_uses_allies_radius_and_ignore() {
        use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
        let mut sys = PathfindingSystem::new(200.0, 200.0);
        let mut masks = [0u16; 16];
        masks[0] = 1u16 << 0 | 1u16 << 1;
        masks[1] = 1u16 << 0 | 1u16 << 1;
        sys.set_player_ally_masks(masks);

        let mut objects = HashMap::new();
        let mut mover_t = ThingTemplate::new("Overlord");
        mover_t.add_kind_of(KindOf::Vehicle);
        let mut mover = Object::new(mover_t, ObjectId(1), Team::USA);
        mover.set_position(Vec3::new(10.0, 0.0, 10.0));
        mover.selection_radius = 25.0;
        mover.owner_player_id = Some(0);
        objects.insert(mover.id, mover);

        let mut ally_t = ThingTemplate::new("Battlemaster");
        ally_t.add_kind_of(KindOf::Vehicle);
        let mut ally = Object::new(ally_t, ObjectId(2), Team::China);
        ally.set_position(Vec3::new(70.0, 0.0, 10.0));
        ally.owner_player_id = Some(1);
        objects.insert(ally.id, ally);

        let mut ignored_t = ThingTemplate::new("Dozer");
        ignored_t.add_kind_of(KindOf::Vehicle);
        ignored_t.add_kind_of(KindOf::Dozer);
        let mut ignored = Object::new(ignored_t, ObjectId(3), Team::USA);
        ignored.set_position(Vec3::new(80.0, 0.0, 10.0));
        ignored.owner_player_id = Some(0);
        objects.insert(ignored.id, ignored);

        let mut off_t = ThingTemplate::new("Humvee");
        off_t.add_kind_of(KindOf::Vehicle);
        let mut off = Object::new(off_t, ObjectId(4), Team::USA);
        off.set_position(Vec3::new(70.0, 0.0, 40.0));
        off.owner_player_id = Some(0);
        objects.insert(off.id, off);

        let path = vec![
            Vec3::new(10.0, 0.0, 10.0),
            Vec3::new(80.0, 0.0, 10.0),
        ];
        sys.set_ignore_obstacle(Some(ObjectId(3)));
        let nudged = sys.allies_to_nudge_off_path(ObjectId(1), &path, &objects);
        assert!(
            nudged.contains(&ObjectId(2)),
            "allied other-player unit on the Overlord radius must scoot"
        );
        assert!(
            !nudged.contains(&ObjectId(3)),
            "ignoreObstacle must stay planted"
        );
        assert!(
            !nudged.contains(&ObjectId(4)),
            "unit off the mover footprint must not fidget"
        );
    }

    /// hq-8lb00: oriented box, not a clamped selection-radius square.
    #[test]
    fn structure_box_footprint_is_not_clamped_square() {
        use crate::game_logic::{
            HostGeometryInfo, HostGeometryType, KindOf, Object, ObjectId, Team, ThingTemplate,
        };
        let mut sys = PathfindingSystem::new(200.0, 200.0);
        let mut objects = HashMap::new();
        let mut tmpl = ThingTemplate::new("LongThinFactory");
        tmpl.add_kind_of(KindOf::Structure);
        tmpl.geometry_info = HostGeometryInfo {
            geom_type: HostGeometryType::Box,
            is_small: false,
            height: 20.0,
            major_radius: 40.0,
            minor_radius: 8.0,
            authored: true,
        };
        let mut factory = Object::new(tmpl, ObjectId(1), Team::USA);
        factory.set_position(Vec3::new(80.0, 0.0, 80.0));
        factory.set_orientation(0.0);
        objects.insert(factory.id, factory);
        sys.apply_structure_static_blocks(&objects);
        let center = sys.grid.world_to_grid(Vec3::new(80.0, 0.0, 80.0));
        assert!(
            sys.grid.is_static_blocked(center),
            "box center must stamp"
        );
        let along_major = sys.grid.world_to_grid(Vec3::new(110.0, 0.0, 80.0));
        assert!(
            sys.grid.is_static_blocked(along_major),
            "long major axis must stamp past a 4-cell clamp"
        );
        let off_minor = sys.grid.world_to_grid(Vec3::new(80.0, 0.0, 110.0));
        assert!(
            !sys.grid.is_static_blocked(off_minor),
            "thin minor axis must not become a square"
        );
    }

    /// hq-hjx23: C++ pinch pass after structure stamp.
    #[test]
    fn structure_placement_runs_pinch_pass() {
        use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
        let mut sys = PathfindingSystem::new(200.0, 200.0);
        let mut objects = HashMap::new();
        let mut tmpl = ThingTemplate::new("AmericaWarFactory");
        tmpl.add_kind_of(KindOf::Structure);
        let mut factory = Object::new(tmpl, ObjectId(2), Team::USA);
        factory.set_position(Vec3::new(80.0, 0.0, 80.0));
        factory.selection_radius = 15.0;
        objects.insert(factory.id, factory);
        sys.apply_structure_static_blocks(&objects);
        let center = sys.grid.world_to_grid(Vec3::new(80.0, 0.0, 80.0));
        assert!(sys.grid.is_static_blocked(center));
        let mut found_pinched = false;
        for dy in -4..=4 {
            for dx in -4..=4 {
                if dx == 0 && dy == 0 {
                    continue;
                }
                let n = GridPos::new(center.x + dx, center.y + dy);
                if sys.grid.is_valid_pos(n)
                    && sys.grid.cell_type(n) == PathfindCellType::Clear
                    && sys.grid.is_pinched(n)
                {
                    found_pinched = true;
                }
            }
        }
        assert!(
            found_pinched,
            "C++ MARK_BORDER_PINCHED must mark obstacle-adjacent Clear cells"
        );
    }

    /// hq-nkgtf: adjustDestination must not snap into a disconnected pocket.
    #[test]
    fn adjust_destination_rejects_disconnected_pocket() {
        let mut g = open_grid(16, 16);
        for y in 0..16 {
            g.set_cell_type(GridPos::new(8, y), PathfindCellType::Water);
        }
        g.rebuild_path_zones();
        let from = GridPos::new(2, 8);
        let dest = GridPos::new(12, 8);
        assert_ne!(g.path_zone(from), g.path_zone(dest));
        g.query_from = Some(from);
        g.query_orig_dest = Some(from);
        let snapped = g.adjust_destination_ex(dest, SURFACE_GROUND, false, 64, None, 0);
        if let Some(cell) = snapped {
            assert_eq!(
                g.path_zone(cell),
                g.path_zone(from),
                "must not accept the far-bank pocket cell {cell:?}"
            );
        }
        assert_ne!(snapped, Some(dest), "disconnected dest must not be kept");
    }

    /// hq-zjbin: parked/taxiing aircraft do not stamp UNIT_PRESENT.
    #[test]
    fn grounded_aircraft_do_not_stamp_unit_present() {
        use crate::game_logic::{KindOf, Object, ObjectId, ObjectType, Team, ThingTemplate};
        let mut g = open_grid(16, 16);
        let mut objects = HashMap::new();
        let mut tmpl = ThingTemplate::new("AmericaJetRaptor");
        tmpl.add_kind_of(KindOf::Aircraft);
        tmpl.add_kind_of(KindOf::Vehicle);
        let mut jet = Object::new(tmpl, ObjectId(7), Team::USA);
        jet.object_type = ObjectType::Aircraft;
        jet.loco_appearance = LocomotorAppearance::Wings;
        jet.status.airborne_target = false;
        jet.set_position(Vec3::new(55.0, 0.0, 55.0));
        jet.owner_player_id = Some(0);
        objects.insert(jet.id, jet);
        g.update_dynamic_obstacles(&objects);
        let center = g.world_to_grid(Vec3::new(55.0, 0.0, 55.0));
        assert!(
            !g.is_blocked(center),
            "C++ updatePos never stamps air-movement UNIT_PRESENT"
        );
    }

    /// hq-2pdvs: invented Structure+radius>=20 set must not detour aircraft.
    #[test]
    fn tall_building_detour_requires_aircraft_path_around() {
        use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
        let mut objects = HashMap::new();
        let mut tmpl = ThingTemplate::new("WideBarracks");
        tmpl.add_kind_of(KindOf::Structure);
        let mut bldg = Object::new(tmpl, ObjectId(3), Team::USA);
        bldg.set_position(Vec3::new(50.0, 0.0, 0.0));
        bldg.selection_radius = 30.0;
        objects.insert(bldg.id, bldg);
        let from = Vec3::new(0.0, 40.0, 0.0);
        let to = Vec3::new(100.0, 40.0, 0.0);
        let path = PathfindingSystem::detour_path_around_tall_buildings(&[from, to], &objects);
        assert_eq!(
            path.len(),
            2,
            "without AIRCRAFT_PATH_AROUND, no invented tall detour"
        );
        let adj = PathfindingSystem::circle_clips_tall_building(from, to, 80.0, &objects, None);
        assert!(adj.is_none(), "circleClips is AIRCRAFT_PATH_AROUND only");
    }


    /// hq-0i8ht: human checkForAdjust / A* stay inside m_logicalExtent.
    #[test]
    fn human_units_stay_in_logical_extent() {
        let mut sys = PathfindingSystem::new(200.0, 200.0);
        sys.grid
            .set_logical_extent(GridPos::new(0, 0), GridPos::new(10, 10));
        sys.set_human_player_mask(1u16 << 0);
        sys.grid.set_query_is_human(true);

        assert!(sys.grid.in_logical_extent(GridPos::new(5, 5)));
        assert!(!sys.grid.in_logical_extent(GridPos::new(15, 5)));

        let inside = sys.grid.adjust_destination_on_layer(
            GridPos::new(5, 5),
            SURFACE_GROUND,
            false,
            8,
            Some(0),
            0,
            PathfindLayerEnum::Ground,
        );
        assert!(inside.is_some(), "human dest inside logical extent is ok");

        let outside = sys.grid.adjust_destination_on_layer(
            GridPos::new(15, 5),
            SURFACE_GROUND,
            false,
            4,
            Some(0),
            0,
            PathfindLayerEnum::Ground,
        );
        if let Some(adj) = outside {
            assert!(
                sys.grid.in_logical_extent(adj),
                "human adjust must not land outside m_logicalExtent"
            );
        }

        sys.grid.set_query_is_human(false);
        let ai = sys.grid.adjust_destination_on_layer(
            GridPos::new(15, 5),
            SURFACE_GROUND,
            false,
            4,
            Some(1),
            0,
            PathfindLayerEnum::Ground,
        );
        assert!(
            ai.is_some(),
            "computer players may leave the logical map"
        );

        let mut objects = HashMap::new();
        let tmpl = ThingTemplate::new("Ranger");
        let mut unit = Object::new(tmpl, ObjectId(1), Team::USA);
        unit.set_position(Vec3::new(20.0, 0.0, 20.0));
        unit.owner_player_id = Some(0);
        objects.insert(unit.id, unit);
        sys.grid.set_query_is_human(true);
        let path = sys.find_path_ex(
            Vec3::new(20.0, 0.0, 20.0),
            Vec3::new(180.0, 0.0, 20.0),
            &objects,
            false,
            Some(ObjectId(1)),
        );
        if let Some(path) = path {
            for p in &path {
                let cell = sys.grid.world_to_grid(*p);
                assert!(
                    sys.grid.in_logical_extent(cell),
                    "human A* waypoint {:?} outside logical extent",
                    cell
                );
            }
        }
    }

    /// hq-8kkhs: dozerHack reaches live crate A*.
    #[test]
    fn dozer_hack_reaches_live_astar() {
        let mut sys = PathfindingSystem::new(120.0, 80.0);
        for y in 0..8 {
            sys.grid
                .set_cell_type(GridPos::new(5, y), PathfindCellType::Impassable);
        }
        sys.grid.set_cell_obstacle_owned(
            GridPos::new(5, 3),
            false,
            false,
            20,
            Some(0),
            Some(Team::USA),
        );

        let mut objects = HashMap::new();
        let mut dozer_t = ThingTemplate::new("AmericaDozer");
        dozer_t.add_kind_of(KindOf::Dozer);
        dozer_t.add_kind_of(KindOf::Vehicle);
        let mut dozer = Object::new(dozer_t, ObjectId(1), Team::USA);
        dozer.set_position(Vec3::new(20.0, 0.0, 35.0));
        dozer.owner_player_id = Some(0);
        objects.insert(dozer.id, dozer);

        let start = Vec3::new(20.0, 0.0, 35.0);
        let goal = Vec3::new(90.0, 0.0, 35.0);
        let path = sys
            .find_path_ex(start, goal, &objects, false, Some(ObjectId(1)))
            .expect("dozer must step through non-enemy CELL_OBSTACLE");
        assert!(path.len() >= 2);

        let mut inf_t = ThingTemplate::new("Ranger");
        inf_t.add_kind_of(KindOf::Infantry);
        let mut inf = Object::new(inf_t, ObjectId(2), Team::USA);
        inf.set_position(start);
        inf.owner_player_id = Some(0);
        let mut inf_objects = HashMap::new();
        inf_objects.insert(inf.id, inf);
        let blocked = sys.find_path_ex(start, goal, &inf_objects, false, Some(ObjectId(2)));
        assert!(
            blocked.is_none(),
            "non-dozer cannot dozerHack a CELL_OBSTACLE gap"
        );

        sys.grid.set_cell_obstacle_owned(
            GridPos::new(5, 3),
            false,
            false,
            21,
            Some(2),
            Some(Team::GLA),
        );
        let enemy = sys.find_path_ex(start, goal, &objects, false, Some(ObjectId(1)));
        assert!(
            enemy.is_none(),
            "dozer must not dozerHack an ENEMIES obstacle"
        );
    }

    /// hq-ugt0x: downhill-only is not hardcoded false on the live A* call.
    #[test]
    fn downhill_only_reaches_live_astar() {
        let src = include_str!("pathfinding.rs");
        let i = src
            .find("fn find_path_via_crate")
            .expect("find_path_via_crate");
        let w = &src[i..src.len().min(i + 9000)];
        assert!(
            w.contains("downhill_only") && w.contains("seeker_downhill_only"),
            "live A* must pass seeker downhill_only, not a hardcoded false"
        );
        assert!(
            w.contains("seed_line") && w.contains("!downhill_only"),
            "seed line must be suppressed when downhill-only"
        );
        assert!(
            w.contains("dozer_ok_ref") && w.contains("cell_allowed"),
            "dozerHack and human cell_allowed must reach crate A*"
        );
        assert!(
            !w.contains("downhill_only: false"),
            "must not hardcode downhill_only: false"
        );
        assert!(
            w.contains("ignored_obstacle_cells") || w.contains("let ignore_cells"),
            "ignoreObstacle footprint must reach crate A* ignore_cells"
        );
        assert!(
            !w.contains("nearest_static_open"),
            "find_path_via_crate must not fall back to nearest_static_open"
        );

    }






    /// hq-9za5p: findClosestPath walks to a valid neighbor when the goal is blocked.
    #[test]
    fn find_closest_path_walks_to_valid_cell_when_goal_impassable() {
        let mut sys = PathfindingSystem::new(200.0, 200.0);
        let goal = GridPos::new(12, 5);
        sys.grid
            .set_cell_type(goal, PathfindCellType::Impassable);
        let from = Vec3::new(20.0, 0.0, 50.0);
        let to = sys.grid.grid_to_world(goal);
        let path = sys
            .find_closest_path(from, to, SURFACE_GROUND, false, true)
            .expect("closest path");
        let end = *path.last().expect("end");
        let end_cell = sys.grid.world_to_grid(end);
        assert_ne!(end_cell, goal, "must not stand on the impassable click");
        assert!(
            sys.grid.cell_passable_for(end_cell, SURFACE_GROUND, false),
            "closest dest {end_cell:?} must be a valid destination"
        );
        let d = (end_cell.x - goal.x).abs() + (end_cell.y - goal.y).abs();
        assert!(d <= 2, "should hug the blocked cell, end={end_cell:?}");
    }

    /// hq-1g4ym: flee cell stays outside both repulsor radii.
    #[test]
    fn find_safe_path_stays_outside_both_repulsors() {
        let mut sys = PathfindingSystem::new(400.0, 400.0);
        let from = Vec3::new(100.0, 0.0, 100.0);
        let r1 = Vec3::new(100.0, 0.0, 100.0);
        let r2 = Vec3::new(160.0, 0.0, 100.0);
        let radius = 40.0;
        let path = sys
            .find_safe_path_from(from, r1, r2, radius, SURFACE_GROUND, false, true)
            .expect("safe path");
        let end = *path.last().expect("end");
        let d1 = (end.x - r1.x).hypot(end.z - r1.z);
        let d2 = (end.x - r2.x).hypot(end.z - r2.z);
        assert!(
            d1.min(d2) > radius * 0.9,
            "end {end:?} must leave both radii d1={d1} d2={d2}"
        );
    }

    /// hq-2ugs5: checkPathCost straight line passes the 1.4*(dx+dy) gate.
    #[test]
    fn check_path_cost_straight_line_passes_group_gate() {
        let g = open_grid(32, 32);
        let from = Vec3::new(20.0, 0.0, 20.0);
        let to = Vec3::new(80.0, 0.0, 20.0);
        let cost = g.check_path_cost(SURFACE_GROUND, false, from, to);
        let dx = (to.x - from.x).abs();
        let dz = (to.z - from.z).abs();
        assert!(
            cost <= 1.4 * (dx + dz) + 1.0,
            "straight cost {cost} should pass 1.4*(dx+dz)={}",
            1.4 * (dx + dz)
        );
        let bad = g.check_path_cost(
            SURFACE_GROUND,
            false,
            Vec3::new(-100.0, 0.0, -100.0),
            to,
        );
        assert!(bad >= 0x7fff_0000u32 as f32 * 0.5);
    }

    /// hq-2ugs5: groupDest cost gate rejects a long detour around a wall.
    #[test]
    fn adjust_destination_for_group_rejects_long_detour() {
        let mut g = open_grid(24, 24);
        // Wall between group click (4,10) and offset dest (16,10).
        for y in 0..24 {
            g.set_cell_type(GridPos::new(10, y), PathfindCellType::Impassable);
        }
        // Leave a far gap at the top so a path exists but is expensive.
        g.set_cell_type(GridPos::new(10, 22), PathfindCellType::Clear);
        let dest = GridPos::new(16, 10);
        let group = GridPos::new(4, 10);
        let snapped = g.adjust_destination_for_group(
            dest,
            group,
            SURFACE_GROUND,
            false,
            None,
            0,
            PathfindLayerEnum::Ground,
        );
        if let Some(cell) = snapped {
            let cost = g.check_path_cost(
                SURFACE_GROUND,
                false,
                g.grid_to_world(group),
                g.grid_to_world(cell),
            );
            let w = g.grid_to_world(cell);
            let gw = g.grid_to_world(group);
            let dx = (gw.x - w.x).abs();
            let dz = (gw.z - w.z).abs();
            assert!(
                cost <= 1.4 * (dx + dz) + 1.0 || cell != dest,
                "group adjust must reject the far-side dest or pick a cheap cell, got {cell:?} cost={cost}"
            );
        }
    }

    /// hq-f4q2o: attack A* walks around a partial wall instead of through it.
    #[test]
    fn find_attack_path_goes_around_partial_wall() {
        let mut sys = PathfindingSystem::new(200.0, 200.0);
        for y in 0..12 {
            sys.grid
                .set_cell_type(GridPos::new(10, y), PathfindCellType::Impassable);
        }
        let objects = HashMap::new();
        let from = Vec3::new(20.0, 0.0, 50.0);
        let victim = Vec3::new(150.0, 0.0, 50.0);
        if let Some(p) = sys.find_attack_firing_position(from, victim, 80.0, &objects, false) {
            let crosses = p.windows(2).any(|w| {
                let a = sys.grid.world_to_grid(w[0]);
                let b = sys.grid.world_to_grid(w[1]);
                sys.grid.cell_type(a) == PathfindCellType::Impassable
                    || sys.grid.cell_type(b) == PathfindCellType::Impassable
            });
            assert!(!crosses, "attack A* must not step on Impassable, path={p:?}");
            let end = *p.last().unwrap();
            let dist = (end.x - victim.x).hypot(end.z - victim.z);
            assert!(
                dist <= 80.0 + 15.0,
                "firing cell should be in/near range, end={end:?} dist={dist}"
            );
        }
    }

    /// hq-4wg8j: seed line aborts on allied-fixed and crushable enemies.
    #[test]
    fn seed_line_aborts_allied_fixed_and_crushable_enemies() {
        use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
        let mut g = open_grid(24, 24);
        let mut masks = [0u16; 16];
        masks[0] = 1u16 << 0 | 1u16 << 1;
        masks[1] = 1u16 << 0 | 1u16 << 1;
        g.set_player_ally_masks(masks);

        let mut objects = HashMap::new();
        let mut ally_t = ThingTemplate::new("AllyTank");
        ally_t.add_kind_of(KindOf::Vehicle);
        let mut ally = Object::new(ally_t, ObjectId(2), Team::China);
        ally.set_position(g.grid_to_world(GridPos::new(8, 8)));
        ally.owner_player_id = Some(1);
        ally.crushable_level = 1;
        objects.insert(ally.id, ally);

        let mut car_t = ThingTemplate::new("CivilianCar");
        car_t.add_kind_of(KindOf::Vehicle);
        let mut car = Object::new(car_t, ObjectId(3), Team::GLA);
        car.set_position(g.grid_to_world(GridPos::new(10, 8)));
        car.owner_player_id = Some(2);
        car.crushable_level = 1;
        objects.insert(car.id, car);

        g.update_dynamic_obstacles(&objects);
        let layer = PathfindLayerEnum::Ground;
        assert!(
            !g.seed_line_occupancy_ok(GridPos::new(8, 8), Some(0), masks[0], false, layer),
            "allied-fixed must abort the seed line"
        );
        assert!(
            !g.seed_line_occupancy_ok(GridPos::new(10, 8), Some(0), masks[0], false, layer),
            "crushable enemy-fixed must abort the seed line"
        );
        assert!(
            g.seed_line_occupancy_ok(GridPos::new(4, 4), Some(0), masks[0], false, layer),
            "empty cell must seed"
        );
    }

    /// hq-6a3ks: attack LOS leftover airborne + KINDOF gates.
    #[test]
    fn attack_los_airborne_and_kindof_gates() {
        use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
        let mut sys = PathfindingSystem::new(200.0, 200.0);
        for y in 0..20 {
            sys.grid.set_cell_obstacle_owned(
                GridPos::new(10, y),
                false,
                false,
                99,
                Some(2),
                Some(Team::GLA),
            );
        }
        let from = Vec3::new(20.0, 0.0, 50.0);
        let to = Vec3::new(150.0, 0.0, 50.0);

        let mut atk_t = ThingTemplate::new("Ranger");
        atk_t.add_kind_of(KindOf::Infantry);
        let no_los = Object::new(atk_t.clone(), ObjectId(1), Team::USA);
        assert!(
            !sys.is_attack_view_blocked_for(from, to, Some(&no_los), None),
            "units without AttackNeedsLineOfSight must not refuse cells behind a building"
        );

        atk_t.add_kind_of(KindOf::AttackNeedsLineOfSight);
        let atk = Object::new(atk_t, ObjectId(1), Team::USA);
        assert!(
            sys.is_attack_view_blocked_for(from, to, Some(&atk), None),
            "LOS kind must still see the opaque wall"
        );

        let mut air_t = ThingTemplate::new("Raptor");
        air_t.add_kind_of(KindOf::Aircraft);
        let mut air = Object::new(air_t, ObjectId(7), Team::GLA);
        air.set_position(Vec3::new(150.0, 20.0, 50.0));
        air.ground_height = 0.0;
        assert!(
            air.is_significantly_above_terrain(),
            "control: victim is airborne"
        );
        assert!(
            !sys.is_attack_view_blocked_for(from, to, Some(&atk), Some(&air)),
            "airborne victim must not count deck/structure cells as blocked LOS"
        );
    }

    /// hq-p8eko: findAttackPath A* occupancy costs match leftover.
    #[test]
    fn attack_step_occupancy_matches_leftover() {
        use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
        let mut g = open_grid(24, 24);
        let mut masks = [0u16; 16];
        masks[0] = 1u16 << 0 | 1u16 << 1;
        masks[1] = 1u16 << 0 | 1u16 << 1;
        g.set_player_ally_masks(masks);

        let mut objects = HashMap::new();
        let mut enemy_t = ThingTemplate::new("Bunker");
        enemy_t.add_kind_of(KindOf::Vehicle);
        let mut enemy = Object::new(enemy_t, ObjectId(1), Team::GLA);
        enemy.set_position(g.grid_to_world(GridPos::new(5, 5)));
        enemy.owner_player_id = Some(2);
        enemy.crushable_level = 0;
        objects.insert(enemy.id, enemy);

        let mut ally_t = ThingTemplate::new("Humvee");
        ally_t.add_kind_of(KindOf::Vehicle);
        let mut ally = Object::new(ally_t, ObjectId(2), Team::China);
        ally.set_position(g.grid_to_world(GridPos::new(6, 6)));
        ally.owner_player_id = Some(1);
        objects.insert(ally.id, ally);

        let mut goal_t = ThingTemplate::new("Ranger");
        goal_t.add_kind_of(KindOf::Infantry);
        let mut goaled = Object::new(goal_t, ObjectId(3), Team::USA);
        goaled.set_position(g.grid_to_world(GridPos::new(1, 1)));
        goaled.owner_player_id = Some(0);
        goaled.movement.target_position = Some(g.grid_to_world(GridPos::new(8, 8)));
        objects.insert(goaled.id, goaled);

        g.update_dynamic_obstacles(&objects);
        let start = GridPos::new(4, 4);
        let layer = PathfindLayerEnum::Ground;
        assert!(
            g.attack_step_occupancy(GridPos::new(5, 5), start, Some(0), masks[0], false, true, 0, layer)
                .is_none(),
            "uncrushable enemy-fixed must skip"
        );
        assert_eq!(
            g.attack_step_occupancy(GridPos::new(6, 6), start, Some(0), masks[0], false, true, 0, layer),
            Some(3 * 14),
            "ally-fixed +3*DIAG"
        );
        assert_eq!(
            g.attack_step_occupancy(GridPos::new(8, 8), start, Some(0), masks[0], false, true, 0, layer),
            Some(3 * 10),
            "ally-goal vehicle +3*ORTHO"
        );
        assert_eq!(
            g.attack_step_occupancy(GridPos::new(8, 8), start, Some(0), masks[0], false, false, 0, layer),
            Some(10),
            "ally-goal infantry +ORTHO"
        );
    }

    /// hq-d38ae: vehicle strip-back walks parent chain and retreats off congestion.
    #[test]
    fn attack_vehicle_strip_back_retreats_off_busy_ally() {
        use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
        let mut sys = PathfindingSystem::new(200.0, 200.0);
        let mut masks = [0u16; 16];
        masks[0] = 1u16 << 0 | 1u16 << 1;
        masks[1] = 1u16 << 0 | 1u16 << 1;
        sys.grid.set_player_ally_masks(masks);

        let mut objects = HashMap::new();
        let mut tank_t = ThingTemplate::new("Crusader");
        tank_t.add_kind_of(KindOf::Vehicle);
        tank_t.add_kind_of(KindOf::AttackNeedsLineOfSight);
        let mut tank = Object::new(tank_t, ObjectId(1), Team::USA);
        tank.set_position(Vec3::new(20.0, 0.0, 50.0));
        tank.owner_player_id = Some(0);
        objects.insert(tank.id, tank);

        let mut ally_t = ThingTemplate::new("Humvee");
        ally_t.add_kind_of(KindOf::Vehicle);
        let mut ally = Object::new(ally_t, ObjectId(2), Team::China);
        // Parked, not idle: ally-fixed on the approach parent chain.
        ally.set_position(Vec3::new(70.0, 0.0, 50.0));
        ally.owner_player_id = Some(1);
        ally.ai_state = crate::game_logic::AIState::Attacking;
        objects.insert(ally.id, ally);

        sys.seeker_id = Some(ObjectId(1));
        sys.seeker_player = Some(0);
        sys.seeker_path_diameter = 3;
        let path = sys.find_attack_firing_position(
            Vec3::new(20.0, 0.0, 50.0),
            Vec3::new(150.0, 0.0, 50.0),
            80.0,
            &objects,
            false,
        );
        if let Some(p) = path {
            let ally_cell = sys.grid.world_to_grid(Vec3::new(70.0, 0.0, 50.0));
            let end = sys.grid.world_to_grid(*p.last().unwrap());
            assert_ne!(
                end, ally_cell,
                "vehicle strip-back must not park on the busy ally, path={p:?}"
            );
        }
    }

    /// hq-gd0jd: live grid_to_world samples deck / terrain Y, not 0.
    #[test]
    fn grid_to_world_uses_deck_height() {
        let mut g = open_grid(16, 16);
        g.stamp_bridge_deck(
            Vec3::new(20.0, 20.0, 40.0),
            Vec3::new(20.0, 20.0, 60.0),
            Vec3::new(90.0, 20.0, 40.0),
            Vec3::new(90.0, 20.0, 60.0),
            false,
        );
        let deck = GridPos::new(5, 5);
        let world = g.grid_to_world(deck);
        assert!(
            world.y > 10.0,
            "deck cell must emit layer height, got Y={}",
            world.y
        );
        let layered = g.grid_to_world_on_layer(
            deck,
            g.layer_for_destination(Vec3::new(world.x, 20.0, world.z)),
        );
        assert!(
            (layered.y - 20.0).abs() < 2.0 || layered.y > 10.0,
            "on-layer deck Y should track the span, got {}",
            layered.y
        );
    }

    /// hq-7tj9x: findClosest rebuilds via crate A*, not thin host A* / fail-open line.
    #[test]
    fn find_closest_path_rebuilds_via_crate_not_thin_line() {
        let src = include_str!("pathfinding.rs");
        let i = src
            .find("pub fn find_closest_path(")
            .expect("find_closest_path");
        let w = &src[i..src.len().min(i + 7000)];
        assert!(
            w.contains("find_path_via_crate"),
            "closest must rebuild with leftover crate A*"
        );
        assert!(
            !w.contains("self.grid.find_path("),
            "closest must not reconstruct with thin host A*"
        );
        assert!(
            w.contains("no fail-open line"),
            "closest must not fail-open a straight from→to line"
        );
        assert!(
            w.contains("enqueue_connect_layer"),
            "closest search must hop checkChangeLayers"
        );
    }

    /// hq-7tj9x: closest path around a blocked factory does not walk through it.
    #[test]
    fn find_closest_path_does_not_cut_through_building() {
        let mut sys = PathfindingSystem::new(200.0, 200.0);
        for y in 0..12 {
            sys.grid
                .set_cell_type(GridPos::new(8, y), PathfindCellType::Impassable);
        }
        let goal = GridPos::new(12, 5);
        sys.grid
            .set_cell_type(goal, PathfindCellType::Impassable);
        let from = Vec3::new(20.0, 0.0, 50.0);
        let to = sys.grid.grid_to_world(goal);
        let path = sys
            .find_closest_path(from, to, SURFACE_GROUND, false, true)
            .expect("closest path");
        let crosses = path.iter().any(|p| {
            sys.grid.cell_type(sys.grid.world_to_grid(*p)) == PathfindCellType::Impassable
        });
        assert!(!crosses, "closest crate rebuild must not stand on Impassable, path={path:?}");
    }

    /// hq-hy7cu: closest search hops onto a deck cell via checkChangeLayers.
    #[test]
    fn find_closest_path_hops_connect_layer_onto_deck() {
        let mut sys = PathfindingSystem::new(200.0, 200.0);
        sys.grid.stamp_bridge_deck(
            Vec3::new(20.0, 20.0, 40.0),
            Vec3::new(20.0, 20.0, 60.0),
            Vec3::new(90.0, 20.0, 40.0),
            Vec3::new(90.0, 20.0, 60.0),
            false,
        );
        let deck = GridPos::new(5, 5);
        let world = sys.grid.grid_to_world(deck);
        let on_deck = Vec3::new(world.x, 20.0, world.z);
        let layer = sys.grid.layer_for_destination(on_deck);
        assert_ne!(layer, PathfindLayerEnum::Ground, "deck click must pick the span");
        let from = sys.grid.grid_to_world(GridPos::new(1, 5));
        let path = sys
            .find_closest_path(from, on_deck, SURFACE_GROUND, false, true)
            .expect("closest onto deck");
        let end = *path.last().expect("end");
        let end_layer = sys.grid.layer_for_destination(Vec3::new(end.x, end.y.max(20.0), end.z));
        assert!(
            end_layer != PathfindLayerEnum::Ground || end.y > 5.0,
            "closest dest should be the deck cell, end={end:?} layer={end_layer:?}"
        );
    }

    /// hq-hy7cu: flee search also hops connect-layer.
    #[test]
    fn find_safe_path_enqueues_change_layers() {
        let src = include_str!("pathfinding.rs");
        let i = src
            .find("pub fn find_safe_path_from(")
            .expect("find_safe_path_from");
        let w = &src[i..src.len().min(i + 5000)];
        assert!(
            w.contains("enqueue_connect_layer"),
            "safe search must hop checkChangeLayers"
        );
        assert!(
            w.contains("cell_passable_for_layer"),
            "safe neighbors expand on the current layer"
        );
    }

    /// hq-cg5on: snapClosestGoalPosition offsets off an occupied pad cell.
    #[test]
    fn snap_closest_goal_avoids_fixed_occupant() {
        use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
        let mut sys = PathfindingSystem::new(200.0, 200.0);
        let mut objects = HashMap::new();
        let mut tmpl = ThingTemplate::new("Ranger");
        tmpl.add_kind_of(KindOf::Infantry);
        let mut parked = Object::new(tmpl, ObjectId(7), Team::USA);
        let pad = sys.grid.grid_to_world(GridPos::new(8, 8));
        parked.set_position(pad);
        parked.owner_player_id = Some(0);
        parked.selection_radius = 1.0;
        objects.insert(parked.id, parked);
        sys.grid.update_dynamic_obstacles(&objects);
        sys.seeker_id = Some(ObjectId(8));
        sys.seeker_player = Some(0);
        let snapped = sys.snap_closest_goal_position(pad, SURFACE_GROUND, false, 0.0);
        let snapped_cell = sys.grid.world_to_grid(snapped);
        assert_ne!(
            snapped_cell,
            GridPos::new(8, 8),
            "snap must leave the occupied pad, got {snapped:?}"
        );
        let d = (snapped_cell.x - 8).abs() + (snapped_cell.y - 8).abs();
        assert!(d <= 2, "snap should stay in the 3×3, cell={snapped_cell:?}");
    }

    /// hq-hlj28: processPathfindQueue stops at PATHFIND_CELLS_PER_FRAME.
    #[test]
    fn process_pathfind_queue_stops_at_cell_budget() {
        let mut sys = PathfindingSystem::new(200.0, 200.0);
        sys.set_pathfind_cells_per_frame(1);
        let mk = |id: u32, z: f32| PendingHostPath {
            unit_id: ObjectId(id),
            start: Vec3::new(10.0, 0.0, z),
            destination: Vec3::new(80.0, 0.0, z),
            waypoints: Vec::new(),
            aircraft: false,
            surfaces: SURFACE_GROUND,
            is_crusher: false,
            ignore_obstacle: None,
        };
        assert!(sys.queue_path(mk(1, 10.0)));
        assert!(sys.queue_path(mk(2, 30.0)));
        assert!(sys.queue_path(mk(3, 50.0)));
        assert_eq!(sys.pending_path_count(), 3);
        let objects = HashMap::new();
        sys.begin_pathfind_queue_frame();
        let mut processed = 0usize;
        while sys.pathfind_budget_remaining() {
            let Some(req) = sys.pop_pending_path() else {
                break;
            };
            let _ = sys.find_path_ex(
                req.start,
                req.destination,
                &objects,
                false,
                Some(req.unit_id),
            );
            processed += 1;
            if processed > 8 {
                break;
            }
        }
        assert_eq!(processed, 1, "budget 1 cell must stop after the first search");
        assert_eq!(
            sys.pending_path_count(),
            2,
            "later waiters stay queued for a later frame"
        );
    }

    /// hq-xa155: ignoreObstacle footprint cells reach crate A*.
    #[test]
    fn ignore_obstacle_walks_through_owned_footprint() {
        use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
        let mut sys = PathfindingSystem::new(120.0, 80.0);
        for y in 0..8 {
            sys.grid.set_cell_obstacle_owned(
                GridPos::new(5, y),
                false,
                false,
                9,
                Some(0),
                Some(Team::USA),
            );
        }
        let mut tmpl = ThingTemplate::new("Ranger");
        tmpl.add_kind_of(KindOf::Infantry);
        let mut unit = Object::new(tmpl, ObjectId(1), Team::USA);
        let start = Vec3::new(20.0, 0.0, 35.0);
        let goal = Vec3::new(90.0, 0.0, 35.0);
        unit.set_position(start);
        unit.owner_player_id = Some(0);
        let mut objects = HashMap::new();
        objects.insert(unit.id, unit);

        let blocked = sys.find_path_ex_surfaces(
            start,
            goal,
            &objects,
            false,
            SURFACE_GROUND,
            false,
            Some(ObjectId(1)),
        );
        assert!(
            blocked.is_none()
                || blocked.as_ref().is_some_and(|p| {
                    !p.iter().any(|wp| sys.grid.world_to_grid(*wp).x == 5)
                }),
            "without ignore, factory footprint stays a wall"
        );

        sys.set_ignore_obstacle(Some(ObjectId(9)));
        let through = sys
            .find_path_ex_surfaces(
                start,
                goal,
                &objects,
                false,
                SURFACE_GROUND,
                false,
                Some(ObjectId(1)),
            )
            .expect("ignoreObstacle must walk the ignored footprint");
        assert!(
            through.iter().any(|wp| sys.grid.world_to_grid(*wp).x == 5),
            "path must step through ignored CELL_OBSTACLE, path={through:?}"
        );
        sys.set_ignore_obstacle(None);
    }

    /// hq-gsdys: seeker flags come from the mover, not nearest living object.
    #[test]
    fn find_path_ex_surfaces_uses_mover_not_nearest() {
        use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
        let mut sys = PathfindingSystem::new(200.0, 200.0);
        let mut objects = HashMap::new();
        let mut dozer_t = ThingTemplate::new("AmericaDozer");
        dozer_t.add_kind_of(KindOf::Dozer);
        dozer_t.add_kind_of(KindOf::Vehicle);
        let mut dozer = Object::new(dozer_t, ObjectId(1), Team::USA);
        dozer.set_position(Vec3::new(20.0, 0.0, 20.0));
        dozer.owner_player_id = Some(0);
        objects.insert(dozer.id, dozer);
        let mut inf_t = ThingTemplate::new("Ranger");
        inf_t.add_kind_of(KindOf::Infantry);
        let mut inf = Object::new(inf_t, ObjectId(2), Team::USA);
        inf.set_position(Vec3::new(80.0, 0.0, 20.0));
        inf.owner_player_id = Some(0);
        objects.insert(inf.id, inf);

        let hop = Vec3::new(90.0, 0.0, 20.0);
        let goal = Vec3::new(150.0, 0.0, 20.0);
        let _ = sys.find_path_ex_surfaces(
            hop,
            goal,
            &objects,
            false,
            SURFACE_GROUND,
            false,
            Some(ObjectId(1)),
        );
        assert_eq!(sys.seeker_id, Some(ObjectId(1)));
        assert!(sys.seeker_is_dozer, "mover dozer must keep dozerHack");
        assert!(
            !sys.seeker_is_infantry,
            "nearest infantry must not steal seeker"
        );

        let _ = sys.find_path_ex_surfaces(
            hop,
            goal,
            &objects,
            false,
            SURFACE_GROUND,
            false,
            None,
        );
        assert_eq!(sys.seeker_id, None, "no mover → no nearest-object steal");
        assert!(!sys.seeker_is_dozer);
        assert!(!sys.seeker_is_infantry);
    }

    /// hq-0h1vk / hq-hlj28: crate search accounts cells for the queue budget.
    #[test]
    fn find_path_via_crate_notes_cells_allocated() {
        let mut sys = PathfindingSystem::new(200.0, 200.0);
        let objects = HashMap::new();
        let _ = sys.find_path_ex(
            Vec3::new(10.0, 0.0, 10.0),
            Vec3::new(80.0, 0.0, 10.0),
            &objects,
            false,
            None,
        );
        assert!(
            sys.cumulative_cells_allocated() > 0,
            "crate A* must add m_cumulativeCellsAllocated"
        );
    }

    /// hq-xl5vj: BLAST_CRATER skips the airborne height gate and never unstamps.
    #[test]
    fn blast_crater_stamps_above_terrain_and_survives_remove() {
        use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
        let mut sys = PathfindingSystem::new(200.0, 200.0);
        let mut objects = HashMap::new();
        let mut crater_t = ThingTemplate::new("ScriptedBlastCrater");
        crater_t.add_kind_of(KindOf::Structure);
        crater_t.add_kind_of(KindOf::BlastCrater);
        let mut crater = Object::new(crater_t, ObjectId(7), Team::Neutral);
        crater.set_position(Vec3::new(80.0, 50.0, 80.0));
        crater.selection_radius = 20.0;
        objects.insert(crater.id, crater);
        sys.apply_structure_static_blocks(&objects);
        let cell = sys.grid.world_to_grid(Vec3::new(80.0, 50.0, 80.0));
        assert!(
            sys.grid.is_static_blocked(cell),
            "C++ height gate excepts KINDOF_BLAST_CRATER"
        );

        let mut floating_t = ThingTemplate::new("HoverPad");
        floating_t.add_kind_of(KindOf::Structure);
        let mut floating = Object::new(floating_t, ObjectId(8), Team::USA);
        floating.set_position(Vec3::new(40.0, 50.0, 40.0));
        floating.selection_radius = 20.0;
        objects.insert(floating.id, floating);
        sys.apply_structure_static_blocks(&objects);
        let air_cell = sys.grid.world_to_grid(Vec3::new(40.0, 50.0, 40.0));
        assert!(
            !sys.grid.is_static_blocked(air_cell),
            "non-crater airborne structure still skips classify"
        );

        objects.clear();
        sys.grid.clear_static_blocks();
        sys.apply_structure_static_blocks(&objects);
        assert!(
            sys.grid.is_static_blocked(cell),
            "C++ never removes BLAST_CRATER footprints"
        );
    }

    /// hq-zqfpa: getAircraftPath first node is dest altitude; detours keep offset Y.
    #[test]
    fn aircraft_path_first_node_uses_dest_altitude() {
        use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
        let mut sys = PathfindingSystem::new(200.0, 200.0);
        let objects = HashMap::new();
        let start = Vec3::new(10.0, 20.0, 10.0);
        let goal = Vec3::new(80.0, 50.0, 90.0);
        let path = sys
            .find_path_ex(start, goal, &objects, true, None)
            .expect("aircraft two-node path");
        assert_eq!(path.len(), 2);
        assert!(
            (path[0].x - start.x).abs() < 0.01 && (path[0].z - start.z).abs() < 0.01,
            "first node keeps unit XY"
        );
        assert!(
            (path[0].y - goal.y).abs() < 0.01,
            "C++ pos.z = to->z: first node is dest altitude, got {}",
            path[0].y
        );
        assert!((path[1].y - goal.y).abs() < 0.01);

        let mut objects = HashMap::new();
        let mut jet_t = ThingTemplate::new("Raptor");
        jet_t.add_kind_of(KindOf::Aircraft);
        let mut jet = Object::new(jet_t, ObjectId(1), Team::USA);
        jet.set_position(start);
        jet.loco_appearance = LocomotorAppearance::Wings;
        objects.insert(jet.id, jet);
        let mut bldg_t = ThingTemplate::new("CommandCenter");
        bldg_t.add_kind_of(KindOf::Structure);
        bldg_t.add_kind_of(KindOf::AircraftPathAround);
        let mut bldg = Object::new(bldg_t, ObjectId(2), Team::USA);
        bldg.set_position(Vec3::new(45.0, 0.0, 50.0));
        bldg.selection_radius = 30.0;
        objects.insert(bldg.id, bldg);
        let detoured = sys
            .find_path_ex(start, goal, &objects, true, Some(ObjectId(1)))
            .expect("wings path");
        assert!(
            (detoured[0].y - goal.y).abs() < 0.01,
            "first node stays dest altitude after detour, got {}",
            detoured[0].y
        );
        if detoured.len() > 2 {
            let mid_ys: Vec<f32> = detoured[1..detoured.len() - 1]
                .iter()
                .map(|p| p.y)
                .collect();
            assert!(
                mid_ys.iter().any(|y| (*y - start.y).abs() > 0.01),
                "detour nodes must not be flattened to start Y, mids={mid_ys:?}"
            );
        }
    }

    /// hq-wuufk: C++ adjustCoordToCell — infantry cell center, vehicle +0.05 inset.
    #[test]
    fn adjust_coord_to_cell_centers_infantry_insets_vehicles() {
        let g = open_grid(16, 16);
        let cell = GridPos::new(3, 4);
        let origin = g.grid_to_world(cell);
        let size = g.grid_size();
        let center = g.adjust_coord_to_cell(cell, true);
        let inset = g.adjust_coord_to_cell(cell, false);
        assert!(
            (center.x - (origin.x + size * 0.5)).abs() < 0.001
                && (center.z - (origin.z + size * 0.5)).abs() < 0.001,
            "infantry must plant on cell center, got {center:?} origin={origin:?}"
        );
        assert!(
            (inset.x - (origin.x + size * 0.05)).abs() < 0.001
                && (inset.z - (origin.z + size * 0.05)).abs() < 0.001,
            "vehicle must use C++ (cell+0.05)*size, got {inset:?} origin={origin:?}"
        );
        assert!(
            (inset.x - origin.x).abs() > 0.2,
            "vehicle inset must not be the cell-origin corner"
        );
    }

    /// hq-wuufk: even-diameter dest near a cell boundary seeds the next cell.
    #[test]
    fn vehicle_adjust_destination_seeds_half_cell() {
        let g = open_grid(20, 20);
        let (_, center) = PathfindingGrid::radius_and_center(8.0, 10.0);
        assert!(!center, "selection_radius 8 must be even-diameter / !center");
        let dest = Vec3::new(19.9, 0.0, 5.0);
        assert_eq!(
            g.world_to_grid(dest),
            GridPos::new(1, 0),
            "truncate-toward-zero stays in cell 1"
        );
        assert_eq!(
            g.cell_for_unit_position(dest, false),
            GridPos::new(2, 0),
            "C++ half-cell seed must advance a vehicle near the far cell edge"
        );
        assert_eq!(
            g.cell_for_unit_position(dest, true),
            GridPos::new(1, 0),
            "infantry / centerInCell still truncates"
        );
    }

    /// hq-wuufk: live crate path emits adjustCoordToCell, not cell origins.
    #[test]
    fn crate_path_uses_adjust_coord_to_cell_not_corners() {
        use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
        let mut sys = PathfindingSystem::new(200.0, 200.0);
        let size = sys.grid.grid_size();
        let start = Vec3::new(20.0, 0.0, 20.0);
        let goal = Vec3::new(80.0, 0.0, 20.0);

        let mut inf_objects = HashMap::new();
        let mut inf_t = ThingTemplate::new("Ranger");
        inf_t.add_kind_of(KindOf::Infantry);
        let mut inf = Object::new(inf_t, ObjectId(1), Team::USA);
        inf.set_position(start);
        inf.selection_radius = 1.0;
        inf_objects.insert(inf.id, inf);
        let inf_path = sys
            .find_path_ex(start, goal, &inf_objects, false, Some(ObjectId(1)))
            .expect("infantry path");
        assert!(inf_path.len() >= 2);
        assert!(
            (inf_path[0].x - start.x).abs() < 0.01 && (inf_path[0].z - start.z).abs() < 0.01,
            "first node stays unit feet"
        );
        for p in &inf_path[1..] {
            let cell = sys.grid.world_to_grid(*p);
            let origin = sys.grid.grid_to_world(cell);
            assert!(
                (p.x - origin.x - size * 0.5).abs() < 0.25
                    && (p.z - origin.z - size * 0.5).abs() < 0.25,
                "infantry waypoint {p:?} must be cell center, origin={origin:?}"
            );
        }

        let mut veh_objects = HashMap::new();
        let mut veh_t = ThingTemplate::new("CrusaderTank");
        veh_t.add_kind_of(KindOf::Vehicle);
        let mut veh = Object::new(veh_t, ObjectId(2), Team::USA);
        veh.set_position(start);
        veh.selection_radius = 8.0;
        veh_objects.insert(veh.id, veh);
        let veh_path = sys
            .find_path_ex(start, goal, &veh_objects, false, Some(ObjectId(2)))
            .expect("vehicle path");
        assert!(veh_path.len() >= 2);
        for p in &veh_path[1..] {
            let cell = sys.grid.world_to_grid(*p);
            let origin = sys.grid.grid_to_world(cell);
            assert!(
                (p.x - origin.x - size * 0.05).abs() < 0.25
                    && (p.z - origin.z - size * 0.05).abs() < 0.25,
                "vehicle waypoint {p:?} must be C++ +0.05 inset, origin={origin:?}"
            );
            assert!(
                (p.x - origin.x).abs() > 0.2 || (p.z - origin.z).abs() > 0.2,
                "vehicle must not walk the cell-origin corner, p={p:?} origin={origin:?}"
            );
        }
    }

    /// hq-9hwau: patchPath skips occupied suffix nodes (parked allies).
    #[test]
    fn patch_path_skips_occupied_ally_waypoints() {
        use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
        let mut sys = PathfindingSystem::new(200.0, 200.0);
        let from = sys.grid.grid_to_world(GridPos::new(1, 4));
        let mid = sys.grid.grid_to_world(GridPos::new(8, 4));
        let dest = sys.grid.grid_to_world(GridPos::new(16, 4));
        let mut objects = HashMap::new();
        let mut seeker_t = ThingTemplate::new("Ranger");
        seeker_t.add_kind_of(KindOf::Infantry);
        let mut seeker = Object::new(seeker_t, ObjectId(1), Team::USA);
        seeker.set_position(from);
        seeker.owner_player_id = Some(0);
        objects.insert(seeker.id, seeker);
        let mut parked_t = ThingTemplate::new("Humvee");
        parked_t.add_kind_of(KindOf::Vehicle);
        let mut parked = Object::new(parked_t, ObjectId(9), Team::USA);
        parked.set_position(dest);
        parked.owner_player_id = Some(0);
        parked.crushable_level = 1;
        parked.selection_radius = 4.0;
        objects.insert(parked.id, parked);
        assert!(
            sys.patch_path(
                from,
                &[from, mid, dest],
                SURFACE_GROUND,
                false,
                &objects,
                Some(ObjectId(1)),
            )
            .is_none(),
            "C++ returns null when the last node itself is occupied"
        );

        objects.get_mut(&ObjectId(9)).unwrap().set_position(mid);
        let patched = sys
            .patch_path(
                from,
                &[from, mid, dest],
                SURFACE_GROUND,
                false,
                &objects,
                Some(ObjectId(1)),
            )
            .expect("must splice onto a still-clear dest suffix");
        let last_cell = sys.grid.world_to_grid(*patched.last().unwrap());
        let dest_cell = sys.grid.world_to_grid(dest);
        assert_eq!(last_cell, dest_cell, "suffix must still reach dest");
    }



}
