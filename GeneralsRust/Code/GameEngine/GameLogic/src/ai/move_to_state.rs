//! AI MoveTo State - Integrated with Pathfinding and Locomotor
//!
//! This module implements the AI MoveTo state with full pathfinding and locomotor integration.
//! It connects the high-level AI state machine to low-level movement physics.
//!
//! Matches C++ AIStates.cpp::AIMoveToState and AIInternalMoveToState

use crate::common::{
    BodyDamageType as LogicBodyDamageType, Coord3D, ObjectID, Real, Vec3D,
    SECONDS_PER_LOGICFRAME_REAL,
};
use crate::helpers::TheGameLogic;
use crate::state_machine::{StateReturnType, StateExitType};
use crate::ai::ai_states::{AIState, AIStateType, AIStateMachineContext};
use crate::ai::object_registry::OBJECT_REGISTRY;
use crate::ai::THE_AI;
use crate::locomotor::{PathFollowingState, Locomotor, BodyDamageType as LocoBodyDamageType, update_movement_with_pathfinding};
use crate::modules::AIUpdateInterfaceExt;
use std::sync::{Arc, Mutex};

/// AI MoveTo State with integrated pathfinding
///
/// This state handles movement from current position to a goal position by:
/// 1. Requesting a path from the pathfinding system
/// 2. Converting the path to locomotor waypoints
/// 3. Following the path with proper physics
/// 4. Handling obstacles and replanning
/// 5. Detecting arrival at destination
///
/// Matches C++ AIStates.cpp::AIMoveToState (lines 1992-2114)
#[derive(Debug)]
pub struct AIMoveToState {
    /// Path following state (persistent across frames)
    path_following: Option<PathFollowingState>,

    /// Goal position we're moving to
    goal_position: Coord3D,

    /// Whether this is a true "move-to" command (affects behavior)
    is_move_to: bool,

    /// Whether path computation is in progress
    computing_path: bool,
}

impl AIMoveToState {
    pub fn new() -> Self {
        Self {
            path_following: None,
            goal_position: Coord3D::new(0.0, 0.0, 0.0),
            is_move_to: true,
            computing_path: false,
        }
    }

    /// Get unit's current locomotor (would be from object in full implementation)
    fn get_locomotor(&self, context: &AIStateMachineContext) -> Option<Arc<Mutex<Locomotor>>> {
        let ai_handle = OBJECT_REGISTRY.with_object(context.owner_id, |owner_guard| {
            owner_guard.get_ai_update_interface()
        })??;
        ai_handle.lock().ok()?.get_cur_locomotor()
    }

    /// Check if unit has reached destination
    /// Matches C++ AIStates.cpp:2052-2114 update logic
    fn check_destination_reached(
        &self,
        locomotor: &Locomotor,
        current_pos: &Coord3D,
    ) -> bool {
        // Check if close enough to goal
        let delta = *current_pos - self.goal_position;
        let distance = (delta.x * delta.x + delta.y * delta.y).sqrt();

        distance < locomotor.template.close_enough_dist
    }
}

impl AIState for AIMoveToState {
    /// Called when entering MoveTo state
    /// Matches C++ AIStates.cpp:1999-2044 onEnter
    fn on_enter(&mut self, context: &mut AIStateMachineContext) -> StateReturnType {
        // Get goal position from context
        if let Some(goal) = context.goal_position {
            self.goal_position = goal;
        } else {
            // No goal position - fail immediately
            return StateReturnType::Failed;
        }

        // Initialize path following state
        self.path_following = Some(PathFollowingState::new(self.goal_position));
        self.computing_path = true;

        // In full implementation, would:
        // - Stop current movement
        // - Clear old path
        // - Start movement sound
        // - Set model condition to MOVING

        StateReturnType::Continue
    }

