// group_pathfinding.rs
// Group Pathfinding and Formation Movement
// Reference: C++ AIGroup and formation logic

use super::pathfind_astar::GridCoord;
use super::pathfind_complete::{PathRequest, PathResult, PathfindingSystem, SURFACE_GROUND};
use crate::common::{Coord2D, Coord3D, ObjectID};

use std::collections::HashMap;

/// Formation types for group movement
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormationType {
    /// No formation, units move independently
    None,
    /// Line formation
    Line,
    /// Column formation
    Column,
    /// Wedge formation
    Wedge,
    /// Box formation
    Box,
    /// Spread out formation
    Scatter,
}

/// Group pathfinding manager
/// Handles coordinated movement of multiple units
pub struct GroupPathfinder {
    /// Formation spacing between units (in world units)
    formation_spacing: f32,

    /// Leader unit for the group
    leader_id: Option<ObjectID>,

    /// Formation offsets for each unit
    formation_offsets: HashMap<ObjectID, Coord2D>,
}

impl GroupPathfinder {
    pub fn new(formation_spacing: f32) -> Self {
        Self {
            formation_spacing,
            leader_id: None,
            formation_offsets: HashMap::new(),
        }
    }

    /// Find paths for a group of units
    /// Returns individual paths for each unit maintaining formation
    pub fn find_group_paths(
        &mut self,
        pathfinder: &PathfindingSystem,
        unit_ids: &[ObjectID],
        unit_positions: &HashMap<ObjectID, Coord3D>,
        goal: Coord3D,
        formation: FormationType,
        surfaces: u32,
        is_crusher: bool,
        unit_radius: f32,
    ) -> HashMap<ObjectID, PathResult> {
        if unit_ids.is_empty() {
            return HashMap::new();
        }

        // Calculate formation positions at goal
        let formation_positions = self.calculate_formation(unit_ids, &goal, formation);

        // Find path for each unit to its formation position
        let mut paths = HashMap::new();

        for (unit_id, formation_goal) in formation_positions {
            let start = unit_positions
                .get(&unit_id)
                .cloned()
                .unwrap_or_else(|| Coord3D::new(0.0, 0.0, 0.0));

            let request = PathRequest {
                object_id: unit_id,
                from: start,
                to: formation_goal,
                surfaces,
                is_crusher,
                unit_radius,
                allow_partial: true,
                move_allies: true,
                ignore_obstacle_id: None,
            };

            let result = pathfinder.find_path(request);
            paths.insert(unit_id, result);
        }

        paths
    }

