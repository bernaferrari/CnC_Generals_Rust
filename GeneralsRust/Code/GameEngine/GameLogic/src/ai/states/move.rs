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

/// Move away from repulsors state
/// Matches C++ AIMoveAwayFromRepulsorsState from AIStates.cpp lines 2263-2312.
#[derive(Debug)]
pub struct AIMoveAwayFromRepulsorsState {
    pub(crate) base: AIMoveToState,
    pub(crate) goal_position: Coord3D,
    pub(crate) ok_to_repath_times: i32,
    pub(crate) check_for_path: bool,
    /// Set when `goal_position` should be copied onto the machine.
    pub(crate) goal_dirty: bool,
    /// Enter clears the machine goal object; a later path publish does not.
    pub(crate) goal_clear_object: bool,
}

impl AIMoveAwayFromRepulsorsState {
    pub fn new(machine: &StateMachine) -> Self {
        let mut base = AIMoveToState::new(machine);
        base.base.name = "AIMoveAwayFromRepulsors".to_string();
        Self {
            base,
            goal_position: Coord3D::new(0.0, 0.0, 0.0),
            ok_to_repath_times: 1,
            check_for_path: true,
            goal_dirty: false,
            goal_clear_object: false,
        }
    }
}

/// Wander around a point
/// Matches C++ AIWanderInPlaceState from AIStates.cpp lines 4617-4714.
#[derive(Debug)]
pub struct AIWanderInPlaceState {
    pub(crate) base: AIMoveToState,
    pub(crate) origin: Coord3D,
    pub(crate) goal_position: Coord3D,
    pub(crate) goal_dirty: bool,
    pub(crate) wait_frames: i32,
    pub(crate) timer: i32,
}

impl AIWanderInPlaceState {
    pub fn new(machine: &StateMachine) -> Self {
        let mut base = AIMoveToState::new(machine);
        base.base.name = "AIWanderInPlace".to_string();
        Self {
            base,
            origin: Coord3D::new(0.0, 0.0, 0.0),
            goal_position: Coord3D::new(0.0, 0.0, 0.0),
            goal_dirty: false,
            wait_frames: 0,
            timer: 0,
        }
    }

    pub(crate) fn choose_new_goal(&mut self, ai: &dyn AIUpdateInterface) {
        let mut delta = 3;
        ai.with_cur_locomotor(&mut |loco| {
            delta = ((loco.template.wander_about_point_radius / PATHFIND_CELL_SIZE_F) + 0.5).floor()
                as i32;
        });

        let offset_x = get_game_logic_random_value(-delta, delta) as f32 * PATHFIND_CELL_SIZE_F;
        let offset_y = get_game_logic_random_value(-delta, delta) as f32 * PATHFIND_CELL_SIZE_F;
        self.goal_position = self.origin;
        self.goal_position.x += offset_x;
        self.goal_position.y += offset_y;
        self.goal_dirty = true;
    }
}

impl StateImplementation for AIWanderInPlaceState {
    fn on_enter(&mut self) -> StateReturnType {
        self.cpp_on_enter().unwrap_or(StateReturnType::Failure)
    }

    fn bind_goal_object_id(&mut self, id: crate::common::ObjectID) {
        self.base.bind_goal_object_id(id);
    }

    fn bind_goal_position(&mut self, pos: Coord3D) {
        self.base.bind_goal_position(pos);
    }

    fn take_published_goal(&mut self) -> Option<(Coord3D, bool)> {
        if !self.goal_dirty {
            return None;
        }
        self.goal_dirty = false;
        Some((self.goal_position, false))
    }

    fn on_enter_with_ai(
        &mut self,
        ai: &mut dyn crate::modules::AIUpdateInterface,
        _goal_id: crate::common::ObjectID,
        _goal_pos: Coord3D,
    ) -> StateReturnType {
        let owner = match self.base.base.get_machine_owner() {
            Some(owner) => owner,
            None => return StateReturnType::Failure,
        };
        let Ok(owner_guard) = owner.read() else {
            return StateReturnType::Failure;
        };
        self.origin = *owner_guard.get_position();
        let _ = ai.choose_locomotor_set(LocomotorSetType::Wander);
        self.choose_new_goal(ai);
        self.timer = 0;
        self.wait_frames = 10 + ((owner_guard.get_id() & 0x7) as i32);
        self.base.goal_position = self.goal_position;
        drop(owner_guard);
        if let Ok(mut owner_guard) = owner.write() {
            owner_guard.set_model_condition_state(ModelConditionFlags::MOVING);
        }
        let Ok(owner_guard) = owner.read() else {
            return StateReturnType::Failure;
        };
        self.base
            .start_from_borrowed_ai(ai, &owner_guard)
            .map(|_| StateReturnType::Continue)
            .unwrap_or(StateReturnType::Failure)
    }

    fn update(&mut self) -> StateReturnType {
        self.cpp_on_update().unwrap_or(StateReturnType::Failure)
    }

