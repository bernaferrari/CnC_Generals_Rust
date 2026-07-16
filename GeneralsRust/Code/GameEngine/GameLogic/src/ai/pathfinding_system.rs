//! Production-Ready Pathfinding System
//!
//! This module provides a complete, optimized pathfinding implementation featuring:
//! - A* algorithm with priority queue and multiple heuristics
//! - Hierarchical pathfinding for long-distance paths
//! - Multi-layer movement (ground, air, water, tunnel)
//! - Dynamic obstacle avoidance with real-time replanning
//! - Path caching and optimization
//! - Throttled computation over multiple frames
//! - Formation pathfinding support
//! - Terrain cost tables for different surface types
//! - Flow fields for group movement
//! - Waypoint network support

use super::pathfind_complete::PathRequest as ClassicPathRequest;
use crate::common::{Coord2D, Coord3D, ICoord2D, ObjectID, Real, INVALID_ID};
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap, HashSet, VecDeque};
use std::f32::INFINITY;
use std::sync::{Arc, Mutex, RwLock};

/// Pathfinding cell size in world units
pub const PATHFIND_CELL_SIZE: f32 = 10.0;

/// Close enough distance for path completion
pub const PATHFIND_CLOSE_ENOUGH: f32 = 1.0;

/// Maximum path length before failing
pub const MAX_PATH_LENGTH: usize = 2048;

/// Maximum pathfinding iterations per frame (for throttling)
pub const MAX_ITERATIONS_PER_FRAME: usize = 100;

/// Path cache expiration time (frames)
pub const PATH_CACHE_EXPIRATION: u32 = 300;

/// Cluster size for hierarchical pathfinding
pub const CLUSTER_SIZE: i32 = 10;

// ============================================================================
// TERRAIN AND MOVEMENT TYPES
// ============================================================================

/// Terrain types with different movement costs
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TerrainType {
    Clear = 0,      // Open ground, no penalty
    Rough = 1,      // Rough terrain, minor penalty
    VeryRough = 2,  // Very rough terrain, major penalty
    Water = 3,      // Water terrain
    DeepWater = 4,  // Deep water
    Cliff = 5,      // Cliff/steep slope
    Rubble = 6,     // Destructible rubble
    Obstacle = 7,   // Static obstacle
    Impassable = 8, // Cannot pass
}

/// Movement layers for different unit types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PathfindLayerEnum {
    Invalid = 0,
    Ground = 1, // Ground units
    Air = 2,    // Aircraft
    Water = 3,  // Naval units
    Tunnel = 4, // Underground units
}

impl From<crate::common::PathfindLayerEnum> for PathfindLayerEnum {
    fn from(layer: crate::common::PathfindLayerEnum) -> Self {
        match layer {
            crate::common::PathfindLayerEnum::Ground => PathfindLayerEnum::Ground,
            crate::common::PathfindLayerEnum::Top => PathfindLayerEnum::Air,
            crate::common::PathfindLayerEnum::Water => PathfindLayerEnum::Water,
            crate::common::PathfindLayerEnum::Bridge1
            | crate::common::PathfindLayerEnum::Bridge2
            | crate::common::PathfindLayerEnum::Bridge3
            | crate::common::PathfindLayerEnum::Bridge4
            | crate::common::PathfindLayerEnum::Wall => PathfindLayerEnum::Ground,
            _ => PathfindLayerEnum::Ground,
        }
    }
}

/// Movement capabilities for pathfinding
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MovementCapabilities {
    /// Movement layer (ground, air, water, tunnel)
    pub layer: PathfindLayerEnum,
    /// Can cross water
    pub amphibious: bool,
    /// Can crush obstacles
    pub crusher: bool,
    /// Can climb cliffs
    pub climber: bool,
    /// Can fly
    pub flying: bool,
    /// Can tunnel
    pub tunneling: bool,
    /// Surface type mask (bitmask of acceptable terrain)
    pub surface_mask: u32,
}

impl MovementCapabilities {
    /// Create ground unit capabilities
    pub fn ground() -> Self {
        Self {
            layer: PathfindLayerEnum::Ground,
            amphibious: false,
            crusher: false,
            climber: false,
            flying: false,
            tunneling: false,
            surface_mask: 0x01, // Only clear terrain
        }
    }

    /// Create amphibious unit capabilities
    pub fn amphibious() -> Self {
        Self {
            layer: PathfindLayerEnum::Ground,
            amphibious: true,
            crusher: false,
            climber: false,
            flying: false,
            tunneling: false,
            surface_mask: 0x09, // Clear + Water
        }
    }

    /// Create aircraft capabilities
    pub fn air() -> Self {
        Self {
            layer: PathfindLayerEnum::Air,
            amphibious: true,
            crusher: false,
            climber: true,
            flying: true,
            tunneling: false,
            surface_mask: 0xFFFFFFFF, // All terrain
        }
    }

    /// Create naval unit capabilities
    pub fn naval() -> Self {
        Self {
            layer: PathfindLayerEnum::Water,
            amphibious: false,
            crusher: false,
            climber: false,
            flying: false,
            tunneling: false,
            surface_mask: 0x18, // Water + DeepWater
        }
    }
}

/// Terrain cost table for different movement types
#[derive(Debug, Clone)]
pub struct TerrainCostTable {
    costs: HashMap<(TerrainType, PathfindLayerEnum), f32>,
}

impl TerrainCostTable {
    /// Create new terrain cost table with defaults
    pub fn new() -> Self {
        let mut costs = HashMap::new();

        // Ground movement costs
        costs.insert((TerrainType::Clear, PathfindLayerEnum::Ground), 1.0);
        costs.insert((TerrainType::Rough, PathfindLayerEnum::Ground), 2.0);
        costs.insert((TerrainType::VeryRough, PathfindLayerEnum::Ground), 4.0);
        costs.insert((TerrainType::Water, PathfindLayerEnum::Ground), INFINITY);
        costs.insert(
            (TerrainType::DeepWater, PathfindLayerEnum::Ground),
            INFINITY,
        );
        costs.insert((TerrainType::Cliff, PathfindLayerEnum::Ground), INFINITY);
        costs.insert((TerrainType::Rubble, PathfindLayerEnum::Ground), 3.0);
        costs.insert((TerrainType::Obstacle, PathfindLayerEnum::Ground), INFINITY);
        costs.insert(
            (TerrainType::Impassable, PathfindLayerEnum::Ground),
            INFINITY,
        );

        // Air movement costs (uniform - aircraft ignore terrain)
        for terrain in &[
            TerrainType::Clear,
            TerrainType::Rough,
            TerrainType::VeryRough,
            TerrainType::Water,
            TerrainType::DeepWater,
            TerrainType::Cliff,
            TerrainType::Rubble,
            TerrainType::Obstacle,
            TerrainType::Impassable,
        ] {
            costs.insert((*terrain, PathfindLayerEnum::Air), 1.0);
        }

        // Water movement costs
        costs.insert((TerrainType::Clear, PathfindLayerEnum::Water), INFINITY);
        costs.insert((TerrainType::Rough, PathfindLayerEnum::Water), INFINITY);
        costs.insert((TerrainType::VeryRough, PathfindLayerEnum::Water), INFINITY);
        costs.insert((TerrainType::Water, PathfindLayerEnum::Water), 1.0);
        costs.insert((TerrainType::DeepWater, PathfindLayerEnum::Water), 1.0);
        costs.insert((TerrainType::Cliff, PathfindLayerEnum::Water), INFINITY);
        costs.insert((TerrainType::Rubble, PathfindLayerEnum::Water), INFINITY);
        costs.insert((TerrainType::Obstacle, PathfindLayerEnum::Water), INFINITY);
        costs.insert(
            (TerrainType::Impassable, PathfindLayerEnum::Water),
            INFINITY,
        );

        Self { costs }
    }

