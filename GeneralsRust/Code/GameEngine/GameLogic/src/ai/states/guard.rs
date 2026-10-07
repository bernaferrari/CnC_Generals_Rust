#![allow(deprecated, unused_imports, dead_code)]

use super::attack::*;
use super::attack_machine::*;
use super::dead::*;
use super::dock::*;
use super::enter::*;
use super::face::*;
use super::follow_path::*;
use super::follow_path_core::*;
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

/// Guard state
#[derive(Debug)]
pub struct AIGuardState {
    pub(crate) base: State,
    pub(crate) guard_machine: Option<AIGuardMachine>,
    enter_mode: i32,
    enter_polygon: Option<Arc<PolygonTrigger>>,
}

impl AIGuardState {
    pub fn new(machine: &StateMachine) -> Self {
        Self {
            base: State::new(machine, "AIGuard"),
            guard_machine: None,
            enter_mode: 0,
            enter_polygon: None,
        }
    }

    fn enter_with_ai(
        &mut self,
        ai: &mut dyn crate::modules::AIUpdateInterface,
        goal_id: crate::common::ObjectID,
        goal_pos: Coord3D,
    ) -> StateReturnType {
        if dual_world_registry_unavailable() {
            return StateReturnType::Failure;
        }
        let Some(owner) = self.base.get_machine_owner() else {
            return StateReturnType::Failure;
        };
        let mut guard_machine = AIGuardMachine::new(Arc::downgrade(&owner));
        if let Some(polygon) = self.enter_polygon.clone() {
            guard_machine.set_area_to_guard(Some(polygon.clone()));
            let center = polygon.get_center_point();
            guard_machine.set_target_position_to_guard(&center);
        } else if goal_id != crate::common::INVALID_ID {
            if let Some(target) = crate::helpers::TheGameLogic::find_object_by_id(goal_id)
                .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(goal_id))
            {
                guard_machine.set_target_to_guard(Some(&target));
            } else {
                guard_machine.set_target_position_to_guard(&goal_pos);
            }
        } else {
            guard_machine.set_target_position_to_guard(&goal_pos);
        }
        guard_machine.set_guard_mode(GuardMode::from_i32(self.enter_mode));
        if guard_machine.init_default_state_with_ai(ai).is_failure() {
            return StateReturnType::Failure;
        }
        let result = guard_machine.set_state_with_ai(GuardStateType::Return, ai);
        self.guard_machine = Some(guard_machine);
        result
    }

    fn update_with_ai(
        &mut self,
        ai: &mut dyn crate::modules::AIUpdateInterface,
    ) -> StateReturnType {
        self.guard_machine
            .as_mut()
            .map(|guard_machine| guard_machine.update_with_ai(ai))
            .unwrap_or(StateReturnType::Failure)
    }
}

impl StateImplementation for AIGuardState {
    fn on_enter(&mut self) -> StateReturnType {
        self.cpp_on_enter().unwrap_or(StateReturnType::Failure)
    }

    fn bind_goal_object_id(&mut self, id: crate::common::ObjectID) {
        self.base.goal_object_id = id;
    }

    fn bind_goal_position(&mut self, pos: Coord3D) {
        self.base.goal_position_copied = Some(pos);
    }

    fn bind_goal_squad(&mut self, squad: Option<std::sync::Arc<crate::ai::squad::Squad>>) {
        self.base.goal_squad_copied = squad;
    }

    fn bind_goal_polygon(
        &mut self,
        polygon: Option<std::sync::Arc<crate::polygon_trigger::PolygonTrigger>>,
    ) {
        self.base.goal_polygon_copied = polygon;
    }

    fn note_guard_enter(&mut self, mode: i32, polygon: Option<Arc<PolygonTrigger>>) {
        self.enter_mode = mode;
        self.enter_polygon = polygon;
    }

    fn freezes_parent_during_update(&self) -> bool {
        true
    }
    fn on_enter_with_ai(
        &mut self,
        ai: &mut dyn crate::modules::AIUpdateInterface,
        goal_id: crate::common::ObjectID,
        goal_pos: Coord3D,
    ) -> StateReturnType {
        self.enter_with_ai(ai, goal_id, goal_pos)
    }

