#![allow(deprecated, unused_imports, dead_code)]

use super::attack::*;
use super::attack_machine::*;
use super::dead::*;
use super::dock::*;
use super::enter::*;
use super::face::*;
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

pub(crate) enum FollowPathStateKindMut<'a> {
    FollowPath(&'a mut AIFollowPathState),
    FollowExitProductionPath(&'a mut AIFollowExitProductionPathState),
}

impl<'a> FollowPathStateKindMut<'a> {
    pub(crate) fn set_path(self, path: Vec<Coord3D>, ignore_object: Option<ObjectID>) {
        match self {
            Self::FollowPath(state) => {
                state.set_path(path);
                state.set_ignore_object_id(ignore_object);
            }
            Self::FollowExitProductionPath(state) => {
                state.set_path(path);
                state.base.set_ignore_object_id(ignore_object);
            }
        }
    }

    pub(crate) fn append_path(self, position: Coord3D) {
        match self {
            Self::FollowPath(state) => state.append_path(position),
            Self::FollowExitProductionPath(state) => state.append_path(position),
        }
    }
}

pub(crate) fn state_follow_path_kind(
    state: &mut dyn StateImplementation,
) -> Option<FollowPathStateKindMut<'_>> {
    let any = state as &mut dyn std::any::Any;
    if any.is::<AIFollowExitProductionPathState>() {
        let state = any
            .downcast_mut::<AIFollowExitProductionPathState>()
            .expect("type check and downcast must match");
        return Some(FollowPathStateKindMut::FollowExitProductionPath(state));
    }
    if any.is::<AIFollowPathState>() {
        let state = any
            .downcast_mut::<AIFollowPathState>()
            .expect("type check and downcast must match");
        return Some(FollowPathStateKindMut::FollowPath(state));
    }

    None
}

/// Follow path state
/// Matches C++ AIFollowPathState from AIStates.cpp lines 3229-3389.
#[derive(Debug)]
pub struct AIFollowPathState {
    pub(crate) base: AIMoveToState,
    pub(crate) path: Vec<Coord3D>,
    pub(crate) index: usize,
    pub(crate) adjust_final: bool,
    pub(crate) adjust_final_override: bool,
    pub(crate) retry_count: i32,
    pub(crate) follow_exit_production: bool,
    pub(crate) ignore_object_id: Option<ObjectID>,
}

impl AIFollowPathState {
    pub fn new(machine: &StateMachine, follow_exit_production: bool) -> Self {
        let mut base = AIMoveToState::new(machine);
        base.base.name = if follow_exit_production {
            "AIFollowExitProductionPath".to_string()
        } else {
            "AIFollowPath".to_string()
        };
        Self {
            base,
            path: Vec::new(),
            index: 0,
            adjust_final: true,
            adjust_final_override: true,
            retry_count: 0,
            follow_exit_production,
            ignore_object_id: None,
        }
    }

    pub fn set_path(&mut self, path: Vec<Coord3D>) {
        self.path = path;
        self.index = 0;
    }

    pub fn append_path(&mut self, pos: Coord3D) {
        self.path.push(pos);
    }

    pub fn set_ignore_object_id(&mut self, object_id: Option<ObjectID>) {
        self.ignore_object_id = object_id;
    }

    pub(crate) fn set_goal_position(&mut self, pos: Coord3D) {
        self.base.goal_position = pos;
        if let Ok(machine) = self.base.base.get_machine() {
            if let Ok(mut guard) = machine.lock() {
                guard.set_goal_position(pos);
            }
        }
    }

