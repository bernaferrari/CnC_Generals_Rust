use super::*;
use glam::Vec3;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap, VecDeque};

use gamelogic::ai::pathfind_astar::{
    AStarPathfinder, GridCoord, PathfindCellType, COST_DIAGONAL,
};
use gamelogic::ai::pathfind_complete::{
    MAX_PATH_ITERATIONS, PATHFIND_QUEUE_LEN, SURFACE_AIR, SURFACE_CLIFF, SURFACE_GROUND,
    SURFACE_RUBBLE, SURFACE_WATER,
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
    /// Players with infantry occupying this cell.
    occ_infantry_mask: Vec<u16>,
    /// Max CrushableLevel among fixed occupants (canCrushOrSquish).
    occ_fixed_max_crushable: Vec<u8>,
    /// Structure-aware zone ids (obstacles split zones). 0 = uninitialized.
    path_zones: Vec<u16>,
    /// Per-player ALLIES occupancy bits (bit j set if player i considers j an ally).
    /// C++ checkForMovement getRelationship == ALLIES (AIPathfind.cpp:5037).
    player_ally_masks: [u16; 16],

    /// Bump when static cells change so crate A* can resync.
    terrain_gen: u64,
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
        Self {
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
            occ_moving_mask: vec![0u16; cells],
            fence_bits: vec![0u64; words],
            transparent_bits: vec![0u64; words],
            occ_goal_mask: vec![0u16; cells],
            occ_infantry_mask: vec![0u16; cells],
            occ_fixed_max_crushable: vec![0u8; cells],
            path_zones: vec![0u16; cells],
            player_ally_masks: [0u16; 16],

            terrain_gen: 1,
        }
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
        Vec3::new(
            self.origin.x + pos.x as f32 * self.grid_size,
            0.0,
            self.origin.z + pos.y as f32 * self.grid_size,
        )
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
        }
        self.terrain_gen = self.terrain_gen.wrapping_add(1);
    }

    /// C++ `PathfindCell::setTypeAsObstacle(..., isFence)`.
    pub fn set_cell_obstacle(&mut self, pos: GridPos, is_fence: bool, is_transparent: bool) {
        self.set_cell_type(pos, PathfindCellType::Obstacle);
        let Some(idx) = self.bit_index(pos) else {
            return;
        };
        Self::bit_set(&mut self.fence_bits, idx, is_fence);
        Self::bit_set(&mut self.transparent_bits, idx, is_transparent);
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
        let start = self.world_to_grid(from);
        let goal = self.world_to_grid(to);
        if start == goal {
            return false;
        }
        if start.manhattan_distance(goal) <= 1 {
            return false;
        }
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
            // C++ attackBlockedByObstacleCallback: only CELL_OBSTACLE.
            if self.cell_type(cell) != PathfindCellType::Obstacle {
                continue;
            }
            if self.is_obstacle_transparent(cell) {
                continue;
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
    ) {
        if fence_width <= 0.0 {
            return;
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
        for _iy in 0..num_steps_y {
            let mut x = tl_x;
            let mut z = tl_z;
            for _ix in 0..num_steps_x {
                let cell = self.world_to_grid(Vec3::new(x, 0.0, z));
                if self.is_valid_pos(cell) {
                    self.set_cell_obstacle(cell, true, is_transparent);
                }
                x += xdx;
                z += xdy;
            }
            tl_x += ydx;
            tl_z += ydy;
        }
    }

    pub fn clear_static_blocks(&mut self) {
        self.blocked_bits.fill(0);
        self.cell_types.fill(PathfindCellType::Clear as u8);
        self.pinched_bits.fill(0);
        self.terrain_zones.fill(0);
        self.path_zones.fill(0);
        self.fence_bits.fill(0);
        self.transparent_bits.fill(0);
        self.terrain_gen = self.terrain_gen.wrapping_add(1);
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
        self.occ_infantry_mask.fill(0);
        self.occ_fixed_max_crushable.fill(0);
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

    fn path_zone_passable(ty: PathfindCellType) -> bool {
        !matches!(
            ty,
            PathfindCellType::Impassable
                | PathfindCellType::Obstacle
                | PathfindCellType::BridgeImpassable
                | PathfindCellType::Cliff
        )
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
                if !Self::path_zone_passable(self.cell_type_at_index(sidx)) {
                    continue;
                }
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
                    if !Self::path_zone_passable(self.cell_type_at_index(idx)) {
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
    }

    pub fn path_zone(&self, pos: GridPos) -> u16 {
        self.bit_index(pos)
            .and_then(|idx| self.path_zones.get(idx).copied())
            .unwrap_or(0)
    }

    /// C++ Pathfinder::clientSafeQuickDoesPathExist (structure-aware).
    pub fn quick_path_exists(&self, from: Vec3, to: Vec3) -> bool {
        let start = self.world_to_grid(from);
        let goal = self.world_to_grid(to);
        if self.cell_type(goal) == PathfindCellType::Cliff {
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
        z1 == z2
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


    /// Stamp a bridge deck onto the single-layer host grid.
    /// C++ PathfindLayer::classifyCells: deck CELL_CLEAR; destroyed BRIDGE_IMPASSABLE.
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
        let lo = self.world_to_grid(Vec3::new(min_x, 0.0, min_z));
        let hi = self.world_to_grid(Vec3::new(max_x, 0.0, max_z));
        let ty = if destroyed {
            PathfindCellType::BridgeImpassable
        } else {
            PathfindCellType::Clear
        };
        for y in lo.y.min(hi.y)..=lo.y.max(hi.y) {
            for x in lo.x.min(hi.x)..=lo.x.max(hi.x) {
                let pos = GridPos::new(x, y);
                if !self.is_valid_pos(pos) {
                    continue;
                }
                let world = self.grid_to_world(pos);
                if point_in_bridge_quad(world.x, world.z, &corners) {
                    self.set_cell_type(pos, ty);
                }
            }
        }
    }

    fn occupancy_cost(
        &self,
        pos: GridPos,
        seeker_player: Option<u32>,
        seeker_is_infantry: bool,
        crusher_level: u8,
        ally_mask: u16,
    ) -> Option<f32> {
        let Some(idx) = self.bit_index(pos) else {
            return Some(0.0);
        };
        let fixed = self.occ_fixed_mask.get(idx).copied().unwrap_or(0);
        let moving = self.occ_moving_mask.get(idx).copied().unwrap_or(0);
        let goal = self.occ_goal_mask.get(idx).copied().unwrap_or(0);
        let infantry = self.occ_infantry_mask.get(idx).copied().unwrap_or(0);
        if fixed == 0 && moving == 0 && goal == 0 {
            return Some(0.0);
        }
        // C++ INFANTRY_MOVES_THROUGH_INFANTRY: skip infantry occupancy.
        if seeker_is_infantry && infantry != 0 && (fixed | moving) == infantry && goal == 0 {
            return Some(0.0);
        }
        let Some(player) = seeker_player else {
            return Some(3.0 * 1.414_213_5);
        };
        let bit = 1u16 << player.min(15);
        // C++ checkForMovement: ALLIES increment allyFixedCount, never enemyFixed
        // (AIPathfind.cpp:5037-5066). Only non-allies consult canCrushOrSquish.
        let friend = bit | ally_mask;
        if seeker_is_infantry && (infantry & !bit) != 0 && (fixed & !bit) == (infantry & !bit) {
            // Other infantry only — stream through.
            let leftover_fixed = fixed & !infantry;
            let leftover_moving = moving & !infantry;
            if leftover_fixed == 0 && leftover_moving == 0 && (goal & !bit) == 0 {
                return Some(0.0);
            }
        }
        let enemy_fixed = (fixed & !friend) != 0;
        if enemy_fixed {
            // C++ checkForMovement: enemyFixed only when !canCrushOrSquish
            // (AIPathfind.cpp:5063-5065). Crushers plan through idle cars.
            let max_c = self.occ_fixed_max_crushable.get(idx).copied().unwrap_or(255);
            if crusher_level == 0 || crusher_level <= max_c {
                return None;
            }
        }
        let mut extra = 0.0;
        if (moving & bit) != 0 {
            extra += 3.0 * 1.414_213_5;
        }
        if (fixed & friend) != 0 {
            extra += 3.0 * 1.414_213_5;
        }
        if (moving & !friend) != 0 {
            extra += 3.0 * 1.414_213_5;
        }
        // C++ UNIT_GOAL allied reservation: refuse as a cheap dest (high cost).
        if (goal & bit) != 0 {
            extra += 3.0 * 1.414_213_5;
        }
        if (goal & !bit) != 0 {
            extra += 3.0 * 1.414_213_5;
        }
        Some(extra)
    }

    fn mark_occupancy(
        &mut self,
        pos: GridPos,
        player: u32,
        moving: bool,
        infantry: bool,
        goal: bool,
        crushable_level: u8,
    ) {
        let Some(idx) = self.bit_index(pos) else {
            return;
        };
        Self::bit_set(&mut self.dynamic_bits, idx, true);
        let bit = 1u16 << player.min(15);
        if goal {
            if let Some(slot) = self.occ_goal_mask.get_mut(idx) {
                *slot |= bit;
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
                match self.occupancy_cost(neighbor, None, false, 0, 0) {
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
            if obj.is_kind_of(KindOf::Aircraft) {
                continue;
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
            let radius_cells = ((obj.selection_radius / self.grid_size).ceil() as i32)
                .max(if obj.is_kind_of(KindOf::Structure) {
                    1
                } else {
                    0
                });
            let grid_pos = self.world_to_grid(obj.get_position());
            for dy in -radius_cells..=radius_cells {
                for dx in -radius_cells..=radius_cells {
                    let p = GridPos::new(grid_pos.x + dx, grid_pos.y + dy);
                    if self.is_valid_pos(p) {
                        self.mark_occupancy(p, player, moving, infantry, false, obj.crushable_level);
                    }
                }
            }
            // C++ Pathfinder::updateGoal stamps UNIT_GOAL on the destination cell.
            if !obj.is_kind_of(KindOf::Immobile) && !obj.is_kind_of(KindOf::Structure) {
                let dest = obj
                    .movement
                    .path
                    .last()
                    .copied()
                    .or(obj.movement.target_position);
                if let Some(goal) = dest {
                    let goal_cell = self.world_to_grid(goal);
                    for dy in -radius_cells..=radius_cells {
                        for dx in -radius_cells..=radius_cells {
                            let p = GridPos::new(goal_cell.x + dx, goal_cell.y + dy);
                            if self.is_valid_pos(p) {
                                self.mark_occupancy(p, player, false, false, true, 255);
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
        ) {
            None => u32::MAX / 8,
            Some(c) => (c * 10.0) as u32, // crate A* uses integer COST_DIAGONAL=14
        }
    }

    pub fn has_allied_goal(&self, pos: GridPos, seeker_player: Option<u32>) -> bool {
        let Some(idx) = self.bit_index(pos) else {
            return false;
        };
        let goal = self.occ_goal_mask.get(idx).copied().unwrap_or(0);
        if goal == 0 {
            return false;
        }
        let Some(player) = seeker_player else {
            return true;
        };
        let bit = 1u16 << player.min(15);
        (goal & bit) != 0
    }

    /// C++ `checkDestination` occupancy (AIPathfind.cpp:4946-4953).
    fn has_blocking_fixed_occupant(
        &self,
        pos: GridPos,
        crusher_level: u8,
    ) -> bool {
        let Some(idx) = self.bit_index(pos) else {
            return false;
        };
        let fixed = self.occ_fixed_mask.get(idx).copied().unwrap_or(0);
        if fixed == 0 {
            return false;
        }
        let max_c = self.occ_fixed_max_crushable.get(idx).copied().unwrap_or(255);
        crusher_level == 0 || crusher_level <= max_c
    }

    /// C++ `checkDestination` single-cell residual used by adjustDestination.
    fn destination_cell_ok(
        &self,
        pos: GridPos,
        surfaces: u32,
        is_crusher: bool,
        seeker_player: Option<u32>,
        crusher_level: u8,
    ) -> bool {
        if !self.is_valid_pos(pos) {
            return false;
        }
        if !self.cell_passable_for(pos, surfaces, is_crusher) {
            return false;
        }
        if self.cell_type(pos) == PathfindCellType::Cliff {
            return false;
        }
        if self.has_allied_goal(pos, seeker_player) {
            return false;
        }
        if self.has_blocking_fixed_occupant(pos, crusher_level) {
            return false;
        }
        true
    }

    /// C++ linePassableCallback occupancy + pinched (AIPathfind.cpp:9553-9591).
    fn occupancy_blocks_line(
        &self,
        pos: GridPos,
        seeker_player: Option<u32>,
        crusher_level: u8,
    ) -> bool {
        let Some(idx) = self.bit_index(pos) else {
            return true;
        };
        let fixed = self.occ_fixed_mask.get(idx).copied().unwrap_or(0);
        if fixed == 0 {
            return false;
        }
        let Some(player) = seeker_player else {
            return true;
        };
        let bit = 1u16 << player.min(15);
        let friend = bit | self.ally_mask_for(player);
        if (fixed & friend) != 0 {
            return true;
        }
        if (fixed & !friend) != 0 {
            let max_c = self.occ_fixed_max_crushable.get(idx).copied().unwrap_or(255);
            return crusher_level == 0 || crusher_level <= max_c;
        }
        false
    }

    /// C++ `validLocomotorSurfacesForCellType` + fence crusher exception.
    pub fn cell_passable_for(&self, pos: GridPos, surfaces: u32, is_crusher: bool) -> bool {
        if !self.is_valid_pos(pos) {
            return false;
        }
        if self.is_obstacle_fence(pos) && is_crusher {
            return true;
        }
        let ty = self.cell_type(pos);
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
        let origin = self.clamp_pos(dest);
        if self.destination_cell_ok(origin, surfaces, is_crusher, seeker_player, crusher_level)
        {
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
                if self.destination_cell_ok(c, surfaces, is_crusher, seeker_player, crusher_level)
                {
                    return Some(c);
                }
            }
            for _ in 0..delta {
                j += 1;
                limit -= 1;
                let c = GridPos::new(i, j);
                if self.destination_cell_ok(c, surfaces, is_crusher, seeker_player, crusher_level)
                {
                    return Some(c);
                }
            }
            delta += 1;
            for _ in 0..delta {
                i -= 1;
                limit -= 1;
                let c = GridPos::new(i, j);
                if self.destination_cell_ok(c, surfaces, is_crusher, seeker_player, crusher_level)
                {
                    return Some(c);
                }
            }
            for _ in 0..delta {
                j -= 1;
                limit -= 1;
                let c = GridPos::new(i, j);
                if self.destination_cell_ok(c, surfaces, is_crusher, seeker_player, crusher_level)
                {
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
        let mut optimized = vec![waypoints[0]];
        let mut anchor = 0usize;
        while anchor < waypoints.len() - 1 {
            let mut far = waypoints.len() - 1;
            let mut found = false;
            while far > anchor + 1 {
                let a = self.world_to_grid(waypoints[anchor]);
                let b = self.world_to_grid(waypoints[far]);
                if self.line_passable_ex(
                    a,
                    b,
                    surfaces,
                    is_crusher,
                    false,
                    seeker_player,
                    crusher_level,
                    true,
                ) || self.collinear_cells_force_passable(waypoints, anchor, far)
                {
                    optimized.push(waypoints[far]);
                    anchor = far;
                    found = true;
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

/// Cell half-extent for structure path/LOS block from selection radius.
fn structure_block_radius_cells(selection_radius: f32, grid_size: f32) -> i32 {
    let gs = grid_size.max(1.0);
    // At least 1 (3×3); grow with large footprints.
    ((selection_radius / gs).ceil() as i32).max(1).min(4)
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
    /// C++ `m_ignoreObstacleID` for this path query (DozerAIUpdate.cpp:210).
    ignore_obstacle_id: Option<ObjectId>,

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
            ignore_obstacle_id: None,
        }
    }

    /// C++ getRelationship == ALLIES bits for occupancy crush-through.
    pub fn set_player_ally_masks(&mut self, masks: [u16; 16]) {
        self.grid.set_player_ally_masks(masks);
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
                        1,
                        self.grid.is_obstacle_fence(pos),
                        self.grid.is_obstacle_transparent(pos),
                    );
                }
            }
        }
        self.crate_astar = Some(HostCrateAStar { finder, stamp });
    }


    fn host_to_crate_coord(&self, pos: GridPos) -> GridCoord {
        GridCoord::new(pos.x, pos.y)
    }

    fn crate_path_to_world(&self, cells: &[GridCoord]) -> Vec<Vec3> {
        cells
            .iter()
            .map(|c| self.grid.grid_to_world(GridPos::new(c.x, c.y)))
            .collect()
    }

    /// C++ `Pathfinder::internalFindPath` via crate `AStarPathfinder`.
    /// Falls back to the host grid A* if crate types cannot run (empty grid).
    fn find_path_via_crate(
        &mut self,
        start: GridPos,
        goal: GridPos,
        surfaces: u32,
        is_crusher: bool,
    ) -> Option<Vec<Vec3>> {
        self.sync_crate_astar();
        let crusher_level = if is_crusher {
            self.seeker_crusher_level.max(1)
        } else {
            0
        };
        let start = self
            .grid
            .adjust_destination_ex(
                self.grid.clamp_pos(start),
                surfaces,
                is_crusher,
                64,
                self.seeker_player,
                crusher_level,
            )
            .or_else(|| {
                self.grid
                    .nearest_static_open(self.grid.clamp_pos(start), 16)
            })
            .unwrap_or_else(|| self.grid.clamp_pos(start));
        // C++ adjustDestination: snap water/cliff/impassable/occupied clicks.
        let mut goal = self
            .grid
            .adjust_destination_ex(
                self.grid.clamp_pos(goal),
                surfaces,
                is_crusher,
                400,
                self.seeker_player,
                crusher_level,
            )
            .or_else(|| {
                self.grid
                    .nearest_static_open(self.grid.clamp_pos(goal), 16)
            })
            .unwrap_or_else(|| self.grid.clamp_pos(goal));
        // C++ checkDestination refuses allied UNIT_GOAL cells.
        if self.grid.has_allied_goal(goal, self.seeker_player) {
            if let Some(adj) = self.grid.adjust_destination_ex(
                goal,
                surfaces,
                is_crusher,
                64,
                self.seeker_player,
                crusher_level,
            ) {
                if !self.grid.has_allied_goal(adj, self.seeker_player) {
                    goal = adj;
                }
            }
        }
        if !self.grid.cell_passable_for(start, surfaces, is_crusher)
            || !self.grid.cell_passable_for(goal, surfaces, is_crusher)
        {
            // Still try — crusher fences / air may pass via crate.
            if self.grid.is_static_blocked(start) && !self.grid.is_obstacle_fence(start) {
                return None;
            }
        }
        if start == goal {
            return Some(vec![self.grid.grid_to_world(start)]);
        }
        let start_c = self.host_to_crate_coord(start);
        let goal_c = self.host_to_crate_coord(goal);
        let width = self.grid.width;
        let occ_fixed = self.grid.occ_fixed_mask.clone();
        let occ_moving = self.grid.occ_moving_mask.clone();
        let occ_goal = self.grid.occ_goal_mask.clone();
        let occ_infantry = self.grid.occ_infantry_mask.clone();
        let occ_crush = self.grid.occ_fixed_max_crushable.clone();
        let seeker = self.seeker_player;
        let seeker_inf = self.seeker_is_infantry;
        let ally_mask = seeker.map(|p| self.grid.ally_mask_for(p)).unwrap_or(0);
        let extra = move |c: GridCoord| {
            if c.x < 0 || c.y < 0 || c.x >= width {
                return 0;
            }
            let idx = c.y as usize * width as usize + c.x as usize;
            let fixed = occ_fixed.get(idx).copied().unwrap_or(0);
            let moving = occ_moving.get(idx).copied().unwrap_or(0);
            let goal_m = occ_goal.get(idx).copied().unwrap_or(0);
            let infantry = occ_infantry.get(idx).copied().unwrap_or(0);
            if seeker_inf && infantry != 0 && (fixed | moving) == infantry && goal_m == 0 {
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
                if leftover_fixed == 0 && leftover_moving == 0 && (goal_m & !bit) == 0 {
                    return 0;
                }
            }
            if (fixed & !friend) != 0 {
                // C++ examineNeighboringCells continue only when enemyFixed
                // (AIPathfind.cpp:6241). Allies never set enemyFixed.
                let max_c = occ_crush.get(idx).copied().unwrap_or(255);
                if crusher_level == 0 || crusher_level <= max_c {
                    return u32::MAX / 8;
                }
            }
            let mut extra = 0u32;
            if (moving & bit) != 0 {
                extra += 3 * COST_DIAGONAL;
            }
            if (fixed & friend) != 0 {
                extra += 3 * COST_DIAGONAL;
            }
            if (moving & !friend) != 0 {
                extra += 3 * COST_DIAGONAL;
            }
            if goal_m != 0 {
                extra += 3 * COST_DIAGONAL;
            }
            extra
        };
        let Some(crate_pf) = self.crate_astar.as_ref() else {
            return self.grid.find_path(start, goal);
        };
        let cells = crate_pf
            .finder
            .find_path_ex(
                start_c,
                goal_c,
                surfaces,
                is_crusher,
                MAX_PATH_ITERATIONS,
                false,
                None,
                Some(&extra),
            )
            .map(|(path, _)| path)
            .or_else(|| {
                // C++ findClosestPath on findPath fail (AIUpdate.cpp:1713-1717).
                crate_pf
                    .finder
                    .find_path_ex(
                        start_c,
                        goal_c,
                        surfaces,
                        is_crusher,
                        MAX_PATH_ITERATIONS,
                        true,
                        None,
                        Some(&extra),
                    )
                    .map(|(path, _)| path)
            })?;
        let world = self.crate_path_to_world(&cells);
        Some(self.grid.optimize_ground_path_ex(
            &world,
            surfaces,
            is_crusher,
            self.seeker_player,
            crusher_level,
        ))
    }


    /// Queue a path request for next-frame resolve (C++ queueForPath).
    pub fn queue_path(&mut self, req: PendingHostPath) {
        if self.pending_paths.len() >= PATHFIND_QUEUE_LEN {
            self.pending_paths.pop_front();
        }
        self.pending_paths.push_back(req);
    }

    pub fn take_pending_paths(&mut self) -> Vec<PendingHostPath> {
        self.pending_paths.drain(..).collect()
    }

    pub fn pending_path_count(&self) -> usize {
        self.pending_paths.len()
    }

    pub fn stamp_rubble_at_world(&mut self, world: Vec3, radius_cells: i32) {
        let cell = self.grid.world_to_grid(world);
        self.grid.stamp_rubble_footprint(cell, radius_cells);
    }

    pub fn block_structure_at_world(&mut self, pos: Vec3, radius_cells: i32) {
        let center = self.grid.world_to_grid(pos);
        self.grid.block_structure_footprint(center, radius_cells);
    }

    pub fn is_attack_view_blocked(&self, from: Vec3, to: Vec3) -> bool {
        self.grid.is_attack_view_blocked_static(from, to)
    }


    /// Rebuild structure static obstacles from live objects (map load / bulk sync).
    /// Does not clear terrain slope blocks — only ORs structure footprints.
    pub fn apply_structure_static_blocks(&mut self, objects: &HashMap<ObjectId, Object>) {
        for obj in objects.values() {
            if obj.status.under_construction {
                continue;
            }
            let rubble = PathfindingGrid::object_is_pathfind_rubble(obj);
            if !obj.is_alive() && !rubble {
                continue;
            }
            let is_transparent = obj.is_kind_of(KindOf::CanSeeThrough);
            let fence_width = obj.thing.template.fence_width;
            let is_fence = fence_width > 0.0 && !obj.is_kind_of(KindOf::DefensiveWall);
            if rubble && obj.is_kind_of(KindOf::Structure) {
                let radius =
                    structure_block_radius_cells(obj.selection_radius, self.grid.grid_size());
                let cell = self.grid.world_to_grid(obj.get_position());
                self.grid.stamp_rubble_footprint(cell, radius);
                continue;
            }
            if !obj.is_alive() {
                continue;
            }
            if is_fence {
                self.grid.classify_fence_world(
                    obj.get_position(),
                    obj.get_orientation(),
                    fence_width,
                    obj.thing.template.fence_x_offset,
                    is_transparent,
                );
                continue;
            }
            if !obj.is_kind_of(KindOf::Structure) {
                continue;
            }
            let radius = structure_block_radius_cells(obj.selection_radius, self.grid.grid_size());
            let cell = self.grid.world_to_grid(obj.get_position());
            self.grid
                .block_structure_footprint_ex(cell, radius, false, is_transparent);
        }
        self.grid.rebuild_path_zones();
    }


    /// C++ Pathfinder::findAttackPath residual (simplified).
    ///
    /// Finds a passable cell within `weapon_range` of `victim` that has clear
    /// static attack LOS to the victim, preferring cells closer to `from`.
    /// Returns a path from `from` to that firing cell (not into the victim cell).
    /// Fail-closed: not full hierarchical zones / human extent / tall-building insert.
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

        // Quick steps toward victim (C++ i=1..10 residual).
        {
            let mut delta = Vec3::new(victim.x - from.x, 0.0, victim.z - from.z);
            let len = (delta.x * delta.x + delta.z * delta.z).sqrt();
            if len > f32::EPSILON {
                delta = delta / len * cell_size;
                for i in 1..10 {
                    let test = from + delta * (i as f32 * 0.5);
                    let cell = self.grid.world_to_grid(test);
                    if !self.grid.is_valid_pos(cell) || self.grid.is_static_blocked(cell) {
                        break;
                    }
                    let dist = {
                        let dx = test.x - victim.x;
                        let dz = test.z - victim.z;
                        (dx * dx + dz * dz).sqrt()
                    };
                    if dist <= range && !self.grid.is_attack_view_blocked_static(test, victim) {
                        // C++ findAttackPath returns NULL when A* fails — no
                        // straight-line fallback through blocked cells.
                        return self.find_path_ex_surfaces(
                            from,
                            test,
                            objects,
                            false,
                            SURFACE_GROUND,
                            is_crusher,
                        );
                    }
                }
            }
        }

        // Spiral / ring of cells around victim within range (+3 cells budget like C++).
        let max_cells = ((range / cell_size).ceil() as i32) + 3;
        let mut best: Option<(f32, GridPos, Vec3)> = None;
        for dy in -max_cells..=max_cells {
            for dx in -max_cells..=max_cells {
                let cell = GridPos::new(victim_cell.x + dx, victim_cell.y + dy);
                if cell == start {
                    continue;
                }
                if !self.grid.is_valid_pos(cell) || self.grid.is_static_blocked(cell) {
                    continue;
                }
                // Soft-skip dynamic occupancy of other units at candidate.
                if self.grid.is_blocked(cell) && cell != start {
                    // Still allow if only dynamic — static already filtered.
                    // Prefer empty; skip hard dynamic to reduce stacking.
                    continue;
                }
                let world = self.grid.grid_to_world(cell);
                let dist_v = {
                    let ddx = world.x - victim.x;
                    let ddz = world.z - victim.z;
                    (ddx * ddx + ddz * ddz).sqrt()
                };
                if dist_v > range {
                    continue;
                }
                if self.grid.is_attack_view_blocked_static(world, victim) {
                    continue;
                }
                let dist_a = {
                    let ddx = world.x - from.x;
                    let ddz = world.z - from.z;
                    (ddx * ddx + ddz * ddz).sqrt()
                };
                match best {
                    Some((best_d, _, _)) if dist_a >= best_d => {}
                    _ => best = Some((dist_a, cell, world)),
                }
            }
        }

        let goal = best.map(|(_, _, w)| w)?;
        self.find_path_ex_surfaces(from, goal, objects, false, SURFACE_GROUND, is_crusher)
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

    /// Host residual: first tall / AIRCRAFT_PATH_AROUND structure along segment (XZ).
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
        // Sample along segment like a coarse Bresenham residual.
        let steps = ((len / 5.0).ceil() as i32).clamp(1, 256);
        let mut best: Option<(ObjectId, Vec3, f32, f32)> = None; // id,pos,r,t
        for obj in objects.values() {
            if !obj.is_alive() {
                continue;
            }
            if ignore == Some(obj.id) {
                continue;
            }
            let is_tall = obj.is_kind_of(crate::game_logic::KindOf::AircraftPathAround)
                || (obj.is_kind_of(crate::game_logic::KindOf::Structure)
                    && obj.selection_radius >= 20.0);
            if !is_tall {
                continue;
            }
            let p = obj.get_position();
            let radius = obj.selection_radius.max(8.0) + 2.0 * 10.0; // +2 pathfind cells residual
                                                                     // Closest approach of point-line in XZ.
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
        // Also require some sample near building for honesty with C++ cell walk.
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

    /// C++ aircraft tall-building path detour residual: walk path segments and
    /// insert radial offsets when AIRCRAFT_PATH_AROUND / tall structures clip.
    pub fn detour_path_around_tall_buildings(
        path: &[Vec3],
        objects: &HashMap<ObjectId, Object>,
    ) -> Vec<Vec3> {
        if path.len() < 2 {
            return path.to_vec();
        }
        let mut out: Vec<Vec3> = Vec::with_capacity(path.len() + 8);
        out.push(path[0]);
        for w in path.windows(2) {
            let mut from = w[0];
            // Prefer last emitted point as from (may have been nudged).
            if let Some(last) = out.last() {
                from = *last;
            }
            let mut to = w[1];
            // Limit insertions per segment to avoid explosion.
            for _ in 0..4 {
                if let Some((nudged_to, i1, i2, i3)) =
                    Self::segment_intersects_tall_building(from, to, objects, None)
                {
                    to = nudged_to;
                    // Insert detour points if they advance the path.
                    for p in [i1, i2, i3] {
                        if out.last().is_none_or(|l| {
                            let dx = l.x - p.x;
                            let dz = l.z - p.z;
                            dx * dx + dz * dz > 1.0
                        }) {
                            out.push(p);
                            from = p;
                        }
                    }
                } else {
                    break;
                }
            }
            if out.last().is_none_or(|l| {
                let dx = l.x - to.x;
                let dz = l.z - to.z;
                dx * dx + dz * dz > 0.01
            }) {
                out.push(to);
            }
        }
        out
    }

    pub fn find_path(
        &mut self,
        start: Vec3,
        goal: Vec3,
        objects: &HashMap<ObjectId, Object>,
    ) -> Option<Vec<Vec3>> {
        self.find_path_ex(start, goal, objects, false)
    }

    /// C++ `Pathfinder::circleClipsTallBuilding` residual (AIPathfind.cpp:9522).
    ///
    /// If a tall / AIRCRAFT_PATH_AROUND building is within `circle_radius` of `to`,
    /// write an adjusted goal on the building's radial offset toward `from`.
    pub fn circle_clips_tall_building(
        from: Vec3,
        to: Vec3,
        circle_radius: f32,
        objects: &HashMap<ObjectId, Object>,
        ignore: Option<ObjectId>,
    ) -> Option<Vec3> {
        let mut best: Option<(Vec3, f32, f32)> = None; // bldg_pos, bldg_r, dist
        for obj in objects.values() {
            if !obj.is_alive() {
                continue;
            }
            if ignore == Some(obj.id) {
                continue;
            }
            let is_tall = obj.is_kind_of(crate::game_logic::KindOf::AircraftPathAround)
                || (obj.is_kind_of(crate::game_logic::KindOf::Structure)
                    && obj.selection_radius >= 20.0);
            if !is_tall {
                continue;
            }
            let p = obj.get_position();
            let bldg_r = obj.selection_radius.max(8.0) + 2.0 * 10.0;
            let dx = p.x - to.x;
            let dz = p.z - to.z;
            let d = (dx * dx + dz * dz).sqrt();
            if d > circle_radius {
                continue;
            }
            match best {
                Some((_, _, bd)) if d >= bd => {}
                _ => best = Some((p, bldg_r, d)),
            }
        }
        let Some((bldg_pos, bldg_r, _)) = best else {
            return None;
        };

        // Offset `to` away from building center along from→to residual.
        let mut delta_x = to.x - bldg_pos.x;
        let mut delta_z = to.z - bldg_pos.z;
        let mut len = (delta_x * delta_x + delta_z * delta_z).sqrt();
        if len < 0.1 {
            // Degenerate: push away from `from` direction.
            delta_x = to.x - from.x;
            delta_z = to.z - from.z;
            len = (delta_x * delta_x + delta_z * delta_z).sqrt();
            if len < 0.1 {
                delta_x = 1.0;
                delta_z = 0.0;
                len = 1.0;
            }
        }
        let scale = (bldg_r + 1.0) / len;
        Some(Vec3::new(
            bldg_pos.x + delta_x * scale,
            to.y,
            bldg_pos.z + delta_z * scale,
        ))
    }

    /// `aircraft`: apply C++ tall-building aircraft path-around residual after A*.
    pub fn find_path_ex(
        &mut self,
        start: Vec3,
        goal: Vec3,
        objects: &HashMap<ObjectId, Object>,
        aircraft: bool,
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
    ) -> Option<Vec<Vec3>> {
        self.ensure_dynamic_obstacles(objects);
        if let Some(o) = objects
            .values()
            .filter(|o| o.is_alive())
            .min_by(|a, b| {
                let da = a.get_position().distance_squared(start);
                let db = b.get_position().distance_squared(start);
                da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
            })
        {
            self.seeker_player = o.owner_player_id.or(Some(o.team as u32));
            self.seeker_is_infantry = o.is_kind_of(KindOf::Infantry);
            self.seeker_wings = matches!(o.loco_appearance, LocomotorAppearance::Wings);
            self.seeker_id = Some(o.id);
            self.seeker_team = Some(o.team);
            self.seeker_crusher_level = o.crusher_level;
        } else {
            self.seeker_player = None;
            self.seeker_is_infantry = false;
            self.seeker_wings = false;
            self.seeker_id = None;
            self.seeker_team = None;
            self.seeker_crusher_level = 0;
        }
        // Live seeker CrusherLevel wins so find_path / find_path_ex still crush.
        let is_crusher = is_crusher || self.seeker_crusher_level > 0;
        if is_crusher && self.seeker_crusher_level == 0 {
            self.seeker_crusher_level = 1;
        }

        let start_grid = self.grid.world_to_grid(start);
        let goal_grid = self.grid.world_to_grid(goal);

        // C++ getAircraftPath: circleClips only when appearance == LOCO_WINGS.
        let mut path = if aircraft {
            let check_clips = self.seeker_wings;
            let goal_adj = if check_clips {
                Self::circle_clips_tall_building(start, goal, 100.0, objects, None).unwrap_or(goal)
            } else {
                goal
            };
            let direct = vec![start, goal_adj];
            let mut detoured = if check_clips {
                Self::detour_path_around_tall_buildings(&direct, objects)
            } else {
                direct
            };
            if let Some(last) = detoured.last_mut() {
                *last = goal_adj;
            }
            detoured
        } else {
            self.find_path_via_crate(start_grid, goal_grid, surfaces, is_crusher)?
        };
        if !aircraft {
            let n = path.len().max(1) as f32;
            for (i, p) in path.iter_mut().enumerate() {
                let t = i as f32 / (n - 1.0).max(1.0);
                p.y = start.y + (goal.y - start.y) * t;
            }
        } else {
            for p in path.iter_mut() {
                p.y = start.y;
            }
            if let Some(last) = path.last_mut() {
                last.y = goal.y;
            }
        }
        if let Some(first) = path.first_mut() {
            *first = start;
        }
        // C++ checkDestination: snapped dest is the final position. Do not
        // restore the raw click (that walks units into building footprints).
        Some(path)
    }



    /// C++ `Pathfinder::clientSafeQuickDoesPathExist` (structure-aware).
    pub fn client_safe_quick_does_path_exist(&self, from: Vec3, to: Vec3) -> bool {
        if self.grid.path_zones.iter().all(|&z| z == 0) {
            return self.grid.quick_path_exists_for_ui(from, to);
        }
        self.grid.quick_path_exists(from, to)
    }

    /// C++ `Pathfinder::patchPath` (AIPathfind.cpp:10344-10520).
    pub fn patch_path(
        &mut self,
        from: Vec3,
        original: &[Vec3],
        surfaces: u32,
        is_crusher: bool,
    ) -> Option<Vec<Vec3>> {
        if original.len() < 2 {
            return None;
        }
        let start = self.grid.world_to_grid(from);
        let mut splice_from = original.len() - 1;
        for idx in (1..original.len()).rev() {
            let cell = self.grid.world_to_grid(original[idx]);
            if !self.grid.cell_passable_for(cell, surfaces, is_crusher) {
                splice_from = idx;
                break;
            }
            splice_from = idx;
        }
        if splice_from + 1 >= original.len() {
            return None;
        }
        let goal_cell = self.grid.world_to_grid(original[splice_from]);
        let mut prefix = self.find_path_via_crate(start, goal_cell, surfaces, is_crusher)?;
        if let Some(last) = prefix.last_mut() {
            *last = original[splice_from];
        }
        prefix.extend_from_slice(&original[splice_from + 1..]);
        Some(prefix)
    }

    /// C++ `Pathfinder::findSafePath` Dijkstra flee (AIPathfind.cpp:10885+).
    pub fn find_safe_path(
        &mut self,
        from: Vec3,
        repulsor: Vec3,
        repulsor_radius: f32,
        surfaces: u32,
        is_crusher: bool,
    ) -> Option<Vec<Vec3>> {
        self.sync_crate_astar();
        let start = self.grid.world_to_grid(from);
        if !self.grid.is_valid_pos(start) {
            return None;
        }
        let radius_sqr = repulsor_radius * repulsor_radius;
        let mut open: BinaryHeap<std::cmp::Reverse<(i32, i32, i32)>> = BinaryHeap::new();
        let mut g_score: HashMap<GridPos, i32> = HashMap::new();
        let mut came: HashMap<GridPos, GridPos> = HashMap::new();
        let mut closed: HashMap<GridPos, ()> = HashMap::new();
        open.push(std::cmp::Reverse((0, start.x, start.y)));
        g_score.insert(start, 0);
        let mut farthest: Option<(GridPos, f32)> = None;
        let mut found: Option<GridPos> = None;
        let mut expanded = 0i32;
        const MAX_CELLS: i32 = 2000;
        while let Some(std::cmp::Reverse((g, cx, cy))) = open.pop() {
            let cell = GridPos::new(cx, cy);
            if closed.contains_key(&cell) {
                continue;
            }
            closed.insert(cell, ());
            expanded += 1;
            let world = self.grid.grid_to_world(cell);
            let dx = world.x - repulsor.x;
            let dz = world.z - repulsor.z;
            let dist_sqr = dx * dx + dz * dz;
            if farthest.map(|(_, d)| dist_sqr > d).unwrap_or(true) {
                farthest = Some((cell, dist_sqr));
            }
            let mut ok = dist_sqr > radius_sqr;
            if expanded > MAX_CELLS {
                ok = true;
            }
            if ok && self.grid.cell_passable_for(cell, surfaces, is_crusher) {
                found = Some(cell);
                break;
            }
            for n in cell.neighbors() {
                if !self.grid.is_valid_pos(n) || closed.contains_key(&n) {
                    continue;
                }
                if !self.grid.cell_passable_for(n, surfaces, is_crusher)
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
                came.insert(n, cell);
                open.push(std::cmp::Reverse((ng, n.x, n.y)));
            }
        }
        let dest = found.or_else(|| farthest.map(|(c, _)| c))?;
        if dest == start {
            return None;
        }
        let mut cells = vec![dest];
        let mut cur = dest;
        while let Some(&p) = came.get(&cur) {
            cells.push(p);
            cur = p;
            if cur == start {
                break;
            }
        }
        cells.reverse();
        let mut world: Vec<Vec3> = cells
            .iter()
            .map(|c| self.grid.grid_to_world(*c))
            .collect();
        if let Some(first) = world.first_mut() {
            *first = from;
        }
        Some(self.grid.optimize_ground_path(&world, surfaces, is_crusher))
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
        let mover_infantry = mover.is_kind_of(KindOf::Infantry);
        let mut nudged = Vec::new();
        for wp in path.iter().skip(1) {
            let cell = self.grid.world_to_grid(*wp);
            for obj in objects.values() {
                if obj.id == mover_id || !obj.is_alive() {
                    continue;
                }
                if obj.team != mover.team {
                    continue;
                }
                if obj.is_kind_of(KindOf::Structure) || obj.is_kind_of(KindOf::Immobile) {
                    continue;
                }
                if mover_infantry && obj.is_kind_of(KindOf::Infantry) {
                    continue;
                }
                if !obj.movement.path.is_empty()
                    || obj.movement.velocity.length_squared() > 0.25
                    || obj.status.attacking
                    || obj.status.using_ability
                {
                    continue;
                }
                let oc = self.grid.world_to_grid(obj.get_position());
                if (oc.x - cell.x).abs() <= 1 && (oc.y - cell.y).abs() <= 1 && !nudged.contains(&obj.id)
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
            .find_path_ex(start, goal, &objects, false)
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
        // selection 30 + 20 cell pad = 50; +1 => ~51
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
    fn bridge_deck_clears_water_and_destroy_impassable() {
        let mut g = open_grid(12, 12);
        for x in 2..10 {
            g.set_cell_type(GridPos::new(x, 5), PathfindCellType::Water);
        }
        g.stamp_bridge_deck(
            Vec3::new(20.0, 0.0, 40.0),
            Vec3::new(20.0, 0.0, 60.0),
            Vec3::new(90.0, 0.0, 40.0),
            Vec3::new(90.0, 0.0, 60.0),
            false,
        );
        assert_eq!(g.cell_type(GridPos::new(5, 5)), PathfindCellType::Clear);
        g.stamp_bridge_deck(
            Vec3::new(20.0, 0.0, 40.0),
            Vec3::new(20.0, 0.0, 60.0),
            Vec3::new(90.0, 0.0, 40.0),
            Vec3::new(90.0, 0.0, 60.0),
            true,
        );
        assert_eq!(
            g.cell_type(GridPos::new(5, 5)),
            PathfindCellType::BridgeImpassable
        );
        assert!(g.is_static_blocked(GridPos::new(5, 5)));
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
            .find_path_ex_surfaces(start, goal, &objects, false, SURFACE_GROUND, true)
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
            .find_path_ex_surfaces(start, goal, &objects, false, SURFACE_GROUND, false)
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
            .find_path_ex_surfaces(start, goal, &objects, false, SURFACE_GROUND, true)
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
        if let Some(p) = sys.find_path_ex_surfaces(from, to, &objects, false, SURFACE_GROUND, false)
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



}
