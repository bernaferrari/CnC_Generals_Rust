// pathfind_complete.rs
// Complete Pathfinding System - Faithful C++ Port
// Reference: /GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AIPathfind.cpp

use super::object_footprint_positions;
use super::path_optimization::PathOptimizer;
pub use super::pathfind_astar::{
    AStarPathfinder, GridCoord, PathfindCellType, PathfindLayerEnum, COST_DIAGONAL,
    COST_ORTHOGONAL, PATHFIND_CELL_SIZE, PATHFIND_CELL_SIZE_F,
};
use crate::common::{
    Coord2D, Coord3D, ICoord2D, ObjectID, PathfindLayerEnum as CommonPathfindLayerEnum, INVALID_ID,
};
use crate::helpers::TheTerrainLogic;
use crate::object::registry::OBJECT_REGISTRY;

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Arc, Mutex};

/// Maximum pathfind queue length
/// Matches C++ PATHFIND_QUEUE_LEN at AIPathfind.h:418
pub const PATHFIND_QUEUE_LEN: usize = 512;

/// Maximum iterations for A* to prevent infinite loops
pub const MAX_PATH_ITERATIONS: usize = 10000;

/// Locomotor surface type mask matching C++ LocomotorSurfaceTypeMask
pub type LocomotorSurfaceTypeMask = u32;

pub const SURFACE_GROUND: u32 = 0x01;
pub const SURFACE_WATER: u32 = 0x02;
pub const SURFACE_CLIFF: u32 = 0x04;
pub const SURFACE_AIR: u32 = 0x08;
pub const SURFACE_RUBBLE: u32 = 0x10;

/// Pathfinding request
#[derive(Debug, Clone)]
pub struct PathRequest {
    pub object_id: ObjectID,
    pub from: Coord3D,
    pub to: Coord3D,
    pub surfaces: LocomotorSurfaceTypeMask,
    pub is_crusher: bool,
    pub unit_radius: f32,
    pub allow_partial: bool,
    pub move_allies: bool,
    pub ignore_obstacle_id: Option<ObjectID>,
}

/// Pathfinding result
#[derive(Debug, Clone)]
pub struct PathResult {
    pub success: bool,
    pub waypoints: Vec<Coord3D>,
    pub layers: Vec<PathfindLayerEnum>,
    pub total_cost: u32,
    pub blocked_by_ally: bool,
}