    fn update(&mut self) -> StateReturnType {
        self.cpp_on_update().unwrap_or(StateReturnType::Failure)
    }
    fn update_with_ai(
        &mut self,
        ai: &mut dyn crate::modules::AIUpdateInterface,
    ) -> StateReturnType {
        self.update_with_ai(ai)
    }

    fn on_exit(&mut self, _status: StateExitType) {
        let _ = self.cpp_on_exit(_status);
    }

    fn xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::xfer(self, xfer)
    }
}

impl CppState for AIGuardState {
    fn base_state(&self) -> &State {
        &self.base
    }

    fn base_state_mut(&mut self) -> &mut State {
        &mut self.base
    }

    fn cpp_note_guard_enter(&mut self, mode: i32, polygon: Option<Arc<PolygonTrigger>>) {
        self.enter_mode = mode;
        self.enter_polygon = polygon;
    }

    fn cpp_freezes_parent_during_update(&self) -> bool {
        true
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
            .ok_or_else(|| "guard state missing machine owner".to_string())?;

        let mut guard_machine = AIGuardMachine::new(Arc::downgrade(&owner));

        if let Some(polygon) = self.base.get_machine_goal_polygon() {
            guard_machine.set_area_to_guard(Some(polygon.clone()));
            let center = polygon.get_center_point();
            guard_machine.set_target_position_to_guard(&center);
        } else if let Some(target) = self.base.get_machine_goal_object_id().and_then(|id| {
            crate::helpers::TheGameLogic::find_object_by_id(id)
                .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(id))
        }) {
            guard_machine.set_target_to_guard(Some(&target));
        } else if let Some(pos) = self.base.get_machine_goal_position() {
            guard_machine.set_target_position_to_guard(&pos);
        } else if let Ok(owner_guard) = owner.try_read() {
            guard_machine.set_target_position_to_guard(owner_guard.get_position());
        }

        let guard_mode = self
            .base
            .get_machine()
            .ok()
            .and_then(|machine| {
                machine
                    .try_lock()
                    .ok()
                    .map(|guard| guard.get_guard_mode_raw())
            })
            .unwrap_or(self.enter_mode);
        guard_machine.set_guard_mode(GuardMode::from_i32(guard_mode));

        if guard_machine.init_default_state().is_failure() {
            return Ok(StateReturnType::Failure);
        }

        let result = guard_machine.set_state(GuardStateType::Return);
        self.guard_machine = Some(guard_machine);
        Ok(result)
    }

    fn cpp_on_enter_with_ai(
        &mut self,
        ai: &mut dyn crate::modules::AIUpdateInterface,
    ) -> Result<StateReturnType, String> {
        let goal_id = self
            .base
            .get_machine_goal_object_id()
            .unwrap_or(crate::common::INVALID_ID);
        let goal_pos = self
            .base
            .get_machine_goal_position()
            .unwrap_or(Coord3D::new(0.0, 0.0, 0.0));
        Ok(self.enter_with_ai(ai, goal_id, goal_pos))
    }

    fn cpp_on_update(&mut self) -> Result<StateReturnType, String> {
        let Some(guard_machine) = self.guard_machine.as_mut() else {
            return Ok(StateReturnType::Failure);
        };

        Ok(guard_machine.update())
    }

    fn cpp_on_update_with_ai(
        &mut self,
        ai: &mut dyn crate::modules::AIUpdateInterface,
    ) -> Result<StateReturnType, String> {
        Ok(self.update_with_ai(ai))
    }

    fn cpp_on_exit(&mut self, _exit: StateExitType) -> Result<(), String> {
        if let Some(mut machine) = self.guard_machine.take() {
            let _ = machine.halt();
        }
        // C++ AIGuardState::onExit: obj->getAI()->clearGuardTargetType()
        clear_owner_guard_target_type(&self.base);
        Ok(())
    }

    fn cpp_is_guard_idle(&self) -> bool {
        self.guard_machine
            .as_ref()
            .map(|machine| machine.is_in_guard_idle_state())
            .unwrap_or(false)
    }

    fn cpp_is_attack(&self) -> bool {
        self.guard_machine
            .as_ref()
            .map(|machine| machine.is_in_attack_state())
            .unwrap_or(false)
    }
}

/// Guard retaliate state
#[derive(Debug)]
pub struct AIGuardRetaliateState {
    pub(crate) base: State,
    pub(crate) guard_machine: Option<AIGuardRetaliateMachine>,
}

