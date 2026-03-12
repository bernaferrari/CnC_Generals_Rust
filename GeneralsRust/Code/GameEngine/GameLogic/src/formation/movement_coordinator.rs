//! Movement Coordinator
//!
//! Coordinates movement of units in formation, handles speed matching,
//! and integrates with pathfinding systems.

use super::formation_calculator::FormationLayout;
use super::formation_types::{FormationSettings, FormationShape};
use super::{FormationError, FormationResult};
use crate::common::{Coord3D, ObjectID, Real};
use std::collections::HashMap;

/// Movement order for a single unit
#[derive(Debug, Clone)]
pub struct MovementOrder {
    /// Unit to move
    pub unit_id: ObjectID,

    /// Target position
    pub target_position: Coord3D,

    /// Target facing direction
    pub target_facing: Real,

    /// Movement speed
    pub speed: Real,

    /// Priority (0 = highest)
    pub priority: u32,

    /// Is this a formation-keeping move
    pub formation_move: bool,
}

/// Speed matching strategy
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpeedMatchStrategy {
    /// Match slowest unit
    Slowest,
    /// Match average speed
    Average,
    /// Match leader speed
    Leader,
    /// Weighted average by unit importance
    Weighted,
}

/// Speed matcher for formation movement
pub struct SpeedMatcher {
    /// Unit speeds
    unit_speeds: HashMap<ObjectID, Real>,

    /// Speed matching strategy
    strategy: SpeedMatchStrategy,

    /// Cached formation speed
    formation_speed: Option<Real>,

    /// Speed needs recalculation
    dirty: bool,
}

impl SpeedMatcher {
    /// Create new speed matcher
    pub fn new(strategy: SpeedMatchStrategy) -> Self {
        Self {
            unit_speeds: HashMap::new(),
            strategy,
            formation_speed: None,
            dirty: true,
        }
    }

    /// Set unit speed
    pub fn set_unit_speed(&mut self, unit_id: ObjectID, speed: Real) {
        self.unit_speeds.insert(unit_id, speed);
        self.dirty = true;
    }

    /// Remove unit
    pub fn remove_unit(&mut self, unit_id: ObjectID) {
        self.unit_speeds.remove(&unit_id);
        self.dirty = true;
    }

    /// Get formation speed
    pub fn get_formation_speed(&mut self, leader_id: Option<ObjectID>) -> Real {
        if !self.dirty {
            if let Some(speed) = self.formation_speed {
                return speed;
            }
        }

        let speed = self.calculate_formation_speed(leader_id);
        self.formation_speed = Some(speed);
        self.dirty = false;
        speed
    }

    /// Calculate formation speed based on strategy
    fn calculate_formation_speed(&self, leader_id: Option<ObjectID>) -> Real {
        if self.unit_speeds.is_empty() {
            return 100.0; // Default speed
        }

        match self.strategy {
            SpeedMatchStrategy::Slowest => *self
                .unit_speeds
                .values()
                .min_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
                .unwrap_or(&100.0),
            SpeedMatchStrategy::Average => {
                let sum: Real = self.unit_speeds.values().sum();
                sum / self.unit_speeds.len() as Real
            }
            SpeedMatchStrategy::Leader => {
                if let Some(leader_id) = leader_id {
                    *self.unit_speeds.get(&leader_id).unwrap_or(&100.0)
                } else {
                    // Fall back to average
                    let sum: Real = self.unit_speeds.values().sum();
                    sum / self.unit_speeds.len() as Real
                }
            }
            SpeedMatchStrategy::Weighted => {
                // Weight by inverse of speed (slower units get more weight)
                let mut weighted_sum = 0.0;
                let mut weight_sum = 0.0;

                for &speed in self.unit_speeds.values() {
                    let weight = 1.0 / speed.max(1.0);
                    weighted_sum += speed * weight;
                    weight_sum += weight;
                }

                if weight_sum > 0.0 {
                    weighted_sum / weight_sum
                } else {
                    100.0
                }
            }
        }
    }

    /// Get speed factor for a unit (how much to slow down)
    pub fn get_unit_speed_factor(
        &mut self,
        unit_id: ObjectID,
        leader_id: Option<ObjectID>,
    ) -> Real {
        let formation_speed = self.get_formation_speed(leader_id);
        let unit_speed = self.unit_speeds.get(&unit_id).copied().unwrap_or(100.0);

        if unit_speed > 0.0 {
            (formation_speed / unit_speed).min(1.0)
        } else {
            1.0
        }
    }

    /// Clear all data
    pub fn clear(&mut self) {
        self.unit_speeds.clear();
        self.formation_speed = None;
        self.dirty = true;
    }
}

/// Formation pathfinder - calculates paths for formation movement
pub struct FormationPathfinder {
    /// Current formation layout
    layout: Option<FormationLayout>,

    /// Path waypoints for formation center
    center_path: Vec<Coord3D>,

    /// Current waypoint index
    current_waypoint: usize,