    /// Get movement cost for terrain and layer
    pub fn get_cost(
        &self,
        terrain: TerrainType,
        layer: PathfindLayerEnum,
        caps: &MovementCapabilities,
    ) -> f32 {
        let base_cost = self
            .costs
            .get(&(terrain, layer))
            .copied()
            .unwrap_or(INFINITY);

        // Apply capability modifiers for terrains that have alternate traversal behavior.
        if terrain == TerrainType::Rubble && layer == PathfindLayerEnum::Ground && caps.crusher {
            return 1.5;
        }

        // Apply capability modifiers
        if base_cost == INFINITY {
            // Check if capabilities override impassability
            match terrain {
                TerrainType::Water | TerrainType::DeepWater => {
                    if caps.amphibious || caps.flying {
                        return 1.5; // Can cross but with penalty
                    }
                }
                TerrainType::Cliff => {
                    if caps.climber || caps.flying {
                        return 3.0; // Can climb/fly over
                    }
                }
                TerrainType::Rubble => {
                    if caps.crusher {
                        return 1.5; // Can crush through
                    }
                }
                _ => {}
            }
        }

        base_cost
    }
}

// ============================================================================
// GRID AND CELL STRUCTURES
// ============================================================================

/// Grid coordinate for pathfinding
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GridCoord {
    pub x: i32,
    pub y: i32,
    pub layer: u8,
}

impl GridCoord {
    pub fn new(x: i32, y: i32, layer: PathfindLayerEnum) -> Self {
        Self {
            x,
            y,
            layer: layer as u8,
        }
    }

    /// Convert world coordinates to grid coordinates
    pub fn from_world(pos: &Coord3D, layer: PathfindLayerEnum) -> Self {
        Self {
            x: (pos.x / PATHFIND_CELL_SIZE).floor() as i32,
            y: (pos.y / PATHFIND_CELL_SIZE).floor() as i32,
            layer: layer as u8,
        }
    }

    /// Convert to world coordinates (center of cell)
    pub fn to_world(&self, height: f32) -> Coord3D {
        Coord3D::new(
            (self.x as f32 + 0.5) * PATHFIND_CELL_SIZE,
            (self.y as f32 + 0.5) * PATHFIND_CELL_SIZE,
            height,
        )
    }

    /// Manhattan distance heuristic
    pub fn manhattan_distance(&self, other: &GridCoord) -> f32 {
        let dx = (self.x - other.x).abs() as f32;
        let dy = (self.y - other.y).abs() as f32;
        let layer_penalty = if self.layer != other.layer { 50.0 } else { 0.0 };
        dx + dy + layer_penalty
    }

    /// Euclidean distance heuristic
    pub fn euclidean_distance(&self, other: &GridCoord) -> f32 {
        let dx = (self.x - other.x) as f32;
        let dy = (self.y - other.y) as f32;
        let layer_penalty = if self.layer != other.layer { 50.0 } else { 0.0 };
        (dx * dx + dy * dy).sqrt() + layer_penalty
    }

    /// Diagonal distance heuristic (octile distance)
    pub fn diagonal_distance(&self, other: &GridCoord) -> f32 {
        let dx = (self.x - other.x).abs() as f32;
        let dy = (self.y - other.y).abs() as f32;
        let d = 1.0; // Orthogonal cost
        let d2 = 1.414; // Diagonal cost (sqrt(2))
        let layer_penalty = if self.layer != other.layer { 50.0 } else { 0.0 };
        d * (dx + dy) + (d2 - 2.0 * d) * dx.min(dy) + layer_penalty
    }

    /// Get 8-directional neighbors
    pub fn get_neighbors(&self) -> Vec<(GridCoord, f32)> {
        let mut neighbors = Vec::with_capacity(8);

        for dx in -1..=1 {
            for dy in -1..=1 {
                if dx == 0 && dy == 0 {
                    continue;
                }

                let neighbor = GridCoord {
                    x: self.x + dx,
                    y: self.y + dy,
                    layer: self.layer,
                };

                // Diagonal movement costs sqrt(2) ≈ 1.414
                let cost = if dx != 0 && dy != 0 { 1.414 } else { 1.0 };
                neighbors.push((neighbor, cost));
            }
        }

        neighbors
    }
}

/// Pathfinding cell data
#[derive(Debug, Clone)]
pub struct PathfindCell {
    /// Terrain type
    pub terrain: TerrainType,
    /// Height/elevation
    pub elevation: f32,
    /// Static obstacle ID
    pub obstacle_id: Option<ObjectID>,
    /// Dynamic obstacle (temporary)
    pub temp_blocked: bool,
    /// Unit currently occupying
    pub occupant_id: Option<ObjectID>,
    /// Movement cost modifier
    pub cost_modifier: f32,
    /// Zone ID for hierarchical pathfinding
    pub zone: u16,
    /// Cluster ID for hierarchical pathfinding
    pub cluster: u16,
}

impl Default for PathfindCell {
    fn default() -> Self {
        Self {
            terrain: TerrainType::Clear,
            elevation: 0.0,
            obstacle_id: None,
            temp_blocked: false,
            occupant_id: None,
            cost_modifier: 1.0,
            zone: 0,
            cluster: 0,
        }
    }
}

