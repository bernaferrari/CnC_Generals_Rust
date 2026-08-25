#![allow(deprecated, unused_imports, dead_code)]

use super::attack_machine::*;
use super::dead::*;
use super::dock::*;
use super::enter::*;
use super::face::*;
use super::follow_path::*;
use super::follow_path_core::*;
use super::guard::*;
use super::hack::*;
use super::helpers::*;
use super::hunt::*;
use super::idle::*;
use super::r#move::*;
use super::rappel::*;
use super::state_machine::*;
use super::types::*;
use super::wait_busy::*;
use super::wander_panic::*;
use super::waypoint::*;
use super::*;

use crate::action_manager::{CanEnterType, TheActionManager};
use crate::ai::dock::AIDockMachine;
use crate::ai::group::AIGroup;
use crate::ai::guard::{AIGuardMachine, GuardStateType};
use crate::ai::guard_retaliate::AIGuardRetaliateMachine;
use crate::ai::object_registry::get_legacy_object;
use crate::ai::pathfind::Path;
use crate::ai::squad::Squad;
use crate::ai::tn_guard::{AITNGuardMachine, TNGuardStateType};
use crate::ai::{
    AiCommandInterface, AiCommandParams, GuardMode, MoodMatrixAction, PartitionFilter, THE_AI,
    mood_matrix_adjustment, mood_matrix_parameters, resolve_attack_priority_info_for_object,
    search_qualifiers,
};
use crate::attack::{AbleToAttackType, CanAttackResult};
use crate::command_button::CommandButton;
use crate::common::coord::*;
use crate::common::xfer::XferExt;
use crate::common::*;
use crate::compat::{ClassicState, legacy_transition, register_classic_state};
use crate::control_bar::get_control_bar_bridge;
use crate::damage::DamageInfo;
use crate::helpers::{TheAudio, TheGameLogic, ThePartitionManager, get_game_logic_random_value};
use crate::locomotor::LocomotorAppearance;
use crate::modules::{
    AIUpdateInterface, AIUpdateInterfaceExt, BodyModuleInterfaceExt, ContainModuleInterfaceExt,
    ContainWant, ExitDoorType, FAST_AS_POSSIBLE, PhysicsBehaviorExt,
};
use crate::object::production::AIFreeToExitType;
use crate::object::registry::OBJECT_REGISTRY;
use crate::object::*;
use crate::path::PATHFIND_CELL_SIZE_F;
use crate::physics::GRAVITY;
use crate::player::PlayerType;
use crate::polygon_trigger::PolygonTrigger;
use crate::scripting::engine::get_script_engine;
use crate::state_machine::*;
use crate::team::{Team, TeamID, TheTeamFactory};
use crate::terrain::get_terrain_logic;
use crate::waypoint::{Waypoint, WaypointId};
use crate::weapon::{
    NO_MAX_SHOTS_LIMIT, Weapon, WeaponChoiceCriteria, WeaponLockType, WeaponSlotType, WeaponStatus,
};
use game_engine::common::system::{GeometryType, Snapshotable, Xfer};

use crate::common::INVALID_ID;

use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock, Weak};

/// Attack move-to state (attack while moving)
#[derive(Debug)]
pub struct AIAttackMoveToState {
    pub(crate) base: AIMoveToState,
    pub(crate) attack_move_machine: Option<AIAttackMoveStateMachine>,
    pub(crate) frame_to_sleep_until: UnsignedInt,
    pub(crate) retry_count: i32,
    pub(crate) command_src: CommandSourceType,
}

impl AIAttackMoveToState {
    pub fn new(machine: &StateMachine) -> Self {
        let mut base = AIMoveToState::new(machine);
        base.base.name = "AIAttackMoveTo".to_string();
        base.is_move_to = false;
        Self {
            base,
            attack_move_machine: None,
            frame_to_sleep_until: 0,
            retry_count: ATTACK_RETRY_COUNT,
            command_src: CommandSourceType::FromAi,
        }
    }
}

impl StateImplementation for AIAttackMoveToState {
    fn on_enter(&mut self) -> StateReturnType {
        self.classic_on_enter().unwrap_or(StateReturnType::Failure)
    }

    fn update(&mut self) -> StateReturnType {
        self.classic_on_update().unwrap_or(StateReturnType::Failure)
    }

    fn on_exit(&mut self, _status: StateExitType) {
        let _ = self.classic_on_exit(_status);
    }

    fn xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::xfer(self, xfer)
    }
}

impl ClassicState for AIAttackMoveToState {
    fn base_state(&self) -> &State {
        &self.base.base
    }

    fn base_state_mut(&mut self) -> &mut State {
        &mut self.base.base
    }

    fn classic_xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::xfer(self, xfer)
    }

    fn classic_on_enter(&mut self) -> Result<StateReturnType, String> {
        let result = self.base.classic_on_enter()?;
        let owner = self
            .base
            .base
            .get_machine_owner()
            .ok_or_else(|| "attack move-to missing machine owner".to_string())?;
        let mut attack_machine =
            AIAttackMoveStateMachine::new(Arc::downgrade(&owner), "AIAttackMoveMachine");
        attack_machine.clear();
        let _ = attack_machine.set_state(AIStateType::Idle);
        self.attack_move_machine = Some(attack_machine);

        if let Ok(owner_guard) = owner.read() {
            if let Some(ai) = owner_guard.get_ai_update_interface() {
                if let Ok(ai_guard) = ai.lock() {
                    self.command_src = ai_guard.get_last_command_source();
                }
            }
        }
        self.retry_count = ATTACK_RETRY_COUNT;
        self.frame_to_sleep_until = 0;

        Ok(result)
    }

    fn classic_on_update(&mut self) -> Result<StateReturnType, String> {
        let owner = self
            .base
            .base
            .get_machine_owner()
            .ok_or_else(|| "attack move-to missing machine owner".to_string())?;
        let ai = owner
            .read()
            .ok()
            .and_then(|guard| guard.get_ai_update_interface())
            .ok_or_else(|| "attack move-to missing AIUpdateInterface".to_string())?;
        let mut ai_guard = ai
            .lock()
            .map_err(|_| "attack move-to AI lock poisoned".to_string())?;

        let mut force_retarget_this_frame = false;
        let mut should_repath_this_frame = false;

        if let Some(machine) = self.attack_move_machine.as_mut() {
            if !machine.is_in_idle_state() {
                ai_guard.set_locomotor_goal_none();
                if let Ok(mut owner_guard) = owner.write() {
                    owner_guard.clear_model_condition_state(ModelConditionFlags::MOVING);
                }
                let _ = machine.update();

                if machine.is_in_idle_state() {
                    force_retarget_this_frame = true;
                    should_repath_this_frame = true;
                    ai_guard.set_last_command_source(self.command_src);
                } else {
                    return Ok(StateReturnType::Continue);
                }
            }
        }

        if let Some(machine) = self.attack_move_machine.as_mut() {
            if machine.is_in_idle_state() {
                if let Some(crate_obj) = ai_guard.check_for_crate_to_pickup() {
                    machine.set_goal_object_by_id(crate_obj.read().ok().map(|g| g.get_id()));
                    let _ = machine.set_state(AIStateType::PickUpCrate);
                    return Ok(StateReturnType::Continue);
                }

                if let Some(target) =
                    ai_guard.get_next_mood_target(!force_retarget_this_frame, false)
                {
                    ai_guard.friend_ending_move();
                    machine.set_goal_object_by_id(target.read().ok().map(|g| g.get_id()));
                    let _ = machine.set_state(AIStateType::AttackObject);
                    ai_guard.set_last_command_source(CommandSourceType::FromAi);
                    return Ok(StateReturnType::Continue);
                }
            }
        }

        let current_frame = TheGameLogic::get_frame();
        if self.frame_to_sleep_until > current_frame {
            return Ok(StateReturnType::Continue);
        } else if self.frame_to_sleep_until == current_frame {
            should_repath_this_frame = true;
        }

        if should_repath_this_frame {
            let _ = self.base.classic_on_enter();
            self.base.force_repath();
        }

        let mut ret = self.base.classic_on_update()?;
        if ret != StateReturnType::Continue {
            if self.retry_count < 1 {
                return Ok(ret);
            }
            if let Ok(owner_guard) = owner.read() {
                let dx = owner_guard.get_position().x - self.base.path_goal_position.x;
                let dy = owner_guard.get_position().y - self.base.path_goal_position.y;
                let dist_sqr = dx * dx + dy * dy;
                let close_enough =
                    (ATTACK_CLOSE_ENOUGH_CELLS as f32 * PATHFIND_CELL_SIZE_F).powi(2);
                if dist_sqr < close_enough {
                    return Ok(ret);
                }
            }

            ret = StateReturnType::Continue;
            self.retry_count -= 1;
            self.frame_to_sleep_until = current_frame + 3 * LOGICFRAMES_PER_SECOND;
        }

        Ok(ret)
    }

    fn classic_on_exit(&mut self, exit: StateExitType) -> Result<(), String> {
        if let Some(mut machine) = self.attack_move_machine.take() {
            let _ = machine.halt();
        }
        self.base.classic_on_exit(exit)
    }

    fn classic_is_busy(&self) -> bool {
        true
    }

    fn classic_is_attack(&self) -> bool {
        self.attack_move_machine
            .as_ref()
            .map(|machine| machine.is_in_attack_state())
            .unwrap_or(false)
    }
}

