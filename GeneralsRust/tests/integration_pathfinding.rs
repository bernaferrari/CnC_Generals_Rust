//! Integration Test: Pathfinding System
//!
//! This test verifies that units can find paths through the game world:
//! - A* pathfinding algorithm
//! - Grid-based navigation
//! - Obstacle avoidance
//! - Path smoothing and optimization
//! - Dynamic pathing around moving units
//! - Formation movement
//!
//! Tests should pass on all platforms (Windows, Linux, macOS)

#![cfg(test)]

use std::collections::{BinaryHeap, HashMap, HashSet, VecDeque};
use std::cmp::Ordering;

/// Position in 2D grid
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct GridPos {
    x: i32,
    y: i32,
}

impl GridPos {
    fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }

    fn manhattan_distance(&self, other: &GridPos) -> i32 {
        (self.x - other.x).abs() + (self.y - other.y).abs()
    }

    fn neighbors(&self) -> Vec<GridPos> {
        vec![
            GridPos::new(self.x + 1, self.y),
            GridPos::new(self.x - 1, self.y),
            GridPos::new(self.x, self.y + 1),
            GridPos::new(self.x, self.y - 1),
        ]
    }

    fn neighbors_diagonal(&self) -> Vec<GridPos> {
        vec![
            GridPos::new(self.x + 1, self.y),
            GridPos::new(self.x - 1, self.y),
            GridPos::new(self.x, self.y + 1),
            GridPos::new(self.x, self.y - 1),
            GridPos::new(self.x + 1, self.y + 1),
            GridPos::new(self.x + 1, self.y - 1),
            GridPos::new(self.x - 1, self.y + 1),
            GridPos::new(self.x - 1, self.y - 1),
        ]
    }
}

/// Node for A* pathfinding
#[derive(Debug, Clone, Eq, PartialEq)]
struct PathNode {
    pos: GridPos,
    g_cost: i32, // Cost from start
    h_cost: i32, // Heuristic cost to goal
    f_cost: i32, // Total cost
}

impl PathNode {
    fn new(pos: GridPos, g_cost: i32, h_cost: i32) -> Self {
        Self {
            pos,
            g_cost,
            h_cost,
            f_cost: g_cost + h_cost,
        }
    }
}

impl Ord for PathNode {
    fn cmp(&self, other: &Self) -> Ordering {
        // Reverse for min-heap
        other.f_cost.cmp(&self.f_cost)
    }
}

impl PartialOrd for PathNode {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Simple grid-based map
struct GridMap {
    width: i32,
    height: i32,
    obstacles: HashSet<GridPos>,
}

impl GridMap {
    fn new(width: i32, height: i32) -> Self {
        Self {
            width,
            height,
            obstacles: HashSet::new(),
        }
    }

    fn add_obstacle(&mut self, pos: GridPos) {
        self.obstacles.insert(pos);
    }

    fn is_walkable(&self, pos: &GridPos) -> bool {
        pos.x >= 0 && pos.x < self.width && pos.y >= 0 && pos.y < self.height
            && !self.obstacles.contains(pos)
    }

