//! DockUpdate - Base dock behavior for supply/repair buildings
//!
//! Handles:
//! - Approach lanes and docking positions
//! - Queue management for waiting units
//! - Unit servicing (supplies, repairs, heals)
//! - Exit and release procedures
//!
//! Used by:
//! - Supply centers (loading supplies)
//! - Supply warehouses (unloading supplies/cash)
//! - Repair docks (fixing vehicles)
//! - Prison buildings (POW delivery)
//! - Train stations (rail transport)
//!
//! Original C++ Author: EA Developers
//! Rust conversion: 2025

use crate::common::{Bool, Coord2D, Coord3D, ObjectID, Real, UnsignedInt};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

/// Docking state for a unit
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DockingState {
    /// Unit is approaching the dock
    Approaching,
    /// Unit is waiting in queue
    Waiting,
    /// Unit is actively docked and being serviced
    Docked,
    /// Unit is exiting the dock
    Exiting,
    /// Unit has completed docking
    Complete,
}

/// Information about a dock position
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockPosition {
    /// Position of the dock spot
    pub position: Coord3D,
    /// Approach waypoint
    pub approach_point: Coord3D,
    /// Exit waypoint
    pub exit_point: Coord3D,
    /// Is this dock spot currently occupied?
    pub occupied: Bool,
    /// ID of unit currently docked here
    pub occupant_id: ObjectID,
    /// Starting angle for this dock position
    pub start_angle: Real,
}

impl DockPosition {
    pub fn new(position: Coord3D, approach: Coord3D, exit: Coord3D, angle: Real) -> Self {
        Self {
            position,
            approach_point: approach,
            exit_point: exit,
            occupied: false,
            occupant_id: 0,
            start_angle: angle,
        }
    }

    pub fn is_available(&self) -> Bool {
        !self.occupied
    }

    pub fn reserve(&mut self, unit_id: ObjectID) {
        self.occupied = true;
        self.occupant_id = unit_id;
    }

    pub fn release(&mut self) {
        self.occupied = false;
        self.occupant_id = 0;
    }
}

/// Unit in the docking queue
#[derive(Debug, Clone)]
pub struct DockQueueEntry {
    /// ID of the unit
    pub unit_id: ObjectID,
    /// Current docking state
    pub state: DockingState,
    /// Assigned dock position index
    pub dock_index: Option<usize>,
    /// Frame when docking started
    pub dock_start_frame: UnsignedInt,
    /// Priority (lower = higher priority)
    pub priority: i32,
}

impl DockQueueEntry {
    pub fn new(unit_id: ObjectID, priority: i32) -> Self {
        Self {
            unit_id,
            state: DockingState::Waiting,
            dock_index: None,
            dock_start_frame: 0,
            priority,
        }
    }
}

/// Dock update module configuration (from INI)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockUpdateModuleData {
    /// Number of dock positions
    pub num_docks: usize,
    /// Time (frames) to service a unit
    pub service_time: UnsignedInt,
    /// Allow multiple units to dock simultaneously?
    pub allow_multiple_docks: Bool,
    /// Dock positions (relative to building center)
    #[serde(default)]
    pub dock_positions: Vec<Coord2D>,
    /// Approach points (relative to building center)
    #[serde(default)]
    pub approach_points: Vec<Coord2D>,
    /// Exit points (relative to building center)
    #[serde(default)]
    pub exit_points: Vec<Coord2D>,
    /// Starting angles for each dock
    #[serde(default)]
    pub dock_angles: Vec<Real>,
    /// Tolerance for reaching dock position
    #[serde(default = "default_tolerance")]
    pub dock_tolerance: Real,
}

fn default_tolerance() -> Real {
    5.0
}

impl Default for DockUpdateModuleData {
    fn default() -> Self {
        Self {
            num_docks: 1,
            service_time: 60, // 2 seconds at 30 fps
            allow_multiple_docks: false,
            dock_positions: Vec::new(),
            approach_points: Vec::new(),
            exit_points: Vec::new(),
            dock_angles: Vec::new(),
            dock_tolerance: 5.0,
        }
    }
}

/// Dock update behavior module
#[allow(dead_code)]
pub struct DockUpdate {
    /// Configuration data
    data: DockUpdateModuleData,

    /// Queue of units waiting to dock
    queue: VecDeque<DockQueueEntry>,

    /// Dock positions
    docks: Vec<DockPosition>,

    /// Current frame
    current_frame: UnsignedInt,

    /// Building position (center point)
    building_position: Coord3D,

    /// Building angle
    building_angle: Real,
}

