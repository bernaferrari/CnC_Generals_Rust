#![allow(deprecated, unused_imports, dead_code)]

use crate::modules::ai_state_runtime::AiStateRuntime;

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
    AIUpdateInterfaceExt, BodyModuleInterfaceExt, ContainModuleInterfaceExt, ContainWant,
    ExitDoorType, FAST_AS_POSSIBLE, PhysicsBehaviorExt,
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

/// Wander along a waypoint path.
#[derive(Debug)]
pub struct AIWanderState {
    pub(crate) base: State,
    pub(crate) move_to: AIMoveToState,
    pub(crate) core: FollowWaypointPathCore,
    pub(crate) wait_frames: i32,
    pub(crate) timer: i32,
}

impl AIWanderState {
    pub fn new(machine: &StateMachine) -> Self {
        let mut move_to = AIMoveToState::new(machine);
        // C++ inherits AIInternalMoveToState, not AIMoveToState mood conversion.
        move_to.is_move_to = false;
        Self {
            base: State::new(machine, "AIWander"),
            move_to,
            core: FollowWaypointPathCore::new(false, true),
            wait_frames: 0,
            timer: 0,
        }
    }

    pub(crate) fn update_group_offset(&mut self, ai: &dyn AiStateRuntime) {
        ai.with_cur_locomotor(&mut |loco| {
            let factor = loco.template.wander_width_factor;
            if factor > 0.0 {
                let mut delta = (factor + 0.5).floor() as i32;
                if delta < 1 {
                    delta = 1;
                }
                let x = get_game_logic_random_value(-delta, delta) as f32 * PATHFIND_CELL_SIZE_F;
                let y = get_game_logic_random_value(-delta, delta) as f32 * PATHFIND_CELL_SIZE_F;
                self.core.group_offset = Coord2D::new(x, y);
            }
        });
    }
}

impl StateImplementation for AIWanderState {
    fn on_enter(&mut self) -> StateReturnType {
        self.cpp_on_enter().unwrap_or(StateReturnType::Failure)
    }

    fn bind_goal_object_id(&mut self, id: crate::common::ObjectID) {
        self.base.goal_object_id = id;
        self.move_to.bind_goal_object_id(id);
    }

    fn bind_goal_position(&mut self, pos: Coord3D) {
        self.base.goal_position_copied = Some(pos);
        self.move_to.bind_goal_position(pos);
    }

    fn on_enter_with_waypoint(
        &mut self,
        ai: &mut dyn crate::modules::ai_state_runtime::AiStateRuntime,
        goal_id: crate::common::ObjectID,
        goal_pos: Coord3D,
        waypoint: Option<crate::waypoint::WaypointId>,
    ) -> StateReturnType {
        self.core.current_waypoint = waypoint.and_then(resolve_waypoint_by_id);
        self.core.prior_waypoint = None;
        self.core.group_offset = Coord2D::new(0.0, 0.0);
        let Some(owner) = self.base.get_machine_owner() else {
            return StateReturnType::Failure;
        };
        {
            let Ok(owner_guard) = owner.read() else {
                return StateReturnType::Failure;
            };
            if self.core.current_waypoint.is_none() {
                return StateReturnType::Failure;
            }
            self.update_group_offset(ai);
            self.timer = 0;
            self.wait_frames = 10 + ((owner_guard.get_id() & 0x7) as i32);
            if self
                .core
                .compute_goal(&self.base, &owner_guard, ai, false)
                .is_err()
            {
                return StateReturnType::Failure;
            }
        }
        let ret = self
            .move_to
            .on_enter_with_ai(ai, goal_id, self.core.goal_position);
        if let Ok(mut owner_guard) = owner.write() {
            owner_guard.ai_pending_path_extra = Some(self.core.calc_extra_path_distance());
        }
        ret
    }

    fn update(&mut self) -> StateReturnType {
        self.cpp_on_update().unwrap_or(StateReturnType::Failure)
    }