    fn update_with_ai(
        &mut self,
        ai: &mut dyn crate::modules::AIUpdateInterface,
    ) -> StateReturnType {
        let Some(owner) = self.base.base.get_machine_owner() else {
            return StateReturnType::Failure;
        };
        let Ok(owner_guard) = owner.read() else {
            return StateReturnType::Failure;
        };
        let close_enough = {
            let mut __close = 5.0;
            ai.with_cur_locomotor(&mut |loco| __close = loco.get_close_enough_dist());
            __close
        };
        let arrived =
            !ai.is_waiting_for_path() && ai.get_locomotor_distance_to_goal() <= close_enough;
        if owner_guard.is_kind_of(KindOf::CanBeRepulsed) {
            self.timer -= 1;
            if self.timer < 0 {
                self.timer = self.wait_frames;
                let ai_store = the_ai();
                let enemy_id = ai_store
                    .read()
                    .ok()
                    .and_then(|store| {
                        store
                            .find_closest_repulsor(
                                owner_guard.get_id(),
                                owner_guard.get_vision_range(),
                            )
                            .ok()
                    })
                    .flatten();
                if enemy_id.is_some() {
                    return StateReturnType::Failure;
                }
            }
        }
        if arrived {
            drop(owner_guard);
            if let Ok(mut owner_guard) = owner.write() {
                owner_guard.clear_model_condition_state(ModelConditionFlags::MOVING);
                owner_guard.set_model_condition_state(ModelConditionFlags::MOVING);
            }
            self.choose_new_goal(ai);
            self.base.goal_position = self.goal_position;
            let Ok(owner_guard) = owner.read() else {
                return StateReturnType::Failure;
            };
            return self
                .base
                .start_from_borrowed_ai(ai, &owner_guard)
                .map(|_| StateReturnType::Continue)
                .unwrap_or(StateReturnType::Failure);
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

impl CppState for AIWanderInPlaceState {
    fn base_state(&self) -> &State {
        &self.base.base
    }

    fn base_state_mut(&mut self) -> &mut State {
        &mut self.base.base
    }

    fn cpp_xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::xfer(self, xfer)
    }

    fn cpp_on_enter(&mut self) -> Result<StateReturnType, String> {
        self.wander_in_place_enter(None)
    }

    fn cpp_on_enter_with_ai(
        &mut self,
        ai: &mut dyn crate::modules::AIUpdateInterface,
    ) -> Result<StateReturnType, String> {
        self.wander_in_place_enter(Some(ai))
    }

    fn cpp_on_update(&mut self) -> Result<StateReturnType, String> {
        self.wander_in_place_update(None)
    }

    fn cpp_on_update_with_ai(
        &mut self,
        ai: &mut dyn crate::modules::AIUpdateInterface,
    ) -> Result<StateReturnType, String> {
        self.wander_in_place_update(Some(ai))
    }

    fn cpp_on_exit(&mut self, _exit: StateExitType) -> Result<(), String> {
        self.base.cpp_on_exit(_exit)
    }
}

impl AIWanderInPlaceState {
    fn wander_in_place_enter(
        &mut self,
        mut borrowed: Option<&mut dyn crate::modules::AIUpdateInterface>,
    ) -> Result<StateReturnType, String> {
        let owner = self
            .base
            .base
            .get_machine_owner()
            .ok_or_else(|| "wander in place missing owner".to_string())?;
        let has_ai = borrowed.is_some();
        {
            let owner_guard = owner
                .read()
                .map_err(|_| "wander in place owner lock poisoned".to_string())?;
            self.origin = *owner_guard.get_position();
            self.timer = 0;
            self.wait_frames = 10 + ((owner_guard.get_id() & 0x7) as i32);
            if let Some(ai) = borrowed.as_mut() {
                let _ = ai.choose_locomotor_set(LocomotorSetType::Wander);
                self.choose_new_goal(*ai);
            } else {
                let held_ai = owner_guard
                    .get_ai_update_interface()
                    .ok_or_else(|| "wander in place missing AIUpdateInterface".to_string())?;
                if let Ok(mut ai_guard) = held_ai.lock() {
                    let _ = ai_guard.choose_locomotor_set(LocomotorSetType::Wander);
                    self.choose_new_goal(&*ai_guard);
                }
            }
        }
        if has_ai {
            self.base
                .cpp_on_enter_with_ai(borrowed.expect("wander in place ai"))
        } else {
            self.base.cpp_on_enter()
        }
    }

    fn wander_in_place_update(
        &mut self,
        mut borrowed: Option<&mut dyn crate::modules::AIUpdateInterface>,
    ) -> Result<StateReturnType, String> {
        let has_ai = borrowed.is_some();
        let status = if let Some(ai) = borrowed.as_mut() {
            self.base.cpp_on_update_with_ai(*ai)?
        } else {
            self.base.cpp_on_update()?
        };
        let owner = self
            .base
            .base
            .get_machine_owner()
            .ok_or_else(|| "wander in place missing owner".to_string())?;
        let repulsed = {
            let owner_guard = owner
                .read()
                .map_err(|_| "wander in place owner lock poisoned".to_string())?;
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
            if !has_ai {
                Some(
                    owner_guard
                        .get_ai_update_interface()
                        .ok_or_else(|| "wander in place missing AIUpdateInterface".to_string())?,
                )
            } else {
                None
            }
        };
        if status != StateReturnType::Continue {
            if let Some(ai) = borrowed.as_mut() {
                self.choose_new_goal(*ai);
                let _ = self.base.cpp_on_enter_with_ai(*ai);
            } else if let Some(ai) = repulsed {
                if let Ok(ai_guard) = ai.try_lock() {
                    self.choose_new_goal(&*ai_guard);
                    drop(ai_guard);
                    let _ = self.base.cpp_on_enter();
                }
            }
            return Ok(StateReturnType::Continue);
        }
        Ok(StateReturnType::Continue)
    }
}

/// Move out of the way state
/// Matches C++ AIMoveOutOfTheWayState from AIStates.cpp lines 2125-2168.
#[derive(Debug)]
pub struct AIMoveOutOfTheWayState {
    pub(crate) base: AIMoveToState,
    live_stuck: Option<bool>,
    /// Set when `base.goal_position` should be copied onto the machine.
    goal_dirty: bool,
}

impl AIMoveOutOfTheWayState {
    pub fn new(machine: &StateMachine) -> Self {
        let mut base = AIMoveToState::new(machine);
        base.base.name = "AIMoveOutOfTheWay".to_string();
        Self {
            base,
            live_stuck: None,
            goal_dirty: false,
        }
    }
}

impl StateImplementation for AIMoveOutOfTheWayState {
    fn on_enter(&mut self) -> StateReturnType {
        self.cpp_on_enter().unwrap_or(StateReturnType::Failure)
    }

    fn bind_goal_object_id(&mut self, id: crate::common::ObjectID) {
        self.base.bind_goal_object_id(id);
    }

    fn bind_goal_position(&mut self, pos: Coord3D) {
        self.base.bind_goal_position(pos);
    }

    fn take_published_goal(&mut self) -> Option<(Coord3D, bool)> {
        if !self.goal_dirty {
            return None;
        }
        self.goal_dirty = false;
        // Path destination only. Do not publish the pre-enter machine goal.
        Some((self.base.goal_position, false))
    }

    fn on_enter_with_ai(
        &mut self,
        ai: &mut dyn crate::modules::AIUpdateInterface,
        _goal_id: crate::common::ObjectID,
        _goal_pos: Coord3D,
    ) -> StateReturnType {
        let Some(goal) = ai.get_path_destination() else {
            return StateReturnType::Failure;
        };
        self.base.goal_position = goal;
        self.goal_dirty = true;
        if let Some(owner) = self.base.base.get_machine_owner() {
            if let Ok(mut owner_guard) = owner.write() {
                owner_guard.set_model_condition_state(ModelConditionFlags::MOVING);
            }
            if let Ok(owner_guard) = owner.read() {
                return self
                    .base
                    .start_from_borrowed_ai(ai, &owner_guard)
                    .map(|_| StateReturnType::Continue)
                    .unwrap_or(StateReturnType::Failure);
            }
        }
        StateReturnType::Failure
    }

    fn update(&mut self) -> StateReturnType {
        self.cpp_on_update().unwrap_or(StateReturnType::Failure)
    }

    fn update_with_ai(
        &mut self,
        ai: &mut dyn crate::modules::AIUpdateInterface,
    ) -> StateReturnType {
        let stuck = ai.is_blocked_and_stuck();
        if let Some(owner) = self.base.base.get_machine_owner() {
            if let Ok(mut owner_guard) = owner.write() {
                if owner_guard.is_effectively_dead() {
                    return StateReturnType::Success;
                }
                if stuck {
                    owner_guard.ai_pending_path_through_units = Some(true);
                }
            }
        }
        self.base.update_with_ai(ai)
    }

    fn on_exit(&mut self, _status: StateExitType) {
        let _ = self.cpp_on_exit(_status);
    }
}

impl CppState for AIMoveOutOfTheWayState {
    fn base_state(&self) -> &State {
        &self.base.base
    }

    fn base_state_mut(&mut self) -> &mut State {
        &mut self.base.base
    }

    fn cpp_on_enter(&mut self) -> Result<StateReturnType, String> {
        self.move_aside_enter(None)
    }

    fn cpp_on_enter_with_ai(
        &mut self,
        ai: &mut dyn crate::modules::AIUpdateInterface,
    ) -> Result<StateReturnType, String> {
        self.move_aside_enter(Some(ai))
    }

    fn cpp_on_update(&mut self) -> Result<StateReturnType, String> {
        self.move_aside_update(None)
    }

    fn cpp_on_update_with_ai(
        &mut self,
        ai: &mut dyn crate::modules::AIUpdateInterface,
    ) -> Result<StateReturnType, String> {
        self.move_aside_update(Some(ai))
    }

    fn cpp_on_exit_with_ai(
        &mut self,
        exit: StateExitType,
        ai: &mut dyn AIUpdateInterface,
    ) -> Result<(), String> {
        // Generic temporary entry can fail before MoveOut's native path exists.
        // Its own C++ exit still destroys path, unlike base MoveTo's exit.
        self.base.cpp_on_exit_with_ai(exit, ai)?;
        ai.destroy_path();
        ai.set_can_path_through_units(false)
            .map_err(|e| e.to_string())?;
        ai.clear_move_out_of_way();
        Ok(())
    }

    fn cpp_on_exit(&mut self, _exit: StateExitType) -> Result<(), String> {
        self.base.cpp_on_exit(_exit)?;
        if let Some(owner) = self.base.base.get_machine_owner() {
            if let Ok(mut owner_guard) = owner.write() {
                owner_guard.ai_pending_destroy_path = true;
                owner_guard.ai_pending_path_through_units = Some(false);
                owner_guard.ai_pending_clear_move_out = true;
            }
        }
        Ok(())
    }
}

impl AIMoveOutOfTheWayState {
    fn move_aside_enter(
        &mut self,
        mut borrowed: Option<&mut dyn crate::modules::AIUpdateInterface>,
    ) -> Result<StateReturnType, String> {
        self.base.set_adjusts_destination(true);
        let has_ai = borrowed.is_some();
        let goal = if let Some(ai) = borrowed.as_mut() {
            ai.get_path_destination()
                .ok_or_else(|| "move out of the way missing path destination".to_string())?
        } else {
            let owner = self
                .base
                .base
                .get_machine_owner()
                .ok_or_else(|| "move out of the way missing owner".to_string())?;
            let owner_guard = owner
                .read()
                .map_err(|_| "move out of the way owner lock poisoned".to_string())?;
            let ai = owner_guard
                .get_ai_update_interface()
                .ok_or_else(|| "move out of the way missing AIUpdateInterface".to_string())?;
            let ai_guard = ai
                .lock()
                .map_err(|_| "move out of the way AI lock poisoned".to_string())?;
            ai_guard
                .get_path_destination()
                .ok_or_else(|| "move out of the way missing path destination".to_string())?
        };
        // Machine is already locked on the update that entered this state.
        // Parent enter reads the copied goal while that lock is held.
        self.base.goal_position = goal;
        self.base.base.goal_position_copied = Some(goal);
        self.goal_dirty = true;
        let status = if has_ai {
            self.base.cpp_on_enter_with_ai(borrowed.expect("has_ai"))
        } else {
            self.base.cpp_on_enter()
        };
        // enter_move copies the still-unchanged machine goal. Publish the
        // path destination, not that origin.
        self.base.goal_position = goal;
        self.goal_dirty = true;
        status
    }

    fn move_aside_update(
        &mut self,
        borrowed: Option<&mut dyn crate::modules::AIUpdateInterface>,
    ) -> Result<StateReturnType, String> {
        let live_stuck = self.live_stuck.take();
        let owner = self
            .base
            .base
            .get_machine_owner()
            .ok_or_else(|| "move out of the way missing owner".to_string())?;
        if let Ok(mut owner_guard) = owner.write() {
            if owner_guard.is_effectively_dead() {
                return Ok(StateReturnType::Success);
            }
            if live_stuck.unwrap_or(owner_guard.ai_fire_blocked_and_stuck) {
                owner_guard.ai_pending_path_through_units = Some(true);
            }
        }
        if let Some(ai) = borrowed {
            self.base.cpp_on_update_with_ai(ai)
        } else {
            self.base.cpp_on_update()
        }
    }
}

/// Move and tighten state
/// Matches C++ AIMoveAndTightenState from AIStates.cpp lines 2181-2250.
#[derive(Debug)]
pub struct AIMoveAndTightenState {
    pub(crate) base: AIMoveToState,
    pub(crate) ok_to_repath_times: i32,
    pub(crate) check_for_path: bool,
}

impl AIMoveAndTightenState {
    pub fn new(machine: &StateMachine) -> Self {
        let mut base = AIMoveToState::new(machine);
        base.base.name = "AIMoveAndTighten".to_string();
        Self {
            base,
            ok_to_repath_times: 1,
            check_for_path: true,
        }
    }
}

impl StateImplementation for AIMoveAndTightenState {
    fn on_enter(&mut self) -> StateReturnType {
        self.cpp_on_enter().unwrap_or(StateReturnType::Failure)
    }

    fn bind_goal_object_id(&mut self, id: crate::common::ObjectID) {
        self.base.bind_goal_object_id(id);
    }

    fn bind_goal_position(&mut self, pos: Coord3D) {
        self.base.bind_goal_position(pos);
    }

    fn on_enter_with_ai(
        &mut self,
        ai: &mut dyn crate::modules::AIUpdateInterface,
        goal_id: crate::common::ObjectID,
        goal_pos: Coord3D,
    ) -> StateReturnType {
        self.base.set_adjusts_destination(false);
        self.ok_to_repath_times = 1;
        self.check_for_path = true;
        self.base.set_repath_limit(1, true);
        if goal_id != crate::common::INVALID_ID {
            if let Some(goal_obj) = crate::helpers::TheGameLogic::find_object_by_id(goal_id)
                .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(goal_id))
            {
                if let Ok(goal_guard) = goal_obj.read() {
                    self.base.goal_position = *goal_guard.get_position();
                } else {
                    self.base.goal_position = goal_pos;
                }
            } else {
                self.base.goal_position = goal_pos;
            }
        } else {
            self.base.goal_position = goal_pos;
        }
        let Some(owner) = self.base.base.get_machine_owner() else {
            return StateReturnType::Failure;
        };
        if let Ok(mut owner_guard) = owner.write() {
            if owner_guard.test_status(ObjectStatusTypes::Immobile) {
                return StateReturnType::Failure;
            }
            owner_guard.set_model_condition_state(ModelConditionFlags::MOVING);
        }
        let Ok(owner_guard) = owner.read() else {
            return StateReturnType::Failure;
        };
        self.base
            .start_from_borrowed_ai(ai, &owner_guard)
            .map(|_| StateReturnType::Continue)
            .unwrap_or(StateReturnType::Failure)
    }

    fn update(&mut self) -> StateReturnType {
        self.cpp_on_update().unwrap_or(StateReturnType::Failure)
    }

    fn update_with_ai(
        &mut self,
        ai: &mut dyn crate::modules::AIUpdateInterface,
    ) -> StateReturnType {
        if self.check_for_path && !ai.is_waiting_for_path() && ai.get_path_destination().is_some() {
            self.base.set_adjusts_destination(true);
            self.check_for_path = false;
            if let Some(owner) = self.base.base.get_machine_owner() {
                if let Ok(owner_guard) = owner.read() {
                    let _ = self.base.start_from_borrowed_ai(ai, &owner_guard);
                }
            }
        }
        let close_enough = {
            let mut __close = 5.0;
            ai.with_cur_locomotor(&mut |loco| __close = loco.get_close_enough_dist());
            __close
        };
        if !ai.is_waiting_for_path() && ai.get_locomotor_distance_to_goal() <= close_enough {
            if let Some(owner) = self.base.base.get_machine_owner() {
                if let Ok(mut owner_guard) = owner.write() {
                    owner_guard.clear_model_condition_state(ModelConditionFlags::MOVING);
                }
            }
            return StateReturnType::Success;
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

impl CppState for AIMoveAndTightenState {
    fn base_state(&self) -> &State {
        &self.base.base
    }

    fn base_state_mut(&mut self) -> &mut State {
        &mut self.base.base
    }

    fn cpp_xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::xfer(self, xfer)
    }

    fn cpp_on_enter(&mut self) -> Result<StateReturnType, String> {
        self.base.set_adjusts_destination(false);
        self.ok_to_repath_times = 1;
        self.check_for_path = true;
        self.base.set_repath_limit(1, true);
        self.base.cpp_on_enter()
    }

    fn cpp_on_update(&mut self) -> Result<StateReturnType, String> {
        if self.check_for_path {
            if let Some(owner) = self.base.base.get_machine_owner() {
                if let Ok(owner_guard) = owner.read() {
                    if owner_guard.ai_fire_has_path_destination
                        && !owner_guard.ai_fire_waiting_for_path
                    {
                        self.base.set_adjusts_destination(true);
                        self.check_for_path = false;
                    }
                }
            }
        }

        self.base.cpp_on_update()
    }

    fn cpp_on_exit(&mut self, _exit: StateExitType) -> Result<(), String> {
        self.base.cpp_on_exit(_exit)
    }
}

/// Move and delete state - move to a position then destroy self.
#[derive(Debug)]
pub struct AIMoveAndDeleteState {
    pub(crate) base: AIMoveToState,
}

impl AIMoveAndDeleteState {
    pub fn new(machine: &StateMachine) -> Self {
        let mut base = AIMoveToState::new(machine);
        base.base.name = "AIMoveAndDelete".to_string();
        Self { base }
    }
}

impl StateImplementation for AIMoveAndDeleteState {
    fn on_enter(&mut self) -> StateReturnType {
        self.cpp_on_enter().unwrap_or(StateReturnType::Failure)
    }

    fn bind_goal_object_id(&mut self, id: crate::common::ObjectID) {
        self.base.bind_goal_object_id(id);
    }

    fn bind_goal_position(&mut self, pos: Coord3D) {
        self.base.bind_goal_position(pos);
    }

    fn on_enter_with_ai(
        &mut self,
        ai: &mut dyn crate::modules::AIUpdateInterface,
        goal_id: crate::common::ObjectID,
        goal_pos: Coord3D,
    ) -> StateReturnType {
        self.base.set_adjusts_destination(true);
        if goal_id != crate::common::INVALID_ID {
            if let Some(goal_obj) = crate::helpers::TheGameLogic::find_object_by_id(goal_id)
                .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(goal_id))
            {
                if let Ok(goal_guard) = goal_obj.read() {
                    self.base.goal_position = *goal_guard.get_position();
                } else {
                    self.base.goal_position = goal_pos;
                }
            } else {
                self.base.goal_position = goal_pos;
            }
        } else {
            self.base.goal_position = goal_pos;
        }
        let Some(owner) = self.base.base.get_machine_owner() else {
            return StateReturnType::Failure;
        };
        if let Ok(mut owner_guard) = owner.write() {
            if owner_guard.test_status(ObjectStatusTypes::Immobile) {
                return StateReturnType::Failure;
            }
            owner_guard.set_model_condition_state(ModelConditionFlags::MOVING);
        }
        let Ok(owner_guard) = owner.read() else {
            return StateReturnType::Failure;
        };
        self.base
            .start_from_borrowed_ai(ai, &owner_guard)
            .map(|_| StateReturnType::Continue)
            .unwrap_or(StateReturnType::Failure)
    }

    fn update(&mut self) -> StateReturnType {
        self.cpp_on_update().unwrap_or(StateReturnType::Failure)
    }

    fn update_with_ai(
        &mut self,
        ai: &mut dyn crate::modules::AIUpdateInterface,
    ) -> StateReturnType {
        let Some(owner) = self.base.base.get_machine_owner() else {
            return StateReturnType::Failure;
        };
        let Ok(owner_guard) = owner.read() else {
            return StateReturnType::Failure;
        };
        if owner_guard.is_effectively_dead() {
            return StateReturnType::Failure;
        }
        let close_enough = {
            let mut __close = 5.0;
            ai.with_cur_locomotor(&mut |loco| __close = loco.get_close_enough_dist());
            __close
        };
        if ai.is_waiting_for_path() || ai.get_locomotor_distance_to_goal() > close_enough {
            return StateReturnType::Continue;
        }
        let owner_id = owner_guard.get_id();
        drop(owner_guard);
        let _ = TheGameLogic::destroy_object_by_id(owner_id);
        StateReturnType::Success
    }

    fn on_exit(&mut self, _status: StateExitType) {
        let _ = self.cpp_on_exit(_status);
    }

    fn xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::xfer(self, xfer)
    }
}

impl CppState for AIMoveAndDeleteState {
    fn base_state(&self) -> &State {
        &self.base.base
    }

