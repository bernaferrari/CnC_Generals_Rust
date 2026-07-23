//! AIStates - Complete AI state machine implementation
//!
//! This module implements the complete AI state machine system from the C++ original.
//! It provides all AI behavior states including movement, combat, guarding, pathfinding,
//! and complex tactical behaviors. The state machine drives all AI unit behavior.
//!
//! Author: Converted from C++ original by Michael S. Booth

use crate::ai::dock::AIDockMachine;
use crate::ai::formations::{
    calculate_group_spread, is_group_too_spread, FormationConfig, FormationType,
};
use crate::ai::guard::{AIGuardMachine, GuardStateType};
use crate::ai::guard_retaliate::{AIGuardRetaliateMachine, GuardRetaliateStateType};
use crate::ai::integration::with_ai_integration;
use crate::ai::object_registry::get_legacy_object;
use crate::path::PATHFIND_CLOSE_ENOUGH;
use crate::ai::states::{AIAttackThenIdleStateMachine, AIStateType as LegacyAIStateType};
use crate::ai::tn_guard::{AITNGuardMachine, TNGuardStateType};
use crate::ai::GuardMode;
use crate::ai::{resolve_attack_priority_info_for_object, search_qualifiers, THE_AI};
use crate::ai::{AiCommandInterface, AiCommandParams, AiCommandType, AiError};
use crate::common::{
    CommandSourceType, Coord2D, Coord3D, KindOf, LocomotorSetType, ModelConditionFlags, ObjectID,
    ObjectStatusMaskType, ObjectStatusTypes, Real, Relationship, TurretType, INVALID_ID,
    LOGICFRAMES_PER_SECOND,
};
use crate::damage::DamageInfo;
use crate::helpers::{
    game_logic_random_value, get_game_logic_random_value, TheGameLogic, TheTerrainLogic,
};
use crate::modules::{AIUpdateInterfaceExt, ContainWant};
use crate::object::registry::OBJECT_REGISTRY;
use crate::object::Object as GameObject;
use crate::path::{PATHFIND_CELL_SIZE_F, SURFACE_GROUND};
use crate::player::PlayerType;
use crate::state_machine::{StateExitType, StateReturnType};
use crate::terrain::get_terrain_logic;
use crate::weapon::WeaponChoiceCriteria;
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, RwLock};

const CRATE_PICKUP_RANGE_SQR: f32 = 100.0;

/// AI state context for carrying parameters between states
#[derive(Debug, Clone)]
pub struct AIStateContext {
    pub goal_object: Option<ObjectID>,
    pub goal_position: Option<Coord3D>,
    pub damage_info: DamageInfo,
    pub int_value: i32,
    pub real_value: Real,
    pub bool_value: bool,
}

impl Default for AIStateContext {
    fn default() -> Self {
        Self {
            goal_object: None,
            goal_position: None,
            damage_info: DamageInfo::new(),
            int_value: 0,
            real_value: 0.0,
            bool_value: false,
        }
    }
}

/// AI State IDs matching the C++ original exactly
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AIStateType {
    Idle,
    MoveTo,                               // Move to GoalObject or GoalPosition
    FollowWaypointPathAsTeam,             // Follow waypoint path as team
    FollowWaypointPathAsIndividuals,      // Follow waypoint path individually
    FollowWaypointPathAsTeamExact,        // Follow waypoint path as team (exact)
    FollowWaypointPathAsIndividualsExact, // Follow waypoint path individually (exact)
    FollowPath,                           // Follow simple list of points
    FollowExitProductionPath,             // Same as FollowPath but only when exiting production
    Wait,
    AttackPosition,        // Attack GoalPosition
    AttackObject,          // Attack GoalObject
    ForceAttackObject,     // Force attack GoalObject
    AttackAndFollowObject, // Attack GoalObject, follow if necessary
    Dead,
    Dock,                                  // Dock with GoalObject with DockUpdate
    Enter,                                 // Move to GoalObject and enter when close
    Guard,                                 // Guard current location
    Hunt,                                  // Seek and destroy behavior
    Wander,                                // Wander following waypoint path
    Panic,                                 // Run around screaming following waypoint
    AttackSquad,                           // Attack all objects in goalSquad
    GuardTunnelNetwork,                    // Guard from inside tunnel network
    GetRepaired,                           // Get repaired at repair depot
    MoveOutOfTheWay,                       // Move out of way of another unit
    MoveAndTighten,                        // Move to tighten up formation
    MoveAndEvacuate,                       // Move to then empty transport
    MoveAndEvacuateAndExit,                // Move to then empty transport and exit
    MoveAndDelete,                         // Move to then delete self
    AttackArea,                            // Attack units in an area
    HackInternet,                          // Hack internet for money
    AttackMoveTo,                          // Attack-move to location
    AttackFollowWaypointPathAsIndividuals, // Attack-follow waypoint path individually
    AttackFollowWaypointPathAsTeam,        // Attack-follow waypoint path as team
    FaceObject,                            // Face towards object
    FacePosition,                          // Face towards position
    RappelInto,                            // Rappel from current pos to target
    CombatDrop,                            // Send AI_RAPPEL_INTO to contents
    Exit,                                  // Exit the object, wait if necessary
    PickUpCrate,                           // Pick up crate created by kill
    MoveAwayFromRepulsors,                 // Civilians running from repulsors
    WanderInPlace,                         // Wander around a spot
    Busy,                                  // Busy doing stuff that doesn't require AI
    ExitInstantly,                         // Exit object without waiting
    GuardRetaliate,                        // Attack attacker with restrictions
}

// StateExitType is now imported from crate::state_machine

/// AI State machine context
#[derive(Debug, Clone)]
pub struct AIStateMachineContext {
    pub owner_id: ObjectID,
    pub goal_object: Option<ObjectID>,
    pub goal_position: Option<Coord3D>,
    pub goal_waypoint: Option<u32>, // Waypoint ID
    pub goal_squad: Option<u32>,    // Squad ID
    pub goal_path: Vec<Coord3D>,
    pub damage_info: DamageInfo,
    pub int_value: i32,
    pub command_button: Option<u32>, // Command button ID
    pub current_path: Option<u32>,   // Path ID
}

impl Default for AIStateMachineContext {
    fn default() -> Self {
        Self {
            owner_id: 0,
            goal_object: None,
            goal_position: None,
            goal_waypoint: None,
            goal_squad: None,
            goal_path: Vec::new(),
            damage_info: DamageInfo::default(),
            int_value: 0,
            command_button: None,
            current_path: None,
        }
    }
}

fn out_of_weapon_range_object(context: &AIStateMachineContext) -> bool {
    let Some(target_id) = context.goal_object else {
        return false;
    };
    OBJECT_REGISTRY
        .with_object(context.owner_id, |owner| {
            let Some((weapon, _slot)) = owner.get_current_weapon() else {
                return false;
            };
            if weapon.has_leech_range() {
                return false;
            }
            if OBJECT_REGISTRY.with_object(target_id, |_| ()).is_none() {
                return false;
            }
            !weapon.is_within_attack_range(owner.get_id(), Some(target_id), None)
        })
        .unwrap_or(false)
}

fn out_of_weapon_range_position(context: &AIStateMachineContext) -> bool {
    let Some(goal_position) = context.goal_position else {
        return false;
    };
    OBJECT_REGISTRY
        .with_object(context.owner_id, |owner| {
            let Some((weapon, _slot)) = owner.get_current_weapon() else {
                return false;
            };
            !weapon.is_within_attack_range(owner.get_id(), None, Some(&goal_position))
        })
        .unwrap_or(false)
}

fn want_to_squish_target(context: &AIStateMachineContext) -> bool {
    let Some(target_id) = context.goal_object else {
        return false;
    };
    let target_ok = OBJECT_REGISTRY
        .with_object(target_id, |target| {
            target.get_contained_by().is_none() && target.is_kind_of(KindOf::Infantry)
        })
        .unwrap_or(false);
    if !target_ok {
        return false;
    }

    OBJECT_REGISTRY
        .with_object(context.owner_id, |owner| {
            let turret = owner
                .get_ai_update_interface()
                .map(|ai| ai.get_which_turret_for_cur_weapon())
                .unwrap_or(TurretType::Invalid);
            if turret == TurretType::Invalid {
                return false;
            }

            let is_computer = owner
                .get_controlling_player()
                .and_then(|player| player.read().ok())
                .map(|player| player.get_player_type() == PlayerType::Computer)
                .unwrap_or(false);
            if !is_computer {
                return false;
            }

            owner.get_crusher_level() != 0
        })
        .unwrap_or(false)
}

fn goal_reached(context: &AIStateMachineContext) -> bool {
    let Some(goal_pos) = context.goal_position else {
        return false;
    };
    let Some(current_pos) =
        OBJECT_REGISTRY.with_object(context.owner_id, |owner| *owner.get_position())
    else {
        return false;
    };
    let delta = goal_pos - current_pos;
    let dist_sqr = delta.x * delta.x + delta.y * delta.y;
    let close_enough = PATHFIND_CLOSE_ENOUGH * PATHFIND_CLOSE_ENOUGH;
    dist_sqr <= close_enough
}

fn resolve_goal_position(context: &mut AIStateMachineContext) -> Option<Coord3D> {
    if let Some(target_id) = context.goal_object {
        if let Some(pos) = OBJECT_REGISTRY.with_object(target_id, |target| *target.get_position()) {
            context.goal_position = Some(pos);
            return Some(pos);
        }
    }

    context.goal_position
}

/// Base AI State trait
pub trait AIState: std::fmt::Debug + Send + Sync {
    /// Called when entering the state
    fn on_enter(&mut self, context: &mut AIStateMachineContext) -> StateReturnType;

    /// Called every frame while in the state
    fn update(&mut self, context: &mut AIStateMachineContext) -> StateReturnType;

    /// Called when exiting the state
    fn on_exit(&mut self, context: &mut AIStateMachineContext, exit_type: StateExitType);

    /// Get state type ID
    fn get_state_type(&self) -> AIStateType;

    /// Check if state is idle
    fn is_idle(&self) -> bool {
        false
    }

    /// Check if state is busy
    fn is_busy(&self) -> bool {
        false
    }

    /// Check if state is attack state
    fn is_attack(&self) -> bool {
        false
    }

    /// Check if state is guard idle
    fn is_guard_idle(&self) -> bool {
        false
    }
}

/// AI Idle State
#[derive(Debug)]
pub struct AIIdleState {
    initial_sleep_offset: u16,
    should_look_for_targets: bool,
    inited: bool,
}

impl AIIdleState {
    pub fn new(should_look_for_targets: bool) -> Self {
        Self {
            initial_sleep_offset: 0,
            should_look_for_targets,
            inited: false,
        }
    }

    fn do_init_idle_state(&mut self) {
        if !self.inited {
            self.initial_sleep_offset = game_logic_random_value(0, LOGICFRAMES_PER_SECOND * 2) as u16;
            self.inited = true;
        }
    }
}

impl AIState for AIIdleState {
    fn on_enter(&mut self, _context: &mut AIStateMachineContext) -> StateReturnType {
        self.do_init_idle_state();
        StateReturnType::Continue
    }

    fn update(&mut self, context: &mut AIStateMachineContext) -> StateReturnType {
        if self.should_look_for_targets {
            // Look for enemies to attack
            // This would interface with targeting system
        }
        StateReturnType::Continue
    }

    fn on_exit(&mut self, _context: &mut AIStateMachineContext, _exit_type: StateExitType) {
        // Cleanup when leaving idle state
    }

    fn get_state_type(&self) -> AIStateType {
        AIStateType::Idle
    }

    fn is_idle(&self) -> bool {
        true
    }
}

/// AI Move To State
#[derive(Debug)]
pub struct AIMoveToState {
    goal_position: Coord3D,
    path_goal_position: Coord3D,
    path_timestamp: u32,
    blocked_repath_timestamp: u32,
    adjust_destinations: bool,
    waiting_for_path: bool,
    try_one_more_repath: bool,
}

impl AIMoveToState {
    pub fn new() -> Self {
        Self {
            goal_position: Coord3D::new(0.0, 0.0, 0.0),
            path_goal_position: Coord3D::new(0.0, 0.0, 0.0),
            path_timestamp: 0,
            blocked_repath_timestamp: 0,
            adjust_destinations: true,
            waiting_for_path: false,
            try_one_more_repath: false,
        }
    }

    fn compute_path(&mut self, context: &mut AIStateMachineContext) -> bool {
        let Some(goal_pos) = context.goal_position else {
            return false;
        };
        let Some(start_pos) =
            OBJECT_REGISTRY.with_object(context.owner_id, |owner| *owner.get_position())
        else {
            return false;
        };

        let path_result = with_ai_integration(|manager| {
            manager.request_pathfinding(&start_pos, &goal_pos, SURFACE_GROUND, false)
        });

        if let Some(Ok(Some(path))) = path_result {
            context.goal_path = path;
            self.waiting_for_path = false;
            return true;
        }

        context.goal_path = vec![goal_pos];
        self.waiting_for_path = false;
        true
    }

    fn force_repath(&mut self) {
        self.path_goal_position = Coord3D::new(-100.0, -100.0, -100.0);
        self.path_timestamp = 0;
    }
}

impl AIState for AIMoveToState {
    fn on_enter(&mut self, context: &mut AIStateMachineContext) -> StateReturnType {
        if let Some(goal_pos) = context.goal_position {
            self.goal_position = goal_pos;
            self.compute_path(context);
        }
        StateReturnType::Continue
    }

    fn update(&mut self, context: &mut AIStateMachineContext) -> StateReturnType {
        // Check if we've reached the destination
        if context.goal_position.is_some() {
            // Real completion check depends on locomotor/path integration.
            StateReturnType::Continue
        } else {
            StateReturnType::Failed
        }
    }

    fn on_exit(&mut self, context: &mut AIStateMachineContext, _exit_type: StateExitType) {
        if let Some(ai) = OBJECT_REGISTRY
            .with_object(context.owner_id, |owner| owner.get_ai_update_interface())
            .flatten()
        {
            if let Ok(mut ai_guard) = ai.lock() {
                ai_guard.destroy_path();
            }
        }
    }

    fn get_state_type(&self) -> AIStateType {
        AIStateType::MoveTo
    }
}

/// AI Move Out Of The Way State
/// Matches C++ AIMoveOutOfTheWayState from AIStates.cpp lines 2125-2168.
#[derive(Debug)]
pub struct AIMoveOutOfTheWayState {
    goal_position: Coord3D,
}

impl AIMoveOutOfTheWayState {
    pub fn new() -> Self {
        Self {
            goal_position: Coord3D::new(0.0, 0.0, 0.0),
        }
    }
}

impl AIState for AIMoveOutOfTheWayState {
    fn on_enter(&mut self, context: &mut AIStateMachineContext) -> StateReturnType {
        let Some(ai) = OBJECT_REGISTRY
            .with_object(context.owner_id, |owner| owner.get_ai_update_interface())
            .flatten()
        else {
            return StateReturnType::Failed;
        };
        let Ok(ai_guard) = ai.lock() else {
            return StateReturnType::Failed;
        };
        let Some(goal_pos) = ai_guard.get_path_destination() else {
            return StateReturnType::Failed;
        };

        self.goal_position = goal_pos;
        context.goal_position = Some(goal_pos);

        StateReturnType::Continue
    }

