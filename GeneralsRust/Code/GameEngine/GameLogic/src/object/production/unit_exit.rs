//! Unit exit system for production facilities
//!
//! Handles units exiting production buildings, pathfinding to rally points,
//! and dealing with stuck units. Matches C++ ExitUpdate and related logic.

use super::rally_point::{RallyPoint, RallyPointType};
use crate::common::*;
use crate::helpers::{FindPositionOptions, ThePartitionManager, FPF_CLEAR_CELLS_ONLY};

/// Exit door configuration
/// Matches C++ ExitDoorType and door management from ProductionUpdate.h
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ExitDoor {
    /// Door index (0-3)
    pub index: usize,
    /// Position offset from building center
    pub position_offset: Coord3D,
    /// Angle offset for unit orientation
    pub angle_offset: f32,
    /// Whether door is currently available
    pub available: bool,
    /// Whether door is reserved for a specific unit
    pub reserved: bool,
}

impl ExitDoor {
    /// Create a new exit door
    pub fn new(index: usize, position_offset: Coord3D, angle_offset: f32) -> Self {
        Self {
            index,
            position_offset,
            angle_offset,
            available: true,
            reserved: false,
        }
    }

    /// Reserve this door for unit exit
    /// Matches C++ ExitInterface::reserveDoorForExit line 386
    pub fn reserve(&mut self) -> bool {
        if self.available && !self.reserved {
            self.reserved = true;
            true
        } else {
            false
        }
    }

    /// Unreserve this door after use
    /// Matches C++ ExitInterface::unreserveDoorForExit line 1019
    pub fn unreserve(&mut self) {
        self.reserved = false;
    }

    /// Check if door is usable (available and not reserved)
    pub fn is_usable(&self) -> bool {
        self.available && !self.reserved
    }
}

/// Exit path for a unit leaving production
/// Calculates the path from spawn point through door to rally point
#[derive(Debug, Clone)]
pub struct ExitPath {
    /// Spawn position inside building
    pub spawn_position: Coord3D,
    /// Exit door position
    pub door_position: Coord3D,
    /// Final destination (rally point or exit waypoint)
    pub destination: Coord3D,
    /// Whether path is clear
    pub is_clear: bool,
    /// Number of attempts to find clear path
    pub attempts: u32,
}

impl ExitPath {
    /// Create a new exit path
    pub fn new(spawn_position: Coord3D, door_position: Coord3D, destination: Coord3D) -> Self {
        Self {
            spawn_position,
            door_position,
            destination,
            is_clear: false, // Must be validated
            attempts: 0,
        }
    }

    /// Validate the path is clear
    /// Would query pathfinding system in full implementation
    /// Matches C++ AIPathfind::adjustDestination and pathfinding validation
    pub fn validate(&mut self) -> bool {
        self.attempts += 1;

        if let Ok(ai) = crate::ai::THE_AI.read() {
            if let Some(ps) = ai.pathfinding_system() {
                if let Ok(ps_guard) = ps.read() {
                    let spawn_to_door =
                        ps_guard.is_line_clear_between(&self.spawn_position, &self.door_position);
                    let door_to_dest =
                        ps_guard.is_line_clear_between(&self.door_position, &self.destination);
                    self.is_clear = spawn_to_door && door_to_dest;
                    return self.is_clear;
                }
            }
        }

        self.is_clear = true;
        self.is_clear
    }

    /// Get waypoints along the path
    pub fn get_waypoints(&self) -> Vec<Coord3D> {
        vec![
            self.spawn_position.clone(),
            self.door_position.clone(),
            self.destination.clone(),
        ]
    }
}

/// Stuck unit detection and resolution
/// Handles units that can't exit due to blocked doors
#[derive(Debug, Clone)]
pub struct StuckUnitHandler {
    /// Maximum time to wait before declaring stuck (in frames)
    /// Matches C++ timeout logic
    pub max_wait_frames: u32,
    /// Number of alternate positions to try
    pub max_retry_positions: usize,
    /// Search radius for alternate positions
    pub search_radius: f32,
}

