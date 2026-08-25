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

/// Wait state - does nothing until interrupted.
#[derive(Debug)]
pub struct AIWaitState {
    pub(crate) base: State,
}

impl AIWaitState {
    pub fn new(machine: &StateMachine) -> Self {
        Self {
            base: State::new(machine, "AIWait"),
        }
    }
}

impl StateImplementation for AIWaitState {
    fn on_enter(&mut self) -> StateReturnType {
        StateReturnType::Continue
    }

    fn update(&mut self) -> StateReturnType {
        StateReturnType::Continue
    }

    fn on_exit(&mut self, _status: StateExitType) {}
}

impl ClassicState for AIWaitState {
    fn base_state(&self) -> &State {
        &self.base
    }

    fn base_state_mut(&mut self) -> &mut State {
        &mut self.base
    }

    fn classic_on_enter(&mut self) -> Result<StateReturnType, String> {
        Ok(StateReturnType::Continue)
    }

    fn classic_on_update(&mut self) -> Result<StateReturnType, String> {
        Ok(StateReturnType::Continue)
    }

    fn classic_on_exit(&mut self, _exit: StateExitType) -> Result<(), String> {
        Ok(())
    }
}

/// Busy state - remain busy until AI reports idle.
#[derive(Debug)]
pub struct AIBusyState {
    pub(crate) base: State,
}

impl AIBusyState {
    pub fn new(machine: &StateMachine) -> Self {
        Self {
            base: State::new(machine, "AIBusy"),
        }
    }
}

impl StateImplementation for AIBusyState {
    fn on_enter(&mut self) -> StateReturnType {
        self.classic_on_enter().unwrap_or(StateReturnType::Failure)
    }

    fn update(&mut self) -> StateReturnType {
        self.classic_on_update().unwrap_or(StateReturnType::Failure)
    }

    fn on_exit(&mut self, _status: StateExitType) {
        let _ = self.classic_on_exit(_status);
    }

    fn is_busy(&self) -> bool {
        true
    }
}

impl ClassicState for AIBusyState {
    fn base_state(&self) -> &State {
        &self.base
    }

    fn base_state_mut(&mut self) -> &mut State {
        &mut self.base
    }

    fn classic_on_enter(&mut self) -> Result<StateReturnType, String> {
        let owner = self
            .base
            .get_machine_owner()
            .ok_or_else(|| "busy missing owner".to_string())?;
        let owner_guard = owner
            .read()
            .map_err(|_| "busy owner lock poisoned".to_string())?;
        if let Some(ai) = owner_guard.get_ai_update_interface() {
            if let Ok(mut ai_guard) = ai.lock() {
                let params = AiCommandParams::new(AiCommandType::Busy, CommandSourceType::FromAi);
                let _ = ai_guard.execute_command(&params);
            }
        }
        Ok(StateReturnType::Continue)
    }

    fn classic_on_update(&mut self) -> Result<StateReturnType, String> {
        let owner = self
            .base
            .get_machine_owner()
            .ok_or_else(|| "busy missing owner".to_string())?;
        let owner_guard = owner
            .read()
            .map_err(|_| "busy owner lock poisoned".to_string())?;
        let ai = owner_guard
            .get_ai_update_interface()
            .ok_or_else(|| "busy missing AIUpdateInterface".to_string())?;
        let ai_guard = ai.lock().map_err(|_| "busy AI lock poisoned".to_string())?;
        if ai_guard.is_idle() {
            Ok(StateReturnType::Success)
        } else {
            Ok(StateReturnType::Continue)
        }
    }

    fn classic_on_exit(&mut self, _exit: StateExitType) -> Result<(), String> {
        Ok(())
    }

    fn classic_is_busy(&self) -> bool {
        true
    }
}
