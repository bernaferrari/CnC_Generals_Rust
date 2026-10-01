#![allow(deprecated, unused_imports, dead_code)]

use super::attack::*;
use super::attack_machine::*;
use super::dead::*;
use super::dock::*;
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
    AIUpdateInterface, AIUpdateInterfaceExt,
    ContainWant, ExitDoorType, FAST_AS_POSSIBLE,
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

/// Enter state - enter a transport or building
#[derive(Debug)]
pub struct AIEnterState {
    pub(crate) base: AIMoveToState,
    pub(crate) entry_to_clear: ObjectID,
    pub(crate) goal_position: Coord3D,
    /// Set when the caller already holds the state-machine mutex.
    pub(crate) preset_owner: Option<ObjectID>,
    pub(crate) preset_goal_id: ObjectID,
}

impl AIEnterState {
    pub fn new(machine: &StateMachine) -> Self {
        let mut base = AIMoveToState::new(machine);
        base.base.name = "AIEnter".to_string();
        Self {
            base,
            entry_to_clear: INVALID_ID,
            goal_position: Coord3D::new(0.0, 0.0, 0.0),
            preset_owner: None,
            preset_goal_id: INVALID_ID,
        }
    }
}

impl StateImplementation for AIEnterState {
    fn on_enter(&mut self) -> StateReturnType {
        self.classic_on_enter().unwrap_or(StateReturnType::Failure)
    }

    fn bind_goal_object_id(&mut self, id: crate::common::ObjectID) {
        self.base.base.goal_object_id = id;
    }

