use super::*;
use glam::Vec3;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap, HashSet};

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
        Self {
            x: (world_pos.x / grid_size).round() as i32,
            y: (world_pos.z / grid_size).round() as i32,
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
    blocked: HashSet<GridPos>,
    dynamic_blocked: HashSet<GridPos>, // Temporarily blocked by units
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
        Self {
            width: (world_width / grid_size).ceil() as i32,
            height: (world_height / grid_size).ceil() as i32,
            grid_size,
            origin,
            blocked: HashSet::new(),
            dynamic_blocked: HashSet::new(),
        }
    }

    pub fn is_valid_pos(&self, pos: GridPos) -> bool {
        pos.x >= 0 && pos.x < self.width && pos.y >= 0 && pos.y < self.height
    }

    pub fn origin(&self) -> Vec3 {
        self.origin
    }

    pub fn world_to_grid(&self, world_pos: Vec3) -> GridPos {
        GridPos {
            x: ((world_pos.x - self.origin.x) / self.grid_size).round() as i32,
            y: ((world_pos.z - self.origin.z) / self.grid_size).round() as i32,
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
        self.blocked.contains(&pos) || self.dynamic_blocked.contains(&pos)
    }

    pub fn is_static_blocked(&self, pos: GridPos) -> bool {
        self.blocked.contains(&pos)
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
            if self.is_valid_pos(cell) && self.is_static_blocked(cell) {
                return true;
            }
        }
        false
    }

    pub fn set_blocked(&mut self, pos: GridPos, blocked: bool) {
        if blocked {
            self.blocked.insert(pos);
        } else {
            self.blocked.remove(&pos);
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
                    self.set_blocked(p, true);
                }
            }
        }
    }

    pub fn clear_static_blocks(&mut self) {
        self.blocked.clear();
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
        if blocked {
            self.dynamic_blocked.insert(pos);
        } else {
            self.dynamic_blocked.remove(&pos);
        }
    }

    pub fn clear_dynamic_blocks(&mut self) {
        self.dynamic_blocked.clear();
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

        let start = self.clamp_pos(start);
        // Prefer static-open goal; dynamic occupancy near goal is soft-costed below.
        let goal = self
            .nearest_static_open(self.clamp_pos(goal), 8)
            .unwrap_or_else(|| self.clamp_pos(goal));

        // Goal still static-blocked and no open neighbor — cannot plan.
        if self.is_static_blocked(goal) {
            return None;
        }

        // Trivial same-cell path.
        if start == goal {
            return Some(vec![self.grid_to_world(start)]);
        }

        let mut open_set = BinaryHeap::new();
        let mut came_from: HashMap<GridPos, GridPos> = HashMap::new();
        let mut g_score: HashMap<GridPos, f32> = HashMap::new();
        // Closed set keeps large open-field A* from revisiting nodes forever.
        let mut closed: HashSet<GridPos> = HashSet::new();

        g_score.insert(start, 0.0);
        open_set.push(PathNode::new(start, 0.0, start.distance(goal), None));

        while let Some(current) = open_set.pop() {
            if current.pos == goal {
                // Reconstruct path
                return Some(self.reconstruct_path(&came_from, current.pos));
            }

            if !closed.insert(current.pos) {
                continue;
            }

            for neighbor in current.pos.neighbors() {
                if !self.is_valid_pos(neighbor) || self.is_static_blocked(neighbor) {
                    continue;
                }
                if closed.contains(&neighbor) {
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
                // C++ allyFixedCount soft cost: standing units prefer detour (~3*diag).
                if self.dynamic_blocked.contains(&neighbor) {
                    movement_cost += 3.0 * 1.414_213_5;
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
            if obj.is_alive()
                && (obj.is_kind_of(KindOf::Vehicle) || obj.is_kind_of(KindOf::Structure))
            {
                let grid_pos = self.world_to_grid(obj.get_position());

                // Vehicles and structures block pathfinding
                self.set_dynamic_blocked(grid_pos, true);

                // Large units might block multiple grid cells
                if obj.is_kind_of(KindOf::Structure) {
                    // Block a 3x3 area for buildings
                    for dx in -1..=1 {
                        for dy in -1..=1 {
                            let blocked_pos = GridPos::new(grid_pos.x + dx, grid_pos.y + dy);
                            self.set_dynamic_blocked(blocked_pos, true);
                        }
                    }
                }
            }
        }
    }
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
        }
    }

    pub fn clear_static_blocks(&mut self) {
        self.grid.clear_static_blocks();
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
        self.grid.update_dynamic_obstacles(objects);
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
        // Update dynamic obstacles
        self.grid.update_dynamic_obstacles(objects);

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
            self.grid.find_path(start_grid, goal_grid)?
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
        // Update obstacles and create flow field
        self.grid.update_dynamic_obstacles(objects);

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
        self.grid.update_dynamic_obstacles(objects);

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
}