    fn base_state_mut(&mut self) -> &mut State {
        &mut self.base.base
    }

    fn cpp_xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::xfer(self, xfer)
    }

    fn cpp_on_enter(&mut self) -> Result<StateReturnType, String> {
        self.base.set_adjusts_destination(true);
        self.base.cpp_on_enter()
    }

    fn cpp_on_update(&mut self) -> Result<StateReturnType, String> {
        let status = self.base.cpp_on_update()?;
        if status != StateReturnType::Continue {
            let owner = self
                .base
                .base
                .get_machine_owner()
                .ok_or_else(|| "move+delete missing owner".to_string())?;
            let owner_guard = owner
                .read()
                .map_err(|_| "move+delete owner lock poisoned".to_string())?;
            if owner_guard.is_effectively_dead() {
                return Ok(StateReturnType::Failure);
            }
            let owner_id = owner_guard.get_id();
            drop(owner_guard);
            let _ = TheGameLogic::destroy_object_by_id(owner_id);
        }
        Ok(status)
    }

    fn cpp_on_exit(&mut self, _exit: StateExitType) -> Result<(), String> {
        self.base.cpp_on_exit(_exit)
    }
}

impl StateImplementation for AIMoveAwayFromRepulsorsState {
    fn on_enter(&mut self) -> StateReturnType {
        self.cpp_on_enter().unwrap_or(StateReturnType::Failure)
    }

    fn bind_goal_object_id(&mut self, id: crate::common::ObjectID) {
        self.base.bind_goal_object_id(id);
    }

    fn bind_goal_position(&mut self, pos: Coord3D) {
        self.base.bind_goal_position(pos);
    }

    fn take_published_goal(&mut self) -> Option<(Coord3D, bool)> {
        if !self.goal_dirty {
            return None;
        }
        self.goal_dirty = false;
        let clear_object = self.goal_clear_object;
        self.goal_clear_object = false;
        Some((self.goal_position, clear_object))
    }

    fn note_step_owner(&mut self, owner: std::sync::Arc<std::sync::RwLock<crate::object::Object>>) {
        self.base.base.bind_owner(&owner);
    }