    /// Update MoveTo state each frame
    /// Matches C++ AIStates.cpp:2052-2114 update
    fn update(&mut self, context: &mut AIStateMachineContext) -> StateReturnType {
        let Some(ai_handle) = OBJECT_REGISTRY.with_object(context.owner_id, |guard| {
            guard.get_ai_update_interface()
        }).flatten() else {
            return StateReturnType::Failed;
        };

        let locomotor_handle = match self.get_locomotor(context) {
            Some(loco) => loco,
            None => return StateReturnType::Failed,
        };

        let pathfinding = match THE_AI.read().ok().and_then(|ai| ai.pathfinding_system()) {
            Some(system) => system,
            None => return StateReturnType::Failed,
        };

        let mut path_state = match self.path_following.take() {
            Some(state) => state,
            None => return StateReturnType::Failed,
        };

        let Some((current_pos, current_angle, condition)) =
            OBJECT_REGISTRY.with_object(context.owner_id, |guard| {
            let damage_state = guard
                .get_body()
                .and_then(|body| body.lock().ok().map(|b| b.get_damage_state()))
                .unwrap_or(LogicBodyDamageType::Pristine);
            let condition = match damage_state {
                LogicBodyDamageType::Pristine => LocoBodyDamageType::Pristine,
                LogicBodyDamageType::Damaged => LocoBodyDamageType::Damaged,
                LogicBodyDamageType::ReallyDamaged => LocoBodyDamageType::ReallyDamaged,
                LogicBodyDamageType::Rubble => LocoBodyDamageType::Rubble,
            };
            (*guard.get_position(), guard.get_orientation() as f32, condition)
        }) else {
            return StateReturnType::Failed;
        };

        let current_speed = ai_handle.get_speed();
        let desired_speed = ai_handle
            .lock()
            .ok()
            .map(|guard| guard.get_desired_speed())
            .unwrap_or(crate::modules::FAST_AS_POSSIBLE);
        let current_frame = TheGameLogic::get_frame();
        let delta_time = SECONDS_PER_LOGICFRAME_REAL;

        let mut loco_guard = match locomotor_handle.lock() {
            Ok(guard) => guard,
            Err(_) => return StateReturnType::Failed,
        };

        let result = update_movement_with_pathfinding(
            context.owner_id,
            &mut loco_guard,
            &mut path_state,
            &current_pos,
            current_angle,
            current_speed,
            condition,
            desired_speed,
            current_frame,
            delta_time,
            pathfinding,
        );

        self.path_following = Some(path_state);

        match result {
            Ok(Some((new_pos, new_angle, new_speed))) => {
                let _ = OBJECT_REGISTRY.with_object_mut(context.owner_id, |guard| {
                    let _ = guard.set_position(&new_pos);
                    let _ = guard.set_orientation(new_angle as Real);
                    if let Some(physics) = guard.get_physics() {
                        if let Ok(mut phys_guard) = physics.lock() {
                            let delta = new_pos - current_pos;
                            let velocity = if delta_time > 0.0 {
                                delta / delta_time.max(0.0001)
                            } else {
                                Vec3D::ZERO
                            };
                            phys_guard.set_velocity(&velocity);
                            if delta_time > 0.0 {
                                let mut yaw_delta = new_angle - current_angle;
                                let two_pi = std::f32::consts::PI * 2.0;
                                while yaw_delta > std::f32::consts::PI {
                                    yaw_delta -= two_pi;
                                }
                                while yaw_delta < -std::f32::consts::PI {
                                    yaw_delta += two_pi;
                                }
                                phys_guard.set_yaw_rate(
                                    (yaw_delta / delta_time.max(0.0001)) as Real,
                                );
                            }
                        }
                    }
                });
                StateReturnType::Continue
            }
            Ok(None) => StateReturnType::Complete,
            Err(_) => StateReturnType::Failed,
        }
    }

    /// Called when exiting MoveTo state
    /// Matches C++ AIStates.cpp:2046-2050 onExit
    fn on_exit(&mut self, _context: &mut AIStateMachineContext, _exit_type: StateExitType) {
        // In full implementation, would:
        // - Stop movement sounds
        // - Clear MOVING model condition
        // - Report to team manager
        // - Clean up path state

        self.path_following = None;
        self.computing_path = false;
    }

    fn get_state_type(&self) -> AIStateType {
        AIStateType::MoveTo
    }
}

/// AI Attack MoveTo State
///
/// Similar to MoveTo but engages enemies encountered along the way
/// Matches C++ AIStates.cpp::AIAttackMoveToState
#[derive(Debug)]
pub struct AIAttackMoveToState {
    /// Base move state
    move_state: AIMoveToState,

    /// Whether we're currently attacking
    attacking: bool,

    /// Current attack target
    attack_target: Option<ObjectID>,
}

impl AIAttackMoveToState {
    pub fn new() -> Self {
        let mut move_state = AIMoveToState::new();
        move_state.is_move_to = false;  // Attack-move behaves slightly differently

        Self {
            move_state,
            attacking: false,
            attack_target: None,
        }
    }

    /// Scan for enemies within attack range
    fn scan_for_enemies(&mut self, context: &AIStateMachineContext) -> Option<ObjectID> {
        // In full implementation, would:
        // - Query spatial partition for nearby enemies
        // - Check if in weapon range
        // - Select best target based on threat/priority
        None
    }
}

impl AIState for AIAttackMoveToState {
    fn on_enter(&mut self, context: &mut AIStateMachineContext) -> StateReturnType {
        self.move_state.on_enter(context)
    }

    fn update(&mut self, context: &mut AIStateMachineContext) -> StateReturnType {
        // Check for enemies to attack
        if let Some(enemy_id) = self.scan_for_enemies(context) {
            self.attacking = true;
            self.attack_target = Some(enemy_id);

            // In full implementation, would transition to attack state
            // For now, continue moving
        }

        // Continue moving to destination
        self.move_state.update(context)
    }

    fn on_exit(&mut self, context: &mut AIStateMachineContext, exit_type: StateExitType) {
        self.move_state.on_exit(context, exit_type);
        self.attacking = false;
        self.attack_target = None;
    }

    fn get_state_type(&self) -> AIStateType {
        AIStateType::AttackMoveTo
    }

    fn is_attack(&self) -> bool {
        self.attacking
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_move_to_state_creation() {
        let state = AIMoveToState::new();
        assert!(state.is_move_to);
        assert!(state.path_following.is_none());
    }

    #[test]
    fn test_move_to_state_enter() {
        let mut state = AIMoveToState::new();
        let mut context = AIStateMachineContext::default();
        context.goal_position = Some(Coord3D::new(100.0, 100.0, 0.0));

        let result = state.on_enter(&mut context);
        assert_eq!(result, StateReturnType::Continue);
        assert!(state.path_following.is_some());
    }

    #[test]
    fn test_move_to_state_enter_no_goal() {
        let mut state = AIMoveToState::new();
        let mut context = AIStateMachineContext::default();
        context.goal_position = None;

        let result = state.on_enter(&mut context);
        assert_eq!(result, StateReturnType::Failed);
    }

    #[test]
    fn test_attack_move_to_state_creation() {
        let state = AIAttackMoveToState::new();
        assert!(!state.attacking);
        assert!(state.attack_target.is_none());
    }
}
