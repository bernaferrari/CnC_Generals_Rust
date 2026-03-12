#![allow(deprecated)]

//! Modern pathfinding system using A* algorithm
//! 
//! This module provides a modern Rust implementation of pathfinding using the
//! `pathfinding` crate's A* algorithm, replacing the original C++ pathfinding
//! implementation with a more efficient and maintainable solution.
//!
//! Author: Converted from C++ by Claude, original by Michael S. Booth, October 2001

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, RwLock};
use pathfinding::prelude::*;
use crate::common::types::{Real, Coord3D, Coord2D};
use crate::common::ObjectID;
use crate::path::PATHFIND_CELL_SIZE_F;

// Re-export some types from the existing pathfind module for compatibility
pub use super::pathfind::{PathNode, PathfindLayerEnum, PATHFIND_CLOSE_ENOUGH};

/// Cell size for pathfinding grid (matches path::PATHFIND_CELL_SIZE_F)
pub const PATHFIND_CELL_SIZE: f32 = PATHFIND_CELL_SIZE_F;

/// Maximum priority for pathfinding queue
pub const PATH_MAX_PRIORITY: u32 = u32::MAX;

/// Grid coordinate for pathfinding
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GridCoord {
    pub x: i32,
    pub y: i32,
    pub layer: u8, // PathfindLayerEnum as u8
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

    /// Convert grid coordinates to world coordinates (center of cell)
    pub fn to_world(&self) -> Coord3D {
        Coord3D::new(
            (self.x as f32 + 0.5) * PATHFIND_CELL_SIZE,
            (self.y as f32 + 0.5) * PATHFIND_CELL_SIZE,
            0.0, // Z will be determined by terrain/layer
        )
    }

    /// Manhattan distance to another grid coordinate
    pub fn manhattan_distance(&self, other: &GridCoord) -> u32 {
        if self.layer != other.layer {
            // Cross-layer movement has additional cost
            return ((self.x - other.x).abs() + (self.y - other.y).abs()) as u32 + 50;
        }
        ((self.x - other.x).abs() + (self.y - other.y).abs()) as u32
    }

    /// Get neighbors for A* pathfinding
    pub fn get_neighbors(&self) -> Vec<(GridCoord, u32)> {
        let mut neighbors = Vec::with_capacity(8);
        
        // 8-directional movement
        let directions = [
            (-1, -1, 14), (-1, 0, 10), (-1, 1, 14),
            (0, -1, 10),               (0, 1, 10),
            (1, -1, 14),  (1, 0, 10),  (1, 1, 14),
        ];
        
        for (dx, dy, cost) in directions {
            let neighbor = GridCoord {
                x: self.x + dx,
                y: self.y + dy,
                layer: self.layer,
            };
            neighbors.push((neighbor, cost));
        }
        
        neighbors
    }
}

/// Cell type for pathfinding
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CellType {
    Clear = 0,
    Water = 1,
    Cliff = 2,
    Rubble = 3,
    Obstacle = 4,
    BridgeImpassable = 5,
    Impassable = 6,
}

impl Default for CellType {
    fn default() -> Self {
        CellType::Clear
    }
}

/// Cell flags for unit presence
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CellFlags {
    NoUnits = 0,
    UnitGoal = 1,
    UnitPresentMoving = 2,
    UnitPresentFixed = 3,
    UnitGoalOtherMoving = 5,
}

impl Default for CellFlags {
    fn default() -> Self {
        CellFlags::NoUnits
    }
}

/// Pathfinding cell information
#[derive(Debug, Clone)]
pub struct PathfindCell {
    /// Cell type (terrain)
    cell_type: CellType,
    /// Unit presence flags
    flags: CellFlags,
    /// Movement cost modifier
    movement_cost: u32,
    /// Object occupying this cell
    obstacle_id: Option<ObjectID>,
    /// Whether obstacle is a fence
    obstacle_is_fence: bool,
    /// Whether obstacle is transparent
    obstacle_is_transparent: bool,
    /// Whether cell is blocked by ally
    blocked_by_ally: bool,
    /// Zone ID for hierarchical pathfinding
    zone: u16,
}