    fn on_enter_with_ai(
        &mut self,
        ai: &mut dyn crate::modules::AIUpdateInterface,
        _goal_id: crate::common::ObjectID,
        _goal_pos: Coord3D,
    ) -> StateReturnType {
        if dual_world_registry_unavailable() {
            return StateReturnType::Failure;
        }
        self.base.set_adjusts_destination(false);
        let Some(owner) = self.base.base.get_machine_owner() else {
            return StateReturnType::Failure;
        };
        let Ok(owner_guard) = owner.read() else {
            return StateReturnType::Failure;
        };
        let ai_store = the_ai();
        let Some(enemy_id) = ai_store
            .read()
            .ok()
            .and_then(|store| {
                store
                    .find_closest_repulsor(owner_guard.get_id(), owner_guard.get_vision_range())
                    .ok()
            })
            .flatten()
        else {
            return StateReturnType::Failure;
        };
        let Some(enemy) = get_legacy_object(enemy_id) else {
            return StateReturnType::Failure;
        };
        let Ok(enemy_guard) = enemy.read() else {
            return StateReturnType::Failure;
        };
        let _ = ai.choose_locomotor_set(LocomotorSetType::Panic);
        let has_safe_path = ai.request_safe_path(enemy_id).unwrap_or(false);
        let owner_pos = *owner_guard.get_position();
        let enemy_pos = *enemy_guard.get_position();
        drop(enemy_guard);
        drop(owner_guard);
        if let Ok(mut owner_mut) = owner.write() {
            owner_mut.set_model_condition_state(ModelConditionFlags::PANICKING);
        }
        self.goal_position = if has_safe_path {
            owner_pos
        } else {
            let mut dx = owner_pos.x - enemy_pos.x;
            let mut dy = owner_pos.y - enemy_pos.y;
            let len = (dx * dx + dy * dy).sqrt();
            if len < 0.001 {
                dx = 1.0;
                dy = 0.0;
            } else {
                dx /= len;
                dy /= len;
            }
            let flee_dist = {
                let Ok(owner_guard) = owner.read() else {
                    return StateReturnType::Failure;
                };
                owner_guard.get_vision_range()
            };
            Coord3D::new(
                owner_pos.x + dx * flee_dist,
                owner_pos.y + dy * flee_dist,
                owner_pos.z,
            )
        };
        self.base.goal_position = self.goal_position;
        self.goal_dirty = true;
        self.goal_clear_object = true;
        self.ok_to_repath_times = 1;
        self.check_for_path = true;
        self.base.set_repath_limit(1, false);
        let Ok(owner_guard) = owner.read() else {
            return StateReturnType::Failure;
        };
        self.base
            .start_from_borrowed_ai(ai, &owner_guard)
            .map(|_| StateReturnType::Continue)
            .unwrap_or(StateReturnType::Failure)
    }

    fn update(&mut self) -> StateReturnType {
        self.cpp_on_update().unwrap_or(StateReturnType::Failure)
    }

    fn update_with_ai(
        &mut self,
        ai: &mut dyn crate::modules::AIUpdateInterface,
    ) -> StateReturnType {
        let waiting = ai.is_waiting_for_path();
        if self.check_for_path && !waiting {
            if let Some(dest) = ai.get_path_destination() {
                self.goal_position = dest;
                self.base.goal_position = dest;
                self.goal_dirty = true;
                self.goal_clear_object = false;
                self.base.set_adjusts_destination(false);
                self.check_for_path = false;
                if let Some(owner) = self.base.base.get_machine_owner() {
                    if let Ok(owner_guard) = owner.read() {
                        let _ = self.base.start_from_borrowed_ai(ai, &owner_guard);
                    }
                }
            }
        }
        let close_enough = {
            let mut __close = 5.0;
            ai.with_cur_locomotor(&mut |loco| __close = loco.get_close_enough_dist());
            __close
        };
        if !waiting && ai.get_locomotor_distance_to_goal() <= close_enough {
            if let Some(owner) = self.base.base.get_machine_owner() {
                if let Ok(mut owner_guard) = owner.write() {
                    owner_guard.clear_model_condition_state(ModelConditionFlags::MOVING);
                }
            }
            return StateReturnType::Success;
        }
        StateReturnType::Continue
    }

    fn on_exit(&mut self, _status: StateExitType) {
        let _ = self.cpp_on_exit(_status);
    }
}

impl CppState for AIMoveAwayFromRepulsorsState {
    fn base_state(&self) -> &State {
        &self.base.base
    }

    fn base_state_mut(&mut self) -> &mut State {
        &mut self.base.base
    }

    fn cpp_on_enter(&mut self) -> Result<StateReturnType, String> {
        // Wave 257: empty dual-world → fail-closed state.
        if dual_world_registry_unavailable() {
            return Ok(StateReturnType::Failure);
        }

        self.base.set_adjusts_destination(false);

        let owner = self
            .base
            .base
            .get_machine_owner()
            .ok_or_else(|| "move away from repulsors missing owner".to_string())?;
        let owner_guard = owner
            .read()
            .map_err(|_| "move away from repulsors owner lock poisoned".to_string())?;

        let ai_store = the_ai();
        let enemy_id = ai_store
            .read()
            .ok()
            .and_then(|ai| {
                ai.find_closest_repulsor(owner_guard.get_id(), owner_guard.get_vision_range())
                    .ok()
            })
            .flatten()
            .ok_or_else(|| "move away from repulsors missing enemy".to_string())?;
        let enemy = get_legacy_object(enemy_id)
            .ok_or_else(|| "move away from repulsors missing enemy object".to_string())?;
        let enemy_guard = enemy
            .read()
            .map_err(|_| "move away from repulsors enemy lock poisoned".to_string())?;

        let mut has_safe_path = false;
        if let Some(ai) = owner_guard.get_ai_update_interface() {
            if let Ok(mut ai_guard) = ai.lock() {
                let _ = ai_guard.choose_locomotor_set(LocomotorSetType::Panic);
                has_safe_path = ai_guard.request_safe_path(enemy_id).unwrap_or(false);
            }
        }

        if let Ok(mut owner_mut) = owner.write() {
            owner_mut.set_model_condition_state(ModelConditionFlags::PANICKING);
        }

        let owner_pos = *owner_guard.get_position();
        let enemy_pos = *enemy_guard.get_position();
        drop(enemy_guard);

        if has_safe_path {
            self.goal_position = owner_pos;
        } else {
            let mut dx = owner_pos.x - enemy_pos.x;
            let mut dy = owner_pos.y - enemy_pos.y;
            let len = (dx * dx + dy * dy).sqrt();
            if len < 0.001 {
                dx = 1.0;
                dy = 0.0;
            } else {
                dx /= len;
                dy /= len;
            }

            let flee_dist = owner_guard.get_vision_range();
            self.goal_position = Coord3D::new(
                owner_pos.x + dx * flee_dist,
                owner_pos.y + dy * flee_dist,
                owner_pos.z,
            );
        }

        self.base.goal_position = self.goal_position;
        self.base.base.goal_position_copied = Some(self.goal_position);
        self.base.base.goal_object_id = INVALID_ID;
        self.goal_dirty = true;
        self.goal_clear_object = true;

        self.ok_to_repath_times = 1;
        self.check_for_path = true;
        self.base.set_repath_limit(1, false);

        let status = self.base.cpp_on_enter();
        self.base.goal_position = self.goal_position;
        self.goal_dirty = true;
        self.goal_clear_object = true;
        status
    }

    fn cpp_on_update(&mut self) -> Result<StateReturnType, String> {
        if let Some(owner) = self.base.base.get_machine_owner() {
            if let Ok(owner_guard) = owner.read() {
                if self.check_for_path
                    && !owner_guard.ai_fire_waiting_for_path
                    && let Some(dest) = owner_guard.ai_fire_path_destination
                {
                    self.goal_position = dest;
                    self.base.goal_position = dest;
                    self.goal_dirty = true;
                    self.goal_clear_object = false;
                    self.base.set_adjusts_destination(false);
                    self.check_for_path = false;
                }
            }
        }

        self.base.cpp_on_update()
    }

    fn cpp_on_exit(&mut self, _exit: StateExitType) -> Result<(), String> {
        self.base.cpp_on_exit(_exit)?;
        if let Some(owner) = self.base.base.get_machine_owner() {
            if let Ok(mut owner_guard) = owner.write() {
                owner_guard.clear_model_condition_state(ModelConditionFlags::PANICKING);
            }
        }
        Ok(())
    }
}

/// Move to state - move to a specific position or object
/// Matches C++ AIMoveToState from AIStates.cpp lines 1992-2115
#[derive(Debug)]
pub struct AIMoveToState {
    pub(crate) base: State,
    /// Goal position to move to
    pub(crate) goal_position: Coord3D,
    /// Last path goal position (for detecting if goal moved)
    pub(crate) path_goal_position: Coord3D,
    /// Timestamp when path was computed
    pub(crate) path_timestamp: u32,
    /// Timestamp when blocked repath occurred
    pub(crate) blocked_repath_timestamp: u32,
    /// Whether to adjust destinations for pathfinding
    pub(crate) adjust_destinations: bool,
    /// Optional override for adjust-destination behavior (used by Enter/Exit)
    pub(crate) adjust_destinations_override: Option<bool>,
    /// Whether waiting for pathfinder
    pub(crate) waiting_for_path: bool,
    /// Whether we can try one more repath
    pub(crate) try_one_more_repath: bool,
    /// Goal layer for movement
    pub(crate) goal_layer: u32,
    /// Whether this is truly a MoveTo (vs child class like AttackMove)
    pub(crate) is_move_to: bool,
    /// Handle for looping move sound
    pub(crate) ambient_playing_handle: u32,
    /// Optional repath limiter for derived states.
    pub(crate) repath_limit: Option<RepathLimit>,
    /// Owner when the caller already holds the state-machine mutex.
    pub(crate) preset_owner: Option<Arc<RwLock<crate::object::Object>>>,
}

pub(crate) const MIN_REPATH_TIME: u32 = 10;

#[derive(Debug, Clone, Copy)]
pub(crate) struct RepathLimit {
    pub(crate) remaining: i32,
    pub(crate) blocked_only: bool,
}

impl AIMoveToState {
    /// Create new move to state
    /// C++ constructor from AIStates.cpp line 1992
    pub fn new(machine: &StateMachine) -> Self {
        Self {
            base: State::new(machine, "AIMoveTo"),
            goal_position: Coord3D::new(0.0, 0.0, 0.0),
            path_goal_position: Coord3D::new(0.0, 0.0, 0.0),
            path_timestamp: 0,
            blocked_repath_timestamp: 0,
            adjust_destinations: true,
            adjust_destinations_override: None,
            waiting_for_path: false,
            try_one_more_repath: true,
            goal_layer: 0, // LAYER_INVALID. LAYER_GROUND is 1. Nothing assigns this yet.
            is_move_to: true,
            ambient_playing_handle: 0,
            repath_limit: None,
            preset_owner: None,
        }
    }