pub(crate) const ATTACK_RETRY_COUNT: i32 = 3;

pub(crate) const ATTACK_CLOSE_ENOUGH_CELLS: f32 = 4.0;

/// Attack follow waypoint path as team
#[derive(Debug)]
pub struct AIAttackFollowWaypointPathAsTeamState {
    pub(crate) base: AIFollowWaypointPathAsTeamState,
    pub(crate) attack_follow_machine: Option<AIAttackMoveStateMachine>,
}

impl AIAttackFollowWaypointPathAsTeamState {
    pub fn new(machine: &StateMachine) -> Self {
        let mut base = AIFollowWaypointPathAsTeamState::new(machine);
        base.base.name = "AIAttackFollowWaypointPathAsTeam".to_string();
        base.core.is_follow_waypoint_path_state = false;
        Self {
            base,
            attack_follow_machine: None,
        }
    }
}

impl StateImplementation for AIAttackFollowWaypointPathAsTeamState {
    fn on_enter(&mut self) -> StateReturnType {
        self.classic_on_enter().unwrap_or(StateReturnType::Failure)
    }

    fn update(&mut self) -> StateReturnType {
        self.classic_on_update().unwrap_or(StateReturnType::Failure)
    }

    fn on_exit(&mut self, _status: StateExitType) {
        let _ = self.classic_on_exit(_status);
    }

    fn xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::xfer(self, xfer)
    }
}

impl ClassicState for AIAttackFollowWaypointPathAsTeamState {
    fn base_state(&self) -> &State {
        &self.base.base
    }

    fn base_state_mut(&mut self) -> &mut State {
        &mut self.base.base
    }

    fn classic_xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::xfer(self, xfer)
    }

    fn classic_on_enter(&mut self) -> Result<StateReturnType, String> {
        let result = self.base.classic_on_enter()?;
        let owner = self
            .base
            .base
            .get_machine_owner()
            .ok_or_else(|| "attack follow path missing machine owner".to_string())?;
        let mut attack_machine =
            AIAttackMoveStateMachine::new(Arc::downgrade(&owner), "AIAttackFollowMachine");
        attack_machine.clear();
        let _ = attack_machine.set_state(AIStateType::Idle);
        self.attack_follow_machine = Some(attack_machine);

        Ok(result)
    }

    fn classic_on_update(&mut self) -> Result<StateReturnType, String> {
        let owner = self
            .base
            .base
            .get_machine_owner()
            .ok_or_else(|| "attack follow path missing machine owner".to_string())?;
        let ai = owner
            .read()
            .ok()
            .and_then(|guard| guard.get_ai_update_interface())
            .ok_or_else(|| "attack follow path missing AIUpdateInterface".to_string())?;
        let mut ai_guard = ai
            .lock()
            .map_err(|_| "attack follow path AI lock poisoned".to_string())?;

        let mut force_retarget_this_frame = false;
        let mut should_repath_this_frame = false;

        if let Some(machine) = self.attack_follow_machine.as_mut() {
            if !machine.is_in_idle_state() {
                ai_guard.set_locomotor_goal_none();
                if let Ok(mut owner_guard) = owner.write() {
                    owner_guard.clear_model_condition_state(ModelConditionFlags::MOVING);
                }
                let _ = machine.update();
                if machine.is_in_idle_state() {
                    force_retarget_this_frame = true;
                    should_repath_this_frame = true;
                } else {
                    return Ok(StateReturnType::Continue);
                }
            }
        }

        if let Some(machine) = self.attack_follow_machine.as_mut() {
            if machine.is_in_idle_state() {
                if let Some(crate_obj) = ai_guard.check_for_crate_to_pickup() {
                    machine.set_goal_object_by_id(crate_obj.read().ok().map(|g| g.get_id()));
                    let _ = machine.set_state(AIStateType::PickUpCrate);
                    return Ok(StateReturnType::Continue);
                }

                if let Some(target) =
                    ai_guard.get_next_mood_target(!force_retarget_this_frame, false)
                {
                    machine.set_goal_object_by_id(target.read().ok().map(|g| g.get_id()));
                    let _ = machine.set_state(AIStateType::AttackObject);
                    return Ok(StateReturnType::Continue);
                }
            }
        }

        if should_repath_this_frame {
            if let Ok(owner_guard) = owner.read() {
                self.base.core.compute_goal(
                    &self.base.base,
                    &owner_guard,
                    &mut *ai_guard,
                    self.base.core.move_as_group,
                )?;
                self.base.core.compute_path(&mut *ai_guard)?;
            }
        }

        self.base.classic_on_update()
    }

    fn classic_on_exit(&mut self, exit: StateExitType) -> Result<(), String> {
        if let Some(mut machine) = self.attack_follow_machine.take() {
            let _ = machine.set_state(AIStateType::Idle);
            let _ = machine.halt();
        }
        self.base.classic_on_exit(exit)
    }
}

/// Attack follow waypoint path as individuals
#[derive(Debug)]
pub struct AIAttackFollowWaypointPathAsIndividualsState {
    pub(crate) base: AIFollowWaypointPathAsIndividualsState,
    pub(crate) attack_follow_machine: Option<AIAttackMoveStateMachine>,
}

impl AIAttackFollowWaypointPathAsIndividualsState {
    pub fn new(machine: &StateMachine) -> Self {
        let mut base = AIFollowWaypointPathAsIndividualsState::new(machine);
        base.base.name = "AIAttackFollowWaypointPathAsIndividuals".to_string();
        base.core.is_follow_waypoint_path_state = false;
        Self {
            base,
            attack_follow_machine: None,
        }
    }
}

impl StateImplementation for AIAttackFollowWaypointPathAsIndividualsState {
    fn on_enter(&mut self) -> StateReturnType {
        self.classic_on_enter().unwrap_or(StateReturnType::Failure)
    }

    fn update(&mut self) -> StateReturnType {
        self.classic_on_update().unwrap_or(StateReturnType::Failure)
    }

    fn on_exit(&mut self, _status: StateExitType) {
        let _ = self.classic_on_exit(_status);
    }

    fn xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::xfer(self, xfer)
    }
}

impl ClassicState for AIAttackFollowWaypointPathAsIndividualsState {
    fn base_state(&self) -> &State {
        &self.base.base
    }

    fn base_state_mut(&mut self) -> &mut State {
        &mut self.base.base
    }

    fn classic_xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::xfer(self, xfer)
    }

    fn classic_on_enter(&mut self) -> Result<StateReturnType, String> {
        let result = self.base.classic_on_enter()?;

        let owner = self
            .base
            .base
            .get_machine_owner()
            .ok_or_else(|| "attack follow path missing machine owner".to_string())?;
        let mut attack_machine =
            AIAttackMoveStateMachine::new(Arc::downgrade(&owner), "AIAttackFollowMachine");
        attack_machine.clear();
        let _ = attack_machine.set_state(AIStateType::Idle);
        self.attack_follow_machine = Some(attack_machine);

        Ok(result)
    }

    fn classic_on_update(&mut self) -> Result<StateReturnType, String> {
        let owner = self
            .base
            .base
            .get_machine_owner()
            .ok_or_else(|| "attack follow path missing machine owner".to_string())?;
        let ai = owner
            .read()
            .ok()
            .and_then(|guard| guard.get_ai_update_interface())
            .ok_or_else(|| "attack follow path missing AIUpdateInterface".to_string())?;
        let mut ai_guard = ai
            .lock()
            .map_err(|_| "attack follow path AI lock poisoned".to_string())?;

        let mut force_retarget_this_frame = false;
        let mut should_repath_this_frame = false;

        if let Some(machine) = self.attack_follow_machine.as_mut() {
            if !machine.is_in_idle_state() {
                ai_guard.set_locomotor_goal_none();
                if let Ok(mut owner_guard) = owner.write() {
                    owner_guard.clear_model_condition_state(ModelConditionFlags::MOVING);
                }
                let _ = machine.update();
                if machine.is_in_idle_state() {
                    force_retarget_this_frame = true;
                    should_repath_this_frame = true;
                } else {
                    return Ok(StateReturnType::Continue);
                }
            }
        }

        if let Some(machine) = self.attack_follow_machine.as_mut() {
            if machine.is_in_idle_state() {
                if let Some(crate_obj) = ai_guard.check_for_crate_to_pickup() {
                    machine.set_goal_object_by_id(crate_obj.read().ok().map(|g| g.get_id()));
                    let _ = machine.set_state(AIStateType::PickUpCrate);
                    return Ok(StateReturnType::Continue);
                }

                if let Some(target) =
                    ai_guard.get_next_mood_target(!force_retarget_this_frame, false)
                {
                    machine.set_goal_object_by_id(target.read().ok().map(|g| g.get_id()));
                    let _ = machine.set_state(AIStateType::AttackObject);
                    return Ok(StateReturnType::Continue);
                }
            }
        }

        if should_repath_this_frame {
            if let Ok(owner_guard) = owner.read() {
                self.base.core.compute_goal(
                    &self.base.base,
                    &owner_guard,
                    &mut *ai_guard,
                    false,
                )?;
                self.base.core.compute_path(&mut *ai_guard)?;
            }
        }

        self.base.classic_on_update()
    }

    fn classic_on_exit(&mut self, exit: StateExitType) -> Result<(), String> {
        if let Some(mut machine) = self.attack_follow_machine.take() {
            let _ = machine.set_state(AIStateType::Idle);
            let _ = machine.halt();
        }
        self.base.classic_on_exit(exit)
    }
}