    fn bind_goal_position(&mut self, pos: Coord3D) {
        self.base.base.goal_position_copied = Some(pos);
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

impl ClassicState for AIEnterState {
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
        // Wave 257: empty dual-world → fail-closed state.
        if dual_world_registry_unavailable() {
            return Ok(StateReturnType::Failure);
        }

        self.entry_to_clear = INVALID_ID;

        let owner = self
            .base
            .base
            .get_machine_owner()
            .ok_or_else(|| "enter state missing machine owner".to_string())?;
        let goal_id = if self.preset_goal_id != INVALID_ID {
            self.preset_goal_id
        } else {
            self.base
                .base
                .get_machine_goal_object_id()
                .ok_or_else(|| "enter state missing goal object".to_string())?
        };
        let entered = crate::object::registry::OBJECT_REGISTRY.with_object_mut(owner, |owner_guard| {
        crate::object::registry::OBJECT_REGISTRY.with_object(goal_id, |goal_guard| {
            let cmd_source = owner_guard.ai_fire_last_command_source;
            if !TheActionManager::can_enter_object(
                &*owner_guard,
                goal_guard,
                cmd_source,
                CanEnterType::CheckCapacity,
            ) {
                return Err(StateReturnType::Failure);
            }
            let pos = *goal_guard.get_position();
            let id = goal_guard.get_id();
            if let Some(contain) = goal_guard.get_contain() {
                contain.on_object_wants_to_enter_or_exit(&*owner_guard, ContainWant::WantsToEnter);
            }
            let had = goal_guard.get_contain().is_some();
            owner_guard.ai_pending_ignore_id = Some(id);
            owner_guard.ai_pending_allow_invalid_position = Some(true);
            Ok((pos, id, had))
        })}).flatten().ok_or_else(|| "enter state missing goal object".to_string())?;
        let (pos, id, had_contain) = match entered {
            Err(code) => return Ok(code),
            Ok(v) => v,
        };
        self.goal_position = pos;
        if had_contain {
            self.entry_to_clear = id;
        }
        self.base.goal_position = self.goal_position;
        self.base.preset_owner = self.preset_owner;
        self.base.set_adjusts_destination(false);
        self.base.classic_on_enter()
    }

    fn classic_on_update(&mut self) -> Result<StateReturnType, String> {
        // Wave 257: empty dual-world → fail-closed state.
        if dual_world_registry_unavailable() {
            return Ok(StateReturnType::Failure);
        }

        let owner = self
            .base
            .base
            .get_machine_owner()
            .ok_or_else(|| "enter state missing machine owner".to_string())?;
        let goal_id = if self.preset_goal_id != INVALID_ID {
            self.preset_goal_id
        } else {
            self.base
                .base
                .get_machine_goal_object_id()
                .ok_or_else(|| "enter state missing goal object".to_string())?
        };
        enum EnterUpdate {
            Failure,
            Continue,
            Success,
            Proceed(Coord3D),
        }
        let step = crate::object::registry::OBJECT_REGISTRY.with_object_mut(owner, |owner_guard| {
            crate::object::registry::OBJECT_REGISTRY
            .with_object(goal_id, |goal_guard| {
                if goal_guard.get_contained_by().is_some()
                    && goal_guard.is_above_terrain()
                    && !owner_guard.is_above_terrain()
                {
                    return EnterUpdate::Failure;
                }
                let pos = *goal_guard.get_position();
                let cmd_source = owner_guard.ai_fire_last_command_source;
                if !TheActionManager::can_enter_object(
                    &*owner_guard,
                    goal_guard,
                    cmd_source,
                    CanEnterType::CheckCapacity,
                ) {
                    if owner_guard.relationship_to(goal_guard) == Relationship::Enemies {
                        let can_attack = owner_guard.get_able_to_attack_specific_object(
                            AbleToAttackType::NewTarget,
                            goal_guard,
                            cmd_source,
                        );
                        if matches!(
                            can_attack,
                            CanAttackResult::Possible | CanAttackResult::PossibleAfterMoving
                        ) {
                            if let Some(ai) = owner_guard.get_ai_update_interface() {
                                ai.ai_attack_object(
                                    goal_guard.get_id(),
                                    NO_MAX_SHOTS_LIMIT,
                                    cmd_source,
                                );
                            }
                            return EnterUpdate::Continue;
                        }
                    }
                    return EnterUpdate::Failure;
                }
                if owner_guard.is_disabled_by_type(DisabledType::Held) {
                    return EnterUpdate::Success;
                }
                owner_guard.ai_pending_goal_id = Some(goal_guard.get_id());
                EnterUpdate::Proceed(pos)
            })
        }).flatten()
            .ok_or_else(|| "enter state missing goal object".to_string())?;
        match step {
            EnterUpdate::Failure => return Ok(StateReturnType::Failure),
            EnterUpdate::Continue => return Ok(StateReturnType::Continue),
            EnterUpdate::Success => return Ok(StateReturnType::Success),
            EnterUpdate::Proceed(pos) => {
                self.goal_position = pos;
            }
        }

        let code = self.base.classic_on_update()?;

        if code == StateReturnType::Success {
            let cont = crate::object::registry::OBJECT_REGISTRY.with_object(owner, |owner_guard| {
                crate::object::registry::OBJECT_REGISTRY.with_object(goal_id, |goal_guard| {
                if goal_guard.is_above_terrain() && !owner_guard.is_above_terrain() {
                    return false;
                }
                let owner_pos = owner_guard.get_position();
                let goal_pos = goal_guard.get_position();
                let dx = owner_pos.x - goal_pos.x;
                let dy = owner_pos.y - goal_pos.y;
                let mut radius = goal_guard.get_geometry_info().get_minor_radius();
                if goal_guard.get_template_geometry_type() != Some(GeometryType::Box) {
                    radius = goal_guard.get_geometry_info().get_major_radius();
                }
                let close_enough = dx * dx + dy * dy < radius * radius;
                if close_enough {
                    if let Some(contain) = goal_guard.get_contain() {
                        contain.add_to_contain(&*owner_guard);
                    }
                }
                true
            })
            }).flatten();
            if cont == Some(false) {
                return Ok(StateReturnType::Continue);
            }
        }

        Ok(code)
    }

    fn classic_on_exit(&mut self, _exit: StateExitType) -> Result<(), String> {
        // Wave 257: empty dual-world → Ok(()).
        if dual_world_registry_unavailable() {
            return Ok(());
        }

        self.base.classic_on_exit(_exit)?;
        if let Some(owner) = self.base.base.get_machine_owner() {
            let entry = self.entry_to_clear;
            let _ = crate::object::registry::OBJECT_REGISTRY.with_object_mut(owner, |owner_guard| {
                owner_guard.ai_pending_clear_ignore = true;
                owner_guard.ai_pending_allow_invalid_position = Some(false);
                if entry != INVALID_ID {
                    crate::object::registry::OBJECT_REGISTRY.with_object(entry, |goal_guard| {
                        if let Some(contain) = goal_guard.get_contain() {
                            contain.on_object_wants_to_enter_or_exit(
                                &*owner_guard,
                                ContainWant::WantsNeither,
                            );
                        }
                    });
                }
            });
        }

        self.entry_to_clear = INVALID_ID;
        Ok(())
    }
}

/// Exit state - exit from transport or building
#[derive(Debug)]
pub struct AIExitState {
    pub(crate) base: State,
    pub(crate) entry_to_clear: ObjectID,
}

impl AIExitState {
    pub fn new(machine: &StateMachine) -> Self {
        Self {
            base: State::new(machine, "AIExit"),
            entry_to_clear: INVALID_ID,
        }
    }
}

impl StateImplementation for AIExitState {
    fn on_enter(&mut self) -> StateReturnType {
        self.classic_on_enter().unwrap_or(StateReturnType::Failure)
    }