    pub fn set_adjusts_destination(&mut self, adjust: bool) {
        self.adjust_destinations_override = Some(adjust);
        self.adjust_destinations = adjust;
    }

    pub fn set_repath_limit(&mut self, remaining: i32, blocked_only: bool) {
        self.repath_limit = Some(RepathLimit {
            remaining,
            blocked_only,
        });
    }

    pub fn clear_repath_limit(&mut self) {
        self.repath_limit = None;
    }

    /// Compute path to goal - C++ AIInternalMoveToState::computePath() from AIStates.cpp line 1577
    pub(crate) fn compute_path(&mut self, ai: &mut dyn AIUpdateInterface) -> Result<(), String> {
        self.waiting_for_path = true;
        ai.request_path(&self.goal_position, self.adjust_destinations)?;
        ai.friend_starting_move();
        Ok(())
    }

    pub(crate) fn start_from_borrowed_ai(
        &mut self,
        ai: &mut dyn AIUpdateInterface,
        owner_guard: &Object,
    ) -> Result<(), String> {
        self.ambient_playing_handle = 0;
        self.adjust_destinations = self.adjust_destinations_override.unwrap_or(true);
        if owner_guard.test_status(ObjectStatusTypes::Parachuting)
            || !ai.is_allowed_to_adjust_destination()
        {
            self.adjust_destinations = false;
        }
        self.compute_path(ai)?;
        let _ = ai.set_path_extra_distance(0.0);
        ai.set_desired_speed(FAST_AS_POSSIBLE);
        ai.friend_starting_move();
        ai.with_cur_locomotor_mut(&mut |loco| loco.start_move());
        self.start_move_sound(owner_guard);
        if owner_guard.get_formation_id() != FormationID::NONE {
            if let Some(group_id) = owner_guard.get_group_id() {
                let ai_store = the_ai();
                if let Ok(ai_lock) = ai_store.read() {
                    if let Some(group) = ai_lock.find_group(group_id) {
                        if let Ok(mut group_guard) = group.write() {
                            ai.set_desired_speed(group_guard.get_speed());
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// Force repath by resetting path state
    pub(crate) fn force_repath(&mut self) {
        self.path_goal_position = Coord3D::new(-100.0, -100.0, -100.0);
        self.path_timestamp = 0;
    }

    /// Check if position has changed enough to require repath
    /// C++ isSamePosition() from AIStates.cpp line 183
    pub(crate) fn is_same_position(
        &self,
        our_pos: &Coord3D,
        prev_target_pos: &Coord3D,
        cur_target_pos: &Coord3D,
    ) -> bool {
        // Calculate difference
        let diff_x = cur_target_pos.x - prev_target_pos.x;
        let diff_y = cur_target_pos.y - prev_target_pos.y;

        // Calculate distance to target
        let to_target_x = cur_target_pos.x - our_pos.x;
        let to_target_y = cur_target_pos.y - our_pos.y;

        // Tolerance is (dist/10)^2
        const TOLERANCE_FACTOR: f32 = 1.0 / (10.0 * 10.0);
        let tolerance_sqr =
            (to_target_x * to_target_x + to_target_y * to_target_y) * TOLERANCE_FACTOR;

        // Check if moved beyond tolerance
        diff_x * diff_x + diff_y * diff_y <= tolerance_sqr
    }
    fn finish_move_update(
        &mut self,
        owner_guard: &mut Object,
        ai: &mut dyn crate::modules::AIUpdateInterface,
    ) -> Result<StateReturnType, String> {
        if self.waiting_for_path {
            self.path_timestamp = TheGameLogic::get_frame();
            if ai.is_waiting_for_path() {
                return Ok(StateReturnType::Continue);
            }
            if ai.get_path().is_none() {
                return Ok(StateReturnType::Failure);
            }
            self.waiting_for_path = false;
            if self.adjust_destinations
                && !owner_guard.test_status(crate::common::ObjectStatusTypes::Parachuting)
                && ai.is_allowed_to_adjust_destination()
            {
                if let (Some(last), Some(ordinal)) =
                    (ai.get_path_last_node(), ai.installed_path_last_layer())
                {
                    let _ = ai.update_goal_position(
                        &last,
                        crate::common::PathfindLayerEnum::from_u32(u32::from(ordinal)),
                    );
                }
            } else {
                ai.remove_pathfinder_goal();
            }
            self.path_goal_position = self.goal_position;
            if !ai.get_retry_path() {
                self.try_one_more_repath = false;
            }
        }
        let adjustment = ai.get_mood_matrix_action_adjustment(MoodMatrixAction::Move);
        if self.is_move_to && (adjustment & mood_matrix_adjustment::ACTION_TO_ATTACK_MOVE) != 0 {
            owner_guard.ai_pending_attack_move = Some(self.goal_position);
        }
        let mut goal_moved = false;
        if let Some(goal_obj) = self.base.get_machine_goal_object_id().and_then(|id| {
            crate::helpers::TheGameLogic::find_object_by_id(id)
                .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(id))
        }) {
            let goal_guard = goal_obj
                .read()
                .map_err(|_| "goal object lock poisoned".to_string())?;
            let mut new_goal = *goal_guard.get_position();
            if owner_guard.is_kind_of(KindOf::Projectile) {
                let half_height = goal_guard
                    .get_geometry_info()
                    .get_max_height_above_position()
                    * 0.5;
                new_goal.z += half_height;
                if goal_guard.get_position().z < new_goal.z {
                    new_goal.z += half_height;
                }
            }
            if owner_guard.is_kind_of(KindOf::Projectile)
                && goal_guard.get_physics().is_some()
                && !goal_guard.is_kind_of(KindOf::Immobile)
            {
                let our_pos = owner_guard.get_position();
                let delta = new_goal - *our_pos;
                let my_speed = owner_guard
                    .get_physics()
                    .map(|p| p.get_velocity().length())
                    .unwrap_or(5.0)
                    .max(5.0);
                let goal_speed = goal_guard
                    .get_physics()
                    .map(|p| p.get_velocity().length())
                    .unwrap_or(0.0);
                let lead_distance = 0.5 * delta.length() * goal_speed / my_speed;
                if let Some(physics) = goal_guard.get_physics() {
                    let vel = physics.get_velocity();
                    let vel_len = vel.length();
                    if vel_len > 0.001 {
                        let dir = vel / vel_len;
                        new_goal.x += dir.x * lead_distance;
                        new_goal.y += dir.y * lead_distance;
                        new_goal.z += dir.z * lead_distance;
                    }
                }
            }
            self.goal_position = new_goal;
            if !self.is_same_position(
                owner_guard.get_position(),
                &self.path_goal_position,
                &new_goal,
            ) {
                goal_moved = true;
            }
        }
        let frames_blocked = ai.get_num_frames_blocked();
        let blocked = ai.is_blocked_and_stuck() || frames_blocked > 2 * LOGICFRAMES_PER_SECOND;
        let now = TheGameLogic::get_frame();
        if blocked {
            self.blocked_repath_timestamp = now;
        }
        let mut set_condition_flag = ModelConditionFlags::MOVING;
        if is_cliff_at(owner_guard.get_position()) {
            let moving_backwards = {
                let mut __back = false;
                ai.with_cur_locomotor(&mut |loco| __back = loco.is_moving_backwards());
                __back
            };
            set_condition_flag = if moving_backwards {
                ModelConditionFlags::RAPPELLING
            } else {
                ModelConditionFlags::CLIMBING
            };
        }
        owner_guard.set_model_condition_state(ModelConditionFlags::MOVING);
        if set_condition_flag == ModelConditionFlags::MOVING {
            owner_guard.clear_model_condition_state(ModelConditionFlags::CLIMBING);
            owner_guard.clear_model_condition_state(ModelConditionFlags::RAPPELLING);
        } else {
            let clear_flag = if set_condition_flag == ModelConditionFlags::CLIMBING {
                ModelConditionFlags::RAPPELLING
            } else {
                ModelConditionFlags::CLIMBING
            };
            owner_guard.clear_model_condition_state(clear_flag);
            owner_guard.set_model_condition_state(set_condition_flag);
        }
        if frames_blocked > LOGICFRAMES_PER_SECOND / 4 {
            owner_guard.clear_model_condition_state(ModelConditionFlags::MOVING);
        }
        let null_path = ai.get_path().is_none();
        let quick_projectile =
            owner_guard.is_kind_of(KindOf::Projectile) && ai.can_compute_quick_path();
        if quick_projectile {
            self.path_timestamp = 0;
        }
        let force_recompute = null_path || blocked || quick_projectile;
        let time_up = now.saturating_sub(self.path_timestamp) > MIN_REPATH_TIME;
        if !null_path {
            ai.set_locomotor_goal_position_on_path();
        }
        if force_recompute || time_up {
            let goal_changed = !self.is_same_position(
                owner_guard.get_position(),
                &self.path_goal_position,
                &self.goal_position,
            );
            if force_recompute || goal_changed {
                if let Some(limit) = self.repath_limit.as_mut() {
                    if !(limit.blocked_only && !blocked) {
                        if limit.remaining <= 0 {
                            return Ok(StateReturnType::Failure);
                        }
                        limit.remaining -= 1;
                        self.compute_path(ai)?;
                        if ai.get_path().is_none() {
                            return Ok(StateReturnType::Continue);
                        }
                        ai.set_locomotor_goal_position_on_path();
                    }
                } else {
                    self.compute_path(ai)?;
                    if ai.get_path().is_none() {
                        return Ok(StateReturnType::Continue);
                    }
                    ai.set_locomotor_goal_position_on_path();
                }
            }
        }
        let close_enough = {
            let mut __close = 5.0;
            ai.with_cur_locomotor(&mut |loco| __close = loco.get_close_enough_dist());
            __close
        };
        if ai.get_locomotor_distance_to_goal() <= close_enough {
            owner_guard.clear_model_condition_state(ModelConditionFlags::MOVING);
            return Ok(StateReturnType::Success);
        }
        Ok(StateReturnType::Continue)
    }
}

impl StateImplementation for AIMoveToState {
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
        ai: &mut dyn crate::modules::AIUpdateInterface,
        goal_id: crate::common::ObjectID,
        goal_pos: Coord3D,
    ) -> StateReturnType {
        if dual_world_registry_unavailable() {
            return StateReturnType::Failure;
        }
        if goal_id != crate::common::INVALID_ID {
            if let Some(goal_obj) = crate::helpers::TheGameLogic::find_object_by_id(goal_id)
                .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(goal_id))
            {
                if let Ok(goal_guard) = goal_obj.read() {
                    self.goal_position = *goal_guard.get_position();
                } else {
                    self.goal_position = goal_pos;
                }
            } else {
                self.goal_position = goal_pos;
            }
        } else {
            self.goal_position = goal_pos;
        }
        let Some(owner) = self.base.get_machine_owner() else {
            return StateReturnType::Failure;
        };
        if let Ok(mut owner_guard) = owner.write() {
            if owner_guard.test_status(ObjectStatusTypes::Immobile) {
                return StateReturnType::Failure;
            }
            owner_guard.set_model_condition_state(ModelConditionFlags::MOVING);
        }
        let Ok(owner_guard) = owner.read() else {
            return StateReturnType::Failure;
        };
        self.start_from_borrowed_ai(ai, &owner_guard)
            .map(|_| StateReturnType::Continue)
            .unwrap_or(StateReturnType::Failure)
    }

    fn update(&mut self) -> StateReturnType {
        self.cpp_on_update().unwrap_or(StateReturnType::Failure)
    }

    fn update_with_ai(
        &mut self,
        ai: &mut dyn crate::modules::AIUpdateInterface,
    ) -> StateReturnType {
        let owner = if let Some(owner) = self.preset_owner.clone() {
            owner
        } else {
            let Some(owner) = self.base.get_machine_owner() else {
                return StateReturnType::Failure;
            };
            owner
        };
        let Ok(mut owner_guard) = owner.lock() else {
            return StateReturnType::Failure;
        };
        self.finish_move_update(&mut owner_guard, ai)
            .unwrap_or(StateReturnType::Failure)
    }

    fn on_exit(&mut self, _status: StateExitType) {
        let _ = self.cpp_on_exit(_status);
    }

    fn xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::xfer(self, xfer)
    }
}

impl CppState for AIMoveToState {
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
        self.enter_move(None)
    }

    fn cpp_on_enter_with_ai(
        &mut self,
        ai_ref: &mut dyn crate::modules::AIUpdateInterface,
    ) -> Result<StateReturnType, String> {
        self.enter_move(Some(ai_ref))
    }

    fn cpp_on_update(&mut self) -> Result<StateReturnType, String> {
        if dual_world_registry_unavailable() {
            return Ok(StateReturnType::Failure);
        }
        let owner = if let Some(owner) = self.preset_owner.clone() {
            owner
        } else {
            self.base
                .get_machine_owner()
                .ok_or_else(|| "AIMoveToState missing owner".to_string())?
        };
        let mut owner_guard = owner
            .lock()
            .map_err(|_| "AIMoveToState owner lock poisoned".to_string())?;
        let ai_arc = owner_guard
            .get_ai_update_interface()
            .ok_or_else(|| "AIMoveToState missing AIUpdateInterface".to_string())?;
        let mut ai_guard = ai_arc
            .lock()
            .map_err(|_| "AIMoveToState AI lock poisoned".to_string())?;
        return self.finish_move_update(&mut owner_guard, &mut *ai_guard);
    }

    fn cpp_on_update_with_ai(
        &mut self,
        ai: &mut dyn crate::modules::AIUpdateInterface,
    ) -> Result<StateReturnType, String> {
        if dual_world_registry_unavailable() {
            return Ok(StateReturnType::Failure);
        }
        let owner = if let Some(owner) = self.preset_owner.clone() {
            owner
        } else {
            self.base
                .get_machine_owner()
                .ok_or_else(|| "AIMoveToState missing owner".to_string())?
        };
        let mut owner_guard = owner
            .lock()
            .map_err(|_| "AIMoveToState owner lock poisoned".to_string())?;
        self.finish_move_update(&mut owner_guard, ai)
    }

    fn start_move_sound(&mut self, owner_guard: &Object) {
        let mut use_damaged = false;
        if let Some(body) = owner_guard.get_body_module() {
            if let Ok(body_guard) = body.lock() {
                use_damaged = body_guard.get_damage_state() > BodyDamageType::Damaged;
            }
        }
        let template = owner_guard.get_template();
        let mut start_sound = if use_damaged {
            template.get_sound_move_start_damaged()
        } else {
            template.get_sound_move_start()
        };
        let loop_sound = if use_damaged {
            template.get_sound_move_loop_damaged()
        } else {
            template.get_sound_move_loop()
        };
        if start_sound.get_event_name().is_empty() {
            start_sound = loop_sound.clone();
        }
        if start_sound.get_event_name().is_empty() {
            return;
        }
        start_sound.set_object_id(owner_guard.get_id());
        if let Some(audio) = TheAudio::get() {
            if start_sound.get_event_name() == loop_sound.get_event_name()
                && !loop_sound.get_event_name().is_empty()
            {
                let handle = audio.add_audio_event(&start_sound);
                self.ambient_playing_handle = handle;
            } else {
                audio.add_audio_event(&start_sound);
            }
        }
    }

    fn cpp_on_exit_with_ai(
        &mut self,
        _exit: StateExitType,
        ai: &mut dyn AIUpdateInterface,
    ) -> Result<(), String> {
        // AIInternalMoveToState::onExit: sound, endingMove, exact ground
        // finalization. It does not destroyPath or reset path extra distance.
        if self.ambient_playing_handle != 0 {
            if let Some(audio) = TheAudio::get() {
                audio.remove_audio_event(self.ambient_playing_handle);
            }
            self.ambient_playing_handle = 0;
        }
        ai.friend_ending_move();
        let ground = ai.is_doing_ground_movement();
        let mut ultra = false;
        ai.with_cur_locomotor(&mut |loco| ultra = loco.is_ultra_accurate());
        if ground && ultra {
            if let Some(owner) = self.base.get_machine_owner() {
                let owner = owner
                    .read()
                    .map_err(|_| "movement exit owner poisoned".to_string())?;
                let dx = self.goal_position.x - owner.get_position().x;
                let dy = self.goal_position.y - owner.get_position().y;
                if owner.get_team().is_some()
                    && dx * dx + dy * dy < PATHFIND_CELL_SIZE_F * PATHFIND_CELL_SIZE_F
                {
                    drop(owner);
                    ai.set_final_position(&self.goal_position);
                }
            }
        }
        Ok(())
    }

    fn cpp_on_exit(&mut self, _exit: StateExitType) -> Result<(), String> {
        if self.ambient_playing_handle != 0 {
            if let Some(audio) = TheAudio::get() {
                audio.remove_audio_event(self.ambient_playing_handle);
            }
            self.ambient_playing_handle = 0;
        }
        if let Some(owner) = self.base.get_machine_owner() {
            if let Ok(mut owner_guard) = owner.write() {
                let airborne = matches!(
                    owner_guard.ai_fire_loco_appearance,
                    Some(
                        LocomotorAppearance::Hover
                            | LocomotorAppearance::Thrust
                            | LocomotorAppearance::Wings
                    )
                );
                if owner_guard.ai_fire_ultra_accurate && !airborne {
                    let dx = self.goal_position.x - owner_guard.get_position().x;
                    let dy = self.goal_position.y - owner_guard.get_position().y;
                    if dx * dx + dy * dy < PATHFIND_CELL_SIZE_F * PATHFIND_CELL_SIZE_F {
                        let _ = owner_guard.set_position(&self.goal_position);
                    }
                }
                owner_guard.ai_pending_ending_move = true;
                owner_guard.ai_pending_destroy_path = true;
                owner_guard.clear_model_condition_state(ModelConditionFlags::MOVING);
            }
        }
        Ok(())
    }
}

impl AIMoveToState {
    pub(crate) fn enter_authored_segment(
        &mut self,
        ai: &mut dyn AIUpdateInterface,
        goal: Coord3D,
    ) -> Result<StateReturnType, String> {
        self.enter_move_impl(Some(ai), Some(goal))
    }
    fn enter_move(
        &mut self,
        borrowed: Option<&mut dyn AIUpdateInterface>,
    ) -> Result<StateReturnType, String> {
        self.enter_move_impl(borrowed, None)
    }
    fn enter_move_impl(
        &mut self,
        mut borrowed: Option<&mut dyn crate::modules::AIUpdateInterface>,
        authored_goal: Option<Coord3D>,
    ) -> Result<StateReturnType, String> {
        if dual_world_registry_unavailable() {
            return Ok(StateReturnType::Failure);
        }

        if let Some(owner) = self.base.get_machine_owner() {
            if let Ok(owner_guard) = owner.read() {
                if owner_guard.test_status(ObjectStatusTypes::Immobile) {
                    return Ok(StateReturnType::Failure);
                }
            }
        }

        self.adjust_destinations = self.adjust_destinations_override.unwrap_or(true);
        self.ambient_playing_handle = 0;

        if let Some(goal) = authored_goal {
            self.goal_position = goal;
            self.base.goal_position_copied = Some(goal);
        } else if let Some(goal_obj) = self.base.get_machine_goal_object_id().and_then(|id| {
            crate::helpers::TheGameLogic::find_object_by_id(id)
                .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(id))
        }) {
            let goal_guard = goal_obj
                .read()
                .map_err(|_| "goal object lock poisoned".to_string())?;
            self.goal_position = *goal_guard.get_position();
            if let Some(owner) = self.base.get_machine_owner() {
                if let Ok(owner_guard) = owner.read() {
                    if owner_guard.is_kind_of(KindOf::Projectile) {
                        let half_height = goal_guard
                            .get_geometry_info()
                            .get_max_height_above_position()
                            * 0.5;
                        self.goal_position.z += half_height;
                        if goal_guard.get_position().z < self.goal_position.z {
                            self.goal_position.z += half_height;
                        }
                    }
                }
            }
        } else if let Some(goal_pos) = self.base.get_machine_goal_position() {
            self.goal_position = goal_pos;
        }

