#![allow(deprecated, unused_imports, dead_code)]

use super::attack::*;
use super::attack_machine::*;
use super::dead::*;
use super::dock::*;
use super::enter::*;
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

/// Face object state
///
/// C++ `AIFaceState` with `m_obj == true`. `onEnter` only caches whether the
/// current locomotor can turn in place (`minSpeed == 0`). `update` drives
/// `setLocomotorGoalOrientation` / `setLocomotorGoalPositionExplicit` until
/// the relative heading is within ~2° (`0.035` rad).
#[derive(Debug)]
pub struct AIFaceObjectState {
    pub(crate) base: State,
    can_turn_in_place: bool,
}

impl AIFaceObjectState {
    pub fn new(machine: &StateMachine) -> Self {
        Self {
            base: State::new(machine, "AIFaceObject"),
            can_turn_in_place: false,
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

    fn xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::xfer(self, xfer)
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
        let goal_id = self.base.get_machine_goal_object_id();
        if goal_id.is_none() {
            return Ok(StateReturnType::Failure);
        }
        self.can_turn_in_place = locomotor_can_turn_in_place(&owner_guard);
        Ok(StateReturnType::Continue)
    }

    fn classic_on_update(&mut self) -> Result<StateReturnType, String> {
        if dual_world_registry_unavailable() {
            return Ok(StateReturnType::Failure);
        }
        let owner = self
            .base
            .get_machine_owner()
            .ok_or_else(|| "face object missing owner".to_string())?;
        let Some(goal_id) = self.base.get_machine_goal_object_id() else {
            return Ok(StateReturnType::Failure);
        };
        let Some(goal_pos) = crate::object::registry::OBJECT_REGISTRY
            .with_object(goal_id, |guard| *guard.get_position())
        else {
            return Ok(StateReturnType::Failure);
        };
        face_towards(&owner, goal_pos, self.can_turn_in_place)
    }

    fn classic_on_exit(&mut self, _exit: StateExitType) -> Result<(), String> {
        Ok(())
    }

    fn classic_xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::xfer(self, xfer)
    }
}

/// Face position state
///
/// C++ `AIFaceState` with `m_obj == false`. Same timed turn as face-object.
#[derive(Debug)]
pub struct AIFacePositionState {
    pub(crate) base: State,
    can_turn_in_place: bool,
}

impl AIFacePositionState {
    pub fn new(machine: &StateMachine) -> Self {
        Self {
            base: State::new(machine, "AIFacePosition"),
            can_turn_in_place: false,
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

    fn xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::xfer(self, xfer)
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
        if dual_world_registry_unavailable() {
            return Ok(StateReturnType::Failure);
        }
        let owner = self
            .base
            .get_machine_owner()
            .ok_or_else(|| "face position missing owner".to_string())?;
        let owner_guard = owner
            .read()
            .map_err(|_| "face position owner lock poisoned".to_string())?;
        if self.base.get_machine_goal_position().is_none() {
            return Ok(StateReturnType::Failure);
        }
        self.can_turn_in_place = locomotor_can_turn_in_place(&owner_guard);
        Ok(StateReturnType::Continue)
    }

    fn classic_on_update(&mut self) -> Result<StateReturnType, String> {
        if dual_world_registry_unavailable() {
            return Ok(StateReturnType::Failure);
        }
        let owner = self
            .base
            .get_machine_owner()
            .ok_or_else(|| "face position missing owner".to_string())?;
        let Some(goal) = self.base.get_machine_goal_position() else {
            return Ok(StateReturnType::Failure);
        };
        face_towards(&owner, goal, self.can_turn_in_place)
    }

    fn classic_on_exit(&mut self, _exit: StateExitType) -> Result<(), String> {
        Ok(())
    }

    fn classic_xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::xfer(self, xfer)
    }
}

/// C++ `ThePartitionManager->getRelativeAngle2D` used by `AIFaceState::update`.
fn relative_angle_2d(owner_pos: &Coord3D, owner_orientation: f32, target_pos: &Coord3D) -> f32 {
    let angle_to_target = (target_pos.y - owner_pos.y).atan2(target_pos.x - owner_pos.x);
    let mut rel = angle_to_target - owner_orientation;
    const PI: f32 = std::f32::consts::PI;
    const TAU: f32 = std::f32::consts::TAU;
    while rel > PI {
        rel -= TAU;
    }
    while rel < -PI {
        rel += TAU;
    }
    rel
}

fn locomotor_can_turn_in_place(owner: &Object) -> bool {
    let Some(ai) = owner.get_ai_update_interface() else {
        return false;
    };
    let Ok(ai_guard) = ai.lock() else {
        return false;
    };
    let Some(locomotor) = ai_guard.get_cur_locomotor() else {
        return false;
    };
    locomotor
        .lock()
        .map(|loco| loco.template.min_speed == 0.0)
        .unwrap_or(false)
}

/// C++ `AIFaceState::update` — keep turning until within ~2°.
fn face_towards(
    owner: &Arc<RwLock<Object>>,
    target_pos: Coord3D,
    can_turn_in_place: bool,
) -> Result<StateReturnType, String> {
    const REL_THRESH: f32 = 0.035;
    let Ok(owner_guard) = owner.read() else {
        return Ok(StateReturnType::Failure);
    };
    let owner_pos = *owner_guard.get_position();
    let owner_orientation = owner_guard.get_orientation();
    let rel_angle = relative_angle_2d(&owner_pos, owner_orientation, &target_pos);
    if rel_angle.abs() < REL_THRESH {
        return Ok(StateReturnType::Success);
    }
    let Some(ai) = owner_guard.get_ai_update_interface() else {
        return Ok(StateReturnType::Failure);
    };
    drop(owner_guard);
    let Ok(mut ai_guard) = ai.lock() else {
        return Ok(StateReturnType::Failure);
    };
    if can_turn_in_place {
        ai_guard.set_locomotor_goal_orientation(owner_orientation + rel_angle);
    } else {
        ai_guard.set_locomotor_goal_position_explicit(target_pos);
    }
    Ok(StateReturnType::Continue)
}

/// C++ `AIFaceState::xfer` version 1: `m_canTurnInPlace`.
fn xfer_can_turn_in_place(xfer: &mut dyn Xfer, can_turn_in_place: &mut bool) -> Result<(), String> {
    let mut version: game_engine::common::system::xfer::XferVersion = 1;
    xfer.xfer_version(&mut version, 1)
        .map_err(|e| format!("AIFaceState xfer version failed: {:?}", e))?;
    xfer.xfer_bool(can_turn_in_place)
        .map_err(|e| format!("AIFaceState xfer canTurnInPlace failed: {:?}", e))?;
    Ok(())
}

impl Snapshotable for AIFaceObjectState {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut can_turn_in_place = self.can_turn_in_place;
        xfer_can_turn_in_place(xfer, &mut can_turn_in_place)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        xfer_can_turn_in_place(xfer, &mut self.can_turn_in_place)
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

impl Snapshotable for AIFacePositionState {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut can_turn_in_place = self.can_turn_in_place;
        xfer_can_turn_in_place(xfer, &mut can_turn_in_place)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        xfer_can_turn_in_place(xfer, &mut self.can_turn_in_place)
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}
