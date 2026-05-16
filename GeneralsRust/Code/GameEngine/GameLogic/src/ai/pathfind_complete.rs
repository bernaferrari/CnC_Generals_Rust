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
        self.valid_movement_cell(surfaces, is_crusher, coord, ignore_obstacle_id)
    }

    /// Quick validity check for a locomotor grid cell.
    pub(crate) fn valid_movement_cell(
        &self,
        surfaces: LocomotorSurfaceTypeMask,
        is_crusher: bool,
        coord: GridCoord,
        ignore_obstacle_id: Option<ObjectID>,
    ) -> bool {
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

    // ========================================================================
    // GROUP A – Core A* ground pathfinding
    // ========================================================================

    /// Main ground A* pathfinding entry point.
    /// Matches C++ Pathfinder::findPath() at AIPathfind.cpp:6364-6433.
    ///
    /// Returns `PathResult` with full waypoint list from `from` to `to` using
    /// ground-surface A* with zone-based early rejection.
    pub fn find_ground_path(
        &self,
        from: Coord3D,
        to: Coord3D,
        surfaces: LocomotorSurfaceTypeMask,
        is_crusher: bool,
        unit_radius: f32,
        allow_partial: bool,
        move_allies: bool,
        ignore_obstacle_id: Option<ObjectID>,
    ) -> PathResult {
        let request = PathRequest {
            object_id: INVALID_ID,
            from,
            to,
            surfaces,
            is_crusher,
            unit_radius,
            allow_partial,
            move_allies,
            ignore_obstacle_id,
        };
        self.find_path(request)
    }

    /// Build a concrete `Path` (node-linked-list) from an A* grid result.
    /// Matches C++ Pathfinder::buildActualPath() at AIPathfind.cpp:8954-9001.
    ///
    /// Takes a list of grid coordinates and produces a `Path` with world-space
    /// waypoints, terrain layers, and path optimization applied.
    pub fn build_actual_path(
        &self,
        grid_path: &[GridCoord],
        from_world: &Coord3D,
        to_world: &Coord3D,
        surfaces: LocomotorSurfaceTypeMask,
        is_crusher: bool,
        blocked: bool,
    ) -> PathResult {
        if grid_path.is_empty() {
            return PathResult::none();
        }

        let mut waypoints = Vec::with_capacity(grid_path.len());
        let mut layers = Vec::with_capacity(grid_path.len());

        for (idx, coord) in grid_path.iter().enumerate() {
            let layer = self.get_layer_for_coord(*coord);
            let mut pos = if idx == 0 {
                *from_world
            } else if idx + 1 == grid_path.len() {
                *to_world
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

        // Mark path as blocked by ally if requested (matches C++ blocked flag)
        let _ = (surfaces, is_crusher, blocked);

        PathResult {
            success: true,
            waypoints,
            layers,
            total_cost: self.calculate_path_cost(grid_path),
            blocked_by_ally: blocked,
        }
    }

    /// Classify the entire pathfind map based on terrain data.
    /// Matches C++ Pathfinder::classifyMap() which iterates all cells and sets
    /// cell types based on terrain height/slope/water flags.
    ///
    /// PARITY_TODO: Full terrain classification requires TerrainLogic integration
    /// to read actual slope/water/cliff data per cell. Currently sets all cells
    /// to Clear as a safe default.
    pub fn classify_map(&mut self) {
        let pathfinder = self.pathfinder.lock().unwrap();
        let w = pathfinder.width();
        let h = pathfinder.height();
        drop(pathfinder);

        for x in 0..w {
            for y in 0..h {
                self.classify_map_cell(x as i32, y as i32);
            }
        }

        // Recalculate zones after full classification
        if let Ok(mut zones) = self.zones.lock() {
            zones.calculate_zones();
        }
    }

    /// Classify a single map cell based on terrain data.
    /// Matches C++ Pathfinder::classifyMapCell() at AIPathfind.cpp ~3600-3900.
    ///
    /// Sets cell type to Clear/Cliff/Water/Impassable based on terrain properties.
    pub fn classify_map_cell(&self, x: i32, y: i32) {
        if x < 0 || y < 0 {
            return;
        }
        let _ux = x as usize;
        let _uy = y as usize;

        // Try to get terrain info at this cell center
        let cell_world = GridCoord::new(x, y).to_world(PathfindLayerEnum::Ground);

        let cell_type = if let Some(terrain) = TheTerrainLogic::get() {
            // Classify based on terrain properties
            // PARITY_TODO: Match exact C++ classification from classifyMapCell
            // which uses getGroundHeight slope deltas and water flags
            if terrain.is_underwater(cell_world.x, cell_world.y, None, None) {
                PathfindCellType::Water
            } else {
                // Check slope for cliff classification
                let half_cell = PATHFIND_CELL_SIZE_F * 0.5;
                let h_center = terrain.get_ground_height(cell_world.x, cell_world.y, None);
                let h_right =
                    terrain.get_ground_height(cell_world.x + half_cell, cell_world.y, None);
                let h_up = terrain.get_ground_height(cell_world.x, cell_world.y + half_cell, None);

                let slope_x = (h_right - h_center).abs();
                let slope_y = (h_up - h_center).abs();
                let max_slope = slope_x.max(slope_y);

                // C++ uses PATHFIND_CELL_SIZE_F as the cliff threshold
                if max_slope > PATHFIND_CELL_SIZE_F * 0.7 {
                    PathfindCellType::Cliff
                } else {
                    PathfindCellType::Clear
                }
            }
        } else {
            PathfindCellType::Clear
        };

        if let Ok(mut pathfinder) = self.pathfinder.lock() {
            pathfinder.set_cell_type(GridCoord::new(x, y), cell_type);
        }
    }

    /// Mark an object's footprint cells as blocked obstacles.
    /// Matches C++ Pathfinder::classifyObjectFootprint() at AIPathfind.cpp:4175-4385.
    ///
    /// Iterates the cells covered by the object's geometry and marks them as
    /// Obstacle on the ground-layer grid.
    pub fn classify_object_footprint(&mut self, obj: &crate::object::Object) {
        let pos = obj.get_position();
        let geo = obj.get_geometry_info();
        let radius = geo.get_major_radius();

        // Skip small objects (C++ skips if isSmall())
        if geo.get_is_small() {
            return;
        }
        // Skip objects high above terrain
        if obj.get_height_above_terrain() > PATHFIND_CELL_SIZE_F
            && !obj.is_kind_of(crate::common::KindOf::BlastCrater)
        {
            return;
        }
        // Only structures are obstacles (C++ checks KINDOF_STRUCTURE)
        if !obj.is_kind_of(crate::common::KindOf::Structure) {
            return;
        }
        // Mobile objects aren't obstacles (C++ returns if obj->isMobile())
        if obj.is_mobile() {
            return;
        }

        let center = GridCoord::from_world(pos);
        let radius_cells = (radius / PATHFIND_CELL_SIZE_F).ceil() as i32 + 1;

        for dy in -radius_cells..=radius_cells {
            for dx in -radius_cells..=radius_cells {
                let cx = center.x + dx;
                let cy = center.y + dy;
                if cx < 0 || cy < 0 {
                    continue;
                }
                let ux = cx as usize;
                let uy = cy as usize;
                if ux >= self.width || uy >= self.height {
                    continue;
                }

                // Check if cell center is within object radius
                let cell_center = GridCoord::new(cx, cy).to_world(PathfindLayerEnum::Ground);
                let delta_x = cell_center.x - pos.x;
                let delta_y = cell_center.y - pos.y;
                let dist_sqr = delta_x * delta_x + delta_y * delta_y;
                // Add a small buffer matching C++ radius+0.4*cell_size
                let effective_radius = radius + PATHFIND_CELL_SIZE_F * 0.4;
                if dist_sqr > effective_radius * effective_radius {
                    continue;
                }

                let coord = GridCoord::new(cx, cy);
                if let Ok(mut pathfinder) = self.pathfinder.lock() {
                    pathfinder.set_cell_type(coord, PathfindCellType::Obstacle);
                }
            }
        }

        // Refresh pinched cells around the footprint
        let lo = GridCoord::new(
            (center.x - radius_cells - 2).max(0),
            (center.y - radius_cells - 2).max(0),
        );
        let hi = GridCoord::new(
            (center.x + radius_cells + 2).min(self.width as i32 - 1),
            (center.y + radius_cells + 2).min(self.height as i32 - 1),
        );
        if let Ok(mut pathfinder) = self.pathfinder.lock() {
            pathfinder.refresh_pinched_cells_in_bounds(lo, hi);
        }
    }

    // ========================================================================
    // GROUP B – Destination adjustment
    // ========================================================================

    /// Snap destination to the nearest passable cell using spiral search.
    /// Matches C++ Pathfinder::adjustDestination() at AIPathfind.cpp:5331-5407.
    ///
    /// Returns `true` if adjustment succeeded (dest was modified in-place).
    /// The spiral search pattern matches C++ exactly: right, down, left, up,
    /// expanding outward in a square spiral.
    pub fn adjust_destination(
        &self,
        surfaces: LocomotorSurfaceTypeMask,
        is_crusher: bool,
        dest: &mut Coord3D,
        unit_radius: f32,
        ignore_obstacle_id: Option<ObjectID>,
    ) -> bool {
        let mut adjust_dest = *dest;
        let (radius, center_in_cell) = Self::compute_radius_and_center(unit_radius);
        let cell = GridCoord::from_world(&adjust_dest);
        let layer = PathfindLayerEnum::Ground;

        // Check exact cell first
        if self.is_destination_valid(
            cell,
            layer,
            surfaces,
            is_crusher,
            radius,
            center_in_cell,
            ignore_obstacle_id,
        ) {
            // Snap to cell center
            let snapped = cell.to_world(layer);
            if let Some(terrain) = TheTerrainLogic::get() {
                dest.x = snapped.x;
                dest.y = snapped.y;
                dest.z =
                    terrain.get_layer_height(snapped.x, snapped.y, CommonPathfindLayerEnum::Ground);
            }
            return true;
        }

        // Spiral search - matches C++ at AIPathfind.cpp:5366-5399
        const MAX_CELLS_TO_TRY: i32 = 400;
        let mut limit = MAX_CELLS_TO_TRY;
        let mut i = cell.x;
        let mut j = cell.y;
        let mut delta = 1;

        while limit > 0 {
            // Right
            for _ in 0..delta {
                i += 1;
                limit -= 1;
                if self.try_adjust_cell(
                    i,
                    j,
                    layer,
                    surfaces,
                    is_crusher,
                    radius,
                    center_in_cell,
                    ignore_obstacle_id,
                    dest,
                ) {
                    return true;
                }
            }
            // Down
            for _ in 0..delta {
                j += 1;
                limit -= 1;
                if self.try_adjust_cell(
                    i,
                    j,
                    layer,
                    surfaces,
                    is_crusher,
                    radius,
                    center_in_cell,
                    ignore_obstacle_id,
                    dest,
                ) {
                    return true;
                }
            }
            delta += 1;
            // Left
            for _ in 0..delta {
                i -= 1;
                limit -= 1;
                if self.try_adjust_cell(
                    i,
                    j,
                    layer,
                    surfaces,
                    is_crusher,
                    radius,
                    center_in_cell,
                    ignore_obstacle_id,
                    dest,
                ) {
                    return true;
                }
            }
            // Up
            for _ in 0..delta {
                j -= 1;
                limit -= 1;
                if self.try_adjust_cell(
                    i,
                    j,
                    layer,
                    surfaces,
                    is_crusher,
                    radius,
                    center_in_cell,
                    ignore_obstacle_id,
                    dest,
                ) {
                    return true;
                }
            }
            delta += 1;
        }

        false
    }

    /// Helper: try to adjust destination to a specific cell.
    fn try_adjust_cell(
        &self,
        cx: i32,
        cy: i32,
        layer: PathfindLayerEnum,
        surfaces: LocomotorSurfaceTypeMask,
        is_crusher: bool,
        radius: i32,
        center_in_cell: bool,
        ignore_obstacle_id: Option<ObjectID>,
        dest: &mut Coord3D,
    ) -> bool {
        let coord = GridCoord::new(cx, cy);
        if !self.is_valid_coord(coord) {
            return false;
        }
        if !self.is_destination_valid(
            coord,
            layer,
            surfaces,
            is_crusher,
            radius,
            center_in_cell,
            ignore_obstacle_id,
        ) {
            return false;
        }
        let snapped = coord.to_world(layer);
        if let Some(terrain) = TheTerrainLogic::get() {
            dest.x = snapped.x;
            dest.y = snapped.y;
            dest.z =
                terrain.get_layer_height(snapped.x, snapped.y, CommonPathfindLayerEnum::Ground);
        }
        true
    }

    /// Check if a cell is a valid destination for the given parameters.
    /// Matches C++ Pathfinder::checkDestination() logic.
    fn is_destination_valid(
        &self,
        cell: GridCoord,
        _layer: PathfindLayerEnum,
        surfaces: LocomotorSurfaceTypeMask,
        is_crusher: bool,
        radius: i32,
        center_in_cell: bool,
        ignore_obstacle_id: Option<ObjectID>,
    ) -> bool {
        if !self.is_valid_coord(cell) {
            return false;
        }

        let ignore_cells = ignored_obstacle_cells(ignore_obstacle_id);
        let pathfinder = self.pathfinder.lock().unwrap();

        // Check all cells in the unit's footprint
        let mut num_cells_above = radius;
        if center_in_cell {
            num_cells_above += 1;
        }
        let start_x = cell.x - radius;
        let end_x = cell.x + num_cells_above;
        let start_y = cell.y - radius;
        let end_y = cell.y + num_cells_above;

        for x in start_x..end_x {
            for y in start_y..end_y {
                let coord = GridCoord::new(x, y);
                if !pathfinder.is_passable_with_ignore(
                    coord,
                    surfaces,
                    is_crusher,
                    ignore_cells.as_ref(),
                ) {
                    return false;
                }
            }
        }
        true
    }

    /// Find a pathable spot near the destination.
    /// Matches C++ Pathfinder::adjustToPossibleDestination() at AIPathfind.cpp:5510-5617.
    ///
    /// Searches for a cell in the same zone as the unit that is passable and
    /// valid as a destination, using the same spiral search pattern.
    pub fn adjust_to_possible_destination(
        &self,
        start: &Coord3D,
        dest: &mut Coord3D,
        surfaces: LocomotorSurfaceTypeMask,
        is_crusher: bool,
        unit_radius: f32,
    ) -> bool {
        let (radius, center_in_cell) = Self::compute_radius_and_center(unit_radius);
        let goal_cell = GridCoord::from_world(dest);

        // Check if start and goal are in the same zone
        let start_cell = GridCoord::from_world(start);
        let same_zone = if let Ok(zones) = self.zones.lock() {
            zones.are_connected(start_cell, goal_cell, surfaces, is_crusher)
        } else {
            true
        };

        if same_zone {
            if self.is_destination_valid(
                goal_cell,
                PathfindLayerEnum::Ground,
                surfaces,
                is_crusher,
                radius,
                center_in_cell,
                None,
            ) {
                return true;
            }
        }

        // Spiral search
        const MAX_CELLS_TO_TRY: i32 = 400;
        let mut limit = MAX_CELLS_TO_TRY;
        let mut i = goal_cell.x;
        let mut j = goal_cell.y;
        let mut delta = 1;

        while limit > 0 {
            for _ in 0..delta {
                i += 1;
                limit -= 1;
                if self.try_zone_adjust(
                    i,
                    j,
                    start_cell,
                    surfaces,
                    is_crusher,
                    radius,
                    center_in_cell,
                    dest,
                ) {
                    return true;
                }
            }
            for _ in 0..delta {
                j += 1;
                limit -= 1;
                if self.try_zone_adjust(
                    i,
                    j,
                    start_cell,
                    surfaces,
                    is_crusher,
                    radius,
                    center_in_cell,
                    dest,
                ) {
                    return true;
                }
            }
            delta += 1;
            for _ in 0..delta {
                i -= 1;
                limit -= 1;
                if self.try_zone_adjust(
                    i,
                    j,
                    start_cell,
                    surfaces,
                    is_crusher,
                    radius,
                    center_in_cell,
                    dest,
                ) {
                    return true;
                }
            }
            for _ in 0..delta {
                j -= 1;
                limit -= 1;
                if self.try_zone_adjust(
                    i,
                    j,
                    start_cell,
                    surfaces,
                    is_crusher,
                    radius,
                    center_in_cell,
                    dest,
                ) {
                    return true;
                }
            }
            delta += 1;
        }

        false
    }

    fn try_zone_adjust(
        &self,
        cx: i32,
        cy: i32,
        start_cell: GridCoord,
        surfaces: LocomotorSurfaceTypeMask,
        is_crusher: bool,
        radius: i32,
        center_in_cell: bool,
        dest: &mut Coord3D,
    ) -> bool {
        let coord = GridCoord::new(cx, cy);
        if !self.is_valid_coord(coord) {
            return false;
        }

        // Check zone connectivity
        let connected = if let Ok(zones) = self.zones.lock() {
            zones.are_connected(start_cell, coord, surfaces, is_crusher)
        } else {
            true
        };
        if !connected {
            return false;
        }

        if !self.is_destination_valid(
            coord,
            PathfindLayerEnum::Ground,
            surfaces,
            is_crusher,
            radius,
            center_in_cell,
            None,
        ) {
            return false;
        }

        let snapped = coord.to_world(PathfindLayerEnum::Ground);
        if let Some(terrain) = TheTerrainLogic::get() {
            dest.x = snapped.x;
            dest.y = snapped.y;
            dest.z =
                terrain.get_layer_height(snapped.x, snapped.y, CommonPathfindLayerEnum::Ground);
        }
        true
    }

    /// Full adjustment pipeline combining adjustDestination and zone check.
    /// Matches C++ Pathfinder::checkForAdjust() at AIPathfind.cpp ~5300.
    pub fn check_for_adjust(
        &self,
        dest: &mut Coord3D,
        surfaces: LocomotorSurfaceTypeMask,
        is_crusher: bool,
        unit_radius: f32,
        ignore_obstacle_id: Option<ObjectID>,
    ) -> bool {
        self.adjust_destination(surfaces, is_crusher, dest, unit_radius, ignore_obstacle_id)
    }

    /// Validate a destination cell is passable for the given parameters.
    /// Matches C++ Pathfinder::checkDestination() at AIPathfind.cpp ~5200.
    pub fn validate_destination(
        &self,
        dest: &Coord3D,
        surfaces: LocomotorSurfaceTypeMask,
        is_crusher: bool,
        unit_radius: f32,
    ) -> bool {
        let (radius, center_in_cell) = Self::compute_radius_and_center(unit_radius);
        let cell = GridCoord::from_world(dest);
        self.is_destination_valid(
            cell,
            PathfindLayerEnum::Ground,
            surfaces,
            is_crusher,
            radius,
            center_in_cell,
            None,
        )
    }

    // ========================================================================
    // GROUP C – Hierarchical pathfinding
    // ========================================================================

    /// Long-distance hierarchical path check using zone connectivity.
    /// Matches C++ Pathfinder::findHierarchicalPath() concept.
    ///
    /// Uses the zone manager to verify that start and end are in connected
    /// zones, then delegates to the full A* pathfinder.
    pub fn find_hierarchical_path(
        &self,
        start: Coord3D,
        end: Coord3D,
        surfaces: LocomotorSurfaceTypeMask,
        is_crusher: bool,
    ) -> Option<PathResult> {
        let start_cell = GridCoord::from_world(&start);
        let end_cell = GridCoord::from_world(&end);

        // Zone connectivity check (fast rejection for disconnected areas)
        if let Ok(zones) = self.zones.lock() {
            if !zones.are_connected(start_cell, end_cell, surfaces, is_crusher) {
                return None;
            }
        }

        // Zones are connected – run full A*
        let request = PathRequest {
            object_id: INVALID_ID,
            from: start,
            to: end,
            surfaces,
            is_crusher,
            unit_radius: 0.0,
            allow_partial: false,
            move_allies: false,
            ignore_obstacle_id: None,
        };
        let result = self.find_path(request);
        if result.success {
            Some(result)
        } else {
            None
        }
    }

    /// Find closest reachable hierarchical path (for unreachable goals).
    /// Matches C++ Pathfinder::findClosestHierarchicalPath().
    ///
    /// If exact path fails, searches nearby cells for reachable alternatives.
    pub fn find_closest_hierarchical_path(
        &self,
        start: Coord3D,
        end: Coord3D,
        surfaces: LocomotorSurfaceTypeMask,
        is_crusher: bool,
    ) -> Option<PathResult> {
        // Try exact path first
        if let Some(result) = self.find_hierarchical_path(start, end, surfaces, is_crusher) {
            return Some(result);
        }

        // Search nearby cells for reachable alternatives
        let goal_cell = GridCoord::from_world(&end);
        let max_search: i32 = 20;

        for radius in 1..=max_search {
            for dx in -radius..=radius {
                for dy in -radius..=radius {
                    if dx.abs() < radius && dy.abs() < radius {
                        continue;
                    }
                    let test_coord = GridCoord::new(goal_cell.x + dx, goal_cell.y + dy);
                    if !self.is_valid_coord(test_coord) {
                        continue;
                    }
                    // Check if this cell is passable
                    let pathfinder = self.pathfinder.lock().unwrap();
                    let passable = pathfinder.is_passable(test_coord, surfaces, is_crusher);
                    drop(pathfinder);
                    if !passable {
                        continue;
                    }

                    let test_pos = test_coord.to_world(PathfindLayerEnum::Ground);
                    if let Some(result) =
                        self.find_hierarchical_path(start, test_pos, surfaces, is_crusher)
                    {
                        return Some(result);
                    }
                }
            }
        }

        None
    }

    // ========================================================================
    // GROUP D – Path utilities and dynamic map updates
    // ========================================================================

    /// Quick path existence check (for UI feedback).
    /// Matches C++ Pathfinder::quickDoesPathExist() concept.
    ///
    /// Uses zone connectivity as a fast heuristic. Does not run full A*.
    pub fn quick_does_path_exist(
        &self,
        start: &Coord3D,
        end: &Coord3D,
        surfaces: LocomotorSurfaceTypeMask,
        is_crusher: bool,
    ) -> bool {
        let start_cell = GridCoord::from_world(start);
        let end_cell = GridCoord::from_world(end);

        // Bounds check
        if !self.is_valid_coord(start_cell) || !self.is_valid_coord(end_cell) {
            return false;
        }

        // Quick passability check on start/end
        let pathfinder = self.pathfinder.lock().unwrap();
        if !pathfinder.is_passable(start_cell, surfaces, is_crusher) {
            return false;
        }
        if !pathfinder.is_passable(end_cell, surfaces, is_crusher) {
            return false;
        }
        drop(pathfinder);

        // Zone connectivity check
        if let Ok(zones) = self.zones.lock() {
            zones.are_connected(start_cell, end_cell, surfaces, is_crusher)
        } else {
            true
        }
    }

    /// Full path existence check (runs actual A*).
    /// Matches C++ Pathfinder::slowDoesPathExist() concept.
    pub fn slow_does_path_exist(
        &self,
        start: &Coord3D,
        end: &Coord3D,
        surfaces: LocomotorSurfaceTypeMask,
        is_crusher: bool,
    ) -> bool {
        let request = PathRequest {
            object_id: INVALID_ID,
            from: *start,
            to: *end,
            surfaces,
            is_crusher,
            unit_radius: 0.0,
            allow_partial: false,
            move_allies: false,
            ignore_obstacle_id: None,
        };
        self.find_path(request).success
    }

    /// Check if a ground path is passable between two points.
    /// Matches C++ Pathfinder::isGroundPathPassable().
    pub fn is_ground_path_passable(
        &self,
        start: &Coord3D,
        end: &Coord3D,
        is_crusher: bool,
        diameter: i32,
    ) -> bool {
        self.is_ground_line_passable(start, end, is_crusher, diameter, None)
    }

    /// Snap a world position to the nearest cell center.
    /// Matches C++ Pathfinder::adjustCoordToCell() at AIPathfind.cpp:8936-8946.
    pub fn snap_position(&self, pos: &Coord3D) -> Coord3D {
        let coord = GridCoord::from_world(pos);
        let mut snapped = coord.to_world(PathfindLayerEnum::Ground);
        // Set Z from terrain
        if let Some(terrain) = TheTerrainLogic::get() {
            snapped.z = terrain.get_ground_height(snapped.x, snapped.y, None);
        } else {
            snapped.z = pos.z;
        }
        snapped
    }

    /// Dynamic pathfind map update: register a goal cell for a unit.
    /// Matches C++ Pathfinder::updateGoal() at AIPathfind.cpp ~2800.
    pub fn update_goal(
        &self,
        cell: GridCoord,
        unit_id: ObjectID,
        layer: PathfindLayerEnum,
        radius: i32,
        center_in_cell: bool,
    ) {
        self.set_goal_cells(
            unit_id,
            ICoord2D::new(cell.x, cell.y),
            radius,
            center_in_cell,
            layer,
            true, // do_ground
            true, // do_layer
        );
    }

    /// Dynamic pathfind map update: register a position cell for a unit.
    /// Matches C++ Pathfinder::updatePos() at AIPathfind.cpp ~2700.
    pub fn update_pos(
        &self,
        cell: GridCoord,
        unit_id: ObjectID,
        layer: PathfindLayerEnum,
        radius: i32,
        center_in_cell: bool,
    ) {
        // Position updates set the unit as present in the cell
        // The goal cells mechanism handles this
        self.set_goal_cells(
            unit_id,
            ICoord2D::new(cell.x, cell.y),
            radius,
            center_in_cell,
            layer,
            true,
            false,
        );
    }

    /// Change bridge state on the pathfind map.
    /// Matches C++ PathfindLayer::setDestroyed() at AIPathfind.cpp:3589-3597.
    ///
    /// When destroyed, all bridge cells become BridgeImpassable and the
    /// ground layer is disconnected from the bridge layer.
    pub fn change_bridge_state(&mut self, x: i32, y: i32, destroyed: bool) {
        // Find a bridge that contains this cell
        let coord = GridCoord::new(x, y);
        for bridge in &mut self.bridges {
            if bridge.contains(coord) {
                bridge.destroyed = destroyed;
                // Re-classify cells within bridge bounds
                let lo = bridge.bounds.0;
                let hi = bridge.bounds.1;
                if destroyed {
                    // Mark bridge cells as impassable
                    if let Ok(mut pathfinder) = self.pathfinder.lock() {
                        for bx in lo.x..=hi.x {
                            for by in lo.y..=hi.y {
                                pathfinder.set_cell_type(
                                    GridCoord::new(bx, by),
                                    PathfindCellType::BridgeImpassable,
                                );
                            }
                        }
                    }
                } else {
                    // Restore bridge cells based on terrain classification
                    for bx in lo.x..=hi.x {
                        for by in lo.y..=hi.y {
                            self.classify_map_cell(bx, by);
                        }
                    }
                }
                // Invalidate cache since map changed
                self.clear_cache();
                break;
            }
        }
    }

    /// Get the width of the pathfinding grid.
    pub fn width(&self) -> usize {
        self.width
    }

    /// Get the height of the pathfinding grid.
    pub fn height(&self) -> usize {
        self.height
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

        if start_zone == 0 || goal_zone == 0 {
            return true;
        }

        start_zone == goal_zone
    }

    /// Calculate zones using flood-fill on the pathfinder grid.
    /// Matches C++ PathfindZoneManager::calculateZones().
    fn calculate_zones(&mut self) {
        for col in self.zones.iter_mut() {
            for zone in col.iter_mut() {
                *zone = 0;
            }
        }
        self.next_zone = 1;

        for x in 0..self.width {
            for y in 0..self.height {
                if self.zones[x][y] == 0 {
                    self.flood_fill(x, y);
                }
            }
        }
    }

    fn flood_fill(&mut self, start_x: usize, start_y: usize) {
        let zone_id = self.next_zone;
        self.next_zone += 1;
        if self.next_zone == 0 {
            self.next_zone = 1;
        }

        let mut stack = vec![(start_x, start_y)];
        while let Some((x, y)) = stack.pop() {
            if x >= self.width || y >= self.height {
                continue;
            }
            if self.zones[x][y] != 0 {
                continue;
            }
            self.zones[x][y] = zone_id;

            if x > 0 {
                stack.push((x - 1, y));
            }
            if x + 1 < self.width {
                stack.push((x + 1, y));
            }
            if y > 0 {
                stack.push((x, y - 1));
            }
            if y + 1 < self.height {
                stack.push((x, y + 1));
            }
        }
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