impl AIGuardRetaliateState {
    pub fn new(machine: &StateMachine) -> Self {
        Self {
            base: State::new(machine, "AIGuardRetaliate"),
            guard_machine: None,
        }
    }
}

impl StateImplementation for AIGuardRetaliateState {
    fn on_enter(&mut self) -> StateReturnType {
        self.cpp_on_enter().unwrap_or(StateReturnType::Failure)
    }

    fn on_enter_with_ai(
        &mut self,
        ai: &mut dyn crate::modules::AIUpdateInterface,
        _goal_id: crate::common::ObjectID,
        _goal_pos: Coord3D,
    ) -> StateReturnType {
        self.cpp_on_enter_with_ai(ai)
            .unwrap_or(StateReturnType::Failure)
    }

    fn update_with_ai(
        &mut self,
        ai: &mut dyn crate::modules::AIUpdateInterface,
    ) -> StateReturnType {
        self.cpp_on_update_with_ai(ai)
            .unwrap_or(StateReturnType::Failure)
    }

    fn on_exit_with_ai(
        &mut self,
        exit: StateExitType,
        ai: &mut dyn crate::modules::AIUpdateInterface,
    ) {
        let _ = self.cpp_on_exit_with_ai(exit, ai);
    }

    fn bind_goal_object_id(&mut self, id: crate::common::ObjectID) {
        self.base.goal_object_id = id;
    }

    fn bind_goal_position(&mut self, pos: Coord3D) {
        self.base.goal_position_copied = Some(pos);
    }

    fn bind_goal_squad(&mut self, squad: Option<std::sync::Arc<crate::ai::squad::Squad>>) {
        self.base.goal_squad_copied = squad;
    }

    fn bind_goal_polygon(
        &mut self,
        polygon: Option<std::sync::Arc<crate::polygon_trigger::PolygonTrigger>>,
    ) {
        self.base.goal_polygon_copied = polygon;
    }

    fn update(&mut self) -> StateReturnType {
        self.cpp_on_update().unwrap_or(StateReturnType::Failure)
    }

    fn on_exit(&mut self, _status: StateExitType) {
        let _ = self.cpp_on_exit(_status);
    }

    fn xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::xfer(self, xfer)
    }
}

impl CppState for AIGuardRetaliateState {
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
        self.enter_retaliate(None)
    }

    fn cpp_on_enter_with_ai(
        &mut self,
        ai: &mut dyn crate::modules::AIUpdateInterface,
    ) -> Result<StateReturnType, String> {
        self.enter_retaliate(Some(ai))
    }

    fn cpp_on_update_with_ai(
        &mut self,
        ai: &mut dyn crate::modules::AIUpdateInterface,
    ) -> Result<StateReturnType, String> {
        Ok(self
            .guard_machine
            .as_mut()
            .map(|machine| machine.update_with_ai(ai))
            .unwrap_or(StateReturnType::Failure))
    }

    fn cpp_on_exit_with_ai(
        &mut self,
        _exit: StateExitType,
        ai: &mut dyn crate::modules::AIUpdateInterface,
    ) -> Result<(), String> {
        if let Some(mut machine) = self.guard_machine.take() {
            let _ = machine.halt();
        }
        ai.clear_guard_target_type();
        Ok(())
    }

    fn cpp_on_update(&mut self) -> Result<StateReturnType, String> {
        let Some(guard_machine) = self.guard_machine.as_mut() else {
            return Ok(StateReturnType::Failure);
        };

        Ok(guard_machine.update())
    }

    fn cpp_on_exit(&mut self, _exit: StateExitType) -> Result<(), String> {
        if let Some(mut machine) = self.guard_machine.take() {
            let _ = machine.halt();
        }
        // C++ AIGuardRetaliateState::onExit: obj->getAI()->clearGuardTargetType()
        clear_owner_guard_target_type(&self.base);
        Ok(())
    }

    fn cpp_is_attack(&self) -> bool {
        self.guard_machine
            .as_ref()
            .map(|machine| machine.is_in_attack_state())
            .unwrap_or(false)
    }
}