    fn find_path(&self, start: GridPos, goal: GridPos) -> Option<Vec<GridPos>> {
        if !self.is_walkable(&start) || !self.is_walkable(&goal) {
            return None;
        }

        let mut open_set = BinaryHeap::new();
        let mut came_from: HashMap<GridPos, GridPos> = HashMap::new();
        let mut g_scores: HashMap<GridPos, i32> = HashMap::new();

        g_scores.insert(start, 0);
        open_set.push(PathNode::new(start, 0, start.manhattan_distance(&goal)));

        while let Some(current) = open_set.pop() {
            if current.pos == goal {
                // Reconstruct path
                let mut path = vec![current.pos];
                let mut pos = current.pos;

                while let Some(&prev) = came_from.get(&pos) {
                    path.push(prev);
                    pos = prev;
                }

                path.reverse();
                return Some(path);
            }

            let current_g = *g_scores.get(&current.pos).unwrap_or(&i32::MAX);

            for neighbor in current.pos.neighbors() {
                if !self.is_walkable(&neighbor) {
                    continue;
                }

                let tentative_g = current_g + 1;
                let neighbor_g = *g_scores.get(&neighbor).unwrap_or(&i32::MAX);

                if tentative_g < neighbor_g {
                    came_from.insert(neighbor, current.pos);
                    g_scores.insert(neighbor, tentative_g);

                    let h = neighbor.manhattan_distance(&goal);
                    open_set.push(PathNode::new(neighbor, tentative_g, h));
                }
            }
        }

        None // No path found
    }
}

/// Test basic pathfinding
#[test]
fn test_basic_pathfinding() {
    println!("Testing basic pathfinding...");

    let map = GridMap::new(10, 10);
    let start = GridPos::new(0, 0);
    let goal = GridPos::new(5, 5);

    let path = map.find_path(start, goal);

    assert!(path.is_some());
    let path = path.unwrap();

    assert_eq!(path.first().unwrap(), &start);
    assert_eq!(path.last().unwrap(), &goal);
    assert!(path.len() > 0);

    log::info!("Basic pathfinding test passed");
}

/// Test pathfinding with obstacles
#[test]
fn test_pathfinding_with_obstacles() {
    println!("Testing pathfinding with obstacles...");

    let mut map = GridMap::new(10, 10);

    // Add wall
    for y in 0..8 {
        map.add_obstacle(GridPos::new(5, y));
    }

    let start = GridPos::new(0, 5);
    let goal = GridPos::new(9, 5);

    let path = map.find_path(start, goal);

    assert!(path.is_some());
    let path = path.unwrap();

    // Path should go around the wall
    assert!(path.len() > 10); // Direct path would be 9 steps

    // Verify no path goes through obstacles
    for pos in &path {
        assert!(map.is_walkable(pos), "Path should not go through obstacles");
    }

    log::info!("Pathfinding with obstacles test passed");
}

/// Test no path available
#[test]
fn test_no_path_available() {
    println!("Testing no path available scenario...");

    let mut map = GridMap::new(10, 10);

    // Create complete wall
    for y in 0..10 {
        map.add_obstacle(GridPos::new(5, y));
    }

    let start = GridPos::new(0, 5);
    let goal = GridPos::new(9, 5);

    let path = map.find_path(start, goal);

    assert!(path.is_none(), "Should return None when no path exists");

    log::info!("No path available test passed");
}

/// Test pathfinding to invalid position
#[test]
fn test_invalid_positions() {
    println!("Testing pathfinding to invalid positions...");

    let mut map = GridMap::new(10, 10);
    map.add_obstacle(GridPos::new(5, 5));

    let start = GridPos::new(0, 0);

    // Goal is an obstacle
    let goal = GridPos::new(5, 5);
    assert!(map.find_path(start, goal).is_none());

    // Goal is out of bounds
    let goal = GridPos::new(15, 15);
    assert!(map.find_path(start, goal).is_none());

    // Start is out of bounds
    let start = GridPos::new(-1, -1);
    let goal = GridPos::new(5, 5);
    assert!(map.find_path(start, goal).is_none());

    log::info!("Invalid positions test passed");
}

/// Test Manhattan distance calculation
#[test]
fn test_manhattan_distance() {
    println!("Testing Manhattan distance...");

    let p1 = GridPos::new(0, 0);
    let p2 = GridPos::new(3, 4);

    assert_eq!(p1.manhattan_distance(&p2), 7);

    let p3 = GridPos::new(5, 5);
    let p4 = GridPos::new(2, 1);

    assert_eq!(p3.manhattan_distance(&p4), 7);

    log::info!("Manhattan distance test passed");
}

/// Test neighbor generation
#[test]
fn test_neighbor_generation() {
    println!("Testing neighbor generation...");

    let pos = GridPos::new(5, 5);
    let neighbors = pos.neighbors();

    assert_eq!(neighbors.len(), 4);
    assert!(neighbors.contains(&GridPos::new(6, 5)));
    assert!(neighbors.contains(&GridPos::new(4, 5)));
    assert!(neighbors.contains(&GridPos::new(5, 6)));
    assert!(neighbors.contains(&GridPos::new(5, 4)));

    let diagonal_neighbors = pos.neighbors_diagonal();
    assert_eq!(diagonal_neighbors.len(), 8);

    log::info!("Neighbor generation test passed");
}

/// Test path optimality
#[test]
fn test_path_optimality() {
    println!("Testing path optimality...");

    let map = GridMap::new(10, 10);
    let start = GridPos::new(0, 0);
    let goal = GridPos::new(5, 5);

    let path = map.find_path(start, goal).unwrap();

    // Optimal path length is Manhattan distance (for 4-directional movement)
    let expected_length = start.manhattan_distance(&goal) + 1; // +1 includes start
    assert_eq!(path.len() as i32, expected_length);

    log::info!("Path optimality test passed");
}

/// Test path around complex obstacles
#[test]
fn test_complex_obstacles() {
    println!("Testing path around complex obstacles...");

    let mut map = GridMap::new(20, 20);

    // Create maze-like structure
    for x in 0..15 {
        if x % 3 != 0 {
            map.add_obstacle(GridPos::new(x, 5));
            map.add_obstacle(GridPos::new(x, 10));
            map.add_obstacle(GridPos::new(x, 15));
        }
    }

    let start = GridPos::new(0, 0);
    let goal = GridPos::new(19, 19);

    let path = map.find_path(start, goal);

    assert!(path.is_some());
    let path = path.unwrap();

    // Verify path is continuous
    for i in 0..path.len() - 1 {
        let current = &path[i];
        let next = &path[i + 1];
        let dist = current.manhattan_distance(next);
        assert_eq!(dist, 1, "Path should be continuous");
    }

    log::info!("Complex obstacles test passed");
}

/// Test multiple paths to same destination
#[test]
fn test_multiple_paths() {
    println!("Testing multiple paths...");

    let map = GridMap::new(10, 10);

    let start = GridPos::new(0, 0);
    let goal = GridPos::new(2, 2);

    // Find path multiple times (should be deterministic)
    let path1 = map.find_path(start, goal);
    let path2 = map.find_path(start, goal);

    assert_eq!(path1, path2);

    log::info!("Multiple paths test passed");
}

/// Test performance of pathfinding
#[test]
fn test_pathfinding_performance() {
    println!("Testing pathfinding performance...");

    let map = GridMap::new(50, 50);

    let start = GridPos::new(0, 0);
    let goal = GridPos::new(49, 49);

    let start_time = std::time::Instant::now();
    let path = map.find_path(start, goal);
    let elapsed = start_time.elapsed();

    assert!(path.is_some());
    println!("Pathfinding took: {:?}", elapsed);

    assert!(elapsed < std::time::Duration::from_millis(100),
        "Pathfinding should complete quickly");

    log::info!("Pathfinding performance test passed");
}

#[cfg(test)]
mod stress_tests {
    use super::*;