    /// Path following threshold
    waypoint_threshold: Real,
}

impl FormationPathfinder {
    /// Create new formation pathfinder
    pub fn new() -> Self {
        Self {
            layout: None,
            center_path: Vec::new(),
            current_waypoint: 0,
            waypoint_threshold: 50.0,
        }
    }

    /// Set formation layout
    pub fn set_layout(&mut self, layout: FormationLayout) {
        self.layout = Some(layout);
    }

    /// Set path for formation center
    pub fn set_path(&mut self, path: Vec<Coord3D>) {
        self.center_path = path;
        self.current_waypoint = 0;
    }

    /// Get current target position for formation center
    pub fn get_current_target(&self) -> Option<Coord3D> {
        self.center_path.get(self.current_waypoint).copied()
    }

    /// Update pathfinding (returns true if waypoint changed)
    pub fn update(&mut self, current_center: &Coord3D) -> bool {
        if let Some(target) = self.get_current_target() {
            let distance = Self::distance_2d(current_center, &target);

            if distance < self.waypoint_threshold {
                // Reached waypoint, advance to next
                if self.current_waypoint + 1 < self.center_path.len() {
                    self.current_waypoint += 1;
                    return true;
                }
            }
        }

        false
    }

    /// Check if path is complete
    pub fn is_path_complete(&self, current_center: &Coord3D) -> bool {
        if self.center_path.is_empty() {
            return true;
        }

        if self.current_waypoint >= self.center_path.len() - 1 {
            if let Some(final_target) = self.center_path.last() {
                let distance = Self::distance_2d(current_center, final_target);
                return distance < self.waypoint_threshold;
            }
        }

        false
    }

    /// Calculate formation heading toward current target
    pub fn calculate_target_heading(&self, current_center: &Coord3D) -> Real {
        if let Some(target) = self.get_current_target() {
            let dx = target.x - current_center.x;
            let dy = target.y - current_center.y;
            dy.atan2(dx)
        } else {
            0.0
        }
    }

    /// Get individual movement targets for all units
    pub fn get_unit_targets(&self) -> Option<&HashMap<ObjectID, Coord3D>> {
        self.layout.as_ref().map(|layout| &layout.positions)
    }

    /// Calculate distance between two 2D points
    fn distance_2d(a: &Coord3D, b: &Coord3D) -> Real {
        let dx = a.x - b.x;
        let dy = a.y - b.y;
        (dx * dx + dy * dy).sqrt()
    }

    /// Clear pathfinding data
    pub fn clear(&mut self) {
        self.center_path.clear();
        self.current_waypoint = 0;
    }
}

impl Default for FormationPathfinder {
    fn default() -> Self {
        Self::new()
    }
}

/// Movement coordinator - coordinates all formation movement
pub struct MovementCoordinator {
    /// Speed matcher
    speed_matcher: SpeedMatcher,

    /// Formation pathfinder
    pathfinder: FormationPathfinder,

    /// Formation settings
    settings: FormationSettings,

    /// Current movement orders
    current_orders: Vec<MovementOrder>,

    /// Leader unit ID
    leader_id: Option<ObjectID>,

    /// Is formation currently moving
    is_moving: bool,
}

impl MovementCoordinator {
    /// Create new movement coordinator
    pub fn new(settings: FormationSettings) -> Self {
        Self {
            speed_matcher: SpeedMatcher::new(SpeedMatchStrategy::Slowest),
            pathfinder: FormationPathfinder::new(),
            settings,
            current_orders: Vec::new(),
            leader_id: None,
            is_moving: false,
        }
    }

    /// Set formation layout
    pub fn set_layout(&mut self, layout: FormationLayout) {
        self.pathfinder.set_layout(layout);
    }

    /// Set path for formation
    pub fn set_path(&mut self, path: Vec<Coord3D>) {
        let is_empty = path.is_empty();
        self.pathfinder.set_path(path);
        self.is_moving = !is_empty;
    }

    /// Set leader
    pub fn set_leader(&mut self, leader_id: ObjectID) {
        self.leader_id = Some(leader_id);
    }

    /// Add unit with speed
    pub fn add_unit(&mut self, unit_id: ObjectID, speed: Real) {
        self.speed_matcher.set_unit_speed(unit_id, speed);
    }

    /// Remove unit
    pub fn remove_unit(&mut self, unit_id: ObjectID) {
        self.speed_matcher.remove_unit(unit_id);
    }

    /// Update movement (returns new orders if needed)
    pub fn update(
        &mut self,
        current_center: &Coord3D,
        current_positions: &HashMap<ObjectID, Coord3D>,
    ) -> Vec<MovementOrder> {
        if !self.is_moving {
            return Vec::new();
        }

        // Update pathfinding
        let waypoint_changed = self.pathfinder.update(current_center);

        // Check if path is complete
        if self.pathfinder.is_path_complete(current_center) {
            self.is_moving = false;
            return Vec::new();
        }

        // Get formation speed
        let formation_speed = self.speed_matcher.get_formation_speed(self.leader_id);

        // Get target positions for units
        let unit_targets = self.pathfinder.get_unit_targets();

        if let Some(targets) = unit_targets {
            self.generate_movement_orders(
                current_positions,
                targets,
                formation_speed,
                waypoint_changed,
            )
        } else {
            Vec::new()
        }
    }

