//! Formation Manager
//!
//! Central manager for all formation operations. Coordinates formation
//! creation, updates, movement, combat, and lifecycle management.

use super::combat_integration::{CombatBehavior, CombatState, FormationCombat, FormationTactics};
use super::formation_calculator::{FormationCalculator, FormationLayout};
use super::formation_types::*;
use super::leader_follower::{FollowerRole, LeaderFollowerSystem, LeaderSelection};
use super::movement_coordinator::{MovementCoordinator, MovementOrder};
use super::{FormationError, FormationResult, MAX_FORMATION_SIZE, MIN_FORMATION_SIZE};
use crate::common::{Coord3D, ObjectID, Real};
use crate::helpers::TheGameLogic;
use std::collections::HashMap;

/// Formation member information
#[derive(Debug, Clone)]
pub struct FormationMember {
    /// Unit ID
    pub unit_id: ObjectID,

    /// Current position
    pub position: Coord3D,

    /// Target position in formation
    pub target_position: Coord3D,

    /// Unit movement speed
    pub speed: Real,

    /// Unit health percentage
    pub health: Real,

    /// Is unit in combat
    pub in_combat: bool,

    /// Unit rank/veterancy
    pub rank: u32,
}

/// Formation command types
#[derive(Debug, Clone)]
pub enum FormationCommand {
    /// Move formation to position
    MoveTo(Coord3D),

    /// Set formation type
    SetFormation(FormationType),

    /// Break formation
    Break,

    /// Reform formation
    Reform,

    /// Set leader
    SetLeader(ObjectID),

    /// Set tactics
    SetTactics(FormationTactics),

    /// Add unit
    AddUnit(ObjectID),

    /// Remove unit
    RemoveUnit(ObjectID),

    /// Stop movement
    Stop,
}

/// Formation group - represents an active formation
pub struct FormationGroup {
    /// Unique formation ID
    id: u32,

    /// Formation type
    formation_type: FormationType,

    /// Formation state
    state: FormationState,

    /// Formation settings
    settings: FormationSettings,

    /// Members
    members: HashMap<ObjectID, FormationMember>,

    /// Formation calculator
    calculator: FormationCalculator,

    /// Current layout
    layout: Option<FormationLayout>,

    /// Leader-follower system
    leader_follower: LeaderFollowerSystem,

    /// Movement coordinator
    movement: MovementCoordinator,

    /// Combat integration
    combat: FormationCombat,

    /// Formation center
    center: Coord3D,

    /// Formation heading
    heading: Real,

    /// Creation frame
    created_frame: u32,

    /// Last update frame
    last_update_frame: u32,

    /// Player owner
    owner_player: i32,
}

impl FormationGroup {
    /// Create new formation group
    pub fn new(
        id: u32,
        formation_type: FormationType,
        settings: FormationSettings,
        owner_player: i32,
        frame: u32,
    ) -> Self {
        let leader_follower = LeaderFollowerSystem::new(LeaderSelection::Automatic);
        let movement = MovementCoordinator::new(settings.clone());
        let combat = FormationCombat::new(CombatBehavior::default());

        Self {
            id,
            formation_type,
            state: FormationState::Forming,
            settings,
            members: HashMap::new(),
            calculator: FormationCalculator::new(),
            layout: None,
            leader_follower,
            movement,
            combat,
            center: Coord3D::new(0.0, 0.0, 0.0),
            heading: 0.0,
            created_frame: frame,
            last_update_frame: frame,
            owner_player,
        }
    }

    /// Add unit to formation
    pub fn add_unit(
        &mut self,
        unit_id: ObjectID,
        position: Coord3D,
        speed: Real,
        health: Real,
        rank: u32,
    ) -> FormationResult<()> {
        if self.members.len() >= MAX_FORMATION_SIZE {
            return Err(FormationError::FormationFull);
        }

        if self.members.contains_key(&unit_id) {
            return Ok(()); // Already in formation
        }

        // Add to members
        self.members.insert(
            unit_id,
            FormationMember {
                unit_id,
                position,
                target_position: position,
                speed,
                health,
                in_combat: false,
                rank,
            },
        );

        // Add to subsystems
        self.leader_follower.set_unit_quality(unit_id, health);
        self.leader_follower.set_unit_rank(unit_id, rank);
        self.movement.add_unit(unit_id, speed);
        self.combat.add_unit(unit_id, health);

        // Recalculate formation
        self.recalculate_formation()?;

        Ok(())
    }

