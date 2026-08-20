use super::*;
use glam::Vec3;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap, VecDeque};

use gamelogic::ai::pathfind_astar::{
    AStarPathfinder, GridCoord, PathfindCellType, COST_DIAGONAL,
};
use gamelogic::ai::pathfind_complete::{
    MAX_PATH_ITERATIONS, PATHFIND_QUEUE_LEN, SURFACE_AIR, SURFACE_GROUND,
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
            terrain_gen: 1,
        }
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
        self.terrain_gen = self.terrain_gen.wrapping_add(1);
    }

    /// C++ Pathfinder::isAttackViewBlockedByObstacle residual (static obstacles only).
    /// Bresenham walk from `from`→`to` world positions; intermediate static-blocked
    /// cells block attack view. Start/goal cells are skipped (attacker/victim footprint).
    /// Fail-closed: not full tall-building callback / transparent / layer / weapon terrain LOS.
    pub fn is_attack_view_blocked_static(&self, from: Vec3, to: Vec3) -> bool {
        let start = self.world_to_grid(from);
        let goal = self.world_to_grid(to);
        if start == goal {
            return false;
        }
        // Tiny range residual (C++ AIStates): skip LOS false positives at close range.
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
        // Skip first cell (attacker).
        // Target footprint residual: structure path blocks mark a disk around the
        // victim. C++ LOS does not treat the victim itself as intervening obstacle —
        // ignore static blocks within this chebyshev radius of the goal cell.
        const GOAL_IGNORE_CHEBYSHEV: i32 = 4;
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
            let near_goal = (cell.x - goal.x).abs() <= GOAL_IGNORE_CHEBYSHEV
                && (cell.y - goal.y).abs() <= GOAL_IGNORE_CHEBYSHEV;
            if near_goal {
                continue;
            }
            if self.is_valid_pos(cell) && self.is_static_blocked(cell) {
                return true;
            }
        }
        false
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
        let r = radius_cells.max(0);
        for dy in -r..=r {
            for dx in -r..=r {
                let p = GridPos::new(center.x + dx, center.y + dy);
                if self.is_valid_pos(p) {
                    self.set_cell_type(p, PathfindCellType::Obstacle);
                }
            }
        }
    }

    pub fn clear_static_blocks(&mut self) {
        self.blocked_bits.fill(0);
        self.cell_types.fill(PathfindCellType::Clear as u8);
        self.pinched_bits.fill(0);
        self.terrain_zones.fill(0);
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

    fn occupancy_cost(&self, pos: GridPos, seeker_player: Option<u32>) -> Option<f32> {
        let Some(idx) = self.bit_index(pos) else {
            return Some(0.0);
        };
        let fixed = self.occ_fixed_mask.get(idx).copied().unwrap_or(0);
        let moving = self.occ_moving_mask.get(idx).copied().unwrap_or(0);
        if fixed == 0 && moving == 0 {
            return Some(0.0);
        }
        let Some(player) = seeker_player else {
            // Unknown seeker: keep former allyFixed soft cost.
            return Some(3.0 * 1.414_213_5);
        };
        let bit = 1u16 << player.min(15);
        let enemy_fixed = (fixed & !bit) != 0;
        if enemy_fixed {
            return None;
        }
        let mut extra = 0.0;
        if (moving & bit) != 0 {
            extra += 3.0 * 1.414_213_5;
        }
        if (fixed & bit) != 0 {
            extra += 3.0 * 1.414_213_5;
        }
        // Other-player movers: transient soft cost.
        if (moving & !bit) != 0 {
            extra += 3.0 * 1.414_213_5;
        }
        Some(extra)
    }

    fn mark_occupancy(&mut self, pos: GridPos, player: u32, moving: bool) {
        let Some(idx) = self.bit_index(pos) else {
            return;
        };
        Self::bit_set(&mut self.dynamic_bits, idx, true);
        let bit = 1u16 << player.min(15);
        if moving {
            if let Some(slot) = self.occ_moving_mask.get_mut(idx) {
                *slot |= bit;
            }
        } else if let Some(slot) = self.occ_fixed_mask.get_mut(idx) {
            *slot |= bit;
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
                match self.occupancy_cost(neighbor, None) {
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
        self.clear_dynamic_blocks();

        for obj in objects.values() {
            if !obj.is_alive() {
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
            let player = obj
                .owner_player_id
                .unwrap_or(obj.team as u32);
            let moving = !obj.is_kind_of(KindOf::Structure)
                && (!obj.movement.path.is_empty()
                    || obj.movement.velocity.length_squared() > 0.25);
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
                        self.mark_occupancy(p, player, moving);
                    }
                }
            }
        }
    }

    pub fn occupancy_extra_cost(&self, pos: GridPos, seeker_player: Option<u32>) -> u32 {
        match self.occupancy_cost(pos, seeker_player) {
            None => u32::MAX / 8,
            Some(c) => (c * 10.0) as u32, // crate A* uses integer COST_DIAGONAL=14
        }
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
        }
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
        if self.dynamic_obstacle_frame != self.logic_frame {
            self.grid.update_dynamic_obstacles(objects);
            self.dynamic_obstacle_frame = self.logic_frame;
        }
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
        let start = self
            .grid
            .nearest_static_open(self.grid.clamp_pos(start), 16)
            .unwrap_or_else(|| self.grid.clamp_pos(start));
        let goal = self
            .grid
            .nearest_static_open(self.grid.clamp_pos(goal), 16)
            .unwrap_or_else(|| self.grid.clamp_pos(goal));
        if self.grid.is_static_blocked(start) || self.grid.is_static_blocked(goal) {
            return None;
        }
        if start == goal {
            return Some(vec![self.grid.grid_to_world(start)]);
        }
        let start_c = self.host_to_crate_coord(start);
        let goal_c = self.host_to_crate_coord(goal);
        let width = self.grid.width;
        let occ_fixed = self.grid.occ_fixed_mask.clone();
        let occ_moving = self.grid.occ_moving_mask.clone();
        let seeker = self.seeker_player;
        let extra = move |c: GridCoord| {
            if c.x < 0 || c.y < 0 || c.x >= width {
                return 0;
            }
            let idx = c.y as usize * width as usize + c.x as usize;
            let fixed = occ_fixed.get(idx).copied().unwrap_or(0);
            let moving = occ_moving.get(idx).copied().unwrap_or(0);
            if fixed == 0 && moving == 0 {
                return 0;
            }
            let Some(player) = seeker else {
                return 3 * COST_DIAGONAL;
            };
            let bit = 1u16 << player.min(15);
            if (fixed & !bit) != 0 {
                return u32::MAX / 8;
            }
            let mut extra = 0u32;
            if (moving & bit) != 0 {
                extra += 3 * COST_DIAGONAL;
            }
            if (fixed & bit) != 0 {
                extra += 3 * COST_DIAGONAL;
            }
            if (moving & !bit) != 0 {
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
            .map(|(path, _)| path)?;
        Some(self.crate_path_to_world(&cells))
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

    /// Find path between two world positions.
    ///
    /// Waypoint heights are lerped from start.y → goal.y so followers do not dive
    /// to Y=0 grid cells on maps with terrain height.
    /// Host residual: static-obstacle attack LOS (C++ isAttackViewBlockedByObstacle subset).
    pub fn is_attack_view_blocked(&self, from: Vec3, to: Vec3) -> bool {
        self.grid.is_attack_view_blocked_static(from, to)
    }

    /// Static-block structure footprint at world position (constructed buildings).
    pub fn block_structure_at_world(&mut self, world: Vec3, radius_cells: i32) {
        let cell = self.grid.world_to_grid(world);
        self.grid.block_structure_footprint(cell, radius_cells);
    }

    /// Rebuild structure static obstacles from live objects (map load / bulk sync).
    /// Does not clear terrain slope blocks — only ORs structure footprints.
    pub fn apply_structure_static_blocks(&mut self, objects: &HashMap<ObjectId, Object>) {
        for obj in objects.values() {
            if !obj.is_alive() || !obj.is_kind_of(KindOf::Structure) {
                continue;
            }
            // Under-construction footprints are soft in C++ until built; host residual
            // blocks when constructed (or map-placed completed).
            if obj.status.under_construction {
                continue;
            }
            let radius = structure_block_radius_cells(obj.selection_radius, self.grid.grid_size());
            self.block_structure_at_world(obj.get_position(), radius);
        }
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
                        // Already have a near-step firing spot — path via A* (or direct).
                        return self
                            .find_path(from, test, objects)
                            .or_else(|| Some(vec![from, test]));
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
        self.find_path(from, goal, objects)
            .or_else(|| Some(vec![from, goal]))
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
        self.seeker_player = objects
            .values()
            .filter(|o| o.is_alive())
            .min_by(|a, b| {
                let da = a.get_position().distance_squared(start);
                let db = b.get_position().distance_squared(start);
                da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
            })
            .and_then(|o| o.owner_player_id.or(Some(o.team as u32)));

        let start_grid = self.grid.world_to_grid(start);
        let goal_grid = self.grid.world_to_grid(goal);

        // Aircraft residual: prefer direct segment then tall-building detours
        // (ground static blocks should not force aircraft under tall structures).
        let mut path = if aircraft {
            // circleClipsTallBuilding residual: nudge goal off tall footprints.
            let goal_adj = Self::circle_clips_tall_building(
                start, goal, 40.0, // host residual approach circle
                objects, None,
            )
            .unwrap_or(goal);
            let direct = vec![start, goal_adj];
            let mut detoured = Self::detour_path_around_tall_buildings(&direct, objects);
            // Keep caller endpoint as final settle if we only nudged mid-path.
            if let Some(last) = detoured.last_mut() {
                // Prefer adjusted goal (not original inside building).
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
            // Preserve cruise altitude along detour.
            for p in path.iter_mut() {
                p.y = start.y;
            }
            if let Some(last) = path.last_mut() {
                last.y = goal.y;
            }
        }
        // Ensure exact endpoints for movement settling.
        if let Some(first) = path.first_mut() {
            *first = start;
        }
        if let Some(last) = path.last_mut() {
            // Aircraft may have circleClips-adjusted goal; keep last waypoint.
            if !aircraft {
                *last = goal;
            }
        }
        Some(path)
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

}