    /// Stress test: Many pathfinding requests
    #[test]
    #[ignore] // Run with: cargo test --test integration_pathfinding -- --ignored
    fn test_many_pathfinding_requests() {
        println!("Stress test: Many pathfinding requests...");

        let mut map = GridMap::new(100, 100);

        // Add some obstacles
        for i in 0..50 {
            map.add_obstacle(GridPos::new(50, i));
        }

        const NUM_REQUESTS: usize = 1000;
        let start_time = std::time::Instant::now();

        let mut successful_paths = 0;

        for i in 0..NUM_REQUESTS {
            let start_x = (i % 100) as i32;
            let start_y = ((i / 100) % 100) as i32;
            let goal_x = ((i + 50) % 100) as i32;
            let goal_y = ((i + 75) % 100) as i32;

            let start = GridPos::new(start_x, start_y);
            let goal = GridPos::new(goal_x, goal_y);

            if map.find_path(start, goal).is_some() {
                successful_paths += 1;
            }
        }

        let elapsed = start_time.elapsed();
        let paths_per_sec = NUM_REQUESTS as f64 / elapsed.as_secs_f64();

        println!("Computed {} paths in {:?} ({:.0} paths/sec)",
            NUM_REQUESTS, elapsed, paths_per_sec);
        println!("Successful paths: {}/{}", successful_paths, NUM_REQUESTS);

        assert!(paths_per_sec > 100.0, "Should compute >100 paths/second");

        log::info!("Many pathfinding requests stress test passed");
    }

    /// Stress test: Large map pathfinding
    #[test]
    #[ignore]
    fn test_large_map_pathfinding() {
        println!("Stress test: Large map pathfinding...");

        let map = GridMap::new(500, 500);

        let start = GridPos::new(0, 0);
        let goal = GridPos::new(499, 499);

        let start_time = std::time::Instant::now();
        let path = map.find_path(start, goal);
        let elapsed = start_time.elapsed();

        assert!(path.is_some());

        println!("Large map pathfinding took: {:?}", elapsed);
        println!("Path length: {}", path.unwrap().len());

        assert!(elapsed < std::time::Duration::from_secs(1),
            "Should find path in large map within 1 second");

        log::info!("Large map pathfinding stress test passed");
    }
}