    /// Remove unit from formation
    pub fn remove_unit(&mut self, unit_id: ObjectID) -> FormationResult<bool> {
        if self.members.remove(&unit_id).is_none() {
            return Ok(false);
        }

        // Remove from subsystems
        self.leader_follower.remove_follower(unit_id);
        self.movement.remove_unit(unit_id);
        self.combat.remove_unit(unit_id);

        // Check if formation is still viable
        if self.members.len() < MIN_FORMATION_SIZE {
            self.state = FormationState::Disbanded;
            return Ok(true); // Formation should be disbanded
        }

        // Check if we need a new leader
        if self.leader_follower.get_leader_id() == Some(unit_id) {
            self.select_new_leader()?;
        }

        // Recalculate formation
        self.recalculate_formation()?;

        Ok(false)
    }

    /// Update formation (called every frame)
    pub fn update(&mut self, frame: u32) -> FormationResult<Vec<MovementOrder>> {
        self.last_update_frame = frame;

        // Update subsystems
        self.leader_follower.update(frame);
        self.combat.update(frame)?;

        // Update formation center
        self.update_center();

        // Handle state transitions
        self.update_state()?;

        // Update movement if formation is moving
        let mut orders = Vec::new();
        if self.state == FormationState::Moving {
            let current_positions = self.get_unit_positions();
            orders = self.movement.update(&self.center, &current_positions);
        }

        // Check if leadership needs reconsidering
        if self.leader_follower.should_reconsider_leadership() {
            self.select_new_leader()?;
        }

        Ok(orders)
    }

    /// Update formation state based on conditions
    fn update_state(&mut self) -> FormationResult<()> {
        let combat_state = self.combat.get_combat_state();

        let new_state = match self.state {
            FormationState::Forming => {
                // Check if formation is established
                if self.is_formation_established() {
                    FormationState::Formed
                } else {
                    FormationState::Forming
                }
            }
            FormationState::Formed => {
                // Check for movement or combat
                if self.movement.is_moving() {
                    FormationState::Moving
                } else if combat_state != CombatState::Idle {
                    self.combat.save_pre_combat_state(FormationState::Formed);
                    FormationState::InCombat
                } else {
                    FormationState::Formed
                }
            }
            FormationState::Moving => {
                // Check for combat or completion
                if combat_state != CombatState::Idle {
                    self.combat.save_pre_combat_state(FormationState::Moving);
                    FormationState::InCombat
                } else if !self.movement.is_moving() {
                    FormationState::Formed
                } else {
                    FormationState::Moving
                }
            }
            FormationState::InCombat => {
                if self.combat.should_break_formation() {
                    FormationState::Breaking
                } else if combat_state == CombatState::Idle {
                    if self.combat.should_reform_formation() {
                        FormationState::Reforming
                    } else {
                        FormationState::Formed
                    }
                } else {
                    FormationState::InCombat
                }
            }
            FormationState::Breaking => {
                if combat_state == CombatState::Idle {
                    FormationState::Reforming
                } else {
                    FormationState::Breaking
                }
            }
            FormationState::Reforming => {
                if self.is_formation_established() {
                    FormationState::Formed
                } else {
                    FormationState::Reforming
                }
            }
            FormationState::Disbanded => FormationState::Disbanded,
        };

        self.state = new_state;
        Ok(())
    }

    /// Check if formation is established (units in position)
    fn is_formation_established(&self) -> bool {
        if let Some(ref layout) = self.layout {
            let current_positions = self.get_unit_positions();
            let coherence = FormationCalculator::check_formation_coherence(
                layout,
                &current_positions,
                self.settings.max_deviation,
            );
            coherence > 0.8 // 80% of units in position
        } else {
            false
        }
    }

    /// Execute formation command
    pub fn execute_command(&mut self, command: FormationCommand) -> FormationResult<()> {
        match command {
            FormationCommand::MoveTo(target) => {
                self.move_to(target)?;
            }
            FormationCommand::SetFormation(formation_type) => {
                self.set_formation_type(formation_type)?;
            }
            FormationCommand::Break => {
                self.break_formation();
            }
            FormationCommand::Reform => {
                self.reform_formation()?;
            }
            FormationCommand::SetLeader(unit_id) => {
                self.set_leader(unit_id)?;
            }
            FormationCommand::SetTactics(tactics) => {
                self.combat.set_tactics(tactics);
            }
            FormationCommand::AddUnit(_unit_id) => {
                let Some(obj_arc) = crate::helpers::TheGameLogic::find_object_by_id(_unit_id)
                else {
                    return Err(FormationError::InvalidFormationType);
                };
                let obj_guard = obj_arc
                    .read()
                    .map_err(|_| FormationError::InvalidFormationType)?;

                let position = *obj_guard.get_position();
                let speed = obj_guard
                    .get_physics()
                    .and_then(|phys| phys.lock().ok().map(|p| p.get_velocity().length()))
                    .unwrap_or(0.0);
                let health = obj_guard.get_health_percentage();
                let rank = obj_guard.get_veterancy_level() as u32;

                self.add_unit(_unit_id, position, speed, health, rank)?;
            }
            FormationCommand::RemoveUnit(unit_id) => {
                self.remove_unit(unit_id)?;
            }
            FormationCommand::Stop => {
                self.movement.stop();
                self.state = FormationState::Formed;
            }
        }
        Ok(())
    }