impl Default for PathfindCell {
    fn default() -> Self {
        Self {
            cell_type: CellType::Clear,
            flags: CellFlags::NoUnits,
            movement_cost: 10, // Base movement cost
            obstacle_id: None,
            obstacle_is_fence: false,
            obstacle_is_transparent: false,
            blocked_by_ally: false,
            zone: 0,
        }
    }
}

impl PathfindCell {
    /// Check if cell is passable for given locomotor surface types
    pub fn is_passable(&self, acceptable_surfaces: u32, is_crusher: bool) -> bool {
        match self.cell_type {
            CellType::Clear => true,
            CellType::Water => acceptable_surfaces & (1 << 1) != 0, // SURFACE_WATER
            CellType::Cliff => acceptable_surfaces & (1 << 2) != 0, // SURFACE_CLIFF
            CellType::Rubble => is_crusher || acceptable_surfaces & (1 << 3) != 0,
            CellType::Obstacle => false,
            CellType::BridgeImpassable => false,
            CellType::Impassable => false,
        }
    }

    /// Get movement cost for this cell
    pub fn get_movement_cost(&self, is_crusher: bool) -> u32 {
        let base_cost = match self.cell_type {
            CellType::Clear => 10,
            CellType::Water => 15,
            CellType::Cliff => 20,
            CellType::Rubble => if is_crusher { 12 } else { 25 },
            _ => u32::MAX, // Impassable
        };

        if self.blocked_by_ally {
            base_cost + 5 // Additional cost for ally-blocked cells
        } else {
            base_cost
        }
    }

    /// Set cell as obstacle
    pub fn set_as_obstacle(&mut self, obstacle_id: ObjectID, is_fence: bool) {
        self.cell_type = CellType::Obstacle;
        self.obstacle_id = Some(obstacle_id);
        self.obstacle_is_fence = is_fence;
    }

    /// Remove obstacle from cell
    pub fn remove_obstacle(&mut self, obstacle_id: ObjectID) -> bool {
        if self.obstacle_id == Some(obstacle_id) {
            self.cell_type = CellType::Clear;
            self.obstacle_id = None;
            self.obstacle_is_fence = false;
            self.obstacle_is_transparent = false;
            true
        } else {
            false
        }
    }
}

/// Modern pathfinding grid using A* algorithm
#[derive(Debug)]
pub struct ModernPathfinder {
    /// 2D grid of pathfinding cells
    grid: HashMap<GridCoord, PathfindCell>,
    /// Grid dimensions
    width: i32,
    height: i32,
    /// Grid origin offset
    origin_x: i32,
    origin_y: i32,
    /// Pathfinding request queue
    request_queue: VecDeque<PathfindRequest>,
    /// Maximum requests processed per frame
    max_requests_per_frame: usize,
}

/// Pathfinding request
#[derive(Debug)]
pub struct PathfindRequest {
    pub requester_id: ObjectID,
    pub from: GridCoord,
    pub to: GridCoord,
    pub acceptable_surfaces: u32,
    pub is_crusher: bool,
    pub priority: u32,
}

/// Pathfinding result
#[derive(Debug)]
pub struct PathfindResult {
    pub path: Option<Vec<GridCoord>>,
    pub total_cost: u32,
    pub success: bool,
}

impl ModernPathfinder {
    /// Create new pathfinder with given dimensions
    pub fn new(width: i32, height: i32) -> Self {
        let mut pathfinder = Self {
            grid: HashMap::new(),
            width,
            height,
            origin_x: -width / 2,
            origin_y: -height / 2,
            request_queue: VecDeque::new(),
            max_requests_per_frame: 5, // Limit to prevent frame rate drops
        };
        pathfinder.reset();
        pathfinder
    }

    /// Initialize the pathfinding grid
    pub fn reset(&mut self) {
        self.grid.clear();
        self.request_queue.clear();
        
        // Initialize grid with default cells
        for x in self.origin_x..(self.origin_x + self.width) {
            for y in self.origin_y..(self.origin_y + self.height) {
                for layer in 0..4u8 { // Assuming 4 layers maximum
                    let coord = GridCoord { x, y, layer };
                    self.grid.insert(coord, PathfindCell::default());
                }
            }
        }
    }

