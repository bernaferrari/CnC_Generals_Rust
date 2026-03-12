//! Advanced Pathfinding System
//!
//! This module provides sophisticated pathfinding algorithms including A*,
//! hierarchical pathfinding, flow fields, and multi-threaded path planning.

use super::{Coord3D, ObjectId, Real};
use crate::{GameLogicError, GameLogicResult};

use std::cmp::{Ordering, Reverse};
use std::collections::{BinaryHeap, HashMap, HashSet, VecDeque};
use std::f32::INFINITY;
use std::sync::{Arc, Mutex, RwLock};

use petgraph::graph::NodeIndex;
use petgraph::{Graph, Undirected};
use rayon::prelude::*;

/// Pathfinding cell size in world units
pub const PATHFIND_CELL_SIZE: f32 = 10.0;

/// Maximum path length before giving up
pub const MAX_PATH_LENGTH: usize = 2048;

/// Movement capabilities for different unit types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MovementType {
    /// Ground units - can't cross water or cliffs
    Ground,
    /// Amphibious units - can cross water
    Amphibious,
    /// Aircraft - can fly over obstacles
    Air,
    /// Naval units - water only
    Naval,
    /// Hovering units - can cross most terrain
    Hover,
}

/// Terrain passability levels
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerrainType {
    /// Clear, flat terrain
    Clear = 0,
    /// Rough terrain with movement penalty
    Rough = 1,
    /// Very rough terrain
    VeryRough = 2,
    /// Impassable terrain
    Impassable = 3,
    /// Water
    Water = 4,
    /// Deep water
    DeepWater = 5,
    /// Cliff/steep slope
    Cliff = 6,
}

/// Pathfinding grid cell
#[derive(Debug, Clone)]
pub struct PathfindCell {
    /// Terrain type
    pub terrain: TerrainType,
    /// Movement cost multiplier
    pub cost_multiplier: f32,
    /// Whether cell is currently occupied
    pub occupied: bool,
    /// Occupying object ID (if any)
    pub occupier: Option<ObjectId>,
    /// Height/elevation
    pub elevation: f32,
    /// Temporary obstacles (dynamic)
    pub temp_blocked: bool,
}

/// 2D grid coordinate
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GridCoord {
    pub x: i32,
    pub y: i32,
}

/// Pathfinding request
#[derive(Debug, Clone)]
pub struct PathRequest {
    /// Requesting object ID
    pub requester: ObjectId,
    /// Start position in world coordinates
    pub start: Coord3D,
    /// Goal position in world coordinates
    pub goal: Coord3D,
    /// Movement type of the unit
    pub movement_type: MovementType,
    /// Unit size (radius in world units)
    pub unit_size: f32,
    /// Maximum acceptable path cost
    pub max_cost: f32,
    /// Whether to allow partial paths
    pub allow_partial: bool,
    /// Priority of this request
    pub priority: f32,
}

/// Pathfinding result
#[derive(Debug, Clone)]
pub struct PathResult {
    /// Whether path was found
    pub success: bool,
    /// Waypoints in world coordinates
    pub waypoints: Vec<Coord3D>,
    /// Total path cost
    pub total_cost: f32,
    /// Whether this is a partial path
    pub partial: bool,
    /// Reason for failure (if any)
    pub failure_reason: Option<String>,
}

/// A* pathfinding node
#[derive(Debug, Clone)]
struct AStarNode {
    /// Grid coordinate
    coord: GridCoord,
    /// Cost from start (g score)
    g_score: f32,
    /// Estimated cost to goal (h score)
    h_score: f32,
    /// Parent node for path reconstruction
    parent: Option<GridCoord>,
}

impl AStarNode {
    /// Total estimated cost (f score)
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
        // Reverse ordering for min-heap behavior
        other
            .f_score()
            .partial_cmp(&self.f_score())
            .unwrap_or(Ordering::Equal)
    }
}

/// Hierarchical pathfinding cluster
#[derive(Debug, Clone)]
struct PathfindCluster {
    /// Cluster ID
    id: u32,
    /// Top-left corner of cluster
    top_left: GridCoord,
    /// Cluster dimensions
    width: u32,
    height: u32,
    /// Entry/exit points to other clusters
    portals: Vec<Portal>,
    /// Internal pathfinding data
    internal_graph: Option<Graph<GridCoord, f32, Undirected>>,
}