impl PathResult {
    pub fn none() -> Self {
        Self {
            success: false,
            waypoints: Vec::new(),
            layers: Vec::new(),
            total_cost: u32::MAX,
            blocked_by_ally: false,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct GoalCell {
    goal_unit_ground: ObjectID,
    goal_unit_top: ObjectID,
    goal_aircraft: ObjectID,
}

impl GoalCell {
    fn new() -> Self {
        Self {
            goal_unit_ground: INVALID_ID,
            goal_unit_top: INVALID_ID,
            goal_aircraft: INVALID_ID,
        }
    }

    fn get_goal_unit(&self, layer: PathfindLayerEnum) -> ObjectID {
        match layer {
            PathfindLayerEnum::Ground => self.goal_unit_ground,
            _ => self.goal_unit_top,
        }
    }

    fn set_goal_unit(&mut self, layer: PathfindLayerEnum, unit: ObjectID) {
        match layer {
            PathfindLayerEnum::Ground => self.goal_unit_ground = unit,
            _ => self.goal_unit_top = unit,
        }
    }

    fn clear_goal_unit(&mut self, layer: PathfindLayerEnum, unit: ObjectID) {
        match layer {
            PathfindLayerEnum::Ground => {
                if self.goal_unit_ground == unit {
                    self.goal_unit_ground = INVALID_ID;
                }
            }
            _ => {
                if self.goal_unit_top == unit {
                    self.goal_unit_top = INVALID_ID;
                }
            }
        }
    }

    fn set_goal_aircraft(&mut self, unit: ObjectID) {
        self.goal_aircraft = unit;
    }

    fn clear_goal_aircraft(&mut self, unit: ObjectID) {
        if self.goal_aircraft == unit {
            self.goal_aircraft = INVALID_ID;
        }
    }

    fn has_aircraft_goal(&self) -> bool {
        self.goal_aircraft != INVALID_ID
    }
}

/// Bridge/layer information for pathfinding
/// Matches C++ PathfindLayer at AIPathfind.h:363-412
#[derive(Debug, Clone)]
pub struct BridgeLayer {
    pub layer_id: u32,
    pub bounds: (GridCoord, GridCoord),
    pub destroyed: bool,
    pub zone: u16,
}

impl BridgeLayer {
    pub fn new(layer_id: u32, bounds: (GridCoord, GridCoord)) -> Self {
        Self {
            layer_id,
            bounds,
            destroyed: false,
            zone: 0,
        }
    }

    pub fn contains(&self, coord: GridCoord) -> bool {
        coord.x >= self.bounds.0.x
            && coord.x <= self.bounds.1.x
            && coord.y >= self.bounds.0.y
            && coord.y <= self.bounds.1.y
    }
}

fn ignored_obstacle_cells(ignore_obstacle_id: Option<ObjectID>) -> Option<HashSet<GridCoord>> {
    let object_id = ignore_obstacle_id?;
    if object_id == INVALID_ID {
        return None;
    }

    let obj = OBJECT_REGISTRY.get_object(object_id)?;
    let guard = obj.read().ok()?;
    let positions = object_footprint_positions(&guard)?;
    let mut cells = HashSet::with_capacity(positions.len());
    for pos in positions {
        cells.insert(GridCoord::from_world(&pos));
    }
    if cells.is_empty() {
        None
    } else {
        Some(cells)
    }
}

/// Complete pathfinding system
/// Matches C++ Pathfinder class at AIPathfind.h:568-846
pub struct PathfindingSystem {
    /// Core A* pathfinder
    pathfinder: Arc<Mutex<AStarPathfinder>>,

    /// Path optimizer
    optimizer: PathOptimizer,

    /// Bridge layers for elevated pathfinding
    /// Matches C++ m_layers at AIPathfind.h:832
    bridges: Vec<BridgeLayer>,

    /// Pathfind request queue
    /// Matches C++ m_queuedPathfindRequests at AIPathfind.h:842
    request_queue: Arc<Mutex<VecDeque<PathRequest>>>,
    /// Goal cell tracking (ground/top + aircraft goals).
    goal_cells: Arc<Mutex<Vec<Vec<GoalCell>>>>,

    /// Cached paths
    path_cache: Arc<
        Mutex<
            HashMap<
                (
                    GridCoord,
                    GridCoord,
                    LocomotorSurfaceTypeMask,
                    bool,
                    bool,
                    u32,
                    bool,
                    ObjectID,
                ),
                PathResult,
            >,
        >,
    >,

    /// Zone manager for hierarchical pathfinding
    zones: Arc<Mutex<ZoneManager>>,

    /// Map dimensions
    width: usize,
    height: usize,
}

impl std::fmt::Debug for PathfindingSystem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PathfindingSystem")
            .field("width", &self.width)
            .field("height", &self.height)
            .field("bridge_count", &self.bridges.len())
            .finish()
    }
}

impl PathfindingSystem {
    fn object_uses_aircraft_goal_reservations(object_id: ObjectID) -> bool {
        if object_id == INVALID_ID {
            return false;
        }
        let Some(obj_arc) = OBJECT_REGISTRY.get_object(object_id) else {
            return false;
        };
        let Ok(obj_guard) = obj_arc.read() else {
            return false;
        };
        let Some(ai) = obj_guard.get_ai_update_interface() else {
            return false;
        };
        let Ok(ai_guard) = ai.lock() else {
            return false;
        };
        ai_guard.is_aircraft_that_adjusts_destination()
    }

    fn destination_only_result(from: Coord3D, to: Coord3D, layer: PathfindLayerEnum) -> PathResult {
        let mut waypoints = Vec::with_capacity(2);
        let mut layers = Vec::with_capacity(2);
        if (from.x - to.x).abs() > f32::EPSILON
            || (from.y - to.y).abs() > f32::EPSILON
            || (from.z - to.z).abs() > f32::EPSILON
        {
            waypoints.push(from);
            layers.push(layer);
        }
        waypoints.push(to);
        layers.push(layer);
        PathResult {
            success: true,
            waypoints,
            layers,
            total_cost: 0,
            blocked_by_ally: false,
        }
    }

    pub fn new(width: usize, height: usize) -> Self {
        Self {
            pathfinder: Arc::new(Mutex::new(AStarPathfinder::new(width, height))),
            optimizer: PathOptimizer::new(),
            bridges: Vec::new(),
            request_queue: Arc::new(Mutex::new(VecDeque::new())),
            goal_cells: Arc::new(Mutex::new(vec![vec![GoalCell::new(); height]; width])),
            path_cache: Arc::new(Mutex::new(HashMap::new())),
            zones: Arc::new(Mutex::new(ZoneManager::new(width, height))),
            width,
            height,
        }
    }

    /// Reset pathfinding state for a new map.
    pub fn reset(&mut self) {
        if let Ok(mut queue) = self.request_queue.lock() {
            queue.clear();
        }
        if let Ok(mut cache) = self.path_cache.lock() {
            cache.clear();
        }
        if let Ok(mut goals) = self.goal_cells.lock() {
            for row in goals.iter_mut() {
                for cell in row.iter_mut() {
                    *cell = GoalCell::new();
                }
            }
        }
        if let Ok(mut zones) = self.zones.lock() {
            zones.reset();
        }
        if let Ok(mut pathfinder) = self.pathfinder.lock() {
            pathfinder.reset();
        }
        self.bridges.clear();
    }