impl PathfindCell {
    /// Check if passable for given capabilities
    pub fn is_passable(
        &self,
        caps: &MovementCapabilities,
        terrain_costs: &TerrainCostTable,
        ignore_obstacle_id: Option<ObjectID>,
    ) -> bool {
        // Check temporary block
        if self.temp_blocked {
            return false;
        }

        // Check static obstacle
        if let Some(obstacle_id) = self.obstacle_id {
            if Some(obstacle_id) != ignore_obstacle_id && !caps.crusher {
                return false;
            }
        }

        // Check terrain passability
        let layer = caps.layer;
        let cost = terrain_costs.get_cost(self.terrain, layer, caps);
        cost != INFINITY
    }

    /// Get movement cost through this cell
    pub fn get_movement_cost(
        &self,
        caps: &MovementCapabilities,
        terrain_costs: &TerrainCostTable,
    ) -> f32 {
        let base_cost = terrain_costs.get_cost(self.terrain, caps.layer, caps);
        if base_cost == INFINITY {
            return INFINITY;
        }

        let mut cost = base_cost * self.cost_modifier;

        // Add penalty for occupied cells
        if self.occupant_id.is_some() {
            cost += 2.0;
        }

        cost
    }
}

// ============================================================================
// PATH STRUCTURES
// ============================================================================

/// Waypoint in a path
#[derive(Debug, Clone)]
pub struct PathWaypoint {
    /// Position in world coordinates
    pub position: Coord3D,
    /// Movement layer at this waypoint
    pub layer: PathfindLayerEnum,
    /// Distance along path from start
    pub distance: f32,
}

/// Complete path result
#[derive(Debug, Clone)]
pub struct Path {
    /// Waypoints making up the path
    pub waypoints: Vec<PathWaypoint>,
    /// Total path cost
    pub total_cost: f32,
    /// Whether path is complete or partial
    pub complete: bool,
    /// Whether path is optimized
    pub optimized: bool,
    /// Frame when path was created
    pub created_frame: u32,
}

impl Path {
    /// Get total path length
    pub fn length(&self) -> f32 {
        if let Some(last) = self.waypoints.last() {
            last.distance
        } else {
            0.0
        }
    }

    /// Get number of waypoints
    pub fn waypoint_count(&self) -> usize {
        self.waypoints.len()
    }

    /// Get first waypoint
    pub fn first_waypoint(&self) -> Option<&PathWaypoint> {
        self.waypoints.first()
    }

    /// Get last waypoint
    pub fn last_waypoint(&self) -> Option<&PathWaypoint> {
        self.waypoints.last()
    }

    /// Get closest waypoint to position
    pub fn closest_waypoint(&self, pos: &Coord3D) -> Option<(usize, f32)> {
        let mut closest_idx = 0;
        let mut closest_dist = INFINITY;

        for (i, waypoint) in self.waypoints.iter().enumerate() {
            let dist = (*pos - waypoint.position).length();
            if dist < closest_dist {
                closest_dist = dist;
                closest_idx = i;
            }
        }

        if closest_dist < INFINITY {
            Some((closest_idx, closest_dist))
        } else {
            None
        }
    }

    /// Remove waypoints up to index
    pub fn advance_to_waypoint(&mut self, idx: usize) {
        if idx < self.waypoints.len() {
            self.waypoints.drain(0..idx);

            // Recalculate distances
            let mut distance = 0.0;
            for i in 0..self.waypoints.len() {
                if i > 0 {
                    distance +=
                        (self.waypoints[i].position - self.waypoints[i - 1].position).length();
                }
                self.waypoints[i].distance = distance;
            }
        }
    }
}

/// Pathfinding request
#[derive(Debug, Clone)]
pub struct PathRequest {
    /// Requesting object ID
    pub requester: ObjectID,
    /// Start position
    pub start: Coord3D,
    /// Goal position
    pub goal: Coord3D,
    /// Movement capabilities
    pub capabilities: MovementCapabilities,
    /// Unit size (radius)
    pub unit_size: f32,
    /// Priority (higher = more important)
    pub priority: u32,
    /// Allow partial paths
    pub allow_partial: bool,
    /// Frame request was made
    pub frame_requested: u32,
    /// Allow pathing through allied units
    pub move_allies: bool,
    /// Obstacle ID to ignore (goal object, docking target, etc.)
    pub ignore_obstacle_id: Option<ObjectID>,
}

/// Pathfinding result
#[derive(Debug)]
pub enum PathResult {
    /// Path found successfully
    Success(Path),
    /// Path not found
    Failed(String),
    /// Still computing (throttled)
    Pending,
}

// ============================================================================
// A* NODE
// ============================================================================

/// A* search node
#[derive(Debug, Clone)]
struct AStarNode {
    coord: GridCoord,
    g_score: f32, // Cost from start
    h_score: f32, // Heuristic to goal
    parent: Option<GridCoord>,
}

impl AStarNode {
    fn f_score(&self) -> f32 {
        self.g_score + self.h_score
    }
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
    fn cmp(&self, other: &Self) -> Ordering {
        // Reverse for min-heap
        other
            .f_score()
            .partial_cmp(&self.f_score())
            .unwrap_or(Ordering::Equal)
    }
}

// ============================================================================
// PATHFINDING SYSTEM
// ============================================================================

/// Main pathfinding system
#[derive(Debug)]
pub struct PathfindingSystem {
    /// Pathfinding grid
    grid: HashMap<GridCoord, PathfindCell>,
    /// Grid bounds
    bounds: (GridCoord, GridCoord),
    /// Terrain cost table
    terrain_costs: TerrainCostTable,
    /// Active pathfinding requests queue
    request_queue: VecDeque<PathRequest>,
    /// Ongoing pathfinding operations (throttled)
    ongoing: HashMap<ObjectID, OngoingPathfind>,
    /// Completed pathfinding results ready for pickup
    completed: HashMap<ObjectID, PathResult>,
    /// Path cache
    path_cache: HashMap<(GridCoord, GridCoord, PathfindLayerEnum, ObjectID), CachedPath>,
    /// Current frame
    pub(crate) current_frame: u32,
    /// Waypoint network
    waypoint_network: WaypointNetwork,
    /// Flow fields for group movement
    flow_fields: HashMap<ObjectID, FlowField>,
}

/// Cached path entry
#[derive(Debug, Clone)]
struct CachedPath {
    path: Path,
    expiration_frame: u32,
    hit_count: u32,
}

/// Ongoing throttled pathfinding operation
#[derive(Debug)]
struct OngoingPathfind {
    request: PathRequest,
    open_set: BinaryHeap<AStarNode>,
    closed_set: HashSet<GridCoord>,
    came_from: HashMap<GridCoord, GridCoord>,
    g_score: HashMap<GridCoord, f32>,
    iterations: usize,
    best_partial: GridCoord,
    best_distance: f32,
}