/// Portal between clusters
#[derive(Debug, Clone)]
struct Portal {
    /// Portal position
    position: GridCoord,
    /// Connected cluster ID
    connected_cluster: u32,
    /// Movement cost through portal
    cost: f32,
}

/// Flow field for group movement
#[derive(Debug, Clone)]
pub struct FlowField {
    /// Flow directions for each cell
    directions: HashMap<GridCoord, Coord3D>,
    /// Goal position
    goal: GridCoord,
    /// Field bounds
    bounds: (GridCoord, GridCoord), // (top-left, bottom-right)
}

/// Advanced pathfinding system
pub struct PathfindingSystem {
    /// Pathfinding grid
    grid: Arc<RwLock<HashMap<GridCoord, PathfindCell>>>,
    /// Grid dimensions
    grid_bounds: (GridCoord, GridCoord),
    /// Hierarchical clusters
    clusters: Arc<RwLock<HashMap<u32, PathfindCluster>>>,
    /// Cluster size
    cluster_size: u32,
    /// Active path requests
    active_requests: Arc<Mutex<VecDeque<PathRequest>>>,
    /// Cached paths
    path_cache: Arc<RwLock<HashMap<(GridCoord, GridCoord, MovementType), PathResult>>>,
    /// Flow fields for group movement
    flow_fields: Arc<RwLock<HashMap<ObjectId, FlowField>>>,
}