    /// Queue a pathfinding request
    /// Matches C++ Pathfinder::queueForPath() at AIPathfind.cpp:5624-5663
    pub fn queue_path_request(&self, request: PathRequest) -> Result<(), String> {
        let mut queue = self.request_queue.lock().unwrap();

        // Check if already queued
        if queue.iter().any(|r| r.object_id == request.object_id) {
            return Ok(()); // Already queued
        }

        if queue.len() >= PATHFIND_QUEUE_LEN {
            return Err("Pathfind queue full".to_string());
        }

        queue.push_back(request);
        Ok(())
    }

    /// Process pathfinding queue (call each frame)
    /// Matches C++ Pathfinder::processPathfindQueue() at AIPathfind.cpp:5857-5938
    pub fn process_queue(&self, max_per_frame: usize) -> usize {
        let mut queue = self.request_queue.lock().unwrap();
        let mut processed = 0;

        while processed < max_per_frame && !queue.is_empty() {
            if let Some(request) = queue.pop_front() {
                // Process the request
                let _ = self.find_path_internal(request);
                processed += 1;
            }
        }

        processed
    }

    /// Find path synchronously (blocks until complete)
    /// Matches C++ Pathfinder::findPath() at AIPathfind.cpp:6364-6433
    pub fn find_path(&self, request: PathRequest) -> PathResult {
        // Check cache first
        let cache_key = (
            GridCoord::from_world(&request.from),
            GridCoord::from_world(&request.to),
            request.surfaces,
            request.is_crusher,
            request.allow_partial,
            request.unit_radius.to_bits(),
            request.move_allies,
            request.ignore_obstacle_id.unwrap_or(INVALID_ID),
        );

        if let Ok(cache) = self.path_cache.lock() {
            if let Some(cached) = cache.get(&cache_key) {
                return cached.clone();
            }
        }

        let result = self.find_path_internal(request);

        // Cache the result
        if let Ok(mut cache) = self.path_cache.lock() {
            cache.insert(cache_key, result.clone());

            // Limit cache size
            if cache.len() > 1000 {
                cache.clear();
            }
        }

        result
    }

    /// Internal path finding implementation
    /// Matches C++ Pathfinder::internalFindPath() at AIPathfind.cpp:6438-6694
    fn find_path_internal(&self, request: PathRequest) -> PathResult {
        let start = GridCoord::from_world(&request.from);
        let goal = GridCoord::from_world(&request.to);
        let ignore_cells = ignored_obstacle_cells(request.ignore_obstacle_id);

        // Validate coordinates
        if !self.is_valid_coord(start) || !self.is_valid_coord(goal) {
            return PathResult::none();
        }

        // Check zone connectivity for fast rejection
        // Matches C++ zone check at AIPathfind.cpp:6531-6559
        if let Ok(zones) = self.zones.lock() {
            if !zones.are_connected(start, goal, request.surfaces, request.is_crusher) {
                return PathResult::none();
            }
        }

        // Run A* pathfinding
        let pathfinder = self.pathfinder.lock().unwrap();
        let grid_path = pathfinder.find_path(
            start,
            goal,
            request.surfaces,
            request.is_crusher,
            MAX_PATH_ITERATIONS,
            request.allow_partial,
            ignore_cells.as_ref(),
        );

        drop(pathfinder); // Release lock

        let Some(grid_path) = grid_path else {
            return PathResult::none();
        };

        // Convert grid path to world coordinates
        // Matches C++ buildActualPath() at AIPathfind.cpp:8954-9071
        let mut waypoints = Vec::new();
        let mut layers = Vec::new();

        for (idx, coord) in grid_path.iter().enumerate() {
            let layer = self.get_layer_for_coord(*coord);
            let mut pos = if idx == 0 {
                request.from
            } else if idx + 1 == grid_path.len() {
                request.to
            } else {
                self.world_pos_for_coord(*coord, layer)
            };
            if let Some(terrain) = TheTerrainLogic::get() {
                let common_layer = match layer {
                    PathfindLayerEnum::Invalid => CommonPathfindLayerEnum::Invalid,
                    PathfindLayerEnum::Ground => CommonPathfindLayerEnum::Ground,
                    PathfindLayerEnum::Top => CommonPathfindLayerEnum::Top,
                };
                pos.z = terrain.get_layer_height(pos.x, pos.y, common_layer);
            }
            waypoints.push(pos);
            layers.push(layer);
        }

        // Optimize the path
        // Matches C++ Path::optimize() call at AIPathfind.cpp:6619
        let optimized = self.optimize_path(&waypoints, &layers, &request);

        PathResult {
            success: true,
            waypoints: optimized.0,
            layers: optimized.1,
            total_cost: self.calculate_path_cost(&grid_path),
            blocked_by_ally: false,
        }
    }