    fn bind_goal_object_id(&mut self, id: crate::common::ObjectID) {
        self.base.goal_object_id = id;
    }

    fn bind_goal_position(&mut self, pos: Coord3D) {
        self.base.goal_position_copied = Some(pos);
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

impl ClassicState for AIExitState {
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

        self.entry_to_clear = INVALID_ID;

        let owner = self
            .base
            .get_machine_owner()
            .ok_or_else(|| "exit state missing machine owner".to_string())?;
        let goal_id = self
            .base
            .get_machine_goal_object_id()
            .ok_or_else(|| "exit state missing goal object".to_string())?;
        let cleared = crate::object::registry::OBJECT_REGISTRY.with_object(owner, |owner_guard| {
            crate::object::registry::OBJECT_REGISTRY
            .with_object(goal_id, |goal_guard| {
                if let Some(contain) = goal_guard.get_contain() {
                    contain.on_object_wants_to_enter_or_exit(&*owner_guard, ContainWant::WantsToExit);
                    Some(goal_guard.get_id())
                } else {
                    None
                }
            })
        }).flatten()
            .ok_or_else(|| "exit state missing goal object".to_string())?;
        if let Some(id) = cleared {
            self.entry_to_clear = id;
        }

        Ok(StateReturnType::Continue)
    }

    fn classic_on_update(&mut self) -> Result<StateReturnType, String> {
        // Wave 257: empty dual-world → fail-closed state.
        if dual_world_registry_unavailable() {
            return Ok(StateReturnType::Failure);
        }

        let owner = self
            .base
            .get_machine_owner()
            .ok_or_else(|| "exit state missing machine owner".to_string())?;
        let goal_id = self
            .base
            .get_machine_goal_object_id()
            .ok_or_else(|| "exit state missing goal object".to_string())?;
        let owner_id = owner;
        enum ExitStep<T> {
            Continue,
            Missing,
            Door(T, ExitDoorType),
        }
        let step = crate::object::registry::OBJECT_REGISTRY.with_object(owner, |owner_guard| {
            crate::object::registry::OBJECT_REGISTRY
            .with_object(goal_id, |goal_guard| {
                if let Some(goal_ai) = goal_guard.get_ai_update_interface() {
                    let Ok(goal_ai_guard) = goal_ai.try_lock() else {
                        return ExitStep::Continue;
                    };
                    if goal_ai_guard.get_ai_free_to_exit(&*owner_guard) == AIFreeToExitType::WaitToExit {
                        return ExitStep::Continue;
                    }
                }
                let Some(exit_interface) = goal_guard.get_contain_exit_interface() else {
                    return ExitStep::Missing;
                };
                let door = {
                    let Ok(mut exit_guard) = exit_interface.lock() else {
                        return ExitStep::Missing;
                    };
                    if exit_guard.is_exit_busy() {
                        return ExitStep::Continue;
                    }
                    exit_guard.reserve_door_for_exit(Some(&*owner_guard), Some(&*owner_guard))
                };
                ExitStep::Door(exit_interface, door)
            })
        }).flatten()
            .ok_or_else(|| "exit state missing goal object".to_string())?;
        let (exit_interface, exit_door) = match step {
            ExitStep::Continue => return Ok(StateReturnType::Continue),
            ExitStep::Missing => return Err("exit state missing contain exit interface".to_string()),
            ExitStep::Door(interface, door) => (interface, door),
        };
        if exit_door == ExitDoorType::NoneAvailable {
            return Ok(StateReturnType::Failure);
        }
        exit_interface
            .lock()
            .map_err(|_| "exit state exit interface lock poisoned".to_string())?
            .exit_object_via_door(owner_id, exit_door)
            .map_err(|err| format!("exit state exit_object_via_door failed: {}", err))?;

        Ok(StateReturnType::Success)
    }

    fn classic_on_exit(&mut self, _exit: StateExitType) -> Result<(), String> {
        // Wave 257: empty dual-world → Ok(()).
        if dual_world_registry_unavailable() {
            return Ok(());
        }

        if let Some(owner) = self.base.get_machine_owner() {
            let entry = self.entry_to_clear;
            if entry != INVALID_ID {
                let _ = crate::object::registry::OBJECT_REGISTRY.with_object(owner, |owner_guard| {
                    crate::object::registry::OBJECT_REGISTRY.with_object(entry, |goal_guard| {
                        if let Some(contain) = goal_guard.get_contain() {
                            contain.on_object_wants_to_enter_or_exit(
                                &*owner_guard,
                                ContainWant::WantsNeither,
                            );
                        }
                    });
                });
            }
        }

        self.entry_to_clear = INVALID_ID;
        Ok(())
    }
}

/// Exit instantly state - exit from transport or building immediately
#[derive(Debug)]
pub struct AIExitInstantlyState {
    pub(crate) base: State,
    pub(crate) entry_to_clear: ObjectID,
}

impl AIExitInstantlyState {
    pub fn new(machine: &StateMachine) -> Self {
        Self {
            base: State::new(machine, "AIExitInstantly"),
            entry_to_clear: INVALID_ID,
        }
    }
}

impl StateImplementation for AIExitInstantlyState {
    fn on_enter(&mut self) -> StateReturnType {
        self.classic_on_enter().unwrap_or(StateReturnType::Failure)
    }