/// Attack object state
#[derive(Debug)]
pub struct AIAttackObjectState {
    pub(crate) base: State,
    pub(crate) target_id: ObjectID,
    pub(crate) force_attack: Bool,
    pub(crate) follow_target: Bool,
    pub(crate) issued_attack: Bool,
    pub(crate) attack_machine: Option<AttackStateMachine>,
    pub(crate) original_victim_pos: Coord3D,
    /// Team ID of the victim when attack started (C++ m_victimTeam)
    pub(crate) victim_team: Option<TeamID>,
    /// Weapon slot that was locked when entering attack state (C++ m_lockedWeaponOnEnter)
    pub(crate) locked_weapon_on_enter: Option<WeaponSlotType>,
}

impl AIAttackObjectState {
    pub fn new(machine: &StateMachine, force_attack: Bool, follow_target: Bool) -> Self {
        Self {
            base: State::new(machine, "AIAttackObject"),
            target_id: INVALID_ID,
            force_attack,
            follow_target,
            issued_attack: false,
            attack_machine: None,
            original_victim_pos: Coord3D::new(0.0, 0.0, 0.0),
            victim_team: None,
            locked_weapon_on_enter: None,
        }
    }

    pub fn is_attack(&self) -> bool {
        true
    }
}

/// C++ Team::setTeamTargetObject(NULL) when this victim was the shared team target.
pub(crate) fn team_target_matches_victim(team_target: ObjectID, victim_id: ObjectID) -> bool {
    team_target != INVALID_ID && team_target == victim_id
}

pub(crate) fn clear_team_target_object_if_victim(team: &mut Team, victim_id: ObjectID) {
    if team_target_matches_victim(team.get_team_target_object(), victim_id) {
        team.set_team_target_object(INVALID_ID);
    }
}

fn clear_team_target_if_victim(owner: &Object, victim_id: ObjectID) {
    if let Some(team_arc) = owner.get_team() {
        if let Ok(mut team_guard) = team_arc.write() {
            clear_team_target_object_if_victim(&mut team_guard, victim_id);
        }
    }
}

/// C++ `AIAttackState::update` (`AIStates.cpp:5629-5633`):
/// parent-machine retargets are forwarded into the nested AttackStateMachine.
pub(crate) fn forward_parent_goal_to_nested_machine(
    machine: &mut AttackStateMachine,
    parent_goal: ObjectID,
) {
    if parent_goal != INVALID_ID && machine.get_goal_object_id() != parent_goal {
        machine.set_goal_object(Some(parent_goal));
    }
}

impl StateImplementation for AIAttackObjectState {
    fn on_enter(&mut self) -> StateReturnType {
        self.classic_on_enter().unwrap_or(StateReturnType::Failure)
    }

    fn update(&mut self) -> StateReturnType {
        self.classic_on_update().unwrap_or(StateReturnType::Failure)
    }

    fn on_exit(&mut self, _status: StateExitType) {
        let _ = self.classic_on_exit(_status);
    }

    fn xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::xfer(self, xfer)
    }
}

impl ClassicState for AIAttackObjectState {
    fn base_state(&self) -> &State {
        &self.base
    }

    fn base_state_mut(&mut self) -> &mut State {
        &mut self.base
    }