    /// Get cell at grid coordinates
    pub fn get_cell(&self, coord: &GridCoord) -> Option<&PathfindCell> {
        self.grid.get(coord)
    }

    /// Get mutable cell at grid coordinates
    pub fn get_cell_mut(&mut self, coord: &GridCoord) -> Option<&mut PathfindCell> {
        self.grid.get_mut(coord)
    }

    /// Check if coordinates are within grid bounds
    pub fn is_valid_coord(&self, coord: &GridCoord) -> bool {
        coord.x >= self.origin_x && coord.x < self.origin_x + self.width &&
        coord.y >= self.origin_y && coord.y < self.origin_y + self.height
    }

    /// Add pathfinding request to queue
    pub fn queue_pathfind_request(&mut self, request: PathfindRequest) {
        // Insert in priority order (higher priority first)
        let insert_pos = self.request_queue
            .iter()
            .position(|r| r.priority < request.priority)
            .unwrap_or(self.request_queue.len());
        
        self.request_queue.insert(insert_pos, request);
    }

    /// Process pathfinding requests from the queue
    pub fn process_pathfind_queue(&mut self) -> Result<Vec<(ObjectID, PathfindResult)>, String> {
        let mut results = Vec::new();
        let requests_to_process = self.max_requests_per_frame.min(self.request_queue.len());
        
        for _ in 0..requests_to_process {
            if let Some(request) = self.request_queue.pop_front() {
                let result = self.find_path_internal(&request);
                results.push((request.requester_id, result));
            }
        }
        
        Ok(results)
    }

    /// Find path using A* algorithm
    pub fn find_path(
        &self,
        from: &Coord3D,
        to: &Coord3D,
        layer: PathfindLayerEnum,
        acceptable_surfaces: u32,
        is_crusher: bool,
    ) -> PathfindResult {
        let start = GridCoord::from_world(from, layer);
        let goal = GridCoord::from_world(to, layer);
        
        let request = PathfindRequest {
            requester_id: 0, // Immediate request
            from: start,
            to: goal,
            acceptable_surfaces,
            is_crusher,
            priority: PATH_MAX_PRIORITY,
        };
        
        self.find_path_internal(&request)
    }

    /// Internal pathfinding implementation using A*
    fn find_path_internal(&self, request: &PathfindRequest) -> PathfindResult {
        let start = request.from;
        let goal = request.to;
        
        // Use A* algorithm from pathfinding crate
        let result = astar(
            &start,
            |coord| self.get_successors(coord, request.acceptable_surfaces, request.is_crusher),
            |coord| coord.manhattan_distance(&goal),
            |coord| *coord == goal,
        );
        
        match result {
            Some((path, cost)) => PathfindResult {
                path: Some(path),
                total_cost: cost,
                success: true,
            },
            None => PathfindResult {
                path: None,
                total_cost: 0,
                success: false,
            },
        }
    }

    /// Get valid successors for A* algorithm
    fn get_successors(&self, coord: &GridCoord, acceptable_surfaces: u32, is_crusher: bool) -> Vec<(GridCoord, u32)> {
        let mut successors = Vec::new();
        
        for (neighbor, base_cost) in coord.get_neighbors() {
            if !self.is_valid_coord(&neighbor) {
                continue;
            }
            
            if let Some(cell) = self.get_cell(&neighbor) {
                if cell.is_passable(acceptable_surfaces, is_crusher) {
                    let movement_cost = cell.get_movement_cost(is_crusher);
                    let total_cost = base_cost + movement_cost;
                    successors.push((neighbor, total_cost));
                }
            }
        }
        
        successors
    }

    /// Convert grid path to world coordinates
    pub fn grid_path_to_world_path(&self, grid_path: &[GridCoord]) -> Vec<Coord3D> {
        grid_path.iter()
            .map(|coord| coord.to_world())
            .collect()
    }