    fn update(&mut self, context: &mut AIStateMachineContext) -> StateReturnType {
        let Some(status) = OBJECT_REGISTRY.with_object(context.owner_id, |owner| {
            if owner.is_effectively_dead() {
                return Some(None);
            }
            Some(owner.get_ai_update_interface())
        }) else {
            return StateReturnType::Failed;
        };
        let Some(ai) = status else {
            return StateReturnType::Success;
        };
        if let Ok(mut ai_guard) = ai.lock() {
            if ai_guard.is_blocked_and_stuck() {
                let _ = ai_guard.set_can_path_through_units(true);
            }
        }

        if context.goal_position.is_some() {
            StateReturnType::Continue
        } else {
            StateReturnType::Failed
        }
    }

    fn on_exit(&mut self, context: &mut AIStateMachineContext, _exit_type: StateExitType) {
        if let Some(ai) = OBJECT_REGISTRY
            .with_object(context.owner_id, |owner| owner.get_ai_update_interface())
            .flatten()
        {
            if let Ok(mut ai_guard) = ai.lock() {
                ai_guard.destroy_path();
                let _ = ai_guard.set_can_path_through_units(false);
                ai_guard.clear_move_out_of_way();
            }
        }
    }

    fn get_state_type(&self) -> AIStateType {
        AIStateType::MoveOutOfTheWay
    }
}

/// AI Move And Evacuate State
#[derive(Debug)]
pub struct AIMoveAndEvacuateState {
    origin: Coord3D,
    goal_position: Coord3D,
    evacuate_and_exit: bool,
}

impl AIMoveAndEvacuateState {
    pub fn new(evacuate_and_exit: bool) -> Self {
        Self {
            origin: Coord3D::new(0.0, 0.0, 0.0),
            goal_position: Coord3D::new(0.0, 0.0, 0.0),
            evacuate_and_exit,
        }
    }

    fn evacuate_contents(owner: &mut GameObject) {
        if let Some(contain) = owner.get_contain() {
            if let Ok(mut contain_guard) = contain.lock() {
                let ids: Vec<ObjectID> = contain_guard.get_contained_objects().to_vec();
                for id in ids {
                    let _ = contain_guard.release_object(id);
                }
            }
        }

        if let Some(team) = owner.get_team() {
            if let Ok(mut team_guard) = team.write() {
                team_guard.set_active();
            }
        }
    }
}

impl AIState for AIMoveAndEvacuateState {
    fn on_enter(&mut self, context: &mut AIStateMachineContext) -> StateReturnType {
        let Some(origin) =
            OBJECT_REGISTRY.with_object(context.owner_id, |owner| *owner.get_position())
        else {
            return StateReturnType::Failed;
        };
        self.origin = origin;

        let Some(goal_pos) = resolve_goal_position(context) else {
            return StateReturnType::Failed;
        };
        self.goal_position = goal_pos;
        StateReturnType::Continue
    }

    fn update(&mut self, context: &mut AIStateMachineContext) -> StateReturnType {
        let Some((ret, destroy)) = OBJECT_REGISTRY.with_object_mut(context.owner_id, |owner| {
            if owner.is_effectively_dead() {
                return (StateReturnType::Failed, false);
            }
            if goal_reached(context) {
                Self::evacuate_contents(owner);
                if self.evacuate_and_exit {
                    return (StateReturnType::Success, true);
                }
                return (StateReturnType::Success, false);
            }
            (StateReturnType::Continue, false)
        }) else {
            return StateReturnType::Failed;
        };
        if destroy {
            let _ = TheGameLogic::destroy_object_by_id(context.owner_id);
        }
        ret
    }

    fn on_exit(&mut self, context: &mut AIStateMachineContext, _exit_type: StateExitType) {
        context.goal_position = Some(self.origin);
    }

    fn get_state_type(&self) -> AIStateType {
        if self.evacuate_and_exit {
            AIStateType::MoveAndEvacuateAndExit
        } else {
            AIStateType::MoveAndEvacuate
        }
    }
}

/// AI Move And Delete State
#[derive(Debug)]
pub struct AIMoveAndDeleteState {
    goal_position: Coord3D,
}

impl AIMoveAndDeleteState {
    pub fn new() -> Self {
        Self {
            goal_position: Coord3D::new(0.0, 0.0, 0.0),
        }
    }
}

impl AIState for AIMoveAndDeleteState {
    fn on_enter(&mut self, context: &mut AIStateMachineContext) -> StateReturnType {
        let Some(goal_pos) = resolve_goal_position(context) else {
            return StateReturnType::Failed;
        };
        self.goal_position = goal_pos;
        StateReturnType::Continue
    }

    fn update(&mut self, context: &mut AIStateMachineContext) -> StateReturnType {
        let Some(should_destroy) = OBJECT_REGISTRY.with_object(context.owner_id, |owner| {
            if owner.is_effectively_dead() {
                return None;
            }
            Some(goal_reached(context))
        }) else {
            return StateReturnType::Failed;
        };
        if should_destroy {
            let _ = TheGameLogic::destroy_object_by_id(context.owner_id);
            return StateReturnType::Success;
        }

        StateReturnType::Continue
    }

    fn on_exit(&mut self, context: &mut AIStateMachineContext, _exit_type: StateExitType) {
        if let Some(ai) = OBJECT_REGISTRY
            .with_object(context.owner_id, |owner| owner.get_ai_update_interface())
            .flatten()
        {
            if let Ok(mut ai_guard) = ai.lock() {
                ai_guard.destroy_path();
            }
        }
    }

    fn get_state_type(&self) -> AIStateType {
        AIStateType::MoveAndDelete
    }
}

/// AI Follow Path State
#[derive(Debug)]
pub struct AIFollowPathState {
    path_index: usize,
}

impl AIFollowPathState {
    pub fn new() -> Self {
        Self { path_index: 0 }
    }

    fn set_next_goal(&mut self, context: &mut AIStateMachineContext) -> bool {
        if self.path_index >= context.goal_path.len() {
            return false;
        }
        let next = context.goal_path[self.path_index];
        context.goal_position = Some(next);
        if let Some(ai) = OBJECT_REGISTRY
            .with_object(context.owner_id, |owner| owner.get_ai_update_interface())
            .flatten()
        {
            if let Ok(mut ai_guard) = ai.lock() {
                let _ = ai_guard.set_movement_target(&next);
            }
        }
        true
    }
}

impl AIState for AIFollowPathState {
    fn on_enter(&mut self, context: &mut AIStateMachineContext) -> StateReturnType {
        if context.goal_path.is_empty() {
            return StateReturnType::Failed;
        }
        self.path_index = 0;
        if self.set_next_goal(context) {
            StateReturnType::Continue
        } else {
            StateReturnType::Failed
        }
    }

    fn update(&mut self, context: &mut AIStateMachineContext) -> StateReturnType {
        if context.goal_path.is_empty() {
            return StateReturnType::Success;
        }
        if goal_reached(context) {
            self.path_index = self.path_index.saturating_add(1);
            if !self.set_next_goal(context) {
                return StateReturnType::Success;
            }
        }
        StateReturnType::Continue
    }

    fn on_exit(&mut self, context: &mut AIStateMachineContext, _exit_type: StateExitType) {
        if let Some(ai) = OBJECT_REGISTRY
            .with_object(context.owner_id, |owner| owner.get_ai_update_interface())
            .flatten()
        {
            if let Ok(mut ai_guard) = ai.lock() {
                ai_guard.set_can_path_through_units(false);
                ai_guard.destroy_path();
            }
        }
    }

    fn get_state_type(&self) -> AIStateType {
        AIStateType::FollowPath
    }
}

/// AI Follow Exit Production Path State
#[derive(Debug)]
pub struct AIFollowExitProductionPathState {
    base: AIFollowPathState,
}

impl AIFollowExitProductionPathState {
    pub fn new() -> Self {
        Self {
            base: AIFollowPathState::new(),
        }
    }
}

impl AIState for AIFollowExitProductionPathState {
    fn on_enter(&mut self, context: &mut AIStateMachineContext) -> StateReturnType {
        self.base.on_enter(context)
    }

    fn update(&mut self, context: &mut AIStateMachineContext) -> StateReturnType {
        self.base.update(context)
    }

    fn on_exit(&mut self, context: &mut AIStateMachineContext, exit_type: StateExitType) {
        self.base.on_exit(context, exit_type);
    }

    fn get_state_type(&self) -> AIStateType {
        AIStateType::FollowExitProductionPath
    }
}

/// AI Wait State
#[derive(Debug)]
pub struct AIWaitState {
    wake_frame: u32,
}

impl AIWaitState {
    pub fn new() -> Self {
        Self { wake_frame: 0 }
    }
}

impl AIState for AIWaitState {
    fn on_enter(&mut self, context: &mut AIStateMachineContext) -> StateReturnType {
        let delay = if context.int_value > 0 {
            context.int_value as u32
        } else {
            LOGICFRAMES_PER_SECOND
        };
        self.wake_frame = TheGameLogic::get_frame().wrapping_add(delay);
        StateReturnType::Continue
    }

    fn update(&mut self, _context: &mut AIStateMachineContext) -> StateReturnType {
        if TheGameLogic::get_frame() >= self.wake_frame {
            StateReturnType::Success
        } else {
            StateReturnType::Continue
        }
    }

    fn on_exit(&mut self, _context: &mut AIStateMachineContext, _exit_type: StateExitType) {
        // C++ AIWaitState has no onExit() override -- inherits empty base class default.
        // No temporary state to clean up; wake_frame is recalculated on next on_enter().
    }

    fn get_state_type(&self) -> AIStateType {
        AIStateType::Wait
    }
}

/// AI Dead State
#[derive(Debug)]
pub struct AIDeadState;

impl AIDeadState {
    pub fn new() -> Self {
        Self
    }
}

impl AIState for AIDeadState {
    fn on_enter(&mut self, context: &mut AIStateMachineContext) -> StateReturnType {
        if let Some(ai) = OBJECT_REGISTRY
            .with_object(context.owner_id, |owner| owner.get_ai_update_interface())
            .flatten()
        {
            ai.mark_as_dead();
        }
        StateReturnType::Continue
    }

    fn update(&mut self, context: &mut AIStateMachineContext) -> StateReturnType {
        match OBJECT_REGISTRY.with_object(context.owner_id, |owner| owner.is_effectively_dead()) {
            None | Some(true) => StateReturnType::Success,
            Some(false) => StateReturnType::Continue,
        }
    }

    fn on_exit(&mut self, context: &mut AIStateMachineContext, _exit_type: StateExitType) {
        let _ = OBJECT_REGISTRY.with_object_mut(context.owner_id, |owner| {
            owner.clear_model_condition_state(ModelConditionFlags::DYING);
        });
    }

    fn get_state_type(&self) -> AIStateType {
        AIStateType::Dead
    }
}

/// AI Dock State
#[derive(Debug)]
pub struct AIDockState {
    dock_machine: Option<AIDockMachine>,
}

impl AIDockState {
    pub fn new() -> Self {
        Self { dock_machine: None }
    }
}

impl AIState for AIDockState {
    fn on_enter(&mut self, context: &mut AIStateMachineContext) -> StateReturnType {
        let Some(goal_id) = context.goal_object else {
            return StateReturnType::Failed;
        };
        let has_dock = OBJECT_REGISTRY
            .with_object(goal_id, |goal_guard| {
                goal_guard.with_dock_update_interface(|_| true).unwrap_or(false)
            })
            .unwrap_or(false);
        if !has_dock {
            return StateReturnType::Failed;
        }

        if let Some(ai) = OBJECT_REGISTRY
            .with_object(context.owner_id, |owner_guard| owner_guard.get_ai_update_interface())
            .flatten()
        {
            if let Ok(mut ai_guard) = ai.lock() {
                let legacy_goal = get_legacy_object(goal_id);
                let _ = ai_guard.ignore_obstacle(legacy_goal.as_ref().and_then(|a| a.read().ok().map(|g| g.get_id())));
                let _ = ai_guard.set_can_path_through_units(true);
            }
        }

        let mut dock_machine = match AIDockMachine::new(owner_arc.clone()) {
            Ok(machine) => machine,
            Err(_) => return StateReturnType::Failed,
        };
        if let Ok(mut machine) = dock_machine.state_machine.lock() {
            machine.set_goal_object(Some(Arc::downgrade(&goal_arc)));
            let _ = machine.init_default_state();
        }
        self.dock_machine = Some(dock_machine);
        StateReturnType::Continue
    }

    fn update(&mut self, _context: &mut AIStateMachineContext) -> StateReturnType {
        let Some(machine) = self.dock_machine.as_mut() else {
            return StateReturnType::Failed;
        };
        let Ok(mut state_machine) = machine.state_machine.lock() else {
            return StateReturnType::Failed;
        };
        match state_machine.update() {
            StateReturnType::Sleep(_) => StateReturnType::Continue,
            result => result,
        }
    }

    fn on_exit(&mut self, context: &mut AIStateMachineContext, _exit_type: StateExitType) {
        if let Some(mut machine) = self.dock_machine.take() {
            let _ = machine.halt();
        }
        if let Some(ai) = OBJECT_REGISTRY
            .with_object(context.owner_id, |owner| owner.get_ai_update_interface())
            .flatten()
        {
            if let Ok(mut ai_guard) = ai.lock() {
                let _ = ai_guard.set_can_path_through_units(false);
                let _ = ai_guard.ignore_obstacle(None);
            }
        }
    }

    fn get_state_type(&self) -> AIStateType {
        AIStateType::Dock
    }
}

/// AI Enter State
#[derive(Debug)]
pub struct AIEnterState {
    entry_to_clear: ObjectID,
}

impl AIEnterState {
    pub fn new() -> Self {
        Self {
            entry_to_clear: INVALID_ID,
        }
    }
}

impl AIState for AIEnterState {
    fn on_enter(&mut self, context: &mut AIStateMachineContext) -> StateReturnType {
        self.entry_to_clear = INVALID_ID;
        let Some(goal_id) = context.goal_object else {
            return StateReturnType::Failed;
        };
        let Some(owner_arc) = OBJECT_REGISTRY.get_object(context.owner_id) else {
            return StateReturnType::Failed;
        };
        let Some(goal_arc) = OBJECT_REGISTRY.get_object(goal_id) else {
            return StateReturnType::Failed;
        };

        let Ok(owner_guard) = owner_arc.read() else {
            return StateReturnType::Failed;
        };
        let Ok(goal_guard) = goal_arc.read() else {
            return StateReturnType::Failed;
        };
        let Some(contain) = goal_guard.get_contain() else {
            return StateReturnType::Failed;
        };
        let Ok(mut contain_guard) = contain.lock() else {
            return StateReturnType::Failed;
        };
        if !contain_guard.is_valid_container_for(&*owner_guard, true) {
            return StateReturnType::Failed;
        }
        let _ = contain_guard
            .on_object_wants_to_enter_or_exit(&*owner_guard, ContainWant::WantsToEnter);
        drop(contain_guard);

        let goal_pos = *goal_guard.get_position();
        context.goal_position = Some(goal_pos);

        if let Some(ai) = owner_guard.get_ai_update_interface() {
            if let Ok(mut ai_guard) = ai.lock() {
                let _ = ai_guard.set_allow_invalid_position(true);
                let legacy_goal = get_legacy_object(goal_id);
                let _ = ai_guard.ignore_obstacle(legacy_goal.as_ref().and_then(|a| a.read().ok().map(|g| g.get_id())));
                let _ = ai_guard.set_movement_target(&goal_pos);
            }
        }

        self.entry_to_clear = goal_id;
        StateReturnType::Continue
    }

