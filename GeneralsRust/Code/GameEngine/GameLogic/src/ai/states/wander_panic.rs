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
    mood_matrix_adjustment, mood_matrix_parameters, resolve_attack_priority_info_for_object,
    search_qualifiers, AiCommandInterface, AiCommandParams, GuardMode, MoodMatrixAction,
    PartitionFilter, THE_AI,
};
use crate::attack::{AbleToAttackType, CanAttackResult};
use crate::command_button::CommandButton;
use crate::common::coord::*;
use crate::common::xfer::XferExt;
use crate::common::*;
use crate::compat::{legacy_transition, register_classic_state, ClassicState};
use crate::control_bar::get_control_bar_bridge;
use crate::damage::DamageInfo;
use crate::helpers::{get_game_logic_random_value, TheAudio, TheGameLogic, ThePartitionManager};
use crate::locomotor::LocomotorAppearance;
use crate::modules::{
    AIUpdateInterface, AIUpdateInterfaceExt, BodyModuleInterfaceExt, ContainModuleInterfaceExt,
    ContainWant, ExitDoorType, PhysicsBehaviorExt, FAST_AS_POSSIBLE,
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
    Weapon, WeaponChoiceCriteria, WeaponLockType, WeaponSlotType, WeaponStatus, NO_MAX_SHOTS_LIMIT,
};
use game_engine::common::system::{GeometryType, Snapshotable, Xfer};

use crate::common::INVALID_ID;

use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock, Weak};

/// Wander along a waypoint path.
#[derive(Debug)]
pub struct AIWanderState {
    pub(crate) base: State,
    pub(crate) core: FollowWaypointPathCore,
    pub(crate) wait_frames: i32,
    pub(crate) timer: i32,
}

impl AIWanderState {
    pub fn new(machine: &StateMachine) -> Self {
        Self {
            base: State::new(machine, "AIWander"),
            core: FollowWaypointPathCore::new(false, true),
            wait_frames: 0,
            timer: 0,
        }
    }

    pub(crate) fn update_group_offset(&mut self, ai: &dyn AIUpdateInterface) {
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
                    self.core.group_offset = Coord2D::new(x, y);
                }
            }
        }
    }
}

impl StateImplementation for AIWanderState {
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

impl ClassicState for AIWanderState {
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
        self.core.current_waypoint = waypoint_id.and_then(resolve_waypoint_by_id);
        self.core.prior_waypoint = None;
        self.core.group_offset = Coord2D::new(0.0, 0.0);

        let owner = self
            .base
            .get_machine_owner()
            .ok_or_else(|| "wander missing owner".to_string())?;
        let owner_guard = owner
            .read()
            .map_err(|_| "wander owner lock poisoned".to_string())?;
        let ai = owner_guard
            .get_ai_update_interface()
            .ok_or_else(|| "wander missing AIUpdateInterface".to_string())?;
        let mut ai_guard = ai
            .lock()
            .map_err(|_| "wander AI lock poisoned".to_string())?;

        if self.core.current_waypoint.is_none() {
            return Ok(StateReturnType::Failure);
        }

        self.update_group_offset(&*ai_guard);
        self.timer = 0;
        self.wait_frames = 10 + ((owner_guard.get_id() & 0x7) as i32);

        self.core
            .compute_goal(&self.base, &owner_guard, &mut *ai_guard, false)?;
        self.core.compute_path(&mut *ai_guard)?;
        ai_guard
            .set_path_extra_distance(self.core.calc_extra_path_distance())
            .map_err(|e| format!("wander set_path_extra_distance failed: {}", e))?;

        Ok(StateReturnType::Continue)
    }