impl PathfindingSystem {
    /// Create a new pathfinding system
    pub fn new(width: i32, height: i32, cluster_size: u32) -> Self {
        let grid_bounds = (
            GridCoord { x: 0, y: 0 },
            GridCoord {
                x: width - 1,
                y: height - 1,
            },
        );

        Self {
            grid: Arc::new(RwLock::new(HashMap::new())),
            grid_bounds,
            clusters: Arc::new(RwLock::new(HashMap::new())),
            cluster_size,
            active_requests: Arc::new(Mutex::new(VecDeque::new())),
            path_cache: Arc::new(RwLock::new(HashMap::new())),
            flow_fields: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Initialize the pathfinding system
    pub fn initialize(&self) -> GameLogicResult<()> {
        // Initialize grid with default values
        self.initialize_grid()?;

        // Build hierarchical clusters
        self.build_clusters()?;

        // Precompute inter-cluster connections
        self.precompute_cluster_connections()?;

        Ok(())
    }

    /// Find path using A* algorithm
    pub async fn find_path(&self, request: PathRequest) -> GameLogicResult<PathResult> {
        // Convert world coordinates to grid coordinates
        let start_grid = self.world_to_grid(&request.start);
        let goal_grid = self.world_to_grid(&request.goal);

        // Check cache first
        let cache_key = (start_grid, goal_grid, request.movement_type);
        if let Ok(cache) = self.path_cache.read() {
            if let Some(cached_result) = cache.get(&cache_key) {
                return Ok(cached_result.clone());
            }
        }

        // Determine pathfinding strategy based on distance
        let distance = self.manhattan_distance(&start_grid, &goal_grid);

        let result = if distance > (self.cluster_size * 3) as f32 {
            // Use hierarchical pathfinding for long distances
            self.hierarchical_pathfind(&request, start_grid, goal_grid)
                .await?
        } else {
            // Use direct A* for short distances
            self.astar_pathfind(&request, start_grid, goal_grid).await?
        };

        // Cache the result
        if let Ok(mut cache) = self.path_cache.write() {
            cache.insert(cache_key, result.clone());

            // Limit cache size
            if cache.len() > 1000 {
                cache.clear(); // Simple cache eviction
            }
        }

        Ok(result)
    }

    /// A* pathfinding implementation
    async fn astar_pathfind(
        &self,
        request: &PathRequest,
        start: GridCoord,
        goal: GridCoord,
    ) -> GameLogicResult<PathResult> {
        let mut open_set = BinaryHeap::new();
        let mut closed_set = HashSet::new();
        let mut came_from: HashMap<GridCoord, GridCoord> = HashMap::new();
        let mut g_score: HashMap<GridCoord, f32> = HashMap::new();

        // Initialize start node
        let start_node = AStarNode {
            coord: start,
            g_score: 0.0,
            h_score: self.manhattan_distance(&start, &goal),
            parent: None,
        };

        open_set.push(start_node);
        g_score.insert(start, 0.0);

        while let Some(current) = open_set.pop() {
            if current.coord == goal {
                // Path found - reconstruct
                let waypoints = self.reconstruct_path(&came_from, current.coord)?;
                return Ok(PathResult {
                    success: true,
                    waypoints: self.grid_path_to_world(&waypoints),
                    total_cost: current.g_score,
                    partial: false,
                    failure_reason: None,
                });
            }

            closed_set.insert(current.coord);

            // Examine neighbors
            for neighbor in self.get_neighbors(current.coord, request.movement_type)? {
                if closed_set.contains(&neighbor) {
                    continue;
                }

                let movement_cost = self.get_movement_cost(
                    current.coord,
                    neighbor,
                    request.movement_type,
                    request.unit_size,
                )?;

                if movement_cost == INFINITY {
                    continue; // Impassable
                }

                let tentative_g_score = current.g_score + movement_cost;

                if let Some(&existing_g) = g_score.get(&neighbor) {
                    if tentative_g_score >= existing_g {
                        continue; // Not a better path
                    }
                }

                // This is the best path so far
                came_from.insert(neighbor, current.coord);
                g_score.insert(neighbor, tentative_g_score);

                let neighbor_node = AStarNode {
                    coord: neighbor,
                    g_score: tentative_g_score,
                    h_score: self.manhattan_distance(&neighbor, &goal),
                    parent: Some(current.coord),
                };

                // Remove existing node with same coordinate if present
                // (BinaryHeap doesn't support efficient updates)
                open_set.push(neighbor_node);
            }

            // Limit search to prevent infinite loops
            if closed_set.len() > MAX_PATH_LENGTH {
                // Return partial path if allowed
                if request.allow_partial {
                    let best_node = closed_set
                        .iter()
                        .min_by(|a, b| {
                            self.manhattan_distance(a, &goal)
                                .partial_cmp(&self.manhattan_distance(b, &goal))
                                .unwrap_or(Ordering::Equal)
                        })
                        .cloned()
                        .unwrap_or(start);

                    let waypoints = self.reconstruct_path(&came_from, best_node)?;
                    return Ok(PathResult {
                        success: false,
                        waypoints: self.grid_path_to_world(&waypoints),
                        total_cost: g_score.get(&best_node).copied().unwrap_or(INFINITY),
                        partial: true,
                        failure_reason: Some("Path too long".to_string()),
                    });
                }

                return Ok(PathResult {
                    success: false,
                    waypoints: Vec::new(),
                    total_cost: INFINITY,
                    partial: false,
                    failure_reason: Some("Path too long".to_string()),
                });
            }
        }

        // No path found
        Ok(PathResult {
            success: false,
            waypoints: Vec::new(),
            total_cost: INFINITY,
            partial: false,
            failure_reason: Some("No path exists".to_string()),
        })
    }

    /// Hierarchical pathfinding for long distances
    async fn hierarchical_pathfind(
        &self,
        request: &PathRequest,
        start: GridCoord,
        goal: GridCoord,
    ) -> GameLogicResult<PathResult> {
        // Find clusters containing start and goal
        let start_cluster = self.get_cluster_for_coord(start)?;
        let goal_cluster = self.get_cluster_for_coord(goal)?;

        if start_cluster == goal_cluster {
            // Same cluster - use direct A*
            return self.astar_pathfind(request, start, goal).await;
        }

        // Find path between clusters
        let cluster_path = self.find_cluster_path(start_cluster, goal_cluster).await?;

        if cluster_path.is_empty() {
            return Ok(PathResult {
                success: false,
                waypoints: Vec::new(),
                total_cost: INFINITY,
                partial: false,
                failure_reason: Some("No cluster path found".to_string()),
            });
        }

        // Build detailed path through clusters
        let mut full_waypoints = Vec::new();
        let mut total_cost = 0.0;

        // Path from start to first portal
        let first_cluster_id = cluster_path[0];
        let first_portal = self.find_best_portal(start_cluster, first_cluster_id)?;

        let start_to_portal = self.astar_pathfind(request, start, first_portal).await?;
        if !start_to_portal.success {
            return Ok(PathResult {
                success: false,
                waypoints: Vec::new(),
                total_cost: INFINITY,
                partial: false,
                failure_reason: Some("Cannot reach first portal".to_string()),
            });
        }

        full_waypoints.extend(start_to_portal.waypoints);
        total_cost += start_to_portal.total_cost;

        // Path through intermediate clusters
        for i in 0..cluster_path.len() - 1 {
            let from_cluster = cluster_path[i];
            let to_cluster = cluster_path[i + 1];

            let from_portal = self.find_best_portal(from_cluster, to_cluster)?;
            let to_portal = self.find_best_portal(to_cluster, from_cluster)?;

            // Add inter-cluster movement cost
            total_cost += self.manhattan_distance(&from_portal, &to_portal) * PATHFIND_CELL_SIZE;

            full_waypoints.push(self.grid_to_world(to_portal));
        }

        // Path from last portal to goal
        let last_cluster_id = cluster_path[cluster_path.len() - 1];
        let last_portal = self.find_best_portal(last_cluster_id, goal_cluster)?;

        let portal_to_goal = self.astar_pathfind(request, last_portal, goal).await?;
        if !portal_to_goal.success {
            return Ok(PathResult {
                success: false,
                waypoints: full_waypoints, // Partial path
                total_cost: INFINITY,
                partial: true,
                failure_reason: Some("Cannot reach goal from last portal".to_string()),
            });
        }

        full_waypoints.extend(portal_to_goal.waypoints);
        total_cost += portal_to_goal.total_cost;

        Ok(PathResult {
            success: true,
            waypoints: full_waypoints,
            total_cost,
            partial: false,
            failure_reason: None,
        })
    }

    /// Generate flow field for group movement
    pub async fn generate_flow_field(
        &self,
        goal: Coord3D,
        bounds: (Coord3D, Coord3D),
        movement_type: MovementType,
    ) -> GameLogicResult<FlowField> {
        let goal_grid = self.world_to_grid(&goal);
        let bounds_grid = (self.world_to_grid(&bounds.0), self.world_to_grid(&bounds.1));

        let mut distances: HashMap<GridCoord, f32> = HashMap::new();
        let mut directions: HashMap<GridCoord, Coord3D> = HashMap::new();
        let mut queue: VecDeque<GridCoord> = VecDeque::new();

        // Initialize goal
        distances.insert(goal_grid, 0.0);
        queue.push_back(goal_grid);

        // Dijkstra's algorithm to compute distances
        while let Some(current) = queue.pop_front() {
            let current_dist = distances[&current];

            for neighbor in self.get_neighbors(current, movement_type)? {
                if neighbor.x < bounds_grid.0.x
                    || neighbor.x > bounds_grid.1.x
                    || neighbor.y < bounds_grid.0.y
                    || neighbor.y > bounds_grid.1.y
                {
                    continue; // Outside bounds
                }

                let move_cost = self.get_movement_cost(current, neighbor, movement_type, 1.0)?;
                if move_cost == INFINITY {
                    continue;
                }

                let new_dist = current_dist + move_cost;

                if !distances.contains_key(&neighbor) || new_dist < distances[&neighbor] {
                    distances.insert(neighbor, new_dist);
                    queue.push_back(neighbor);
                }
            }
        }

        // Compute flow directions
        for (coord, _) in &distances {
            if *coord == goal_grid {
                directions.insert(*coord, Coord3D::new(0.0, 0.0, 0.0)); // No flow at goal
                continue;
            }

            let mut best_neighbor = *coord;
            let mut best_distance = INFINITY;

            for neighbor in self.get_neighbors(*coord, movement_type)? {
                if let Some(&neighbor_dist) = distances.get(&neighbor) {
                    if neighbor_dist < best_distance {
                        best_distance = neighbor_dist;
                        best_neighbor = neighbor;
                    }
                }
            }

            if best_neighbor != *coord {
                let direction = Coord3D::new(
                    (best_neighbor.x - coord.x) as f32,
                    (best_neighbor.y - coord.y) as f32,
                    0.0,
                );
                directions.insert(*coord, direction);
            }
        }

        Ok(FlowField {
            directions,
            goal: goal_grid,
            bounds: bounds_grid,
        })
    }

    /// Multi-threaded batch pathfinding
    pub async fn batch_pathfind(
        &self,
        requests: Vec<PathRequest>,
    ) -> GameLogicResult<Vec<PathResult>> {
        // Process requests in parallel using rayon
        let results: Vec<_> = requests
            .into_par_iter()
            .map(|request| {
                // Create a blocking task for async pathfinding
                tokio::task::block_in_place(|| {
                    tokio::runtime::Handle::current()
                        .block_on(async { self.find_path(request).await })
                })
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(results)
    }

    /// Helper methods
    fn initialize_grid(&self) -> GameLogicResult<()> {
        let mut grid = self.grid.write().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire grid lock: {}", e))
        })?;

        for x in self.grid_bounds.0.x..=self.grid_bounds.1.x {
            for y in self.grid_bounds.0.y..=self.grid_bounds.1.y {
                let coord = GridCoord { x, y };
                grid.insert(
                    coord,
                    PathfindCell {
                        terrain: TerrainType::Clear,
                        cost_multiplier: 1.0,
                        occupied: false,
                        occupier: None,
                        elevation: 0.0,
                        temp_blocked: false,
                    },
                );
            }
        }

        Ok(())
    }

    fn build_clusters(&self) -> GameLogicResult<()> {
        let mut clusters = self.clusters.write().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire clusters lock: {}", e))
        })?;

        let mut cluster_id = 0;
        let cluster_size = self.cluster_size as i32;

        let mut x = self.grid_bounds.0.x;
        while x <= self.grid_bounds.1.x {
            let mut y = self.grid_bounds.0.y;
            while y <= self.grid_bounds.1.y {
                let cluster = PathfindCluster {
                    id: cluster_id,
                    top_left: GridCoord { x, y },
                    width: cluster_size.min(self.grid_bounds.1.x - x + 1) as u32,
                    height: cluster_size.min(self.grid_bounds.1.y - y + 1) as u32,
                    portals: Vec::new(),
                    internal_graph: None,
                };

                clusters.insert(cluster_id, cluster);
                cluster_id += 1;

                y += cluster_size;
            }
            x += cluster_size;
        }

        Ok(())
    }