    /// Find closest reachable path (for blocked destinations)
    /// Matches C++ Pathfinder::findClosestPath() at AIPathfind.cpp:8739-8926
    pub fn find_closest_path(&self, mut request: PathRequest) -> PathResult {
        let goal_grid = GridCoord::from_world(&request.to);
        let (unit_radius_cells, center_in_cell) =
            Self::compute_radius_and_center(request.unit_radius);
        let aircraft_goal_only = Self::object_uses_aircraft_goal_reservations(request.object_id);

        if aircraft_goal_only {
            let goal_layer = self.get_layer_for_coord(goal_grid);
            if self.check_destination(
                &request,
                goal_grid,
                goal_layer,
                unit_radius_cells,
                center_in_cell,
            ) {
                let adjusted = self.world_pos_for_coord(goal_grid, goal_layer);
                return Self::destination_only_result(request.from, adjusted, goal_layer);
            }
        } else {
            // First try exact path.
            let exact_result = self.find_path(request.clone());
            if exact_result.success {
                return exact_result;
            }
        }

        // Try to find closest reachable point.
        // Matches C++ adjustDestination() logic at AIPathfind.cpp:5331-5407
        let max_search_radius = 20; // Grid cells

        for radius in 1..=max_search_radius {
            // Try cells in expanding square
            for dx in -radius..=radius {
                for dy in -radius..=radius {
                    if (dx as i32).abs() < radius && (dy as i32).abs() < radius {
                        continue; // Only check perimeter
                    }

                    let test_coord = GridCoord::new(goal_grid.x + dx, goal_grid.y + dy);
                    if !self.is_valid_coord(test_coord) {
                        continue;
                    }

                    let layer = self.get_layer_for_coord(test_coord);
                    if !self.check_destination(
                        &request,
                        test_coord,
                        layer,
                        unit_radius_cells,
                        center_in_cell,
                    ) {
                        continue;
                    }
                    let adjusted = self.world_pos_for_coord(test_coord, layer);
                    if aircraft_goal_only {
                        return Self::destination_only_result(request.from, adjusted, layer);
                    }
                    request.to = adjusted;
                    let result = self.find_path(request.clone());
                    if result.success {
                        return result;
                    }
                }
            }
        }

        PathResult::none()
    }

    /// Optimize path using line-of-sight checks
    fn optimize_path(
        &self,
        waypoints: &[Coord3D],
        layers: &[PathfindLayerEnum],
        request: &PathRequest,
    ) -> (Vec<Coord3D>, Vec<PathfindLayerEnum>) {
        let ignore_cells = ignored_obstacle_cells(request.ignore_obstacle_id);

        // Line passability checker
        let passability = |from: &Coord3D, to: &Coord3D, layer: PathfindLayerEnum| {
            self.is_line_passable(from, to, request.surfaces, layer, ignore_cells.as_ref())
        };

        let ground_passability = |from: &Coord3D, to: &Coord3D, diameter: i32| {
            self.is_ground_line_passable(
                from,
                to,
                request.is_crusher,
                diameter,
                ignore_cells.as_ref(),
            )
        };

        // Basic optimization
        let (opt1, layers1) = self.optimizer.optimize(waypoints, layers, passability);

        // Ground-specific optimization
        let diameter = (request.unit_radius * 2.0) as i32;
        let (opt2, layers2) = self.optimizer.optimize_ground_path(
            &opt1,
            &layers1,
            request.is_crusher,
            diameter,
            ground_passability,
        );

        (opt2, layers2)
    }

    fn world_pos_for_coord(&self, coord: GridCoord, layer: PathfindLayerEnum) -> Coord3D {
        let mut pos = coord.to_world(layer);
        if let Some(terrain) = TheTerrainLogic::get() {
            let common_layer = match layer {
                PathfindLayerEnum::Invalid => CommonPathfindLayerEnum::Invalid,
                PathfindLayerEnum::Ground => CommonPathfindLayerEnum::Ground,
                PathfindLayerEnum::Top => CommonPathfindLayerEnum::Top,
            };
            pos.z = terrain.get_layer_height(pos.x, pos.y, common_layer);
        }
        pos
    }