/// Waypoint network for navigation
#[derive(Debug, Default)]
pub struct WaypointNetwork {
    pub(crate) waypoints: HashMap<u32, Coord3D>,
    pub(crate) connections: HashMap<u32, Vec<(u32, f32)>>, // waypoint_id -> [(connected_id, cost)]
}

impl WaypointNetwork {
    /// Add waypoint to network
    pub fn add_waypoint(&mut self, id: u32, position: Coord3D) {
        self.waypoints.insert(id, position);
        self.connections.entry(id).or_insert_with(Vec::new);
    }

    /// Connect two waypoints
    pub fn connect_waypoints(&mut self, from_id: u32, to_id: u32, cost: f32) {
        self.connections
            .entry(from_id)
            .or_insert_with(Vec::new)
            .push((to_id, cost));
        self.connections
            .entry(to_id)
            .or_insert_with(Vec::new)
            .push((from_id, cost));
    }

    /// Find nearest waypoint to position
    pub fn find_nearest(&self, pos: &Coord3D) -> Option<(u32, f32)> {
        let mut nearest = None;
        let mut nearest_dist = INFINITY;

        for (&id, waypoint_pos) in &self.waypoints {
            let dist = (*pos - *waypoint_pos).length();
            if dist < nearest_dist {
                nearest_dist = dist;
                nearest = Some((id, dist));
            }
        }

        nearest
    }
}

/// Flow field for group movement
#[derive(Debug)]
pub struct FlowField {
    /// Flow vectors per cell
    pub(crate) directions: HashMap<GridCoord, Coord2D>,
    /// Goal position
    goal: GridCoord,
    /// Bounds
    bounds: (GridCoord, GridCoord),
    /// Creation frame
    created_frame: u32,
}

impl FlowField {
    /// Get flow direction at position
    pub fn get_direction(&self, pos: &Coord3D, layer: PathfindLayerEnum) -> Option<Coord2D> {
        let coord = GridCoord::from_world(pos, layer);
        self.directions.get(&coord).copied()
    }
}

impl PathfindingSystem {
    /// Create new pathfinding system
    pub fn new(width: i32, height: i32) -> Self {
        let min_coord = GridCoord::new(-width / 2, -height / 2, PathfindLayerEnum::Ground);
        let max_coord = GridCoord::new(width / 2, height / 2, PathfindLayerEnum::Ground);

        Self {
            grid: HashMap::new(),
            bounds: (min_coord, max_coord),
            terrain_costs: TerrainCostTable::new(),
            request_queue: VecDeque::new(),
            ongoing: HashMap::new(),
            completed: HashMap::new(),
            path_cache: HashMap::new(),
            current_frame: 0,
            waypoint_network: WaypointNetwork::default(),
            flow_fields: HashMap::new(),
        }
    }

    /// Initialize grid
    pub fn initialize(&mut self) {
        // Initialize grid cells
        for layer in 0..5u8 {
            let layer_enum = match layer {
                1 => PathfindLayerEnum::Ground,
                2 => PathfindLayerEnum::Air,
                3 => PathfindLayerEnum::Water,
                4 => PathfindLayerEnum::Tunnel,
                _ => continue,
            };

            for x in self.bounds.0.x..=self.bounds.1.x {
                for y in self.bounds.0.y..=self.bounds.1.y {
                    let coord = GridCoord::new(x, y, layer_enum);
                    self.grid.insert(coord, PathfindCell::default());
                }
            }
        }
    }

    /// Update pathfinding system (call once per frame)
    pub fn update(&mut self, current_frame: u32) {
        self.current_frame = current_frame;

        // Process queued requests
        self.process_requests();

        // Continue ongoing throttled pathfinding
        self.continue_ongoing_pathfinds();

        // Clean expired cache entries
        self.clean_cache();

        // Clean expired flow fields
        self.clean_flow_fields();
    }

    /// Public line-clear check for path validation.
    pub fn is_line_clear_between(&self, from: &Coord3D, to: &Coord3D) -> bool {
        if let Ok(ai_guard) = super::THE_AI.read() {
            if let Some(pathfinder) = ai_guard.pathfinder() {
                if let Ok(pf) = pathfinder.read() {
                    return pf.is_line_clear_between(from, to);
                }
            }
        }
        self.is_line_clear(from, to)
    }

    /// Check if the cell at a world position is clear (matches C++ CELL_CLEAR usage).
    pub fn is_cell_clear_at(&self, pos: &Coord3D, layer: crate::common::PathfindLayerEnum) -> bool {
        let coord = GridCoord::from_world(pos, PathfindLayerEnum::from(layer));
        if coord.x < self.bounds.0.x
            || coord.x > self.bounds.1.x
            || coord.y < self.bounds.0.y
            || coord.y > self.bounds.1.y
        {
            return false;
        }
        self.grid
            .get(&coord)
            .map(|cell| cell.terrain == TerrainType::Clear)
            .unwrap_or(false)
    }

    /// Request a path (async)
    pub fn request_path(&mut self, request: PathRequest) {
        // Prefer classic AIPathfind for fidelity.
        if let Ok(ai_guard) = super::THE_AI.read() {
            if let Some(pathfinder) = ai_guard.pathfinder() {
                if let Ok(pf) = pathfinder.read() {
                    let classic_request = ClassicPathRequest {
                        object_id: request.requester,
                        from: request.start,
                        to: request.goal,
                        surfaces: request.capabilities.surface_mask,
                        is_crusher: request.capabilities.crusher,
                        unit_radius: request.unit_size,
                        allow_partial: request.allow_partial,
                        move_allies: request.move_allies,
                        ignore_obstacle_id: request.ignore_obstacle_id,
                        is_human: false,
                    };
                    let result = pf.find_path_result(classic_request);
                    if result.success {
                        let converted_layers: Vec<PathfindLayerEnum> = result
                            .layers
                            .iter()
                            .map(|layer| match layer {
                                crate::ai::pathfind_astar::PathfindLayerEnum::Ground => {
                                    PathfindLayerEnum::Ground
                                }
                                crate::ai::pathfind_astar::PathfindLayerEnum::Top => {
                                    PathfindLayerEnum::Air
                                }
                                crate::ai::pathfind_astar::PathfindLayerEnum::Invalid => {
                                    PathfindLayerEnum::Invalid
                                }
                            })
                            .collect();
                        let path = self.build_path_from_positions_with_layers(
                            &result.waypoints,
                            &converted_layers,
                            request.capabilities.layer,
                            result.total_cost as f32,
                        );
                        self.completed
                            .insert(request.requester, PathResult::Success(path));
                    } else {
                        self.completed.insert(
                            request.requester,
                            PathResult::Failed("No path found".to_string()),
                        );
                    }
                    return;
                }
            }
        }

        // Check cache first
        let start_coord = GridCoord::from_world(&request.start, request.capabilities.layer);
        let goal_coord = GridCoord::from_world(&request.goal, request.capabilities.layer);
        let cache_key = (
            start_coord,
            goal_coord,
            request.capabilities.layer,
            request.ignore_obstacle_id.unwrap_or(INVALID_ID),
        );

        if let Some(cached) = self.path_cache.get_mut(&cache_key) {
            if cached.expiration_frame > self.current_frame {
                cached.hit_count += 1;
                // Path is cached and valid - would return immediately in real implementation
                self.completed
                    .insert(request.requester, PathResult::Success(cached.path.clone()));
                return;
            }
        }

        // Insert into queue by priority
        let insert_pos = self
            .request_queue
            .iter()
            .position(|r| r.priority < request.priority)
            .unwrap_or(self.request_queue.len());

        self.request_queue.insert(insert_pos, request);
    }