    fn classic_xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::xfer(self, xfer)
    }

    fn classic_on_enter(&mut self) -> Result<StateReturnType, String> {
        // Wave 257: empty dual-world → fail-closed state.
        if dual_world_registry_unavailable() {
            return Ok(StateReturnType::Failure);
        }

        let owner = self
            .base
            .get_machine_owner()
            .ok_or_else(|| "attack object state missing machine owner".to_string())?;

        // C++ lines 5474-5478: Mood matrix sleep mode check
        {
            let owner_guard = owner.read().map_err(|_| "lock poisoned".to_string())?;
            if let Some(ai) = owner_guard.get_ai_update_interface() {
                if let Ok(mut ai_guard) = ai.lock() {
                    let adjustment =
                        ai_guard.get_mood_matrix_action_adjustment(MoodMatrixAction::Attack);
                    if (adjustment & mood_matrix_adjustment::ACTION_OK) == 0 {
                        return Ok(StateReturnType::Success);
                    }
                }
            }

            // C++ lines 5487-5490: Under construction check
            if owner_guard.test_status(ObjectStatusTypes::UnderConstruction) {
                return Ok(StateReturnType::Failure);
            }

            // C++ lines 5493-5495: Out of ammo check
            if owner_guard.is_out_of_ammo() && !owner_guard.is_kind_of(KindOf::Projectile) {
                return Ok(StateReturnType::Failure);
            }
        }

        // C++ lines 5505-5516: Get victim, check dead
        let target_id = self
            .base
            .get_machine_goal_object_id()
            .ok_or_else(|| "attack object state missing goal object".to_string())?;
        let target = crate::helpers::TheGameLogic::find_object_by_id(target_id)
            .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(target_id))
            .ok_or_else(|| "attack object state missing goal object".to_string())?;
        self.target_id = target.read().map(|g| g.get_id()).unwrap_or(INVALID_ID);

        {
            let target_guard = target.read().map_err(|_| "lock poisoned".to_string())?;
            self.original_victim_pos = *target_guard.get_position();

            // C++ lines 5508-5512: Check if victim is dead
            if target_guard.is_effectively_dead() {
                if let Ok(owner_guard) = owner.read() {
                    if let Some(ai) = owner_guard.get_ai_update_interface() {
                        if let Ok(mut ai_guard) = ai.lock() {
                            ai_guard.notify_victim_is_dead();
                        }
                    }
                }
                return Ok(StateReturnType::Failure);
            }

            // C++ line 5513: m_victimTeam = victim->getTeam()
            self.victim_team = target_guard.get_team_id();
        }

        // Set original victim pos on AI
        if let Ok(owner_guard) = owner.read() {
            if let Some(ai) = owner_guard.get_ai_update_interface() {
                if let Ok(mut ai_guard) = ai.lock() {
                    ai_guard.set_original_victim_pos(Some(self.original_victim_pos));
                }
            }
        }

        // C++ lines 5525-5527: Choose weapon
        let cmd_source = {
            let Ok(owner_guard) = owner.read() else {
                return Ok(StateReturnType::Failure);
            };
            if let Some(ai) = owner_guard.get_ai_update_interface() {
                if let Ok(ai_guard) = ai.lock() {
                    ai_guard.get_last_command_source()
                } else {
                    CommandSourceType::FromAi
                }
            } else {
                CommandSourceType::FromAi
            }
        };

        {
            let target_guard = target.read().map_err(|_| "lock poisoned".to_string())?;
            let mut owner_guard = owner.write().map_err(|_| "lock poisoned".to_string())?;
            let weapon_found = owner_guard.choose_best_weapon_for_target(
                &*target_guard,
                WeaponChoiceCriteria::PreferMostDamage,
                cmd_source,
            );
            if !weapon_found {
                return Ok(StateReturnType::Failure);
            }

            // C++ lines 5529-5536: Set max shots and stealth check
            if let Some((weapon, _slot)) = owner_guard.get_current_weapon() {
                let continue_attack_range = weapon.get_continue_attack_range();
                owner_guard.set_current_weapon_max_shot_count(NO_MAX_SHOTS_LIMIT);
                if continue_attack_range > 0.0 {
                    owner_guard.set_status(
                        ObjectStatusMaskType::from_status(ObjectStatusTypes::IgnoringStealth),
                        true,
                    );
                }
            }

            // C++ line 5538: m_lockedWeaponOnEnter = source->isCurWeaponLocked() ? curWeapon : NULL
            if owner_guard.is_cur_weapon_locked() {
                if let Some((_weapon, slot)) = owner_guard.get_current_weapon() {
                    self.locked_weapon_on_enter = Some(slot);
                }
            } else {
                self.locked_weapon_on_enter = None;
            }
        }

        // Create attack machine (C++ lines 5499)
        let mut attack_machine = AttackStateMachine::new(
            Arc::downgrade(&owner),
            "AIAttackMachine",
            self.follow_target,
            true,
            self.force_attack,
        );
        attack_machine.set_goal_object(target.read().ok().map(|g| g.get_id()));
        attack_machine.set_goal_position(self.original_victim_pos);

        // C++ lines 5540-5545: Init default state and set attacking status
        let ret = attack_machine.init_default_state();
        if ret == StateReturnType::Continue {
            if let Ok(mut owner_guard) = owner.write() {
                owner_guard.set_status(
                    ObjectStatusMaskType::from_status(ObjectStatusTypes::IsAttacking),
                    true,
                );
                owner_guard.set_model_condition_state(ModelConditionFlags::ATTACKING);
            }
        }
        self.attack_machine = Some(attack_machine);
        self.issued_attack = true;

        Ok(ret)
    }

    fn classic_on_update(&mut self) -> Result<StateReturnType, String> {
        // Wave 257: empty dual-world → fail-closed state.
        if dual_world_registry_unavailable() {
            return Ok(StateReturnType::Failure);
        }

        let owner = self
            .base
            .get_machine_owner()
            .ok_or_else(|| "attack object state missing owner".to_string())?;

        // C++ lines 5565-5570: Out of ammo check every frame
        {
            let owner_guard = owner.read().map_err(|_| "lock poisoned".to_string())?;
            if owner_guard.is_out_of_ammo() && !owner_guard.is_kind_of(KindOf::Projectile) {
                return Ok(StateReturnType::Failure);
            }
        }

        // C++ AIAttackState::update uses getMachineGoalObject() each frame so parent
        // retargets (transferAttack / hunt re-scan) reach the nested machine.
        if let Some(parent_goal) = self.base.get_machine_goal_object_id() {
            self.target_id = parent_goal;
        }
        if self.target_id == INVALID_ID {
            return Ok(StateReturnType::Failure);
        }
        let Some(target) = crate::helpers::TheGameLogic::find_object_by_id(self.target_id)
            .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(self.target_id))
        else {
            return Ok(StateReturnType::Failure);
        };

        // C++ lines 5576-5579: Check if victim is dead
        {
            let target_guard = target.read().map_err(|_| "lock poisoned".to_string())?;
            if target_guard.is_effectively_dead() {
                if let Ok(owner_guard) = owner.read() {
                    if let Some(ai) = owner_guard.get_ai_update_interface() {
                        if let Ok(mut ai_guard) = ai.lock() {
                            ai_guard.notify_victim_is_dead();
                        }
                    }
                }
                return Ok(StateReturnType::Success);
            }

            // C++ line 5584: setCurrentVictim every frame
            let victim_id = target_guard.get_id();
            if let Ok(owner_guard) = owner.read() {
                if let Some(ai) = owner_guard.get_ai_update_interface() {
                    if let Ok(mut ai_guard) = ai.lock() {
                        ai_guard.set_current_victim(Some(victim_id));
                    }
                }
            }

            // C++ lines 5587-5627: Team change detection
            let target_team = target_guard.get_team_id();
            if self.victim_team != target_team {
                if let Ok(owner_guard) = owner.read() {
                    let relationship = owner_guard.relationship_to(&*target_guard);
                    let empty_garrison = !target_guard.test_status(ObjectStatusTypes::CanAttack)
                        && target_guard.get_contain().is_some_and(|contain| {
                            contain.lock().ok().is_some_and(|contain_guard| {
                                contain_guard.is_garrisonable()
                                    && contain_guard.get_contained_count() == 0
                            })
                        })
                        && relationship == Relationship::Neutral;
                    let should_stop = empty_garrison || relationship != Relationship::Enemies;

                    if should_stop {
                        clear_team_target_if_victim(&*owner_guard, victim_id);
                        if let Some(ai) = owner_guard.get_ai_update_interface() {
                            if let Ok(mut ai_guard) = ai.lock() {
                                ai_guard.set_goal_object(None);
                                ai_guard.notify_victim_is_dead();
                            }
                        }
                        return Ok(StateReturnType::Failure);
                    }
                }
                self.victim_team = target_team;
            }
        }

        // C++ lines 5629-5633: parent goal change is forwarded into AttackStateMachine.
        if let Some(attack_machine) = self.attack_machine.as_mut() {
            forward_parent_goal_to_nested_machine(attack_machine, self.target_id);
        }

        // C++ lines 5640-5642: Re-evaluate weapon choice every frame
        {
            let cmd_source = {
                let Ok(owner_guard) = owner.read() else {
                    return Ok(StateReturnType::Failure);
                };
                if let Some(ai) = owner_guard.get_ai_update_interface() {
                    if let Ok(ai_guard) = ai.lock() {
                        ai_guard.get_last_command_source()
                    } else {
                        CommandSourceType::FromAi
                    }
                } else {
                    CommandSourceType::FromAi
                }
            };

            let target_guard = target.read().map_err(|_| "lock poisoned".to_string())?;
            let mut owner_guard = owner.write().map_err(|_| "lock poisoned".to_string())?;
            let weapon_found = owner_guard.choose_best_weapon_for_target(
                &*target_guard,
                WeaponChoiceCriteria::PreferMostDamage,
                cmd_source,
            );
            if !weapon_found {
                return Ok(StateReturnType::Failure);
            }

            // C++ lines 5649-5650: Locked weapon check
            if let Some(locked_slot) = self.locked_weapon_on_enter {
                if let Some((_weapon, cur_slot)) = owner_guard.get_current_weapon() {
                    if cur_slot != locked_slot {
                        return Ok(StateReturnType::Failure);
                    }
                }
            }

            // C++ lines 5653-5654: Shot count check
            if let Some((weapon, _slot)) = owner_guard.get_current_weapon() {
                if weapon.get_max_shot_count() <= 0 {
                    return Ok(StateReturnType::Failure);
                }
            } else {
                return Ok(StateReturnType::Failure);
            }
        }

        // C++ line 5664: Run attack machine (CONVERT_SLEEP_TO_CONTINUE)
        if let Some(attack_machine) = self.attack_machine.as_mut() {
            let result = attack_machine.update();
            return Ok(match result {
                StateReturnType::Sleep(_) => StateReturnType::Continue,
                other => other,
            });
        }

        Ok(StateReturnType::Continue)
    }

    fn classic_on_exit(&mut self, _exit: StateExitType) -> Result<(), String> {
        // Stop attacking — destroy attack machine (C++ AIAttackState::onExit)
        self.target_id = INVALID_ID;
        self.issued_attack = false;
        if let Some(mut machine) = self.attack_machine.take() {
            let _ = machine.halt();
        }

        if let Some(owner) = self.base.get_machine_owner() {
            // Clear attack-related status flags
            if let Ok(mut owner_guard) = owner.write() {
                owner_guard.clear_status(
                    ObjectStatusMaskType::IS_FIRING_WEAPON
                        | ObjectStatusMaskType::IS_AIMING_WEAPON
                        | ObjectStatusMaskType::IS_ATTACKING
                        | ObjectStatusMaskType::IGNORING_STEALTH,
                );
                owner_guard.clear_model_condition_state(ModelConditionFlags::ATTACKING);
                owner_guard.clear_leech_range_mode_for_all_weapons();

                // Clear AI state: current victim, turret targets, goal object
                if let Some(ai) = owner_guard.get_ai_update_interface() {
                    if let Ok(mut ai_guard) = ai.lock() {
                        ai_guard.set_original_victim_pos(None);
                        ai_guard.set_current_victim(None);
                        for turret in [TurretType::Primary, TurretType::Secondary] {
                            ai_guard.set_turret_target_object(turret, None, false);
                        }
                        ai_guard.set_goal_object(None);
                    }
                }
            }
        }
        Ok(())
    }

    fn classic_is_attack(&self) -> bool {
        true
    }

    fn classic_is_busy(&self) -> bool {
        self.target_id != INVALID_ID
    }
}

/// Attack position state
#[derive(Debug)]
pub struct AIAttackPositionState {
    pub(crate) base: State,
    pub(crate) target_position: Coord3D,
    pub(crate) issued_attack: Bool,
    pub(crate) attack_machine: Option<AttackStateMachine>,
    /// Weapon slot that was locked when entering attack state (C++ m_lockedWeaponOnEnter)
    pub(crate) locked_weapon_on_enter: Option<WeaponSlotType>,
}

impl AIAttackPositionState {
    pub fn new(machine: &StateMachine) -> Self {
        Self {
            base: State::new(machine, "AIAttackPosition"),
            target_position: Coord3D::new(0.0, 0.0, 0.0),
            issued_attack: false,
            attack_machine: None,
            locked_weapon_on_enter: None,
        }
    }

    pub fn is_attack(&self) -> bool {
        true
    }
}

impl StateImplementation for AIAttackPositionState {
    fn on_enter(&mut self) -> StateReturnType {
        self.classic_on_enter().unwrap_or(StateReturnType::Failure)
    }

    fn update(&mut self) -> StateReturnType {
        self.classic_on_update().unwrap_or(StateReturnType::Failure)
    }

    fn on_exit(&mut self, _status: StateExitType) {
        let _ = self.classic_on_exit(_status);
    }

    fn xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::xfer(self, xfer)
    }
}

impl ClassicState for AIAttackPositionState {
    fn base_state(&self) -> &State {
        &self.base
    }

    fn base_state_mut(&mut self) -> &mut State {
        &mut self.base
    }