    fn bind_goal_object_id(&mut self, id: crate::common::ObjectID) {
        self.base.goal_object_id = id;
    }

    fn bind_goal_position(&mut self, pos: Coord3D) {
        self.base.goal_position_copied = Some(pos);
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

impl ClassicState for AIExitInstantlyState {
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

        self.entry_to_clear = INVALID_ID;

        let owner = self
            .base
            .get_machine_owner()
            .ok_or_else(|| "exit instantly state missing machine owner".to_string())?;
        let goal_id = self
            .base
            .get_machine_goal_object_id()
            .ok_or_else(|| "exit instantly state missing goal object".to_string())?;
        let owner_id = owner;
        let exit_interface = crate::object::registry::OBJECT_REGISTRY.with_object(owner, |owner_guard| {
            crate::object::registry::OBJECT_REGISTRY
            .with_object(goal_id, |goal_guard| {
                let contain = goal_guard.get_contain()?;
                contain.on_object_wants_to_enter_or_exit(&*owner_guard, ContainWant::WantsToExit);
                goal_guard.get_contain_exit_interface()
            })
        }).flatten()
            .ok_or_else(|| "exit instantly state missing goal object".to_string())?;
        let Some(exit_interface) = exit_interface else {
            return Ok(StateReturnType::Failure);
        };
        self.entry_to_clear = goal_id;
        exit_interface
            .lock()
            .map_err(|_| "exit instantly exit interface lock poisoned".to_string())?
            .exit_object_via_door(owner_id, ExitDoorType::Door1)
            .map_err(|err| format!("exit instantly exit_object_via_door failed: {}", err))?;

        Ok(StateReturnType::Continue)
    }

    fn classic_on_update(&mut self) -> Result<StateReturnType, String> {
        Ok(StateReturnType::Success)
    }

    fn classic_on_exit(&mut self, _exit: StateExitType) -> Result<(), String> {
        // Wave 257: empty dual-world → Ok(()).
        if dual_world_registry_unavailable() {
            return Ok(());
        }

        if let Some(owner) = self.base.get_machine_owner() {
            let entry = self.entry_to_clear;
            if entry != INVALID_ID {
                let _ = crate::object::registry::OBJECT_REGISTRY.with_object(owner, |owner_guard| {
                    crate::object::registry::OBJECT_REGISTRY.with_object(entry, |goal_guard| {
                        if let Some(contain) = goal_guard.get_contain() {
                            contain.on_object_wants_to_enter_or_exit(
                                &*owner_guard,
                                ContainWant::WantsNeither,
                            );
                        }
                    });
                });
            }
        }

        self.entry_to_clear = INVALID_ID;
        Ok(())
    }
}

impl Snapshotable for AIExitState {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: game_engine::common::system::xfer::XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to crc version: {:?}", e))?;

