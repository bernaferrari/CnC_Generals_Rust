#![allow(deprecated, unused_imports, dead_code)]

use super::attack::*;
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

/// Follow waypoint path as team
#[derive(Debug)]
pub struct AIFollowWaypointPathAsTeamState {
    pub(crate) base: State,
    pub(crate) core: FollowWaypointPathCore,
}

impl AIFollowWaypointPathAsTeamState {
    pub fn new(machine: &StateMachine) -> Self {
        Self {
            base: State::new(machine, "AIFollowWaypointPathAsTeam"),
            core: FollowWaypointPathCore::new(true, true),
        }
    }
}

impl StateImplementation for AIFollowWaypointPathAsTeamState {
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

impl ClassicState for AIFollowWaypointPathAsTeamState {
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
        self.core.append_goal_position = false;
        self.core.prior_waypoint = None;
        self.core.frames_sleeping = 0;
        self.core.group_offset = Coord2D::new(0.0, 0.0);
        self.core.angle = 0.0;

        let machine = self.base.get_machine()?;
        let waypoint_id = machine
            .lock()
            .ok()
            .and_then(|guard| guard.get_goal_waypoint());
        self.core.current_waypoint = waypoint_id.and_then(resolve_waypoint_by_id);
        if self.core.current_waypoint.is_none() && !self.core.move_as_group {
            return Ok(StateReturnType::Failure);
        }

        if let Some(current) = self.core.current_waypoint.as_ref() {
            if let Ok(mut guard) = machine.lock() {
                guard.set_goal_position(current.position);
            }
        }

        let owner = self
            .base
            .get_machine_owner()
            .ok_or_else(|| "follow waypoint path missing owner".to_string())?;
        let owner_guard = owner
            .lock()
            .map_err(|_| "follow waypoint owner lock poisoned".to_string())?;
        let ai = owner_guard
            .get_ai_update_interface()
            .ok_or_else(|| "follow waypoint path missing AIUpdateInterface".to_string())?;
        let mut ai_guard = ai
            .lock()
            .map_err(|_| "follow waypoint path AI lock poisoned".to_string())?;

        let mut speed = FAST_AS_POSSIBLE;
        if self.core.move_as_group {
            if self.core.current_waypoint.is_none() {
                if let Some(team_arc) = owner_guard.get_team() {
                    if let Ok(team) = team_arc.read() {
                        self.core.current_waypoint = team
                            .get_current_waypoint_id()
                            .and_then(resolve_waypoint_by_id);
                    }
                }
            }
            if let Some(current) = self.core.current_waypoint.as_ref() {
                if let Some(team) = owner_guard.get_team() {
                    if let Ok(mut team_guard) = team.write() {
                        team_guard.set_current_waypoint_id(Some(current.id));
                    }
                }
            }
            if let Some(group_id) = owner_guard.get_group_id() {
                if let Ok(ai_lock) = THE_AI.read() {
                    if let Some(group) = ai_lock.find_group(group_id) {
                        if let Ok(mut group_guard) = group.write() {
                            speed = group_guard.get_speed();
                            if let Some(center) = group_guard.get_center() {
                                let pos = owner_guard.get_position();
                                self.core.group_offset.x = pos.x - center.x;
                                self.core.group_offset.y = pos.y - center.y;
                            }
                        }
                    }
                }
            }
        }

        self.core.compute_goal(
            &self.base,
            &owner_guard,
            &mut *ai_guard,
            self.core.move_as_group,
        )?;
        if !self.core.has_next_waypoint() && ai_guard.is_doing_ground_movement() {
            if !ai_guard.adjust_destination(&mut self.core.goal_position) {
                return Ok(StateReturnType::Failure);
            }
        }
        self.core.compute_path(&mut *ai_guard)?;
        ai_guard.set_desired_speed(speed);
        ai_guard
            .set_path_extra_distance(self.core.calc_extra_path_distance())
            .map_err(|e| e.to_string())?;
        if ai_guard.is_doing_ground_movement() {
            let _ = ai_guard.update_goal_position(&self.core.goal_position, self.core.goal_layer);
        }

        Ok(StateReturnType::Continue)
    }