    fn precompute_cluster_connections(&self) -> GameLogicResult<()> {
        let mut clusters = self.clusters.write().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire clusters lock: {}", e))
        })?;

        // Reset portals before rebuilding.
        for cluster in clusters.values_mut() {
            cluster.portals.clear();
        }

        let ids: Vec<u32> = clusters.keys().copied().collect();
        for i in 0..ids.len() {
            for j in (i + 1)..ids.len() {
                let a_id = ids[i];
                let b_id = ids[j];

                let (a, b) = match (clusters.get(&a_id), clusters.get(&b_id)) {
                    (Some(a), Some(b)) => (a.clone(), b.clone()),
                    _ => continue,
                };

                let a_left = a.top_left.x;
                let a_top = a.top_left.y;
                let a_right = a_left + a.width as i32 - 1;
                let a_bottom = a_top + a.height as i32 - 1;

                let b_left = b.top_left.x;
                let b_top = b.top_left.y;
                let b_right = b_left + b.width as i32 - 1;
                let b_bottom = b_top + b.height as i32 - 1;

                // Vertical adjacency (left/right neighbors).
                if a_right + 1 == b_left || b_right + 1 == a_left {
                    let overlap_top = a_top.max(b_top);
                    let overlap_bottom = a_bottom.min(b_bottom);
                    if overlap_top <= overlap_bottom {
                        let mid_y = overlap_top + (overlap_bottom - overlap_top) / 2;
                        let (a_portal, b_portal) = if a_right < b_left {
                            (
                                GridCoord {
                                    x: a_right,
                                    y: mid_y,
                                },
                                GridCoord {
                                    x: b_left,
                                    y: mid_y,
                                },
                            )
                        } else {
                            (
                                GridCoord {
                                    x: a_left,
                                    y: mid_y,
                                },
                                GridCoord {
                                    x: b_right,
                                    y: mid_y,
                                },
                            )
                        };

                        if let Some(cluster) = clusters.get_mut(&a_id) {
                            cluster.portals.push(Portal {
                                position: a_portal,
                                connected_cluster: b_id,
                                cost: 1.0,
                            });
                        }
                        if let Some(cluster) = clusters.get_mut(&b_id) {
                            cluster.portals.push(Portal {
                                position: b_portal,
                                connected_cluster: a_id,
                                cost: 1.0,
                            });
                        }
                    }
                }

                // Horizontal adjacency (top/bottom neighbors).
                if a_bottom + 1 == b_top || b_bottom + 1 == a_top {
                    let overlap_left = a_left.max(b_left);
                    let overlap_right = a_right.min(b_right);
                    if overlap_left <= overlap_right {
                        let mid_x = overlap_left + (overlap_right - overlap_left) / 2;
                        let (a_portal, b_portal) = if a_bottom < b_top {
                            (
                                GridCoord {
                                    x: mid_x,
                                    y: a_bottom,
                                },
                                GridCoord { x: mid_x, y: b_top },
                            )
                        } else {
                            (
                                GridCoord { x: mid_x, y: a_top },
                                GridCoord {
                                    x: mid_x,
                                    y: b_bottom,
                                },
                            )
                        };

                        if let Some(cluster) = clusters.get_mut(&a_id) {
                            cluster.portals.push(Portal {
                                position: a_portal,
                                connected_cluster: b_id,
                                cost: 1.0,
                            });
                        }
                        if let Some(cluster) = clusters.get_mut(&b_id) {
                            cluster.portals.push(Portal {
                                position: b_portal,
                                connected_cluster: a_id,
                                cost: 1.0,
                            });
                        }
                    }
                }
            }
        }

        Ok(())
    }

    fn world_to_grid(&self, world_pos: &Coord3D) -> GridCoord {
        GridCoord {
            x: (world_pos[0] / PATHFIND_CELL_SIZE).floor() as i32,
            y: (world_pos[1] / PATHFIND_CELL_SIZE).floor() as i32,
        }
    }

    fn grid_to_world(&self, grid_pos: GridCoord) -> Coord3D {
        Coord3D::new(
            grid_pos.x as f32 * PATHFIND_CELL_SIZE + PATHFIND_CELL_SIZE * 0.5,
            grid_pos.y as f32 * PATHFIND_CELL_SIZE + PATHFIND_CELL_SIZE * 0.5,
            0.0,
        )
    }

    fn grid_path_to_world(&self, grid_path: &[GridCoord]) -> Vec<Coord3D> {
        grid_path
            .iter()
            .map(|&coord| self.grid_to_world(coord))
            .collect()
    }

    fn manhattan_distance(&self, a: &GridCoord, b: &GridCoord) -> f32 {
        ((a.x - b.x).abs() + (a.y - b.y).abs()) as f32
    }

    fn get_neighbors(
        &self,
        coord: GridCoord,
        _movement_type: MovementType,
    ) -> GameLogicResult<Vec<GridCoord>> {
        let mut neighbors = Vec::new();

        // 8-directional movement
        for dx in -1..=1 {
            for dy in -1..=1 {
                if dx == 0 && dy == 0 {
                    continue;
                }

                let neighbor = GridCoord {
                    x: coord.x + dx,
                    y: coord.y + dy,
                };

                if neighbor.x >= self.grid_bounds.0.x
                    && neighbor.x <= self.grid_bounds.1.x
                    && neighbor.y >= self.grid_bounds.0.y
                    && neighbor.y <= self.grid_bounds.1.y
                {
                    neighbors.push(neighbor);
                }
            }
        }

        Ok(neighbors)
    }

    fn get_movement_cost(
        &self,
        _from: GridCoord,
        to: GridCoord,
        movement_type: MovementType,
        unit_size: f32,
    ) -> GameLogicResult<f32> {
        let grid = self.grid.read().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire grid lock: {}", e))
        })?;

        if let Some(cell) = grid.get(&to) {
            if cell.temp_blocked || (cell.occupied && unit_size > 0.5) {
                return Ok(INFINITY);
            }

            let base_cost = match movement_type {
                MovementType::Ground => match cell.terrain {
                    TerrainType::Clear => 1.0,
                    TerrainType::Rough => 2.0,
                    TerrainType::VeryRough => 4.0,
                    TerrainType::Water | TerrainType::DeepWater | TerrainType::Cliff => INFINITY,
                    TerrainType::Impassable => INFINITY,
                },
                MovementType::Amphibious => match cell.terrain {
                    TerrainType::Clear => 1.0,
                    TerrainType::Rough => 2.0,
                    TerrainType::VeryRough => 4.0,
                    TerrainType::Water => 1.5,
                    TerrainType::DeepWater => 2.0,
                    TerrainType::Cliff => INFINITY,
                    TerrainType::Impassable => INFINITY,
                },
                MovementType::Air => 1.0, // Aircraft ignore terrain
                MovementType::Naval => match cell.terrain {
                    TerrainType::Water => 1.0,
                    TerrainType::DeepWater => 1.0,
                    _ => INFINITY,
                },
                MovementType::Hover => match cell.terrain {
                    TerrainType::Clear => 1.0,
                    TerrainType::Rough => 1.2,
                    TerrainType::VeryRough => 1.5,
                    TerrainType::Water => 1.0,
                    TerrainType::DeepWater => 1.0,
                    TerrainType::Cliff => 3.0,
                    TerrainType::Impassable => INFINITY,
                },
            };

            Ok(base_cost * cell.cost_multiplier)
        } else {
            Ok(INFINITY)
        }
    }

    fn reconstruct_path(
        &self,
        came_from: &HashMap<GridCoord, GridCoord>,
        mut current: GridCoord,
    ) -> GameLogicResult<Vec<GridCoord>> {
        let mut path = vec![current];

        while let Some(&parent) = came_from.get(&current) {
            current = parent;
            path.push(current);
        }

        path.reverse();
        Ok(path)
    }

    fn get_cluster_for_coord(&self, coord: GridCoord) -> GameLogicResult<u32> {
        let cluster_size = self.cluster_size as i32;
        let cluster_x = coord.x / cluster_size;
        let cluster_y = coord.y / cluster_size;

        // Simple cluster ID calculation
        Ok((cluster_x
            * ((self.grid_bounds.1.y - self.grid_bounds.0.y + cluster_size) / cluster_size)
            + cluster_y) as u32)
    }

    async fn find_cluster_path(
        &self,
        start_cluster: u32,
        goal_cluster: u32,
    ) -> GameLogicResult<Vec<u32>> {
        if start_cluster == goal_cluster {
            return Ok(vec![start_cluster]);
        }

        let clusters = self.clusters.read().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire clusters lock: {}", e))
        })?;

        if !clusters.contains_key(&start_cluster) || !clusters.contains_key(&goal_cluster) {
            return Ok(Vec::new());
        }

        let mut visited: HashSet<u32> = HashSet::new();
        let mut came_from: HashMap<u32, u32> = HashMap::new();
        let mut open: VecDeque<u32> = VecDeque::new();

        visited.insert(start_cluster);
        open.push_back(start_cluster);

        while let Some(current) = open.pop_front() {
            if current == goal_cluster {
                break;
            }

            let Some(cluster) = clusters.get(&current) else {
                continue;
            };
            for portal in &cluster.portals {
                let neighbor = portal.connected_cluster;
                if visited.insert(neighbor) {
                    came_from.insert(neighbor, current);
                    open.push_back(neighbor);
                }
            }
        }

        if !visited.contains(&goal_cluster) {
            return Ok(Vec::new());
        }

        let mut path = vec![goal_cluster];
        let mut current = goal_cluster;
        while let Some(&parent) = came_from.get(&current) {
            current = parent;
            path.push(current);
            if current == start_cluster {
                break;
            }
        }
        path.reverse();
        Ok(path)
    }

    fn find_best_portal(&self, from_cluster: u32, to_cluster: u32) -> GameLogicResult<GridCoord> {
        let clusters = self.clusters.read().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire clusters lock: {}", e))
        })?;

        if let Some(cluster) = clusters.get(&from_cluster) {
            if let Some(portal) = cluster
                .portals
                .iter()
                .filter(|portal| portal.connected_cluster == to_cluster)
                .min_by(|a, b| a.cost.partial_cmp(&b.cost).unwrap_or(Ordering::Equal))
            {
                return Ok(portal.position);
            }

            // Fallback to cluster center when direct portal metadata is unavailable.
            Ok(GridCoord {
                x: cluster.top_left.x + (cluster.width / 2) as i32,
                y: cluster.top_left.y + (cluster.height / 2) as i32,
            })
        } else {
            Err(GameLogicError::Configuration(
                "Cluster not found".to_string(),
            ))
        }
    }

    /// Update cell occupancy
    pub fn set_cell_occupied(
        &self,
        world_pos: Coord3D,
        occupied: bool,
        occupier: Option<ObjectId>,
    ) -> GameLogicResult<()> {
        let grid_coord = self.world_to_grid(&world_pos);
        let mut grid = self.grid.write().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire grid lock: {}", e))
        })?;

        if let Some(cell) = grid.get_mut(&grid_coord) {
            cell.occupied = occupied;
            cell.occupier = occupier;
        }

        Ok(())
    }

    /// Set temporary obstacle
    pub fn set_temporary_obstacle(&self, world_pos: Coord3D, blocked: bool) -> GameLogicResult<()> {
        let grid_coord = self.world_to_grid(&world_pos);
        let mut grid = self.grid.write().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire grid lock: {}", e))
        })?;

        if let Some(cell) = grid.get_mut(&grid_coord) {
            cell.temp_blocked = blocked;
        }

        Ok(())
    }

    /// Clear path cache
    pub fn clear_cache(&self) -> GameLogicResult<()> {
        let mut cache = self.path_cache.write().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire cache lock: {}", e))
        })?;
        cache.clear();
        Ok(())
    }
}