        let mut entry_to_clear = self.entry_to_clear;
        xfer.xfer_object_id(&mut entry_to_clear)
            .map_err(|e| format!("Failed to crc entry_to_clear: {:?}", e))?;
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: game_engine::common::system::xfer::XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to xfer version: {:?}", e))?;

        xfer.xfer_object_id(&mut self.entry_to_clear)
            .map_err(|e| format!("Failed to xfer entry_to_clear: {:?}", e))?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

impl Snapshotable for AIExitInstantlyState {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: game_engine::common::system::xfer::XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to crc version: {:?}", e))?;

        let mut entry_to_clear = self.entry_to_clear;
        xfer.xfer_object_id(&mut entry_to_clear)
            .map_err(|e| format!("Failed to crc entry_to_clear: {:?}", e))?;
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: game_engine::common::system::xfer::XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to xfer version: {:?}", e))?;

        xfer.xfer_object_id(&mut self.entry_to_clear)
            .map_err(|e| format!("Failed to xfer entry_to_clear: {:?}", e))?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

impl Snapshotable for AIEnterState {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: game_engine::common::system::xfer::XferVersion = 2;
        xfer.xfer_version(&mut version, 2)
            .map_err(|e| format!("Failed to crc version: {:?}", e))?;

        if version >= 2 {
            self.base.crc(xfer)?;
        }

        let mut entry_to_clear = self.entry_to_clear;
        xfer.xfer_object_id(&mut entry_to_clear)
            .map_err(|e| format!("Failed to crc entry_to_clear: {:?}", e))?;
        let mut goal_position = self.goal_position.clone();
        xfer.xfer_coord3d(&mut goal_position);

        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: game_engine::common::system::xfer::XferVersion = 2;
        xfer.xfer_version(&mut version, 2)
            .map_err(|e| format!("Failed to xfer version: {:?}", e))?;

        if version >= 2 {
            self.base.xfer(xfer)?;
        }

        xfer.xfer_object_id(&mut self.entry_to_clear)
            .map_err(|e| format!("Failed to xfer entry_to_clear: {:?}", e))?;
        xfer.xfer_coord3d(&mut self.goal_position);
        if xfer.is_loading() {
            self.base.goal_position = self.goal_position;
        }

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Snapshotable::load_post_process(&mut self.base)
    }
}