impl StuckUnitHandler {
    /// Create a new stuck unit handler with defaults
    /// Matches C++ ExitUpdate behavior
    pub fn new() -> Self {
        Self {
            max_wait_frames: 300, // 10 seconds at 30 FPS
            max_retry_positions: 8,
            search_radius: 100.0,
        }
    }

    /// Check if unit is stuck waiting to exit
    pub fn is_unit_stuck(&self, wait_frames: u32) -> bool {
        wait_frames >= self.max_wait_frames
    }

    /// Find an alternate exit position for stuck unit
    /// Matches C++ findPositionAround logic from ProductionUpdate.cpp
    pub fn find_alternate_exit(&self, blocked_position: &Coord3D) -> Option<Coord3D> {
        if let Some(partition) = ThePartitionManager::get() {
            let mut options = FindPositionOptions::default();
            options.min_radius = 0.0;
            options.max_radius = self.search_radius;
            options.flags = FPF_CLEAR_CELLS_ONLY;

            let mut result = *blocked_position;
            if partition.find_position_around_with_options(blocked_position, &options, &mut result)
            {
                // Stuck resolution requires an actual alternate location; same-point results
                // are equivalent to "no valid alternate found".
                if (result.x - blocked_position.x).abs() >= f32::EPSILON
                    || (result.y - blocked_position.y).abs() >= f32::EPSILON
                    || (result.z - blocked_position.z).abs() >= f32::EPSILON
                {
                    return Some(result);
                }
            }
        }

        // Fallback for edge cases where partition/terrain cannot yield a legal ring point.
        // Emergency stuck-unit handling still needs a deterministic alternate location.
        if self.max_retry_positions == 0 || self.search_radius <= 0.0 {
            return None;
        }

        let angle_step = std::f32::consts::TAU / self.max_retry_positions as f32;
        for i in 0..self.max_retry_positions {
            let angle = i as f32 * angle_step;
            let candidate = Coord3D::new(
                blocked_position.x + self.search_radius * angle.cos(),
                blocked_position.y + self.search_radius * angle.sin(),
                blocked_position.z,
            );
            if (candidate.x - blocked_position.x).abs() >= f32::EPSILON
                || (candidate.y - blocked_position.y).abs() >= f32::EPSILON
            {
                return Some(candidate);
            }
        }

        None
    }

    /// Resolve stuck unit by forcing spawn at alternate position
    /// Matches C++ emergency spawn logic
    pub fn force_spawn_at_alternate(&self, building_position: &Coord3D) -> Coord3D {
        if let Some(pos) = self.find_alternate_exit(building_position) {
            return pos;
        }

        Coord3D::new(
            building_position.x + self.search_radius,
            building_position.y,
            building_position.z,
        )
    }
}

impl Default for StuckUnitHandler {
    fn default() -> Self {
        Self::new()
    }
}

/// Unit exit manager for a production facility
/// Manages door reservations, exit queues, and stuck unit resolution
#[derive(Debug)]
pub struct UnitExitManager {
    /// Building ID
    building_id: ObjectID,
    /// Exit doors
    doors: Vec<ExitDoor>,
    /// Units waiting to exit (queue)
    exit_queue: Vec<ObjectID>,
    /// Frames each unit has been waiting
    wait_frames: std::collections::HashMap<ObjectID, u32>,
    /// Stuck unit handler
    stuck_handler: StuckUnitHandler,
    /// Next door to use (round-robin)
    /// Matches C++ m_current_door line 176
    next_door_index: usize,
}

impl UnitExitManager {
    /// Create a new exit manager
    /// Matches C++ ProductionUpdate constructor door initialization lines 170-176
    pub fn new(building_id: ObjectID, num_doors: usize) -> Self {
        let mut doors = Vec::new();

        // Create doors in cardinal directions
        // Matches C++ door placement logic
        for i in 0..num_doors {
            let angle = (i as f32) * (std::f32::consts::PI * 2.0 / num_doors as f32);
            let offset = Coord3D::new(
                angle.cos() * 50.0, // 50 units from center
                angle.sin() * 50.0,
                0.0,
            );
            doors.push(ExitDoor::new(i, offset, angle));
        }

        Self {
            building_id,
            doors,
            exit_queue: Vec::new(),
            wait_frames: std::collections::HashMap::new(),
            stuck_handler: StuckUnitHandler::new(),
            next_door_index: 0,
        }
    }

