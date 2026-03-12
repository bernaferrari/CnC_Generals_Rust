//! PathfindCell implementation matching C++ PathfindCell class
#![allow(missing_docs)]
//!
//! This represents one cell in the pathfinding grid.
//! These cells categorize the world into idealized cellular states,
//! and are also used for efficient A* pathfinding.

use super::*;
use crate::common::*;
use crate::path::PathfindLayerEnum;
use std::ptr;

/// Cell type enumeration matching C++ PathfindCell::CellType
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum PathfindCellType {
    Clear = 0x00,            // clear, unobstructed ground
    Water = 0x01,            // water area
    Cliff = 0x02,            // steep altitude change
    Rubble = 0x03,           // Cell is occupied by rubble
    Obstacle = 0x04,         // Occupied by a structure
    BridgeImpassable = 0x05, // Piece of a bridge that is impassable
    Impassable = 0x06,       // Just plain impassable except for aircraft
}

impl Default for PathfindCellType {
    fn default() -> Self {
        PathfindCellType::Clear
    }
}

/// Cell flags enumeration matching C++ PathfindCell::CellFlags
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum PathfindCellFlags {
    NoUnits = 0x00,             // No units in this cell
    UnitGoal = 0x01,            // A unit is heading to this cell
    UnitPresentMoving = 0x02,   // A unit is moving through this cell
    UnitPresentFixed = 0x03,    // A unit is stationary in this cell
    UnitGoalOtherMoving = 0x05, // A unit is moving through this cell, and another unit has this as goal
}

/// PathfindCell structure matching C++ PathfindCell class
#[derive(Debug)]
pub struct PathfindCell {
    // Cell info pointer (managed separately for memory efficiency)
    info: Option<Box<PathfindCellInfo>>,

    // Packed data to match C++ bit fields
    zone: ZoneStorageType,                // Zone (14 bits in C++)
    aircraft_goal: bool,                  // This is an aircraft goal cell (1 bit)
    pinched: bool,                        // This cell is surrounded by obstacle cells (1 bit)
    cell_type: PathfindCellType,          // what type of cell terrain this is (4 bits)
    flags: PathfindCellFlags, // what type of units are in or moving through this cell (4 bits)
    connects_to_layer: PathfindLayerEnum, // This cell can pathfind onto this layer (4 bits)
    layer: PathfindLayerEnum, // Layer of this cell (4 bits)
}

impl PathfindCell {
    /// Create a new PathfindCell
    pub fn new() -> Self {
        Self {
            info: None,
            zone: 0,
            aircraft_goal: false,
            pinched: false,
            cell_type: PathfindCellType::Clear,
            flags: PathfindCellFlags::NoUnits,
            connects_to_layer: PathfindLayerEnum::Invalid,
            layer: PathfindLayerEnum::Ground,
        }
    }

    /// Reset the cell
    pub fn reset(&mut self) {
        self.info = None;
        self.zone = 0;
        self.aircraft_goal = false;
        self.pinched = false;
        self.cell_type = PathfindCellType::Clear;
        self.flags = PathfindCellFlags::NoUnits;
        self.connects_to_layer = PathfindLayerEnum::Invalid;
        self.layer = PathfindLayerEnum::Ground;
    }

    /// Set type as obstacle from the given object
    pub fn set_type_as_obstacle(
        &mut self,
        obstacle: ObjectID,
        is_fence: bool,
        pos: &ICoord2D,
    ) -> bool {
        self.cell_type = PathfindCellType::Obstacle;

        // Allocate info if needed and set obstacle data
        if self.info.is_none() {
            self.info = Some(Box::new(PathfindCellInfo::new(pos)));
        }

        if let Some(ref mut info) = self.info {
            info.set_obstacle_id(obstacle);
            info.set_obstacle_is_fence(is_fence);
        }

        true
    }

    /// Remove obstacle from the given object
    pub fn remove_obstacle(&mut self, obstacle: ObjectID) -> bool {
        if let Some(ref info) = self.info {
            if info.get_obstacle_id() == obstacle {
                self.cell_type = PathfindCellType::Clear;
                if let Some(ref mut info) = self.info {
                    info.set_obstacle_id(INVALID_ID);
                    info.set_obstacle_is_fence(false);
                }
                return true;
            }
        }
        false
    }

    /// Set the cell type
    pub fn set_type(&mut self, cell_type: PathfindCellType) {
        self.cell_type = cell_type;
    }