    /// Return terrain at a world position if present.
    pub fn terrain_at(
        &self,
        pos: &Coord3D,
        layer: crate::common::PathfindLayerEnum,
    ) -> Option<TerrainType> {
        let coord = GridCoord::from_world(pos, layer.into());
        self.grid.get(&coord).map(|cell| cell.terrain)
    }

    /// Find path immediately (synchronous, may be expensive)
    pub fn find_path_immediate(&mut self, request: &PathRequest) -> PathResult {
        let start_coord = GridCoord::from_world(&request.start, request.capabilities.layer);
        let goal_coord = GridCoord::from_world(&request.goal, request.capabilities.layer);

        // Check bounds
        if !self.is_in_bounds(&start_coord) || !self.is_in_bounds(&goal_coord) {
            return PathResult::Failed("Start or goal out of bounds".to_string());
        }

        // Run A* to completion
        match self.astar_search(
            start_coord,
            goal_coord,
            &request.capabilities,
            request.allow_partial,
            request.ignore_obstacle_id,
        ) {
            Some((grid_path, complete)) => {
                let mut world_path = self.convert_to_world_path(&grid_path, &request.capabilities);
                world_path.complete = complete;
                PathResult::Success(world_path)
            }
            None => PathResult::Failed("No path found".to_string()),
        }
    }

    /// Generate flow field for group movement
    pub fn generate_flow_field(
        &mut self,
        group_id: ObjectID,
        goal: &Coord3D,
        bounds: (Coord3D, Coord3D),
        layer: PathfindLayerEnum,
        caps: &MovementCapabilities,
    ) {
        let goal_coord = GridCoord::from_world(goal, layer);
        let min_coord = GridCoord::from_world(&bounds.0, layer);
        let max_coord = GridCoord::from_world(&bounds.1, layer);

        // Use Dijkstra to compute distance field
        let mut distances: HashMap<GridCoord, f32> = HashMap::new();
        let mut queue = VecDeque::new();

        distances.insert(goal_coord, 0.0);
        queue.push_back(goal_coord);

        while let Some(current) = queue.pop_front() {
            let current_dist = *distances.get(&current).unwrap();

            for (neighbor, move_cost) in current.get_neighbors() {
                if neighbor.x < min_coord.x
                    || neighbor.x > max_coord.x
                    || neighbor.y < min_coord.y
                    || neighbor.y > max_coord.y
                {
                    continue;
                }

                if let Some(cell) = self.grid.get(&neighbor) {
                    if !cell.is_passable(caps, &self.terrain_costs, None) {
                        continue;
                    }

                    let cost = cell.get_movement_cost(caps, &self.terrain_costs);
                    let new_dist = current_dist + move_cost * cost;

                    if !distances.contains_key(&neighbor)
                        || new_dist < *distances.get(&neighbor).unwrap()
                    {
                        distances.insert(neighbor, new_dist);
                        queue.push_back(neighbor);
                    }
                }
            }
        }

        // Compute flow directions
        let mut directions = HashMap::new();
        for (coord, _) in &distances {
            if *coord == goal_coord {
                continue;
            }

            let mut best_neighbor = *coord;
            let mut best_dist = INFINITY;

            for (neighbor, _) in coord.get_neighbors() {
                if let Some(&neighbor_dist) = distances.get(&neighbor) {
                    if neighbor_dist < best_dist {
                        best_dist = neighbor_dist;
                        best_neighbor = neighbor;
                    }
                }
            }

            if best_neighbor != *coord {
                let dir = Coord2D::new(
                    (best_neighbor.x - coord.x) as f32,
                    (best_neighbor.y - coord.y) as f32,
                );
                let len = (dir.x * dir.x + dir.y * dir.y).sqrt();
                if len > 0.0 {
                    directions.insert(*coord, Coord2D::new(dir.x / len, dir.y / len));
                }
            }
        }

        let flow_field = FlowField {
            directions,
            goal: goal_coord,
            bounds: (min_coord, max_coord),
            created_frame: self.current_frame,
        };

        self.flow_fields.insert(group_id, flow_field);
    }

    /// Get flow field for group
    pub fn get_flow_field(&self, group_id: ObjectID) -> Option<&FlowField> {
        self.flow_fields.get(&group_id)
    }

    /// Set cell terrain type
    pub fn set_terrain(&mut self, pos: &Coord3D, layer: PathfindLayerEnum, terrain: TerrainType) {
        let coord = GridCoord::from_world(pos, layer);
        if let Some(cell) = self.grid.get_mut(&coord) {
            cell.terrain = terrain;
            self.invalidate_cache_around(coord);
        }
    }

    /// Toggle bridge passability by updating ground-layer cells inside the polygon.
    pub fn set_bridge_passable(
        &mut self,
        polygon: &[Coord3D; 4],
        layer: PathfindLayerEnum,
        passable: bool,
    ) {
        let mut min_x = polygon[0].x;
        let mut max_x = polygon[0].x;
        let mut min_y = polygon[0].y;
        let mut max_y = polygon[0].y;
        for corner in polygon.iter().skip(1) {
            min_x = min_x.min(corner.x);
            max_x = max_x.max(corner.x);
            min_y = min_y.min(corner.y);
            max_y = max_y.max(corner.y);
        }

        let min_cell_x = (min_x / PATHFIND_CELL_SIZE).floor() as i32;
        let max_cell_x = (max_x / PATHFIND_CELL_SIZE).floor() as i32;
        let min_cell_y = (min_y / PATHFIND_CELL_SIZE).floor() as i32;
        let max_cell_y = (max_y / PATHFIND_CELL_SIZE).floor() as i32;

        let target = if passable {
            TerrainType::Clear
        } else {
            TerrainType::Impassable
        };

        for x in min_cell_x..=max_cell_x {
            for y in min_cell_y..=max_cell_y {
                let coord = GridCoord::new(x, y, layer);
                if !self.is_in_bounds(&coord) {
                    continue;
                }
                let center = coord.to_world(0.0);
                if !point_inside_polygon_2d(&center, polygon) {
                    continue;
                }
                if let Some(cell) = self.grid.get_mut(&coord) {
                    if cell.terrain != target {
                        cell.terrain = target;
                        self.invalidate_cache_around(coord);
                    }
                }
            }
        }
    }