        let owner = if let Some(owner) = self.preset_owner.clone() {
            owner
        } else {
            self.base
                .get_machine_owner()
                .ok_or_else(|| "AIMoveToState missing owner".to_string())?
        };
        let mut owner_guard = owner
            .lock()
            .map_err(|_| "AIMoveToState owner lock poisoned".to_string())?;
        owner_guard.set_model_condition_state(ModelConditionFlags::MOVING);
        if is_cliff_at(owner_guard.get_position()) {
            owner_guard.set_model_condition_state(ModelConditionFlags::CLIMBING);
            owner_guard.clear_model_condition_state(ModelConditionFlags::RAPPELLING);
        }
        let parachuting = owner_guard.test_status(ObjectStatusTypes::Parachuting);
        let formation_group = (authored_goal.is_none()
            && owner_guard.get_formation_id() != FormationID::NONE)
            .then(|| owner_guard.get_group_id())
            .flatten();
        let installed_ai = owner_guard.get_ai_update_interface();
        let formation_member_ai = installed_ai.clone();
        drop(owner_guard);
        let ai_arc;
        let mut locked_ai;
        let ai_guard: &mut dyn crate::modules::AIUpdateInterface =
            if let Some(ai_ref) = borrowed.as_mut() {
                *ai_ref
            } else {
                ai_arc = installed_ai
                    .ok_or_else(|| "AIMoveToState missing AIUpdateInterface".to_string())?;
                locked_ai = ai_arc
                    .lock()
                    .map_err(|_| "AIMoveToState AI lock poisoned".to_string())?;
                &mut *locked_ai
            };