    /// Move formation to target
    fn move_to(&mut self, target: Coord3D) -> FormationResult<()> {
        if self.state == FormationState::Disbanded {
            return Err(FormationError::InvalidFormationType);
        }

        // Calculate heading toward target
        self.heading = FormationCalculator::calculate_heading_to_target(&self.center, &target);

        // Recalculate formation for movement
        self.recalculate_formation()?;

        // Set path (simple direct path for now)
        let path = vec![self.center, target];
        self.movement.set_path(path);

        self.state = FormationState::Moving;
        Ok(())
    }

    /// Set formation type
    fn set_formation_type(&mut self, formation_type: FormationType) -> FormationResult<()> {
        if self.formation_type != formation_type {
            self.formation_type = formation_type;
            self.recalculate_formation()?;
        }
        Ok(())
    }

    /// Break formation
    fn break_formation(&mut self) {
        self.state = FormationState::Breaking;
        self.movement.stop();
    }

    /// Reform formation
    fn reform_formation(&mut self) -> FormationResult<()> {
        self.state = FormationState::Reforming;
        self.recalculate_formation()?;
        Ok(())
    }

    /// Set leader explicitly
    fn set_leader(&mut self, unit_id: ObjectID) -> FormationResult<()> {
        if !self.members.contains_key(&unit_id) {
            return Err(FormationError::UnitNotInFormation);
        }

        let member = self.members.get(&unit_id).unwrap();
        self.leader_follower
            .set_leader(unit_id, member.position, self.heading, member.speed)?;

        Ok(())
    }

    /// Select new leader automatically
    fn select_new_leader(&mut self) -> FormationResult<()> {
        let unit_ids: Vec<ObjectID> = self.members.keys().copied().collect();
        let leader_id = self.leader_follower.select_leader(&unit_ids)?;

        if let Some(member) = self.members.get(&leader_id) {
            self.leader_follower.set_leader(
                leader_id,
                member.position,
                self.heading,
                member.speed,
            )?;
        }

        Ok(())
    }

    /// Recalculate formation layout
    fn recalculate_formation(&mut self) -> FormationResult<()> {
        let unit_positions = self.get_unit_positions();

        if unit_positions.is_empty() {
            return Ok(());
        }

        // Create new layout
        let layout = self.calculator.create_layout(
            self.formation_type,
            &unit_positions,
            self.heading,
            Some(self.settings.max_deviation),
        )?;

        // Update member target positions
        for (&unit_id, &target_pos) in &layout.positions {
            if let Some(member) = self.members.get_mut(&unit_id) {
                member.target_position = target_pos;
            }
        }

        // Update movement coordinator
        self.movement.set_layout(layout.clone());

        self.layout = Some(layout);
        Ok(())
    }

    /// Update formation center
    fn update_center(&mut self) {
        if self.members.is_empty() {
            return;
        }

        let mut sum_x = 0.0;
        let mut sum_y = 0.0;
        let mut sum_z = 0.0;
        let count = self.members.len() as Real;

        for member in self.members.values() {
            sum_x += member.position.x;
            sum_y += member.position.y;
            sum_z += member.position.z;
        }

        self.center = Coord3D::new(sum_x / count, sum_y / count, sum_z / count);
    }

    /// Get unit positions map
    fn get_unit_positions(&self) -> HashMap<ObjectID, Coord3D> {
        self.members
            .iter()
            .map(|(&id, member)| (id, member.position))
            .collect()
    }

    /// Get formation ID
    pub fn get_id(&self) -> u32 {
        self.id
    }

    /// Get formation state
    pub fn get_state(&self) -> FormationState {
        self.state
    }

    /// Get formation type
    pub fn get_formation_type(&self) -> FormationType {
        self.formation_type
    }

    /// Get member count
    pub fn get_member_count(&self) -> usize {
        self.members.len()
    }

    /// Get formation center
    pub fn get_center(&self) -> Coord3D {
        self.center
    }

    /// Get formation heading
    pub fn get_heading(&self) -> Real {
        self.heading
    }

    /// Is formation viable
    pub fn is_viable(&self) -> bool {
        self.members.len() >= MIN_FORMATION_SIZE && self.state != FormationState::Disbanded
    }