    fn classic_xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::xfer(self, xfer)
    }

    fn classic_on_enter(&mut self) -> Result<StateReturnType, String> {
        let owner = self
            .base
            .get_machine_owner()
            .ok_or_else(|| "attack position state missing machine owner".to_string())?;

        // C++ lines 5474-5478: Mood matrix sleep mode check
        {
            let owner_guard = owner.read().map_err(|_| "lock poisoned".to_string())?;
            if let Some(ai) = owner_guard.get_ai_update_interface() {
                if let Ok(mut ai_guard) = ai.lock() {
                    let adjustment =
                        ai_guard.get_mood_matrix_action_adjustment(MoodMatrixAction::Attack);
                    if (adjustment & mood_matrix_adjustment::ACTION_OK) == 0 {
                        return Ok(StateReturnType::Success);
                    }
                }
            }

            // C++ lines 5487-5490: Under construction check
            if owner_guard.test_status(ObjectStatusTypes::UnderConstruction) {
                return Ok(StateReturnType::Failure);
            }

            // C++ lines 5493-5495: Out of ammo check
            if owner_guard.is_out_of_ammo() && !owner_guard.is_kind_of(KindOf::Projectile) {
                return Ok(StateReturnType::Failure);
            }
        }

        // Set target position
        if let Some(pos) = self.base.get_machine_goal_position() {
            self.target_position = pos;
        } else {
            if let Ok(owner_guard) = owner.read() {
                self.target_position = *owner_guard.get_position();
            }
        }

        // C++ lines 5525-5527: Choose weapon (position variant uses INVALID_ID)
        let cmd_source = {
            let Ok(owner_guard) = owner.read() else {
                return Ok(StateReturnType::Failure);
            };
            if let Some(ai) = owner_guard.get_ai_update_interface() {
                if let Ok(ai_guard) = ai.lock() {
                    ai_guard.get_last_command_source()
                } else {
                    CommandSourceType::FromAi
                }
            } else {
                CommandSourceType::FromAi
            }
        };

        {
            let mut owner_guard = owner.write().map_err(|_| "lock poisoned".to_string())?;
            let weapon_found = owner_guard.choose_best_weapon_for_target_id(
                INVALID_ID,
                WeaponChoiceCriteria::PreferMostDamage,
                cmd_source,
            );
            if !weapon_found {
                return Ok(StateReturnType::Failure);
            }

            // C++ lines 5529-5536: Set max shots
            if let Some((weapon, _slot)) = owner_guard.get_current_weapon() {
                let continue_attack_range = weapon.get_continue_attack_range();
                owner_guard.set_current_weapon_max_shot_count(NO_MAX_SHOTS_LIMIT);
                if continue_attack_range > 0.0 {
                    owner_guard.set_status(
                        ObjectStatusMaskType::from_status(ObjectStatusTypes::IgnoringStealth),
                        true,
                    );
                }
            }

            // C++ line 5538: Track locked weapon on enter
            if owner_guard.is_cur_weapon_locked() {
                if let Some((_weapon, slot)) = owner_guard.get_current_weapon() {
                    self.locked_weapon_on_enter = Some(slot);
                }
            } else {
                self.locked_weapon_on_enter = None;
            }
        }

        // Create attack machine
        let mut attack_machine = AttackStateMachine::new(
            Arc::downgrade(&owner),
            "AIAttackMachine",
            false,
            false,
            false,
        );
        attack_machine.set_goal_position(self.target_position);

        // C++ lines 5540-5545: Init default state and set attacking status
        let ret = attack_machine.init_default_state();
        if ret == StateReturnType::Continue {
            if let Ok(mut owner_guard) = owner.write() {
                owner_guard.set_status(
                    ObjectStatusMaskType::from_status(ObjectStatusTypes::IsAttacking),
                    true,
                );
                owner_guard.set_model_condition_state(ModelConditionFlags::ATTACKING);
            }
        }
        self.attack_machine = Some(attack_machine);
        self.issued_attack = true;

        Ok(ret)
    }

    fn classic_on_update(&mut self) -> Result<StateReturnType, String> {
        let owner = self
            .base
            .get_machine_owner()
            .ok_or_else(|| "attack position state missing owner".to_string())?;

        // C++ lines 5565-5570: Out of ammo check every frame
        {
            let owner_guard = owner.read().map_err(|_| "lock poisoned".to_string())?;
            if owner_guard.is_out_of_ammo() && !owner_guard.is_kind_of(KindOf::Projectile) {
                return Ok(StateReturnType::Failure);
            }
        }

        // C++ lines 5640-5642: Re-evaluate weapon choice every frame
        {
            let cmd_source = {
                let Ok(owner_guard) = owner.read() else {
                    return Ok(StateReturnType::Failure);
                };
                if let Some(ai) = owner_guard.get_ai_update_interface() {
                    if let Ok(ai_guard) = ai.lock() {
                        ai_guard.get_last_command_source()
                    } else {
                        CommandSourceType::FromAi
                    }
                } else {
                    CommandSourceType::FromAi
                }
            };

            let mut owner_guard = owner.write().map_err(|_| "lock poisoned".to_string())?;
            let weapon_found = owner_guard.choose_best_weapon_for_target_id(
                crate::common::INVALID_ID,
                WeaponChoiceCriteria::PreferMostDamage,
                cmd_source,
            );
            if !weapon_found {
                return Ok(StateReturnType::Failure);
            }

            // C++ lines 5649-5650: Locked weapon drift check
            if let Some(locked_slot) = self.locked_weapon_on_enter {
                if let Some((_weapon, cur_slot)) = owner_guard.get_current_weapon() {
                    if cur_slot != locked_slot {
                        return Ok(StateReturnType::Failure);
                    }
                }
            }

            // C++ lines 5653-5654: Shot count check
            if let Some((weapon, _slot)) = owner_guard.get_current_weapon() {
                if weapon.get_max_shot_count() <= 0 {
                    return Ok(StateReturnType::Failure);
                }
            } else {
                return Ok(StateReturnType::Failure);
            }
        }

        // C++ line 5664: Run attack machine with CONVERT_SLEEP_TO_CONTINUE
        if let Some(attack_machine) = self.attack_machine.as_mut() {
            let result = attack_machine.update();
            return Ok(if matches!(result, StateReturnType::Sleep(_)) {
                StateReturnType::Continue
            } else {
                result
            });
        }

        Ok(StateReturnType::Continue)
    }

    fn classic_on_exit(&mut self, _exit: StateExitType) -> Result<(), String> {
        // Stop attacking — destroy attack machine (C++ AIAttackState::onExit)
        self.issued_attack = false;
        if let Some(mut machine) = self.attack_machine.take() {
            let _ = machine.halt();
        }

        if let Some(owner) = self.base.get_machine_owner() {
            // Clear attack-related status flags
            if let Ok(mut owner_guard) = owner.write() {
                owner_guard.clear_status(
                    ObjectStatusMaskType::IS_FIRING_WEAPON
                        | ObjectStatusMaskType::IS_AIMING_WEAPON
                        | ObjectStatusMaskType::IS_ATTACKING
                        | ObjectStatusMaskType::IGNORING_STEALTH,
                );
                owner_guard.clear_model_condition_state(ModelConditionFlags::ATTACKING);
                owner_guard.clear_leech_range_mode_for_all_weapons();

                // Clear AI state: current victim, turret targets, goal object
                if let Some(ai) = owner_guard.get_ai_update_interface() {
                    if let Ok(mut ai_guard) = ai.lock() {
                        ai_guard.set_current_victim(None);
                        for turret in [TurretType::Primary, TurretType::Secondary] {
                            ai_guard.set_turret_target_object(turret, None, false);
                        }
                        ai_guard.set_goal_object(None);
                    }
                }
            }
        }
        Ok(())
    }

    fn classic_is_attack(&self) -> bool {
        true
    }
}

pub(crate) const ENEMY_SCAN_RATE: u32 = LOGICFRAMES_PER_SECOND;

pub(crate) const CRATE_PICKUP_RANGE_SQR: f32 = 100.0;

#[derive(Debug)]
pub struct AIAttackThenIdleStateMachine {
    pub(crate) base: StateMachine,
}

impl AIAttackThenIdleStateMachine {
    pub fn new(owner: Weak<RwLock<Object>>, name: &str) -> Self {
        let mut base = StateMachine::new(Some(owner), name);
        let attack_state = AIAttackObjectState::new(&base, false, false);
        let pickup_state = AIPickUpCrateState::new(&base);
        let idle_state = AIIdleState::new(&base, false);
        register_classic_state(
            &mut base,
            AIStateType::AttackObject as u32,
            attack_state,
            Some(AIStateType::Idle as u32),
            Some(AIStateType::Idle as u32),
            &[],
        );
        register_classic_state(
            &mut base,
            AIStateType::PickUpCrate as u32,
            pickup_state,
            Some(AIStateType::Idle as u32),
            Some(AIStateType::Idle as u32),
            &[],
        );
        register_classic_state(
            &mut base,
            AIStateType::Idle as u32,
            idle_state,
            Some(AIStateType::Idle as u32),
            Some(AIStateType::Idle as u32),
            &[],
        );
        Self { base }
    }

    pub fn init_default_state(&mut self) -> StateReturnType {
        self.base.init_default_state()
    }

    pub fn set_goal_object(&mut self, obj_id: Option<ObjectID>) {
        self.base.set_goal_object_by_id(obj_id);
    }

    pub fn set_state(&mut self, state: AIStateType) -> StateReturnType {
        self.base.set_current_state(state as u32)
    }

    pub fn get_current_state_id(&self) -> Option<u32> {
        self.base.get_current_state_id()
    }

    pub fn update(&mut self) -> StateReturnType {
        self.base.update()
    }

    pub fn halt(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base.halt()
    }
}

#[derive(Debug)]
pub struct AIPickUpCrateState {
    pub(crate) base: AIMoveToState,
    pub(crate) delay_counter: i32,
    pub(crate) goal_position: Coord3D,
}

impl AIPickUpCrateState {
    pub fn new(machine: &StateMachine) -> Self {
        let mut base = AIMoveToState::new(machine);
        base.base.name = "AIAttackPickUpCrateState".to_string();
        Self {
            base,
            delay_counter: 0,
            goal_position: Coord3D::new(0.0, 0.0, 0.0),
        }
    }
}

impl StateImplementation for AIPickUpCrateState {
    fn on_enter(&mut self) -> StateReturnType {
        self.classic_on_enter().unwrap_or(StateReturnType::Failure)
    }

    fn update(&mut self) -> StateReturnType {
        self.classic_on_update().unwrap_or(StateReturnType::Failure)
    }