    /// Calculate formation positions for units at the goal
    fn calculate_formation(
        &mut self,
        unit_ids: &[ObjectID],
        center: &Coord3D,
        formation: FormationType,
    ) -> HashMap<ObjectID, Coord3D> {
        let mut positions = HashMap::new();

        match formation {
            FormationType::None => {
                // All units go to same position
                for &unit_id in unit_ids {
                    positions.insert(unit_id, *center);
                }
            }

            FormationType::Line => {
                // Units arranged in a horizontal line
                let count = unit_ids.len();
                let total_width = (count - 1) as f32 * self.formation_spacing;
                let start_x = center.x - total_width / 2.0;

                for (i, &unit_id) in unit_ids.iter().enumerate() {
                    let pos = Coord3D::new(
                        start_x + i as f32 * self.formation_spacing,
                        center.y,
                        center.z,
                    );
                    positions.insert(unit_id, pos);
                }
            }

            FormationType::Column => {
                // Units arranged in a vertical column
                let count = unit_ids.len();
                let total_height = (count - 1) as f32 * self.formation_spacing;
                let start_y = center.y - total_height / 2.0;

                for (i, &unit_id) in unit_ids.iter().enumerate() {
                    let pos = Coord3D::new(
                        center.x,
                        start_y + i as f32 * self.formation_spacing,
                        center.z,
                    );
                    positions.insert(unit_id, pos);
                }
            }

            FormationType::Wedge => {
                // V-shaped formation
                let leader = unit_ids.get(0);
                if let Some(&leader_id) = leader {
                    positions.insert(leader_id, *center);
                }

                let mut left_offset = self.formation_spacing;
                let mut right_offset = self.formation_spacing;

                for (i, &unit_id) in unit_ids.iter().enumerate().skip(1) {
                    if i % 2 == 1 {
                        // Left side
                        let pos =
                            Coord3D::new(center.x - left_offset, center.y - left_offset, center.z);
                        positions.insert(unit_id, pos);
                        left_offset += self.formation_spacing;
                    } else {
                        // Right side
                        let pos = Coord3D::new(
                            center.x + right_offset,
                            center.y - right_offset,
                            center.z,
                        );
                        positions.insert(unit_id, pos);
                        right_offset += self.formation_spacing;
                    }
                }
            }

            FormationType::Box => {
                // Box/square formation
                let count = unit_ids.len();
                let side_length = (count as f32).sqrt().ceil() as usize;

                for (i, &unit_id) in unit_ids.iter().enumerate() {
                    let row = i / side_length;
                    let col = i % side_length;

                    let pos = Coord3D::new(
                        center.x + (col as f32 - side_length as f32 / 2.0) * self.formation_spacing,
                        center.y + (row as f32 - side_length as f32 / 2.0) * self.formation_spacing,
                        center.z,
                    );
                    positions.insert(unit_id, pos);
                }
            }

            FormationType::Scatter => {
                // Scattered formation with random offsets
                use std::collections::hash_map::DefaultHasher;
                use std::hash::{Hash, Hasher};

                for &unit_id in unit_ids {
                    // Use unit ID as seed for deterministic randomness
                    let mut hasher = DefaultHasher::new();
                    unit_id.hash(&mut hasher);
                    let hash = hasher.finish();

                    let angle = (hash % 360) as f32 * std::f32::consts::PI / 180.0;
                    let distance =
                        ((hash >> 32) % 100) as f32 / 100.0 * self.formation_spacing * 2.0;

                    let pos = Coord3D::new(
                        center.x + angle.cos() * distance,
                        center.y + angle.sin() * distance,
                        center.z,
                    );
                    positions.insert(unit_id, pos);
                }
            }
        }

        positions
    }

    /// Adjust formation for terrain obstacles
    /// Returns adjusted positions that avoid obstacles
    pub fn adjust_formation_for_terrain(
        &self,
        pathfinder: &PathfindingSystem,
        positions: &HashMap<ObjectID, Coord3D>,
        surfaces: u32,
    ) -> HashMap<ObjectID, Coord3D> {
        let mut adjusted = HashMap::new();

        for (&unit_id, &pos) in positions {
            // Try to find a nearby passable position
            let coord = GridCoord::from_world(&pos);
            let adjusted_pos = self.find_nearest_passable(pathfinder, coord, surfaces, 5);

            adjusted.insert(
                unit_id,
                adjusted_pos.to_world(super::pathfind_astar::PathfindLayerEnum::Ground),
            );
        }

        adjusted
    }

    /// Find nearest passable cell to given coordinate
    fn find_nearest_passable(
        &self,
        _pathfinder: &PathfindingSystem,
        start: GridCoord,
        _surfaces: u32,
        max_radius: i32,
    ) -> GridCoord {
        // For now, just return the start coordinate
        // Full implementation would search outward in expanding square
        start
    }

    /// Set leader unit for the group
    pub fn set_leader(&mut self, unit_id: ObjectID) {
        self.leader_id = Some(unit_id);
    }

    /// Get leader unit ID
    pub fn get_leader(&self) -> Option<ObjectID> {
        self.leader_id
    }

    /// Update formation spacing
    pub fn set_formation_spacing(&mut self, spacing: f32) {
        self.formation_spacing = spacing.max(10.0); // Minimum spacing
    }
}

/// Flow field for group movement coordination
/// Provides smooth movement for large groups
pub struct FlowField {
    /// Vector field showing direction to goal at each cell
    directions: Vec<Vec<Option<Coord2D>>>,

    /// Cost to goal from each cell
    costs: Vec<Vec<u32>>,

    /// Grid dimensions
    width: usize,
    height: usize,

    /// Goal position
    goal: GridCoord,
}