    fn update_with_ai(
        &mut self,
        ai: &mut dyn crate::modules::ai_state_runtime::AiStateRuntime,
    ) -> StateReturnType {
        let status = self.move_to.update_with_ai(ai);
        let Some(owner) = self.base.get_machine_owner() else {
            return StateReturnType::Failure;
        };
        let Ok(owner_guard) = owner.read() else {
            return StateReturnType::Failure;
        };
        if owner_guard.is_kind_of(KindOf::CanBeRepulsed) {
            self.timer -= 1;
            if self.timer < 0 {
                self.timer = self.wait_frames;
                let enemy_id = the_ai().read().ok().and_then(|store| {
                    store
                        .find_closest_repulsor(owner_guard.get_id(), owner_guard.get_vision_range())
                        .ok()
                        .flatten()
                });
                if enemy_id.is_some() {
                    return StateReturnType::Failure;
                }
            }
        }
        if status != StateReturnType::Continue {
            self.core.current_waypoint = self.core.get_next_waypoint(&self.base);
            if self.core.current_waypoint.is_none() {
                ai.set_completed_waypoint_id(
                    self.core
                        .prior_waypoint
                        .as_ref()
                        .map(|waypoint| waypoint.id),
                );
                return StateReturnType::Success;
            }
            self.update_group_offset(ai);
            if self
                .core
                .compute_goal(&self.base, &owner_guard, ai, false)
                .is_err()
            {
                return StateReturnType::Failure;
            }
            self.move_to.goal_position = self.core.goal_position;
            if self.core.compute_path(ai).is_err() {
                return StateReturnType::Failure;
            }
            return StateReturnType::Continue;
        }
        StateReturnType::Continue
    }

    fn on_exit(&mut self, _status: StateExitType) {
        let _ = self.cpp_on_exit(_status);
    }

    fn xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::xfer(self, xfer)
    }
}

impl CppState for AIWanderState {
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
        self.wander_enter(None)
    }

    fn cpp_on_enter_with_ai(
        &mut self,
        ai: &mut dyn crate::modules::ai_state_runtime::AiStateRuntime,
    ) -> Result<StateReturnType, String> {
        self.wander_enter(Some(ai))
    }

    fn cpp_on_update(&mut self) -> Result<StateReturnType, String> {
        self.wander_update(None)
    }

    fn cpp_on_update_with_ai(
        &mut self,
        ai: &mut dyn crate::modules::ai_state_runtime::AiStateRuntime,
    ) -> Result<StateReturnType, String> {
        self.wander_update(Some(ai))
    }

    fn cpp_on_exit(&mut self, exit: StateExitType) -> Result<(), String> {
        // C++ AIWanderState::onExit → AIFollowWaypointPathState::onExit → InternalMoveTo
        self.move_to.cpp_on_exit(exit)
    }
}

impl AIWanderState {
    fn wander_enter(
        &mut self,
        mut borrowed: Option<&mut dyn crate::modules::ai_state_runtime::AiStateRuntime>,
    ) -> Result<StateReturnType, String> {
        let machine = self.base.get_machine()?;
        let waypoint_id = machine
            .try_lock()
            .ok()
            .and_then(|guard| guard.get_goal_waypoint())
            .or(self.base.goal_waypoint_copied);
        self.core.current_waypoint = waypoint_id.and_then(resolve_waypoint_by_id);
        self.core.prior_waypoint = None;
        self.core.group_offset = Coord2D::new(0.0, 0.0);
        let owner = self
            .base
            .get_machine_owner()
            .ok_or_else(|| "wander missing owner".to_string())?;
        let has_ai = borrowed.is_some();
        {
            let owner_guard = owner
                .read()
                .map_err(|_| "wander owner lock poisoned".to_string())?;
            let ai_arc;
            let mut locked_ai;
            let ai_guard: &mut dyn crate::modules::ai_state_runtime::AiStateRuntime =
                if let Some(ai) = borrowed.as_mut() {
                    *ai
                } else {
                    ai_arc = owner_guard
                        .get_ai_update_interface()
                        .ok_or_else(|| "wander missing AiStateRuntime".to_string())?;
                    locked_ai = ai_arc
                        .lock()
                        .map_err(|_| "wander AI lock poisoned".to_string())?;
                    &mut crate::modules::ai_state_runtime::AiUpdateRuntimeAdapter(&mut *locked_ai)
                };
            if self.core.current_waypoint.is_none() {
                return Ok(StateReturnType::Failure);
            }
            self.update_group_offset(&*ai_guard);
            self.timer = 0;
            self.wait_frames = 10 + ((owner_guard.get_id() & 0x7) as i32);
            self.core
                .compute_goal(&self.base, &owner_guard, &mut *ai_guard, false)?;
        }
        let ret = if has_ai {
            self.move_to
                .cpp_on_enter_with_ai(borrowed.expect("wander ai"))?
        } else {
            self.move_to.cpp_on_enter()?
        };
        if let Ok(mut owner_guard) = owner.write() {
            owner_guard.ai_pending_path_extra = Some(self.core.calc_extra_path_distance());
        }
        Ok(ret)
    }

