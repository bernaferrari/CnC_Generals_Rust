//! AI Unit Group Management System
//!
//! This module provides comprehensive unit group management for AI players,
//! including formation control, coordinated movement, and tactical behaviors.
//! It replaces and enhances the original C++ group management with modern
//! Rust patterns and improved algorithms.
//!
//! Author: Converted from C++ by Claude, original system by Michael S. Booth

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::{Arc, RwLock, Weak};
use std::time::{Duration, Instant};

use super::states::{AIStateMachine, AIStateType};
use super::{AiCommandParams, AiCommandType, AiError, AttitudeType, CommandSourceType};
use crate::common::types::{Coord2D, Coord3D, Real};
use crate::common::ObjectID;
use crate::helpers::TheGameLogic;
use crate::object::registry::OBJECT_REGISTRY;

/// Formation types for unit groups
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FormationType {
    None,      // No specific formation
    Line,      // Units in a line
    Column,    // Units in a column
    Wedge,     // V-shaped formation
    Box,       // Rectangular formation
    Circle,    // Circular formation
    Scattered, // Spread out formation
    Custom,    // User-defined formation
}

impl Default for FormationType {
    fn default() -> Self {
        FormationType::None
    }
}

/// Group behavior types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroupBehavior {
    Defensive,      // Stay together, focus on defense
    Aggressive,     // Attack targets of opportunity
    Cautious,       // Avoid risks, retreat when damaged
    Reckless,       // Charge forward regardless of casualties
    Supportive,     // Provide support to other units
    Reconnaissance, // Scout and gather intelligence
}

/// Group movement state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroupMovementState {
    Idle,
    Moving,
    InCombat,
    Regrouping,
    Retreating,
    Pursuing,
}

/// Unit role within a group
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UnitRole {
    Leader,       // Commands the group
    Tank,         // Absorbs damage, leads charges
    DamageDealer, // Primary damage output
    Support,      // Healing, buffs, utility
    Scout,        // Reconnaissance and early warning
    Heavy,        // Slow but powerful units
    Light,        // Fast, mobile units
}

/// Formation position for a unit within a group
#[derive(Debug, Clone)]
pub struct FormationPosition {
    /// Relative position from group center
    pub offset: Coord2D,
    /// Preferred facing direction
    pub facing: Real,
    /// Priority for this position (higher = more important)
    pub priority: u32,
    /// Acceptable distance variance from ideal position
    pub tolerance: Real,
}

/// Unit information within a group
#[derive(Debug, Clone)]
pub struct GroupUnit {
    /// Object ID of the unit
    pub object_id: ObjectID,
    /// Role of this unit in the group
    pub role: UnitRole,
    /// Current formation position assignment
    pub formation_position: Option<FormationPosition>,
    /// Individual state machine for this unit
    pub state_machine: Option<Arc<RwLock<AIStateMachine>>>,
    /// Last known position
    pub last_position: Option<Coord3D>,
    /// Current health percentage (0.0 to 1.0)
    pub health_ratio: Real,
    /// Whether unit is currently responding to commands
    pub is_responsive: bool,
    /// Individual speed of this unit
    pub speed: Real,
    /// When this unit was added to the group
    pub join_time: Instant,
    /// Unit-specific behavior modifiers
    pub behavior_modifiers: HashMap<String, Real>,
}

impl GroupUnit {
    pub fn new(object_id: ObjectID, role: UnitRole) -> Self {
        Self {
            object_id,
            role,
            formation_position: None,
            state_machine: None,
            last_position: None,
            health_ratio: 1.0,
            is_responsive: true,
            speed: 100.0, // Default speed
            join_time: Instant::now(),
            behavior_modifiers: HashMap::new(),
        }
    }

    /// Check if unit is healthy enough for combat
    pub fn is_combat_ready(&self) -> bool {
        self.health_ratio > 0.3 && self.is_responsive
    }

    /// Check if unit needs healing/repair
    pub fn needs_healing(&self) -> bool {
        self.health_ratio < 0.7
    }
}