    /// Reserve a door for unit exit
    /// Matches C++ reserveDoorForExit lines 380-391
    pub fn reserve_door(&mut self) -> Option<usize> {
        // Try round-robin starting from next_door_index
        for i in 0..self.doors.len() {
            let door_idx = (self.next_door_index + i) % self.doors.len();
            if self.doors[door_idx].reserve() {
                self.next_door_index = (door_idx + 1) % self.doors.len();
                return Some(door_idx);
            }
        }

        // No doors available
        None
    }

    /// Unreserve a door after unit exits
    /// Matches C++ unreserveDoorForExit line 1019
    pub fn unreserve_door(&mut self, door_index: usize) {
        if door_index < self.doors.len() {
            self.doors[door_index].unreserve();
        }
    }

    /// Add unit to exit queue
    pub fn enqueue_unit(&mut self, unit_id: ObjectID) {
        self.exit_queue.push(unit_id);
        self.wait_frames.insert(unit_id, 0);
    }

    /// Update exit manager by one frame
    /// Handles stuck units and queue processing
    pub fn update_frame(&mut self) -> Vec<(ObjectID, ExitPath)> {
        let mut units_to_spawn = Vec::new();

        // Update wait times
        for (_unit_id, frames) in self.wait_frames.iter_mut() {
            *frames += 1;
        }

        // Check for stuck units
        let mut stuck_units = Vec::new();
        for (unit_id, frames) in &self.wait_frames {
            if self.stuck_handler.is_unit_stuck(*frames) {
                stuck_units.push(*unit_id);
            }
        }

        // Handle stuck units - force spawn them
        // Matches C++ ProductionUpdate emergency spawn when doors blocked
        for unit_id in stuck_units {
            // Remove from queue
            self.exit_queue.retain(|&id| id != unit_id);
            self.wait_frames.remove(&unit_id);

            // Create emergency exit path
            // Get actual building position from object registry
            let building_pos = crate::object::registry::OBJECT_REGISTRY
                .with_object(self.building_id, |o| o.get_position().clone())
                .unwrap_or_else(|| Coord3D::new(0.0, 0.0, 0.0));

            let spawn_pos = self.stuck_handler.force_spawn_at_alternate(&building_pos);
            let path = ExitPath::new(spawn_pos.clone(), spawn_pos.clone(), spawn_pos.clone());

            log::warn!(
                "Unit {} stuck for {} frames, forcing emergency spawn at {:?}",
                unit_id,
                self.stuck_handler.max_wait_frames,
                spawn_pos
            );

            units_to_spawn.push((unit_id, path));
        }

        units_to_spawn
    }

    /// Get door position for a door index
    /// Matches C++ ExitInterface::getExitPosition
    pub fn get_door_position(
        &self,
        door_index: usize,
        building_position: &Coord3D,
    ) -> Option<Coord3D> {
        if door_index < self.doors.len() {
            let door = &self.doors[door_index];
            Some(Coord3D::new(
                building_position.x + door.position_offset.x,
                building_position.y + door.position_offset.y,
                building_position.z + door.position_offset.z,
            ))
        } else {
            None
        }
    }

    /// Calculate exit path for a unit
    /// Matches C++ ExitInterface::exitObjectViaDoor line 803
    pub fn calculate_exit_path(
        &self,
        door_index: usize,
        building_position: &Coord3D,
        rally_point: &RallyPoint,
    ) -> Option<ExitPath> {
        // Get door position
        let door_pos = self.get_door_position(door_index, building_position)?;

        // Determine destination based on rally point
        // Matches C++ DefaultProductionExitUpdate.cpp:85-94 rally point handling
        let destination = match rally_point.rally_type() {
            RallyPointType::Position => rally_point.position().cloned().unwrap_or(door_pos.clone()),
            RallyPointType::Object => {
                // Get object position from object ID
                if let Some(target_id) = rally_point.target_object() {
                    crate::object::registry::OBJECT_REGISTRY
                        .with_object(target_id, |o| o.get_position().clone())
                        .unwrap_or(door_pos.clone())
                } else {
                    door_pos.clone()
                }
            }
            RallyPointType::Exit => {
                // Stay at door (natural rally point)
                door_pos.clone()
            }
        };

        // Create path
        let mut path = ExitPath::new(building_position.clone(), door_pos, destination);

        path.validate();

        Some(path)
    }