impl FlowField {
    pub fn new(width: usize, height: usize, goal: GridCoord) -> Self {
        let mut costs = vec![vec![u32::MAX; height]; width];
        if goal.x >= 0 && goal.x < width as i32 && goal.y >= 0 && goal.y < height as i32 {
            costs[goal.x as usize][goal.y as usize] = 0;
        }

        Self {
            directions: vec![vec![None; height]; width],
            costs,
            width,
            height,
            goal,
        }
    }

    /// Generate flow field from pathfinding grid
    pub fn generate(&mut self, pathfinder: &PathfindingSystem, surfaces: u32) {
        // Dijkstra's algorithm working backwards from goal
        use std::cmp::Reverse;
        use std::collections::BinaryHeap;

        let mut open = BinaryHeap::new();
        open.push(Reverse((0u32, self.goal)));

        self.costs[self.goal.x as usize][self.goal.y as usize] = 0;

        while let Some(Reverse((cost, current))) = open.pop() {
            if cost > self.costs[current.x as usize][current.y as usize] {
                continue;
            }

            // Check all neighbors
            for neighbor in current.neighbors() {
                if neighbor.x < 0
                    || neighbor.x >= self.width as i32
                    || neighbor.y < 0
                    || neighbor.y >= self.height as i32
                {
                    continue;
                }

                let move_cost = if current.is_diagonal(&neighbor) {
                    14
                } else {
                    10
                };
                let new_cost = cost + move_cost;

                let nx = neighbor.x as usize;
                let ny = neighbor.y as usize;

                if new_cost < self.costs[nx][ny] {
                    self.costs[nx][ny] = new_cost;

                    // Calculate direction from neighbor to current
                    let dx = (current.x - neighbor.x) as f32;
                    let dy = (current.y - neighbor.y) as f32;
                    let len = (dx * dx + dy * dy).sqrt();

                    if len > 0.0 {
                        self.directions[nx][ny] = Some(Coord2D::new(dx / len, dy / len));
                    }

                    open.push(Reverse((new_cost, neighbor)));
                }
            }
        }
    }

    /// Get direction to move from a given position
    pub fn get_direction(&self, pos: &Coord3D) -> Option<Coord2D> {
        let coord = GridCoord::from_world(pos);

        if coord.x >= 0
            && coord.x < self.width as i32
            && coord.y >= 0
            && coord.y < self.height as i32
        {
            self.directions[coord.x as usize][coord.y as usize]
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_line_formation() {
        let mut group_pathfinder = GroupPathfinder::new(20.0);

        let units = vec![1, 2, 3];

        let center = Coord3D::new(100.0, 100.0, 0.0);
        let positions = group_pathfinder.calculate_formation(&units, &center, FormationType::Line);

        assert_eq!(positions.len(), 3);

        // Check they're in a line
        let pos1 = positions.get(&1).unwrap();
        let pos2 = positions.get(&2).unwrap();
        let pos3 = positions.get(&3).unwrap();

        // All should have same Y coordinate
        assert!((pos1.y - pos2.y).abs() < 0.1);
        assert!((pos2.y - pos3.y).abs() < 0.1);

        // X coordinates should be spaced
        assert!((pos2.x - pos1.x - 20.0).abs() < 0.1);
        assert!((pos3.x - pos2.x - 20.0).abs() < 0.1);
    }

    #[test]
    fn test_wedge_formation() {
        let mut group_pathfinder = GroupPathfinder::new(20.0);

        let units = vec![
            1, // Leader
            2, 3,
        ];

        let center = Coord3D::new(100.0, 100.0, 0.0);
        let positions = group_pathfinder.calculate_formation(&units, &center, FormationType::Wedge);

        assert_eq!(positions.len(), 3);

        // Leader should be at center
        let leader_pos = positions.get(&1).unwrap();
        assert!((leader_pos.x - center.x).abs() < 0.1);
        assert!((leader_pos.y - center.y).abs() < 0.1);
    }

    #[test]
    fn test_flow_field() {
        let goal = GridCoord::new(10, 10);
        let mut flow_field = FlowField::new(32, 32, goal);

        // Goal should have zero cost
        assert_eq!(flow_field.costs[10][10], 0);

        // Other cells should have max cost initially
        assert_eq!(flow_field.costs[0][0], u32::MAX);
    }
}