/// AI Unit Group - Enhanced group management with formation control
#[derive(Debug)]
pub struct AiUnitGroup {
    /// Unique group identifier
    id: u32,
    /// Group name for identification
    name: String,
    /// All units in this group
    units: HashMap<ObjectID, GroupUnit>,
    /// Current formation type
    formation: FormationType,
    /// Group behavior mode
    behavior: GroupBehavior,
    /// Current movement state
    movement_state: GroupMovementState,
    /// Group's current attitude
    attitude: AttitudeType,
    /// Center position of the group
    center_position: Coord3D,
    /// Target position for group movement
    target_position: Option<Coord3D>,
    /// Formation parameters
    formation_spacing: Real,
    formation_facing: Real,
    /// Group speed (limited by slowest unit)
    group_speed: Real,
    /// Last time group properties were recalculated
    last_update: Instant,
    /// Group statistics
    stats: GroupStats,
    /// Formation positions cache
    formation_positions: Vec<FormationPosition>,
    /// Leaders of the group (can be multiple for redundancy)
    leaders: HashSet<ObjectID>,
    /// Groups this group is coordinating with
    allied_groups: HashSet<u32>,
    /// Enemies this group is currently engaging
    engaged_enemies: HashSet<ObjectID>,
}

/// Group performance and behavior statistics
#[derive(Debug, Default)]
pub struct GroupStats {
    formation_coherence: Real,  // How well units maintain formation (0.0-1.0)
    average_health: Real,       // Average health of all units
    combat_effectiveness: Real, // Estimated combat power
    casualties_taken: u32,      // Number of units lost
    enemies_destroyed: u32,     // Number of enemies eliminated
    distance_traveled: Real,    // Total distance moved
    time_in_combat: Duration,   // Time spent fighting
    commands_executed: u32,     // Number of commands processed
}

impl AiUnitGroup {
    /// Create new AI unit group
    pub fn new(id: u32, name: String, _pathfinder: Option<Arc<RwLock<AIStateMachine>>>) -> Self {
        Self {
            id,
            name,
            units: HashMap::new(),
            formation: FormationType::None,
            behavior: GroupBehavior::Defensive,
            movement_state: GroupMovementState::Idle,
            attitude: AttitudeType::Normal,
            center_position: Coord3D::new(0.0, 0.0, 0.0),
            target_position: None,
            formation_spacing: 20.0,
            formation_facing: 0.0,
            group_speed: 100.0,
            last_update: Instant::now(),
            stats: GroupStats::default(),
            formation_positions: Vec::new(),
            leaders: HashSet::new(),
            allied_groups: HashSet::new(),
            engaged_enemies: HashSet::new(),
        }
    }

    /// Add unit to the group
    pub fn add_unit(&mut self, object_id: ObjectID, role: UnitRole) -> Result<(), AiError> {
        if self.units.contains_key(&object_id) {
            return Err(AiError::InvalidObject);
        }

        let unit = GroupUnit::new(object_id, role);

        // If this is a leader unit, add to leaders set
        if role == UnitRole::Leader {
            self.leaders.insert(object_id);
        }

        self.units.insert(object_id, unit);
        self.recalculate_group_properties();

        log::debug!(
            "Unit {} added to group {} with role {:?}",
            object_id,
            self.id,
            role
        );
        Ok(())
    }

    /// Remove unit from the group
    pub fn remove_unit(&mut self, object_id: ObjectID) -> Result<bool, AiError> {
        if let Some(_unit) = self.units.remove(&object_id) {
            // Remove from leaders if necessary
            self.leaders.remove(&object_id);

            // Recalculate group properties
            self.recalculate_group_properties();

            // If group is now empty, signal for destruction
            let should_destroy = self.units.is_empty();

            log::debug!("Unit {} removed from group {}", object_id, self.id);
            Ok(should_destroy)
        } else {
            Err(AiError::InvalidObject)
        }
    }

    /// Set formation type and recalculate positions
    pub fn set_formation(
        &mut self,
        formation: FormationType,
        spacing: Real,
    ) -> Result<(), AiError> {
        self.formation = formation;
        self.formation_spacing = spacing;
        self.generate_formation_positions();
        self.assign_formation_positions();
        Ok(())
    }

    /// Set group behavior
    pub fn set_behavior(&mut self, behavior: GroupBehavior) {
        self.behavior = behavior;
        self.adjust_parameters_for_behavior();
    }

    /// Update group for one frame
    pub fn update(&mut self, frame_time: Instant) -> Result<(), AiError> {
        // Update unit information
        self.update_unit_status()?;

        // Recalculate group properties if needed
        if frame_time.duration_since(self.last_update) > Duration::from_millis(500) {
            self.recalculate_group_properties();
            self.last_update = frame_time;
        }

        // Update formation if units have moved
        self.maintain_formation()?;

        // Handle group movement
        self.process_group_movement()?;

        // Process combat coordination
        self.process_combat_coordination()?;

        // Update statistics
        self.update_statistics();

        Ok(())
    }