    /// Check if line between points is passable
    /// Matches C++ Pathfinder::isLinePassable() at AIPathfind.cpp:3989-4090
    fn is_line_passable(
        &self,
        from: &Coord3D,
        to: &Coord3D,
        surfaces: LocomotorSurfaceTypeMask,
        _layer: PathfindLayerEnum,
        ignore_cells: Option<&HashSet<GridCoord>>,
    ) -> bool {
        let pathfinder = self.pathfinder.lock().unwrap();

        // Sample along the line
        let dx = to.x - from.x;
        let dy = to.y - from.y;
        let distance = (dx * dx + dy * dy).sqrt();

        if distance < 0.1 {
            return true;
        }

        let steps = (distance / (PATHFIND_CELL_SIZE_F * 0.5)).ceil() as i32;
        let steps = steps.max(1);

        for i in 0..=steps {
            let t = i as f32 / steps as f32;
            let sample = Coord3D::new(from.x + dx * t, from.y + dy * t, 0.0);

            let coord = GridCoord::from_world(&sample);
            if !pathfinder.is_passable_with_ignore(coord, surfaces, false, ignore_cells) {
                return false;
            }
        }

        true
    }

    /// Check if ground path is passable
    /// Matches C++ Pathfinder::isGroundPathPassable() at AIPathfind.cpp:4065-4090
    fn is_ground_line_passable(
        &self,
        from: &Coord3D,
        to: &Coord3D,
        is_crusher: bool,
        diameter: i32,
        ignore_cells: Option<&HashSet<GridCoord>>,
    ) -> bool {
        let pathfinder = self.pathfinder.lock().unwrap();
        let radius = (diameter / 2).max(1);

        let dx = to.x - from.x;
        let dy = to.y - from.y;
        let distance = (dx * dx + dy * dy).sqrt();

        if distance < 0.1 {
            return true;
        }

        let steps = (distance / PATHFIND_CELL_SIZE_F).ceil() as i32;

        for i in 0..=steps {
            let t = i as f32 / steps as f32;
            let center = Coord3D::new(from.x + dx * t, from.y + dy * t, 0.0);
            let center_grid = GridCoord::from_world(&center);

            // Check all cells in radius
            for rx in -radius..=radius {
                for ry in -radius..=radius {
                    let coord = GridCoord::new(center_grid.x + rx, center_grid.y + ry);
                    if !pathfinder.is_passable_with_ignore(
                        coord,
                        SURFACE_GROUND,
                        is_crusher,
                        ignore_cells,
                    ) {
                        return false;
                    }
                }
            }
        }

        true
    }

    /// Get layer for a grid coordinate (checks bridges)
    fn get_layer_for_coord(&self, coord: GridCoord) -> PathfindLayerEnum {
        // Check if coordinate is on a bridge
        for bridge in &self.bridges {
            if !bridge.destroyed && bridge.contains(coord) {
                return PathfindLayerEnum::Top; // Or specific layer ID
            }
        }

        PathfindLayerEnum::Ground
    }

    /// Calculate total path cost
    fn calculate_path_cost(&self, path: &[GridCoord]) -> u32 {
        let mut cost = 0;

        for i in 0..path.len() - 1 {
            let dist = if path[i].is_diagonal(&path[i + 1]) {
                COST_DIAGONAL
            } else {
                COST_ORTHOGONAL
            };
            cost += dist;
        }

        cost
    }

    /// Check if coordinate is valid
    fn is_valid_coord(&self, coord: GridCoord) -> bool {
        coord.x >= 0 && coord.x < self.width as i32 && coord.y >= 0 && coord.y < self.height as i32
    }

    fn compute_radius_and_center(unit_radius: f32) -> (i32, bool) {
        let mut diameter = 2.0 * unit_radius;
        if diameter > PATHFIND_CELL_SIZE_F && diameter < 2.0 * PATHFIND_CELL_SIZE_F {
            diameter = 2.0 * PATHFIND_CELL_SIZE_F;
        }

        let mut radius = (diameter / PATHFIND_CELL_SIZE_F + 0.3).floor() as i32;
        let mut center_in_cell = false;
        if radius == 0 {
            radius = 1;
        }
        if (radius & 1) != 0 {
            center_in_cell = true;
        }
        radius /= 2;
        if radius > 2 {
            radius = 2;
            center_in_cell = true;
        }

        (radius, center_in_cell)
    }