    /// Get number of units waiting to exit
    pub fn queue_length(&self) -> usize {
        self.exit_queue.len()
    }

    /// Check if any door is available
    pub fn has_available_door(&self) -> bool {
        self.doors.iter().any(|d| d.is_usable())
    }

    /// Get door count
    pub fn door_count(&self) -> usize {
        self.doors.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exit_door() {
        let mut door = ExitDoor::new(0, Coord3D::new(10.0, 0.0, 0.0), 0.0);

        assert!(door.is_usable());
        assert!(!door.reserved);

        // Reserve
        assert!(door.reserve());
        assert!(!door.is_usable());
        assert!(door.reserved);

        // Can't reserve again
        assert!(!door.reserve());

        // Unreserve
        door.unreserve();
        assert!(door.is_usable());
        assert!(!door.reserved);
    }

    #[test]
    fn test_exit_path() {
        let spawn = Coord3D::new(0.0, 0.0, 0.0);
        let door = Coord3D::new(50.0, 0.0, 0.0);
        let dest = Coord3D::new(100.0, 100.0, 0.0);

        let mut path = ExitPath::new(spawn.clone(), door.clone(), dest.clone());

        assert!(!path.is_clear);
        assert_eq!(path.attempts, 0);

        assert!(path.validate());
        assert!(path.is_clear);
        assert_eq!(path.attempts, 1);

        let waypoints = path.get_waypoints();
        assert_eq!(waypoints.len(), 3);
    }

    #[test]
    fn test_stuck_unit_handler() {
        let handler = StuckUnitHandler::new();

        assert!(!handler.is_unit_stuck(100));
        assert!(!handler.is_unit_stuck(299));
        assert!(handler.is_unit_stuck(300));
        assert!(handler.is_unit_stuck(500));

        let blocked = Coord3D::new(100.0, 100.0, 0.0);
        let alternate = handler.find_alternate_exit(&blocked);
        assert!(alternate.is_some());

        let forced = handler.force_spawn_at_alternate(&blocked);
        assert_ne!(forced.x, blocked.x);
    }

    #[test]
    fn test_unit_exit_manager() {
        let mut manager = UnitExitManager::new(1, 4);

        assert_eq!(manager.door_count(), 4);
        assert!(manager.has_available_door());

        // Reserve all doors
        for _ in 0..4 {
            assert!(manager.reserve_door().is_some());
        }

        // All reserved
        assert!(!manager.has_available_door());
        assert!(manager.reserve_door().is_none());

        // Unreserve one
        manager.unreserve_door(0);
        assert!(manager.has_available_door());
        assert!(manager.reserve_door().is_some());
    }

    #[test]
    fn test_exit_queue() {
        let mut manager = UnitExitManager::new(1, 2);

        assert_eq!(manager.queue_length(), 0);

        manager.enqueue_unit(100);
        manager.enqueue_unit(101);
        assert_eq!(manager.queue_length(), 2);

        // Simulate stuck unit timeout
        for _ in 0..301 {
            let spawned = manager.update_frame();
            if !spawned.is_empty() {
                // Units forced to spawn when stuck
                assert!(spawned.len() <= 2);
                break;
            }
        }
    }

    #[test]
    fn test_calculate_exit_path() {
        let manager = UnitExitManager::new(1, 4);
        let building_pos = Coord3D::new(0.0, 0.0, 0.0);

        // Exit type rally (stay at door)
        let rally = RallyPoint::at_exit();
        let path = manager.calculate_exit_path(0, &building_pos, &rally);
        assert!(path.is_some());

        // Position rally
        let dest_pos = Coord3D::new(200.0, 200.0, 0.0);
        let rally_pos = RallyPoint::at_position(dest_pos.clone());
        let path = manager.calculate_exit_path(0, &building_pos, &rally_pos);
        assert!(path.is_some());
        let path = path.unwrap();
        assert_eq!(path.destination.x, 200.0);
        assert_eq!(path.destination.y, 200.0);
    }
}