    /// Generate movement orders for units
    fn generate_movement_orders(
        &self,
        current_positions: &HashMap<ObjectID, Coord3D>,
        target_positions: &HashMap<ObjectID, Coord3D>,
        formation_speed: Real,
        force_update: bool,
    ) -> Vec<MovementOrder> {
        let mut orders = Vec::new();

        for (&unit_id, target_pos) in target_positions {
            if let Some(current_pos) = current_positions.get(&unit_id) {
                let distance = Self::distance_3d(current_pos, target_pos);

                // Only issue order if unit is far from target or forced update
                if force_update || distance > self.settings.max_deviation {
                    // Calculate facing toward target
                    let dx = target_pos.x - current_pos.x;
                    let dy = target_pos.y - current_pos.y;
                    let target_facing = dy.atan2(dx);

                    // Get speed factor for this unit
                    let speed_factor = self
                        .speed_matcher
                        .unit_speeds
                        .get(&unit_id)
                        .map(|&s| (formation_speed / s).min(1.0))
                        .unwrap_or(1.0);

                    let adjusted_speed = formation_speed * speed_factor;

                    orders.push(MovementOrder {
                        unit_id,
                        target_position: *target_pos,
                        target_facing,
                        speed: adjusted_speed,
                        priority: if Some(unit_id) == self.leader_id {
                            0
                        } else {
                            1
                        },
                        formation_move: true,
                    });
                }
            }
        }

        orders
    }

    /// Get formation speed
    pub fn get_formation_speed(&mut self) -> Real {
        self.speed_matcher.get_formation_speed(self.leader_id)
    }

    /// Check if formation is moving
    pub fn is_moving(&self) -> bool {
        self.is_moving
    }

    /// Stop movement
    pub fn stop(&mut self) {
        self.is_moving = false;
        self.pathfinder.clear();
        self.current_orders.clear();
    }

    /// Calculate distance between two 3D points
    fn distance_3d(a: &Coord3D, b: &Coord3D) -> Real {
        let dx = a.x - b.x;
        let dy = a.y - b.y;
        let dz = a.z - b.z;
        (dx * dx + dy * dy + dz * dz).sqrt()
    }

    /// Update settings
    pub fn set_settings(&mut self, settings: FormationSettings) {
        self.settings = settings;
    }

    /// Clear all data
    pub fn clear(&mut self) {
        self.speed_matcher.clear();
        self.pathfinder.clear();
        self.current_orders.clear();
        self.is_moving = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_speed_matcher_slowest() {
        let mut matcher = SpeedMatcher::new(SpeedMatchStrategy::Slowest);

        matcher.set_unit_speed(100, 100.0);
        matcher.set_unit_speed(101, 150.0);
        matcher.set_unit_speed(102, 80.0);

        let formation_speed = matcher.get_formation_speed(None);
        assert_eq!(formation_speed, 80.0);
    }

    #[test]
    fn test_speed_matcher_average() {
        let mut matcher = SpeedMatcher::new(SpeedMatchStrategy::Average);

        matcher.set_unit_speed(100, 100.0);
        matcher.set_unit_speed(101, 200.0);

        let formation_speed = matcher.get_formation_speed(None);
        assert_eq!(formation_speed, 150.0);
    }

    #[test]
    fn test_speed_matcher_leader() {
        let mut matcher = SpeedMatcher::new(SpeedMatchStrategy::Leader);

        matcher.set_unit_speed(100, 100.0);
        matcher.set_unit_speed(101, 200.0);

        let formation_speed = matcher.get_formation_speed(Some(100));
        assert_eq!(formation_speed, 100.0);
    }

    #[test]
    fn test_formation_pathfinder() {
        let mut pathfinder = FormationPathfinder::new();

        let path = vec![
            Coord3D::new(0.0, 0.0, 0.0),
            Coord3D::new(100.0, 0.0, 0.0),
            Coord3D::new(200.0, 0.0, 0.0),
        ];

        pathfinder.set_path(path);

        assert_eq!(
            pathfinder.get_current_target(),
            Some(Coord3D::new(0.0, 0.0, 0.0))
        );
        assert!(!pathfinder.is_path_complete(&Coord3D::new(0.0, 0.0, 0.0)));
    }

    #[test]
    fn test_movement_coordinator() {
        let settings = FormationSettings::default();
        let mut coordinator = MovementCoordinator::new(settings);

        coordinator.add_unit(100, 100.0);
        coordinator.add_unit(101, 150.0);

        let formation_speed = coordinator.get_formation_speed();
        assert_eq!(formation_speed, 100.0); // Slowest
    }
}