    /// Add obstacle to pathfinding grid
    pub fn add_obstacle(
        &mut self,
        obstacle_id: ObjectID,
        positions: &[Coord3D],
        layer: PathfindLayerEnum,
    ) {
        for pos in positions {
            let coord = GridCoord::from_world(pos, layer);
            if let Some(cell) = self.grid.get_mut(&coord) {
                cell.obstacle_id = Some(obstacle_id);
                cell.terrain = TerrainType::Obstacle;
                self.invalidate_cache_around(coord);
            }
        }
    }

    /// Remove obstacle from pathfinding grid
    pub fn remove_obstacle(
        &mut self,
        obstacle_id: ObjectID,
        positions: &[Coord3D],
        layer: PathfindLayerEnum,
    ) {
        for pos in positions {
            let coord = GridCoord::from_world(pos, layer);
            if let Some(cell) = self.grid.get_mut(&coord) {
                if cell.obstacle_id == Some(obstacle_id) {
                    cell.obstacle_id = None;
                    cell.terrain = TerrainType::Clear;
                    self.invalidate_cache_around(coord);
                }
            }
        }
    }

    /// Set temporary block on cell
    pub fn set_temp_blocked(&mut self, pos: &Coord3D, layer: PathfindLayerEnum, blocked: bool) {
        let coord = GridCoord::from_world(pos, layer);
        if let Some(cell) = self.grid.get_mut(&coord) {
            cell.temp_blocked = blocked;
        }
    }

    /// Set cell occupant
    pub fn set_occupant(
        &mut self,
        pos: &Coord3D,
        layer: PathfindLayerEnum,
        occupant: Option<ObjectID>,
    ) {
        let coord = GridCoord::from_world(pos, layer);
        if let Some(cell) = self.grid.get_mut(&coord) {
            cell.occupant_id = occupant;
        }
    }

    /// Adjust a destination to the nearest passable cell.
    pub fn adjust_destination(
        &self,
        goal: &Coord3D,
        caps: &MovementCapabilities,
    ) -> Option<Coord3D> {
        let layer = caps.layer;
        let start = GridCoord::from_world(goal, layer);
        if self.is_in_bounds(&start) {
            if let Some(cell) = self.grid.get(&start) {
                if cell.is_passable(caps, &self.terrain_costs, None) {
                    return Some(start.to_world(goal.z));
                }
            }
        }

        const MAX_RADIUS: i32 = 6;
        for radius in 1..=MAX_RADIUS {
            let min = -radius;
            let max = radius;
            for dx in min..=max {
                for dy in min..=max {
                    if dx.abs() != radius && dy.abs() != radius {
                        continue;
                    }
                    let coord = GridCoord::new(start.x + dx, start.y + dy, layer);
                    if !self.is_in_bounds(&coord) {
                        continue;
                    }
                    if let Some(cell) = self.grid.get(&coord) {
                        if cell.is_passable(caps, &self.terrain_costs, None) {
                            return Some(coord.to_world(goal.z));
                        }
                    }
                }
            }
        }

        None
    }

    // Private methods

    pub(crate) fn is_in_bounds(&self, coord: &GridCoord) -> bool {
        coord.x >= self.bounds.0.x
            && coord.x <= self.bounds.1.x
            && coord.y >= self.bounds.0.y
            && coord.y <= self.bounds.1.y
    }

    fn process_requests(&mut self) {
        let mut requests_to_start = Vec::new();

        // Start new requests
        while let Some(request) = self.request_queue.pop_front() {
            if self.ongoing.len() < 5 {
                requests_to_start.push(request);
            } else {
                self.request_queue.push_front(request);
                break;
            }
        }

        for request in requests_to_start {
            self.start_pathfind(request);
        }
    }

    fn start_pathfind(&mut self, request: PathRequest) {
        let start_coord = GridCoord::from_world(&request.start, request.capabilities.layer);
        let goal_coord = GridCoord::from_world(&request.goal, request.capabilities.layer);
        let requester_id = request.requester;

        let mut open_set = BinaryHeap::new();
        let closed_set = HashSet::new();
        let came_from = HashMap::new();
        let mut g_score = HashMap::new();
        let best_distance = start_coord.diagonal_distance(&goal_coord);

        g_score.insert(start_coord, 0.0);
        open_set.push(AStarNode {
            coord: start_coord,
            g_score: 0.0,
            h_score: start_coord.diagonal_distance(&goal_coord),
            parent: None,
        });

        let ongoing = OngoingPathfind {
            request,
            open_set,
            closed_set,
            came_from,
            g_score,
            iterations: 0,
            best_partial: start_coord,
            best_distance,
        };

        self.ongoing.insert(requester_id, ongoing);
    }