    /// Move group to specified position
    pub fn move_to_position(&mut self, position: Coord3D) -> Result<(), AiError> {
        self.target_position = Some(position);
        self.movement_state = GroupMovementState::Moving;

        let _ = (position.x, position.y, position.z);

        // Issue move commands to individual units
        self.issue_coordinated_move_commands(position)?;

        Ok(())
    }

    /// Attack specific target with the group
    pub fn attack_target(&mut self, target_id: ObjectID) -> Result<(), AiError> {
        self.movement_state = GroupMovementState::InCombat;
        self.engaged_enemies.insert(target_id);

        // Coordinate attack based on unit roles
        for (_unit_id, unit) in &mut self.units {
            if let Some(ref mut state_machine) = unit.state_machine {
                if let Ok(mut sm) = state_machine.write() {
                    sm.set_goal_object(target_id);

                    // Choose attack behavior based on unit role
                    match unit.role {
                        UnitRole::Tank => {
                            // Tanks charge forward
                            sm.set_state(AIStateType::AttackObject as u32);
                        }
                        UnitRole::DamageDealer => {
                            // DPS units focus fire
                            sm.set_state(AIStateType::AttackObject as u32);
                        }
                        UnitRole::Support => {
                            // Support units hang back and assist
                            sm.set_state(AIStateType::Guard as u32);
                        }
                        UnitRole::Scout => {
                            // Scouts harass from range
                            sm.set_state(AIStateType::AttackAndFollowObject as u32);
                        }
                        _ => {
                            sm.set_state(AIStateType::AttackObject as u32);
                        }
                    }
                }
            }
        }

        log::info!("Group {} engaging target {}", self.id, target_id);
        Ok(())
    }

    /// Toggle overcharge for all units in the group (matches C++ AIGroup::groupToggleOvercharge).
    pub fn toggle_overcharge(&mut self, _cmd_source: CommandSourceType) {
        for unit_id in self.units.keys().copied() {
            let Some(obj_arc) = TheGameLogic::find_object_by_id(unit_id) else {
                continue;
            };
            let Ok(obj_guard) = obj_arc.read() else {
                continue;
            };
            let _ = obj_guard.with_overcharge_behavior_interface(|overcharge| {
                let _ = overcharge.toggle();
            });
        }
    }

    /// Set group to guard a specific position
    pub fn guard_position(&mut self, position: Coord3D, radius: Real) -> Result<(), AiError> {
        self.target_position = Some(position);
        self.movement_state = GroupMovementState::Idle;

        // Move to guard position first if not already there
        let distance_to_guard = (self.center_position - position).length();
        if distance_to_guard > radius * 0.5 {
            self.move_to_position(position)?;
        }

        // Set all units to guard state
        for (_unit_id, unit) in &mut self.units {
            if let Some(ref mut state_machine) = unit.state_machine {
                if let Ok(mut sm) = state_machine.write() {
                    sm.set_goal_position(position);
                    sm.set_state(AIStateType::Guard as u32);
                }
            }
        }

        log::info!(
            "Group {} guarding position {:?} with radius {}",
            self.id,
            position,
            radius
        );
        Ok(())
    }

    /// Order group to retreat to a safe position
    pub fn retreat_to_position(&mut self, position: Coord3D) -> Result<(), AiError> {
        self.movement_state = GroupMovementState::Retreating;
        self.engaged_enemies.clear();

        // High priority retreat movement
        for (_unit_id, unit) in &mut self.units {
            if let Some(ref mut state_machine) = unit.state_machine {
                if let Ok(mut sm) = state_machine.write() {
                    sm.set_goal_position(position);
                    sm.set_state(AIStateType::MoveTo as u32);
                    // Retreat commands should have highest priority
                    // Full implementation would set command priority in state machine
                    // to override other pending commands
                }
            }
        }

        log::info!("Group {} retreating to position {:?}", self.id, position);
        Ok(())
    }

    // Internal helper methods