    fn check_destination(
        &self,
        request: &PathRequest,
        cell: GridCoord,
        layer: PathfindLayerEnum,
        radius: i32,
        center_in_cell: bool,
    ) -> bool {
        let ignore_cells = ignored_obstacle_cells(request.ignore_obstacle_id);
        let pathfinder = self.pathfinder.lock().unwrap();
        let center_cell = ICoord2D::new(cell.x, cell.y);
        let check_for_aircraft = Self::object_uses_aircraft_goal_reservations(request.object_id);

        let obj = if request.object_id != INVALID_ID {
            OBJECT_REGISTRY.get_object(request.object_id)
        } else {
            None
        };

        let mut ok = true;
        self.for_goal_cells(center_cell, radius, center_in_cell, |coord| {
            if !ok {
                return;
            }
            if !self.is_valid_coord(coord) {
                ok = false;
                return;
            }

            if check_for_aircraft {
                let goal_aircraft = self.get_goal_aircraft(coord);
                if goal_aircraft == INVALID_ID || goal_aircraft == request.object_id {
                    return;
                }
                ok = false;
                return;
            }

            if !pathfinder.is_passable_with_ignore(
                coord,
                request.surfaces,
                request.is_crusher,
                ignore_cells.as_ref(),
            ) {
                ok = false;
                return;
            }

            let goal_unit = self.get_goal_unit(coord, layer);
            if goal_unit == INVALID_ID
                || goal_unit == request.object_id
                || request.ignore_obstacle_id == Some(goal_unit)
            {
                return;
            }

            let Some(obj_arc) = obj.as_ref() else {
                ok = false;
                return;
            };
            let Some(goal_arc) = OBJECT_REGISTRY.get_object(goal_unit) else {
                return;
            };
            let Ok(obj_guard) = obj_arc.read() else {
                return;
            };
            let Ok(goal_guard) = goal_arc.read() else {
                return;
            };
            let relationship = obj_guard.relationship_to(&goal_guard);
            if !request.move_allies && matches!(relationship, crate::common::Relationship::Allies) {
                ok = false;
                return;
            }
        });

        ok
    }

    fn for_goal_cells<F>(&self, center_cell: ICoord2D, radius: i32, center_in_cell: bool, mut f: F)
    where
        F: FnMut(GridCoord),
    {
        let mut num_cells_above = radius;
        if center_in_cell {
            num_cells_above += 1;
        }

        let start_x = center_cell.x - radius;
        let end_x = center_cell.x + num_cells_above;
        let start_y = center_cell.y - radius;
        let end_y = center_cell.y + num_cells_above;

        for x in start_x..end_x {
            for y in start_y..end_y {
                f(GridCoord::new(x, y));
            }
        }
    }

    /// Add a bridge layer
    /// Matches C++ Pathfinder::addBridge() at AIPathfind.h:698
    pub fn add_bridge(&mut self, bounds: (GridCoord, GridCoord)) -> u32 {
        let layer_id = self.bridges.len() as u32 + 2; // Start from 2 (Ground=1)
        self.bridges.push(BridgeLayer::new(layer_id, bounds));
        layer_id
    }

    /// Set bridge destroyed state
    pub fn set_bridge_destroyed(&mut self, layer_id: u32, destroyed: bool) {
        if let Some(bridge) = self.bridges.iter_mut().find(|b| b.layer_id == layer_id) {
            bridge.destroyed = destroyed;
        }
    }

    /// Find a bridge layer by its assigned pathfinder layer id.
    pub fn bridge_by_layer_id(&self, layer_id: u32) -> Option<&BridgeLayer> {
        self.bridges
            .iter()
            .find(|bridge| bridge.layer_id == layer_id)
    }

    /// Clear path cache
    pub fn clear_cache(&self) {
        if let Ok(mut cache) = self.path_cache.lock() {
            cache.clear();
        }
    }

    /// Set cell type at world position
    pub fn set_cell_type(&self, pos: &Coord3D, cell_type: PathfindCellType) {
        let coord = GridCoord::from_world(pos);
        if let Ok(mut pathfinder) = self.pathfinder.lock() {
            pathfinder.set_cell_type(coord, cell_type);
        }
    }

    /// Get cell type at world position.
    pub fn get_cell_type(&self, pos: &Coord3D) -> Option<PathfindCellType> {
        let coord = GridCoord::from_world(pos);
        let pathfinder = self.pathfinder.lock().ok()?;
        pathfinder.get_cell_type(coord)
    }

    /// Quick validity check for a locomotor position (C++ validMovementPosition usage).
    pub fn valid_movement_position(
        &self,
        surfaces: LocomotorSurfaceTypeMask,
        is_crusher: bool,
        pos: &Coord3D,
        ignore_obstacle_id: Option<ObjectID>,
    ) -> bool {
        let coord = GridCoord::from_world(pos);
        if !self.is_valid_coord(coord) {
            return false;
        }
        let ignore_cells = ignored_obstacle_cells(ignore_obstacle_id);
        let pathfinder = self.pathfinder.lock().unwrap();
        pathfinder.is_passable_with_ignore(coord, surfaces, is_crusher, ignore_cells.as_ref())
    }