    pub(crate) fn configure_segment(
        &mut self,
        owner_guard: &Object,
        ai_guard: &mut dyn AIUpdateInterface,
        allow_adjust: bool,
    ) -> Result<(), String> {
        let next_pos = self.path.get(self.index + 1).copied();
        if let Some(next) = next_pos {
            let dx = next.x - self.base.goal_position.x;
            let dy = next.y - self.base.goal_position.y;
            let mut offset = (dx * dx + dy * dy).sqrt();
            if self.path.get(self.index + 2).is_some() {
                offset += 4.0 * PATHFIND_CELL_SIZE_F;
            }
            ai_guard
                .set_path_extra_distance(offset)
                .map_err(|e| format!("follow path set_path_extra_distance failed: {}", e))?;
            self.base.set_adjusts_destination(false);
        } else {
            let adjust_final = self.adjust_final
                && (self.adjust_final_override || ai_guard.is_doing_ground_movement());
            self.base.set_adjusts_destination(adjust_final);
            let _ = ai_guard.set_path_extra_distance(0.0);
            if allow_adjust && self.base.adjust_destinations {
                let mut adjusted_goal = self.base.goal_position;
                if !ai_guard.adjust_destination(&mut adjusted_goal) {
                    return Err("follow path failed to adjust destination".to_string());
                }
                self.set_goal_position(adjusted_goal);
                let _ = ai_guard.update_goal_position(&adjusted_goal, PathfindLayerEnum::Ground);
            }
            if owner_guard.is_kind_of(KindOf::Projectile) {
                let _ = ai_guard.set_precise_z_pos(true);
            }
        }
        Ok(())
    }
}

impl StateImplementation for AIFollowPathState {
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

impl ClassicState for AIFollowPathState {
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
        if self.path.is_empty() {
            return Ok(StateReturnType::Failure);
        }
        self.index = 0;
        self.adjust_final = true;
        self.adjust_final_override = true;

        let owner = self
            .base
            .base
            .get_machine_owner()
            .ok_or_else(|| "follow path missing owner".to_string())?;
        {
            let owner_guard = owner
                .read()
                .map_err(|_| "follow path owner lock poisoned".to_string())?;
            let ai = owner_guard
                .get_ai_update_interface()
                .ok_or_else(|| "follow path missing AIUpdateInterface".to_string())?;
            let mut ai_guard = ai
                .lock()
                .map_err(|_| "follow path AI lock poisoned".to_string())?;

            self.set_goal_position(self.path[0]);
            if let Some(ignore_id) = self.ignore_object_id {
                let _ = ai_guard.ignore_obstacle_id(ignore_id);
            }
            let _ = ai_guard.set_current_goal_path_index(self.index as i32);
            if self.follow_exit_production {
                let _ = ai_guard.set_can_path_through_units(true);
                self.base.set_adjusts_destination(false);
            }
        }

