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

impl StateImplementation for AIDockState {
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

impl ClassicState for AIDockState {
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
            .ok_or_else(|| "dock state missing machine owner".to_string())?;

        let Some(goal_id) = self.base.get_machine_goal_object_id() else {
            return Ok(StateReturnType::Failure);
        };

        let has_dock = crate::object::registry::OBJECT_REGISTRY
            .with_object(goal_id, |guard| {
                guard.with_dock_update_interface(|_| true).unwrap_or(false)
            })
            .unwrap_or(false);
        if !has_dock {
            return Ok(StateReturnType::Failure);
        }
        let Some(goal) = crate::helpers::TheGameLogic::find_object_by_id(goal_id)
            .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(goal_id))
        else {
            return Ok(StateReturnType::Failure);
        };

        if let Ok(owner_guard) = owner.try_read() {
            if let Some(ai) = owner_guard.get_ai_update_interface() {
                if let Ok(mut ai_guard) = ai.lock() {
                    let _ = ai_guard.ignore_obstacle(goal.read().ok().map(|g| g.get_id()));
                }
            }
        }

        let dock_machine = AIDockMachine::new(owner.clone())?;
        let init_result = if let Ok(mut machine) = dock_machine.state_machine.lock() {
            machine.set_goal_object_by_id(goal.read().ok().map(|g| g.get_id()));
            Some(machine.init_default_state())
        } else {
            None
        };
        if let Some(result) = init_result {
            self.dock_machine = Some(dock_machine);
            return Ok(result);
        }

        Ok(StateReturnType::Failure)
    }

    fn classic_on_update(&mut self) -> Result<StateReturnType, String> {
        let Some(dock_machine) = self.dock_machine.as_mut() else {
            return Ok(StateReturnType::Failure);
        };

        if let Some(owner) = self.base.get_machine_owner() {
            if let Ok(owner_guard) = owner.try_read() {
                if let Some(ai) = owner_guard.get_ai_update_interface() {
                    if let Ok(mut ai_guard) = ai.lock() {
                        let _ = ai_guard.set_can_path_through_units(true);
                    }
                }
            }
        }

        let result = dock_machine
            .state_machine
            .lock()
            .map_err(|_| "dock state machine lock failed".to_string())?
            .update();

        Ok(match result {
            StateReturnType::Sleep(_) => StateReturnType::Continue,
            other => other,
        })
    }

    fn classic_on_exit(&mut self, _exit: StateExitType) -> Result<(), String> {
        if let Some(mut machine) = self.dock_machine.take() {
            let _ = machine.halt();
        }

        let owner = self.base.get_machine_owner();
        if let Some(owner) = owner {
            if let Ok(owner_guard) = owner.try_read() {
                if let Some(ai) = owner_guard.get_ai_update_interface() {
                    if let Ok(mut ai_guard) = ai.lock() {
                        let _ = ai_guard.set_can_path_through_units(false);
                        let _ = ai_guard.ignore_obstacle(None);
                    }
                }
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

        if let Some(machine) = self.dock_machine.as_mut() {
            machine.xfer(xfer)?;
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
