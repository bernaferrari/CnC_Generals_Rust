//! Rally point system for production facilities
//!
//! Manages rally points where newly produced units should move after
//! exiting the production facility.

use crate::common::*;
use std::sync::{Arc, Mutex};

/// Type of rally point
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RallyPointType {
    /// Rally to a specific position
    Position,
    /// Rally to a specific object (follow/guard)
    Object,
    /// Rally to the exit point (default)
    Exit,
}

/// Rally point for newly produced units
#[derive(Debug, Clone)]
pub struct RallyPoint {
    /// Type of rally point
    rally_type: RallyPointType,
    /// Target position (if rally_type is Position)
    position: Option<Coord3D>,
    /// Target object ID (if rally_type is Object)
    target_object: Option<ObjectID>,
    /// Whether units should attack-move to rally point
    attack_move: bool,
    /// Whether units should guard the rally point
    guard_mode: bool,
}

impl RallyPoint {
    /// Create a new rally point at a position
    pub fn at_position(position: Coord3D) -> Self {
        Self {
            rally_type: RallyPointType::Position,
            position: Some(position),
            target_object: None,
            attack_move: false,
            guard_mode: false,
        }
    }

    /// Create a new rally point targeting an object
    pub fn at_object(object_id: ObjectID) -> Self {
        Self {
            rally_type: RallyPointType::Object,
            position: None,
            target_object: Some(object_id),
            attack_move: false,
            guard_mode: false,
        }
    }

    /// Create a default rally point (exit)
    pub fn at_exit() -> Self {
        Self {
            rally_type: RallyPointType::Exit,
            position: None,
            target_object: None,
            attack_move: false,
            guard_mode: false,
        }
    }

    /// Get the rally type
    pub fn rally_type(&self) -> RallyPointType {
        self.rally_type
    }

    /// Get the position if this is a position rally
    pub fn position(&self) -> Option<&Coord3D> {
        self.position.as_ref()
    }

    /// Get the target object if this is an object rally
    pub fn target_object(&self) -> Option<ObjectID> {
        self.target_object
    }

    /// Check if attack-move is enabled
    pub fn is_attack_move(&self) -> bool {
        self.attack_move
    }

    /// Set attack-move mode
    pub fn set_attack_move(&mut self, enabled: bool) {
        self.attack_move = enabled;
    }

    /// Check if guard mode is enabled
    pub fn is_guard_mode(&self) -> bool {
        self.guard_mode
    }

    /// Set guard mode
    pub fn set_guard_mode(&mut self, enabled: bool) {
        self.guard_mode = enabled;
    }

    /// Update the rally point position
    pub fn set_position(&mut self, position: Coord3D) {
        self.rally_type = RallyPointType::Position;
        self.position = Some(position);
        self.target_object = None;
    }

    /// Update the rally point target object
    pub fn set_object(&mut self, object_id: ObjectID) {
        self.rally_type = RallyPointType::Object;
        self.target_object = Some(object_id);
        self.position = None;
    }

    /// Clear the rally point (reset to exit)
    pub fn clear(&mut self) {
        self.rally_type = RallyPointType::Exit;
        self.position = None;
        self.target_object = None;
        self.attack_move = false;
        self.guard_mode = false;
    }

    /// Check if the rally point is valid
    pub fn is_valid(&self) -> bool {
        match self.rally_type {
            RallyPointType::Position => self.position.is_some(),
            RallyPointType::Object => self.target_object.is_some(),
            RallyPointType::Exit => true,
        }
    }
}

impl Default for RallyPoint {
    fn default() -> Self {
        Self::at_exit()
    }
}

/// Manager for multiple rally points (e.g., per unit type)
#[derive(Debug)]
pub struct RallyPointManager {
    /// Default rally point for all units
    default_rally: RallyPoint,
    /// Rally points per unit type (template name -> rally point)
    type_rallies: std::collections::HashMap<String, RallyPoint>,
    /// Whether to use type-specific rallies
    use_type_rallies: bool,
}

impl RallyPointManager {
    /// Create a new rally point manager
    pub fn new() -> Self {
        Self {
            default_rally: RallyPoint::default(),
            type_rallies: std::collections::HashMap::new(),
            use_type_rallies: false,
        }
    }

    /// Set the default rally point
    pub fn set_default(&mut self, rally: RallyPoint) {
        self.default_rally = rally;
    }

    /// Get the default rally point
    pub fn default(&self) -> &RallyPoint {
        &self.default_rally
    }

    /// Get mutable default rally point
    pub fn default_mut(&mut self) -> &mut RallyPoint {
        &mut self.default_rally
    }

    /// Set a rally point for a specific unit type
    pub fn set_type_rally(&mut self, unit_type: String, rally: RallyPoint) {
        self.type_rallies.insert(unit_type, rally);
        self.use_type_rallies = true;
    }

    /// Get the rally point for a specific unit type
    pub fn get_rally(&self, unit_type: Option<&str>) -> &RallyPoint {
        if self.use_type_rallies {
            if let Some(utype) = unit_type {
                if let Some(rally) = self.type_rallies.get(utype) {
                    return rally;
                }
            }
        }
        &self.default_rally
    }

    /// Get mutable rally point for a specific unit type
    pub fn get_rally_mut(&mut self, unit_type: Option<&str>) -> &mut RallyPoint {
        if self.use_type_rallies {
            if let Some(utype) = unit_type {
                if let Some(rally) = self.type_rallies.get_mut(utype) {
                    return rally;
                }
            }
        }
        &mut self.default_rally
    }