    /// Add object to pathfinding map as obstacle
    pub fn add_object_to_map(&mut self, object_id: ObjectID, positions: &[Coord3D], is_fence: bool) {
        for pos in positions {
            let coord = GridCoord::from_world(pos, PathfindLayerEnum::Ground);
            if let Some(cell) = self.get_cell_mut(&coord) {
                cell.set_as_obstacle(object_id, is_fence);
            }
        }
    }

    /// Remove object from pathfinding map
    pub fn remove_object_from_map(&mut self, object_id: ObjectID, positions: &[Coord3D]) {
        for pos in positions {
            let coord = GridCoord::from_world(pos, PathfindLayerEnum::Ground);
            if let Some(cell) = self.get_cell_mut(&coord) {
                cell.remove_obstacle(object_id);
            }
        }
    }

    /// Check if path exists between two points (quick check without full pathfinding)
    pub fn does_path_exist(&self, from: &Coord3D, to: &Coord3D, acceptable_surfaces: u32) -> bool {
        let start = GridCoord::from_world(from, PathfindLayerEnum::Ground);
        let goal = GridCoord::from_world(to, PathfindLayerEnum::Ground);
        
        // Use Dijkstra for quick reachability check (stops at goal)
        let result = dijkstra(
            &start,
            |coord| self.get_successors(coord, acceptable_surfaces, false),
            |coord| *coord == goal,
        );
        
        result.is_some()
    }

    /// Optimize path by removing unnecessary waypoints
    pub fn optimize_path(&self, path: &[GridCoord]) -> Vec<GridCoord> {
        if path.len() < 3 {
            return path.to_vec();
        }
        
        let mut optimized = Vec::new();
        optimized.push(path[0]);
        
        let mut i = 0;
        while i < path.len() - 1 {
            let mut j = path.len() - 1;
            
            // Find the furthest point we can reach directly
            while j > i + 1 {
                if self.is_line_clear(&path[i], &path[j]) {
                    break;
                }
                j -= 1;
            }
            
            if j > i + 1 {
                optimized.push(path[j]);
                i = j;
            } else {
                i += 1;
                if i < path.len() {
                    optimized.push(path[i]);
                }
            }
        }
        
        optimized
    }

    /// Check if line between two points is clear
    fn is_line_clear(&self, from: &GridCoord, to: &GridCoord) -> bool {
        // Simple Bresenham-like line check
        let dx = (to.x - from.x).abs();
        let dy = (to.y - from.y).abs();
        let sx = if from.x < to.x { 1 } else { -1 };
        let sy = if from.y < to.y { 1 } else { -1 };
        let mut err = dx - dy;
        
        let mut x = from.x;
        let mut y = from.y;
        
        loop {
            let coord = GridCoord { x, y, layer: from.layer };
            
            if let Some(cell) = self.get_cell(&coord) {
                if !cell.is_passable(u32::MAX, false) { // Check all surfaces
                    return false;
                }
            } else {
                return false; // Out of bounds
            }
            
            if x == to.x && y == to.y {
                break;
            }
            
            let e2 = 2 * err;
            if e2 > -dy {
                err -= dy;
                x += sx;
            }
            if e2 < dx {
                err += dx;
                y += sy;
            }
        }
        
        true
    }

    /// Update goal position for unit
    pub fn update_goal(&mut self, _unit_id: ObjectID, new_goal: &Coord3D, layer: PathfindLayerEnum) {
        let coord = GridCoord::from_world(new_goal, layer);
        if let Some(cell) = self.get_cell_mut(&coord) {
            cell.flags = CellFlags::UnitGoal;
        }
    }

    /// Remove unit goal from pathfinding map
    pub fn remove_goal(&mut self, old_goal: &Coord3D, layer: PathfindLayerEnum) {
        let coord = GridCoord::from_world(old_goal, layer);
        if let Some(cell) = self.get_cell_mut(&coord) {
            if cell.flags == CellFlags::UnitGoal {
                cell.flags = CellFlags::NoUnits;
            }
        }
    }

    /// Get statistics about the pathfinder
    pub fn get_stats(&self) -> PathfindingStats {
        let total_cells = self.grid.len();
        let obstacle_cells = self.grid.values()
            .filter(|cell| cell.cell_type == CellType::Obstacle)
            .count();
        
        PathfindingStats {
            total_cells,
            obstacle_cells,
            queued_requests: self.request_queue.len(),
            grid_width: self.width,
            grid_height: self.height,
        }
    }
}