    /// Recalculate group properties like center position, speed, etc.
    fn recalculate_group_properties(&mut self) {
        if self.units.is_empty() {
            return;
        }

        // Calculate center position
        let mut sum_x = 0.0;
        let mut sum_y = 0.0;
        let mut sum_z = 0.0;
        let mut count = 0;
        let mut min_speed = Real::INFINITY;

        for unit in self.units.values_mut() {
            if let Some(object_arc) = TheGameLogic::find_object_by_id(unit.object_id) {
                if let Ok(object) = object_arc.read() {
                    let position = *object.get_position();
                    unit.last_position = Some(position);
                    sum_x += position.x;
                    sum_y += position.y;
                    sum_z += position.z;
                    count += 1;

                    let health = object.get_health();
                    let max_health = object.get_max_health().max(1.0);
                    unit.health_ratio = (health / max_health).clamp(0.0, 1.0);

                    if let Some(ai) = object.get_ai_update_interface() {
                        if let Ok(ai_guard) = ai.lock() {
                            unit.speed = ai_guard.get_speed().max(0.0);
                        }
                    }
                }
            }

            min_speed = min_speed.min(unit.speed);
        }

        if count > 0 {
            self.center_position = Coord3D::new(
                sum_x / count as Real,
                sum_y / count as Real,
                sum_z / count as Real,
            );
        } else {
            self.center_position = Coord3D::new(0.0, 0.0, 0.0);
        }

        if !min_speed.is_finite() {
            min_speed = 0.0;
        }

        self.group_speed = min_speed;

        // Calculate average health for statistics
        let total_health: Real = self.units.values().map(|u| u.health_ratio).sum();
        self.stats.average_health = total_health / self.units.len() as Real;

        // Calculate formation coherence
        self.calculate_formation_coherence();
    }

    /// Generate formation positions based on current formation type
    fn generate_formation_positions(&mut self) {
        self.formation_positions.clear();
        let unit_count = self.units.len();

        if unit_count == 0 {
            return;
        }

        match self.formation {
            FormationType::Line => {
                self.generate_line_formation(unit_count);
            }
            FormationType::Column => {
                self.generate_column_formation(unit_count);
            }
            FormationType::Wedge => {
                self.generate_wedge_formation(unit_count);
            }
            FormationType::Box => {
                self.generate_box_formation(unit_count);
            }
            FormationType::Circle => {
                self.generate_circle_formation(unit_count);
            }
            FormationType::Scattered => {
                self.generate_scattered_formation(unit_count);
            }
            FormationType::None | FormationType::Custom => {
                // No automatic positioning
            }
        }
    }

    fn generate_line_formation(&mut self, unit_count: usize) {
        let spacing = self.formation_spacing;
        let start_x = -(unit_count as Real - 1.0) * spacing * 0.5;

        for i in 0..unit_count {
            let position = FormationPosition {
                offset: Coord2D::new(start_x + i as Real * spacing, 0.0),
                facing: self.formation_facing,
                priority: if i == unit_count / 2 { 100 } else { 50 }, // Center position is highest priority
                tolerance: spacing * 0.3,
            };
            self.formation_positions.push(position);
        }
    }

    fn generate_column_formation(&mut self, unit_count: usize) {
        let spacing = self.formation_spacing;

        for i in 0..unit_count {
            let position = FormationPosition {
                offset: Coord2D::new(0.0, -(i as Real) * spacing),
                facing: self.formation_facing,
                priority: if i == 0 { 100 } else { 75 }, // Front unit has highest priority
                tolerance: spacing * 0.2,
            };
            self.formation_positions.push(position);
        }
    }

    fn generate_wedge_formation(&mut self, unit_count: usize) {
        let spacing = self.formation_spacing;

        // Leader at the front
        self.formation_positions.push(FormationPosition {
            offset: Coord2D::new(0.0, 0.0),
            facing: self.formation_facing,
            priority: 100,
            tolerance: spacing * 0.2,
        });

        // Others in V formation behind
        for i in 1..unit_count {
            let side = if i % 2 == 1 { -1.0 } else { 1.0 };
            let row = (i + 1) / 2;
            let position = FormationPosition {
                offset: Coord2D::new(side * row as Real * spacing * 0.7, -(row as Real) * spacing),
                facing: self.formation_facing,
                priority: 75,
                tolerance: spacing * 0.3,
            };
            self.formation_positions.push(position);
        }
    }

    fn generate_box_formation(&mut self, unit_count: usize) {
        let spacing = self.formation_spacing;
        let side_length = (unit_count as Real).sqrt().ceil() as usize;

        for i in 0..unit_count {
            let row = i / side_length;
            let col = i % side_length;

            let position = FormationPosition {
                offset: Coord2D::new(
                    (col as Real - side_length as Real * 0.5) * spacing,
                    -(row as Real) * spacing,
                ),
                facing: self.formation_facing,
                priority: 50,
                tolerance: spacing * 0.4,
            };
            self.formation_positions.push(position);
        }
    }