    fn on_exit(&mut self, _status: StateExitType) {
        let _ = self.classic_on_exit(_status);
    }

    fn xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::xfer(self, xfer)
    }
}

impl ClassicState for AIPickUpCrateState {
    fn base_state(&self) -> &State {
        &self.base.base
    }

    fn base_state_mut(&mut self) -> &mut State {
        &mut self.base.base
    }

    fn classic_xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::xfer(self, xfer)
    }

    fn classic_on_enter(&mut self) -> Result<StateReturnType, String> {
        // Wave 257: empty dual-world → fail-closed state.
        if dual_world_registry_unavailable() {
            return Ok(StateReturnType::Failure);
        }

        let goal_id = self
            .base
            .base
            .get_machine_goal_object_id()
            .ok_or_else(|| "pick up crate missing goal object".to_string())?;
        let goal = crate::helpers::TheGameLogic::find_object_by_id(goal_id)
            .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(goal_id))
            .ok_or_else(|| "pick up crate missing goal object".to_string())?;

        if let Ok(goal_guard) = goal.read() {
            self.goal_position = *goal_guard.get_position();
        }
        self.delay_counter = 3;
        self.base.set_adjusts_destination(true);

        Ok(StateReturnType::Continue)
    }

    fn classic_on_update(&mut self) -> Result<StateReturnType, String> {
        if self.delay_counter > 0 {
            self.delay_counter -= 1;
            if self.delay_counter == 0 {
                return self.base.classic_on_enter();
            }
            return Ok(StateReturnType::Continue);
        }

        self.base.classic_on_update()
    }

    fn classic_on_exit(&mut self, _exit: StateExitType) -> Result<(), String> {
        self.base.classic_on_exit(_exit)?;
        Ok(())
    }
}

#[derive(Debug)]
pub struct AIAttackSquadState {
    pub(crate) base: State,
    pub(crate) attack_squad_machine: Option<AIAttackThenIdleStateMachine>,
}

impl AIAttackSquadState {
    pub fn new(machine: &StateMachine) -> Self {
        Self {
            base: State::new(machine, "AIAttackSquad"),
            attack_squad_machine: None,
        }
    }

    pub(crate) fn choose_victim(&mut self) -> Option<Arc<RwLock<Object>>> {
        // Wave 257: empty dual-world → None.
        if dual_world_registry_unavailable() {
            return None;
        }

        let squad = self.base.get_machine_goal_squad()?;
        let owner = self.base.get_machine_owner()?;
        let owner_guard = owner.read().ok()?;
        let owner_pos = *owner_guard.get_position();
        let owner_off_map = owner_guard.is_off_map();
        let ai = owner_guard.get_ai_update_interface()?;

        let mood_val = ai
            .try_lock()
            .ok()
            .map(|guard| guard.get_mood_matrix_value())
            .unwrap_or(0);
        if (mood_val & mood_matrix_parameters::CONTROLLER_AI) != 0 {
            if (mood_val & mood_matrix_parameters::MOOD_SLEEP) != 0 {
                return None;
            }
            if (mood_val & mood_matrix_parameters::MOOD_PASSIVE) != 0 {
                let victim_id = owner_guard
                    .get_body_module()
                    .and_then(|body| body.get_last_damage_info())
                    .map(|info| info.input.source_id)
                    .unwrap_or(INVALID_ID);
                if victim_id == INVALID_ID {
                    return None;
                }
                return TheGameLogic::find_object_by_id(victim_id);
            }
        }

        let mut difficulty = owner_guard
            .get_controlling_player()
            .and_then(|player| {
                player
                    .read()
                    .ok()
                    .map(|player| player.get_player_difficulty())
            })
            .unwrap_or(crate::player::GameDifficulty::Normal);

        if ai.get_last_command_source() == CommandSourceType::FromPlayer {
            difficulty = crate::player::GameDifficulty::Hard;
        }
        if let Ok(script_guard) = get_script_engine().read() {
            if script_guard
                .as_ref()
                .map(|engine| engine.get_choose_victim_always_uses_normal())
                .unwrap_or(false)
            {
                difficulty = crate::player::GameDifficulty::Normal;
            }
        }
        drop(owner_guard);

        let mut squad_guard = squad.lock().ok()?;
        let object_ids = squad_guard.get_live_object_ids();

        match difficulty {
            crate::player::GameDifficulty::Easy => {
                if object_ids.is_empty() {
                    return None;
                }
                let idx =
                    GameLogicRandomValue(0, object_ids.len().saturating_sub(1) as i32) as usize;
                let id = *object_ids.get(idx)?;
                OBJECT_REGISTRY.get_object(id)
            }
            crate::player::GameDifficulty::Normal => {
                let mut best_id: Option<ObjectID> = None;
                let mut best_dist_sqr = f32::MAX;
                for id in &object_ids {
                    let Some(dist_sqr) = OBJECT_REGISTRY
                        .with_object(*id, |obj_guard| {
                            if obj_guard.is_off_map() != owner_off_map {
                                return None;
                            }
                            let target_pos = *obj_guard.get_position();
                            let dx = owner_pos.x - target_pos.x;
                            let dy = owner_pos.y - target_pos.y;
                            Some(dx * dx + dy * dy)
                        })
                        .flatten()
                    else {
                        continue;
                    };
                    if dist_sqr < best_dist_sqr {
                        best_dist_sqr = dist_sqr;
                        best_id = Some(*id);
                    }
                }
                best_id.and_then(|id| OBJECT_REGISTRY.get_object(id))
            }
            crate::player::GameDifficulty::Hard | crate::player::GameDifficulty::Brutal => {
                object_ids
                    .first()
                    .and_then(|id| OBJECT_REGISTRY.get_object(*id))
            }
        }
    }
}

impl StateImplementation for AIAttackSquadState {
    fn on_enter(&mut self) -> StateReturnType {
        self.classic_on_enter().unwrap_or(StateReturnType::Failure)
    }

    fn update(&mut self) -> StateReturnType {
        self.classic_on_update().unwrap_or(StateReturnType::Failure)
    }

    fn on_exit(&mut self, _status: StateExitType) {
        let _ = self.classic_on_exit(_status);
    }

    fn xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::xfer(self, xfer)
    }
}

impl ClassicState for AIAttackSquadState {
    fn base_state(&self) -> &State {
        &self.base
    }

    fn base_state_mut(&mut self) -> &mut State {
        &mut self.base
    }

    fn classic_xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::xfer(self, xfer)
    }

    fn classic_on_enter(&mut self) -> Result<StateReturnType, String> {
        let owner = self
            .base
            .get_machine_owner()
            .ok_or_else(|| "attack squad missing owner".to_string())?;
        let mut attack_machine =
            AIAttackThenIdleStateMachine::new(Arc::downgrade(&owner), "AIAttackMachine");

        let victim = self.choose_victim();
        if let Some(victim) = victim.as_ref() {
            attack_machine.set_goal_object(victim.read().ok().map(|g| g.get_id()));
        }

        let result = attack_machine.init_default_state();
        self.attack_squad_machine = Some(attack_machine);
        Ok(result)
    }

    fn classic_on_update(&mut self) -> Result<StateReturnType, String> {
        let attack_status = {
            let Some(attack_machine) = self.attack_squad_machine.as_mut() else {
                return Ok(StateReturnType::Failure);
            };
            let status = match attack_machine.update() {
                StateReturnType::Sleep(_) => StateReturnType::Continue,
                other => other,
            };
            if attack_machine.get_current_state_id() != Some(AIStateType::Idle as u32) {
                return Ok(status);
            }
            status
        };
        let _ = attack_status;

        if let Ok(owner_guard) = self
            .base
            .get_machine_owner()
            .ok_or_else(|| "attack squad missing owner".to_string())?
            .read()
        {
            if let Some(ai) = owner_guard.get_ai_update_interface() {
                if let Ok(ai_guard) = ai.try_lock() {
                    if let Some(crate_obj) = ai_guard.check_for_crate_to_pickup() {
                        if let Some(attack_machine) = self.attack_squad_machine.as_mut() {
                            attack_machine
                                .set_goal_object(crate_obj.read().ok().map(|g| g.get_id()));
                            attack_machine.set_state(AIStateType::PickUpCrate);
                        }
                        return Ok(StateReturnType::Continue);
                    }
                }
            }
        }

        let victim = self.choose_victim();
        let Some(victim) = victim else {
            return Ok(StateReturnType::Success);
        };

        if let Some(attack_machine) = self.attack_squad_machine.as_mut() {
            attack_machine.set_goal_object(victim.read().ok().map(|g| g.get_id()));
            attack_machine.set_state(AIStateType::AttackObject);
        }
        Ok(StateReturnType::Continue)
    }

    fn classic_on_exit(&mut self, _exit: StateExitType) -> Result<(), String> {
        if let Some(mut machine) = self.attack_squad_machine.take() {
            let _ = machine.halt();
        }
        Ok(())
    }

    fn classic_is_attack(&self) -> bool {
        true
    }
}

#[derive(Debug)]
pub struct AIAttackAreaState {
    pub(crate) base: State,
    pub(crate) attack_machine: Option<AIAttackThenIdleStateMachine>,
    pub(crate) next_enemy_scan_time: u32,
}

impl AIAttackAreaState {
    pub fn new(machine: &StateMachine) -> Self {
        Self {
            base: State::new(machine, "AIAttackArea"),
            attack_machine: None,
            next_enemy_scan_time: 0,
        }
    }