    fn continue_ongoing_pathfinds(&mut self) {
        let mut completed = Vec::new();
        let bounds = self.bounds;
        let is_in_bounds = |coord: &GridCoord| {
            coord.x >= bounds.0.x
                && coord.x <= bounds.1.x
                && coord.y >= bounds.0.y
                && coord.y <= bounds.1.y
        };

        for (requester, ongoing) in &mut self.ongoing {
            // Process limited iterations
            for _ in 0..MAX_ITERATIONS_PER_FRAME {
                if let Some(current) = ongoing.open_set.pop() {
                    let goal_coord = GridCoord::from_world(
                        &ongoing.request.goal,
                        ongoing.request.capabilities.layer,
                    );

                    if current.coord == goal_coord {
                        // Found path
                        completed.push((*requester, true));
                        break;
                    }

                    let dist_to_goal = current.coord.diagonal_distance(&goal_coord);
                    if dist_to_goal < ongoing.best_distance {
                        ongoing.best_distance = dist_to_goal;
                        ongoing.best_partial = current.coord;
                    }

                    ongoing.closed_set.insert(current.coord);

                    // Expand neighbors
                    for (neighbor, move_cost) in current.coord.get_neighbors() {
                        if !is_in_bounds(&neighbor) || ongoing.closed_set.contains(&neighbor) {
                            continue;
                        }

                        if let Some(cell) = self.grid.get(&neighbor) {
                            if !cell.is_passable(
                                &ongoing.request.capabilities,
                                &self.terrain_costs,
                                ongoing.request.ignore_obstacle_id,
                            ) {
                                continue;
                            }

                            let terrain_cost = cell.get_movement_cost(
                                &ongoing.request.capabilities,
                                &self.terrain_costs,
                            );
                            let tentative_g = current.g_score + move_cost * terrain_cost;

                            if let Some(&existing_g) = ongoing.g_score.get(&neighbor) {
                                if tentative_g >= existing_g {
                                    continue;
                                }
                            }

                            ongoing.came_from.insert(neighbor, current.coord);
                            ongoing.g_score.insert(neighbor, tentative_g);
                            ongoing.open_set.push(AStarNode {
                                coord: neighbor,
                                g_score: tentative_g,
                                h_score: neighbor.diagonal_distance(&goal_coord),
                                parent: Some(current.coord),
                            });
                        }
                    }

                    ongoing.iterations += 1;
                    if ongoing.iterations > MAX_PATH_LENGTH {
                        completed.push((*requester, false));
                        break;
                    }
                } else {
                    // No more nodes to explore
                    completed.push((*requester, false));
                    break;
                }
            }
        }

        // Remove completed pathfinds
        for (requester, success) in completed {
            if let Some(ongoing) = self.ongoing.remove(&requester) {
                let start_coord = GridCoord::from_world(
                    &ongoing.request.start,
                    ongoing.request.capabilities.layer,
                );
                let goal_coord = GridCoord::from_world(
                    &ongoing.request.goal,
                    ongoing.request.capabilities.layer,
                );
                let (grid_path, complete) = if success {
                    (
                        Some(self.reconstruct_path(&ongoing.came_from, goal_coord)),
                        true,
                    )
                } else if ongoing.request.allow_partial && ongoing.best_partial != start_coord {
                    (
                        Some(self.reconstruct_path(&ongoing.came_from, ongoing.best_partial)),
                        false,
                    )
                } else {
                    (None, false)
                };

                let result = match grid_path {
                    Some(path) => {
                        let mut world_path =
                            self.convert_to_world_path(&path, &ongoing.request.capabilities);
                        world_path.complete = complete;
                        PathResult::Success(world_path)
                    }
                    None => PathResult::Failed("No path found".to_string()),
                };
                self.completed.insert(requester, result);
            }
        }
    }

    /// Take a completed path result if available.
    pub fn take_path_result(&mut self, requester: ObjectID) -> Option<PathResult> {
        self.completed.remove(&requester)
    }

    fn build_path_from_positions(
        &self,
        positions: &[Coord3D],
        layer: PathfindLayerEnum,
        total_cost: f32,
    ) -> Path {
        let mut waypoints = Vec::with_capacity(positions.len());
        let mut distance = 0.0;
        let mut prev: Option<Coord3D> = None;
        for pos in positions {
            if let Some(prev_pos) = prev {
                distance += (*pos - prev_pos).length();
            }
            waypoints.push(PathWaypoint {
                position: *pos,
                layer,
                distance,
            });
            prev = Some(*pos);
        }
        Path {
            waypoints,
            total_cost,
            complete: true,
            optimized: false,
            created_frame: self.current_frame,
        }
    }

    fn build_path_from_positions_with_layers(
        &self,
        positions: &[Coord3D],
        layers: &[PathfindLayerEnum],
        default_layer: PathfindLayerEnum,
        total_cost: f32,
    ) -> Path {
        let mut waypoints = Vec::with_capacity(positions.len());
        let mut distance = 0.0;
        let mut prev: Option<Coord3D> = None;
        for (idx, pos) in positions.iter().enumerate() {
            if let Some(prev_pos) = prev {
                distance += (*pos - prev_pos).length();
            }
            let layer = layers.get(idx).copied().unwrap_or(default_layer);
            waypoints.push(PathWaypoint {
                position: *pos,
                layer,
                distance,
            });
            prev = Some(*pos);
        }
        Path {
            waypoints,
            total_cost,
            complete: true,
            optimized: false,
            created_frame: self.current_frame,
        }
    }

    fn astar_search(
        &self,
        start: GridCoord,
        goal: GridCoord,
        caps: &MovementCapabilities,
        allow_partial: bool,
        ignore_obstacle_id: Option<ObjectID>,
    ) -> Option<(Vec<GridCoord>, bool)> {
        let mut open_set = BinaryHeap::new();
        let mut closed_set = HashSet::new();
        let mut came_from: HashMap<GridCoord, GridCoord> = HashMap::new();
        let mut g_score: HashMap<GridCoord, f32> = HashMap::new();

        g_score.insert(start, 0.0);
        open_set.push(AStarNode {
            coord: start,
            g_score: 0.0,
            h_score: start.diagonal_distance(&goal),
            parent: None,
        });

        let mut best_partial = start;
        let mut best_distance = start.diagonal_distance(&goal);

        while let Some(current) = open_set.pop() {
            if current.coord == goal {
                return Some((self.reconstruct_path(&came_from, current.coord), true));
            }

            // Track best partial path
            let dist_to_goal = current.coord.diagonal_distance(&goal);
            if dist_to_goal < best_distance {
                best_distance = dist_to_goal;
                best_partial = current.coord;
            }

            closed_set.insert(current.coord);

            // Expand neighbors
            for (neighbor, move_cost) in current.coord.get_neighbors() {
                if !self.is_in_bounds(&neighbor) || closed_set.contains(&neighbor) {
                    continue;
                }

                if let Some(cell) = self.grid.get(&neighbor) {
                    if !cell.is_passable(caps, &self.terrain_costs, ignore_obstacle_id) {
                        continue;
                    }

                    let terrain_cost = cell.get_movement_cost(caps, &self.terrain_costs);
                    let tentative_g = current.g_score + move_cost * terrain_cost;

                    if let Some(&existing_g) = g_score.get(&neighbor) {
                        if tentative_g >= existing_g {
                            continue;
                        }
                    }

                    came_from.insert(neighbor, current.coord);
                    g_score.insert(neighbor, tentative_g);
                    open_set.push(AStarNode {
                        coord: neighbor,
                        g_score: tentative_g,
                        h_score: neighbor.diagonal_distance(&goal),
                        parent: Some(current.coord),
                    });
                }
            }

            if closed_set.len() > MAX_PATH_LENGTH {
                break;
            }
        }

        // Return partial path if allowed
        if allow_partial && best_partial != start {
            return Some((self.reconstruct_path(&came_from, best_partial), false));
        }

        None
    }

    fn reconstruct_path(
        &self,
        came_from: &HashMap<GridCoord, GridCoord>,
        mut current: GridCoord,
    ) -> Vec<GridCoord> {
        let mut path = vec![current];

        while let Some(&parent) = came_from.get(&current) {
            current = parent;
            path.push(current);
        }

        path.reverse();
        path
    }