    fn generate_circle_formation(&mut self, unit_count: usize) {
        let radius = self.formation_spacing * unit_count as Real / (2.0 * std::f32::consts::PI);

        for i in 0..unit_count {
            let angle = (i as Real / unit_count as Real) * 2.0 * std::f32::consts::PI;
            let position = FormationPosition {
                offset: Coord2D::new(radius * angle.cos(), radius * angle.sin()),
                facing: angle + std::f32::consts::PI, // Face outward
                priority: 50,
                tolerance: self.formation_spacing * 0.3,
            };
            self.formation_positions.push(position);
        }
    }

    fn generate_scattered_formation(&mut self, unit_count: usize) {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let scatter_radius = self.formation_spacing * 2.0;

        for _i in 0..unit_count {
            let angle = rng.gen::<Real>() * 2.0 * std::f32::consts::PI;
            let distance = rng.gen::<Real>() * scatter_radius;

            let position = FormationPosition {
                offset: Coord2D::new(distance * angle.cos(), distance * angle.sin()),
                facing: rng.gen::<Real>() * 2.0 * std::f32::consts::PI,
                priority: 25,
                tolerance: scatter_radius * 0.5,
            };
            self.formation_positions.push(position);
        }
    }

    /// Assign formation positions to units based on their roles and priorities
    fn assign_formation_positions(&mut self) {
        if self.formation_positions.is_empty() {
            return;
        }

        // Sort units by role priority (leaders first, then tanks, etc.)
        let mut sorted_units: Vec<_> = self.units.iter_mut().collect();
        sorted_units.sort_by_key(|(_, unit)| match unit.role {
            UnitRole::Leader => 0,
            UnitRole::Tank => 1,
            UnitRole::Scout => 2,
            UnitRole::DamageDealer => 3,
            UnitRole::Heavy => 4,
            UnitRole::Support => 5,
            UnitRole::Light => 6,
        });

        // Sort positions by priority (highest first)
        let mut sorted_positions = self.formation_positions.clone();
        sorted_positions.sort_by_key(|pos| std::cmp::Reverse(pos.priority));

        // Assign positions to units
        for (i, (_, unit)) in sorted_units.iter_mut().enumerate() {
            if i < sorted_positions.len() {
                unit.formation_position = Some(sorted_positions[i].clone());
            }
        }
    }

