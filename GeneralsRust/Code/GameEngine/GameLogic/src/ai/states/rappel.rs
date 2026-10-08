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
    AiCommandInterface, AiCommandParams, GuardMode, MoodMatrixAction, PartitionFilter,
    mood_matrix_adjustment, mood_matrix_parameters, resolve_attack_priority_info_for_object,
    search_qualifiers, the_ai,
};
use crate::attack::{AbleToAttackType, CanAttackResult};
use crate::command_button::CommandButton;
use crate::common::coord::*;
use crate::common::xfer::XferExt;
use crate::common::*;
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
use crate::state_machine::cpp_state::{CppState, cpp_transition, register_cpp_state};
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

/// Rappel into state (simplified; triggers AI command).
#[derive(Debug)]
pub struct AIRappelIntoState {
    pub(crate) base: State,
    pub(crate) rappel_rate: Real,
    pub(crate) dest_z: Real,
    pub(crate) target_is_bldg: Bool,
}

impl AIRappelIntoState {
    pub fn new(machine: &StateMachine) -> Self {
        Self {
            // C++ parity: class name is AIRappelState in AIStates.cpp.
            base: State::new(machine, "AIRappelState"),
            rappel_rate: 0.0,
            dest_z: 0.0,
            target_is_bldg: false,
        }
    }
}

impl StateImplementation for AIRappelIntoState {
    fn on_enter(&mut self) -> StateReturnType {
        self.cpp_on_enter().unwrap_or(StateReturnType::Failure)
    }

    fn bind_goal_object_id(&mut self, id: crate::common::ObjectID) {
        self.base.goal_object_id = id;
    }

    fn bind_goal_position(&mut self, pos: Coord3D) {
        self.base.goal_position_copied = Some(pos);
    }

    fn on_enter_with_ai(
        &mut self,
        ai: &mut dyn crate::modules::ai_state_runtime::AiStateRuntime,
        _goal_id: crate::common::ObjectID,
        _goal_pos: Coord3D,
    ) -> StateReturnType {
        let speed = ai.get_desired_speed();
        let result = self.cpp_on_enter().unwrap_or(StateReturnType::Failure);
        let max_rappel_rate = GRAVITY.abs() * (LOGICFRAMES_PER_SECOND as Real) * 2.5;
        self.rappel_rate = -speed.min(max_rappel_rate);
        result
    }

    fn update(&mut self) -> StateReturnType {
        self.cpp_on_update().unwrap_or(StateReturnType::Failure)
    }

    fn update_with_ai(
        &mut self,
        ai: &mut dyn crate::modules::ai_state_runtime::AiStateRuntime,
    ) -> StateReturnType {
        if ai.is_in_rappel_state() {
            StateReturnType::Continue
        } else {
            StateReturnType::Success
        }
    }

    fn on_exit(&mut self, _status: StateExitType) {
        let _ = self.cpp_on_exit(_status);
    }

    fn on_exit_with_ai(
        &mut self,
        status: StateExitType,
        ai: &mut dyn crate::modules::ai_state_runtime::AiStateRuntime,
    ) {
        ai.set_desired_speed(FAST_AS_POSSIBLE);
        self.on_exit(status);
        if let Some(owner) = self.base.get_machine_owner() {
            if let Ok(mut owner_guard) = owner.write() {
                owner_guard.ai_pending_desired_speed = None;
            }
        }
    }

    fn xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::xfer(self, xfer)
    }
}

impl CppState for AIRappelIntoState {
    fn base_state(&self) -> &State {
        &self.base
    }

    fn base_state_mut(&mut self) -> &mut State {
        &mut self.base
    }