        if parachuting {
            self.adjust_destinations = false;
        } else if !ai_guard.is_allowed_to_adjust_destination() {
            self.adjust_destinations = false;
        }

        ai_guard.set_adjusts_destination(self.adjust_destinations);
        self.compute_path(&mut *ai_guard)?;
        let _ = ai_guard.set_path_extra_distance(0.0);
        ai_guard.set_desired_speed(FAST_AS_POSSIBLE);

        // C++ AIInternalMoveToState::onEnter (AIStates.cpp:1604-1605): startMove.
        ai_guard.friend_starting_move();
        ai_guard.with_cur_locomotor_mut(&mut |loco| loco.start_move());

        {
            let owner_guard = owner
                .read()
                .map_err(|_| "movement sound owner poisoned".to_string())?;
            self.start_move_sound(&owner_guard);
        }

        if let Some(group_id) = formation_group {
            // Capture the same getter the existing group algorithm uses before
            // its lock; the caller's installed AI is already borrowed here.
            let member_speed = ai_guard.get_speed();
            {
                let ai_store = the_ai();
                if let Ok(ai_lock) = ai_store.read() {
                    if let Some(group) = ai_lock.find_group(group_id) {
                        if let Ok(mut group_guard) = group.write() {
                            let speed = if let Some(installed) = formation_member_ai.as_ref() {
                                group_guard.get_speed_with_borrowed_member(
                                    &owner,
                                    installed,
                                    member_speed,
                                )
                            } else {
                                group_guard.get_speed()
                            };
                            ai_guard.set_desired_speed(speed);
                        }
                    }
                }
            }
        }

        Ok(StateReturnType::Continue)
    }
}

/// Move and evacuate state - move to a position then evacuate transport.
#[derive(Debug)]
pub struct AIMoveAndEvacuateState {
    pub(crate) base: AIMoveToState,
    pub(crate) origin: Coord3D,
}

impl AIMoveAndEvacuateState {
    pub fn new(machine: &StateMachine, name: &str) -> Self {
        let mut base = AIMoveToState::new(machine);
        base.base.name = name.to_string();
        Self {
            base,
            origin: Coord3D::new(0.0, 0.0, 0.0),
        }
    }
}

impl StateImplementation for AIMoveAndEvacuateState {
    fn on_enter(&mut self) -> StateReturnType {
        self.cpp_on_enter().unwrap_or(StateReturnType::Failure)
    }

    fn bind_goal_object_id(&mut self, id: crate::common::ObjectID) {
        self.base.bind_goal_object_id(id);
    }

    fn bind_goal_position(&mut self, pos: Coord3D) {
        self.base.bind_goal_position(pos);
    }

    fn on_enter_with_ai(
        &mut self,
        ai: &mut dyn crate::modules::AIUpdateInterface,
        goal_id: crate::common::ObjectID,
        goal_pos: Coord3D,
    ) -> StateReturnType {
        if let Some(owner) = self.base.base.get_machine_owner() {
            if let Ok(owner_guard) = owner.read() {
                self.origin = *owner_guard.get_position();
            }
        }
        self.base.set_adjusts_destination(true);
        if goal_id != crate::common::INVALID_ID {
            if let Some(goal_obj) = crate::helpers::TheGameLogic::find_object_by_id(goal_id)
                .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(goal_id))
            {
                if let Ok(goal_guard) = goal_obj.read() {
                    self.base.goal_position = *goal_guard.get_position();
                } else {
                    self.base.goal_position = goal_pos;
                }
            } else {
                self.base.goal_position = goal_pos;
            }
        } else {
            self.base.goal_position = goal_pos;
        }
        let Some(owner) = self.base.base.get_machine_owner() else {
            return StateReturnType::Failure;
        };
        if let Ok(mut owner_guard) = owner.write() {
            if owner_guard.test_status(ObjectStatusTypes::Immobile) {
                return StateReturnType::Failure;
            }
            owner_guard.set_model_condition_state(ModelConditionFlags::MOVING);
        }
        let Ok(owner_guard) = owner.read() else {
            return StateReturnType::Failure;
        };
        self.base
            .start_from_borrowed_ai(ai, &owner_guard)
            .map(|_| StateReturnType::Continue)
            .unwrap_or(StateReturnType::Failure)
    }

    fn update(&mut self) -> StateReturnType {
        self.cpp_on_update().unwrap_or(StateReturnType::Failure)
    }

    fn update_with_ai(
        &mut self,
        ai: &mut dyn crate::modules::AIUpdateInterface,
    ) -> StateReturnType {
        let Some(owner) = self.base.base.get_machine_owner() else {
            return StateReturnType::Failure;
        };
        let Ok(owner_guard) = owner.read() else {
            return StateReturnType::Failure;
        };
        if owner_guard.is_effectively_dead() {
            return StateReturnType::Failure;
        }
        let close_enough = {
            let mut __close = 5.0;
            ai.with_cur_locomotor(&mut |loco| __close = loco.get_close_enough_dist());
            __close
        };
        if ai.is_waiting_for_path() || ai.get_locomotor_distance_to_goal() > close_enough {
            return StateReturnType::Continue;
        }
        drop(owner_guard);
        if let Ok(mut owner_guard) = owner.write() {
            owner_guard.ai_pending_evacuate = true;
            if let Some(team) = owner_guard.get_team() {
                if let Ok(mut team_guard) = team.write() {
                    team_guard.set_active();
                }
            }
        }
        StateReturnType::Success
    }

    fn on_exit_with_ai(
        &mut self,
        status: StateExitType,
        _ai: &mut dyn crate::modules::AIUpdateInterface,
    ) {
        let _ = self.base.cpp_on_exit(status);
    }

    fn locks_machine(&self) -> bool {
        true
    }

    fn exit_restore_goal(&self) -> Option<Coord3D> {
        Some(self.origin)
    }

    fn on_exit(&mut self, _status: StateExitType) {
        let _ = self.cpp_on_exit(_status);
    }

    fn xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::xfer(self, xfer)
    }
}

impl CppState for AIMoveAndEvacuateState {
    fn base_state(&self) -> &State {
        &self.base.base
    }

    fn cpp_locks_machine(&self) -> bool {
        true
    }

    fn cpp_exit_restore_goal(&self) -> Option<Coord3D> {
        Some(self.origin)
    }

    fn base_state_mut(&mut self) -> &mut State {
        &mut self.base.base
    }

    fn cpp_xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::xfer(self, xfer)
    }

    fn cpp_on_enter(&mut self) -> Result<StateReturnType, String> {
        let owner = self
            .base
            .base
            .get_machine_owner()
            .ok_or_else(|| "move+evacuate missing machine owner".to_string())?;
        if let Ok(owner_guard) = owner.read() {
            self.origin = *owner_guard.get_position();
        }

        self.base.set_adjusts_destination(true);
        self.base.cpp_on_enter()
    }

    fn cpp_on_update(&mut self) -> Result<StateReturnType, String> {
        let status = self.base.cpp_on_update()?;
        if status != StateReturnType::Continue {
            let owner = self
                .base
                .base
                .get_machine_owner()
                .ok_or_else(|| "move+evacuate missing machine owner".to_string())?;
            if let Ok(mut owner_guard) = owner.write() {
                if owner_guard.is_effectively_dead() {
                    return Ok(StateReturnType::Failure);
                }
                owner_guard.ai_pending_evacuate = true;
                if let Some(team) = owner_guard.get_team() {
                    if let Ok(mut team_guard) = team.write() {
                        team_guard.set_active();
                    }
                }
            };
        }
        Ok(status)
    }

    fn cpp_on_exit(&mut self, _exit: StateExitType) -> Result<(), String> {
        Ok(())
    }
}