    pub(crate) fn set_goal_cells(
        &self,
        unit_id: ObjectID,
        center_cell: ICoord2D,
        radius: i32,
        center_in_cell: bool,
        layer: PathfindLayerEnum,
        do_ground: bool,
        do_layer: bool,
    ) {
        let Ok(mut goals) = self.goal_cells.lock() else {
            return;
        };
        self.for_goal_cells(center_cell, radius, center_in_cell, |coord| {
            if !self.is_valid_coord(coord) {
                return;
            }
            if let Some(cell) = goals
                .get_mut(coord.x as usize)
                .and_then(|row| row.get_mut(coord.y as usize))
            {
                if do_ground {
                    cell.set_goal_unit(PathfindLayerEnum::Ground, unit_id);
                }
                if do_layer {
                    cell.set_goal_unit(layer, unit_id);
                }
            }
        });
    }

    pub(crate) fn clear_goal_cells(
        &self,
        unit_id: ObjectID,
        center_cell: ICoord2D,
        radius: i32,
        center_in_cell: bool,
        layer: PathfindLayerEnum,
        clear_ground: bool,
        clear_layer: bool,
    ) {
        let Ok(mut goals) = self.goal_cells.lock() else {
            return;
        };
        self.for_goal_cells(center_cell, radius, center_in_cell, |coord| {
            if !self.is_valid_coord(coord) {
                return;
            }
            if let Some(cell) = goals
                .get_mut(coord.x as usize)
                .and_then(|row| row.get_mut(coord.y as usize))
            {
                if clear_ground {
                    cell.clear_goal_unit(PathfindLayerEnum::Ground, unit_id);
                }
                if clear_layer {
                    cell.clear_goal_unit(layer, unit_id);
                }
            }
        });
    }

    pub(crate) fn set_aircraft_goal_cells(
        &self,
        unit_id: ObjectID,
        center_cell: ICoord2D,
        radius: i32,
        center_in_cell: bool,
    ) {
        let Ok(mut goals) = self.goal_cells.lock() else {
            return;
        };
        self.for_goal_cells(center_cell, radius, center_in_cell, |coord| {
            if !self.is_valid_coord(coord) {
                return;
            }
            if let Some(cell) = goals
                .get_mut(coord.x as usize)
                .and_then(|row| row.get_mut(coord.y as usize))
            {
                cell.set_goal_aircraft(unit_id);
            }
        });
    }

    pub(crate) fn clear_aircraft_goal_cells(
        &self,
        unit_id: ObjectID,
        center_cell: ICoord2D,
        radius: i32,
        center_in_cell: bool,
    ) {
        let Ok(mut goals) = self.goal_cells.lock() else {
            return;
        };
        self.for_goal_cells(center_cell, radius, center_in_cell, |coord| {
            if !self.is_valid_coord(coord) {
                return;
            }
            if let Some(cell) = goals
                .get_mut(coord.x as usize)
                .and_then(|row| row.get_mut(coord.y as usize))
            {
                cell.clear_goal_aircraft(unit_id);
            }
        });
    }

    pub(crate) fn get_goal_unit(&self, coord: GridCoord, layer: PathfindLayerEnum) -> ObjectID {
        let Ok(goals) = self.goal_cells.lock() else {
            return INVALID_ID;
        };
        goals
            .get(coord.x as usize)
            .and_then(|row| row.get(coord.y as usize))
            .map(|cell| cell.get_goal_unit(layer))
            .unwrap_or(INVALID_ID)
    }

    pub(crate) fn get_goal_aircraft(&self, coord: GridCoord) -> ObjectID {
        let Ok(goals) = self.goal_cells.lock() else {
            return INVALID_ID;
        };
        goals
            .get(coord.x as usize)
            .and_then(|row| row.get(coord.y as usize))
            .map(|cell| cell.goal_aircraft)
            .unwrap_or(INVALID_ID)
    }

    pub(crate) fn has_aircraft_goal(&self, coord: GridCoord) -> bool {
        let Ok(goals) = self.goal_cells.lock() else {
            return false;
        };
        goals
            .get(coord.x as usize)
            .and_then(|row| row.get(coord.y as usize))
            .map(|cell| cell.has_aircraft_goal())
            .unwrap_or(false)
    }

    pub fn refresh_pinched_for_positions(&self, positions: &[Coord3D]) {
        if positions.is_empty() {
            return;
        }

        let mut lo = GridCoord::from_world(&positions[0]);
        let mut hi = lo;
        for pos in positions.iter().skip(1) {
            let coord = GridCoord::from_world(pos);
            lo.x = lo.x.min(coord.x);
            lo.y = lo.y.min(coord.y);
            hi.x = hi.x.max(coord.x);
            hi.y = hi.y.max(coord.y);
        }

        let margin = 2;
        lo.x = (lo.x - margin).max(0);
        lo.y = (lo.y - margin).max(0);
        hi.x = (hi.x + margin).min(self.width.saturating_sub(1) as i32);
        hi.y = (hi.y + margin).min(self.height.saturating_sub(1) as i32);

        if let Ok(mut pathfinder) = self.pathfinder.lock() {
            pathfinder.refresh_pinched_cells_in_bounds(lo, hi);
        }
    }