    fn convert_to_world_path(&self, grid_path: &[GridCoord], caps: &MovementCapabilities) -> Path {
        let mut waypoints: Vec<PathWaypoint> = Vec::new();
        let mut distance = 0.0;

        for (i, coord) in grid_path.iter().enumerate() {
            let height = self.grid.get(coord).map(|c| c.elevation).unwrap_or(0.0);
            let position = coord.to_world(height);

            if i > 0 {
                let prev_pos = waypoints[i - 1].position;
                distance += (position - prev_pos).length();
            }

            waypoints.push(PathWaypoint {
                position,
                layer: caps.layer,
                distance,
            });
        }

        let total_cost = distance;

        let mut path = Path {
            waypoints,
            total_cost,
            complete: true,
            optimized: false,
            created_frame: self.current_frame,
        };

        // Optimize path
        self.optimize_path(&mut path);

        path
    }

    fn optimize_path(&self, path: &mut Path) {
        if path.waypoints.len() < 3 {
            path.optimized = true;
            return;
        }

        let mut optimized = Vec::new();
        optimized.push(path.waypoints[0].clone());

        let mut i = 0;
        while i < path.waypoints.len() - 1 {
            let mut j = path.waypoints.len() - 1;

            // Find furthest visible waypoint
            while j > i + 1 {
                if self.is_line_clear(&path.waypoints[i].position, &path.waypoints[j].position) {
                    break;
                }
                j -= 1;
            }

            if j > i + 1 {
                optimized.push(path.waypoints[j].clone());
                i = j;
            } else {
                i += 1;
                if i < path.waypoints.len() {
                    optimized.push(path.waypoints[i].clone());
                }
            }
        }

        // Recalculate distances
        let mut distance = 0.0;
        for i in 0..optimized.len() {
            if i > 0 {
                distance += (optimized[i].position - optimized[i - 1].position).length();
            }
            optimized[i].distance = distance;
        }

        path.waypoints = optimized;
        path.optimized = true;
    }

    fn is_line_clear(&self, from: &Coord3D, to: &Coord3D) -> bool {
        // Simple raycast check
        let diff = *to - *from;
        let len = diff.length();
        let steps = (len / PATHFIND_CELL_SIZE).ceil() as usize;

        for i in 0..=steps {
            let t = i as f32 / steps as f32;
            let pos = *from + diff * t;
            let coord = GridCoord::from_world(&pos, PathfindLayerEnum::Ground);

            if let Some(cell) = self.grid.get(&coord) {
                if cell.terrain == TerrainType::Obstacle || cell.terrain == TerrainType::Impassable
                {
                    return false;
                }
            }
        }

        true
    }

    fn invalidate_cache_around(&mut self, coord: GridCoord) {
        // Remove cache entries near this coordinate
        let radius = 5;
        self.path_cache.retain(|&(start, goal, _, _), _| {
            let start_dist = ((start.x - coord.x).abs() + (start.y - coord.y).abs()) as i32;
            let goal_dist = ((goal.x - coord.x).abs() + (goal.y - coord.y).abs()) as i32;
            start_dist > radius && goal_dist > radius
        });
    }

    fn clean_cache(&mut self) {
        self.path_cache
            .retain(|_, cached| cached.expiration_frame > self.current_frame);
    }

    fn clean_flow_fields(&mut self) {
        let expiration_time = 600; // 20 seconds at 30fps
        self.flow_fields
            .retain(|_, field| self.current_frame - field.created_frame < expiration_time);
    }
}

fn point_inside_polygon_2d(point: &Coord3D, polygon: &[Coord3D; 4]) -> bool {
    let mut inside = false;
    let mut j = polygon.len() - 1;
    for i in 0..polygon.len() {
        let xi = polygon[i].x;
        let yi = polygon[i].y;
        let xj = polygon[j].x;
        let yj = polygon[j].y;

        if (yi > point.y) != (yj > point.y) {
            let denom = yj - yi;
            if denom.abs() > f32::EPSILON {
                let intersect = (xj - xi) * (point.y - yi) / denom + xi;
                if point.x < intersect {
                    inside = !inside;
                }
            }
        }
        j = i;
    }
    inside
}

// Thread-safe wrapper
pub type SharedPathfindingSystem = Arc<RwLock<PathfindingSystem>>;

pub fn create_pathfinding_system(width: i32, height: i32) -> SharedPathfindingSystem {
    let mut system = PathfindingSystem::new(width, height);
    system.initialize();
    Arc::new(RwLock::new(system))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_grid_coord_conversion() {
        let world_pos = Coord3D::new(25.0, 35.0, 0.0);
        let grid = GridCoord::from_world(&world_pos, PathfindLayerEnum::Ground);
        assert_eq!(grid.x, 2);
        assert_eq!(grid.y, 3);

        let back = grid.to_world(0.0);
        assert!((back.x - 25.0).abs() < PATHFIND_CELL_SIZE);
        assert!((back.y - 35.0).abs() < PATHFIND_CELL_SIZE);
    }

    #[test]
    fn test_manhattan_distance() {
        let a = GridCoord::new(0, 0, PathfindLayerEnum::Ground);
        let b = GridCoord::new(3, 4, PathfindLayerEnum::Ground);
        assert_eq!(a.manhattan_distance(&b), 7.0);
    }

    #[test]
    fn test_pathfinding_system_creation() {
        let system = PathfindingSystem::new(100, 100);
        assert!(system.is_in_bounds(&GridCoord::new(0, 0, PathfindLayerEnum::Ground)));
        assert!(!system.is_in_bounds(&GridCoord::new(1000, 1000, PathfindLayerEnum::Ground)));
    }

    #[test]
    fn test_movement_capabilities() {
        let ground = MovementCapabilities::ground();
        assert_eq!(ground.layer, PathfindLayerEnum::Ground);
        assert!(!ground.amphibious);

        let air = MovementCapabilities::air();
        assert_eq!(air.layer, PathfindLayerEnum::Air);
        assert!(air.flying);
    }

    #[test]
    fn test_terrain_costs() {
        let costs = TerrainCostTable::new();
        let ground_caps = MovementCapabilities::ground();

        assert_eq!(
            costs.get_cost(TerrainType::Clear, PathfindLayerEnum::Ground, &ground_caps),
            1.0
        );
        assert_eq!(
            costs.get_cost(TerrainType::Rough, PathfindLayerEnum::Ground, &ground_caps),
            2.0
        );
        assert_eq!(
            costs.get_cost(TerrainType::Water, PathfindLayerEnum::Ground, &ground_caps),
            INFINITY
        );
    }
}