    fn classic_on_update(&mut self) -> Result<StateReturnType, String> {
        if self.core.frames_sleeping > 0 {
            self.core.frames_sleeping = self.core.frames_sleeping.saturating_sub(1);
            return Ok(StateReturnType::Continue);
        }

        let owner = self
            .base
            .get_machine_owner()
            .ok_or_else(|| "follow waypoint path missing owner".to_string())?;
        let owner_guard = owner
            .lock()
            .map_err(|_| "follow waypoint owner lock poisoned".to_string())?;
        let ai = owner_guard
            .get_ai_update_interface()
            .ok_or_else(|| "follow waypoint path missing AIUpdateInterface".to_string())?;
        let mut ai_guard = ai
            .lock()
            .map_err(|_| "follow waypoint path AI lock poisoned".to_string())?;

        if let Some(current) = self.core.current_waypoint.as_ref() {
            if let Ok(machine) = self.base.get_machine() {
                if let Ok(mut guard) = machine.lock() {
                    guard.set_goal_position(current.position);
                }
            }
        } else {
            return Ok(StateReturnType::Success);
        }

        if self.core.is_follow_waypoint_path_state {
            let adjustment = ai_guard.get_mood_matrix_action_adjustment(MoodMatrixAction::Move);
            if (adjustment & mood_matrix_adjustment::ACTION_TO_ATTACK_MOVE) != 0 {
                if let Some(current) = self.core.current_waypoint.as_ref() {
                    if self.core.move_as_group {
                        ai.ai_attack_follow_waypoint_path_as_team(
                            current,
                            NO_MAX_SHOTS_LIMIT,
                            CommandSourceType::FromAi,
                        );
                    } else {
                        ai.ai_attack_follow_waypoint_path(
                            current,
                            NO_MAX_SHOTS_LIMIT,
                            CommandSourceType::FromAi,
                        );
                    }
                }
            }
        }

        if self.core.append_goal_position
            && !ai_guard.is_waiting_for_path()
            && ai.get_path().is_some()
        {
            ai_guard.append_goal_position_to_path(&self.core.goal_position)?;
            self.core.append_goal_position = false;
        }

        if self.core.move_as_group {
            if let Some(team) = owner_guard.get_team() {
                if let Ok(team_guard) = team.read() {
                    if team_guard.get_current_waypoint_id()
                        != self.core.current_waypoint.as_ref().map(|w| w.id)
                    {
                        self.core.prior_waypoint = self.core.current_waypoint.clone();
                        self.core.current_waypoint = team_guard
                            .get_current_waypoint_id()
                            .and_then(resolve_waypoint_by_id);
                        if self.core.current_waypoint.is_none() {
                            return Ok(StateReturnType::Success);
                        }
                        self.core
                            .compute_goal(&self.base, &owner_guard, &mut *ai_guard, false)?;
                        if !self.core.has_next_waypoint() && ai_guard.is_doing_ground_movement() {
                            if !ai_guard.adjust_destination(&mut self.core.goal_position) {
                                return Ok(StateReturnType::Failure);
                            }
                        }
                        ai_guard.friend_starting_move();
                        self.core.compute_path(&mut *ai_guard)?;
                        if ai_guard.is_doing_ground_movement() {
                            let _ = ai_guard.update_goal_position(
                                &self.core.goal_position,
                                self.core.goal_layer,
                            );
                        }
                    }
                }
            }
        }

        let frames_blocked = ai_guard.get_num_frames_blocked();
        let blocked =
            ai_guard.is_blocked_and_stuck() || frames_blocked > 2 * LOGICFRAMES_PER_SECOND;
        if blocked {
            let _ = self.core.compute_path(&mut *ai_guard);
        }

        let close_enough = ai_guard
            .get_cur_locomotor()
            .and_then(|loc| loc.lock().ok().map(|loco| loco.get_close_enough_dist()))
            .unwrap_or(0.0);

        let mut status = StateReturnType::Continue;
        if ai_guard.get_locomotor_distance_to_goal() <= close_enough {
            status = StateReturnType::Success;
        }

        if self.core.move_as_group {
            if let Some(player) = owner_guard.get_controlling_player() {
                if let Ok(player_guard) = player.read() {
                    if player_guard.is_skirmish_ai() {
                        if let Some(group_id) = owner_guard.get_group_id() {
                            if let Ok(ai_lock) = THE_AI.read() {
                                if let Some(group) = ai_lock.find_group(group_id) {
                                    if let Ok(group_guard) = group.read() {
                                        if let Some(center) = group_guard.get_center() {
                                            let dx = center.x - self.core.goal_position.x;
                                            let dy = center.y - self.core.goal_position.y;
                                            let dist = (dx * dx + dy * dy).sqrt();
                                            let num = group_guard.get_count() as f32;
                                            let fudge = ai_lock
                                                .get_ai_data()
                                                .read()
                                                .map(|d| d.skirmish_group_fudge_value)
                                                .unwrap_or(0.0);
                                            if dist <= num * fudge {
                                                status = StateReturnType::Success;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        if status != StateReturnType::Continue {
            let prior_id = self.core.prior_waypoint.as_ref().map(|w| w.id);
            if let Some(prior) = prior_id {
                ai_guard.set_prior_waypoint_id(prior);
            }
            let next = self.core.get_next_waypoint(&self.base);
            self.core.current_waypoint = next.clone();
            if let Some(current) = next.as_ref() {
                ai_guard.set_current_waypoint_id(current.id);
            }

            if next.is_none() {
                ai_guard.set_completed_waypoint_id(prior_id);
                return Ok(StateReturnType::Success);
            }

            self.core
                .compute_goal(&self.base, &owner_guard, &mut *ai_guard, false)?;
            if !self.core.has_next_waypoint() && ai_guard.is_doing_ground_movement() {
                if !ai_guard.adjust_destination(&mut self.core.goal_position) {
                    return Ok(StateReturnType::Failure);
                }
            }
            ai_guard.friend_starting_move();
            self.core.compute_path(&mut *ai_guard)?;
            if ai_guard.is_doing_ground_movement() {
                let _ =
                    ai_guard.update_goal_position(&self.core.goal_position, self.core.goal_layer);
            }
            if let Some(current) = self.core.current_waypoint.as_ref() {
                if self.core.move_as_group {
                    if let Some(team) = owner_guard.get_team() {
                        if let Ok(mut team_guard) = team.write() {
                            team_guard.set_current_waypoint_id(Some(current.id));
                        }
                    }
                }
            }
        }

        Ok(status)
    }

    fn classic_on_exit(&mut self, _exit: StateExitType) -> Result<(), String> {
        if let Some(owner) = self.base.get_machine_owner() {
            if let Ok(owner_guard) = owner.lock() {
                if let Some(ai) = owner_guard.get_ai_update_interface() {
                    if let Ok(ai_guard) = ai.lock() {
                        if let Some(loco) = ai_guard.get_cur_locomotor() {
                            if let Ok(mut guard) = loco.lock() {
                                guard.set_precise_z_pos(false);
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn classic_is_busy(&self) -> bool {
        true
    }

    fn classic_is_attack(&self) -> bool {
        false
    }
}

/// Follow waypoint path exact as team (no pathfinding, follow waypoint links exactly).
#[derive(Debug)]
pub struct AIFollowWaypointPathAsTeamExactState {
    pub(crate) base: State,
    pub(crate) move_as_group: bool,
    pub(crate) last_waypoint: Option<Arc<Waypoint>>,
}

impl AIFollowWaypointPathAsTeamExactState {
    pub fn new(machine: &StateMachine) -> Self {
        Self {
            base: State::new(machine, "AIFollowWaypointPathAsTeamExact"),
            move_as_group: true,
            last_waypoint: None,
        }
    }
}

impl StateImplementation for AIFollowWaypointPathAsTeamExactState {
    fn on_enter(&mut self) -> StateReturnType {
        self.classic_on_enter().unwrap_or(StateReturnType::Failure)
    }

    fn update(&mut self) -> StateReturnType {
        self.classic_on_update().unwrap_or(StateReturnType::Failure)
    }

    fn on_exit(&mut self, status: StateExitType) {
        let _ = self.classic_on_exit(status);
    }

    fn xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::xfer(self, xfer)
    }
}

impl ClassicState for AIFollowWaypointPathAsTeamExactState {
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
        let machine = self.base.get_machine()?;
        let waypoint_id = machine
            .lock()
            .ok()
            .and_then(|guard| guard.get_goal_waypoint());
        let current = waypoint_id.and_then(resolve_waypoint_by_id);
        let current =
            current.ok_or_else(|| "follow waypoint exact missing waypoint".to_string())?;

        if let Ok(mut guard) = machine.lock() {
            guard.set_goal_position(current.position);
        }

        let owner = self
            .base
            .get_machine_owner()
            .ok_or_else(|| "follow waypoint exact missing owner".to_string())?;
        let owner_guard = owner
            .lock()
            .map_err(|_| "follow waypoint exact owner lock poisoned".to_string())?;
        let ai = owner_guard
            .get_ai_update_interface()
            .ok_or_else(|| "follow waypoint exact missing AIUpdateInterface".to_string())?;
        let mut ai_guard = ai
            .lock()
            .map_err(|_| "follow waypoint exact AI lock poisoned".to_string())?;

        let mut speed = FAST_AS_POSSIBLE;
        let mut group_offset = Coord2D::new(0.0, 0.0);
        if self.move_as_group {
            if let Some(group_id) = owner_guard.get_group_id() {
                if let Ok(ai_lock) = THE_AI.read() {
                    if let Some(group) = ai_lock.find_group(group_id) {
                        if let Ok(mut group_guard) = group.write() {
                            speed = group_guard.get_speed();
                            if let Some(center) = group_guard.get_center() {
                                let pos = owner_guard.get_position();
                                group_offset.x = pos.x - center.x;
                                group_offset.y = pos.y - center.y;
                            }
                        }
                    }
                }
            }
        }

        let _ = ai_guard.set_can_path_through_units(true);
        ai_guard.set_adjusts_destination(false);
        ai_guard.set_path_from_waypoint(&current, &group_offset)?;
        let _ = ai_guard.set_allow_invalid_position(true);
        ai_guard.set_desired_speed(speed);

        self.last_waypoint = Some(current);
        Ok(StateReturnType::Continue)
    }

    fn classic_on_update(&mut self) -> Result<StateReturnType, String> {
        let owner = self
            .base
            .get_machine_owner()
            .ok_or_else(|| "follow waypoint exact missing owner".to_string())?;
        let owner_guard = owner
            .lock()
            .map_err(|_| "follow waypoint exact owner lock poisoned".to_string())?;
        let ai = owner_guard
            .get_ai_update_interface()
            .ok_or_else(|| "follow waypoint exact missing AIUpdateInterface".to_string())?;
        let mut ai_guard = ai
            .lock()
            .map_err(|_| "follow waypoint exact AI lock poisoned".to_string())?;

        let _ = ai_guard.set_can_path_through_units(true);
        if !ai_guard.is_moving() && ai_guard.is_waypoint_queue_empty() {
            return Ok(StateReturnType::Success);
        }

        Ok(StateReturnType::Continue)
    }

    fn classic_on_exit(&mut self, _exit: StateExitType) -> Result<(), String> {
        if let Some(owner) = self.base.get_machine_owner() {
            if let Ok(owner_guard) = owner.lock() {
                if let Some(ai) = owner_guard.get_ai_update_interface() {
                    if let Ok(mut ai_guard) = ai.lock() {
                        if let Some(last) = self.last_waypoint.as_ref() {
                            ai_guard.set_completed_waypoint_id(Some(last.id));
                        }
                        let _ = ai_guard.set_can_path_through_units(false);
                        let _ = ai_guard.set_allow_invalid_position(false);
                    }
                }
            }
        }
        Ok(())
    }

    fn classic_is_busy(&self) -> bool {
        true
    }
}

/// Follow waypoint path as individuals
#[derive(Debug)]
pub struct AIFollowWaypointPathAsIndividualsState {
    pub(crate) base: State,
    pub(crate) core: FollowWaypointPathCore,
}

impl AIFollowWaypointPathAsIndividualsState {
    pub fn new(machine: &StateMachine) -> Self {
        Self {
            base: State::new(machine, "AIFollowWaypointPathAsIndividuals"),
            core: FollowWaypointPathCore::new(false, true),
        }
    }
}

impl StateImplementation for AIFollowWaypointPathAsIndividualsState {
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

impl ClassicState for AIFollowWaypointPathAsIndividualsState {
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
        self.core.append_goal_position = false;
        self.core.prior_waypoint = None;
        self.core.frames_sleeping = 0;
        self.core.group_offset = Coord2D::new(0.0, 0.0);
        self.core.angle = 0.0;

        let machine = self.base.get_machine()?;
        let waypoint_id = machine
            .lock()
            .ok()
            .and_then(|guard| guard.get_goal_waypoint());
        self.core.current_waypoint = waypoint_id.and_then(resolve_waypoint_by_id);
        if self.core.current_waypoint.is_none() && !self.core.move_as_group {
            return Ok(StateReturnType::Failure);
        }

        if let Some(current) = self.core.current_waypoint.as_ref() {
            if let Ok(mut guard) = machine.lock() {
                guard.set_goal_position(current.position);
            }
        }

        let owner = self
            .base
            .get_machine_owner()
            .ok_or_else(|| "follow waypoint path missing owner".to_string())?;
        let owner_guard = owner
            .lock()
            .map_err(|_| "follow waypoint owner lock poisoned".to_string())?;
        let ai = owner_guard
            .get_ai_update_interface()
            .ok_or_else(|| "follow waypoint path missing AIUpdateInterface".to_string())?;
        let mut ai_guard = ai
            .lock()
            .map_err(|_| "follow waypoint path AI lock poisoned".to_string())?;

        self.core
            .compute_goal(&self.base, &owner_guard, &mut *ai_guard, false)?;
        if !self.core.has_next_waypoint() && ai_guard.is_doing_ground_movement() {
            if !ai_guard.adjust_destination(&mut self.core.goal_position) {
                return Ok(StateReturnType::Failure);
            }
        }
        self.core.compute_path(&mut *ai_guard)?;
        ai_guard.set_desired_speed(FAST_AS_POSSIBLE);
        ai_guard
            .set_path_extra_distance(self.core.calc_extra_path_distance())
            .map_err(|e| e.to_string())?;
        if ai_guard.is_doing_ground_movement() {
            let _ = ai_guard.update_goal_position(&self.core.goal_position, self.core.goal_layer);
        }

        Ok(StateReturnType::Continue)
    }

    fn classic_on_update(&mut self) -> Result<StateReturnType, String> {
        if self.core.frames_sleeping > 0 {
            self.core.frames_sleeping = self.core.frames_sleeping.saturating_sub(1);
            return Ok(StateReturnType::Continue);
        }

        let owner = self
            .base
            .get_machine_owner()
            .ok_or_else(|| "follow waypoint path missing owner".to_string())?;
        let owner_guard = owner
            .lock()
            .map_err(|_| "follow waypoint owner lock poisoned".to_string())?;
        let ai = owner_guard
            .get_ai_update_interface()
            .ok_or_else(|| "follow waypoint path missing AIUpdateInterface".to_string())?;
        let mut ai_guard = ai
            .lock()
            .map_err(|_| "follow waypoint path AI lock poisoned".to_string())?;

        if let Some(current) = self.core.current_waypoint.as_ref() {
            if let Ok(machine) = self.base.get_machine() {
                if let Ok(mut guard) = machine.lock() {
                    guard.set_goal_position(current.position);
                }
            }
        } else {
            return Ok(StateReturnType::Success);
        }

        if self.core.is_follow_waypoint_path_state {
            let adjustment = ai_guard.get_mood_matrix_action_adjustment(MoodMatrixAction::Move);
            if (adjustment & mood_matrix_adjustment::ACTION_TO_ATTACK_MOVE) != 0 {
                if let Some(current) = self.core.current_waypoint.as_ref() {
                    ai.ai_attack_follow_waypoint_path(
                        current,
                        NO_MAX_SHOTS_LIMIT,
                        CommandSourceType::FromAi,
                    );
                }
            }
        }

        if self.core.append_goal_position
            && !ai_guard.is_waiting_for_path()
            && ai.get_path().is_some()
        {
            ai_guard.append_goal_position_to_path(&self.core.goal_position)?;
            self.core.append_goal_position = false;
        }

        let frames_blocked = ai_guard.get_num_frames_blocked();
        let blocked =
            ai_guard.is_blocked_and_stuck() || frames_blocked > 2 * LOGICFRAMES_PER_SECOND;
        if blocked {
            let _ = self.core.compute_path(&mut *ai_guard);
        }

        let close_enough = ai_guard
            .get_cur_locomotor()
            .and_then(|loc| loc.lock().ok().map(|loco| loco.get_close_enough_dist()))
            .unwrap_or(0.0);

        let mut status = StateReturnType::Continue;
        if ai_guard.get_locomotor_distance_to_goal() <= close_enough {
            status = StateReturnType::Success;
        }

        if status != StateReturnType::Continue {
            let prior_id = self.core.prior_waypoint.as_ref().map(|w| w.id);
            if let Some(prior) = prior_id {
                ai_guard.set_prior_waypoint_id(prior);
            }
            let next = self.core.get_next_waypoint(&self.base);
            self.core.current_waypoint = next.clone();
            if let Some(current) = next.as_ref() {
                ai_guard.set_current_waypoint_id(current.id);
            }

            if next.is_none() {
                ai_guard.set_completed_waypoint_id(prior_id);
                return Ok(StateReturnType::Success);
            }

            self.core
                .compute_goal(&self.base, &owner_guard, &mut *ai_guard, false)?;
            if !self.core.has_next_waypoint() && ai_guard.is_doing_ground_movement() {
                if !ai_guard.adjust_destination(&mut self.core.goal_position) {
                    return Ok(StateReturnType::Failure);
                }
            }
            ai_guard.friend_starting_move();
            self.core.compute_path(&mut *ai_guard)?;
            if ai_guard.is_doing_ground_movement() {
                let _ =
                    ai_guard.update_goal_position(&self.core.goal_position, self.core.goal_layer);
            }
        }

        Ok(status)
    }

    fn classic_on_exit(&mut self, _exit: StateExitType) -> Result<(), String> {
        if let Some(owner) = self.base.get_machine_owner() {
            if let Ok(owner_guard) = owner.lock() {
                if let Some(ai) = owner_guard.get_ai_update_interface() {
                    if let Ok(ai_guard) = ai.lock() {
                        if let Some(loco) = ai_guard.get_cur_locomotor() {
                            if let Ok(mut guard) = loco.lock() {
                                guard.set_precise_z_pos(false);
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn classic_is_busy(&self) -> bool {
        true
    }

    fn classic_is_attack(&self) -> bool {
        false
    }
}

/// Follow waypoint path exact as individuals (no pathfinding).
#[derive(Debug)]
pub struct AIFollowWaypointPathAsIndividualsExactState {
    pub(crate) base: State,
    pub(crate) move_as_group: bool,
    pub(crate) last_waypoint: Option<Arc<Waypoint>>,
}

impl AIFollowWaypointPathAsIndividualsExactState {
    pub fn new(machine: &StateMachine) -> Self {
        Self {
            base: State::new(machine, "AIFollowWaypointPathAsIndividualsExact"),
            move_as_group: false,
            last_waypoint: None,
        }
    }
}

impl StateImplementation for AIFollowWaypointPathAsIndividualsExactState {
    fn on_enter(&mut self) -> StateReturnType {
        self.classic_on_enter().unwrap_or(StateReturnType::Failure)
    }

    fn update(&mut self) -> StateReturnType {
        self.classic_on_update().unwrap_or(StateReturnType::Failure)
    }

    fn on_exit(&mut self, status: StateExitType) {
        let _ = self.classic_on_exit(status);
    }

    fn xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::xfer(self, xfer)
    }
}

impl ClassicState for AIFollowWaypointPathAsIndividualsExactState {
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
        let machine = self.base.get_machine()?;
        let waypoint_id = machine
            .lock()
            .ok()
            .and_then(|guard| guard.get_goal_waypoint());
        let current = waypoint_id.and_then(resolve_waypoint_by_id);
        let current =
            current.ok_or_else(|| "follow waypoint exact missing waypoint".to_string())?;

        if let Ok(mut guard) = machine.lock() {
            guard.set_goal_position(current.position);
        }

        let owner = self
            .base
            .get_machine_owner()
            .ok_or_else(|| "follow waypoint exact missing owner".to_string())?;
        let owner_guard = owner
            .lock()
            .map_err(|_| "follow waypoint exact owner lock poisoned".to_string())?;
        let ai = owner_guard
            .get_ai_update_interface()
            .ok_or_else(|| "follow waypoint exact missing AIUpdateInterface".to_string())?;
        let mut ai_guard = ai
            .lock()
            .map_err(|_| "follow waypoint exact AI lock poisoned".to_string())?;

        let group_offset = Coord2D::new(0.0, 0.0);
        let _ = ai_guard.set_can_path_through_units(true);
        ai_guard.set_adjusts_destination(false);
        ai_guard.set_path_from_waypoint(&current, &group_offset)?;
        let _ = ai_guard.set_allow_invalid_position(true);
        ai_guard.set_desired_speed(FAST_AS_POSSIBLE);

        self.last_waypoint = Some(current);
        Ok(StateReturnType::Continue)
    }

    fn classic_on_update(&mut self) -> Result<StateReturnType, String> {
        let owner = self
            .base
            .get_machine_owner()
            .ok_or_else(|| "follow waypoint exact missing owner".to_string())?;
        let owner_guard = owner
            .lock()
            .map_err(|_| "follow waypoint exact owner lock poisoned".to_string())?;
        let ai = owner_guard
            .get_ai_update_interface()
            .ok_or_else(|| "follow waypoint exact missing AIUpdateInterface".to_string())?;
        let mut ai_guard = ai
            .lock()
            .map_err(|_| "follow waypoint exact AI lock poisoned".to_string())?;

        let _ = ai_guard.set_can_path_through_units(true);
        if !ai_guard.is_moving() && ai_guard.is_waypoint_queue_empty() {
            return Ok(StateReturnType::Success);
        }

        Ok(StateReturnType::Continue)
    }

    fn classic_on_exit(&mut self, _exit: StateExitType) -> Result<(), String> {
        if let Some(owner) = self.base.get_machine_owner() {
            if let Ok(owner_guard) = owner.lock() {
                if let Some(ai) = owner_guard.get_ai_update_interface() {
                    if let Ok(mut ai_guard) = ai.lock() {
                        if let Some(last) = self.last_waypoint.as_ref() {
                            ai_guard.set_completed_waypoint_id(Some(last.id));
                        }
                        let _ = ai_guard.set_can_path_through_units(false);
                        let _ = ai_guard.set_allow_invalid_position(false);
                    }
                }
            }
        }
        Ok(())
    }

    fn classic_is_busy(&self) -> bool {
        true
    }
}

impl Snapshotable for AIFollowWaypointPathAsTeamState {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: game_engine::common::system::xfer::XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to crc version: {:?}", e))?;

        self.core.crc(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: game_engine::common::system::xfer::XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to xfer version: {:?}", e))?;

        self.core.xfer(xfer)
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

impl Snapshotable for AIFollowWaypointPathAsIndividualsState {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: game_engine::common::system::xfer::XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to crc version: {:?}", e))?;

        self.core.crc(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: game_engine::common::system::xfer::XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to xfer version: {:?}", e))?;

        self.core.xfer(xfer)
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

impl Snapshotable for AIFollowWaypointPathAsTeamExactState {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: game_engine::common::system::xfer::XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to crc version: {:?}", e))?;

        let mut id: WaypointId = self
            .last_waypoint
            .as_ref()
            .map(|w| w.id)
            .unwrap_or(INVALID_ID);
        xfer.xfer_unsigned_int(&mut id)
            .map_err(|e| format!("Failed to crc team waypoint id: {:?}", e))?;

        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: game_engine::common::system::xfer::XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to xfer version: {:?}", e))?;

        let mut id: WaypointId = self
            .last_waypoint
            .as_ref()
            .map(|w| w.id)
            .unwrap_or(INVALID_ID);
        xfer.xfer_unsigned_int(&mut id)
            .map_err(|e| format!("Failed to xfer team waypoint id: {:?}", e))?;
        if xfer.is_loading() {
            self.last_waypoint = if id == INVALID_ID {
                None
            } else {
                resolve_waypoint_by_id(id)
            };
        }

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

impl Snapshotable for AIFollowWaypointPathAsIndividualsExactState {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: game_engine::common::system::xfer::XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to crc version: {:?}", e))?;

        let mut id: WaypointId = self
            .last_waypoint
            .as_ref()
            .map(|w| w.id)
            .unwrap_or(INVALID_ID);
        xfer.xfer_unsigned_int(&mut id)
            .map_err(|e| format!("Failed to crc individual waypoint id: {:?}", e))?;

        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: game_engine::common::system::xfer::XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to xfer version: {:?}", e))?;

        let mut id: WaypointId = self
            .last_waypoint
            .as_ref()
            .map(|w| w.id)
            .unwrap_or(INVALID_ID);
        xfer.xfer_unsigned_int(&mut id)
            .map_err(|e| format!("Failed to xfer individual waypoint id: {:?}", e))?;
        if xfer.is_loading() {
            self.last_waypoint = if id == INVALID_ID {
                None
            } else {
                resolve_waypoint_by_id(id)
            };
        }

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}