    fn wander_update(
        &mut self,
        mut borrowed: Option<&mut dyn crate::modules::ai_state_runtime::AiStateRuntime>,
    ) -> Result<StateReturnType, String> {
        let has_ai = borrowed.is_some();
        let status = if let Some(ai) = borrowed.as_mut() {
            self.move_to.cpp_on_update_with_ai(*ai)?
        } else {
            self.move_to.cpp_on_update()?
        };
        let owner = self
            .base
            .get_machine_owner()
            .ok_or_else(|| "wander missing owner".to_string())?;
        let owner_guard = owner
            .read()
            .map_err(|_| "wander owner lock poisoned".to_string())?;
        let ai_arc;
        let mut locked_ai;
        let ai_guard: &mut dyn crate::modules::ai_state_runtime::AiStateRuntime =
            if let Some(ai) = borrowed.as_mut() {
                *ai
            } else {
                ai_arc = owner_guard
                    .get_ai_update_interface()
                    .ok_or_else(|| "wander missing AiStateRuntime".to_string())?;
                locked_ai = ai_arc
                    .lock()
                    .map_err(|_| "wander AI lock poisoned".to_string())?;
                &mut crate::modules::ai_state_runtime::AiUpdateRuntimeAdapter(&mut *locked_ai)
            };
        let _ = has_ai;
        if owner_guard.is_kind_of(KindOf::CanBeRepulsed) {
            self.timer -= 1;
            if self.timer < 0 {
                self.timer = self.wait_frames;
                let ai_store = the_ai();
                let enemy_id = ai_store
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
        if status != StateReturnType::Continue {
            self.core.current_waypoint = self.core.get_next_waypoint(&self.base);
            if self.core.current_waypoint.is_none() {
                ai_guard.set_completed_waypoint_id(
                    self.core
                        .prior_waypoint
                        .as_ref()
                        .map(|waypoint| waypoint.id),
                );
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
}

/// Panic state - wander while panicking.
#[derive(Debug)]
pub struct AIPanicState {
    pub(crate) base: State,
    pub(crate) move_to: AIMoveToState,
    pub(crate) core: FollowWaypointPathCore,
    pub(crate) wait_frames: i32,
    pub(crate) timer: i32,
}

impl AIPanicState {
    pub fn new(machine: &StateMachine) -> Self {
        let mut move_to = AIMoveToState::new(machine);
        move_to.is_move_to = false;
        Self {
            base: State::new(machine, "AIPanic"),
            move_to,
            core: FollowWaypointPathCore::new(false, true),
            wait_frames: 0,
            timer: 0,
        }
    }

    pub(crate) fn update_group_offset(&mut self, ai: &dyn AiStateRuntime) {
        ai.with_cur_locomotor(&mut |loco| {
            let factor = loco.template.wander_width_factor;
            if factor > 0.0 {
                let mut delta = (factor + 0.5).floor() as i32;
                if delta < 1 {
                    delta = 1;
                }
                let x = get_game_logic_random_value(-delta, delta) as f32 * PATHFIND_CELL_SIZE_F;
                let y = get_game_logic_random_value(-delta, delta) as f32 * PATHFIND_CELL_SIZE_F;
                self.core.group_offset = Coord2D::new(x, y);
            }
        });
    }
}

impl StateImplementation for AIPanicState {
    fn on_enter(&mut self) -> StateReturnType {
        self.cpp_on_enter().unwrap_or(StateReturnType::Failure)
    }

    fn bind_goal_object_id(&mut self, id: crate::common::ObjectID) {
        self.base.goal_object_id = id;
        self.move_to.bind_goal_object_id(id);
    }

    fn bind_goal_position(&mut self, pos: Coord3D) {
        self.base.goal_position_copied = Some(pos);
        self.move_to.bind_goal_position(pos);
    }

    fn on_enter_with_waypoint(
        &mut self,
        ai: &mut dyn crate::modules::ai_state_runtime::AiStateRuntime,
        goal_id: crate::common::ObjectID,
        _goal_pos: Coord3D,
        waypoint: Option<crate::waypoint::WaypointId>,
    ) -> StateReturnType {
        self.core.current_waypoint = waypoint.and_then(resolve_waypoint_by_id);
        self.core.prior_waypoint = None;
        self.core.group_offset = Coord2D::new(0.0, 0.0);
        let Some(owner) = self.base.get_machine_owner() else {
            return StateReturnType::Failure;
        };
        {
            let Ok(owner_guard) = owner.read() else {
                return StateReturnType::Failure;
            };
            if self.core.current_waypoint.is_none() {
                return StateReturnType::Failure;
            }
            self.update_group_offset(ai);
            self.timer = 0;
            self.wait_frames = 10 + ((owner_guard.get_id() & 0x7) as i32);
            if self
                .core
                .compute_goal(&self.base, &owner_guard, ai, false)
                .is_err()
            {
                return StateReturnType::Failure;
            }
        }
        let ret = self
            .move_to
            .on_enter_with_ai(ai, goal_id, self.core.goal_position);
        if let Ok(mut owner_guard) = owner.write() {
            owner_guard.ai_pending_path_extra = Some(self.core.calc_extra_path_distance());
            owner_guard.set_model_condition_state(ModelConditionFlags::PANICKING);
        }
        ret
    }

    fn update(&mut self) -> StateReturnType {
        self.cpp_on_update().unwrap_or(StateReturnType::Failure)
    }

    fn update_with_ai(
        &mut self,
        ai: &mut dyn crate::modules::ai_state_runtime::AiStateRuntime,
    ) -> StateReturnType {
        let status = self.move_to.update_with_ai(ai);
        let Some(owner) = self.base.get_machine_owner() else {
            return StateReturnType::Failure;
        };
        let Ok(owner_guard) = owner.read() else {
            return StateReturnType::Failure;
        };
        if status != StateReturnType::Continue {
            self.core.current_waypoint = self.core.get_next_waypoint(&self.base);
            if self.core.current_waypoint.is_none() {
                ai.set_completed_waypoint_id(
                    self.core
                        .prior_waypoint
                        .as_ref()
                        .map(|waypoint| waypoint.id),
                );
                return StateReturnType::Success;
            }
            self.update_group_offset(ai);
            if self
                .core
                .compute_goal(&self.base, &owner_guard, ai, false)
                .is_err()
            {
                return StateReturnType::Failure;
            }
            self.move_to.goal_position = self.core.goal_position;
            if self.core.compute_path(ai).is_err() {
                return StateReturnType::Failure;
            }
        }
        StateReturnType::Continue
    }

    fn on_exit(&mut self, _status: StateExitType) {
        let _ = self.cpp_on_exit(_status);
    }

    fn xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::xfer(self, xfer)
    }
}

impl CppState for AIPanicState {
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
        self.panic_enter(None)
    }

    fn cpp_on_enter_with_ai(
        &mut self,
        ai: &mut dyn crate::modules::ai_state_runtime::AiStateRuntime,
    ) -> Result<StateReturnType, String> {
        self.panic_enter(Some(ai))
    }

    fn cpp_on_update(&mut self) -> Result<StateReturnType, String> {
        self.panic_update(None)
    }

    fn cpp_on_update_with_ai(
        &mut self,
        ai: &mut dyn crate::modules::ai_state_runtime::AiStateRuntime,
    ) -> Result<StateReturnType, String> {
        self.panic_update(Some(ai))
    }

    fn cpp_on_exit(&mut self, exit: StateExitType) -> Result<(), String> {
        if let Some(owner) = self.base.get_machine_owner() {
            if let Ok(mut owner_guard) = owner.write() {
                owner_guard.clear_model_condition_state(ModelConditionFlags::PANICKING);
            }
        }
        // C++ AIPanicState::onExit: clear PANICKING then AIInternalMoveToState::onExit
        self.move_to.cpp_on_exit(exit)
    }
}

impl AIPanicState {
    fn panic_enter(
        &mut self,
        mut borrowed: Option<&mut dyn crate::modules::ai_state_runtime::AiStateRuntime>,
    ) -> Result<StateReturnType, String> {
        let machine = self.base.get_machine()?;
        let waypoint_id = machine
            .try_lock()
            .ok()
            .and_then(|guard| guard.get_goal_waypoint())
            .or(self.base.goal_waypoint_copied);
        self.core.current_waypoint = waypoint_id.and_then(resolve_waypoint_by_id);
        self.core.prior_waypoint = None;
        self.core.group_offset = Coord2D::new(0.0, 0.0);
        let owner = self
            .base
            .get_machine_owner()
            .ok_or_else(|| "panic missing owner".to_string())?;
        let has_ai = borrowed.is_some();
        {
            let owner_guard = owner
                .read()
                .map_err(|_| "panic owner lock poisoned".to_string())?;
            let ai_arc;
            let mut locked_ai;
            let ai_guard: &mut dyn crate::modules::ai_state_runtime::AiStateRuntime =
                if let Some(ai) = borrowed.as_mut() {
                    *ai
                } else {
                    ai_arc = owner_guard
                        .get_ai_update_interface()
                        .ok_or_else(|| "panic missing AiStateRuntime".to_string())?;
                    locked_ai = ai_arc
                        .lock()
                        .map_err(|_| "panic AI lock poisoned".to_string())?;
                    &mut crate::modules::ai_state_runtime::AiUpdateRuntimeAdapter(&mut *locked_ai)
                };
            if self.core.current_waypoint.is_none() {
                return Ok(StateReturnType::Failure);
            }
            self.update_group_offset(&*ai_guard);
            self.core
                .compute_goal(&self.base, &owner_guard, &mut *ai_guard, false)?;
        }
        let ret = if has_ai {
            self.move_to
                .cpp_on_enter_with_ai(borrowed.expect("panic ai"))?
        } else {
            self.move_to.cpp_on_enter()?
        };
        self.timer = 0;
        self.wait_frames = 10
            + ((self
                .base
                .get_machine_owner()
                .and_then(|owner| owner.read().ok().map(|guard| guard.get_id()))
                .unwrap_or(0)
                & 0x7) as i32);
        if let Ok(mut owner_guard) = owner.write() {
            owner_guard.ai_pending_path_extra = Some(self.core.calc_extra_path_distance());
        }
        if let Ok(mut owner_write) = owner.write() {
            owner_write.set_model_condition_state(ModelConditionFlags::PANICKING);
        }
        Ok(ret)
    }

    fn panic_update(
        &mut self,
        mut borrowed: Option<&mut dyn crate::modules::ai_state_runtime::AiStateRuntime>,
    ) -> Result<StateReturnType, String> {
        let status = if let Some(ai) = borrowed.as_mut() {
            self.move_to.cpp_on_update_with_ai(*ai)?
        } else {
            self.move_to.cpp_on_update()?
        };
        let owner = self
            .base
            .get_machine_owner()
            .ok_or_else(|| "panic missing owner".to_string())?;
        let owner_guard = owner
            .read()
            .map_err(|_| "panic owner lock poisoned".to_string())?;
        let ai_arc;
        let mut locked_ai;
        let ai_guard: &mut dyn crate::modules::ai_state_runtime::AiStateRuntime =
            if let Some(ai) = borrowed.as_mut() {
                *ai
            } else {
                ai_arc = owner_guard
                    .get_ai_update_interface()
                    .ok_or_else(|| "panic missing AiStateRuntime".to_string())?;
                locked_ai = ai_arc
                    .lock()
                    .map_err(|_| "panic AI lock poisoned".to_string())?;
                &mut crate::modules::ai_state_runtime::AiUpdateRuntimeAdapter(&mut *locked_ai)
            };
        if owner_guard.is_kind_of(KindOf::CanBeRepulsed) {
            self.timer -= 1;
            if self.timer < 0 {
                self.timer = self.wait_frames;
                let ai_store = the_ai();
                let enemy_id = ai_store
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
        if status == StateReturnType::Success {
            self.core.current_waypoint = self.core.get_next_waypoint(&self.base);
            if self.core.current_waypoint.is_none() {
                ai_guard.set_completed_waypoint_id(
                    self.core
                        .prior_waypoint
                        .as_ref()
                        .map(|waypoint| waypoint.id),
                );
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
}

impl Snapshotable for AIWanderState {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: game_engine::common::system::xfer::XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to crc version: {:?}", e))?;

        self.move_to.crc(xfer)?;
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

        self.move_to.xfer(xfer)?;
        self.core.xfer(xfer)?;
        xfer.xfer_int(&mut self.wait_frames)
            .map_err(|e| format!("Failed to xfer wait_frames: {:?}", e))?;
        xfer.xfer_int(&mut self.timer)
            .map_err(|e| format!("Failed to xfer timer: {:?}", e))?;

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Snapshotable::load_post_process(&mut self.move_to)
    }
}

impl Snapshotable for AIPanicState {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: game_engine::common::system::xfer::XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to crc version: {:?}", e))?;

        self.move_to.crc(xfer)?;
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

        self.move_to.xfer(xfer)?;
        self.core.xfer(xfer)?;
        xfer.xfer_int(&mut self.wait_frames)
            .map_err(|e| format!("Failed to xfer wait_frames: {:?}", e))?;
        xfer.xfer_int(&mut self.timer)
            .map_err(|e| format!("Failed to xfer timer: {:?}", e))?;

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Snapshotable::load_post_process(&mut self.move_to)
    }
}