    /// Remove type-specific rally point
    pub fn remove_type_rally(&mut self, unit_type: &str) -> Option<RallyPoint> {
        let result = self.type_rallies.remove(unit_type);
        if self.type_rallies.is_empty() {
            self.use_type_rallies = false;
        }
        result
    }

    /// Clear all rally points
    pub fn clear_all(&mut self) {
        self.default_rally.clear();
        self.type_rallies.clear();
        self.use_type_rallies = false;
    }

    /// Check if has type-specific rallies
    pub fn has_type_rallies(&self) -> bool {
        self.use_type_rallies
    }

    /// Get count of type-specific rallies
    pub fn type_rally_count(&self) -> usize {
        self.type_rallies.len()
    }

    /// Toggle use of type-specific rallies
    pub fn set_use_type_rallies(&mut self, enabled: bool) {
        self.use_type_rallies = enabled;
    }
}

impl Default for RallyPointManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rally_point_position() {
        let pos = Coord3D::new(100.0, 200.0, 0.0);
        let rally = RallyPoint::at_position(pos.clone());

        assert_eq!(rally.rally_type(), RallyPointType::Position);
        assert_eq!(rally.position(), Some(&pos));
        assert_eq!(rally.target_object(), None);
        assert!(rally.is_valid());
    }

    #[test]
    fn test_rally_point_object() {
        let target_id = 42;
        let rally = RallyPoint::at_object(target_id);

        assert_eq!(rally.rally_type(), RallyPointType::Object);
        assert_eq!(rally.target_object(), Some(target_id));
        assert_eq!(rally.position(), None);
        assert!(rally.is_valid());
    }

    #[test]
    fn test_rally_point_exit() {
        let rally = RallyPoint::at_exit();

        assert_eq!(rally.rally_type(), RallyPointType::Exit);
        assert_eq!(rally.position(), None);
        assert_eq!(rally.target_object(), None);
        assert!(rally.is_valid());
    }

    #[test]
    fn test_rally_point_modes() {
        let mut rally = RallyPoint::at_position(Coord3D::new(0.0, 0.0, 0.0));

        assert!(!rally.is_attack_move());
        assert!(!rally.is_guard_mode());

        rally.set_attack_move(true);
        assert!(rally.is_attack_move());

        rally.set_guard_mode(true);
        assert!(rally.is_guard_mode());
    }

    #[test]
    fn test_rally_point_update() {
        let mut rally = RallyPoint::at_exit();

        let pos = Coord3D::new(50.0, 75.0, 0.0);
        rally.set_position(pos.clone());

        assert_eq!(rally.rally_type(), RallyPointType::Position);
        assert_eq!(rally.position(), Some(&pos));

        rally.set_object(99);
        assert_eq!(rally.rally_type(), RallyPointType::Object);
        assert_eq!(rally.target_object(), Some(99));
        assert_eq!(rally.position(), None);

        rally.clear();
        assert_eq!(rally.rally_type(), RallyPointType::Exit);
    }

    #[test]
    fn test_rally_manager_default() {
        let mut manager = RallyPointManager::new();

        let pos = Coord3D::new(100.0, 200.0, 0.0);
        manager.set_default(RallyPoint::at_position(pos.clone()));

        let rally = manager.get_rally(None);
        assert_eq!(rally.position(), Some(&pos));

        let rally2 = manager.get_rally(Some("Tank"));
        assert_eq!(rally2.position(), Some(&pos)); // Falls back to default
    }

    #[test]
    fn test_rally_manager_type_specific() {
        let mut manager = RallyPointManager::new();

        let default_pos = Coord3D::new(0.0, 0.0, 0.0);
        manager.set_default(RallyPoint::at_position(default_pos.clone()));

        let tank_pos = Coord3D::new(100.0, 100.0, 0.0);
        manager.set_type_rally(
            "Tank".to_string(),
            RallyPoint::at_position(tank_pos.clone()),
        );

        assert!(manager.has_type_rallies());
        assert_eq!(manager.type_rally_count(), 1);

        // Tank should get its specific rally
        let tank_rally = manager.get_rally(Some("Tank"));
        assert_eq!(tank_rally.position(), Some(&tank_pos));

        // Other units should get default
        let infantry_rally = manager.get_rally(Some("Infantry"));
        assert_eq!(infantry_rally.position(), Some(&default_pos));
    }

    #[test]
    fn test_rally_manager_remove() {
        let mut manager = RallyPointManager::new();

        manager.set_type_rally(
            "Tank".to_string(),
            RallyPoint::at_position(Coord3D::new(100.0, 100.0, 0.0)),
        );

        assert_eq!(manager.type_rally_count(), 1);

        let removed = manager.remove_type_rally("Tank");
        assert!(removed.is_some());
        assert_eq!(manager.type_rally_count(), 0);
        assert!(!manager.has_type_rallies());
    }

    #[test]
    fn test_rally_manager_clear_all() {
        let mut manager = RallyPointManager::new();

        manager.set_default(RallyPoint::at_position(Coord3D::new(50.0, 50.0, 0.0)));
        manager.set_type_rally(
            "Tank".to_string(),
            RallyPoint::at_position(Coord3D::new(100.0, 100.0, 0.0)),
        );

        assert!(manager.has_type_rallies());

        manager.clear_all();

        assert!(!manager.has_type_rallies());
        assert_eq!(manager.type_rally_count(), 0);
        assert_eq!(manager.default().rally_type(), RallyPointType::Exit);
    }
}