impl FlowField {
    /// Get flow direction at world position
    pub fn get_flow_direction(&self, world_pos: Coord3D) -> Option<Coord3D> {
        let grid_coord = GridCoord {
            x: (world_pos[0] / PATHFIND_CELL_SIZE).floor() as i32,
            y: (world_pos[1] / PATHFIND_CELL_SIZE).floor() as i32,
        };

        self.directions.get(&grid_coord).copied()
    }

    /// Check if position is within field bounds
    pub fn contains_position(&self, world_pos: Coord3D) -> bool {
        let grid_coord = GridCoord {
            x: (world_pos[0] / PATHFIND_CELL_SIZE).floor() as i32,
            y: (world_pos[1] / PATHFIND_CELL_SIZE).floor() as i32,
        };

        grid_coord.x >= self.bounds.0.x
            && grid_coord.x <= self.bounds.1.x
            && grid_coord.y >= self.bounds.0.y
            && grid_coord.y <= self.bounds.1.y
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_pathfinding_system_creation() {
        let pathfinding = PathfindingSystem::new(100, 100, 10);
        assert!(pathfinding.initialize().is_ok());
    }

    #[tokio::test]
    async fn test_simple_pathfinding() {
        let pathfinding = PathfindingSystem::new(100, 100, 10);
        pathfinding.initialize().unwrap();

        let request = PathRequest {
            requester: 1,
            start: [0.0, 0.0, 0.0].into(),
            goal: [100.0, 100.0, 0.0].into(),
            movement_type: MovementType::Ground,
            unit_size: 1.0,
            max_cost: 1000.0,
            allow_partial: true,
            priority: 1.0,
        };

        let result = pathfinding.find_path(request).await.unwrap();
        assert!(result.waypoints.len() > 0);
    }

    #[tokio::test]
    async fn test_flow_field_generation() {
        let pathfinding = PathfindingSystem::new(50, 50, 10);
        pathfinding.initialize().unwrap();

        let flow_field = pathfinding
            .generate_flow_field(
                [250.0, 250.0, 0.0].into(),
                ([0.0, 0.0, 0.0].into(), [500.0, 500.0, 0.0].into()),
                MovementType::Ground,
            )
            .await
            .unwrap();

        assert!(!flow_field.directions.is_empty());
    }

    #[test]
    fn test_coordinate_conversion() {
        let pathfinding = PathfindingSystem::new(100, 100, 10);

        let world_pos: Coord3D = [55.0, 37.0, 0.0].into();
        let grid_coord = pathfinding.world_to_grid(&world_pos);
        let back_to_world = pathfinding.grid_to_world(grid_coord);

        assert_eq!(grid_coord.x, 5);
        assert_eq!(grid_coord.y, 3);
        assert!((back_to_world[0] - 55.0).abs() < PATHFIND_CELL_SIZE);
    }
}