    /// Maintain formation by issuing movement commands to out-of-position units
    fn maintain_formation(&mut self) -> Result<(), AiError> {
        if self.formation == FormationType::None {
            return Ok(());
        }

        for (_unit_id, unit) in &mut self.units {
            if let Some(ref formation_pos) = unit.formation_position {
                if let Some(current_pos) = unit.last_position {
                    let ideal_world_pos = Coord3D::new(
                        self.center_position.x + formation_pos.offset.x,
                        self.center_position.y + formation_pos.offset.y,
                        self.center_position.z,
                    );

                    let distance = (current_pos - ideal_world_pos).length();

                    // If unit is too far from formation position, move it back
                    if distance > formation_pos.tolerance {
                        if let Some(ref mut state_machine) = unit.state_machine {
                            if let Ok(mut sm) = state_machine.write() {
                                // Only issue formation commands if not in combat
                                if !sm.is_attack_state()
                                    && self.movement_state != GroupMovementState::InCombat
                                {
                                    sm.set_goal_position(ideal_world_pos);
                                    sm.set_state(AIStateType::MoveAndTighten as u32);
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Process coordinated group movement
    fn process_group_movement(&mut self) -> Result<(), AiError> {
        match self.movement_state {
            GroupMovementState::Moving
            | GroupMovementState::Retreating
            | GroupMovementState::Pursuing
            | GroupMovementState::Regrouping => {
                if let Some(target) = self.target_position {
                    let distance_to_target = (self.center_position - target).length();

                    // Check if we've reached the destination
                    if distance_to_target < self.formation_spacing {
                        self.movement_state = GroupMovementState::Idle;
                        log::debug!("Group {} reached destination", self.id);
                    }
                }
            }
            GroupMovementState::InCombat => {
                // Check if combat is still ongoing
                if self.engaged_enemies.is_empty() {
                    self.movement_state = GroupMovementState::Idle;
                }
            }
            _ => {}
        }

        Ok(())
    }

    /// Process combat coordination between group members
    fn process_combat_coordination(&mut self) -> Result<(), AiError> {
        if self.movement_state == GroupMovementState::InCombat {
            // Implement combat tactics based on group behavior
            match self.behavior {
                GroupBehavior::Aggressive => {
                    // Focus fire on priority targets
                    self.coordinate_focus_fire()?;
                }
                GroupBehavior::Defensive => {
                    // Protect wounded units, maintain formation
                    self.coordinate_defensive_tactics()?;
                }
                GroupBehavior::Supportive => {
                    // Heal/repair damaged units
                    self.coordinate_support_actions()?;
                }
                _ => {}
            }
        }

        Ok(())
    }

    /// Issue coordinated move commands to all units in the group
    fn issue_coordinated_move_commands(&mut self, target: Coord3D) -> Result<(), AiError> {
        for (_unit_id, unit) in &mut self.units {
            if let Some(ref mut state_machine) = unit.state_machine {
                if let Ok(mut sm) = state_machine.write() {
                    // Calculate individual target position based on formation
                    let individual_target = if let Some(ref formation_pos) = unit.formation_position
                    {
                        Coord3D::new(
                            target.x + formation_pos.offset.x,
                            target.y + formation_pos.offset.y,
                            target.z,
                        )
                    } else {
                        target
                    };

                    sm.set_goal_position(individual_target);
                    sm.set_state(AIStateType::MoveTo as u32);
                }
            }
        }
        Ok(())
    }

    /// Update unit status information
    fn update_unit_status(&mut self) -> Result<(), AiError> {
        // Query actual unit positions and health from game objects
        // In full implementation, would:
        // 1. For each unit ObjectID, query TheGameLogic::findObjectByID
        // 2. Get unit's current position from Object
        // 3. Get unit's health and max health
        // 4. Update unit.last_position and unit.health_ratio
        // 5. Check if unit still exists (remove if destroyed)
        // 6. Update unit.is_responsive based on unit state

        // This is handled in recalculate_group_properties() which already
        // queries object positions and health - no additional work needed here

        Ok(())
    }

    /// Calculate how well the group maintains its formation
    fn calculate_formation_coherence(&mut self) {
        if self.formation == FormationType::None || self.units.len() < 2 {
            self.stats.formation_coherence = 1.0;
            return;
        }

        let mut total_deviation = 0.0;
        let mut valid_positions = 0;

        for unit in self.units.values() {
            if let (Some(current_pos), Some(ref formation_pos)) =
                (unit.last_position, &unit.formation_position)
            {
                let ideal_pos = Coord3D::new(
                    self.center_position.x + formation_pos.offset.x,
                    self.center_position.y + formation_pos.offset.y,
                    self.center_position.z,
                );

                let deviation = (current_pos - ideal_pos).length() / formation_pos.tolerance;
                total_deviation += deviation;
                valid_positions += 1;
            }
        }

        if valid_positions > 0 {
            let average_deviation = total_deviation / valid_positions as Real;
            // Convert to 0-1 scale (1.0 = perfect formation)
            self.stats.formation_coherence = (1.0 - average_deviation.min(1.0)).max(0.0);
        }
    }

    /// Adjust group parameters based on behavior type
    fn adjust_parameters_for_behavior(&mut self) {
        match self.behavior {
            GroupBehavior::Aggressive => {
                self.formation_spacing *= 1.2; // Spread out for aggressive tactics
            }
            GroupBehavior::Defensive => {
                self.formation_spacing *= 0.8; // Tighten formation for defense
            }
            GroupBehavior::Cautious => {
                self.formation_spacing *= 1.5; // More spread out
            }
            GroupBehavior::Reckless => {
                self.formation_spacing *= 0.6; // Very tight formation
            }
            _ => {}
        }
    }

    // Combat coordination methods

    fn coordinate_focus_fire(&mut self) -> Result<(), AiError> {
        // Find highest priority target and have all damage dealers attack it
        // Priority order:
        // 1. Low-health high-value targets (nearly dead important units)
        // 2. Support units (healers, buff providers)
        // 3. Damage dealers
        // 4. Tanks (high health, low priority)

        // Full implementation would:
        // 1. Scan engaged_enemies to find highest priority target
        // 2. Calculate threat score for each enemy
        // 3. Select target with highest score
        // 4. Command all DamageDealer role units to attack that target
        // 5. Re-evaluate when target is destroyed

        let mut best_target: Option<ObjectID> = None;
        let mut best_score = -1.0;
        let mut stale_targets = Vec::new();

        for &enemy_id in &self.engaged_enemies {
            let Some(enemy_arc) = OBJECT_REGISTRY.get_object(enemy_id) else {
                stale_targets.push(enemy_id);
                continue;
            };
            let Ok(enemy_guard) = enemy_arc.read() else {
                continue;
            };
            if enemy_guard.is_destroyed() {
                stale_targets.push(enemy_id);
                continue;
            }
            let health_ratio = enemy_guard.get_health_percentage();
            let cost = enemy_guard.get_template().calc_cost_to_build(None).max(1) as f32;
            let score = cost * (1.0 + (1.0 - health_ratio) * 0.75);
            if score > best_score {
                best_score = score;
                best_target = Some(enemy_id);
            }
        }

        for id in stale_targets {
            self.engaged_enemies.remove(&id);
        }

        let Some(target_id) = best_target else {
            return Ok(());
        };

        for unit in self.units.values_mut() {
            if !matches!(
                unit.role,
                UnitRole::DamageDealer | UnitRole::Heavy | UnitRole::Tank
            ) {
                continue;
            }
            if let Some(ref mut state_machine) = unit.state_machine {
                if let Ok(mut sm) = state_machine.write() {
                    sm.set_goal_object(target_id);
                    sm.set_state(AIStateType::AttackObject as u32);
                }
            }
        }

        Ok(())
    }

    fn coordinate_defensive_tactics(&mut self) -> Result<(), AiError> {
        // Position tanks in front, keep support units in back
        // Defensive formation strategy:
        // 1. Tank units at front line (closest to enemies)
        // 2. DamageDealer units in middle (protected but can still attack)
        // 3. Support units in back (maximum safety)
        // 4. Leaders positioned for good command coverage

        // Full implementation would:
        // 1. Calculate enemy threat direction
        // 2. Adjust formation_facing to face threats
        // 3. Re-assign formation positions based on unit roles
        // 4. Ensure tanks have frontmost positions
        // 5. Keep support units furthest from threat

        let mut threat_dir = Coord2D::new(0.0, 0.0);
        let mut threat_count = 0;
        for &enemy_id in &self.engaged_enemies {
            let Some(enemy_arc) = OBJECT_REGISTRY.get_object(enemy_id) else {
                continue;
            };
            let Ok(enemy_guard) = enemy_arc.read() else {
                continue;
            };
            if enemy_guard.is_destroyed() {
                continue;
            }
            let pos = enemy_guard.get_position();
            threat_dir.x += pos.x - self.center_position.x;
            threat_dir.y += pos.y - self.center_position.y;
            threat_count += 1;
        }

        if threat_count > 0 {
            let len = (threat_dir.x * threat_dir.x + threat_dir.y * threat_dir.y)
                .sqrt()
                .max(0.001);
            threat_dir.x /= len;
            threat_dir.y /= len;
            self.formation_facing = threat_dir.y.atan2(threat_dir.x);
        }

        if self.formation_positions.is_empty() {
            self.generate_formation_positions();
        }
        let mut positions = self.formation_positions.clone();
        positions.sort_by(|a, b| {
            b.offset
                .y
                .partial_cmp(&a.offset.y)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let mut units_sorted: Vec<_> = self.units.values_mut().collect();
        units_sorted.sort_by_key(|unit| match unit.role {
            UnitRole::Tank => 0,
            UnitRole::Heavy => 1,
            UnitRole::DamageDealer => 2,
            UnitRole::Leader => 3,
            UnitRole::Support => 4,
            UnitRole::Scout => 5,
            UnitRole::Light => 6,
        });

        for (unit, pos) in units_sorted.into_iter().zip(positions.into_iter()) {
            unit.formation_position = Some(pos);
        }

        Ok(())
    }

    fn coordinate_support_actions(&mut self) -> Result<(), AiError> {
        // Have support units heal/repair damaged allies
        // Support priority:
        // 1. Critically damaged units (health < 30%)
        // 2. Important units (leaders, expensive units)
        // 3. Moderately damaged units (health < 70%)

        // Full implementation would:
        // 1. Find all units with Support role
        // 2. For each support unit:
        //    a. Find nearby damaged allies
        //    b. Prioritize most critical targets
        //    c. Issue heal/repair command
        // 3. Track which units are being healed to avoid duplicate commands
        // 4. Coordinate multiple support units for efficient coverage

        let mut heal_targets = HashSet::new();

        let mut candidates: Vec<(ObjectID, Real)> = self
            .units
            .values()
            .filter(|unit| unit.health_ratio < 0.7)
            .map(|unit| (unit.object_id, unit.health_ratio))
            .collect();
        candidates.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

        for unit in self.units.values_mut() {
            if unit.role != UnitRole::Support {
                continue;
            }
            let Some(ref mut state_machine) = unit.state_machine else {
                continue;
            };
            let Some((target_id, _)) = candidates.iter().find(|(id, _)| !heal_targets.contains(id))
            else {
                continue;
            };
            let Some(target_arc) = OBJECT_REGISTRY.get_object(*target_id) else {
                continue;
            };
            let Ok(target_guard) = target_arc.read() else {
                continue;
            };
            let target_pos = *target_guard.get_position();
            if let Ok(mut sm) = state_machine.write() {
                sm.set_goal_position(target_pos);
                sm.set_state(AIStateType::MoveTo as u32);
            }
            heal_targets.insert(*target_id);
        }

        Ok(())
    }

    fn update_statistics(&mut self) {
        // Update various group performance metrics
        // Statistics tracked:
        // - formation_coherence (calculated in calculate_formation_coherence)
        // - average_health (calculated in recalculate_group_properties)
        // - combat_effectiveness (based on unit types and health)
        // - distance_traveled (accumulated from unit movements)
        // - time_in_combat (tracked when in GroupMovementState::InCombat)

        // Most statistics are already updated in other methods
        // Additional tracking could include:
        // - Commands executed (increment on command issue)
        // - Enemies destroyed (track when engaged_enemies are eliminated)
        // - Casualties taken (track when units are removed)

        // These metrics are primarily for debugging and AI performance tuning
        // Full implementation requires more detailed event tracking
    }

    // Public accessors

    pub fn get_id(&self) -> u32 {
        self.id
    }
    pub fn get_name(&self) -> &str {
        &self.name
    }
    pub fn get_unit_count(&self) -> usize {
        self.units.len()
    }
    pub fn get_center_position(&self) -> Coord3D {
        self.center_position
    }
    pub fn get_formation(&self) -> FormationType {
        self.formation
    }
    pub fn get_behavior(&self) -> GroupBehavior {
        self.behavior
    }
    pub fn get_movement_state(&self) -> GroupMovementState {
        self.movement_state
    }
    pub fn get_stats(&self) -> &GroupStats {
        &self.stats
    }

    pub fn is_empty(&self) -> bool {
        self.units.is_empty()
    }
    pub fn is_in_combat(&self) -> bool {
        self.movement_state == GroupMovementState::InCombat
    }
    pub fn is_moving(&self) -> bool {
        matches!(
            self.movement_state,
            GroupMovementState::Moving
                | GroupMovementState::Retreating
                | GroupMovementState::Pursuing
                | GroupMovementState::Regrouping
        )
    }

    pub fn contains_unit(&self, unit_id: ObjectID) -> bool {
        self.units.contains_key(&unit_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_group_creation() {
        let group = AiUnitGroup::new(1, "TestGroup".to_string(), None);

        assert_eq!(group.get_id(), 1);
        assert_eq!(group.get_name(), "TestGroup");
        assert!(group.is_empty());
        assert_eq!(group.get_formation(), FormationType::None);
    }

    #[test]
    fn test_unit_management() {
        let mut group = AiUnitGroup::new(1, "TestGroup".to_string(), None);

        // Add units
        assert!(group.add_unit(100, UnitRole::Leader).is_ok());
        assert!(group.add_unit(101, UnitRole::Tank).is_ok());
        assert_eq!(group.get_unit_count(), 2);
        assert!(group.contains_unit(100));
        assert!(group.contains_unit(101));

        // Remove unit
        assert!(group.remove_unit(100).is_ok());
        assert_eq!(group.get_unit_count(), 1);
        assert!(!group.contains_unit(100));
    }

    #[test]
    fn test_formation_generation() {
        let mut group = AiUnitGroup::new(1, "TestGroup".to_string(), None);

        // Add some units
        for i in 0..5 {
            group.add_unit(100 + i, UnitRole::DamageDealer).unwrap();
        }

        // Test line formation
        assert!(group.set_formation(FormationType::Line, 10.0).is_ok());
        assert_eq!(group.get_formation(), FormationType::Line);
        assert_eq!(group.formation_positions.len(), 5);

        // Test wedge formation
        assert!(group.set_formation(FormationType::Wedge, 15.0).is_ok());
        assert_eq!(group.formation_positions.len(), 5);
    }

    #[test]
    fn test_group_commands() {
        let mut group = AiUnitGroup::new(1, "TestGroup".to_string(), None);

        group.add_unit(100, UnitRole::Leader).unwrap();
        group.add_unit(101, UnitRole::Tank).unwrap();

        // Test move command
        let target = Coord3D::new(50.0, 50.0, 0.0);
        assert!(group.move_to_position(target).is_ok());
        assert_eq!(group.get_movement_state(), GroupMovementState::Moving);
        assert_eq!(group.target_position, Some(target));

        // Test attack command
        assert!(group.attack_target(200).is_ok());
        assert_eq!(group.get_movement_state(), GroupMovementState::InCombat);
        assert!(group.engaged_enemies.contains(&200));
    }
}