impl Snapshotable for AIMoveToState {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: game_engine::common::system::xfer::XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("AIMoveToState crc version failed: {:?}", e))?;
        let mut goal_position_x = self.goal_position.x;
        xfer.xfer_real(&mut goal_position_x)
            .map_err(|e| format!("AIMoveToState crc goal_position.x failed: {:?}", e))?;
        let mut goal_position_y = self.goal_position.y;
        xfer.xfer_real(&mut goal_position_y)
            .map_err(|e| format!("AIMoveToState crc goal_position.y failed: {:?}", e))?;
        let mut goal_position_z = self.goal_position.z;
        xfer.xfer_real(&mut goal_position_z)
            .map_err(|e| format!("AIMoveToState crc goal_position.z failed: {:?}", e))?;
        let mut goal_layer = self.goal_layer as u8;
        xfer.xfer_u8(&mut goal_layer);
        let mut waiting_for_path = self.waiting_for_path;
        xfer.xfer_bool(&mut waiting_for_path)
            .map_err(|e| format!("AIMoveToState crc waiting_for_path failed: {:?}", e))?;
        let mut path_goal_position_x = self.path_goal_position.x;
        xfer.xfer_real(&mut path_goal_position_x)
            .map_err(|e| format!("AIMoveToState crc path_goal_position.x failed: {:?}", e))?;
        let mut path_goal_position_y = self.path_goal_position.y;
        xfer.xfer_real(&mut path_goal_position_y)
            .map_err(|e| format!("AIMoveToState crc path_goal_position.y failed: {:?}", e))?;
        let mut path_goal_position_z = self.path_goal_position.z;
        xfer.xfer_real(&mut path_goal_position_z)
            .map_err(|e| format!("AIMoveToState crc path_goal_position.z failed: {:?}", e))?;
        let mut path_timestamp = self.path_timestamp;
        xfer.xfer_unsigned_int(&mut path_timestamp)
            .map_err(|e| format!("AIMoveToState crc path_timestamp failed: {:?}", e))?;
        let mut blocked_repath_timestamp = self.blocked_repath_timestamp;
        xfer.xfer_unsigned_int(&mut blocked_repath_timestamp)
            .map_err(|e| format!("AIMoveToState crc blocked_repath_timestamp failed: {:?}", e))?;
        let mut adjust_destinations = self.adjust_destinations;
        xfer.xfer_bool(&mut adjust_destinations)
            .map_err(|e| format!("AIMoveToState crc adjust_destinations failed: {:?}", e))?;
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: game_engine::common::system::xfer::XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("AIMoveToState xfer version failed: {:?}", e))?;

        xfer.xfer_real(&mut self.goal_position.x)
            .map_err(|e| format!("AIMoveToState xfer goal_position.x failed: {:?}", e))?;
        xfer.xfer_real(&mut self.goal_position.y)
            .map_err(|e| format!("AIMoveToState xfer goal_position.y failed: {:?}", e))?;
        xfer.xfer_real(&mut self.goal_position.z)
            .map_err(|e| format!("AIMoveToState xfer goal_position.z failed: {:?}", e))?;
        // C++ AIInternalMoveToState::xfer writes PathfindLayerEnum via
        // xferUser(sizeof(enum)); this is a four-byte payload, not a byte.
        xfer.xfer_unsigned_int(&mut self.goal_layer)
            .map_err(|e| format!("AIMoveToState xfer goal_layer failed: {e:?}"))?;
        xfer.xfer_bool(&mut self.waiting_for_path)
            .map_err(|e| format!("AIMoveToState xfer waiting_for_path failed: {:?}", e))?;
        xfer.xfer_real(&mut self.path_goal_position.x)
            .map_err(|e| format!("AIMoveToState xfer path_goal_position.x failed: {:?}", e))?;
        xfer.xfer_real(&mut self.path_goal_position.y)
            .map_err(|e| format!("AIMoveToState xfer path_goal_position.y failed: {:?}", e))?;
        xfer.xfer_real(&mut self.path_goal_position.z)
            .map_err(|e| format!("AIMoveToState xfer path_goal_position.z failed: {:?}", e))?;
        xfer.xfer_unsigned_int(&mut self.path_timestamp)
            .map_err(|e| format!("AIMoveToState xfer path_timestamp failed: {:?}", e))?;
        xfer.xfer_unsigned_int(&mut self.blocked_repath_timestamp)
            .map_err(|e| {
                format!(
                    "AIMoveToState xfer blocked_repath_timestamp failed: {:?}",
                    e
                )
            })?;
        xfer.xfer_bool(&mut self.adjust_destinations)
            .map_err(|e| format!("AIMoveToState xfer adjust_destinations failed: {:?}", e))?;

        if xfer.is_loading() {
            self.adjust_destinations_override = None;
            self.repath_limit = None;
            self.ambient_playing_handle = 0;
        }

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        if let Some(owner) = self.base.get_machine_owner() {
            if let Ok(owner_guard) = owner.read() {
                self.start_move_sound(&owner_guard);
            }
        }
        Ok(())
    }
}

impl Snapshotable for AIWanderInPlaceState {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: game_engine::common::system::xfer::XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("AIWanderInPlaceState crc version failed: {:?}", e))?;
        Snapshotable::crc(&self.base, xfer)?;
        let mut origin_x = self.origin.x;
        xfer.xfer_real(&mut origin_x)
            .map_err(|e| format!("AIWanderInPlaceState crc origin.x failed: {:?}", e))?;
        let mut origin_y = self.origin.y;
        xfer.xfer_real(&mut origin_y)
            .map_err(|e| format!("AIWanderInPlaceState crc origin.y failed: {:?}", e))?;
        let mut origin_z = self.origin.z;
        xfer.xfer_real(&mut origin_z)
            .map_err(|e| format!("AIWanderInPlaceState crc origin.z failed: {:?}", e))?;
        let mut wait_frames = self.wait_frames;
        xfer.xfer_int(&mut wait_frames)
            .map_err(|e| format!("AIWanderInPlaceState crc wait_frames failed: {:?}", e))?;
        let mut timer = self.timer;
        xfer.xfer_int(&mut timer)
            .map_err(|e| format!("AIWanderInPlaceState crc timer failed: {:?}", e))?;
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: game_engine::common::system::xfer::XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("AIWanderInPlaceState xfer version failed: {:?}", e))?;

        Snapshotable::xfer(&mut self.base, xfer)?;
        xfer.xfer_real(&mut self.origin.x)
            .map_err(|e| format!("AIWanderInPlaceState xfer origin.x failed: {:?}", e))?;
        xfer.xfer_real(&mut self.origin.y)
            .map_err(|e| format!("AIWanderInPlaceState xfer origin.y failed: {:?}", e))?;
        xfer.xfer_real(&mut self.origin.z)
            .map_err(|e| format!("AIWanderInPlaceState xfer origin.z failed: {:?}", e))?;
        xfer.xfer_int(&mut self.wait_frames)
            .map_err(|e| format!("AIWanderInPlaceState xfer wait_frames failed: {:?}", e))?;
        xfer.xfer_int(&mut self.timer)
            .map_err(|e| format!("AIWanderInPlaceState xfer timer failed: {:?}", e))?;

        if xfer.is_loading() {
            self.goal_position = self.base.goal_position;
        }
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Snapshotable::load_post_process(&mut self.base)
    }
}

impl Snapshotable for AIMoveAndTightenState {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: game_engine::common::system::xfer::XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("AIMoveAndTightenState crc version failed: {:?}", e))?;
        Snapshotable::crc(&self.base, xfer)?;
        let mut ok_to_repath_times = self.ok_to_repath_times;
        xfer.xfer_int(&mut ok_to_repath_times).map_err(|e| {
            format!(
                "AIMoveAndTightenState crc ok_to_repath_times failed: {:?}",
                e
            )
        })?;
        let mut check_for_path = self.check_for_path;
        xfer.xfer_bool(&mut check_for_path)
            .map_err(|e| format!("AIMoveAndTightenState crc check_for_path failed: {:?}", e))?;
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: game_engine::common::system::xfer::XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("AIMoveAndTightenState xfer version failed: {:?}", e))?;

        Snapshotable::xfer(&mut self.base, xfer)?;
        xfer.xfer_int(&mut self.ok_to_repath_times).map_err(|e| {
            format!(
                "AIMoveAndTightenState xfer ok_to_repath_times failed: {:?}",
                e
            )
        })?;
        xfer.xfer_bool(&mut self.check_for_path)
            .map_err(|e| format!("AIMoveAndTightenState xfer check_for_path failed: {:?}", e))?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Snapshotable::load_post_process(&mut self.base)
    }
}

impl Snapshotable for AIMoveAndDeleteState {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: game_engine::common::system::xfer::XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("AIMoveAndDeleteState crc version failed: {:?}", e))?;
        Snapshotable::crc(&self.base, xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: game_engine::common::system::xfer::XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("AIMoveAndDeleteState xfer version failed: {:?}", e))?;

        Snapshotable::xfer(&mut self.base, xfer)
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Snapshotable::load_post_process(&mut self.base)
    }
}

impl Snapshotable for AIMoveAndEvacuateState {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: game_engine::common::system::xfer::XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("AIMoveAndEvacuateState crc version failed: {:?}", e))?;
        Snapshotable::crc(&self.base, xfer)?;
        let mut origin_x = self.origin.x;
        xfer.xfer_real(&mut origin_x)
            .map_err(|e| format!("AIMoveAndEvacuateState crc origin.x failed: {:?}", e))?;
        let mut origin_y = self.origin.y;
        xfer.xfer_real(&mut origin_y)
            .map_err(|e| format!("AIMoveAndEvacuateState crc origin.y failed: {:?}", e))?;
        let mut origin_z = self.origin.z;
        xfer.xfer_real(&mut origin_z)
            .map_err(|e| format!("AIMoveAndEvacuateState crc origin.z failed: {:?}", e))?;
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: game_engine::common::system::xfer::XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("AIMoveAndEvacuateState xfer version failed: {:?}", e))?;

        Snapshotable::xfer(&mut self.base, xfer)?;
        xfer.xfer_real(&mut self.origin.x)
            .map_err(|e| format!("AIMoveAndEvacuateState xfer origin.x failed: {:?}", e))?;
        xfer.xfer_real(&mut self.origin.y)
            .map_err(|e| format!("AIMoveAndEvacuateState xfer origin.y failed: {:?}", e))?;
        xfer.xfer_real(&mut self.origin.z)
            .map_err(|e| format!("AIMoveAndEvacuateState xfer origin.z failed: {:?}", e))?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Snapshotable::load_post_process(&mut self.base)
    }
}