/// Pathfinding statistics
#[derive(Debug)]
pub struct PathfindingStats {
    pub total_cells: usize,
    pub obstacle_cells: usize,
    pub queued_requests: usize,
    pub grid_width: i32,
    pub grid_height: i32,
}

/// Thread-safe wrapper for the pathfinder
pub type SharedPathfinder = Arc<RwLock<ModernPathfinder>>;

/// Create a shared pathfinder instance
pub fn create_shared_pathfinder(width: i32, height: i32) -> SharedPathfinder {
    Arc::new(RwLock::new(ModernPathfinder::new(width, height)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_grid_coord_conversion() {
        let world_pos = Coord3D::new(25.0, 35.0, 0.0);
        let grid_coord = GridCoord::from_world(&world_pos, PathfindLayerEnum::Ground);
        
        assert_eq!(grid_coord.x, 2); // 25.0 / 10.0 = 2.5, floor = 2
        assert_eq!(grid_coord.y, 3); // 35.0 / 10.0 = 3.5, floor = 3
        
        let back_to_world = grid_coord.to_world();
        assert_eq!(back_to_world.x, 25.0); // (2 + 0.5) * 10 = 25
        assert_eq!(back_to_world.y, 35.0); // (3 + 0.5) * 10 = 35
    }

    #[test]
    fn test_manhattan_distance() {
        let coord1 = GridCoord::new(0, 0, PathfindLayerEnum::Ground);
        let coord2 = GridCoord::new(3, 4, PathfindLayerEnum::Ground);
        
        assert_eq!(coord1.manhattan_distance(&coord2), 7); // |0-3| + |0-4| = 7
    }

    #[test]
    fn test_pathfinder_creation() {
        let pathfinder = ModernPathfinder::new(100, 100);
        assert_eq!(pathfinder.width, 100);
        assert_eq!(pathfinder.height, 100);
        assert_eq!(pathfinder.origin_x, -50);
        assert_eq!(pathfinder.origin_y, -50);
    }

    #[test]
    fn test_cell_passability() {
        let mut cell = PathfindCell::default();
        assert!(cell.is_passable(u32::MAX, false)); // Clear cell is passable
        
        cell.cell_type = CellType::Obstacle;
        assert!(!cell.is_passable(u32::MAX, false)); // Obstacle is not passable
        
        cell.cell_type = CellType::Rubble;
        assert!(!cell.is_passable(1, false)); // Rubble not passable for non-crusher
        assert!(cell.is_passable(1, true)); // Rubble passable for crusher
    }

    #[test]
    fn test_pathfinding_simple() {
        let pathfinder = ModernPathfinder::new(10, 10);
        
        let from = Coord3D::new(0.0, 0.0, 0.0);
        let to = Coord3D::new(30.0, 40.0, 0.0);
        
        let result = pathfinder.find_path(
            &from, 
            &to, 
            PathfindLayerEnum::Ground,
            u32::MAX, 
            false
        );
        
        // Should find a path in an empty grid
        assert!(result.success);
        assert!(result.path.is_some());
    }

    #[test]
    fn test_obstacle_management() {
        let mut pathfinder = ModernPathfinder::new(10, 10);
        pathfinder.reset();
        
        let obstacle_pos = vec![Coord3D::new(15.0, 15.0, 0.0)];
        pathfinder.add_object_to_map(123, &obstacle_pos, false);
        
        let coord = GridCoord::from_world(&obstacle_pos[0], PathfindLayerEnum::Ground);
        let cell = pathfinder.get_cell(&coord).unwrap();
        assert_eq!(cell.cell_type, CellType::Obstacle);
        assert_eq!(cell.obstacle_id, Some(123));
        
        pathfinder.remove_object_from_map(123, &obstacle_pos);
        let cell = pathfinder.get_cell(&coord).unwrap();
        assert_eq!(cell.cell_type, CellType::Clear);
        assert_eq!(cell.obstacle_id, None);
    }
}