    fn classic_on_update(&mut self) -> Result<StateReturnType, String> {
        let owner = self
            .base
            .get_machine_owner()
            .ok_or_else(|| "wander missing owner".to_string())?;
        let owner_guard = owner
            .read()
            .map_err(|_| "wander owner lock poisoned".to_string())?;
        let ai = owner_guard
            .get_ai_update_interface()
            .ok_or_else(|| "wander missing AIUpdateInterface".to_string())?;
        let mut ai_guard = ai
            .lock()
            .map_err(|_| "wander AI lock poisoned".to_string())?;

        if owner_guard.is_kind_of(KindOf::CanBeRepulsed) {
            self.timer -= 1;
            if self.timer < 0 {
                self.timer = self.wait_frames;
                let enemy_id = THE_AI
                    .read()
                    .ok()
                    .and_then(|ai| {
                        ai.find_closest_repulsor(
                            owner_guard.get_id(),
                            owner_guard.get_vision_range(),
                        )
                        .ok()
                    })
                    .flatten();
                if enemy_id.is_some() {
                    return Ok(StateReturnType::Failure);
                }
            }
        }

        let close_enough = ai_guard
            .get_cur_locomotor()
            .and_then(|loc| loc.lock().ok().map(|loco| loco.get_close_enough_dist()))
            .unwrap_or(0.0);
        let status = if ai_guard.get_locomotor_distance_to_goal() <= close_enough {
            StateReturnType::Success
        } else {
            StateReturnType::Continue
        };

        if status != StateReturnType::Continue {
            self.core.current_waypoint = self.core.get_next_waypoint(&self.base);
            if self.core.current_waypoint.is_none() {
                return Ok(StateReturnType::Success);
            }
            self.update_group_offset(&*ai_guard);
            self.core
                .compute_goal(&self.base, &owner_guard, &mut *ai_guard, false)?;
            self.core.compute_path(&mut *ai_guard)?;
            return Ok(StateReturnType::Continue);
        }

        Ok(StateReturnType::Continue)
    }

    fn classic_on_exit(&mut self, _exit: StateExitType) -> Result<(), String> {
        Ok(())
    }
}

/// Panic state - wander while panicking.
#[derive(Debug)]
pub struct AIPanicState {
    pub(crate) base: State,
    pub(crate) core: FollowWaypointPathCore,
    pub(crate) wait_frames: i32,
    pub(crate) timer: i32,
}

impl AIPanicState {
    pub fn new(machine: &StateMachine) -> Self {
        Self {
            base: State::new(machine, "AIPanic"),
            core: FollowWaypointPathCore::new(false, true),
            wait_frames: 0,
            timer: 0,
        }
    }

    pub(crate) fn update_group_offset(&mut self, ai: &dyn AIUpdateInterface) {
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
                    self.core.group_offset = Coord2D::new(x, y);
                }
            }
        }
    }
}

impl StateImplementation for AIPanicState {
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

impl ClassicState for AIPanicState {
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
        self.core.current_waypoint = waypoint_id.and_then(resolve_waypoint_by_id);
        self.core.prior_waypoint = None;
        self.core.group_offset = Coord2D::new(0.0, 0.0);

        let owner = self
            .base
            .get_machine_owner()
            .ok_or_else(|| "panic missing owner".to_string())?;
        {
            let owner_guard = owner
                .read()
                .map_err(|_| "panic owner lock poisoned".to_string())?;
            let ai = owner_guard
                .get_ai_update_interface()
                .ok_or_else(|| "panic missing AIUpdateInterface".to_string())?;
            let mut ai_guard = ai
                .lock()
                .map_err(|_| "panic AI lock poisoned".to_string())?;

            if self.core.current_waypoint.is_none() {
                return Ok(StateReturnType::Failure);
            }

            self.update_group_offset(&*ai_guard);
            self.timer = 0;
            self.wait_frames = 10 + ((owner_guard.get_id() & 0x7) as i32);

            self.core
                .compute_goal(&self.base, &owner_guard, &mut *ai_guard, false)?;
            self.core.compute_path(&mut *ai_guard)?;
            ai_guard
                .set_path_extra_distance(self.core.calc_extra_path_distance())
                .map_err(|e| format!("panic set_path_extra_distance failed: {}", e))?;
        }

        if let Ok(mut owner_write) = owner.write() {
            owner_write.set_model_condition_state(ModelConditionFlags::PANICKING);
        }

        Ok(StateReturnType::Continue)
    }