    fn cpp_xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::xfer(self, xfer)
    }

    fn cpp_on_enter(&mut self) -> Result<StateReturnType, String> {
        // Wave 257: empty dual-world → fail-closed state.
        if dual_world_registry_unavailable() {
            return Ok(StateReturnType::Failure);
        }

        let owner = self
            .base
            .get_machine_owner()
            .ok_or_else(|| "rappel missing owner".to_string())?;
        let mut owner_guard = owner
            .write()
            .map_err(|_| "rappel owner lock poisoned".to_string())?;
        if !owner_guard.is_kind_of(KindOf::CanRappel) {
            return Ok(StateReturnType::Failure);
        }
        owner_guard.set_model_condition_state(ModelConditionFlags::RAPPELLING);
        if let Some(physics) = owner_guard.get_physics() {
            physics.reset_dynamic_physics();
        }

        self.target_is_bldg = false;
        if let Some(goal_id) = self.base.get_machine_goal_object_id() {
            if crate::object::registry::OBJECT_REGISTRY
                .with_object(goal_id, |goal_guard| {
                    !goal_guard.is_effectively_dead() && goal_guard.is_kind_of(KindOf::Structure)
                })
                .unwrap_or(false)
            {
                self.target_is_bldg = true;
            }
        }

        let terrain = get_terrain_logic();
        let terrain_guard = terrain
            .read()
            .map_err(|_| "rappel terrain lock poisoned".to_string())?;
        let owner_pos = *owner_guard.get_position();
        let layer = terrain_guard.get_highest_layer_for_destination(&owner_pos);
        self.dest_z = terrain_guard.get_layer_height(owner_pos.x, owner_pos.y, layer, None, false);
        if self.target_is_bldg {
            if let Some(goal_id) = self.base.get_machine_goal_object_id() {
                if let Some(height) =
                    crate::object::registry::OBJECT_REGISTRY.with_object(goal_id, |goal_guard| {
                        goal_guard
                            .get_geometry_info()
                            .get_max_height_above_position()
                    })
                {
                    self.dest_z += height;
                }
            }
        }

        let max_rappel_rate = GRAVITY.abs() * (LOGICFRAMES_PER_SECOND as Real) * 2.5;
        self.rappel_rate = -owner_guard.ai_fire_desired_speed.min(max_rappel_rate);
        owner_guard.ai_pending_rappel = true;
        owner_guard.ai_pending_rappel_obj = self.base.get_machine_goal_object_id();
        owner_guard.ai_pending_rappel_pos = self.base.get_machine_goal_position();
        Ok(StateReturnType::Continue)
    }

    fn cpp_on_update(&mut self) -> Result<StateReturnType, String> {
        let owner = self
            .base
            .get_machine_owner()
            .ok_or_else(|| "rappel missing owner".to_string())?;
        let owner_guard = owner
            .read()
            .map_err(|_| "rappel owner lock poisoned".to_string())?;
        if owner_guard.ai_fire_in_rappel {
            Ok(StateReturnType::Continue)
        } else {
            Ok(StateReturnType::Success)
        }
    }

    fn cpp_on_exit(&mut self, _exit: StateExitType) -> Result<(), String> {
        let owner = self
            .base
            .get_machine_owner()
            .ok_or_else(|| "rappel missing owner".to_string())?;
        let mut owner_guard = owner
            .write()
            .map_err(|_| "rappel owner lock poisoned".to_string())?;
        owner_guard.clear_model_condition_state(ModelConditionFlags::RAPPELLING);
        owner_guard.ai_pending_desired_speed = Some(FAST_AS_POSSIBLE);
        Ok(())
    }
}

/// Combat drop state (simplified; triggers AI command).
#[derive(Debug)]
pub struct AICombatDropState {
    pub(crate) base: State,
    pub(crate) issued_command: bool,
}

impl AICombatDropState {
    pub fn new(machine: &StateMachine) -> Self {
        Self {
            base: State::new(machine, "AICombatDrop"),
            issued_command: false,
        }
    }
}

impl StateImplementation for AICombatDropState {
    fn on_enter(&mut self) -> StateReturnType {
        self.cpp_on_enter().unwrap_or(StateReturnType::Failure)
    }

    fn bind_goal_object_id(&mut self, id: crate::common::ObjectID) {
        self.base.goal_object_id = id;
    }

    fn bind_goal_position(&mut self, pos: Coord3D) {
        self.base.goal_position_copied = Some(pos);
    }

