#![allow(deprecated, unused_imports, dead_code)]

use super::attack::*;
use super::attack_machine::*;
use super::dead::*;
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

/// Dock state - dock with a goal object that supports docking
#[derive(Debug)]
pub struct AIDockState {
    pub(crate) base: State,
    pub(crate) dock_machine: Option<AIDockMachine>,
    pub(crate) using_precision_movement: bool,
}

impl AIDockState {
    pub fn new(machine: &StateMachine) -> Self {
        Self {
            base: State::new(machine, "AIDock"),
            dock_machine: None,
            using_precision_movement: false,
        }
    }
}

impl AIDockState {
    fn enter(&mut self, ai: Option<&mut dyn AIUpdateInterface>) -> Result<StateReturnType, String> {
        // Wave 257: empty dual-world → fail-closed state.
        if dual_world_registry_unavailable() {
            return Ok(StateReturnType::Failure);
        }

        let owner = self
            .base
            .get_machine_owner()
            .ok_or_else(|| "dock state missing machine owner".to_string())?;

        let Some(goal_id) = self.base.get_machine_goal_object_id() else {
            return Ok(StateReturnType::Failure);
        };

        let has_dock = crate::object::registry::OBJECT_REGISTRY
            .with_object(goal_id, |guard| guard.dock_update_handle().is_some())
            .unwrap_or(false);
        if !has_dock {
            return Ok(StateReturnType::Failure);
        }
        let Some(goal) = crate::helpers::TheGameLogic::find_object_by_id(goal_id)
            .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(goal_id))
        else {
            return Ok(StateReturnType::Failure);
        };

        if let Ok(mut owner_guard) = owner.write() {
            owner_guard.ai_pending_ignore_id = Some(goal_id);
        }

        let mut dock_machine = AIDockMachine::new(owner)?;
        let result = match ai {
            Some(ai) => dock_machine.start_with_ai(goal_id, ai),
            None => dock_machine.start(goal_id),
        };
        self.dock_machine = Some(dock_machine);
        Ok(result)
    }

    fn step(&mut self, ai: Option<&mut dyn AIUpdateInterface>) -> Result<StateReturnType, String> {
        let Some(dock_machine) = self.dock_machine.as_mut() else {
            return Ok(StateReturnType::Failure);
        };

        if let Some(owner) = self.base.get_machine_owner() {
            if let Ok(mut owner_guard) = owner.write() {
                owner_guard.ai_pending_path_through_units = Some(true);
            }
        }

        let result = match ai {
            Some(ai) => dock_machine.update_with_ai(ai),
            None => dock_machine.update(),
        };

        Ok(match result {
            StateReturnType::Sleep(_) => StateReturnType::Continue,
            other => other,
        })
    }
}

impl StateImplementation for AIDockState {
    fn on_enter(&mut self) -> StateReturnType {
        self.cpp_on_enter().unwrap_or(StateReturnType::Failure)
    }

    fn on_enter_with_ai(
        &mut self,
        ai: &mut dyn AIUpdateInterface,
        _id: ObjectID,
        _pos: Coord3D,
    ) -> StateReturnType {
        self.enter(Some(ai)).unwrap_or(StateReturnType::Failure)
    }
    fn update_with_ai(&mut self, ai: &mut dyn AIUpdateInterface) -> StateReturnType {
        self.step(Some(ai)).unwrap_or(StateReturnType::Failure)
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

    fn on_exit(&mut self, status: StateExitType) {
        let _ = self.cpp_on_exit(status);
    }

    fn xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::xfer(self, xfer)
    }
}

impl CppState for AIDockState {
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
        self.enter(None)
    }
    fn cpp_on_update(&mut self) -> Result<StateReturnType, String> {
        self.step(None)
    }

    fn cpp_on_exit(&mut self, _exit: StateExitType) -> Result<(), String> {
        if let Some(mut machine) = self.dock_machine.take() {
            let _ = machine.halt();
        }

        let owner = self.base.get_machine_owner();
        if let Some(owner) = owner {
            if let Ok(mut owner_guard) = owner.write() {
                owner_guard.ai_pending_path_through_units = Some(false);
                owner_guard.ai_pending_clear_ignore = true;
            }
        }
        Ok(())
    }
}

impl Snapshotable for AIDockState {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: game_engine::common::system::xfer::XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to crc version: {:?}", e))?;

        let mut has_machine = self.dock_machine.is_some();
        xfer.xfer_bool(&mut has_machine)
            .map_err(|e| format!("Failed to crc dock has_machine: {:?}", e))?;

        if let Some(machine) = self.dock_machine.as_ref() {
            machine.crc(xfer)?;
        }

        let mut using_precision_movement = self.using_precision_movement;
        xfer.xfer_bool(&mut using_precision_movement)
            .map_err(|e| format!("Failed to crc precision movement: {:?}", e))?;

        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: game_engine::common::system::xfer::XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to xfer version: {:?}", e))?;

        let mut has_machine = self.dock_machine.is_some();
        xfer.xfer_bool(&mut has_machine)
            .map_err(|e| format!("Failed to xfer dock has_machine: {:?}", e))?;

        if xfer.is_loading() && has_machine && self.dock_machine.is_none() {
            let owner = self
                .base
                .get_machine_owner()
                .ok_or_else(|| "dock state missing machine owner".to_string())?;
            self.dock_machine = Some(AIDockMachine::new(owner)?);
        }

        // C++ AIStates.cpp:6040 gates child bytes on the wire flag, even
        // when the receiving parent already has a child. Retain that child.
        if has_machine {
            if let Some(machine) = self.dock_machine.as_mut() {
                machine.xfer(xfer)?;
            }
        }

        xfer.xfer_bool(&mut self.using_precision_movement)
            .map_err(|e| format!("Failed to xfer precision movement: {:?}", e))?;

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        if let Some(machine) = self.dock_machine.as_mut() {
            machine.load_post_process()?;
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "dock_snapshot_tests.rs"]
mod snapshot_tests;