impl AIGuardRetaliateState {
    fn enter_retaliate(
        &mut self,
        ai: Option<&mut dyn crate::modules::AIUpdateInterface>,
    ) -> Result<StateReturnType, String> {
        // Wave 257: empty dual-world → fail-closed state.
        if dual_world_registry_unavailable() {
            return Ok(StateReturnType::Failure);
        }

        let owner = self
            .base
            .get_machine_owner()
            .ok_or_else(|| "guard retaliate state missing machine owner".to_string())?;

        let mut guard_machine = AIGuardRetaliateMachine::new(Arc::downgrade(&owner));

        if let Some(pos) = self.base.get_machine_goal_position() {
            guard_machine.set_target_position_to_guard(&pos);
        } else if let Ok(owner_guard) = owner.try_read() {
            guard_machine.set_target_position_to_guard(owner_guard.get_position());
        }

        if let Some(goal) = self.base.get_machine_goal_object_id().and_then(|id| {
            crate::helpers::TheGameLogic::find_object_by_id(id)
                .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(id))
        }) {
            if let Ok(goal_guard) = goal.try_read() {
                guard_machine.set_nemesis_id(goal_guard.get_id());
            }
        }

        let result = match ai {
            Some(ai) => guard_machine.init_default_state_with_ai(ai),
            None => guard_machine.init_default_state(),
        };
        self.guard_machine = Some(guard_machine);
        Ok(result)
    }
}

/// Tunnel network guard state
#[derive(Debug)]
pub struct AITunnelNetworkGuardState {
    pub(crate) base: State,
    pub(crate) guard_machine: Option<AITNGuardMachine>,
}

impl AITunnelNetworkGuardState {
    pub fn new(machine: &StateMachine) -> Self {
        Self {
            base: State::new(machine, "AITunnelNetworkGuard"),
            guard_machine: None,
        }
    }
}

impl StateImplementation for AITunnelNetworkGuardState {
    fn on_enter(&mut self) -> StateReturnType {
        self.cpp_on_enter().unwrap_or(StateReturnType::Failure)
    }

    fn bind_goal_object_id(&mut self, id: crate::common::ObjectID) {
        self.base.goal_object_id = id;
    }

    fn bind_goal_position(&mut self, pos: Coord3D) {
        self.base.goal_position_copied = Some(pos);
    }

    fn bind_goal_squad(&mut self, squad: Option<std::sync::Arc<crate::ai::squad::Squad>>) {
        self.base.goal_squad_copied = squad;
    }

    fn bind_goal_polygon(
        &mut self,
        polygon: Option<std::sync::Arc<crate::polygon_trigger::PolygonTrigger>>,
    ) {
        self.base.goal_polygon_copied = polygon;
    }

    fn update(&mut self) -> StateReturnType {
        self.cpp_on_update().unwrap_or(StateReturnType::Failure)
    }

    fn on_exit(&mut self, _status: StateExitType) {
        let _ = self.cpp_on_exit(_status);
    }

    fn xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::xfer(self, xfer)
    }
}

impl CppState for AITunnelNetworkGuardState {
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
            .ok_or_else(|| "tunnel network guard state missing machine owner".to_string())?;

        let mut guard_machine = AITNGuardMachine::new(Arc::downgrade(&owner));

        if let Some(pos) = self.base.get_machine_goal_position() {
            guard_machine.set_target_position_to_guard(&pos);
        } else if let Ok(owner_guard) = owner.try_read() {
            guard_machine.set_target_position_to_guard(owner_guard.get_position());
        }

        guard_machine.set_guard_mode(GuardMode::Normal);

        if guard_machine.init_default_state().is_failure() {
            return Ok(StateReturnType::Failure);
        }

        let result = guard_machine.set_state(TNGuardStateType::Return);
        self.guard_machine = Some(guard_machine);
        Ok(result)
    }

    fn cpp_on_update(&mut self) -> Result<StateReturnType, String> {
        let Some(guard_machine) = self.guard_machine.as_mut() else {
            return Ok(StateReturnType::Failure);
        };

        Ok(guard_machine.update())
    }

    fn cpp_on_exit(&mut self, _exit: StateExitType) -> Result<(), String> {
        if let Some(mut machine) = self.guard_machine.take() {
            let _ = machine.halt();
        }
        // C++ AITunnelNetworkGuardState::onExit: obj->getAI()->clearGuardTargetType()
        clear_owner_guard_target_type(&self.base);
        Ok(())
    }

    fn cpp_is_attack(&self) -> bool {
        self.guard_machine
            .as_ref()
            .map(|machine| machine.is_in_attack_state())
            .unwrap_or(false)
    }
}