    pub(crate) fn find_area_victim(&self, owner: &Object) -> Option<Arc<RwLock<Object>>> {
        let polygon = self.base.get_machine_goal_polygon()?;
        let owner_id = owner.get_id();
        let attack_priority = resolve_attack_priority_info_for_object(owner_id);

        struct PolygonFilter {
            pub(crate) polygon: PolygonTrigger,
        }

        impl PartitionFilter for PolygonFilter {
            fn allow(&self, object: ObjectID) -> bool {
                let Some(target_arc) = TheGameLogic::find_object_by_id(object) else {
                    return false;
                };
                let Ok(target_guard) = target_arc.read() else {
                    return false;
                };
                let pos = target_guard.get_position();
                self.polygon.point_in_trigger(&Coord2D::new(pos.x, pos.y))
            }

            fn debug_get_name(&self) -> &str {
                "PartitionFilterPolygonTrigger"
            }
        }

        let filter = PolygonFilter {
            polygon: (*polygon).clone(),
        };

        let victim_id = THE_AI
            .read()
            .ok()?
            .find_closest_enemy(
                owner_id,
                9999.9,
                search_qualifiers::CAN_ATTACK,
                attack_priority.as_ref(),
                Some(&filter),
            )
            .ok()??;
        TheGameLogic::find_object_by_id(victim_id)
    }
}

impl StateImplementation for AIAttackAreaState {
    fn on_enter(&mut self) -> StateReturnType {
        self.classic_on_enter().unwrap_or(StateReturnType::Failure)
    }

    fn update(&mut self) -> StateReturnType {
        self.classic_on_update().unwrap_or(StateReturnType::Failure)
    }

    fn on_exit(&mut self, _status: StateExitType) {
        let _ = self.classic_on_exit(_status);
    }

    fn xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::xfer(self, xfer)
    }
}

impl ClassicState for AIAttackAreaState {
    fn base_state(&self) -> &State {
        &self.base
    }

    fn base_state_mut(&mut self) -> &mut State {
        &mut self.base
    }

    fn classic_xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::xfer(self, xfer)
    }

    fn classic_on_enter(&mut self) -> Result<StateReturnType, String> {
        let owner = self
            .base
            .get_machine_owner()
            .ok_or_else(|| "attack area missing owner".to_string())?;
        let mut attack_machine = AIAttackThenIdleStateMachine::new(
            Arc::downgrade(&owner),
            "AIAttackThenIdleStateMachine",
        );

        let now = TheGameLogic::get_frame();
        let jitter = GameLogicRandomValue(0, ENEMY_SCAN_RATE as i32) as u32;
        self.next_enemy_scan_time = now + jitter;

        let result = attack_machine.init_default_state();
        self.attack_machine = Some(attack_machine);
        Ok(result)
    }

    fn classic_on_update(&mut self) -> Result<StateReturnType, String> {
        let now = TheGameLogic::get_frame();
        if now >= self.next_enemy_scan_time {
            let owner = self
                .base
                .get_machine_owner()
                .ok_or_else(|| "attack area missing owner".to_string())?;

            if let Ok(owner_guard) = owner.read() {
                if owner_guard.is_out_of_ammo() && !owner_guard.is_kind_of(KindOf::Projectile) {
                    return Ok(StateReturnType::Failure);
                }
            }

            self.next_enemy_scan_time = now + ENEMY_SCAN_RATE;
            if self.base.get_machine_goal_polygon().is_none() {
                return Ok(StateReturnType::Failure);
            }
            let victim = owner
                .read()
                .ok()
                .and_then(|owner_guard| self.find_area_victim(&owner_guard));

            if let Some(attack_machine) = self.attack_machine.as_mut() {
                attack_machine.set_goal_object(
                    victim
                        .as_ref()
                        .and_then(|a| a.read().ok().map(|g| g.get_id())),
                );

                if attack_machine.get_current_state_id() == Some(AIStateType::Idle as u32)
                    && victim.is_some()
                {
                    attack_machine.set_state(AIStateType::AttackObject);
                }
            }

            if victim.is_none() {
                return Ok(StateReturnType::Success);
            }
        }

        if let Some(attack_machine) = self.attack_machine.as_mut() {
            if let Ok(machine) = self.base.get_machine() {
                if let Ok(mut machine_guard) = machine.lock() {
                    machine_guard.lock();
                    let result = attack_machine.update();
                    machine_guard.unlock();
                    return Ok(match result {
                        StateReturnType::Sleep(_) => StateReturnType::Continue,
                        other => other,
                    });
                }
            }
            return Ok(match attack_machine.update() {
                StateReturnType::Sleep(_) => StateReturnType::Continue,
                other => other,
            });
        }

        Ok(StateReturnType::Failure)
    }

    fn classic_on_exit(&mut self, _exit: StateExitType) -> Result<(), String> {
        if let Some(mut machine) = self.attack_machine.take() {
            let _ = machine.halt();
        }
        Ok(())
    }

    fn classic_is_attack(&self) -> bool {
        true
    }
}

impl Snapshotable for AIAttackObjectState {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: game_engine::common::system::xfer::XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to crc version: {:?}", e))?;
        let mut has_machine = self.attack_machine.is_some();
        xfer.xfer_bool(&mut has_machine)
            .map_err(|e| format!("Failed to crc attack object has_machine: {:?}", e))?;
        let mut original_victim_pos = self.original_victim_pos.clone();
        xfer.xfer_coord3d(&mut original_victim_pos);
        if let Some(machine) = self.attack_machine.as_ref() {
            machine.crc(xfer)?;
        }
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        // Wave 257: empty dual-world → Ok(()).
        if dual_world_registry_unavailable() {
            return Ok(());
        }

        let mut version: game_engine::common::system::xfer::XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to xfer version: {:?}", e))?;

        let mut has_machine = self.attack_machine.is_some();
        xfer.xfer_bool(&mut has_machine)
            .map_err(|e| format!("Failed to xfer attack object has_machine: {:?}", e))?;
        xfer.xfer_coord3d(&mut self.original_victim_pos);

        if xfer.is_loading() && has_machine && self.attack_machine.is_none() {
            let owner = self
                .base
                .get_machine_owner()
                .ok_or_else(|| "attack object state missing machine owner".to_string())?;
            let mut machine = AttackStateMachine::new(
                Arc::downgrade(&owner),
                "AIAttackMachine",
                self.follow_target,
                true,
                self.force_attack,
            );
            if self.target_id != INVALID_ID {
                if let Some(target) = TheGameLogic::find_object_by_id(self.target_id)
                    .or_else(|| OBJECT_REGISTRY.get_object(self.target_id))
                {
                    machine.set_goal_object(target.read().ok().map(|g| g.get_id()));
                }
            }
            self.attack_machine = Some(machine);
        }

        if let Some(machine) = self.attack_machine.as_mut() {
            machine.xfer(xfer)?;
        }

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        if let Some(machine) = self.attack_machine.as_mut() {
            machine.load_post_process()?;
        }
        Ok(())
    }
}

impl Snapshotable for AIAttackPositionState {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: game_engine::common::system::xfer::XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to crc version: {:?}", e))?;
        let mut has_machine = self.attack_machine.is_some();
        xfer.xfer_bool(&mut has_machine)
            .map_err(|e| format!("Failed to crc attack position has_machine: {:?}", e))?;
        let mut target_position = self.target_position.clone();
        xfer.xfer_coord3d(&mut target_position);
        if let Some(machine) = self.attack_machine.as_ref() {
            machine.crc(xfer)?;
        }
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: game_engine::common::system::xfer::XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to xfer version: {:?}", e))?;

        let mut has_machine = self.attack_machine.is_some();
        xfer.xfer_bool(&mut has_machine)
            .map_err(|e| format!("Failed to xfer attack position has_machine: {:?}", e))?;
        xfer.xfer_coord3d(&mut self.target_position);

        if xfer.is_loading() && has_machine && self.attack_machine.is_none() {
            let owner = self
                .base
                .get_machine_owner()
                .ok_or_else(|| "attack position state missing machine owner".to_string())?;
            let mut machine = AttackStateMachine::new(
                Arc::downgrade(&owner),
                "AIAttackMachine",
                false,
                false,
                false,
            );
            machine.set_goal_position(self.target_position);
            self.attack_machine = Some(machine);
        }

        if let Some(machine) = self.attack_machine.as_mut() {
            machine.xfer(xfer)?;
        }

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        if let Some(machine) = self.attack_machine.as_mut() {
            machine.load_post_process()?;
        }
        Ok(())
    }
}

impl Snapshotable for AIAttackMoveToState {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: game_engine::common::system::xfer::XferVersion = 2;
        xfer.xfer_version(&mut version, 2)
            .map_err(|e| format!("Failed to crc version: {:?}", e))?;

        Snapshotable::crc(&self.base, xfer)?;

        let mut has_machine = self.attack_move_machine.is_some();
        xfer.xfer_bool(&mut has_machine)
            .map_err(|e| format!("Failed to crc attack move has_machine: {:?}", e))?;
        if version >= 2 {
            let mut frame_to_sleep_until = self.frame_to_sleep_until;
            let mut retry_count = self.retry_count;
            xfer.xfer_unsigned_int(&mut frame_to_sleep_until)
                .map_err(|e| format!("Failed to crc frame_to_sleep_until: {:?}", e))?;
            xfer.xfer_int(&mut retry_count)
                .map_err(|e| format!("Failed to crc retry_count: {:?}", e))?;
        }