impl DockUpdate {
    pub fn new(data: DockUpdateModuleData, building_pos: Coord3D, building_angle: Real) -> Self {
        // Initialize dock positions from configuration
        let mut docks = Vec::new();

        for i in 0..data.num_docks {
            let dock_pos_2d = data
                .dock_positions
                .get(i)
                .copied()
                .unwrap_or(Coord2D::new(0.0, 0.0));
            let approach_2d = data
                .approach_points
                .get(i)
                .copied()
                .unwrap_or(Coord2D::new(0.0, -20.0));
            let exit_2d = data
                .exit_points
                .get(i)
                .copied()
                .unwrap_or(Coord2D::new(0.0, 20.0));
            let angle = data.dock_angles.get(i).copied().unwrap_or(0.0);

            // Convert 2D positions to 3D (relative to building)
            let dock_pos = Self::relative_to_world(dock_pos_2d, building_pos, building_angle);
            let approach_pos = Self::relative_to_world(approach_2d, building_pos, building_angle);
            let exit_pos = Self::relative_to_world(exit_2d, building_pos, building_angle);

            docks.push(DockPosition::new(dock_pos, approach_pos, exit_pos, angle));
        }

        Self {
            data,
            queue: VecDeque::new(),
            docks,
            current_frame: 0,
            building_position: building_pos,
            building_angle,
        }
    }

    /// Convert relative 2D position to world 3D coordinates
    fn relative_to_world(relative: Coord2D, center: Coord3D, angle: Real) -> Coord3D {
        let cos_a = angle.cos();
        let sin_a = angle.sin();

        let x = relative[0] * cos_a - relative[1] * sin_a + center[0];
        let y = relative[0] * sin_a + relative[1] * cos_a + center[1];
        let z = center[2];

        Coord3D::new(x, y, z)
    }

    /// Request docking for a unit
    pub fn request_dock(&mut self, unit_id: ObjectID, priority: i32) -> Bool {
        // Check if unit is already in queue
        if self.queue.iter().any(|entry| entry.unit_id == unit_id) {
            return false;
        }

        // Add to queue
        let entry = DockQueueEntry::new(unit_id, priority);
        self.queue.push_back(entry);

        // Sort queue by priority
        self.queue.make_contiguous().sort_by_key(|e| e.priority);

        true
    }

    /// Cancel docking request for a unit
    pub fn cancel_dock(&mut self, unit_id: ObjectID) -> Bool {
        // Find and remove from queue
        if let Some(pos) = self.queue.iter().position(|e| e.unit_id == unit_id) {
            let entry = self.queue.remove(pos).unwrap();

            // Release dock if assigned
            if let Some(dock_idx) = entry.dock_index {
                if let Some(dock) = self.docks.get_mut(dock_idx) {
                    dock.release();
                }
            }

            return true;
        }

        false
    }

    /// Find an available dock position
    fn find_available_dock(&self) -> Option<usize> {
        self.docks.iter().position(|dock| dock.is_available())
    }

    /// Assign a dock to a queued unit
    fn assign_dock_to_unit(&mut self, queue_idx: usize, dock_idx: usize) -> Bool {
        if let Some(entry) = self.queue.get_mut(queue_idx) {
            if let Some(dock) = self.docks.get_mut(dock_idx) {
                dock.reserve(entry.unit_id);
                entry.dock_index = Some(dock_idx);
                entry.state = DockingState::Approaching;
                return true;
            }
        }
        false
    }

    /// Update unit that is approaching
    fn update_approaching(&mut self, queue_idx: usize) {
        if let Some(entry) = self.queue.get_mut(queue_idx) {
            if let Some(dock_idx) = entry.dock_index {
                if let Some(_dock) = self.docks.get(dock_idx) {
                    // Check if unit has reached dock position
                    // Would check distance here

                    // For now, assume it arrives
                    entry.state = DockingState::Docked;
                    entry.dock_start_frame = self.current_frame;

                    // Would send command to unit to move to dock position
                    // Would update unit animation state
                }
            }
        }
    }

    /// Update unit that is docked (being serviced)
    fn update_docked(&mut self, queue_idx: usize) {
        let Some(entry) = self.queue.get(queue_idx) else {
            return;
        };
        let unit_id = entry.unit_id;
        let frames_docked = self.current_frame.saturating_sub(entry.dock_start_frame);

        // Perform service logic (override in derived classes)
        self.perform_service(unit_id, frames_docked);

        // Check if service complete
        if frames_docked >= self.data.service_time {
            if let Some(entry) = self.queue.get_mut(queue_idx) {
                entry.state = DockingState::Exiting;
            }
        }
    }

    /// Perform service on docked unit (override in derived classes)
    fn perform_service(&mut self, _unit_id: ObjectID, _frames: UnsignedInt) {
        // Base implementation does nothing
        // Derived classes override to provide supplies, repairs, etc.
    }

    /// Update unit that is exiting
    fn update_exiting(&mut self, queue_idx: usize) {
        if let Some(entry) = self.queue.get_mut(queue_idx) {
            if let Some(dock_idx) = entry.dock_index {
                if let Some(_dock) = self.docks.get(dock_idx) {
                    // Send unit to exit point
                    // Would check if unit has reached exit

                    // For now, mark as complete
                    entry.state = DockingState::Complete;
                }
            }
        }
    }

    /// Get dock position for a unit
    pub fn get_dock_position(&self, unit_id: ObjectID) -> Option<Coord3D> {
        self.queue
            .iter()
            .find(|e| e.unit_id == unit_id)
            .and_then(|e| e.dock_index)
            .and_then(|idx| self.docks.get(idx))
            .map(|dock| dock.position)
    }