    /// Get the cell type
    pub fn get_type(&self) -> PathfindCellType {
        self.cell_type
    }

    /// Get the cell flags
    pub fn get_flags(&self) -> PathfindCellFlags {
        self.flags
    }

    /// Check if this is an aircraft goal
    pub fn is_aircraft_goal(&self) -> bool {
        self.aircraft_goal
    }

    /// Check if the given object ID is registered as an obstacle in this cell
    pub fn is_obstacle_present(&self, obj_id: ObjectID) -> bool {
        if obj_id != INVALID_ID && self.cell_type == PathfindCellType::Obstacle {
            if let Some(ref info) = self.info {
                return info.get_obstacle_id() == obj_id;
            }
        }
        false
    }

    /// Return true if the obstacle in the cell is transparent
    pub fn is_obstacle_transparent(&self) -> bool {
        self.info
            .as_ref()
            .map_or(false, |info| info.is_obstacle_transparent())
    }

    /// Return true if the obstacle in the cell is a fence
    pub fn is_obstacle_fence(&self) -> bool {
        self.info
            .as_ref()
            .map_or(false, |info| info.is_obstacle_fence())
    }

    /// Return estimated cost from this cell to reach goal cell
    pub fn cost_to_goal(&self, goal: &PathfindCell) -> u32 {
        if let (Some(ref self_info), Some(ref goal_info)) = (&self.info, &goal.info) {
            let dx = (self_info.get_pos().x - goal_info.get_pos().x).abs();
            let dy = (self_info.get_pos().y - goal_info.get_pos().y).abs();

            // Manhattan distance with diagonal adjustment (A* heuristic)
            let diagonal = dx.min(dy) as u32;
            let straight = (dx.max(dy) - dx.min(dy)) as u32;

            // Diagonal cost is approximately 1.4 * straight cost
            diagonal * 14 + straight * 10
        } else {
            1000 // Default high cost if no info available
        }
    }

    /// Return estimated cost from parent cell to this cell
    pub fn cost_so_far(&self, parent: &PathfindCell) -> u32 {
        if let (Some(ref self_info), Some(ref parent_info)) = (&self.info, &parent.info) {
            let dx = (self_info.get_pos().x - parent_info.get_pos().x).abs();
            let dy = (self_info.get_pos().y - parent_info.get_pos().y).abs();

            // Basic movement cost
            let mut cost = if dx == 1 && dy == 1 {
                14 // Diagonal move
            } else {
                10 // Straight move
            };

            // Add terrain cost modifiers
            cost += match self.cell_type {
                PathfindCellType::Clear => 0,
                PathfindCellType::Water => 5,
                PathfindCellType::Rubble => 15,
                PathfindCellType::Cliff => 20,
                _ => 1000, // Impassable
            };

            cost
        } else {
            10 // Default move cost
        }
    }

    /// Check if blocked by ally
    pub fn is_blocked_by_ally(&self) -> bool {
        self.info
            .as_ref()
            .map_or(false, |info| info.is_blocked_by_ally())
    }

    /// Set blocked by ally
    pub fn set_blocked_by_ally(&mut self, blocked: bool) {
        if self.info.is_none() {
            self.info = Some(Box::new(PathfindCellInfo::new(&ICoord2D::new(0, 0))));
        }
        if let Some(ref mut info) = self.info {
            info.set_blocked_by_ally(blocked);
        }
    }

    /// Get cell zone
    pub fn get_zone(&self) -> ZoneStorageType {
        self.zone
    }

    /// Set cell zone
    pub fn set_zone(&mut self, zone: ZoneStorageType) {
        self.zone = zone;
    }

    /// Get if cell is pinched (surrounded by obstacles)
    pub fn get_pinched(&self) -> bool {
        self.pinched
    }

    /// Set if cell is pinched
    pub fn set_pinched(&mut self, pinched: bool) {
        self.pinched = pinched;
    }

    /// Get cell layer
    pub fn get_layer(&self) -> PathfindLayerEnum {
        self.layer
    }

    /// Set cell layer
    pub fn set_layer(&mut self, layer: PathfindLayerEnum) {
        self.layer = layer;
    }

    /// Get connect layer
    pub fn get_connect_layer(&self) -> PathfindLayerEnum {
        self.connects_to_layer
    }

    /// Set connect layer
    pub fn set_connect_layer(&mut self, layer: PathfindLayerEnum) {
        self.connects_to_layer = layer;
    }