    /// Update unit status
    pub fn update_unit_status(
        &mut self,
        unit_id: ObjectID,
        position: Coord3D,
        health: Real,
        in_combat: bool,
    ) -> FormationResult<()> {
        if let Some(member) = self.members.get_mut(&unit_id) {
            member.position = position;
            member.health = health;
            member.in_combat = in_combat;

            // Update combat system
            self.combat
                .update_unit_combat(unit_id, in_combat, health, None);

            Ok(())
        } else {
            Err(FormationError::UnitNotInFormation)
        }
    }
}

/// Formation Manager - manages all active formations
pub struct FormationManager {
    /// Active formations
    formations: HashMap<u32, FormationGroup>,

    /// Next formation ID
    next_id: u32,

    /// Unit to formation mapping
    unit_to_formation: HashMap<ObjectID, u32>,

    /// Current frame
    current_frame: u32,
}

impl FormationManager {
    /// Create new formation manager
    pub fn new() -> Self {
        Self {
            formations: HashMap::new(),
            next_id: 1,
            unit_to_formation: HashMap::new(),
            current_frame: 0,
        }
    }

    /// Create new formation
    pub fn create_formation(
        &mut self,
        formation_type: FormationType,
        settings: FormationSettings,
        owner_player: i32,
    ) -> u32 {
        let id = self.next_id;
        self.next_id += 1;

        let formation = FormationGroup::new(
            id,
            formation_type,
            settings,
            owner_player,
            self.current_frame,
        );

        self.formations.insert(id, formation);
        id
    }

    /// Get formation
    pub fn get_formation(&self, formation_id: u32) -> Option<&FormationGroup> {
        self.formations.get(&formation_id)
    }

    /// Get formation (mutable)
    pub fn get_formation_mut(&mut self, formation_id: u32) -> Option<&mut FormationGroup> {
        self.formations.get_mut(&formation_id)
    }

    /// Delete formation
    pub fn delete_formation(&mut self, formation_id: u32) -> bool {
        if let Some(formation) = self.formations.remove(&formation_id) {
            // Clear unit mappings
            for unit_id in formation.members.keys() {
                self.unit_to_formation.remove(unit_id);
            }
            true
        } else {
            false
        }
    }

    /// Find formation containing unit
    pub fn find_formation_for_unit(&self, unit_id: ObjectID) -> Option<u32> {
        self.unit_to_formation.get(&unit_id).copied()
    }

    /// Update all formations
    pub fn update(&mut self, frame: u32) -> HashMap<u32, Vec<MovementOrder>> {
        self.current_frame = frame;

        let mut all_orders = HashMap::new();
        let mut formations_to_remove = Vec::new();

        for (&formation_id, formation) in &mut self.formations {
            match formation.update(frame) {
                Ok(orders) => {
                    if !orders.is_empty() {
                        all_orders.insert(formation_id, orders);
                    }

                    // Mark for removal if disbanded
                    if !formation.is_viable() {
                        formations_to_remove.push(formation_id);
                    }
                }
                Err(_) => {
                    formations_to_remove.push(formation_id);
                }
            }
        }

        // Remove disbanded formations
        for formation_id in formations_to_remove {
            self.delete_formation(formation_id);
        }

        all_orders
    }

    /// Get active formation count
    pub fn get_formation_count(&self) -> usize {
        self.formations.len()
    }

    /// Clear all formations
    pub fn clear(&mut self) {
        self.formations.clear();
        self.unit_to_formation.clear();
    }
}

impl Default for FormationManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_formation_manager() {
        let mut manager = FormationManager::new();
        let settings = FormationSettings::default();

        let formation_id = manager.create_formation(FormationType::Line, settings, 1);
        assert!(manager.get_formation(formation_id).is_some());
    }

    #[test]
    fn test_formation_group() {
        let settings = FormationSettings::default();
        let mut formation = FormationGroup::new(1, FormationType::Line, settings, 1, 0);

        formation
            .add_unit(100, Coord3D::new(0.0, 0.0, 0.0), 100.0, 1.0, 0)
            .unwrap();
        formation
            .add_unit(101, Coord3D::new(10.0, 0.0, 0.0), 100.0, 1.0, 0)
            .unwrap();

        assert_eq!(formation.get_member_count(), 2);
        assert!(formation.is_viable());
    }

    #[test]
    fn test_formation_movement() {
        let settings = FormationSettings::default();
        let mut formation = FormationGroup::new(1, FormationType::Line, settings, 1, 0);

        formation
            .add_unit(100, Coord3D::new(0.0, 0.0, 0.0), 100.0, 1.0, 0)
            .unwrap();
        formation
            .add_unit(101, Coord3D::new(10.0, 0.0, 0.0), 100.0, 1.0, 0)
            .unwrap();

        formation
            .execute_command(FormationCommand::MoveTo(Coord3D::new(100.0, 100.0, 0.0)))
            .unwrap();

        assert_eq!(formation.get_state(), FormationState::Moving);
    }
}