    fn update(&mut self) -> StateReturnType {
        self.cpp_on_update().unwrap_or(StateReturnType::Failure)
    }

    fn update_with_ai(
        &mut self,
        ai: &mut dyn crate::modules::ai_state_runtime::AiStateRuntime,
    ) -> StateReturnType {
        if ai.is_doing_combat_drop() {
            StateReturnType::Continue
        } else {
            StateReturnType::Success
        }
    }

    fn on_exit(&mut self, _status: StateExitType) {
        let _ = self.cpp_on_exit(_status);
    }

    fn xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::xfer(self, xfer)
    }
}

impl CppState for AICombatDropState {
    fn base_state(&self) -> &State {
        &self.base
    }

    fn base_state_mut(&mut self) -> &mut State {
        &mut self.base
    }

    fn cpp_xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::xfer(self, xfer)
    }

    fn cpp_on_enter(&mut self) -> Result<StateReturnType, String> {
        let owner = self
            .base
            .get_machine_owner()
            .ok_or_else(|| "combat drop missing owner".to_string())?;
        let mut owner_guard = owner
            .write()
            .map_err(|_| "combat drop owner lock poisoned".to_string())?;
        owner_guard.ai_pending_combat_drop = true;
        owner_guard.ai_pending_combat_drop_obj = self.base.get_machine_goal_object_id();
        owner_guard.ai_pending_combat_drop_pos = self.base.get_machine_goal_position();
        self.issued_command = true;
        Ok(StateReturnType::Continue)
    }

    fn cpp_on_update(&mut self) -> Result<StateReturnType, String> {
        if !self.issued_command {
            return Ok(StateReturnType::Failure);
        }
        let owner = self
            .base
            .get_machine_owner()
            .ok_or_else(|| "combat drop missing owner".to_string())?;
        let owner_guard = owner
            .read()
            .map_err(|_| "combat drop owner lock poisoned".to_string())?;
        if owner_guard.ai_fire_combat_drop {
            Ok(StateReturnType::Continue)
        } else {
            Ok(StateReturnType::Success)
        }
    }

    fn cpp_on_exit(&mut self, _exit: StateExitType) -> Result<(), String> {
        Ok(())
    }
}

impl Snapshotable for AIRappelIntoState {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: game_engine::common::system::xfer::XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to crc version: {:?}", e))?;
        let mut rappel_rate = self.rappel_rate;
        xfer.xfer_real(&mut rappel_rate)
            .map_err(|e| format!("Failed to crc rappel_rate: {:?}", e))?;
        let mut dest_z = self.dest_z;
        xfer.xfer_real(&mut dest_z)
            .map_err(|e| format!("Failed to crc rappel dest_z: {:?}", e))?;
        let mut target_is_bldg = self.target_is_bldg;
        xfer.xfer_bool(&mut target_is_bldg)
            .map_err(|e| format!("Failed to crc rappel target_is_bldg: {:?}", e))?;
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: game_engine::common::system::xfer::XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to xfer version: {:?}", e))?;
        xfer.xfer_real(&mut self.rappel_rate)
            .map_err(|e| format!("Failed to xfer rappel_rate: {:?}", e))?;
        xfer.xfer_real(&mut self.dest_z)
            .map_err(|e| format!("Failed to xfer rappel dest_z: {:?}", e))?;
        xfer.xfer_bool(&mut self.target_is_bldg)
            .map_err(|e| format!("Failed to xfer rappel target_is_bldg: {:?}", e))?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

impl Snapshotable for AICombatDropState {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: game_engine::common::system::xfer::XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to crc version: {:?}", e))?;
        let mut issued_command = self.issued_command;
        xfer.xfer_bool(&mut issued_command)
            .map_err(|e| format!("Failed to crc combat drop issued_command: {:?}", e))?;
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: game_engine::common::system::xfer::XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to xfer version: {:?}", e))?;
        xfer.xfer_bool(&mut self.issued_command)
            .map_err(|e| format!("Failed to xfer combat drop issued_command: {:?}", e))?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}
