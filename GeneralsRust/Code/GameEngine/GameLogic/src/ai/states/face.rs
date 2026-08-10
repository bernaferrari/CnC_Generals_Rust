#![allow(deprecated, unused_imports, dead_code)]

use super::*;
use super::helpers::*;
use super::follow_path_core::*;
use super::types::*;
use super::state_machine::*;
use super::idle::*;
use super::r#move::*;
use super::follow_path::*;
use super::wait_busy::*;
use super::wander_panic::*;
use super::hack::*;
use super::rappel::*;
use super::waypoint::*;
use super::attack::*;
use super::attack_machine::*;
use super::guard::*;
use super::hunt::*;
use super::dock::*;
use super::enter::*;
use super::dead::*;

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


/// Face object state
#[derive(Debug)]
pub struct AIFaceObjectState {
    pub(crate) base: State,
}


impl AIFaceObjectState {
    pub fn new(machine: &StateMachine) -> Self {
        Self {
            base: State::new(machine, "AIFaceObject"),
        }
    }
}


impl StateImplementation for AIFaceObjectState {
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


impl ClassicState for AIFaceObjectState {
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

        let owner = self
            .base
            .get_machine_owner()
            .ok_or_else(|| "face object missing owner".to_string())?;
        let owner_guard = owner
            .read()
            .map_err(|_| "face object owner lock poisoned".to_string())?;
        let goal_id = self
            .base
            .get_machine_goal_object_id()
            .ok_or_else(|| "face object missing goal".to_string())?;
        let Some(goal_pos) = crate::object::registry::OBJECT_REGISTRY
            .with_object(goal_id, |guard| *guard.get_position())
        else {
            return Ok(StateReturnType::Failure);
        };
        let dx = goal_pos.x - owner_guard.get_position().x;
        let dy = goal_pos.y - owner_guard.get_position().y;
        let angle = dy.atan2(dx);
        drop(owner_guard);
        if let Ok(mut owner_write) = owner.write() {
            let _ = owner_write.set_orientation(angle);
        }
        Ok(StateReturnType::Success)
    }

    fn classic_on_update(&mut self) -> Result<StateReturnType, String> {
        Ok(StateReturnType::Success)
    }

    fn classic_on_exit(&mut self, _exit: StateExitType) -> Result<(), String> {
        Ok(())
    }
}


/// Face position state
#[derive(Debug)]
pub struct AIFacePositionState {
    pub(crate) base: State,
}


impl AIFacePositionState {
    pub fn new(machine: &StateMachine) -> Self {
        Self {
            base: State::new(machine, "AIFacePosition"),
        }
    }
}


impl StateImplementation for AIFacePositionState {
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


impl ClassicState for AIFacePositionState {
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
            .ok_or_else(|| "face position missing owner".to_string())?;
        let owner_guard = owner
            .read()
            .map_err(|_| "face position owner lock poisoned".to_string())?;
        let goal = self
            .base
            .get_machine_goal_position()
            .ok_or_else(|| "face position missing goal position".to_string())?;
        let dx = goal.x - owner_guard.get_position().x;
        let dy = goal.y - owner_guard.get_position().y;
        let angle = dy.atan2(dx);
        drop(owner_guard);
        if let Ok(mut owner_write) = owner.write() {
            let _ = owner_write.set_orientation(angle);
        }
        Ok(StateReturnType::Success)
    }

    fn classic_on_update(&mut self) -> Result<StateReturnType, String> {
        Ok(StateReturnType::Success)
    }

    fn classic_on_exit(&mut self, _exit: StateExitType) -> Result<(), String> {
        Ok(())
    }
}