    fn classic_on_update(&mut self) -> Result<StateReturnType, String> {
        let owner = self
            .base
            .get_machine_owner()
            .ok_or_else(|| "panic missing owner".to_string())?;
        let owner_guard = owner
            .read()
            .map_err(|_| "panic owner lock poisoned".to_string())?;
        let ai = owner_guard
            .get_ai_update_interface()
            .ok_or_else(|| "panic missing AIUpdateInterface".to_string())?;
        let mut ai_guard = ai
            .lock()
            .map_err(|_| "panic AI lock poisoned".to_string())?;

        if owner_guard.is_kind_of(KindOf::CanBeRepulsed) {
            self.timer -= 1;
            if self.timer < 0 {
                self.timer = self.wait_frames;
                let enemy_id = THE_AI
                    .read()
                    .ok()
                    .and_then(|ai| {
                        ai.find_closest_repulsor(
                            owner_guard.get_id(),
                            owner_guard.get_vision_range(),
                        )
                        .ok()
                    })
                    .flatten();
                if enemy_id.is_some() {
                    return Ok(StateReturnType::Failure);
                }
            }
        }

        let close_enough = ai_guard
            .get_cur_locomotor()
            .and_then(|loc| loc.lock().ok().map(|loco| loco.get_close_enough_dist()))
            .unwrap_or(0.0);
        let status = if ai_guard.get_locomotor_distance_to_goal() <= close_enough {
            StateReturnType::Success
        } else {
            StateReturnType::Continue
        };

        if status != StateReturnType::Continue {
            self.core.current_waypoint = self.core.get_next_waypoint(&self.base);
            if self.core.current_waypoint.is_none() {
                return Ok(StateReturnType::Success);
            }
            self.update_group_offset(&*ai_guard);
            self.core
                .compute_goal(&self.base, &owner_guard, &mut *ai_guard, false)?;
            self.core.compute_path(&mut *ai_guard)?;
            return Ok(StateReturnType::Continue);
        }

        Ok(StateReturnType::Continue)
    }

    fn classic_on_exit(&mut self, _exit: StateExitType) -> Result<(), String> {
        if let Some(owner) = self.base.get_machine_owner() {
            if let Ok(mut owner_guard) = owner.write() {
                owner_guard.clear_model_condition_state(ModelConditionFlags::PANICKING);
            }
        }
        Ok(())
    }
}

impl Snapshotable for AIWanderState {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: game_engine::common::system::xfer::XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to crc version: {:?}", e))?;

        self.core.crc(xfer)?;
        let mut wait_frames = self.wait_frames;
        xfer.xfer_int(&mut wait_frames)
            .map_err(|e| format!("Failed to crc wait_frames: {:?}", e))?;
        let mut timer = self.timer;
        xfer.xfer_int(&mut timer)
            .map_err(|e| format!("Failed to crc timer: {:?}", e))?;

        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: game_engine::common::system::xfer::XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to xfer version: {:?}", e))?;

        self.core.xfer(xfer)?;
        xfer.xfer_int(&mut self.wait_frames)
            .map_err(|e| format!("Failed to xfer wait_frames: {:?}", e))?;
        xfer.xfer_int(&mut self.timer)
            .map_err(|e| format!("Failed to xfer timer: {:?}", e))?;

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

impl Snapshotable for AIPanicState {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: game_engine::common::system::xfer::XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to crc version: {:?}", e))?;

        self.core.crc(xfer)?;
        let mut wait_frames = self.wait_frames;
        xfer.xfer_int(&mut wait_frames)
            .map_err(|e| format!("Failed to crc wait_frames: {:?}", e))?;
        let mut timer = self.timer;
        xfer.xfer_int(&mut timer)
            .map_err(|e| format!("Failed to crc timer: {:?}", e))?;

        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: game_engine::common::system::xfer::XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to xfer version: {:?}", e))?;

        self.core.xfer(xfer)?;
        xfer.xfer_int(&mut self.wait_frames)
            .map_err(|e| format!("Failed to xfer wait_frames: {:?}", e))?;
        xfer.xfer_int(&mut self.timer)
            .map_err(|e| format!("Failed to xfer timer: {:?}", e))?;

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}