        let status = self.base.classic_on_enter()?;
        if let Ok(owner_guard) = owner.read() {
            if owner_guard.get_formation_id() != FormationID::NONE {
                if let Some(group_id) = owner_guard.get_group_id() {
                    if let Ok(ai_lock) = THE_AI.read() {
                        if let Some(group) = ai_lock.find_group(group_id) {
                            if let Ok(mut group_guard) = group.write() {
                                if let Some(ai) = owner_guard.get_ai_update_interface() {
                                    if let Ok(mut ai_guard) = ai.lock() {
                                        ai_guard.set_desired_speed(group_guard.get_speed());
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        if let Ok(owner_guard) = owner.read() {
            if let Some(ai) = owner_guard.get_ai_update_interface() {
                if let Ok(mut ai_guard) = ai.lock() {
                    self.configure_segment(&owner_guard, &mut *ai_guard, false)?;
                }
            }
        }
        Ok(status)
    }

    fn classic_on_update(&mut self) -> Result<StateReturnType, String> {
        if let Ok(machine) = self.base.base.get_machine() {
            if let Ok(mut guard) = machine.lock() {
                guard.set_goal_position(self.base.goal_position);
            }
        }
        let status = self.base.classic_on_update()?;

        if status == StateReturnType::Continue {
            return Ok(status);
        }

        let owner = self
            .base
            .base
            .get_machine_owner()
            .ok_or_else(|| "follow path missing owner".to_string())?;
        let owner_guard = owner
            .read()
            .map_err(|_| "follow path owner lock poisoned".to_string())?;
        let ai = owner_guard
            .get_ai_update_interface()
            .ok_or_else(|| "follow path missing AIUpdateInterface".to_string())?;
        let mut ai_guard = ai
            .lock()
            .map_err(|_| "follow path AI lock poisoned".to_string())?;

        if status == StateReturnType::Failure && self.retry_count > 0 {
            self.retry_count -= 1;
        } else {
            self.index = self.index.saturating_add(1);
        }

        while self.index < self.path.len() {
            let pos = self.path[self.index];
            let dx = pos.x - owner_guard.get_position().x;
            let dy = pos.y - owner_guard.get_position().y;
            if dx * dx + dy * dy >= PATHFIND_CELL_SIZE_F * PATHFIND_CELL_SIZE_F {
                break;
            }
            self.index = self.index.saturating_add(1);
        }

        let Some(pos) = self.path.get(self.index).copied() else {
            return Ok(StateReturnType::Success);
        };

        let _ = ai_guard.set_current_goal_path_index(self.index as i32);
        let _ = ai_guard.ignore_obstacle(None);
        ai_guard.friend_starting_move();

        self.set_goal_position(pos);
        self.configure_segment(&owner_guard, &mut *ai_guard, true)?;
        self.base.compute_path(&mut *ai_guard)?;
        Ok(StateReturnType::Continue)
    }

    fn classic_on_exit(&mut self, _exit: StateExitType) -> Result<(), String> {
        self.base.classic_on_exit(_exit)?;
        if let Some(owner) = self.base.base.get_machine_owner() {
            if let Ok(owner_guard) = owner.read() {
                if let Some(ai) = owner_guard.get_ai_update_interface() {
                    if let Ok(mut ai_guard) = ai.lock() {
                        let _ = ai_guard.set_can_path_through_units(false);
                        let _ = ai_guard.set_precise_z_pos(false);
                        let _ = ai_guard.set_path_extra_distance(0.0);
                        let _ = ai_guard.set_current_goal_path_index(-1);
                    }
                }
            }
        }
        Ok(())
    }
}

/// Follow exit-production path state
#[derive(Debug)]
pub struct AIFollowExitProductionPathState {
    pub(crate) base: AIFollowPathState,
}

impl AIFollowExitProductionPathState {
    pub fn new(machine: &StateMachine) -> Self {
        Self {
            base: AIFollowPathState::new(machine, true),
        }
    }

    pub fn set_path(&mut self, path: Vec<Coord3D>) {
        self.base.set_path(path);
    }

    pub fn append_path(&mut self, pos: Coord3D) {
        self.base.append_path(pos);
    }
}

impl StateImplementation for AIFollowExitProductionPathState {
    fn on_enter(&mut self) -> StateReturnType {
        self.base.on_enter()
    }

    fn update(&mut self) -> StateReturnType {
        self.base.update()
    }

    fn on_exit(&mut self, status: StateExitType) {
        self.base.on_exit(status);
    }

    fn xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.base.xfer_snapshot(xfer)
    }
}

impl ClassicState for AIFollowExitProductionPathState {
    fn base_state(&self) -> &State {
        self.base.base_state()
    }

    fn base_state_mut(&mut self) -> &mut State {
        self.base.base_state_mut()
    }

    fn classic_xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.base.xfer_snapshot(xfer)
    }

    fn classic_on_enter(&mut self) -> Result<StateReturnType, String> {
        self.base.classic_on_enter()
    }

    fn classic_on_update(&mut self) -> Result<StateReturnType, String> {
        self.base.classic_on_update()
    }

    fn classic_on_exit(&mut self, exit: StateExitType) -> Result<(), String> {
        self.base.classic_on_exit(exit)
    }
}

/// Follow state - move toward and track the goal object.
/// PARITY_NOTE: C++ does not have a standalone AIFollowState. C++ uses
/// AIAttackAndFollowObject (attack+follow) or AIFollowPathState (path following).
/// This state provides pure follow behavior: move toward goal object's position,
/// re-issueing move commands as the target moves. No attack logic is included.
/// This is used for formation following, escort, and group movement scenarios.
#[derive(Debug)]
pub struct AIFollowState {
    pub(crate) base: State,
    /// The object we are following
    pub(crate) target_id: ObjectID,
    /// Whether a move command has been issued this frame
    pub(crate) issued_move: bool,
    /// Distance threshold to consider "close enough" to the target
    pub(crate) follow_distance: Real,
    /// Last known target position (for detecting target movement)
    pub(crate) last_target_pos: Coord3D,
}

impl AIFollowState {
    pub fn new(machine: &StateMachine) -> Self {
        Self {
            base: State::new(machine, "AIFollow"),
            target_id: INVALID_ID,
            issued_move: false,
            follow_distance: 3.0 * PATHFIND_CELL_SIZE_F,
            last_target_pos: Coord3D::new(0.0, 0.0, 0.0),
        }
    }

    pub fn set_follow_distance(&mut self, distance: Real) {
        self.follow_distance = distance;
    }
}

impl StateImplementation for AIFollowState {
    fn on_enter(&mut self) -> StateReturnType {
        self.classic_on_enter().unwrap_or(StateReturnType::Failure)
    }

    fn update(&mut self) -> StateReturnType {
        self.classic_on_update().unwrap_or(StateReturnType::Failure)
    }

    fn on_exit(&mut self, _status: StateExitType) {
        let _ = self.classic_on_exit(_status);
    }
}

impl ClassicState for AIFollowState {
    fn base_state(&self) -> &State {
        &self.base
    }

    fn base_state_mut(&mut self) -> &mut State {
        &mut self.base
    }

    fn classic_on_enter(&mut self) -> Result<StateReturnType, String> {
        // Wave 257: empty dual-world → fail-closed state.
        if dual_world_registry_unavailable() {
            return Ok(StateReturnType::Failure);
        }

        let target_id = self
            .base
            .get_machine_goal_object_id()
            .ok_or_else(|| "AIFollow state missing goal object".to_string())?;
        self.target_id = target_id;
        if let Some(pos) = crate::object::registry::OBJECT_REGISTRY
            .with_object(target_id, |guard| *guard.get_position())
        {
            self.last_target_pos = pos;
        }

        self.issued_move = false;

        Ok(StateReturnType::Continue)
    }

    fn classic_on_update(&mut self) -> Result<StateReturnType, String> {
        // Wave 257: empty dual-world → fail-closed state.
        if dual_world_registry_unavailable() {
            return Ok(StateReturnType::Failure);
        }

        if self.target_id == INVALID_ID {
            return Ok(StateReturnType::Failure);
        }
        let Some(target) = TheGameLogic::find_object_by_id(self.target_id)
            .or_else(|| OBJECT_REGISTRY.get_object(self.target_id))
        else {
            return Ok(StateReturnType::Failure);
        };

        let target_pos = {
            let Ok(target_guard) = target.read() else {
                return Ok(StateReturnType::Failure);
            };
            if target_guard.is_effectively_dead() {
                return Ok(StateReturnType::Failure);
            }
            *target_guard.get_position()
        };

        let owner = self
            .base
            .get_machine_owner()
            .ok_or_else(|| "AIFollow state missing machine owner".to_string())?;

        let owner_pos = {
            let Ok(owner_guard) = owner.read() else {
                return Ok(StateReturnType::Failure);
            };
            *owner_guard.get_position()
        };

        let dx = target_pos.x - owner_pos.x;
        let dy = target_pos.y - owner_pos.y;
        let dist = (dx * dx + dy * dy).sqrt();

        let tdx = target_pos.x - self.last_target_pos.x;
        let tdy = target_pos.y - self.last_target_pos.y;
        let target_moved = (tdx * tdx + tdy * tdy).sqrt() > PATHFIND_CELL_SIZE_F;

        if dist <= self.follow_distance && !target_moved {
            self.issued_move = false;
            return Ok(StateReturnType::Continue);
        }

        self.last_target_pos = target_pos;

        if !self.issued_move || target_moved {
            if let Ok(owner_guard) = owner.read() {
                if let Some(ai) = owner_guard.get_ai_update_interface() {
                    if let Ok(mut ai_guard) = ai.lock() {
                        let mut params = AiCommandParams::new(
                            crate::ai::AiCommandType::MoveToPosition,
                            crate::ai::CommandSourceType::FromAi,
                        );
                        params.pos = target_pos;
                        let _ = ai_guard.execute_command(&params);
                        self.issued_move = true;
                    }
                }
            }
        }

        Ok(StateReturnType::Continue)
    }

    fn classic_on_exit(&mut self, _exit: StateExitType) -> Result<(), String> {
        self.target_id = INVALID_ID;
        self.issued_move = false;
        Ok(())
    }
}

impl Snapshotable for AIFollowPathState {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: game_engine::common::system::xfer::XferVersion = 2;
        xfer.xfer_version(&mut version, 2)
            .map_err(|e| format!("Failed to crc version: {:?}", e))?;

        let mut index = self.index as i32;
        xfer.xfer_int(&mut index)
            .map_err(|e| format!("Failed to crc follow path index: {:?}", e))?;

        let mut adjust_final = self.adjust_final;
        xfer.xfer_bool(&mut adjust_final)
            .map_err(|e| format!("Failed to crc follow path adjust_final: {:?}", e))?;
        let mut adjust_final_override = self.adjust_final_override;
        xfer.xfer_bool(&mut adjust_final_override)
            .map_err(|e| format!("Failed to crc follow path adjust_final_override: {:?}", e))?;
        let mut retry_count = self.retry_count;
        xfer.xfer_int(&mut retry_count)
            .map_err(|e| format!("Failed to crc follow path retry_count: {:?}", e))?;

        let mut path_len = self.path.len() as i32;
        xfer.xfer_int(&mut path_len)
            .map_err(|e| format!("Failed to crc follow path length: {:?}", e))?;
        for idx in 0..path_len.max(0) {
            let mut pos = self
                .path
                .get(idx as usize)
                .copied()
                .unwrap_or_else(|| Coord3D::new(0.0, 0.0, 0.0));
            xfer.xfer_coord3d(&mut pos);
        }

        if version >= 2 {
            let mut has_ignore_object = self.ignore_object_id.is_some();
            xfer.xfer_bool(&mut has_ignore_object)
                .map_err(|e| format!("Failed to crc follow path has_ignore_object: {:?}", e))?;
            let mut ignore_object_id = self.ignore_object_id.unwrap_or(crate::common::INVALID_ID);
            xfer.xfer_object_id(&mut ignore_object_id)
                .map_err(|e| format!("Failed to crc follow path ignore_object_id: {:?}", e))?;
        }

        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: game_engine::common::system::xfer::XferVersion = 2;
        xfer.xfer_version(&mut version, 2)
            .map_err(|e| format!("Failed to xfer version: {:?}", e))?;

        let mut index = self.index as i32;
        xfer.xfer_int(&mut index)
            .map_err(|e| format!("Failed to xfer follow path index: {:?}", e))?;
        if xfer.is_loading() {
            self.index = index.max(0) as usize;
        }

        xfer.xfer_bool(&mut self.adjust_final)
            .map_err(|e| format!("Failed to xfer follow path adjust_final: {:?}", e))?;
        xfer.xfer_bool(&mut self.adjust_final_override)
            .map_err(|e| format!("Failed to xfer follow path adjust_final_override: {:?}", e))?;
        xfer.xfer_int(&mut self.retry_count)
            .map_err(|e| format!("Failed to xfer follow path retry_count: {:?}", e))?;

        let mut path_len = self.path.len() as i32;
        xfer.xfer_int(&mut path_len)
            .map_err(|e| format!("Failed to xfer follow path length: {:?}", e))?;
        if xfer.is_loading() {
            self.path.clear();
        }
        for idx in 0..path_len.max(0) {
            let mut pos = if xfer.is_loading() {
                Coord3D::new(0.0, 0.0, 0.0)
            } else {
                self.path
                    .get(idx as usize)
                    .copied()
                    .unwrap_or_else(|| Coord3D::new(0.0, 0.0, 0.0))
            };
            xfer.xfer_coord3d(&mut pos);
            if xfer.is_loading() {
                self.path.push(pos);
            }
        }

        if version >= 2 {
            let mut has_ignore_object = self.ignore_object_id.is_some();
            xfer.xfer_bool(&mut has_ignore_object)
                .map_err(|e| format!("Failed to xfer follow path has_ignore_object: {:?}", e))?;
            let mut ignore_object_id = self.ignore_object_id.unwrap_or(crate::common::INVALID_ID);
            xfer.xfer_object_id(&mut ignore_object_id)
                .map_err(|e| format!("Failed to xfer follow path ignore_object_id: {:?}", e))?;
            if xfer.is_loading() {
                self.ignore_object_id =
                    if has_ignore_object && ignore_object_id != crate::common::INVALID_ID {
                        Some(ignore_object_id)
                    } else {
                        None
                    };
            }
        } else if xfer.is_loading() {
            self.ignore_object_id = None;
        }

        if xfer.is_loading() && self.index > self.path.len() {
            self.index = self.path.len();
        }

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}