impl Snapshotable for AIGuardState {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: game_engine::common::system::xfer::XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to crc version: {:?}", e))?;

        let mut has_machine = self.guard_machine.is_some();
        xfer.xfer_bool(&mut has_machine)
            .map_err(|e| format!("Failed to crc guard has_machine: {:?}", e))?;

        if let Some(machine) = self.guard_machine.as_ref() {
            machine.crc(xfer)?;
        }

        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: game_engine::common::system::xfer::XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to xfer version: {:?}", e))?;

        let mut has_machine = self.guard_machine.is_some();
        xfer.xfer_bool(&mut has_machine)
            .map_err(|e| format!("Failed to xfer guard has_machine: {:?}", e))?;

        if xfer.is_loading() && has_machine && self.guard_machine.is_none() {
            let owner = self
                .base
                .get_machine_owner()
                .ok_or_else(|| "guard state missing machine owner".to_string())?;
            self.guard_machine = Some(AIGuardMachine::new(Arc::downgrade(&owner)));
        }

        if let Some(machine) = self.guard_machine.as_mut() {
            machine.xfer(xfer)?;
        }

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        if let Some(machine) = self.guard_machine.as_mut() {
            machine.load_post_process()?;
        }
        Ok(())
    }
}

impl Snapshotable for AIGuardRetaliateState {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: game_engine::common::system::xfer::XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to crc version: {:?}", e))?;

        let mut has_machine = self.guard_machine.is_some();
        xfer.xfer_bool(&mut has_machine)
            .map_err(|e| format!("Failed to crc guard retaliate has_machine: {:?}", e))?;

        if let Some(machine) = self.guard_machine.as_ref() {
            machine.crc(xfer)?;
        }

        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: game_engine::common::system::xfer::XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to xfer version: {:?}", e))?;

        let mut has_machine = self.guard_machine.is_some();
        xfer.xfer_bool(&mut has_machine)
            .map_err(|e| format!("Failed to xfer guard retaliate has_machine: {:?}", e))?;

        if xfer.is_loading() && has_machine && self.guard_machine.is_none() {
            let owner = self
                .base
                .get_machine_owner()
                .ok_or_else(|| "guard retaliate state missing machine owner".to_string())?;
            self.guard_machine = Some(AIGuardRetaliateMachine::new(Arc::downgrade(&owner)));
        }

        if let Some(machine) = self.guard_machine.as_mut() {
            machine.xfer(xfer)?;
        }

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        if let Some(machine) = self.guard_machine.as_mut() {
            machine.load_post_process()?;
        }
        Ok(())
    }
}

impl Snapshotable for AITunnelNetworkGuardState {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: game_engine::common::system::xfer::XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to crc version: {:?}", e))?;

        let mut has_machine = self.guard_machine.is_some();
        xfer.xfer_bool(&mut has_machine)
            .map_err(|e| format!("Failed to crc tunnel-guard has_machine: {:?}", e))?;

        if let Some(machine) = self.guard_machine.as_ref() {
            machine.crc(xfer)?;
        }

        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: game_engine::common::system::xfer::XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to xfer version: {:?}", e))?;

        let mut has_machine = self.guard_machine.is_some();
        xfer.xfer_bool(&mut has_machine)
            .map_err(|e| format!("Failed to xfer tunnel-guard has_machine: {:?}", e))?;

        if xfer.is_loading() && has_machine && self.guard_machine.is_none() {
            let owner = self
                .base
                .get_machine_owner()
                .ok_or_else(|| "tunnel network guard state missing machine owner".to_string())?;
            self.guard_machine = Some(AITNGuardMachine::new(Arc::downgrade(&owner)));
        }

        if let Some(machine) = self.guard_machine.as_mut() {
            machine.xfer(xfer)?;
        }

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        if let Some(machine) = self.guard_machine.as_mut() {
            machine.load_post_process()?;
        }
        Ok(())
    }
}

/// C++ `AIUpdateInterface::clearGuardTargetType` from the three guard-state onExits.
fn clear_owner_guard_target_type(state: &State) {
    let Some(owner) = state.get_machine_owner() else {
        return;
    };
    let Ok(mut owner_guard) = owner.write() else {
        return;
    };
    owner_guard.ai_pending_clear_guard_target = true;
}