    fn update(&mut self, context: &mut AIStateMachineContext) -> StateReturnType {
        let Some(goal_id) = context.goal_object else {
            return StateReturnType::Failed;
        };
        let Some(owner_arc) = OBJECT_REGISTRY.get_object(context.owner_id) else {
            return StateReturnType::Failed;
        };
        let Some(goal_arc) = OBJECT_REGISTRY.get_object(goal_id) else {
            return StateReturnType::Failed;
        };

        let Ok(owner_guard) = owner_arc.read() else {
            return StateReturnType::Failed;
        };
        let Ok(goal_guard) = goal_arc.read() else {
            return StateReturnType::Failed;
        };

        if let Some(ai) = owner_guard.get_ai_update_interface() {
            if let Ok(mut ai_guard) = ai.lock() {
                let current_goal = context
                    .goal_position
                    .unwrap_or_else(|| *goal_guard.get_position());
                if current_goal != *goal_guard.get_position() {
                    let new_goal = *goal_guard.get_position();
                    context.goal_position = Some(new_goal);
                    let _ = ai_guard.set_movement_target(&new_goal);
                }
            }
        }

        let Some(contain) = goal_guard.get_contain() else {
            return StateReturnType::Failed;
        };
        let Ok(mut contain_guard) = contain.lock() else {
            return StateReturnType::Failed;
        };
        if !contain_guard.is_valid_container_for(&*owner_guard, true) {
            return StateReturnType::Failed;
        }

        let owner_pos = owner_guard.get_position();
        let goal_pos = goal_guard.get_position();
        let dx = owner_pos.x - goal_pos.x;
        let dy = owner_pos.y - goal_pos.y;
        let radius = goal_guard.get_geometry_info().get_major_radius();
        if dx * dx + dy * dy <= radius * radius {
            let _ = contain_guard.add_to_contain(&*owner_guard);
            return StateReturnType::Success;
        }

        StateReturnType::Continue
    }

