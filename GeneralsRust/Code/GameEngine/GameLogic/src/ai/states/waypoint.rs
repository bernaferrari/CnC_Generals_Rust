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
    AiCommandInterface, AiCommandParams, GuardMode, MoodMatrixAction, PartitionFilter, the_ai,
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

    fn on_enter_with_waypoint(
        &mut self,
        ai: &mut dyn crate::modules::AIUpdateInterface,
        _goal_id: crate::common::ObjectID,
        _goal_pos: Coord3D,
        waypoint: Option<crate::waypoint::WaypointId>,
    ) -> StateReturnType {
        self.core.append_goal_position = false;
        self.core.prior_waypoint = None;
        self.core.frames_sleeping = 0;
        self.core.group_offset = Coord2D::new(0.0, 0.0);
        self.core.angle = 0.0;
        self.core.current_waypoint = waypoint.and_then(resolve_waypoint_by_id);
        if self.core.current_waypoint.is_none() && !self.core.move_as_group {
            return StateReturnType::Failure;
        }
        let Some(owner) = self.base.get_machine_owner() else {
            return StateReturnType::Failure;
        };
        let Some(result) = crate::object::registry::OBJECT_REGISTRY.with_object(owner, |owner_guard| {
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
                if let Ok(ai_lock) = the_ai().read() {
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
        if self
            .core
            .compute_goal(&self.base, owner_guard, ai, self.core.move_as_group)
            .is_err()
        {
            return StateReturnType::Failure;
        }
        if !self.core.has_next_waypoint()
            && ai.is_doing_ground_movement()
            && !ai.adjust_destination(&mut self.core.goal_position)
        {
            return StateReturnType::Failure;
        }
        if self.core.compute_path(ai).is_err() {
            return StateReturnType::Failure;
        }
        ai.set_desired_speed(speed);
        if ai
            .set_path_extra_distance(self.core.calc_extra_path_distance())
            .is_err()
        {
            return StateReturnType::Failure;
        }
        if ai.is_doing_ground_movement() {
            let _ = ai.update_goal_position(&self.core.goal_position, self.core.goal_layer);
        }
        StateReturnType::Continue
        }) else {
            return StateReturnType::Failure;
        };
        result
    }

    fn update(&mut self) -> StateReturnType {
        self.classic_on_update().unwrap_or(StateReturnType::Failure)
    }

    fn update_with_ai(
        &mut self,
        ai: &mut dyn crate::modules::AIUpdateInterface,
    ) -> StateReturnType {
        if self.core.frames_sleeping > 0 {
            self.core.frames_sleeping = self.core.frames_sleeping.saturating_sub(1);
            return StateReturnType::Continue;
        }
        if self.core.current_waypoint.is_none() {
            return StateReturnType::Success;
        }
        if self.core.is_follow_waypoint_path_state
            && (ai.get_mood_matrix_action_adjustment(MoodMatrixAction::Move)
                & mood_matrix_adjustment::ACTION_TO_ATTACK_MOVE)
                != 0
        {
            if let Some(owner) = self.base.get_machine_owner() {
                let _ = crate::object::registry::OBJECT_REGISTRY.with_object_mut(owner, |owner_guard| {
                    owner_guard.ai_pending_attack_follow_waypoint =
                        self.core.current_waypoint.as_ref().map(|waypoint| waypoint.id);
                    owner_guard.ai_pending_attack_follow_as_team = true;
                });
            }
        }
        let Some(owner) = self.base.get_machine_owner() else {
            return StateReturnType::Failure;
        };
        let stepped = crate::object::registry::OBJECT_REGISTRY.with_object(owner, |owner_guard| {
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
                            return Some(StateReturnType::Success);
                        }
                        if self
                            .core
                            .compute_goal(&self.base, owner_guard, ai, self.core.move_as_group)
                            .is_err()
                        {
                            return Some(StateReturnType::Failure);
                        }
                        if !self.core.has_next_waypoint()
                            && ai.is_doing_ground_movement()
                            && !ai.adjust_destination(&mut self.core.goal_position)
                        {
                            return Some(StateReturnType::Failure);
                        }
                        ai.friend_starting_move();
                        if self.core.compute_path(ai).is_err() {
                            return Some(StateReturnType::Failure);
                        }
                        if ai.is_doing_ground_movement() {
                            let _ = ai
                                .update_goal_position(&self.core.goal_position, self.core.goal_layer);
                        }
                    }
                }
            }
        }
        let frames_blocked = ai.get_num_frames_blocked();
        if ai.is_blocked_and_stuck() || frames_blocked > 2 * LOGICFRAMES_PER_SECOND {
            let _ = self.core.compute_path(ai);
        }
        let close_enough = {
            let mut __close = 5.0;
            ai.with_cur_locomotor(&mut |loco| __close = loco.get_close_enough_dist());
            __close
        };
        if ai.get_locomotor_distance_to_goal() > close_enough {
            return StateReturnType::Continue;
        }
        let prior_id = self.core.prior_waypoint.as_ref().map(|w| w.id);
        if let Some(prior) = prior_id {
            ai.set_prior_waypoint_id(prior);
        }
        let next = self.core.get_next_waypoint(&self.base);
        self.core.current_waypoint = next.clone();
            None
        });
        let Some(stepped) = stepped else {
            return StateReturnType::Failure;
        };
        if let Some(early) = stepped {
            return early;
        }
        let team = crate::object::registry::OBJECT_REGISTRY
            .with_object(owner, |owner_guard| owner_guard.get_team())
            .flatten();
        if let Some(current) = next.as_ref() {
            ai.set_current_waypoint_id(current.id);
            if let Some(team) = team {
                if let Ok(mut team_guard) = team.write() {
                    team_guard.set_current_waypoint_id(Some(current.id));
                }
            }
        }
        if next.is_none() {
            ai.set_completed_waypoint_id(prior_id);
            return StateReturnType::Success;
        }
        let Some(owner) = self.base.get_machine_owner() else {
            return StateReturnType::Failure;
        };
        let Some(result) = crate::object::registry::OBJECT_REGISTRY.with_object(owner, |owner_guard| {
        if self
            .core
            .compute_goal(&self.base, owner_guard, ai, self.core.move_as_group)
            .is_err()
        {
            return StateReturnType::Failure;
        }
        if !self.core.has_next_waypoint()
            && ai.is_doing_ground_movement()
            && !ai.adjust_destination(&mut self.core.goal_position)
        {
            return StateReturnType::Failure;
        }
        ai.friend_starting_move();
        if self.core.compute_path(ai).is_err() {
            return StateReturnType::Failure;
        }
        if ai.is_doing_ground_movement() {
            let _ = ai.update_goal_position(&self.core.goal_position, self.core.goal_layer);
        }
            StateReturnType::Continue
        }) else {
            return StateReturnType::Failure;
        };
        result
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
        self.waypoint_enter(None)
    }

    fn classic_on_enter_with_ai(
        &mut self,
        ai: &mut dyn crate::modules::AIUpdateInterface,
    ) -> Result<StateReturnType, String> {
        self.waypoint_enter(Some(ai))
    }

    fn classic_on_update(&mut self) -> Result<StateReturnType, String> {
        self.waypoint_update(None)
    }

    fn classic_on_update_with_ai(
        &mut self,
        ai: &mut dyn crate::modules::AIUpdateInterface,
    ) -> Result<StateReturnType, String> {
        self.waypoint_update(Some(ai))
    }

    fn classic_on_exit(&mut self, _exit: StateExitType) -> Result<(), String> {
        if let Some(owner) = self.base.get_machine_owner() {
            let _ = crate::object::registry::OBJECT_REGISTRY.with_object_mut(owner, |owner_guard| {
                owner_guard.ai_pending_precise_z = Some(false);
            });
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

impl AIFollowWaypointPathAsTeamState {
    fn waypoint_enter(
        &mut self,
        mut borrowed: Option<&mut dyn crate::modules::AIUpdateInterface>,
    ) -> Result<StateReturnType, String> {
        self.core.append_goal_position = false;
        self.core.prior_waypoint = None;
        self.core.frames_sleeping = 0;
        self.core.group_offset = Coord2D::new(0.0, 0.0);
        self.core.angle = 0.0;

        let waypoint_id = self.base.goal_waypoint_copied;
        self.core.current_waypoint = waypoint_id.and_then(resolve_waypoint_by_id);
        if self.core.current_waypoint.is_none() && !self.core.move_as_group {
            return Ok(StateReturnType::Failure);
        }
        let owner = self
            .base
            .get_machine_owner()
            .ok_or_else(|| "follow waypoint path missing owner".to_string())?;
        let stepped = crate::object::registry::OBJECT_REGISTRY.with_object_mut(owner, |owner_guard| -> Result<StateReturnType, String> {
        let ai_arc;
        let mut locked_ai;
        let ai_guard: &mut dyn crate::modules::AIUpdateInterface = if let Some(ai) = borrowed.as_mut()
        {
            *ai
        } else {
            ai_arc = owner_guard
                .get_ai_update_interface()
                .ok_or_else(|| "follow waypoint path missing AIUpdateInterface".to_string())?;
            locked_ai = ai_arc
                .lock()
                .map_err(|_| "follow waypoint path AI lock poisoned".to_string())?;
            &mut *locked_ai
        };
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
                let ai_store = the_ai();
                if let Ok(ai_lock) = ai_store.read() {
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
            owner_guard,
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
        });
    return match stepped {
        Some(v) => v,
        None => Err("owner missing".to_string()),
    };
}
}

impl AIFollowWaypointPathAsTeamState {
    fn waypoint_update(
        &mut self,
        mut borrowed: Option<&mut dyn crate::modules::AIUpdateInterface>,
    ) -> Result<StateReturnType, String> {
        if self.core.frames_sleeping > 0 {
            self.core.frames_sleeping = self.core.frames_sleeping.saturating_sub(1);
            return Ok(StateReturnType::Continue);
        }

        let owner = self
            .base
            .get_machine_owner()
            .ok_or_else(|| "follow waypoint path missing owner".to_string())?;
        let stepped = crate::object::registry::OBJECT_REGISTRY.with_object_mut(owner, |owner_guard| -> Result<StateReturnType, String> {
        let ai_arc;
        let mut locked_ai;
        let ai_guard: &mut dyn crate::modules::AIUpdateInterface = if let Some(ai) = borrowed.as_mut()
        {
            *ai
        } else {
            ai_arc = owner_guard
                .get_ai_update_interface()
                .ok_or_else(|| "follow waypoint path missing AIUpdateInterface".to_string())?;
            locked_ai = ai_arc
                .lock()
                .map_err(|_| "follow waypoint path AI lock poisoned".to_string())?;
            &mut *locked_ai
        };

        if self.core.current_waypoint.is_none() {
            return Ok(StateReturnType::Success);
        }

        if self.core.is_follow_waypoint_path_state {
            let adjustment = ai_guard.get_mood_matrix_action_adjustment(MoodMatrixAction::Move);
            if (adjustment & mood_matrix_adjustment::ACTION_TO_ATTACK_MOVE) != 0 {
                if let Some(current) = self.core.current_waypoint.as_ref() {
                    owner_guard.ai_pending_attack_follow_waypoint = Some(current.id);
                    owner_guard.ai_pending_attack_follow_as_team = self.core.move_as_group;
                }
            }
        }

        if self.core.append_goal_position
            && !ai_guard.is_waiting_for_path()
            && ai_guard.get_path().is_some()
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
                            .compute_goal(&self.base, owner_guard, &mut *ai_guard, false)?;
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

        let close_enough = {
            let mut __close = 5.0;
            ai_guard.with_cur_locomotor(&mut |loco| __close = loco.get_close_enough_dist());
            __close
        };

        let mut status = StateReturnType::Continue;
        if ai_guard.get_locomotor_distance_to_goal() <= close_enough {
            status = StateReturnType::Success;
        }

        if self.core.move_as_group {
            if let Some(player) = owner_guard.get_controlling_player() {
                if let Ok(player_guard) = player.read() {
                    if player_guard.is_skirmish_ai() {
                        if let Some(group_id) = owner_guard.get_group_id() {
                            let ai_store = the_ai(); if let Ok(ai_lock) = ai_store.read() {
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
                .compute_goal(&self.base, owner_guard, &mut *ai_guard, false)?;
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
        });
    return match stepped {
        Some(v) => v,
        None => Err("owner missing".to_string()),
    };
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

    fn on_enter_with_waypoint(
        &mut self,
        ai: &mut dyn crate::modules::AIUpdateInterface,
        _goal_id: crate::common::ObjectID,
        _goal_pos: Coord3D,
        waypoint: Option<crate::waypoint::WaypointId>,
    ) -> StateReturnType {
        let Some(current) = waypoint.and_then(resolve_waypoint_by_id) else {
            return StateReturnType::Failure;
        };
        let Some(owner) = self.base.get_machine_owner() else {
            return StateReturnType::Failure;
        };
        let Some(result) = crate::object::registry::OBJECT_REGISTRY.with_object(owner, |owner_guard| {
        let mut speed = FAST_AS_POSSIBLE;
        let mut group_offset = Coord2D::new(0.0, 0.0);
        if self.move_as_group {
            if let Some(group_id) = owner_guard.get_group_id() {
                if let Ok(ai_lock) = the_ai().read() {
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
        drop(owner_guard);
        let _ = ai.set_can_path_through_units(true);
        ai.set_adjusts_destination(false);
        if ai.set_path_from_waypoint(&current, &group_offset).is_err() {
            return StateReturnType::Failure;
        }
        let _ = ai.set_allow_invalid_position(true);
        ai.set_desired_speed(speed);
        self.last_waypoint = Some(current);
        StateReturnType::Continue
        }) else {
        return StateReturnType::Failure;
    };
    result
}

    fn update(&mut self) -> StateReturnType {
        self.classic_on_update().unwrap_or(StateReturnType::Failure)
    }

    fn update_with_ai(
        &mut self,
        _ai: &mut dyn crate::modules::AIUpdateInterface,
    ) -> StateReturnType {
        let Some(owner) = self.base.get_machine_owner() else {
            return StateReturnType::Failure;
        };
        let Some(result) = crate::object::registry::OBJECT_REGISTRY.with_object_mut(owner, |owner_guard| {
        owner_guard.ai_pending_path_through_units = Some(true);
        if !owner_guard.ai_fire_is_moving && owner_guard.ai_fire_waypoint_queue_empty {
            return StateReturnType::Success;
        }
        StateReturnType::Continue
        }) else {
        return StateReturnType::Failure;
    };
    result
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
        self.exact_team_enter(None)
    }

    fn classic_on_enter_with_ai(
        &mut self,
        ai: &mut dyn crate::modules::AIUpdateInterface,
    ) -> Result<StateReturnType, String> {
        self.exact_team_enter(Some(ai))
    }

    fn classic_on_update(&mut self) -> Result<StateReturnType, String> {
        let owner = self
            .base
            .get_machine_owner()
            .ok_or_else(|| "follow waypoint exact missing owner".to_string())?;
        let stepped = crate::object::registry::OBJECT_REGISTRY.with_object_mut(owner, |owner_guard| -> Result<StateReturnType, String> {
        owner_guard.ai_pending_path_through_units = Some(true);
        if !owner_guard.ai_fire_is_moving && owner_guard.ai_fire_waypoint_queue_empty {
            return Ok(StateReturnType::Success);
        }

        Ok(StateReturnType::Continue)
        });
    return match stepped {
        Some(v) => v,
        None => Err("owner missing".to_string()),
    };
}

    fn classic_on_exit(&mut self, _exit: StateExitType) -> Result<(), String> {
        if let Some(owner) = self.base.get_machine_owner() {
            let _ = crate::object::registry::OBJECT_REGISTRY.with_object_mut(owner, |owner_guard| {
                if let Some(last) = self.last_waypoint.as_ref() {
                    owner_guard.ai_pending_completed_waypoint = Some(last.id);
                }
                owner_guard.ai_pending_path_through_units = Some(false);
                owner_guard.ai_pending_allow_invalid_position = Some(false);
            });
        }
        Ok(())
    }

    fn classic_is_busy(&self) -> bool {
        true
    }
}

impl AIFollowWaypointPathAsTeamExactState {
    fn exact_team_enter(
        &mut self,
        mut borrowed: Option<&mut dyn crate::modules::AIUpdateInterface>,
    ) -> Result<StateReturnType, String> {
        let waypoint_id = self.base.goal_waypoint_copied;
        let current = waypoint_id
            .and_then(resolve_waypoint_by_id)
            .ok_or_else(|| "follow waypoint exact missing waypoint".to_string())?;
        let owner = self
            .base
            .get_machine_owner()
            .ok_or_else(|| "follow waypoint exact missing owner".to_string())?;
        let stepped = crate::object::registry::OBJECT_REGISTRY.with_object_mut(owner, |owner_guard| -> Result<StateReturnType, String> {
        let ai_arc;
        let mut locked_ai;
        let ai_guard: &mut dyn crate::modules::AIUpdateInterface = if let Some(ai) = borrowed.as_mut()
        {
            *ai
        } else {
            ai_arc = owner_guard
                .get_ai_update_interface()
                .ok_or_else(|| "follow waypoint exact missing AIUpdateInterface".to_string())?;
            locked_ai = ai_arc
                .lock()
                .map_err(|_| "follow waypoint exact AI lock poisoned".to_string())?;
            &mut *locked_ai
        };
        let mut speed = FAST_AS_POSSIBLE;
        let mut group_offset = Coord2D::new(0.0, 0.0);
        if self.move_as_group {
            if let Some(group_id) = owner_guard.get_group_id() {
                let ai_store = the_ai();
                if let Ok(ai_lock) = ai_store.read() {
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
        });
    return match stepped {
        Some(v) => v,
        None => Err("owner missing".to_string()),
    };
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

    fn on_enter_with_waypoint(
        &mut self,
        ai: &mut dyn crate::modules::AIUpdateInterface,
        _goal_id: crate::common::ObjectID,
        _goal_pos: Coord3D,
        waypoint: Option<crate::waypoint::WaypointId>,
    ) -> StateReturnType {
        self.core.append_goal_position = false;
        self.core.prior_waypoint = None;
        self.core.frames_sleeping = 0;
        self.core.group_offset = Coord2D::new(0.0, 0.0);
        self.core.angle = 0.0;
        self.core.current_waypoint = waypoint.and_then(resolve_waypoint_by_id);
        if self.core.current_waypoint.is_none() && !self.core.move_as_group {
            return StateReturnType::Failure;
        }
        let Some(owner) = self.base.get_machine_owner() else {
            return StateReturnType::Failure;
        };
        let Some(result) = crate::object::registry::OBJECT_REGISTRY.with_object(owner, |owner_guard| {
        if self
            .core
            .compute_goal(&self.base, owner_guard, ai, false)
            .is_err()
        {
            return StateReturnType::Failure;
        }
        if !self.core.has_next_waypoint()
            && ai.is_doing_ground_movement()
            && !ai.adjust_destination(&mut self.core.goal_position)
        {
            return StateReturnType::Failure;
        }
        if self.core.compute_path(ai).is_err() {
            return StateReturnType::Failure;
        }
        ai.set_desired_speed(FAST_AS_POSSIBLE);
        if ai
            .set_path_extra_distance(self.core.calc_extra_path_distance())
            .is_err()
        {
            return StateReturnType::Failure;
        }
        if ai.is_doing_ground_movement() {
            let _ = ai.update_goal_position(&self.core.goal_position, self.core.goal_layer);
        }
        StateReturnType::Continue
        }) else {
        return StateReturnType::Failure;
    };
    result
}

    fn update(&mut self) -> StateReturnType {
        self.classic_on_update().unwrap_or(StateReturnType::Failure)
    }

    fn update_with_ai(
        &mut self,
        ai: &mut dyn crate::modules::AIUpdateInterface,
    ) -> StateReturnType {
        if self.core.frames_sleeping > 0 {
            self.core.frames_sleeping = self.core.frames_sleeping.saturating_sub(1);
            return StateReturnType::Continue;
        }
        if self.core.current_waypoint.is_none() {
            return StateReturnType::Success;
        }
        if self.core.is_follow_waypoint_path_state
            && (ai.get_mood_matrix_action_adjustment(MoodMatrixAction::Move)
                & mood_matrix_adjustment::ACTION_TO_ATTACK_MOVE)
                != 0
        {
            if let Some(owner) = self.base.get_machine_owner() {
                let _ = crate::object::registry::OBJECT_REGISTRY.with_object_mut(owner, |owner_guard| {
                    owner_guard.ai_pending_attack_follow_waypoint =
                        self.core.current_waypoint.as_ref().map(|waypoint| waypoint.id);
                    owner_guard.ai_pending_attack_follow_as_team = self.core.move_as_group;
                });
            }
        }
        let Some(owner) = self.base.get_machine_owner() else {
            return StateReturnType::Failure;
        };
        let Some(result) = crate::object::registry::OBJECT_REGISTRY.with_object(owner, |owner_guard| {
        if self.core.append_goal_position && !ai.is_waiting_for_path() && ai.get_path().is_some() {
            if ai
                .append_goal_position_to_path(&self.core.goal_position)
                .is_err()
            {
                return StateReturnType::Failure;
            }
            self.core.append_goal_position = false;
        }
        let frames_blocked = ai.get_num_frames_blocked();
        let blocked = ai.is_blocked_and_stuck() || frames_blocked > 2 * LOGICFRAMES_PER_SECOND;
        if blocked {
            let _ = self.core.compute_path(ai);
        }
        let close_enough = {
            let mut __close = 5.0;
            ai.with_cur_locomotor(&mut |loco| __close = loco.get_close_enough_dist());
            __close
        };
        if ai.get_locomotor_distance_to_goal() > close_enough {
            return StateReturnType::Continue;
        }
        let prior_id = self.core.prior_waypoint.as_ref().map(|w| w.id);
        if let Some(prior) = prior_id {
            ai.set_prior_waypoint_id(prior);
        }
        let next = self.core.get_next_waypoint(&self.base);
        self.core.current_waypoint = next.clone();
        if let Some(current) = next.as_ref() {
            ai.set_current_waypoint_id(current.id);
        }
        if next.is_none() {
            ai.set_completed_waypoint_id(prior_id);
            return StateReturnType::Success;
        }
        if self
            .core
            .compute_goal(&self.base, owner_guard, ai, false)
            .is_err()
        {
            return StateReturnType::Failure;
        }
        if !self.core.has_next_waypoint()
            && ai.is_doing_ground_movement()
            && !ai.adjust_destination(&mut self.core.goal_position)
        {
            return StateReturnType::Failure;
        }
        ai.friend_starting_move();
        if self.core.compute_path(ai).is_err() {
            return StateReturnType::Failure;
        }
        if ai.is_doing_ground_movement() {
            let _ = ai.update_goal_position(&self.core.goal_position, self.core.goal_layer);
        }
        StateReturnType::Continue
        }) else {
        return StateReturnType::Failure;
    };
    result
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
        self.individuals_enter(None)
    }

    fn classic_on_enter_with_ai(
        &mut self,
        ai: &mut dyn crate::modules::AIUpdateInterface,
    ) -> Result<StateReturnType, String> {
        self.individuals_enter(Some(ai))
    }

    fn classic_on_update(&mut self) -> Result<StateReturnType, String> {
        self.individuals_update(None)
    }

    fn classic_on_update_with_ai(
        &mut self,
        ai: &mut dyn crate::modules::AIUpdateInterface,
    ) -> Result<StateReturnType, String> {
        self.individuals_update(Some(ai))
    }

    fn classic_on_exit(&mut self, _exit: StateExitType) -> Result<(), String> {
        if let Some(owner) = self.base.get_machine_owner() {
            let _ = crate::object::registry::OBJECT_REGISTRY.with_object_mut(owner, |owner_guard| {
                owner_guard.ai_pending_precise_z = Some(false);
            });
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

impl AIFollowWaypointPathAsIndividualsState {
    fn individuals_enter(
        &mut self,
        mut borrowed: Option<&mut dyn crate::modules::AIUpdateInterface>,
    ) -> Result<StateReturnType, String> {
        self.core.append_goal_position = false;
        self.core.prior_waypoint = None;
        self.core.frames_sleeping = 0;
        self.core.group_offset = Coord2D::new(0.0, 0.0);
        self.core.angle = 0.0;
        let waypoint_id = self.base.goal_waypoint_copied;
        self.core.current_waypoint = waypoint_id.and_then(resolve_waypoint_by_id);
        if self.core.current_waypoint.is_none() && !self.core.move_as_group {
            return Ok(StateReturnType::Failure);
        }
        let owner = self
            .base
            .get_machine_owner()
            .ok_or_else(|| "follow waypoint path missing owner".to_string())?;
        let stepped = crate::object::registry::OBJECT_REGISTRY.with_object_mut(owner, |owner_guard| -> Result<StateReturnType, String> {
        let ai_arc;
        let mut locked_ai;
        let ai_guard: &mut dyn crate::modules::AIUpdateInterface = if let Some(ai) = borrowed.as_mut()
        {
            *ai
        } else {
            ai_arc = owner_guard
                .get_ai_update_interface()
                .ok_or_else(|| "follow waypoint path missing AIUpdateInterface".to_string())?;
            locked_ai = ai_arc
                .lock()
                .map_err(|_| "follow waypoint path AI lock poisoned".to_string())?;
            &mut *locked_ai
        };
        self.core
            .compute_goal(&self.base, owner_guard, &mut *ai_guard, false)?;
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
        });
    return match stepped {
        Some(v) => v,
        None => Err("owner missing".to_string()),
    };
}

    fn individuals_update(
        &mut self,
        mut borrowed: Option<&mut dyn crate::modules::AIUpdateInterface>,
    ) -> Result<StateReturnType, String> {
        if self.core.frames_sleeping > 0 {
            self.core.frames_sleeping = self.core.frames_sleeping.saturating_sub(1);
            return Ok(StateReturnType::Continue);
        }
        let owner = self
            .base
            .get_machine_owner()
            .ok_or_else(|| "follow waypoint path missing owner".to_string())?;
        let stepped = crate::object::registry::OBJECT_REGISTRY.with_object_mut(owner, |owner_guard| -> Result<StateReturnType, String> {
        let ai_arc;
        let mut locked_ai;
        let ai_guard: &mut dyn crate::modules::AIUpdateInterface = if let Some(ai) = borrowed.as_mut()
        {
            *ai
        } else {
            ai_arc = owner_guard
                .get_ai_update_interface()
                .ok_or_else(|| "follow waypoint path missing AIUpdateInterface".to_string())?;
            locked_ai = ai_arc
                .lock()
                .map_err(|_| "follow waypoint path AI lock poisoned".to_string())?;
            &mut *locked_ai
        };
        if self.core.current_waypoint.is_none() {
            return Ok(StateReturnType::Success);
        }
        if self.core.is_follow_waypoint_path_state {
            let adjustment = ai_guard.get_mood_matrix_action_adjustment(MoodMatrixAction::Move);
            if (adjustment & mood_matrix_adjustment::ACTION_TO_ATTACK_MOVE) != 0 {
                if let Some(current) = self.core.current_waypoint.as_ref() {
                    owner_guard.ai_pending_attack_follow_waypoint = Some(current.id);
                }
            }
        }
        if self.core.append_goal_position
            && !ai_guard.is_waiting_for_path()
            && ai_guard.get_path().is_some()
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
        let close_enough = {
            let mut __close = 5.0;
            ai_guard.with_cur_locomotor(&mut |loco| __close = loco.get_close_enough_dist());
            __close
        };
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
                .compute_goal(&self.base, owner_guard, &mut *ai_guard, false)?;
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
        });
    return match stepped {
        Some(v) => v,
        None => Err("owner missing".to_string()),
    };
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

    fn on_enter_with_waypoint(
        &mut self,
        ai: &mut dyn crate::modules::AIUpdateInterface,
        _goal_id: crate::common::ObjectID,
        _goal_pos: Coord3D,
        waypoint: Option<crate::waypoint::WaypointId>,
    ) -> StateReturnType {
        let Some(current) = waypoint.and_then(resolve_waypoint_by_id) else {
            return StateReturnType::Failure;
        };
        let _ = ai.set_can_path_through_units(true);
        ai.set_adjusts_destination(false);
        if ai
            .set_path_from_waypoint(&current, &Coord2D::new(0.0, 0.0))
            .is_err()
        {
            return StateReturnType::Failure;
        }
        let _ = ai.set_allow_invalid_position(true);
        ai.set_desired_speed(FAST_AS_POSSIBLE);
        self.last_waypoint = Some(current);
        StateReturnType::Continue
    }

    fn update(&mut self) -> StateReturnType {
        self.classic_on_update().unwrap_or(StateReturnType::Failure)
    }

    fn update_with_ai(
        &mut self,
        _ai: &mut dyn crate::modules::AIUpdateInterface,
    ) -> StateReturnType {
        let Some(owner) = self.base.get_machine_owner() else {
            return StateReturnType::Failure;
        };
        let Some(result) = crate::object::registry::OBJECT_REGISTRY.with_object_mut(owner, |owner_guard| {
        owner_guard.ai_pending_path_through_units = Some(true);
        if !owner_guard.ai_fire_is_moving && owner_guard.ai_fire_waypoint_queue_empty {
            return StateReturnType::Success;
        }
        StateReturnType::Continue
        }) else {
        return StateReturnType::Failure;
    };
    result
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
        self.exact_individuals_enter(None)
    }

    fn classic_on_enter_with_ai(
        &mut self,
        ai: &mut dyn crate::modules::AIUpdateInterface,
    ) -> Result<StateReturnType, String> {
        self.exact_individuals_enter(Some(ai))
    }

    fn classic_on_update(&mut self) -> Result<StateReturnType, String> {
        let owner = self
            .base
            .get_machine_owner()
            .ok_or_else(|| "follow waypoint exact missing owner".to_string())?;
        let stepped = crate::object::registry::OBJECT_REGISTRY.with_object_mut(owner, |owner_guard| -> Result<StateReturnType, String> {
        owner_guard.ai_pending_path_through_units = Some(true);
        if !owner_guard.ai_fire_is_moving && owner_guard.ai_fire_waypoint_queue_empty {
            return Ok(StateReturnType::Success);
        }

        Ok(StateReturnType::Continue)
        });
    return match stepped {
        Some(v) => v,
        None => Err("owner missing".to_string()),
    };
}

    fn classic_on_exit(&mut self, _exit: StateExitType) -> Result<(), String> {
        if let Some(owner) = self.base.get_machine_owner() {
            let _ = crate::object::registry::OBJECT_REGISTRY.with_object_mut(owner, |owner_guard| {
                if let Some(last) = self.last_waypoint.as_ref() {
                    owner_guard.ai_pending_completed_waypoint = Some(last.id);
                }
                owner_guard.ai_pending_path_through_units = Some(false);
                owner_guard.ai_pending_allow_invalid_position = Some(false);
            });
        }
        Ok(())
    }

    fn classic_is_busy(&self) -> bool {
        true
    }
}

impl AIFollowWaypointPathAsIndividualsExactState {
    fn exact_individuals_enter(
        &mut self,
        mut borrowed: Option<&mut dyn crate::modules::AIUpdateInterface>,
    ) -> Result<StateReturnType, String> {
        let waypoint_id = self.base.goal_waypoint_copied;
        let current = waypoint_id
            .and_then(resolve_waypoint_by_id)
            .ok_or_else(|| "follow waypoint exact missing waypoint".to_string())?;
        let owner = self
            .base
            .get_machine_owner()
            .ok_or_else(|| "follow waypoint exact missing owner".to_string())?;
        let stepped = crate::object::registry::OBJECT_REGISTRY.with_object_mut(owner, |owner_guard| -> Result<StateReturnType, String> {
        let ai_arc;
        let mut locked_ai;
        let ai_guard: &mut dyn crate::modules::AIUpdateInterface = if let Some(ai) = borrowed.as_mut()
        {
            *ai
        } else {
            ai_arc = owner_guard
                .get_ai_update_interface()
                .ok_or_else(|| "follow waypoint exact missing AIUpdateInterface".to_string())?;
            locked_ai = ai_arc
                .lock()
                .map_err(|_| "follow waypoint exact AI lock poisoned".to_string())?;
            &mut *locked_ai
        };
        let group_offset = Coord2D::new(0.0, 0.0);
        let _ = ai_guard.set_can_path_through_units(true);
        ai_guard.set_adjusts_destination(false);
        ai_guard.set_path_from_waypoint(&current, &group_offset)?;
        let _ = ai_guard.set_allow_invalid_position(true);
        ai_guard.set_desired_speed(FAST_AS_POSSIBLE);
        self.last_waypoint = Some(current);
        Ok(StateReturnType::Continue)
        });
    return match stepped {
        Some(v) => v,
        None => Err("owner missing".to_string()),
    };
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