    /// Allocate info structure if needed
    pub fn allocate_info(&mut self, pos: &ICoord2D) -> bool {
        if self.info.is_none() {
            self.info = Some(Box::new(PathfindCellInfo::new(pos)));
            true
        } else {
            false
        }
    }

    /// Release info structure
    pub fn release_info(&mut self) {
        self.info = None;
    }

    /// Check if info structure is allocated
    pub fn has_info(&self) -> bool {
        self.info.is_some()
    }

    /// Get cell info (for A* search)
    pub fn get_info(&self) -> Option<&PathfindCellInfo> {
        self.info.as_ref().map(|info| info.as_ref())
    }

    /// Get mutable cell info (for A* search)
    pub fn get_info_mut(&mut self) -> Option<&mut PathfindCellInfo> {
        self.info.as_mut().map(|info| info.as_mut())
    }

    /// Set goal unit
    pub fn set_goal_unit(&mut self, unit: ObjectID, pos: &ICoord2D) {
        if self.info.is_none() {
            self.info = Some(Box::new(PathfindCellInfo::new(pos)));
        }
        if let Some(ref mut info) = self.info {
            info.set_goal_unit_id(unit);
        }
    }

    /// Set goal aircraft
    pub fn set_goal_aircraft(&mut self, unit: ObjectID, pos: &ICoord2D) {
        if self.info.is_none() {
            self.info = Some(Box::new(PathfindCellInfo::new(pos)));
        }
        if let Some(ref mut info) = self.info {
            info.set_goal_aircraft_id(unit);
        }
        self.aircraft_goal = unit != INVALID_ID;
    }

    /// Set position unit
    pub fn set_pos_unit(&mut self, unit: ObjectID, pos: &ICoord2D) {
        if self.info.is_none() {
            self.info = Some(Box::new(PathfindCellInfo::new(pos)));
        }
        if let Some(ref mut info) = self.info {
            info.set_pos_unit_id(unit);
        }
    }

    /// Get goal unit
    pub fn get_goal_unit(&self) -> ObjectID {
        self.info
            .as_ref()
            .map_or(INVALID_ID, |info| info.get_goal_unit_id())
    }

    /// Get goal aircraft
    pub fn get_goal_aircraft(&self) -> ObjectID {
        self.info
            .as_ref()
            .map_or(INVALID_ID, |info| info.get_goal_aircraft_id())
    }

    /// Get position unit
    pub fn get_pos_unit(&self) -> ObjectID {
        self.info
            .as_ref()
            .map_or(INVALID_ID, |info| info.get_pos_unit_id())
    }

    /// Get obstacle ID
    pub fn get_obstacle_id(&self) -> ObjectID {
        self.info
            .as_ref()
            .map_or(INVALID_ID, |info| info.get_obstacle_id())
    }
}

impl Default for PathfindCell {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pathfind_cell_creation() {
        let cell = PathfindCell::new();
        assert_eq!(cell.get_type(), PathfindCellType::Clear);
        assert_eq!(cell.get_flags(), PathfindCellFlags::NoUnits);
        assert!(!cell.is_aircraft_goal());
        assert!(!cell.get_pinched());
    }

    #[test]
    fn test_pathfind_cell_obstacle() {
        let mut cell = PathfindCell::new();
        let obj_id = 123;
        let pos = ICoord2D::new(10, 20);

        assert!(cell.set_type_as_obstacle(obj_id, false, &pos));
        assert_eq!(cell.get_type(), PathfindCellType::Obstacle);
        assert!(cell.is_obstacle_present(obj_id));
        assert!(!cell.is_obstacle_present(456));

        assert!(cell.remove_obstacle(obj_id));
        assert_eq!(cell.get_type(), PathfindCellType::Clear);
        assert!(!cell.is_obstacle_present(obj_id));
    }

    #[test]
    fn test_pathfind_cell_zone() {
        let mut cell = PathfindCell::new();
        assert_eq!(cell.get_zone(), 0);

        cell.set_zone(42);
        assert_eq!(cell.get_zone(), 42);
    }

    #[test]
    fn test_pathfind_cell_layer() {
        let mut cell = PathfindCell::new();
        assert_eq!(cell.get_layer(), PathfindLayerEnum::Ground);

        cell.set_layer(PathfindLayerEnum::Top);
        assert_eq!(cell.get_layer(), PathfindLayerEnum::Top);
    }
}