    fn on_exit(&mut self, context: &mut AIStateMachineContext, _exit_type: StateExitType) {
        if let Some(ai) = OBJECT_REGISTRY
            .with_object(context.owner_id, |owner| owner.get_ai_update_interface())
            .flatten()
        {
            if let Ok(mut ai_guard) = ai.lock() {
                let _ = ai_guard.set_allow_invalid_position(false);
                let _ = ai_guard.ignore_obstacle(None);
            }
        }
        if self.entry_to_clear != INVALID_ID {
            if let Some(goal_arc) = OBJECT_REGISTRY.get_object(self.entry_to_clear) {
                if let Ok(goal_guard) = goal_arc.read() {
                    if let Some(contain) = goal_guard.get_contain() {
                        if let Ok(mut contain_guard) = contain.lock() {
                            if let Some(owner_arc) = OBJECT_REGISTRY.get_object(context.owner_id) {
                                if let Ok(owner_guard) = owner_arc.read() {
                                    let _ = contain_guard.on_object_wants_to_enter_or_exit(
                                        &*owner_guard,
                                        ContainWant::WantsNeither,
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
        self.entry_to_clear = INVALID_ID;
    }

    fn get_state_type(&self) -> AIStateType {
        AIStateType::Enter
    }
}

/// AI Exit State
#[derive(Debug)]
pub struct AIExitState;

impl AIExitState {
    pub fn new() -> Self {
        Self
    }
}

impl AIState for AIExitState {
    fn on_enter(&mut self, context: &mut AIStateMachineContext) -> StateReturnType {
        let Some(owner_arc) = OBJECT_REGISTRY.get_object(context.owner_id) else {
            return StateReturnType::Failed;
        };
        let Ok(owner_guard) = owner_arc.read() else {
            return StateReturnType::Failed;
        };
        let Some(container_id) = owner_guard.get_contained_by() else {
            return StateReturnType::Success;
        };
        let Some(container_arc) = OBJECT_REGISTRY.get_object(container_id) else {
            return StateReturnType::Failed;
        };
        let Ok(container_guard) = container_arc.read() else {
            return StateReturnType::Failed;
        };
        let Some(contain) = container_guard.get_contain() else {
            return StateReturnType::Failed;
        };
        let Ok(mut contain_guard) = contain.lock() else {
            return StateReturnType::Failed;
        };
        let _ =
            contain_guard.on_object_wants_to_enter_or_exit(&*owner_guard, ContainWant::WantsToExit);
        StateReturnType::Continue
    }

    fn update(&mut self, context: &mut AIStateMachineContext) -> StateReturnType {
        match OBJECT_REGISTRY
            .with_object(context.owner_id, |owner| owner.get_contained_by().is_none())
        {
            None | Some(true) => StateReturnType::Success,
            Some(false) => StateReturnType::Continue,
        }
    }

    fn on_exit(&mut self, context: &mut AIStateMachineContext, _exit_type: StateExitType) {
        let _ = OBJECT_REGISTRY.with_object(context.owner_id, |owner_guard| {
            let Some(container_id) = owner_guard.get_contained_by() else {
                return;
            };
            let Some(contain) = OBJECT_REGISTRY
                .with_object(container_id, |container_guard| container_guard.get_contain())
                .flatten()
            else {
                return;
            };
            if let Ok(mut contain_guard) = contain.lock() {
                let _ = contain_guard.on_object_wants_to_enter_or_exit(
                    owner_guard,
                    ContainWant::WantsNeither,
                );
            }
        });
    }

    fn get_state_type(&self) -> AIStateType {
        AIStateType::Exit
    }
}

/// AI Pick Up Crate State
#[derive(Debug)]
pub struct AIPickUpCrateState;

impl AIPickUpCrateState {
    pub fn new() -> Self {
        Self
    }
}

impl AIState for AIPickUpCrateState {
    fn on_enter(&mut self, context: &mut AIStateMachineContext) -> StateReturnType {
        let Some(goal_id) = context.goal_object else {
            return StateReturnType::Failed;
        };
        let Some(owner_arc) = get_legacy_object(context.owner_id) else {
            return StateReturnType::Failed;
        };
        let Some(goal_arc) = get_legacy_object(goal_id) else {
            return StateReturnType::Failed;
        };

        if let Ok(owner_guard) = owner_arc.read() {
            if let Ok(goal_guard) = goal_arc.read() {
                if let Some(ai) = owner_guard.get_ai_update_interface() {
                    if let Ok(mut ai_guard) = ai.lock() {
                        let _ = ai_guard.set_movement_target(goal_guard.get_position());
                    }
                }
            }
        }

        StateReturnType::Continue
    }

    fn update(&mut self, context: &mut AIStateMachineContext) -> StateReturnType {
        let Some(goal_id) = context.goal_object else {
            return StateReturnType::Success;
        };
        let Some(owner_arc) = get_legacy_object(context.owner_id) else {
            return StateReturnType::Failed;
        };
        let Some(goal_arc) = get_legacy_object(goal_id) else {
            return StateReturnType::Success;
        };

        let Ok(owner_guard) = owner_arc.read() else {
            return StateReturnType::Continue;
        };
        let Ok(goal_guard) = goal_arc.read() else {
            return StateReturnType::Success;
        };

        let owner_pos = owner_guard.get_position();
        let goal_pos = goal_guard.get_position();
        let dx = owner_pos.x - goal_pos.x;
        let dy = owner_pos.y - goal_pos.y;
        let dist_sqr = dx * dx + dy * dy;
        if dist_sqr <= CRATE_PICKUP_RANGE_SQR {
            StateReturnType::Success
        } else {
            StateReturnType::Continue
        }
    }

    fn on_exit(&mut self, context: &mut AIStateMachineContext, _exit_type: StateExitType) {
        if let Some(ai) = OBJECT_REGISTRY
            .with_object(context.owner_id, |owner| owner.get_ai_update_interface())
            .flatten()
        {
            if let Ok(mut ai_guard) = ai.lock() {
                ai_guard.destroy_path();
            }
        }
    }

    fn get_state_type(&self) -> AIStateType {
        AIStateType::PickUpCrate
    }
}

/// AI Attack Squad State
#[derive(Debug)]
pub struct AIAttackSquadState {
    attack_machine: Option<AIAttackThenIdleStateMachine>,
}

impl AIAttackSquadState {
    pub fn new() -> Self {
        Self {
            attack_machine: None,
        }
    }
}

impl AIState for AIAttackSquadState {
    fn on_enter(&mut self, context: &mut AIStateMachineContext) -> StateReturnType {
        let Some(owner_arc) = get_legacy_object(context.owner_id) else {
            return StateReturnType::Failed;
        };
        let mut attack_machine = AIAttackThenIdleStateMachine::new(
            Arc::downgrade(&owner_arc),
            "AIAttackSquadStateMachine",
        );
        let result = attack_machine.init_default_state();
        self.attack_machine = Some(attack_machine);
        result
    }

    fn update(&mut self, context: &mut AIStateMachineContext) -> StateReturnType {
        let Some(attack_machine) = self.attack_machine.as_mut() else {
            return StateReturnType::Failed;
        };
        // Fallback behavior: attack closest enemy when no squad context is available.
        if attack_machine.get_current_state_id() == Some(LegacyAIStateType::Idle as u32) {
            let attack_priority = resolve_attack_priority_info_for_object(context.owner_id);
            if let Ok(Some(victim)) = THE_AI.read().map(|ai| {
                ai.find_closest_enemy(
                    context.owner_id,
                    9999.9,
                    search_qualifiers::CAN_ATTACK,
                    attack_priority.as_ref(),
                    None,
                )
            }) {
                if let Some(victim_arc) = get_legacy_object(victim) {
                    attack_machine.set_goal_object(victim_arc.read().ok().map(|g| g.get_id()));
                    let _ = attack_machine.set_state(LegacyAIStateType::AttackObject);
                }
            }
        }
        attack_machine.update()
    }

    fn on_exit(&mut self, _context: &mut AIStateMachineContext, _exit_type: StateExitType) {
        if let Some(mut machine) = self.attack_machine.take() {
            let _ = machine.halt();
        }
    }

    fn get_state_type(&self) -> AIStateType {
        AIStateType::AttackSquad
    }
}

/// AI Hack Internet State
#[derive(Debug)]
pub struct AIHackInternetState;

impl AIHackInternetState {
    pub fn new() -> Self {
        Self
    }
}

impl AIState for AIHackInternetState {
    fn on_enter(&mut self, context: &mut AIStateMachineContext) -> StateReturnType {
        if let Some(ai) = OBJECT_REGISTRY
            .with_object(context.owner_id, |owner| owner.get_ai_update_interface())
            .flatten()
        {
            if let Ok(mut ai_guard) = ai.lock() {
                let mut params = AiCommandParams::new(
                    AiCommandType::HackInternet,
                    CommandSourceType::FromAi,
                );
                let _ = ai_guard.execute_command(&params);
            }
        }
        StateReturnType::Continue
    }

    fn update(&mut self, context: &mut AIStateMachineContext) -> StateReturnType {
        let Some(status) = OBJECT_REGISTRY.with_object(context.owner_id, |owner_guard| {
            let Some(ai) = owner_guard.get_ai_update_interface() else {
                return None; // Failed
            };
            let Ok(mut ai_guard) = ai.lock() else {
                return None; // Failed
            };
            let Some(hack) = ai_guard.get_hack_internet_ai_update_interface() else {
                return Some(false); // Success (not busy)
            };
            Some(hack.is_hacking_packing_or_unpacking())
        })
        .flatten() else {
            return StateReturnType::Failed;
        };
        if status {
            StateReturnType::Continue
        } else {
            StateReturnType::Success
        }
    }

    fn on_exit(&mut self, context: &mut AIStateMachineContext, _exit_type: StateExitType) {
        // C++ HackInternetState::onExit clears MODELCONDITION_FIRING_A on the owner.
        let _ = OBJECT_REGISTRY.with_object_mut(context.owner_id, |owner| {
            owner.clear_model_condition_state(ModelConditionFlags::FIRING_A);
        });
    }

    fn get_state_type(&self) -> AIStateType {
        AIStateType::HackInternet
    }
}

/// AI Face Object State
#[derive(Debug)]
pub struct AIFaceObjectState;

impl AIFaceObjectState {
    pub fn new() -> Self {
        Self
    }
}

impl AIState for AIFaceObjectState {
    fn on_enter(&mut self, context: &mut AIStateMachineContext) -> StateReturnType {
        let Some(goal_id) = context.goal_object else {
            return StateReturnType::Failed;
        };
        let Some(goal_pos) =
            OBJECT_REGISTRY.with_object(goal_id, |goal_guard| *goal_guard.get_position())
        else {
            return StateReturnType::Failed;
        };
        let oriented = OBJECT_REGISTRY
            .with_object_mut(context.owner_id, |owner_guard| {
                let owner_pos = owner_guard.get_position();
                let dx = goal_pos.x - owner_pos.x;
                let dy = goal_pos.y - owner_pos.y;
                let angle = dy.atan2(dx);
                let _ = owner_guard.set_orientation(angle);
            })
            .is_some();
        if !oriented {
            return StateReturnType::Failed;
        }
        StateReturnType::Success
    }

    fn update(&mut self, _context: &mut AIStateMachineContext) -> StateReturnType {
        StateReturnType::Success
    }

    fn on_exit(&mut self, _context: &mut AIStateMachineContext, _exit_type: StateExitType) {
        // C++ AIFaceState::onExit() is genuinely empty -- orientation is set in onEnter/update only.
    }

    fn get_state_type(&self) -> AIStateType {
        AIStateType::FaceObject
    }
}

/// AI Face Position State
#[derive(Debug)]
pub struct AIFacePositionState;

impl AIFacePositionState {
    pub fn new() -> Self {
        Self
    }
}

impl AIState for AIFacePositionState {
    fn on_enter(&mut self, context: &mut AIStateMachineContext) -> StateReturnType {
        let Some(goal_pos) = context.goal_position else {
            return StateReturnType::Failed;
        };
        let oriented = OBJECT_REGISTRY
            .with_object_mut(context.owner_id, |owner_guard| {
                let owner_pos = owner_guard.get_position();
                let dx = goal_pos.x - owner_pos.x;
                let dy = goal_pos.y - owner_pos.y;
                let angle = dy.atan2(dx);
                let _ = owner_guard.set_orientation(angle);
            })
            .is_some();
        if !oriented {
            return StateReturnType::Failed;
        }
        StateReturnType::Success
    }

    fn update(&mut self, _context: &mut AIStateMachineContext) -> StateReturnType {
        StateReturnType::Success
    }

    fn on_exit(&mut self, _context: &mut AIStateMachineContext, _exit_type: StateExitType) {
        // C++ AIFaceState::onExit() is genuinely empty -- same class as FaceObject, no cleanup needed.
    }

    fn get_state_type(&self) -> AIStateType {
        AIStateType::FacePosition
    }
}

/// AI Rappel Into State
#[derive(Debug)]
pub struct AIRappelIntoState;

impl AIRappelIntoState {
    pub fn new() -> Self {
        Self
    }
}

impl AIState for AIRappelIntoState {
    fn on_enter(&mut self, context: &mut AIStateMachineContext) -> StateReturnType {
        if let Some(ai) = OBJECT_REGISTRY
            .with_object(context.owner_id, |owner| owner.get_ai_update_interface())
            .flatten()
        {
            if let Ok(mut ai_guard) = ai.lock() {
                let mut params = AiCommandParams::new(
                    AiCommandType::RappelInto,
                    CommandSourceType::FromAi,
                );
                params.obj = context.goal_object;
                if let Some(goal_pos) = context.goal_position {
                    params.pos = goal_pos;
                }
                let _ = ai_guard.execute_command(&params);
            }
        }
        StateReturnType::Continue
    }

    fn update(&mut self, _context: &mut AIStateMachineContext) -> StateReturnType {
        StateReturnType::Success
    }

    fn on_exit(&mut self, context: &mut AIStateMachineContext, _exit_type: StateExitType) {
        let _ = OBJECT_REGISTRY.with_object_mut(context.owner_id, |owner| {
            owner.clear_model_condition_state(ModelConditionFlags::RAPPELLING);
        });
        if let Some(ai) = OBJECT_REGISTRY
            .with_object(context.owner_id, |owner| owner.get_ai_update_interface())
            .flatten()
        {
            if let Ok(mut ai_guard) = ai.lock() {
                ai_guard.set_desired_speed(f32::MAX);
            }
        }
    }

    fn get_state_type(&self) -> AIStateType {
        AIStateType::RappelInto
    }
}

/// AI Combat Drop State
#[derive(Debug)]
pub struct AICombatDropState;

impl AICombatDropState {
    pub fn new() -> Self {
        Self
    }
}

impl AIState for AICombatDropState {
    fn on_enter(&mut self, context: &mut AIStateMachineContext) -> StateReturnType {
        if let Some(ai) = OBJECT_REGISTRY
            .with_object(context.owner_id, |owner| owner.get_ai_update_interface())
            .flatten()
        {
            if let Ok(mut ai_guard) = ai.lock() {
                let mut params = AiCommandParams::new(
                    AiCommandType::CombatDrop,
                    CommandSourceType::FromAi,
                );
                params.obj = context.goal_object;
                if let Some(goal_pos) = context.goal_position {
                    params.pos = goal_pos;
                }
                let _ = ai_guard.execute_command(&params);
            }
        }
        StateReturnType::Continue
    }

    fn update(&mut self, _context: &mut AIStateMachineContext) -> StateReturnType {
        StateReturnType::Success
    }

    fn on_exit(&mut self, context: &mut AIStateMachineContext, _exit_type: StateExitType) {
        // C++ ChinookCombatDropState::onExit clears DISABLED_HELD, sets flight status to FLYING,
        // idles any rappellers if the owner died, and expires rope drawables.
        let _ = OBJECT_REGISTRY.with_object_mut(context.owner_id, |owner| {
            owner.clear_disabled(crate::common::DisabledType::Held);
        });
    }

    fn get_state_type(&self) -> AIStateType {
        AIStateType::CombatDrop
    }
}

/// AI Busy State
#[derive(Debug)]
pub struct AIBusyState;

impl AIBusyState {
    pub fn new() -> Self {
        Self
    }
}

impl AIState for AIBusyState {
    fn on_enter(&mut self, context: &mut AIStateMachineContext) -> StateReturnType {
        if let Some(ai) = OBJECT_REGISTRY
            .with_object(context.owner_id, |owner| owner.get_ai_update_interface())
            .flatten()
        {
            if let Ok(mut ai_guard) = ai.lock() {
                let params =
                    AiCommandParams::new(AiCommandType::Busy, CommandSourceType::FromAi);
                let _ = ai_guard.execute_command(&params);
            }
        }
        StateReturnType::Continue
    }

    fn update(&mut self, context: &mut AIStateMachineContext) -> StateReturnType {
        let idle = OBJECT_REGISTRY
            .with_object(context.owner_id, |owner_guard| {
                owner_guard.get_ai_update_interface().and_then(|ai| {
                    ai.lock().ok().map(|ai_guard| ai_guard.is_idle())
                })
            })
            .flatten()
            .unwrap_or(true);
        if idle {
            StateReturnType::Success
        } else {
            StateReturnType::Continue
        }
    }

    fn on_exit(&mut self, _context: &mut AIStateMachineContext, _exit_type: StateExitType) {
        // C++ AIBusyState::onExit() is genuinely empty -- inline in AIStateMachine.h line 325.
    }

    fn get_state_type(&self) -> AIStateType {
        AIStateType::Busy
    }
}

/// AI Exit Instantly State
#[derive(Debug)]
pub struct AIExitInstantlyState;

impl AIExitInstantlyState {
    pub fn new() -> Self {
        Self
    }

    fn release_from_container(owner: &GameObject) {
        if let Some(container_id) = owner.get_container_id() {
            let _ = crate::object::registry::OBJECT_REGISTRY.with_object_mut(container_id, |container| {
                if let Some(contain) = container.get_contain() {
                    if let Ok(mut contain_guard) = contain.lock() {
                        let _ = contain_guard.release_object(owner.get_id());
                    }
                }
            });
        }
    }

    fn evacuate_contents(owner: &mut GameObject) {
        if let Some(contain) = owner.get_contain() {
            if let Ok(mut contain_guard) = contain.lock() {
                let ids: Vec<ObjectID> = contain_guard.get_contained_objects().to_vec();
                for id in ids {
                    let _ = contain_guard.release_object(id);
                }
            }
        }
    }
}

impl AIState for AIExitInstantlyState {
    fn on_enter(&mut self, context: &mut AIStateMachineContext) -> StateReturnType {
        let Some(ok) = OBJECT_REGISTRY.with_object_mut(context.owner_id, |owner| {
            if owner.is_effectively_dead() {
                return false;
            }
            Self::release_from_container(owner);
            Self::evacuate_contents(owner);
            true
        }) else {
            return StateReturnType::Failed;
        };
        if ok {
            StateReturnType::Success
        } else {
            StateReturnType::Failed
        }
    }

    fn update(&mut self, _context: &mut AIStateMachineContext) -> StateReturnType {
        StateReturnType::Success
    }

    fn on_exit(&mut self, context: &mut AIStateMachineContext, _exit_type: StateExitType) {
        let _ = OBJECT_REGISTRY.with_object(context.owner_id, |owner_guard| {
            let Some(container_id) = owner_guard.get_contained_by() else {
                return;
            };
            let Some(contain) = OBJECT_REGISTRY
                .with_object(container_id, |container_guard| container_guard.get_contain())
                .flatten()
            else {
                return;
            };
            if let Ok(mut contain_guard) = contain.lock() {
                let _ = contain_guard.on_object_wants_to_enter_or_exit(
                    owner_guard,
                    ContainWant::WantsNeither,
                );
            }
        });
    }

    fn get_state_type(&self) -> AIStateType {
        AIStateType::ExitInstantly
    }
}

/// AI Get Repaired State
#[derive(Debug)]
pub struct AIGetRepairedState {
    goal_position: Coord3D,
}

impl AIGetRepairedState {
    pub fn new() -> Self {
        Self {
            goal_position: Coord3D::new(0.0, 0.0, 0.0),
        }
    }
}

impl AIState for AIGetRepairedState {
    fn on_enter(&mut self, context: &mut AIStateMachineContext) -> StateReturnType {
        let Some(goal_pos) = resolve_goal_position(context) else {
            return StateReturnType::Failed;
        };
        self.goal_position = goal_pos;
        StateReturnType::Continue
    }

    fn update(&mut self, context: &mut AIStateMachineContext) -> StateReturnType {
        let Some(dead) = OBJECT_REGISTRY.with_object(context.owner_id, |owner| {
            owner.is_effectively_dead()
        }) else {
            return StateReturnType::Failed;
        };
        if dead {
            return StateReturnType::Failed;
        }
        if goal_reached(context) {
            return StateReturnType::Success;
        }
        StateReturnType::Continue
    }

    fn on_exit(&mut self, context: &mut AIStateMachineContext, _exit_type: StateExitType) {
        // C++ has no AIGetRepairedState class -- GetRepaired delegates to AIDockState/landing states.
        // Destroy any path that may have been computed for the repair depot approach.
        if let Some(ai) = OBJECT_REGISTRY
            .with_object(context.owner_id, |owner| owner.get_ai_update_interface())
            .flatten()
        {
            if let Ok(mut ai_guard) = ai.lock() {
                ai_guard.destroy_path();
            }
        }
    }

    fn get_state_type(&self) -> AIStateType {
        AIStateType::GetRepaired
    }
}

/// AI Attack State
#[derive(Debug)]
pub struct AIAttackState {
    follow: bool,
    attacking_object: bool,
    force_attacking: bool,
    attack_area: bool,
    original_victim_pos: Coord3D,
    victim_team: Option<u32>,
}

impl AIAttackState {
    pub fn new(
        follow: bool,
        attacking_object: bool,
        force_attacking: bool,
        attack_area: bool,
    ) -> Self {
        Self {
            follow,
            attacking_object,
            force_attacking,
            attack_area,
            original_victim_pos: Coord3D::new(0.0, 0.0, 0.0),
            victim_team: None,
        }
    }

    fn choose_weapon(&self, context: &AIStateMachineContext) -> bool {
        let Some(owner_arc) = OBJECT_REGISTRY.get_object(context.owner_id) else {
            return false;
        };

        let mut owner = match owner_arc.write() {
            Ok(guard) => guard,
            Err(_) => return false,
        };

        let cmd_source = owner
            .get_ai()
            .map(|ai| ai.get_last_command_source())
            .unwrap_or(CommandSourceType::FromAi);

        let found = if self.attacking_object {
            let Some(target_id) = context.goal_object else {
                return false;
            };
            let Some(target_arc) = OBJECT_REGISTRY.get_object(target_id) else {
                return false;
            };
            let Ok(target) = target_arc.read() else {
                return false;
            };
            owner.choose_best_weapon_for_target(
                &target,
                WeaponChoiceCriteria::PreferMostDamage,
                cmd_source,
            )
        } else {
            owner.choose_best_weapon_for_target_id(
                INVALID_ID,
                WeaponChoiceCriteria::PreferMostDamage,
                cmd_source,
            )
        };

        owner.adjust_model_condition_for_weapon_status();
        found
    }
}

impl AIState for AIAttackState {
    fn on_enter(&mut self, context: &mut AIStateMachineContext) -> StateReturnType {
        let Some(owner_arc) = OBJECT_REGISTRY.get_object(context.owner_id) else {
            return StateReturnType::Failed;
        };
        {
            let Ok(owner) = owner_arc.read() else {
                return StateReturnType::Failed;
            };
            if owner.test_status(ObjectStatusTypes::UnderConstruction) {
                return StateReturnType::Failed;
            }
            if owner.is_out_of_ammo() && !owner.is_kind_of(KindOf::Projectile) {
                return StateReturnType::Failed;
            }
        }

        if self.attacking_object {
            let Some(target_id) = context.goal_object else {
                return StateReturnType::Failed;
            };
            let Some(target_arc) = OBJECT_REGISTRY.get_object(target_id) else {
                return StateReturnType::Failed;
            };
            let Ok(target) = target_arc.read() else {
                return StateReturnType::Failed;
            };
            if target.is_effectively_dead() {
                return StateReturnType::Failed;
            }
            self.original_victim_pos = *target.get_position();
            self.victim_team = target.get_team_id();
        } else {
            let Some(pos) = context.goal_position else {
                return StateReturnType::Failed;
            };
            self.original_victim_pos = pos;
        }

        if !self.choose_weapon(context) {
            return StateReturnType::Failed;
        }

        if let Ok(mut owner) = owner_arc.write() {
            if let Some((weapon, _slot)) = owner.get_current_weapon() {
                if weapon.get_lock_on_range() > 0.0 {
                    owner.set_status(
                        ObjectStatusMaskType::from(ObjectStatusTypes::IgnoringStealth),
                        true,
                    );
                }
            }
            owner.set_status(
                ObjectStatusMaskType::from(ObjectStatusTypes::IsAttacking),
                true,
            );
        }

        StateReturnType::Continue
    }

    fn update(&mut self, context: &mut AIStateMachineContext) -> StateReturnType {
        // Attack state update logic
        let Some(owner_arc) = OBJECT_REGISTRY.get_object(context.owner_id) else {
            return StateReturnType::Failed;
        };
        let Ok(owner) = owner_arc.read() else {
            return StateReturnType::Failed;
        };
        if owner.is_out_of_ammo() && !owner.is_kind_of(KindOf::Projectile) {
            return StateReturnType::Failed;
        }

        if self.attacking_object {
            let Some(target_id) = context.goal_object else {
                return StateReturnType::Complete;
            };

            let Some(target_arc) = OBJECT_REGISTRY.get_object(target_id) else {
                return StateReturnType::Complete;
            };
            let Ok(target) = target_arc.read() else {
                return StateReturnType::Failed;
            };
            if target.is_effectively_dead() {
                return StateReturnType::Complete;
            }

            let relationship = owner.relationship_to(&target);
            if !target.test_status(ObjectStatusTypes::CanAttack) {
                if let Some(contain) = target.get_contain() {
                    if let Ok(contain_guard) = contain.lock() {
                        if contain_guard.is_garrisonable()
                            && contain_guard.get_contained_count() == 0
                            && relationship == Relationship::Neutral
                        {
                            return StateReturnType::Failed;
                        }
                    }
                }
            }

            if relationship != Relationship::Enemies {
                return StateReturnType::Failed;
            }

            if out_of_weapon_range_object(context) {
                return StateReturnType::Failed;
            }

            if want_to_squish_target(context) {
                return StateReturnType::Failed;
            }
        } else {
            if context.goal_position.is_none() {
                return StateReturnType::Failed;
            }

            if out_of_weapon_range_position(context) {
                return StateReturnType::Failed;
            }
        }

        if !self.choose_weapon(context) {
            return StateReturnType::Failed;
        }
        let Some((weapon, _slot)) = owner.get_current_weapon() else {
            return StateReturnType::Failed;
        };
        if weapon.get_max_shot_count() <= 0 {
            return StateReturnType::Failed;
        }

        StateReturnType::Continue
    }

    fn on_exit(&mut self, context: &mut AIStateMachineContext, _exit_type: StateExitType) {
        let _ = OBJECT_REGISTRY.with_object_mut(context.owner_id, |owner| {
            owner.set_status(
                ObjectStatusMaskType::from(ObjectStatusTypes::IsAttacking),
                false,
            );
            owner.set_status(
                ObjectStatusMaskType::from(ObjectStatusTypes::IgnoringStealth),
                false,
            );
        });
    }

    fn get_state_type(&self) -> AIStateType {
        if self.attacking_object {
            if self.force_attacking {
                AIStateType::ForceAttackObject
            } else if self.follow {
                AIStateType::AttackAndFollowObject
            } else {
                AIStateType::AttackObject
            }
        } else {
            if self.attack_area {
                AIStateType::AttackArea
            } else if self.follow {
                AIStateType::AttackMoveTo
            } else {
                AIStateType::AttackPosition
            }
        }
    }

    fn is_attack(&self) -> bool {
        true
    }
}

/// AI Guard State
#[derive(Debug)]
pub struct AIGuardState {
    guard_position: Option<Coord3D>,
    guard_object: Option<ObjectID>,
    guard_mode: GuardMode,
    scan_timer: u32,
    last_enemy_scan_time: u32,
    guard_machine: Option<AIGuardMachine>,
}

impl AIGuardState {
    pub fn new() -> Self {
        Self {
            guard_position: None,
            guard_object: None,
            guard_mode: GuardMode::Normal,
            scan_timer: 0,
            last_enemy_scan_time: 0,
            guard_machine: None,
        }
    }
}

impl AIState for AIGuardState {
    fn on_enter(&mut self, context: &mut AIStateMachineContext) -> StateReturnType {
        self.guard_position = context.goal_position;
        self.guard_object = context.goal_object;
        self.guard_mode = match context.int_value {
            0 => GuardMode::Normal,
            1 => GuardMode::GuardWithoutPursuit,
            2 => GuardMode::GuardFlyingUnitsOnly,
            _ => GuardMode::Normal,
        };

        if let Some(owner_arc) = get_legacy_object(context.owner_id) {
            let mut guard_machine = AIGuardMachine::new(Arc::downgrade(&owner_arc));

            if let Some(target_id) = context.goal_object {
                if let Some(target_arc) = get_legacy_object(target_id) {
                    guard_machine.set_target_to_guard(Some(&target_arc));
                }
            } else if let Some(pos) = context.goal_position {
                guard_machine.set_target_position_to_guard(&pos);
            } else if let Ok(owner_guard) = owner_arc.read() {
                guard_machine.set_target_position_to_guard(owner_guard.get_position());
            }

            guard_machine.set_guard_mode(self.guard_mode);
            if guard_machine.init_default_state().is_failure() {
                return StateReturnType::Failed;
            }
            let result = guard_machine.set_state(GuardStateType::Return);
            self.guard_machine = Some(guard_machine);
            return result;
        }

        StateReturnType::Continue
    }

    fn update(&mut self, _context: &mut AIStateMachineContext) -> StateReturnType {
        if let Some(guard_machine) = self.guard_machine.as_mut() {
            return guard_machine.update();
        }

        // Guard behavior - scan for enemies, respond to threats
        self.scan_timer += 1;

        if self.scan_timer >= 30 {
            // Scan every second
            self.scan_timer = 0;
            self.last_enemy_scan_time += 30;

            // Scan for enemies in guard range
            // If enemy found, attack based on guard mode
            // Guard mode influences pursuit/target filters
            // Guard modes influence pursuit behavior when an enemy is found
        }

        StateReturnType::Continue
    }

    fn on_exit(&mut self, _context: &mut AIStateMachineContext, _exit_type: StateExitType) {
        if let Some(mut guard_machine) = self.guard_machine.take() {
            let _ = guard_machine.halt();
        }
    }

    fn get_state_type(&self) -> AIStateType {
        AIStateType::Guard
    }

    fn is_attack(&self) -> bool {
        self.guard_machine
            .as_ref()
            .map(|machine| machine.is_in_attack_state())
            .unwrap_or(false)
    }

    fn is_guard_idle(&self) -> bool {
        self.guard_machine
            .as_ref()
            .map(|machine| machine.is_in_guard_idle_state())
            .unwrap_or(true)
    }
}

/// AI Follow Waypoint Path State
#[derive(Debug)]
pub struct AIFollowWaypointPathState {
    move_as_group: bool,
    is_follow_waypoint_path_state: bool,
    attack_follow: bool,
    group_offset: Coord2D,
    angle: Real,
    frames_sleeping: i32,
    current_waypoint: Option<u32>,
    prior_waypoint: Option<u32>,
    append_goal_position: bool,
    exact_path: bool,
}

impl AIFollowWaypointPathState {
    fn get_waypoint_link_count(&self, waypoint_id: u32) -> usize {
        let Ok(terrain) = get_terrain_logic().read() else {
            return 0;
        };
        terrain
            .get_waypoint_by_id(waypoint_id)
            .map(|w| w.get_num_links())
            .unwrap_or(0)
    }

    fn get_waypoint_link(&self, waypoint_id: u32, index: usize) -> Option<u32> {
        let Ok(terrain) = get_terrain_logic().read() else {
            return None;
        };
        terrain
            .get_waypoint_by_id(waypoint_id)
            .and_then(|w| w.get_link(index))
    }

    fn get_waypoint_location(&self, waypoint_id: u32) -> Option<Coord3D> {
        let Ok(terrain) = get_terrain_logic().read() else {
            return None;
        };
        terrain
            .get_waypoint_by_id(waypoint_id)
            .map(|w| *w.get_location())
    }

    fn calc_extra_path_distance(&self) -> Real {
        let mut extra = PATHFIND_CELL_SIZE_F / 10.0;
        let mut current = self.current_waypoint;
        let mut limit = 5;

        while let Some(current_id) = current {
            if limit == 0 {
                break;
            }
            limit -= 1;
            let link_count = self.get_waypoint_link_count(current_id);
            if link_count == 0 {
                return extra;
            }
            let Some(next_id) = self.get_waypoint_link(current_id, 0) else {
                return extra;
            };
            let Some(cur_loc) = self.get_waypoint_location(current_id) else {
                return extra;
            };
            let Some(next_loc) = self.get_waypoint_location(next_id) else {
                return extra;
            };
            let dx = next_loc.x - cur_loc.x;
            let dy = next_loc.y - cur_loc.y;
            extra += (dx * dx + dy * dy).sqrt();
            current = Some(next_id);
        }

        extra
    }

    fn get_next_waypoint(&mut self) -> Option<u32> {
        let current_id = self.current_waypoint?;
        let link_count = self.get_waypoint_link_count(current_id);
        if link_count == 0 {
            return None;
        }

        if link_count == 1 {
            let next_id = self.get_waypoint_link(current_id, 0)?;
            if self.prior_waypoint == Some(next_id) {
                return None;
            }
            self.prior_waypoint = Some(current_id);
            self.current_waypoint = Some(next_id);
            return Some(next_id);
        }

        let idx = game_logic_random_value(0, (link_count - 1) as u32) as usize;
        let next_id = self.get_waypoint_link(current_id, idx)?;
        self.prior_waypoint = Some(current_id);
        self.current_waypoint = Some(next_id);
        Some(next_id)
    }

    pub fn new(as_group: bool) -> Self {
        Self::new_with_exact(as_group, false)
    }

    pub fn new_with_exact(as_group: bool, exact: bool) -> Self {
        Self::new_with_exact_and_attack(as_group, exact, false)
    }

    pub fn new_with_exact_and_attack(as_group: bool, exact: bool, attack_follow: bool) -> Self {
        Self {
            move_as_group: as_group,
            is_follow_waypoint_path_state: true,
            attack_follow,
            group_offset: Coord2D::new(0.0, 0.0),
            angle: 0.0,
            frames_sleeping: 0,
            current_waypoint: None,
            prior_waypoint: None,
            append_goal_position: exact,
            exact_path: exact,
        }
    }

    fn compute_goal(&mut self, context: &mut AIStateMachineContext, _use_group_offsets: bool) {
        if self.current_waypoint.is_none() {
            self.current_waypoint = context.goal_waypoint;
        }
        let Some(waypoint_id) = self.current_waypoint else {
            return;
        };
        let Some(mut dest) = self.get_waypoint_location(waypoint_id) else {
            return;
        };

        let mut goal = dest;
        goal.x += self.group_offset.x;
        goal.y += self.group_offset.y;

        if let Ok(terrain) = get_terrain_logic().read() {
            goal.z = terrain.get_ground_height(goal.x, goal.y, None);
        }

        if let Some(terrain) = TheTerrainLogic::get() {
            let extent = terrain.get_maximum_pathfind_extent();
            let dest_in = dest.x >= extent.lo.x
                && dest.x <= extent.hi.x
                && dest.y >= extent.lo.y
                && dest.y <= extent.hi.y;
            let mut goal_in = goal.x >= extent.lo.x
                && goal.x <= extent.hi.x
                && goal.y >= extent.lo.y
                && goal.y <= extent.hi.y;

            if dest_in && !goal_in {
                if goal.x < extent.lo.x + PATHFIND_CELL_SIZE_F {
                    goal.x = extent.lo.x + PATHFIND_CELL_SIZE_F;
                }
                if goal.y < extent.lo.y + PATHFIND_CELL_SIZE_F {
                    goal.y = extent.lo.y + PATHFIND_CELL_SIZE_F;
                }
                if goal.x > extent.hi.x - PATHFIND_CELL_SIZE_F {
                    goal.x = extent.hi.x - PATHFIND_CELL_SIZE_F;
                }
                if goal.y > extent.hi.y - PATHFIND_CELL_SIZE_F {
                    goal.y = extent.hi.y - PATHFIND_CELL_SIZE_F;
                }
                goal_in = goal.x >= extent.lo.x
                    && goal.x <= extent.hi.x
                    && goal.y >= extent.lo.y
                    && goal.y <= extent.hi.y;
            }

            if !goal_in {
                self.append_goal_position = true;
                if let Some(ai) = OBJECT_REGISTRY
                    .with_object(context.owner_id, |owner| owner.get_ai_update_interface())
                    .flatten()
                {
                    if let Ok(mut ai_guard) = ai.lock() {
                        ai_guard.set_allow_invalid_position(true);
                    }
                }
            } else {
                self.append_goal_position = false;
            }
        }

        let is_projectile = OBJECT_REGISTRY
            .with_object(context.owner_id, |owner| owner.is_kind_of(KindOf::Projectile))
            .unwrap_or(false);
        if !self.has_next_waypoint() && is_projectile {
            if let Some(ai) = OBJECT_REGISTRY
                .with_object(context.owner_id, |owner| owner.get_ai_update_interface())
                .flatten()
            {
                if let Ok(ai_guard) = ai.lock() {
                    if let Some(locomotor) = ai_guard.get_cur_locomotor() {
                        if let Ok(mut locomotor_guard) = locomotor.lock() {
                            locomotor_guard.set_precise_z_pos(true);
                        }
                    }
                }
            }
        }
        if let Some(ai) = OBJECT_REGISTRY
            .with_object(context.owner_id, |owner| owner.get_ai_update_interface())
            .flatten()
        {
            if let Ok(mut ai_guard) = ai.lock() {
                let _ = ai_guard.set_path_extra_distance(self.calc_extra_path_distance());
            }
        }

        context.goal_position = Some(goal);
    }

    fn has_next_waypoint(&self) -> bool {
        let Some(current_id) = self.current_waypoint else {
            return false;
        };
        let link_count = self.get_waypoint_link_count(current_id);
        if link_count == 0 {
            return false;
        }
        if self.prior_waypoint.is_none() {
            return true;
        }
        if link_count > 1 {
            return true;
        }
        let Some(next_id) = self.get_waypoint_link(current_id, 0) else {
            return false;
        };
        self.prior_waypoint != Some(next_id)
    }
}

impl AIState for AIFollowWaypointPathState {
    fn on_enter(&mut self, context: &mut AIStateMachineContext) -> StateReturnType {
        self.append_goal_position = false;
        self.prior_waypoint = None;
        self.current_waypoint = context.goal_waypoint;

        if self.current_waypoint.is_none() && !self.move_as_group {
            return StateReturnType::Failed;
        }

        self.frames_sleeping = 0;
        self.group_offset = Coord2D::new(0.0, 0.0);
        self.compute_goal(context, self.move_as_group);
        StateReturnType::Continue
    }

    fn update(&mut self, context: &mut AIStateMachineContext) -> StateReturnType {
        let Some(goal_pos) = context.goal_position else {
            return StateReturnType::Failed;
        };
        let Some(current_pos) =
            OBJECT_REGISTRY.with_object(context.owner_id, |owner| *owner.get_position())
        else {
            return StateReturnType::Failed;
        };
        let delta = goal_pos - current_pos;
        let dist_sqr = delta.x * delta.x + delta.y * delta.y;
        let close_enough = PATHFIND_CLOSE_ENOUGH * PATHFIND_CLOSE_ENOUGH;

        if dist_sqr <= close_enough {
            if self.has_next_waypoint() {
                let _ = self.get_next_waypoint();
                self.compute_goal(context, self.move_as_group);
                return StateReturnType::Continue;
            }
            return StateReturnType::Complete;
        }

        StateReturnType::Continue
    }

    fn on_exit(&mut self, context: &mut AIStateMachineContext, _exit_type: StateExitType) {
        if let Some(ai) = OBJECT_REGISTRY
            .with_object(context.owner_id, |owner| owner.get_ai_update_interface())
            .flatten()
        {
            if let Ok(mut ai_guard) = ai.lock() {
                ai_guard.destroy_path();
            }
        }
    }

    fn get_state_type(&self) -> AIStateType {
        if self.attack_follow {
            if self.move_as_group {
                return AIStateType::AttackFollowWaypointPathAsTeam;
            }
            return AIStateType::AttackFollowWaypointPathAsIndividuals;
        }
        if self.move_as_group {
            if self.exact_path {
                AIStateType::FollowWaypointPathAsTeamExact
            } else {
                AIStateType::FollowWaypointPathAsTeam
            }
        } else {
            if self.exact_path {
                AIStateType::FollowWaypointPathAsIndividualsExact
            } else {
                AIStateType::FollowWaypointPathAsIndividuals
            }
        }
    }
}

/// AI Wander State
#[derive(Debug)]
pub struct AIWanderState {
    follow: AIFollowWaypointPathState,
    wait_frames: i32,
    timer: i32,
}

impl AIWanderState {
    pub fn new() -> Self {
        Self {
            follow: AIFollowWaypointPathState::new(false),
            wait_frames: 0,
            timer: 0,
        }
    }

    fn update_group_offset(&mut self, ai: &dyn crate::modules::AIUpdateInterface) {
        if let Some(locomotor) = ai.get_cur_locomotor() {
            if let Ok(locomotor_guard) = locomotor.lock() {
                let factor = locomotor_guard.template.wander_width_factor;
                if factor > 0.0 {
                    let mut delta = (factor + 0.5).floor() as i32;
                    if delta < 1 {
                        delta = 1;
                    }
                    let x =
                        get_game_logic_random_value(-delta, delta) as f32 * PATHFIND_CELL_SIZE_F;
                    let y =
                        get_game_logic_random_value(-delta, delta) as f32 * PATHFIND_CELL_SIZE_F;
                    self.follow.group_offset = Coord2D::new(x, y);
                }
            }
        }
    }
}

impl AIState for AIWanderState {
    fn on_enter(&mut self, context: &mut AIStateMachineContext) -> StateReturnType {
        self.follow.current_waypoint = context.goal_waypoint;
        self.follow.prior_waypoint = None;
        self.follow.group_offset = Coord2D::new(0.0, 0.0);

        if self.follow.current_waypoint.is_none() {
            return StateReturnType::Failed;
        }
        let Some(ai) = OBJECT_REGISTRY
            .with_object(context.owner_id, |owner| owner.get_ai_update_interface())
            .flatten()
        else {
            return StateReturnType::Failed;
        };

        if let Ok(ai_guard) = ai.lock() {
            self.update_group_offset(&*ai_guard);
        }

        self.timer = 0;
        self.wait_frames = 10 + ((context.owner_id & 0x7) as i32);
        self.follow.compute_goal(context, false);

        StateReturnType::Continue
    }

    fn update(&mut self, context: &mut AIStateMachineContext) -> StateReturnType {
        let Some((can_be_repulsed, vision_range, ai)) =
            OBJECT_REGISTRY.with_object(context.owner_id, |owner| {
                (
                    owner.is_kind_of(KindOf::CanBeRepulsed),
                    owner.get_vision_range(),
                    owner.get_ai_update_interface(),
                )
            })
        else {
            return StateReturnType::Failed;
        };

        if can_be_repulsed {
            self.timer -= 1;
            if self.timer < 0 {
                self.timer = self.wait_frames;
                let enemy_id = THE_AI
                    .read()
                    .ok()
                    .and_then(|ai| {
                        ai.find_closest_repulsor(context.owner_id, vision_range)
                            .ok()
                    })
                    .flatten();
                if enemy_id.is_some() {
                    return StateReturnType::Failed;
                }
            }
        }

        if goal_reached(context) {
            if self.follow.get_next_waypoint().is_none() {
                return StateReturnType::Complete;
            }

            if let Some(ai) = ai {
                if let Ok(ai_guard) = ai.lock() {
                    self.update_group_offset(&*ai_guard);
                }
            }

            self.follow.compute_goal(context, false);
            return StateReturnType::Continue;
        }

        StateReturnType::Continue
    }

    fn on_exit(&mut self, context: &mut AIStateMachineContext, _exit_type: StateExitType) {
        if let Some(ai) = OBJECT_REGISTRY
            .with_object(context.owner_id, |owner| owner.get_ai_update_interface())
            .flatten()
        {
            if let Ok(mut ai_guard) = ai.lock() {
                ai_guard.destroy_path();
            }
        }
    }

    fn get_state_type(&self) -> AIStateType {
        AIStateType::Wander
    }
}

/// AI Wander In Place State
#[derive(Debug)]
pub struct AIWanderInPlaceState {
    origin: Coord3D,
    goal_position: Coord3D,
    wait_frames: i32,
    timer: i32,
}

impl AIWanderInPlaceState {
    pub fn new() -> Self {
        Self {
            origin: Coord3D::new(0.0, 0.0, 0.0),
            goal_position: Coord3D::new(0.0, 0.0, 0.0),
            wait_frames: 0,
            timer: 0,
        }
    }

    fn choose_new_goal(&mut self, ai: &dyn crate::modules::AIUpdateInterface) {
        let mut delta = 3;
        if let Some(locomotor) = ai.get_cur_locomotor() {
            if let Ok(locomotor_guard) = locomotor.lock() {
                delta = ((locomotor_guard.template.wander_about_point_radius
                    / PATHFIND_CELL_SIZE_F)
                    + 0.5)
                    .floor() as i32;
            }
        }

        let offset_x = get_game_logic_random_value(-delta, delta) as f32 * PATHFIND_CELL_SIZE_F;
        let offset_y = get_game_logic_random_value(-delta, delta) as f32 * PATHFIND_CELL_SIZE_F;
        self.goal_position = self.origin;
        self.goal_position.x += offset_x;
        self.goal_position.y += offset_y;
    }
}

impl AIState for AIWanderInPlaceState {
    fn on_enter(&mut self, context: &mut AIStateMachineContext) -> StateReturnType {
        let Some(ai) = OBJECT_REGISTRY
            .with_object(context.owner_id, |owner| {
                self.origin = *owner.get_position();
                owner.get_ai_update_interface()
            })
            .flatten()
        else {
            return StateReturnType::Failed;
        };
        if let Ok(ai_guard) = ai.lock() {
            ai_guard.choose_locomotor_set(LocomotorSetType::Wander);
            self.choose_new_goal(&*ai_guard);
        }

        self.timer = 0;
        self.wait_frames = 10 + ((context.owner_id & 0x7) as i32);
        context.goal_position = Some(self.goal_position);

        StateReturnType::Continue
    }

    fn update(&mut self, context: &mut AIStateMachineContext) -> StateReturnType {
        let Some((can_be_repulsed, vision_range, ai)) =
            OBJECT_REGISTRY.with_object(context.owner_id, |owner| {
                (
                    owner.is_kind_of(KindOf::CanBeRepulsed),
                    owner.get_vision_range(),
                    owner.get_ai_update_interface(),
                )
            })
        else {
            return StateReturnType::Failed;
        };
        let Some(ai) = ai else {
            return StateReturnType::Failed;
        };

        if can_be_repulsed {
            self.timer -= 1;
            if self.timer < 0 {
                self.timer = self.wait_frames;
                let enemy_id = THE_AI
                    .read()
                    .ok()
                    .and_then(|ai| {
                        ai.find_closest_repulsor(context.owner_id, vision_range)
                            .ok()
                    })
                    .flatten();
                if enemy_id.is_some() {
                    return StateReturnType::Failed;
                }
            }
        }

        if goal_reached(context) {
            if let Ok(ai_guard) = ai.lock() {
                self.choose_new_goal(&*ai_guard);
            }
            context.goal_position = Some(self.goal_position);
            return StateReturnType::Continue;
        }

        StateReturnType::Continue
    }

    fn on_exit(&mut self, context: &mut AIStateMachineContext, _exit_type: StateExitType) {
        if let Some(ai) = OBJECT_REGISTRY
            .with_object(context.owner_id, |owner| owner.get_ai_update_interface())
            .flatten()
        {
            if let Ok(mut ai_guard) = ai.lock() {
                ai_guard.destroy_path();
                ai_guard.choose_locomotor_set(LocomotorSetType::Normal);
            }
        }
    }

    fn get_state_type(&self) -> AIStateType {
        AIStateType::WanderInPlace
    }
}

/// AI Panic State
#[derive(Debug)]
pub struct AIPanicState {
    follow: AIFollowWaypointPathState,
    wait_frames: i32,
    timer: i32,
}

impl AIPanicState {
    pub fn new() -> Self {
        Self {
            follow: AIFollowWaypointPathState::new(false),
            wait_frames: 0,
            timer: 0,
        }
    }

    fn update_group_offset(&mut self, ai: &dyn crate::modules::AIUpdateInterface) {
        if let Some(locomotor) = ai.get_cur_locomotor() {
            if let Ok(locomotor_guard) = locomotor.lock() {
                let factor = locomotor_guard.template.wander_width_factor;
                if factor > 0.0 {
                    let mut delta = (factor + 0.5).floor() as i32;
                    if delta < 1 {
                        delta = 1;
                    }
                    let x =
                        get_game_logic_random_value(-delta, delta) as f32 * PATHFIND_CELL_SIZE_F;
                    let y =
                        get_game_logic_random_value(-delta, delta) as f32 * PATHFIND_CELL_SIZE_F;
                    self.follow.group_offset = Coord2D::new(x, y);
                }
            }
        }
    }
}

impl AIState for AIPanicState {
    fn on_enter(&mut self, context: &mut AIStateMachineContext) -> StateReturnType {
        self.follow.current_waypoint = context.goal_waypoint;
        self.follow.prior_waypoint = None;
        self.follow.group_offset = Coord2D::new(0.0, 0.0);

        if self.follow.current_waypoint.is_none() {
            return StateReturnType::Failed;
        }
        let Some(ai) = OBJECT_REGISTRY
            .with_object(context.owner_id, |owner| owner.get_ai_update_interface())
            .flatten()
        else {
            return StateReturnType::Failed;
        };

        if let Ok(ai_guard) = ai.lock() {
            self.update_group_offset(&*ai_guard);
        }

        self.follow.compute_goal(context, false);
        self.timer = 0;
        self.wait_frames = 10 + ((context.owner_id & 0x7) as i32);

        if let Ok(mut owner) = owner_arc.write() {
            owner.set_model_condition_state(ModelConditionFlags::PANICKING);
        }

        StateReturnType::Continue
    }

    fn update(&mut self, context: &mut AIStateMachineContext) -> StateReturnType {
        let Some((can_be_repulsed, vision_range, ai)) =
            OBJECT_REGISTRY.with_object(context.owner_id, |owner| {
                (
                    owner.is_kind_of(KindOf::CanBeRepulsed),
                    owner.get_vision_range(),
                    owner.get_ai_update_interface(),
                )
            })
        else {
            return StateReturnType::Failed;
        };

        if can_be_repulsed {
            self.timer -= 1;
            if self.timer < 0 {
                self.timer = self.wait_frames;
                let enemy_id = THE_AI
                    .read()
                    .ok()
                    .and_then(|ai| {
                        ai.find_closest_repulsor(context.owner_id, vision_range)
                            .ok()
                    })
                    .flatten();
                if enemy_id.is_some() {
                    return StateReturnType::Failed;
                }
            }
        }

        if goal_reached(context) {
            if self.follow.get_next_waypoint().is_none() {
                return StateReturnType::Complete;
            }

            if let Some(ai) = ai {
                if let Ok(ai_guard) = ai.lock() {
                    self.update_group_offset(&*ai_guard);
                }
            }

            self.follow.compute_goal(context, false);
            return StateReturnType::Continue;
        }

        StateReturnType::Continue
    }

    fn on_exit(&mut self, context: &mut AIStateMachineContext, _exit_type: StateExitType) {
        let _ = OBJECT_REGISTRY.with_object_mut(context.owner_id, |owner| {
            owner.clear_model_condition_state(ModelConditionFlags::PANICKING);
        });
    }

    fn get_state_type(&self) -> AIStateType {
        AIStateType::Panic
    }
}

/// AI Guard Tunnel Network State
#[derive(Debug)]
pub struct AIGuardTunnelNetworkState {
    guard_mode: GuardMode,
    guard_machine: Option<AITNGuardMachine>,
}

impl AIGuardTunnelNetworkState {
    pub fn new() -> Self {
        Self {
            guard_mode: GuardMode::Normal,
            guard_machine: None,
        }
    }
}

impl AIState for AIGuardTunnelNetworkState {
    fn on_enter(&mut self, context: &mut AIStateMachineContext) -> StateReturnType {
        self.guard_mode = match context.int_value {
            0 => GuardMode::Normal,
            1 => GuardMode::GuardWithoutPursuit,
            2 => GuardMode::GuardFlyingUnitsOnly,
            _ => GuardMode::Normal,
        };

        if let Some(owner_arc) = get_legacy_object(context.owner_id) {
            let mut guard_machine = AITNGuardMachine::new(Arc::downgrade(&owner_arc));
            guard_machine.set_guard_mode(self.guard_mode);
            if let Ok(owner_guard) = owner_arc.read() {
                guard_machine.set_target_position_to_guard(owner_guard.get_position());
            }
            if guard_machine.init_default_state().is_failure() {
                return StateReturnType::Failure;
            }
            let result = guard_machine.set_state(TNGuardStateType::Return);
            self.guard_machine = Some(guard_machine);
            return result;
        }

        StateReturnType::Continue
    }

    fn update(&mut self, _context: &mut AIStateMachineContext) -> StateReturnType {
        if let Some(guard_machine) = self.guard_machine.as_mut() {
            return guard_machine.update();
        }
        StateReturnType::Continue
    }

    fn on_exit(&mut self, _context: &mut AIStateMachineContext, _exit_type: StateExitType) {
        if let Some(mut guard_machine) = self.guard_machine.take() {
            let _ = guard_machine.halt();
        }
    }

    fn get_state_type(&self) -> AIStateType {
        AIStateType::GuardTunnelNetwork
    }

    fn is_attack(&self) -> bool {
        self.guard_machine
            .as_ref()
            .map(|machine| machine.is_in_attack_state())
            .unwrap_or(false)
    }

    fn is_guard_idle(&self) -> bool {
        self.guard_machine
            .as_ref()
            .map(|machine| machine.is_in_guard_idle_state())
            .unwrap_or(true)
    }
}

/// AI Guard Retaliate State
#[derive(Debug)]
pub struct AIGuardRetaliateState {
    guard_machine: Option<AIGuardRetaliateMachine>,
}

impl AIGuardRetaliateState {
    pub fn new() -> Self {
        Self {
            guard_machine: None,
        }
    }
}

impl AIState for AIGuardRetaliateState {
    fn on_enter(&mut self, context: &mut AIStateMachineContext) -> StateReturnType {
        if let Some(owner_arc) = get_legacy_object(context.owner_id) {
            let mut guard_machine = AIGuardRetaliateMachine::new(Arc::downgrade(&owner_arc));
            if let Some(pos) = context.goal_position {
                guard_machine.set_target_position_to_guard(&pos);
            } else if let Ok(owner_guard) = owner_arc.read() {
                guard_machine.set_target_position_to_guard(owner_guard.get_position());
            }
            if let Some(target_id) = context.goal_object {
                guard_machine.set_nemesis_id(target_id);
            }
            if guard_machine.init_default_state().is_failure() {
                return StateReturnType::Failure;
            }
            let result = guard_machine.set_state(GuardRetaliateStateType::Return);
            self.guard_machine = Some(guard_machine);
            return result;
        }

        StateReturnType::Continue
    }

    fn update(&mut self, _context: &mut AIStateMachineContext) -> StateReturnType {
        if let Some(guard_machine) = self.guard_machine.as_mut() {
            return guard_machine.update();
        }
        StateReturnType::Continue
    }

    fn on_exit(&mut self, _context: &mut AIStateMachineContext, _exit_type: StateExitType) {
        if let Some(mut guard_machine) = self.guard_machine.take() {
            let _ = guard_machine.halt();
        }
    }

    fn get_state_type(&self) -> AIStateType {
        AIStateType::GuardRetaliate
    }

    fn is_attack(&self) -> bool {
        self.guard_machine
            .as_ref()
            .map(|machine| machine.is_in_attack_state())
            .unwrap_or(false)
    }
}

/// AI Hunt State
#[derive(Debug)]
pub struct AIHuntState {
    next_enemy_scan_time: u32,
    hunt_radius: Real,
    current_target: Option<ObjectID>,
    hunt_machine: Option<AIAttackThenIdleStateMachine>,
}

impl AIHuntState {
    pub fn new() -> Self {
        Self {
            next_enemy_scan_time: 0,
            hunt_radius: 9999.9,
            current_target: None,
            hunt_machine: None,
        }
    }

    fn scan_for_enemies(&mut self, context: &mut AIStateMachineContext) -> Option<ObjectID> {
        let ai = THE_AI.read().ok()?;
        let attack_priority = resolve_attack_priority_info_for_object(context.owner_id);
        ai.find_closest_enemy(
            context.owner_id,
            self.hunt_radius,
            search_qualifiers::CAN_ATTACK,
            attack_priority.as_ref(),
            None,
        )
        .ok()
        .flatten()
    }
}

impl AIState for AIHuntState {
    fn on_enter(&mut self, context: &mut AIStateMachineContext) -> StateReturnType {
        self.current_target = None;
        self.hunt_machine = None;

        let now = TheGameLogic::get_frame();
        let sleep_time = game_logic_random_value(0, LOGICFRAMES_PER_SECOND);
        self.next_enemy_scan_time = now.wrapping_add(sleep_time);

        if let Some(owner_arc) = get_legacy_object(context.owner_id) {
            let mut hunt_machine = AIAttackThenIdleStateMachine::new(
                Arc::downgrade(&owner_arc),
                "AIAttackThenIdleStateMachine",
            );
            let result = hunt_machine.init_default_state();
            self.hunt_machine = Some(hunt_machine);
            return result;
        }

        StateReturnType::Continue
    }

    fn update(&mut self, context: &mut AIStateMachineContext) -> StateReturnType {
        let Some(hunt_machine) = self.hunt_machine.as_mut() else {
            return StateReturnType::Failed;
        };

        let current_frame = TheGameLogic::get_frame();
        if current_frame >= self.next_enemy_scan_time {
            let Some(owner_arc) = get_legacy_object(context.owner_id) else {
                return StateReturnType::Failed;
            };
            let Ok(owner) = owner_arc.read() else {
                return StateReturnType::Failed;
            };

            if owner.is_out_of_ammo() && !owner.is_kind_of(KindOf::Projectile) {
                return StateReturnType::Failed;
            }

            if let Some(ai) = owner.get_ai_update_interface() {
                if let Ok(ai_guard) = ai.lock() {
                    if let Some(crate_obj) = ai_guard.check_for_crate_to_pickup() {
                        hunt_machine.set_goal_object(crate_obj.read().ok().map(|g| g.get_id()));
                        let _ = hunt_machine.set_state(LegacyAIStateType::PickUpCrate);
                        return StateReturnType::Continue;
                    }
                }
            }

            self.next_enemy_scan_time = current_frame + LOGICFRAMES_PER_SECOND;

            let victim = self.scan_for_enemies(context);
            self.current_target = victim;
            let victim_arc = victim.and_then(get_legacy_object);
            hunt_machine.set_goal_object(victim_arc.as_ref().and_then(|a| a.read().ok().map(|g| g.get_id())));

            if hunt_machine.get_current_state_id() == Some(LegacyAIStateType::Idle as u32)
                && victim_arc.is_some()
            {
                let _ = hunt_machine.set_state(LegacyAIStateType::AttackObject);
            }

            if let Some(player_arc) = owner.get_controlling_player() {
                if let Ok(player) = player_arc.read() {
                    if !player.get_units_should_hunt()
                        && hunt_machine.get_current_state_id()
                            == Some(LegacyAIStateType::Idle as u32)
                        && victim_arc.is_none()
                    {
                        return StateReturnType::Complete;
                    }
                }
            }
        }

        hunt_machine.update()
    }

    fn on_exit(&mut self, _context: &mut AIStateMachineContext, _exit_type: StateExitType) {
        self.current_target = None;
        if let Some(mut hunt_machine) = self.hunt_machine.take() {
            let _ = hunt_machine.halt();
        }
    }

    fn get_state_type(&self) -> AIStateType {
        AIStateType::Hunt
    }

    fn is_attack(&self) -> bool {
        self.current_target.is_some()
    }
}

/// AI Move And Tighten State
///
/// This state is used to tighten up group formations when units are too spread out.
/// Matches C++ AIMoveAndTightenState from AIStates.cpp lines 2181-2250
#[derive(Debug)]
pub struct AIMoveAndTightenState {
    goal_position: Coord3D,
    path_goal_position: Coord3D,
    path_timestamp: u32,
    ok_to_repath_times: i32,
    check_for_path: bool,
    waiting_for_path: bool,
    formation_config: FormationConfig,
    spread_threshold: Real,
}

impl AIMoveAndTightenState {
    fn compute_path(&mut self, context: &mut AIStateMachineContext) -> bool {
        let Some(goal_pos) = context.goal_position else {
            return false;
        };
        let Some(start_pos) =
            OBJECT_REGISTRY.with_object(context.owner_id, |owner| *owner.get_position())
        else {
            return false;
        };

        let path_result = with_ai_integration(|manager| {
            manager.request_pathfinding(&start_pos, &goal_pos, SURFACE_GROUND, false)
        });

        if let Some(Ok(Some(path))) = path_result {
            context.goal_path = path;
            self.waiting_for_path = false;
            return true;
        }

        context.goal_path = vec![goal_pos];
        self.waiting_for_path = false;
        true
    }

    pub fn new() -> Self {
        Self {
            goal_position: Coord3D::new(0.0, 0.0, 0.0),
            path_goal_position: Coord3D::new(0.0, 0.0, 0.0),
            path_timestamp: 0,
            ok_to_repath_times: 1,
            check_for_path: true,
            waiting_for_path: false,
            formation_config: FormationConfig::default(),
            spread_threshold: 50.0, // Matches C++ default threshold
        }
    }

    /// Check if group needs tightening
    ///
    /// # Arguments
    ///
    /// * `group_positions` - Positions of all units in the group
    ///
    /// # Returns
    ///
    /// Returns true if the group spread exceeds the threshold
    pub fn needs_tightening(&self, group_positions: &[Coord3D]) -> bool {
        is_group_too_spread(group_positions, self.spread_threshold)
    }

    /// Calculate spread distance for diagnostic purposes
    pub fn get_group_spread(&self, group_positions: &[Coord3D]) -> Real {
        calculate_group_spread(group_positions)
    }
}

impl AIState for AIMoveAndTightenState {
    fn on_enter(&mut self, context: &mut AIStateMachineContext) -> StateReturnType {
        // Matches C++ AIMoveAndTightenState::onEnter() from AIStates.cpp line 2211
        self.ok_to_repath_times = 1;
        self.check_for_path = true;
        self.waiting_for_path = false;

        if let Some(goal_pos) = context.goal_position {
            self.goal_position = goal_pos;
            self.compute_path(context);
        }

        StateReturnType::Continue
    }

    fn update(&mut self, context: &mut AIStateMachineContext) -> StateReturnType {
        // Matches C++ AIMoveAndTightenState::update() from AIStates.cpp line 2225

        if self.check_for_path {
            if !self.waiting_for_path && !context.goal_path.is_empty() {
                self.check_for_path = false;
            }
        }

        // Check if we've reached the destination
        if context.goal_position.is_some() {
            StateReturnType::Continue
        } else {
            StateReturnType::Failed
        }
    }

    fn on_exit(&mut self, context: &mut AIStateMachineContext, _exit_type: StateExitType) {
        if let Some(ai) = OBJECT_REGISTRY
            .with_object(context.owner_id, |owner| owner.get_ai_update_interface())
            .flatten()
        {
            if let Ok(mut ai_guard) = ai.lock() {
                ai_guard.destroy_path();
            }
        }
    }

    fn get_state_type(&self) -> AIStateType {
        AIStateType::MoveAndTighten
    }
}

/// AI Move Away From Repulsors State
/// Matches C++ AIMoveAwayFromRepulsorsState from AIStates.cpp lines 2263-2312
#[derive(Debug)]
pub struct AIMoveAwayFromRepulsorsState {
    goal_position: Coord3D,
    ok_to_repath_times: i32,
    check_for_path: bool,
    waiting_for_path: bool,
}

impl AIMoveAwayFromRepulsorsState {
    pub fn new() -> Self {
        Self {
            goal_position: Coord3D::new(0.0, 0.0, 0.0),
            ok_to_repath_times: 1,
            check_for_path: true,
            waiting_for_path: false,
        }
    }

    fn compute_path(&mut self, context: &mut AIStateMachineContext) -> bool {
        if self.ok_to_repath_times <= 0 {
            return false;
        }
        self.ok_to_repath_times -= 1;

        let Some(goal_pos) = context.goal_position else {
            return false;
        };
        let Some(start_pos) =
            OBJECT_REGISTRY.with_object(context.owner_id, |owner| *owner.get_position())
        else {
            return false;
        };

        let path_result = with_ai_integration(|manager| {
            manager.request_pathfinding(&start_pos, &goal_pos, SURFACE_GROUND, false)
        });

        if let Some(Ok(Some(path))) = path_result {
            context.goal_path = path;
            self.waiting_for_path = false;
            return true;
        }

        context.goal_path = vec![goal_pos];
        self.waiting_for_path = false;
        true
    }
}

impl AIState for AIMoveAwayFromRepulsorsState {
    fn on_enter(&mut self, context: &mut AIStateMachineContext) -> StateReturnType {
        let Some(owner_arc) = OBJECT_REGISTRY.get_object(context.owner_id) else {
            return StateReturnType::Failed;
        };
        let Ok(owner) = owner_arc.read() else {
            return StateReturnType::Failed;
        };

        let enemy_id = THE_AI
            .read()
            .ok()
            .and_then(|ai| {
                ai.find_closest_repulsor(context.owner_id, owner.get_vision_range())
                    .ok()
            })
            .flatten();
        let Some(enemy_id) = enemy_id else {
            return StateReturnType::Failed;
        };
        let Some(enemy_arc) = OBJECT_REGISTRY.get_object(enemy_id) else {
            return StateReturnType::Failed;
        };
        let Ok(enemy) = enemy_arc.read() else {
            return StateReturnType::Failed;
        };

        if let Some(ai) = owner.get_ai_update_interface() {
            if let Ok(mut ai_guard) = ai.lock() {
                let _ = ai_guard.choose_locomotor_set(LocomotorSetType::Panic);
            }
        }

        let owner_pos = *owner.get_position();
        let enemy_pos = *enemy.get_position();
        let mut dx = owner_pos.x - enemy_pos.x;
        let mut dy = owner_pos.y - enemy_pos.y;
        let len = (dx * dx + dy * dy).sqrt();
        if len < 0.001 {
            dx = 1.0;
            dy = 0.0;
        } else {
            dx /= len;
            dy /= len;
        }

        let flee_dist = owner.get_vision_range();
        self.goal_position = Coord3D::new(
            owner_pos.x + dx * flee_dist,
            owner_pos.y + dy * flee_dist,
            owner_pos.z,
        );
        context.goal_position = Some(self.goal_position);

        self.ok_to_repath_times = 1;
        self.check_for_path = true;
        self.waiting_for_path = false;
        self.compute_path(context);

        StateReturnType::Continue
    }

    fn update(&mut self, context: &mut AIStateMachineContext) -> StateReturnType {
        if self.check_for_path {
            if !self.waiting_for_path && !context.goal_path.is_empty() {
                self.check_for_path = false;
                if let Some(last) = context.goal_path.last().copied() {
                    self.goal_position = last;
                    context.goal_position = Some(last);
                }
            }
        }

        if context.goal_position.is_some() {
            StateReturnType::Continue
        } else {
            StateReturnType::Failed
        }
    }

    fn on_exit(&mut self, context: &mut AIStateMachineContext, _exit_type: StateExitType) {
        let _ = OBJECT_REGISTRY.with_object_mut(context.owner_id, |owner| {
            owner.clear_model_condition_state(ModelConditionFlags::PANICKING);
        });
        if let Some(ai) = OBJECT_REGISTRY
            .with_object(context.owner_id, |owner| owner.get_ai_update_interface())
            .flatten()
        {
            if let Ok(mut ai_guard) = ai.lock() {
                ai_guard.destroy_path();
                ai_guard.choose_locomotor_set(LocomotorSetType::Normal);
            }
        }
    }

    fn get_state_type(&self) -> AIStateType {
        AIStateType::MoveAwayFromRepulsors
    }
}

/// Main AI State Machine
#[derive(Debug)]
pub struct AIStateMachine {
    owner_id: ObjectID,
    name: String,
    current_state: Option<Box<dyn AIState>>,
    context: AIStateMachineContext,
    temporary_state: Option<Box<dyn AIState>>,
    temporary_state_frame_end: Option<u32>,
}

impl AIStateMachine {
    /// Create new AI state machine
    pub fn new(owner_id: ObjectID, name: String) -> Self {
        let mut machine = Self {
            owner_id,
            name,
            current_state: None,
            context: AIStateMachineContext::default(),
            temporary_state: None,
            temporary_state_frame_end: None,
        };

        machine.context.owner_id = owner_id;

        // Start in idle state
        machine.set_state(AIStateType::Idle);

        machine
    }

    /// Clear state machine
    pub fn clear(&mut self) {
        self.current_state = None;
        self.temporary_state = None;
        self.temporary_state_frame_end = None;
        self.context = AIStateMachineContext::default();
        self.context.owner_id = self.owner_id;
    }

    /// Reset to default state
    pub fn reset_to_default_state(&mut self) -> StateReturnType {
        self.set_state(AIStateType::Idle)
    }

    /// Set new state
    pub fn set_state(&mut self, state_type: AIStateType) -> StateReturnType {
        // Exit current state
        if let Some(mut current) = self.current_state.take() {
            current.on_exit(&mut self.context, StateExitType::Normal);
        }

        // Create new state
        let new_state: Box<dyn AIState> = match state_type {
            AIStateType::Idle => Box::new(AIIdleState::new(true)),
            AIStateType::MoveTo => Box::new(AIMoveToState::new()),
            AIStateType::MoveOutOfTheWay => Box::new(AIMoveOutOfTheWayState::new()),
            AIStateType::AttackObject => Box::new(AIAttackState::new(false, true, false, false)),
            AIStateType::AttackPosition => Box::new(AIAttackState::new(false, false, false, false)),
            AIStateType::ForceAttackObject => {
                Box::new(AIAttackState::new(false, true, true, false))
            }
            AIStateType::AttackAndFollowObject => {
                Box::new(AIAttackState::new(true, true, false, false))
            }
            AIStateType::AttackMoveTo => Box::new(AIAttackState::new(true, false, false, false)),
            AIStateType::AttackArea => Box::new(AIAttackState::new(false, false, false, true)),
            AIStateType::Guard => Box::new(AIGuardState::new()),
            AIStateType::GuardTunnelNetwork => Box::new(AIGuardTunnelNetworkState::new()),
            AIStateType::GuardRetaliate => Box::new(AIGuardRetaliateState::new()),
            AIStateType::Hunt => Box::new(AIHuntState::new()),
            AIStateType::FollowWaypointPathAsTeam => Box::new(AIFollowWaypointPathState::new(true)),
            AIStateType::FollowWaypointPathAsIndividuals => {
                Box::new(AIFollowWaypointPathState::new(false))
            }
            AIStateType::FollowWaypointPathAsTeamExact => {
                Box::new(AIFollowWaypointPathState::new_with_exact(true, true))
            }
            AIStateType::FollowWaypointPathAsIndividualsExact => {
                Box::new(AIFollowWaypointPathState::new_with_exact(false, true))
            }
            AIStateType::AttackFollowWaypointPathAsTeam => Box::new(
                AIFollowWaypointPathState::new_with_exact_and_attack(true, false, true),
            ),
            AIStateType::AttackFollowWaypointPathAsIndividuals => Box::new(
                AIFollowWaypointPathState::new_with_exact_and_attack(false, false, true),
            ),
            AIStateType::MoveAndTighten => Box::new(AIMoveAndTightenState::new()),
            AIStateType::MoveAndEvacuate => Box::new(AIMoveAndEvacuateState::new(false)),
            AIStateType::MoveAndEvacuateAndExit => Box::new(AIMoveAndEvacuateState::new(true)),
            AIStateType::MoveAndDelete => Box::new(AIMoveAndDeleteState::new()),
            AIStateType::ExitInstantly => Box::new(AIExitInstantlyState::new()),
            AIStateType::GetRepaired => Box::new(AIGetRepairedState::new()),
            AIStateType::MoveAwayFromRepulsors => Box::new(AIMoveAwayFromRepulsorsState::new()),
            AIStateType::Wander => Box::new(AIWanderState::new()),
            AIStateType::WanderInPlace => Box::new(AIWanderInPlaceState::new()),
            AIStateType::Panic => Box::new(AIPanicState::new()),
            AIStateType::FollowPath => Box::new(AIFollowPathState::new()),
            AIStateType::FollowExitProductionPath => {
                Box::new(AIFollowExitProductionPathState::new())
            }
            AIStateType::Wait => Box::new(AIWaitState::new()),
            AIStateType::Dead => Box::new(AIDeadState::new()),
            AIStateType::Dock => Box::new(AIDockState::new()),
            AIStateType::Enter => Box::new(AIEnterState::new()),
            AIStateType::Exit => Box::new(AIExitState::new()),
            AIStateType::PickUpCrate => Box::new(AIPickUpCrateState::new()),
            AIStateType::AttackSquad => Box::new(AIAttackSquadState::new()),
            AIStateType::HackInternet => Box::new(AIHackInternetState::new()),
            AIStateType::FaceObject => Box::new(AIFaceObjectState::new()),
            AIStateType::FacePosition => Box::new(AIFacePositionState::new()),
            AIStateType::RappelInto => Box::new(AIRappelIntoState::new()),
            AIStateType::CombatDrop => Box::new(AICombatDropState::new()),
            AIStateType::Busy => Box::new(AIBusyState::new()),
        };

        // Enter new state
        let mut new_state = new_state;
        let result = new_state.on_enter(&mut self.context);
        self.current_state = Some(new_state);

        result
    }

    /// Set temporary state
    pub fn set_temporary_state(
        &mut self,
        state_type: AIStateType,
        frame_limit: u32,
    ) -> StateReturnType {
        // Create temporary state
        let temp_state: Box<dyn AIState> = match state_type {
            AIStateType::MoveOutOfTheWay => Box::new(AIMoveOutOfTheWayState::new()),
            AIStateType::MoveAndTighten => Box::new(AIMoveAndTightenState::new()),
            _ => return StateReturnType::Failed, // Only certain states can be temporary
        };

        let mut temp_state = temp_state;
        let result = temp_state.on_enter(&mut self.context);

        self.temporary_state = Some(temp_state);
        self.temporary_state_frame_end = Some(frame_limit);

        result
    }

    /// Update state machine
    pub fn update_state_machine(&mut self) -> StateReturnType {
        let current_frame = TheGameLogic::get_frame();

        // Handle temporary state first
        if let Some(ref mut temp_state) = self.temporary_state {
            if let Some(end_frame) = self.temporary_state_frame_end {
                if current_frame >= end_frame {
                    // Temporary state expired
                    temp_state.on_exit(&mut self.context, StateExitType::Normal);
                    self.temporary_state = None;
                    self.temporary_state_frame_end = None;
                } else {
                    // Update temporary state
                    return temp_state.update(&mut self.context);
                }
            }
        }

        // Update main state
        if let Some(ref mut current) = self.current_state {
            current.update(&mut self.context)
        } else {
            // No current state, reset to default
            self.reset_to_default_state()
        }
    }

    /// Set goal path
    pub fn set_goal_path(&mut self, path: &[Coord3D]) {
        self.context.goal_path = path.to_vec();
    }

    /// Add to goal path
    pub fn add_to_goal_path(&mut self, path_point: &Coord3D) {
        self.context.goal_path.push(*path_point);
    }

    /// Set goal waypoint
    pub fn set_goal_waypoint(&mut self, waypoint_id: u32) {
        self.context.goal_waypoint = Some(waypoint_id);
    }

    /// Set goal object
    pub fn set_goal_object(&mut self, object_id: ObjectID) {
        self.context.goal_object = Some(object_id);
    }

    /// Set goal position
    pub fn set_goal_position(&mut self, position: Coord3D) {
        self.context.goal_position = Some(position);
    }

    /// Get current state type
    pub fn get_current_state_type(&self) -> Option<AIStateType> {
        self.current_state.as_ref().map(|s| s.get_state_type())
    }

    /// Check if in attack state
    pub fn is_in_attack_state(&self) -> bool {
        self.current_state.as_ref().map_or(false, |s| s.is_attack())
    }

    /// Check if idle
    pub fn is_idle(&self) -> bool {
        self.current_state.as_ref().map_or(false, |s| s.is_idle())
    }

    /// Check if busy
    pub fn is_busy(&self) -> bool {
        self.current_state.as_ref().map_or(false, |s| s.is_busy())
    }

    /// Check if guard idle
    pub fn is_guard_idle(&self) -> bool {
        self.current_state
            .as_ref()
            .map_or(false, |s| s.is_guard_idle())
    }
}

/// AI Command Interface implementation for state machine
impl AiCommandInterface for AIStateMachine {
    fn ai_do_command(&mut self, params: &AiCommandParams) -> Result<(), AiError> {
        // Update context with command parameters
        self.context.goal_object = params.obj;
        self.context.goal_position = if params.pos != Coord3D::new(0.0, 0.0, 0.0) {
            Some(params.pos)
        } else {
            None
        };
        if self.context.goal_position.is_none() {
            if let Some(trigger_id) = params.polygon {
                if let Ok(terrain_guard) = get_terrain_logic().read() {
                    if let Some(trigger) = terrain_guard.get_trigger_areas().get_by_id(trigger_id) {
                        self.context.goal_position = Some(trigger.get_center_point());
                    }
                }
            }
        }
        self.context.goal_path = params.coords.clone();
        self.context.command_button = params.command_button;
        self.context.int_value = params.int_value;
        // Convert ai::DamageInfo to damage::DamageInfo (ai params currently carry no damage fields)
        self.context.damage_info = crate::damage::DamageInfo::new();

        // Set appropriate state based on command
        let state_type = match params.cmd {
            AiCommandType::Idle => AIStateType::Idle,
            AiCommandType::MoveToPosition => {
                self.context.goal_position = Some(params.pos);
                AIStateType::MoveTo
            }
            AiCommandType::MoveToObject => {
                self.context.goal_object = params.obj;
                AIStateType::MoveTo
            }
            AiCommandType::TightenToPosition => {
                self.context.goal_position = Some(params.pos);
                AIStateType::MoveAndTighten
            }
            AiCommandType::MoveToPositionAndEvacuate => {
                self.context.goal_position = Some(params.pos);
                AIStateType::MoveAndEvacuate
            }
            AiCommandType::MoveToPositionAndEvacuateAndExit => {
                self.context.goal_position = Some(params.pos);
                AIStateType::MoveAndEvacuateAndExit
            }
            AiCommandType::AttackObject => AIStateType::AttackObject,
            AiCommandType::ForceAttackObject => AIStateType::ForceAttackObject,
            AiCommandType::AttackTeam => AIStateType::AttackSquad,
            AiCommandType::AttackPosition => {
                self.context.goal_position = Some(params.pos);
                AIStateType::AttackPosition
            }
            AiCommandType::AttackMoveToPosition => {
                self.context.goal_position = Some(params.pos);
                AIStateType::AttackMoveTo
            }
            AiCommandType::AttackArea => {
                self.context.goal_position = Some(params.pos);
                AIStateType::AttackArea
            }
            AiCommandType::FollowPath => AIStateType::FollowPath,
            AiCommandType::FollowExitProductionPath => AIStateType::FollowExitProductionPath,
            AiCommandType::FollowUserPath => AIStateType::FollowPath,
            AiCommandType::FollowPathAppend => AIStateType::FollowPath,
            AiCommandType::GuardPosition => {
                self.context.goal_position = Some(params.pos);
                AIStateType::Guard
            }
            AiCommandType::GuardObject => {
                self.context.goal_object = params.obj;
                AIStateType::Guard
            }
            AiCommandType::GuardArea => AIStateType::Guard,
            AiCommandType::Hunt => AIStateType::Hunt,
            AiCommandType::Repair => AIStateType::GetRepaired,
            AiCommandType::GetHealed => AIStateType::GetRepaired,
            AiCommandType::Enter => AIStateType::Enter,
            AiCommandType::Dock => AIStateType::Dock,
            AiCommandType::Exit => AIStateType::Exit,
            AiCommandType::Evacuate => AIStateType::Exit,
            AiCommandType::FollowWaypointPath => {
                if let Some(waypoint) = params.waypoint {
                    self.context.goal_waypoint = Some(waypoint);
                }
                AIStateType::FollowWaypointPathAsIndividuals
            }
            AiCommandType::FollowWaypointPathAsTeam => {
                if let Some(waypoint) = params.waypoint {
                    self.context.goal_waypoint = Some(waypoint);
                }
                AIStateType::FollowWaypointPathAsTeam
            }
            AiCommandType::FollowWaypointPathExact => {
                if let Some(waypoint) = params.waypoint {
                    self.context.goal_waypoint = Some(waypoint);
                }
                AIStateType::FollowWaypointPathAsIndividualsExact
            }
            AiCommandType::FollowWaypointPathAsTeamExact => {
                if let Some(waypoint) = params.waypoint {
                    self.context.goal_waypoint = Some(waypoint);
                }
                AIStateType::FollowWaypointPathAsTeamExact
            }
            AiCommandType::AttackFollowWaypointPath => {
                if let Some(waypoint) = params.waypoint {
                    self.context.goal_waypoint = Some(waypoint);
                }
                AIStateType::AttackFollowWaypointPathAsIndividuals
            }
            AiCommandType::AttackFollowWaypointPathAsTeam => {
                if let Some(waypoint) = params.waypoint {
                    self.context.goal_waypoint = Some(waypoint);
                }
                AIStateType::AttackFollowWaypointPathAsTeam
            }
            AiCommandType::FaceObject => {
                self.context.goal_object = params.obj;
                AIStateType::FaceObject
            }
            AiCommandType::FacePosition => {
                self.context.goal_position = Some(params.pos);
                AIStateType::FacePosition
            }
            AiCommandType::RappelInto => AIStateType::RappelInto,
            AiCommandType::CombatDrop => AIStateType::CombatDrop,
            AiCommandType::Wander => AIStateType::Wander,
            AiCommandType::WanderInPlace => AIStateType::WanderInPlace,
            AiCommandType::Panic => AIStateType::Panic,
            AiCommandType::Busy => AIStateType::Busy,
            AiCommandType::MoveAwayFromUnit => AIStateType::MoveOutOfTheWay,
            AiCommandType::HackInternet => AIStateType::HackInternet,
            AiCommandType::NoCommand => AIStateType::Idle,
            AiCommandType::MoveToPositionEvenIfSleeping => {
                self.context.goal_position = Some(params.pos);
                AIStateType::MoveTo
            }
            AiCommandType::PickUpPrisoner => AIStateType::PickUpCrate,
            AiCommandType::ReturnPrisoners => AIStateType::Busy,
            AiCommandType::ResumeConstruction => AIStateType::Busy,
            AiCommandType::GetRepaired => AIStateType::GetRepaired,
            AiCommandType::ExecuteRailedTransport => AIStateType::Busy,
            AiCommandType::GoProne => AIStateType::Busy,
            AiCommandType::DeployAssaultReturn => AIStateType::Busy,
            AiCommandType::CommandButton => AIStateType::Busy,
            AiCommandType::CommandButtonObj => AIStateType::Busy,
            AiCommandType::CommandButtonPos => AIStateType::Busy,
            AiCommandType::GuardTunnelNetwork => AIStateType::GuardTunnelNetwork,
            AiCommandType::EvacuateInstantly => AIStateType::ExitInstantly,
            AiCommandType::ExitInstantly => AIStateType::ExitInstantly,
            AiCommandType::GuardRetaliate => AIStateType::GuardRetaliate,
        };

        self.set_state(state_type);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ai_state_machine_creation() {
        let machine = AIStateMachine::new(123, "TestMachine".to_string());
        assert_eq!(machine.owner_id, 123);
        assert_eq!(machine.name, "TestMachine");
        assert!(machine.is_idle()); // Should start in idle state
    }

    #[test]
    fn test_state_transitions() {
        let mut machine = AIStateMachine::new(123, "TestMachine".to_string());

        // Test move to state
        machine.set_goal_position([100.0, 200.0, 0.0]);
        machine.set_state(AIStateType::MoveTo);
        assert_eq!(machine.get_current_state_type(), Some(AIStateType::MoveTo));

        // Test attack state
        machine.set_goal_object(456);
        machine.set_state(AIStateType::AttackObject);
        assert_eq!(
            machine.get_current_state_type(),
            Some(AIStateType::AttackObject)
        );
        assert!(machine.is_in_attack_state());
    }

    #[test]
    fn test_ai_command_interface() {
        let mut machine = AIStateMachine::new(123, "TestMachine".to_string());

        // Test move command
        let mut params =
            AiCommandParams::new(AiCommandType::MoveToPosition, CommandSourceType::FromAi);
        params.pos = [150.0, 250.0, 0.0];

        assert!(machine.ai_do_command(&params).is_ok());
        assert_eq!(machine.get_current_state_type(), Some(AIStateType::MoveTo));

        // Test attack command
        params.cmd = AiCommandType::AttackObject;
        params.obj = Some(789);

        assert!(machine.ai_do_command(&params).is_ok());
        assert_eq!(
            machine.get_current_state_type(),
            Some(AIStateType::AttackObject)
        );
    }

    #[test]
    fn test_temporary_states() {
        let mut machine = AIStateMachine::new(123, "TestMachine".to_string());

        // Set a temporary state
        let result = machine.set_temporary_state(AIStateType::MoveOutOfTheWay, 100);
        assert_eq!(result, StateReturnType::Continue);

        // Check that temporary state is set
        assert!(machine.temporary_state.is_some());
        assert_eq!(machine.temporary_state_frame_end, Some(100));
    }

    #[test]
    fn test_ai_idle_state() {
        let mut idle_state = AIIdleState::new(true);
        assert!(idle_state.is_idle());
        assert_eq!(idle_state.get_state_type(), AIStateType::Idle);

        let mut context = AIStateMachineContext::default();
        let result = idle_state.on_enter(&mut context);
        assert_eq!(result, StateReturnType::Continue);
    }

    #[test]
    fn test_ai_attack_state() {
        let mut attack_state = AIAttackState::new(false, true, false, false);
        assert!(attack_state.is_attack());
        assert_eq!(attack_state.get_state_type(), AIStateType::AttackObject);

        let mut context = AIStateMachineContext::default();
        context.goal_object = Some(456);

        let result = attack_state.on_enter(&mut context);
        assert_eq!(result, StateReturnType::Continue);
    }

    #[test]
    fn test_move_and_tighten_state() {
        let mut tighten_state = AIMoveAndTightenState::new();
        assert_eq!(tighten_state.get_state_type(), AIStateType::MoveAndTighten);

        let mut context = AIStateMachineContext::default();
        context.goal_position = Some([100.0, 200.0, 0.0]);

        let result = tighten_state.on_enter(&mut context);
        assert_eq!(result, StateReturnType::Continue);

        // Verify goal position was set
        assert_eq!(tighten_state.goal_position.x, 100.0);
        assert_eq!(tighten_state.goal_position.y, 200.0);
    }

    #[test]
    fn test_move_and_tighten_needs_tightening() {
        let tighten_state = AIMoveAndTightenState::new();

        // Tight formation - should not need tightening
        let tight_positions = vec![[0.0, 0.0, 0.0], [5.0, 0.0, 0.0], [0.0, 5.0, 0.0]];
        assert!(!tighten_state.needs_tightening(&tight_positions));

        // Spread formation - should need tightening
        let spread_positions = vec![[0.0, 0.0, 0.0], [100.0, 0.0, 0.0], [0.0, 100.0, 0.0]];
        assert!(tighten_state.needs_tightening(&spread_positions));
    }

    #[test]
    fn test_move_and_tighten_spread_calculation() {
        let tighten_state = AIMoveAndTightenState::new();

        let positions = vec![[0.0, 0.0, 0.0], [10.0, 0.0, 0.0], [0.0, 10.0, 0.0]];

        let spread = tighten_state.get_group_spread(&positions);

        // Spread should be greater than 0
        assert!(spread > 0.0);
        // Spread should be reasonable for these positions
        assert!(spread < 20.0);
    }

    #[test]
    fn test_state_machine_move_and_tighten() {
        let mut machine = AIStateMachine::new(123, "TestMachine".to_string());

        // Set goal position and switch to MoveAndTighten state
        machine.set_goal_position([100.0, 200.0, 0.0]);
        machine.set_state(AIStateType::MoveAndTighten);

        assert_eq!(
            machine.get_current_state_type(),
            Some(AIStateType::MoveAndTighten)
        );
    }

    #[test]
    fn test_temporary_move_and_tighten() {
        let mut machine = AIStateMachine::new(123, "TestMachine".to_string());

        // Set a temporary MoveAndTighten state
        machine.set_goal_position([50.0, 50.0, 0.0]);
        let result = machine.set_temporary_state(AIStateType::MoveAndTighten, 100);
        assert_eq!(result, StateReturnType::Continue);

        // Check that temporary state is set
        assert!(machine.temporary_state.is_some());
        assert_eq!(machine.temporary_state_frame_end, Some(100));
    }
}