        if let Some(machine) = self.attack_move_machine.as_ref() {
            machine.crc(xfer)?;
        }

        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: game_engine::common::system::xfer::XferVersion = 2;
        xfer.xfer_version(&mut version, 2)
            .map_err(|e| format!("Failed to xfer version: {:?}", e))?;

        Snapshotable::xfer(&mut self.base, xfer)?;

        let mut has_machine = self.attack_move_machine.is_some();
        xfer.xfer_bool(&mut has_machine)
            .map_err(|e| format!("Failed to xfer attack move has_machine: {:?}", e))?;
        if version >= 2 {
            xfer.xfer_unsigned_int(&mut self.frame_to_sleep_until)
                .map_err(|e| format!("Failed to xfer frame_to_sleep_until: {:?}", e))?;
            xfer.xfer_int(&mut self.retry_count)
                .map_err(|e| format!("Failed to xfer retry_count: {:?}", e))?;
        }

        if xfer.is_loading() && has_machine && self.attack_move_machine.is_none() {
            let owner = self
                .base
                .base
                .get_machine_owner()
                .ok_or_else(|| "attack move-to missing machine owner".to_string())?;
            self.attack_move_machine = Some(AIAttackMoveStateMachine::new(
                Arc::downgrade(&owner),
                "AIAttackMoveMachine",
            ));
        }

        if let Some(machine) = self.attack_move_machine.as_mut() {
            machine.xfer(xfer)?;
        }

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        if let Some(machine) = self.attack_move_machine.as_mut() {
            machine.load_post_process()?;
        }
        Ok(())
    }
}

impl Snapshotable for AIAttackFollowWaypointPathAsTeamState {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: game_engine::common::system::xfer::XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to crc version: {:?}", e))?;

        self.base.crc(xfer)?;

        let mut has_machine = self.attack_follow_machine.is_some();
        xfer.xfer_bool(&mut has_machine)
            .map_err(|e| format!("Failed to crc attack-follow-team has_machine: {:?}", e))?;

        if let Some(machine) = self.attack_follow_machine.as_ref() {
            machine.crc(xfer)?;
        }

        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: game_engine::common::system::xfer::XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to xfer version: {:?}", e))?;

        self.base.xfer(xfer)?;

        let mut has_machine = self.attack_follow_machine.is_some();
        xfer.xfer_bool(&mut has_machine)
            .map_err(|e| format!("Failed to xfer attack-follow-team has_machine: {:?}", e))?;

        if xfer.is_loading() && has_machine && self.attack_follow_machine.is_none() {
            let owner = self
                .base
                .base
                .get_machine_owner()
                .ok_or_else(|| "attack follow path missing machine owner".to_string())?;
            self.attack_follow_machine = Some(AIAttackMoveStateMachine::new(
                Arc::downgrade(&owner),
                "AIAttackFollowMachine",
            ));
        }

        if let Some(machine) = self.attack_follow_machine.as_mut() {
            machine.xfer(xfer)?;
        }

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        if let Some(machine) = self.attack_follow_machine.as_mut() {
            machine.load_post_process()?;
        }
        Ok(())
    }
}

impl Snapshotable for AIAttackFollowWaypointPathAsIndividualsState {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: game_engine::common::system::xfer::XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to crc version: {:?}", e))?;

        self.base.crc(xfer)?;

        let mut has_machine = self.attack_follow_machine.is_some();
        xfer.xfer_bool(&mut has_machine).map_err(|e| {
            format!(
                "Failed to crc attack-follow-individuals has_machine: {:?}",
                e
            )
        })?;

        if let Some(machine) = self.attack_follow_machine.as_ref() {
            machine.crc(xfer)?;
        }

        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: game_engine::common::system::xfer::XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to xfer version: {:?}", e))?;

        self.base.xfer(xfer)?;

        let mut has_machine = self.attack_follow_machine.is_some();
        xfer.xfer_bool(&mut has_machine).map_err(|e| {
            format!(
                "Failed to xfer attack-follow-individuals has_machine: {:?}",
                e
            )
        })?;

        if xfer.is_loading() && has_machine && self.attack_follow_machine.is_none() {
            let owner = self
                .base
                .base
                .get_machine_owner()
                .ok_or_else(|| "attack follow path missing machine owner".to_string())?;
            self.attack_follow_machine = Some(AIAttackMoveStateMachine::new(
                Arc::downgrade(&owner),
                "AIAttackFollowMachine",
            ));
        }

        if let Some(machine) = self.attack_follow_machine.as_mut() {
            machine.xfer(xfer)?;
        }

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        if let Some(machine) = self.attack_follow_machine.as_mut() {
            machine.load_post_process()?;
        }
        Ok(())
    }
}

impl Snapshotable for AIAttackThenIdleStateMachine {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: game_engine::common::system::xfer::XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to crc version: {:?}", e))?;
        self.base
            .crc(xfer)
            .map_err(|e| format!("Failed to crc attack-then-idle machine: {:?}", e))?;
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: game_engine::common::system::xfer::XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to xfer version: {:?}", e))?;
        self.base
            .xfer(xfer)
            .map_err(|e| format!("Failed to xfer attack-then-idle machine: {:?}", e))?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        self.base.load_post_process().map_err(|e| {
            format!(
                "Failed to load post process attack-then-idle machine: {:?}",
                e
            )
        })
    }
}

impl Snapshotable for AIAttackSquadState {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: game_engine::common::system::xfer::XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to crc version: {:?}", e))?;

        let mut has_machine = self.attack_squad_machine.is_some();
        xfer.xfer_bool(&mut has_machine)
            .map_err(|e| format!("Failed to crc attack-squad has_machine: {:?}", e))?;

        if let Some(machine) = self.attack_squad_machine.as_ref() {
            machine.crc(xfer)?;
        }

        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: game_engine::common::system::xfer::XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to xfer version: {:?}", e))?;

        let mut has_machine = self.attack_squad_machine.is_some();
        xfer.xfer_bool(&mut has_machine)
            .map_err(|e| format!("Failed to xfer attack-squad has_machine: {:?}", e))?;

        if xfer.is_loading() && has_machine && self.attack_squad_machine.is_none() {
            let owner = self
                .base
                .get_machine_owner()
                .ok_or_else(|| "attack squad missing owner".to_string())?;
            self.attack_squad_machine = Some(AIAttackThenIdleStateMachine::new(
                Arc::downgrade(&owner),
                "AIAttackMachine",
            ));
        }

        if let Some(machine) = self.attack_squad_machine.as_mut() {
            machine.xfer(xfer)?;
        }

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        if let Some(machine) = self.attack_squad_machine.as_mut() {
            machine.load_post_process()?;
        }
        Ok(())
    }
}

impl Snapshotable for AIAttackAreaState {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: game_engine::common::system::xfer::XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to crc version: {:?}", e))?;

        let mut has_machine = self.attack_machine.is_some();
        xfer.xfer_bool(&mut has_machine)
            .map_err(|e| format!("Failed to crc attack-area has_machine: {:?}", e))?;

        if let Some(machine) = self.attack_machine.as_ref() {
            machine.crc(xfer)?;
        }

        let mut next_enemy_scan_time = self.next_enemy_scan_time;
        xfer.xfer_unsigned_int(&mut next_enemy_scan_time)
            .map_err(|e| format!("Failed to crc next_enemy_scan_time: {:?}", e))?;

        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: game_engine::common::system::xfer::XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to xfer version: {:?}", e))?;

        let mut has_machine = self.attack_machine.is_some();
        xfer.xfer_bool(&mut has_machine)
            .map_err(|e| format!("Failed to xfer attack-area has_machine: {:?}", e))?;

        if xfer.is_loading() && has_machine && self.attack_machine.is_none() {
            let owner = self
                .base
                .get_machine_owner()
                .ok_or_else(|| "attack area missing owner".to_string())?;
            self.attack_machine = Some(AIAttackThenIdleStateMachine::new(
                Arc::downgrade(&owner),
                "AIAttackThenIdleStateMachine",
            ));
        }

        if let Some(machine) = self.attack_machine.as_mut() {
            machine.xfer(xfer)?;
        }

        xfer.xfer_unsigned_int(&mut self.next_enemy_scan_time)
            .map_err(|e| format!("Failed to xfer next_enemy_scan_time: {:?}", e))?;

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        if let Some(machine) = self.attack_machine.as_mut() {
            machine.load_post_process()?;
        }
        Ok(())
    }
}

impl Snapshotable for AIPickUpCrateState {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: game_engine::common::system::xfer::XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to crc version: {:?}", e))?;

        let mut delay_counter = self.delay_counter;
        xfer.xfer_int(&mut delay_counter)
            .map_err(|e| format!("Failed to crc pick up crate delay_counter: {:?}", e))?;
        let mut goal_position = self.goal_position.clone();
        xfer.xfer_coord3d(&mut goal_position);

        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: game_engine::common::system::xfer::XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to xfer version: {:?}", e))?;

        xfer.xfer_int(&mut self.delay_counter)
            .map_err(|e| format!("Failed to xfer pick up crate delay_counter: {:?}", e))?;
        xfer.xfer_coord3d(&mut self.goal_position);

        if xfer.is_loading() {
            self.base.goal_position = self.goal_position;
        }

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}