    /// Line-clear check against impassable cells (matches C++ path validation usage).
    pub fn is_line_clear_between(&self, from: &Coord3D, to: &Coord3D) -> bool {
        let dx = to.x - from.x;
        let dy = to.y - from.y;
        let dz = to.z - from.z;
        let distance = (dx * dx + dy * dy + dz * dz).sqrt();
        if distance <= f32::EPSILON {
            return true;
        }

        let step = (PATHFIND_CELL_SIZE_F * 0.5).max(0.1);
        let steps = (distance / step).ceil().max(1.0) as i32;

        let Ok(pathfinder) = self.pathfinder.lock() else {
            return true;
        };

        for i in 0..=steps {
            let t = i as f32 / steps as f32;
            let sample = Coord3D::new(from.x + dx * t, from.y + dy * t, from.z + dz * t);
            let cell = GridCoord::from_world(&sample);
            if pathfinder.is_impassable_cell(cell) {
                return false;
            }
        }

        true
    }

    /// Line passability check using surface mask and optional ignored obstacle.
    pub fn is_line_passable_for_surfaces(
        &self,
        from: &Coord3D,
        to: &Coord3D,
        surfaces: LocomotorSurfaceTypeMask,
        ignore_obstacle_id: Option<ObjectID>,
    ) -> bool {
        let ignore_cells = ignored_obstacle_cells(ignore_obstacle_id);
        self.is_line_passable(
            from,
            to,
            surfaces,
            PathfindLayerEnum::Ground,
            ignore_cells.as_ref(),
        )
    }
}

/// Zone manager for hierarchical pathfinding
/// Matches C++ PathfindZoneManager at AIPathfind.h:475-531
struct ZoneManager {
    zones: Vec<Vec<u16>>,
    width: usize,
    height: usize,
    next_zone: u16,
}

impl ZoneManager {
    fn new(width: usize, height: usize) -> Self {
        Self {
            zones: vec![vec![0; height]; width],
            width,
            height,
            next_zone: 1,
        }
    }

    fn reset(&mut self) {
        for column in self.zones.iter_mut() {
            for zone in column.iter_mut() {
                *zone = 0;
            }
        }
        self.next_zone = 1;
    }

    fn are_connected(
        &self,
        start: GridCoord,
        goal: GridCoord,
        _surfaces: LocomotorSurfaceTypeMask,
        _is_crusher: bool,
    ) -> bool {
        // Simple zone check
        if start.x < 0
            || start.x >= self.width as i32
            || start.y < 0
            || start.y >= self.height as i32
        {
            return false;
        }

        if goal.x < 0 || goal.x >= self.width as i32 || goal.y < 0 || goal.y >= self.height as i32 {
            return false;
        }

        let start_zone = self.zones[start.x as usize][start.y as usize];
        let goal_zone = self.zones[goal.x as usize][goal.y as usize];

        // If either is unzoned (0), allow pathfinding attempt
        if start_zone == 0 || goal_zone == 0 {
            return true;
        }

        start_zone == goal_zone
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pathfinding_system_creation() {
        let system = PathfindingSystem::new(128, 128);
        assert_eq!(system.width, 128);
        assert_eq!(system.height, 128);
    }

    #[test]
    fn test_queue_path_request() {
        let system = PathfindingSystem::new(128, 128);

        let request = PathRequest {
            object_id: 1,
            from: Coord3D::new(0.0, 0.0, 0.0),
            to: Coord3D::new(50.0, 50.0, 0.0),
            surfaces: SURFACE_GROUND,
            is_crusher: false,
            unit_radius: 5.0,
            allow_partial: false,
            move_allies: false,
            ignore_obstacle_id: None,
        };

        assert!(system.queue_path_request(request).is_ok());
    }

    #[test]
    fn test_simple_pathfinding() {
        let system = PathfindingSystem::new(64, 64);

        let request = PathRequest {
            object_id: 1,
            from: Coord3D::new(50.0, 50.0, 0.0),
            to: Coord3D::new(150.0, 150.0, 0.0),
            surfaces: SURFACE_GROUND,
            is_crusher: false,
            unit_radius: 5.0,
            allow_partial: false,
            move_allies: false,
            ignore_obstacle_id: None,
        };

        let result = system.find_path(request);
        assert!(result.success);
        assert!(!result.waypoints.is_empty());
    }

    #[test]
    fn test_bridge_layers() {
        let mut system = PathfindingSystem::new(64, 64);

        let bridge_id = system.add_bridge((GridCoord::new(10, 10), GridCoord::new(20, 20)));

        assert_eq!(bridge_id, 2); // First bridge gets ID 2
        assert_eq!(system.bridges.len(), 1);
    }
}