    /// Get approach position for a unit
    pub fn get_approach_position(&self, unit_id: ObjectID) -> Option<Coord3D> {
        self.queue
            .iter()
            .find(|e| e.unit_id == unit_id)
            .and_then(|e| e.dock_index)
            .and_then(|idx| self.docks.get(idx))
            .map(|dock| dock.approach_point)
    }

    /// Get exit position for a unit
    pub fn get_exit_position(&self, unit_id: ObjectID) -> Option<Coord3D> {
        self.queue
            .iter()
            .find(|e| e.unit_id == unit_id)
            .and_then(|e| e.dock_index)
            .and_then(|idx| self.docks.get(idx))
            .map(|dock| dock.exit_point)
    }

    /// Is a specific dock occupied?
    pub fn is_dock_occupied(&self, dock_index: usize) -> Bool {
        self.docks
            .get(dock_index)
            .map(|d| d.occupied)
            .unwrap_or(false)
    }

    /// Get number of units waiting
    pub fn get_queue_length(&self) -> usize {
        self.queue.len()
    }

    /// Update the dock system
    pub fn update(&mut self, current_frame: UnsignedInt) {
        self.current_frame = current_frame;

        // Process queue
        let mut i = 0;
        while i < self.queue.len() {
            let entry_state = self.queue[i].state;

            match entry_state {
                DockingState::Approaching => {
                    self.update_approaching(i);
                }
                DockingState::Waiting => {
                    // Try to assign a dock
                    if let Some(dock_idx) = self.find_available_dock() {
                        self.assign_dock_to_unit(i, dock_idx);
                    }
                }
                DockingState::Docked => {
                    self.update_docked(i);
                }
                DockingState::Exiting => {
                    self.update_exiting(i);
                }
                DockingState::Complete => {
                    // Remove from queue and release dock
                    if let Some(entry) = self.queue.remove(i) {
                        if let Some(dock_idx) = entry.dock_index {
                            if let Some(dock) = self.docks.get_mut(dock_idx) {
                                dock.release();
                            }
                        }
                    }
                    continue; // Don't increment i
                }
            }

            i += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dock_creation() {
        let data = DockUpdateModuleData {
            num_docks: 2,
            service_time: 60,
            ..Default::default()
        };

        let dock = DockUpdate::new(data, Coord3D::new(100.0, 100.0, 0.0), 0.0);
        assert_eq!(dock.docks.len(), 2);
        assert_eq!(dock.get_queue_length(), 0);
    }

    #[test]
    fn test_dock_request() {
        let data = DockUpdateModuleData::default();
        let mut dock = DockUpdate::new(data, Coord3D::new(0.0, 0.0, 0.0), 0.0);

        let unit_id: ObjectID = 123;
        assert!(dock.request_dock(unit_id, 0));
        assert_eq!(dock.get_queue_length(), 1);

        // Can't request twice
        assert!(!dock.request_dock(unit_id, 0));
        assert_eq!(dock.get_queue_length(), 1);
    }

    #[test]
    fn test_dock_cancel() {
        let data = DockUpdateModuleData::default();
        let mut dock = DockUpdate::new(data, Coord3D::new(0.0, 0.0, 0.0), 0.0);

        let unit_id: ObjectID = 123;
        dock.request_dock(unit_id, 0);

        assert!(dock.cancel_dock(unit_id));
        assert_eq!(dock.get_queue_length(), 0);
    }

    #[test]
    fn test_dock_priority() {
        let data = DockUpdateModuleData::default();
        let mut dock = DockUpdate::new(data, Coord3D::new(0.0, 0.0, 0.0), 0.0);

        dock.request_dock(1, 10);
        dock.request_dock(2, 5);
        dock.request_dock(3, 15);

        // Should be sorted by priority (lower first)
        assert_eq!(dock.queue[0].unit_id, 2); // priority 5
        assert_eq!(dock.queue[1].unit_id, 1); // priority 10
        assert_eq!(dock.queue[2].unit_id, 3); // priority 15
    }

    #[test]
    fn dock_request_assigns_available_dock_before_approach() {
        let data = DockUpdateModuleData {
            service_time: 1,
            ..Default::default()
        };
        let mut dock = DockUpdate::new(data, Coord3D::new(0.0, 0.0, 0.0), 0.0);

        assert!(dock.request_dock(123, 0));
        assert_eq!(dock.queue[0].state, DockingState::Waiting);

        dock.update(10);
        assert_eq!(dock.queue[0].state, DockingState::Approaching);
        assert_eq!(dock.queue[0].dock_index, Some(0));
        assert!(dock.is_dock_occupied(0));

        dock.update(11);
        assert_eq!(dock.queue[0].state, DockingState::Docked);
        assert_eq!(dock.queue[0].dock_start_frame, 11);

        dock.update(12);
        assert_eq!(dock.queue[0].state, DockingState::Exiting);

        dock.update(13);
        assert_eq!(dock.queue[0].state, DockingState::Complete);

        dock.update(14);
        assert_eq!(dock.get_queue_length(), 0);
        assert!(!dock.is_dock_occupied(0));
    }
}
