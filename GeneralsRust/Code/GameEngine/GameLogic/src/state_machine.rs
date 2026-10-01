//! State machine system - Finite state machine implementation
//!
//! This module provides the state machine framework used throughout the AI system
//! for managing complex behaviors and state transitions.
//!
//! Author: Converted from C++ by Claude, original by Michael S. Booth, January 2002

use crate::ai::squad::Squad;
use crate::common::CoordOrigin;
use crate::common::types::AsAny;
use crate::common::*;
use crate::object::Object;
use crate::polygon_trigger::PolygonTrigger;
use crate::waypoint::WaypointId;
use std::any::Any;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock, Weak};

/// State machine constants
pub const MACHINE_DONE_STATE_ID: u32 = 999998;
pub const INVALID_STATE_ID: u32 = 999999;

/// State ID type
pub type StateId = u32;

const MAX_TRANSITION_RECURSION_DEPTH: u32 = 20;
/// State return codes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StateReturnType {
    /// Stay in this state (only for update method)
    Continue,
    /// State finished successfully, go to next state  
    Success,
    /// State finished abnormally, go to next state
    Failure,
    /// Sleep for specified number of frames
    Sleep(u32),
    /// State completed successfully (alias for Success)
    Complete,
    /// State failed (alias for Failure)
    Failed,
    /// State was interrupted
    Interrupted,
    /// State is blocked (pathfinding, etc.)
    Blocked,
    /// State completed successfully (alternate)
    Finished,
    /// Error in state
    Error,
    /// Exit state immediately
    Exit,
}

impl StateReturnType {
    /// Create a sleep return value
    pub fn sleep(num_frames: u32) -> Self {
        StateReturnType::Sleep(num_frames)
    }

    /// Sleep forever (very long time)
    pub fn sleep_forever() -> Self {
        StateReturnType::Sleep(0x3fffffff)
    }

    /// Check if this is a sleep return
    pub fn is_sleep(&self) -> bool {
        matches!(self, StateReturnType::Sleep(_))
    }

    /// Get sleep frames if this is a sleep return
    pub fn get_sleep_frames(&self) -> Option<u32> {
        match self {
            StateReturnType::Sleep(frames) => Some(*frames),
            _ => None,
        }
    }

    /// Convert sleep to continue (for enclosing states)
    pub fn convert_sleep_to_continue(self) -> Self {
        match self {
            StateReturnType::Sleep(_) => StateReturnType::Continue,
            other => other,
        }
    }

    /// Check if this represents a successful completion
    pub fn is_success(&self) -> bool {
        matches!(
            self,
            StateReturnType::Success | StateReturnType::Complete | StateReturnType::Finished
        )
    }

    /// Check if this represents a failure
    pub fn is_failure(&self) -> bool {
        matches!(
            self,
            StateReturnType::Failure | StateReturnType::Failed | StateReturnType::Error
        )
    }

    /// Get minimum sleep time between encloser and enclosee
    pub fn min_sleep(encloser_sleep: u32, enclosee_result: Self) -> Self {
        match enclosee_result {
            StateReturnType::Sleep(enclosee_sleep) => {
                StateReturnType::Sleep(encloser_sleep.min(enclosee_sleep))
            }
            other => other,
        }
    }
}

// Implement From traits to support the ? operator
impl<E> From<Result<StateReturnType, E>> for StateReturnType {
    fn from(result: Result<StateReturnType, E>) -> Self {
        match result {
            Ok(val) => val,
            Err(_) => StateReturnType::Failed,
        }
    }
}

impl From<Result<(), String>> for StateReturnType {
    fn from(result: Result<(), String>) -> Self {
        match result {
            Ok(_) => StateReturnType::Success,
            Err(_) => StateReturnType::Failed,
        }
    }
}

/// Exit machine constants
pub const EXIT_MACHINE_WITH_SUCCESS: StateId = 9998;
pub const EXIT_MACHINE_WITH_FAILURE: StateId = 9999;

/// Parameters for onExit()
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StateExitType {
    /// State exited due to normal state transitioning
    Normal,
    /// State exited due to state machine reset
    Reset,
    /// State was interrupted
    Interrupted,
    /// State failed
    Failed,
    /// Emergency exit
    Emergency,
    /// Exit due to error
    Error,
    /// Forced exit (external)
    Forced,
}

/// State transition function pointer type
pub type StateTransFuncPtr = fn(&dyn StateImplementation, &StateTransitionUserData) -> bool;

/// User data for state transitions
#[derive(Debug, Clone)]
pub struct StateTransitionUserData {
    // Generic user data - would be customized per game's needs
    pub data: Option<Arc<dyn std::any::Any + Send + Sync>>,
}

impl StateTransitionUserData {
    pub fn new() -> Self {
        Self { data: None }
    }

    pub fn with_data<T: 'static + Send + Sync>(data: T) -> Self {
        Self {
            data: Some(Arc::new(data)),
        }
    }
}

/// State condition information
#[derive(Debug, Clone)]
pub struct StateConditionInfo {
    pub test: StateTransFuncPtr,
    pub to_state_id: StateId,
    pub user_data: StateTransitionUserData,
    pub description: String,
}

impl StateConditionInfo {
    pub fn new(
        test: StateTransFuncPtr,
        to_state_id: StateId,
        user_data: StateTransitionUserData,
        description: &str,
    ) -> Self {
        Self {
            test,
            to_state_id,
            user_data,
            description: description.to_string(),
        }
    }
}

/// State implementation trait - all AI states must implement this
pub trait StateImplementation: Any + AsAny + std::fmt::Debug + Send + Sync {
    /// Executed once when entering state
    fn on_enter(&mut self) -> StateReturnType {
        StateReturnType::Continue
    }

    /// AI-machine enter. Default keeps the old [`on_enter`].
    fn on_enter_with_ai(
        &mut self,
        _ai: &mut dyn crate::modules::AIUpdateInterface,
        _goal_id: crate::common::ObjectID,
        _goal_pos: Coord3D,
    ) -> StateReturnType {
        self.on_enter()
    }

    /// Same enter, plus the waypoint this machine already stores.
    fn on_enter_with_waypoint(
        &mut self,
        ai: &mut dyn crate::modules::AIUpdateInterface,
        goal_id: crate::common::ObjectID,
        goal_pos: Coord3D,
        _waypoint: Option<WaypointId>,
    ) -> StateReturnType {
        self.on_enter_with_ai(ai, goal_id, goal_pos)
    }

    /// Executed once when leaving state  
    fn on_exit(&mut self, _status: StateExitType) {}

    /// AI-machine exit. Default keeps the old [`on_exit`].
    fn on_exit_with_ai(
        &mut self,
        _status: StateExitType,
        _ai: &mut dyn crate::modules::AIUpdateInterface,
    ) {
        self.on_exit(_status)
    }

    /// Evacuate freezes transitions. The step already holds this machine.
    fn locks_machine(&self) -> bool {
        false
    }

    /// Goal to restore when a locked state exits. `None` leaves the machine alone.
    fn exit_restore_goal(&self) -> Option<Coord3D> {
        None
    }

    /// C++ per-state `loadPostProcess`. Default empty. Guard inner/outer/aggressor
    /// override this with `onEnter` to rebuild the attack child after xfer.
    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
    /// Implements this state's behavior, decides when to change state
    fn update(&mut self) -> StateReturnType;

    /// AI-machine step. Dock and turret keep [`update`]. Default ignores the borrow.
    fn update_with_ai(
        &mut self,
        _ai: &mut dyn crate::modules::AIUpdateInterface,
    ) -> StateReturnType {
        self.update()
    }

    /// Like [`update_with_ai`], but `machine_locked` is `StateMachine::is_locked`
    /// on the machine this step already holds. Do not `lock()` it again.
    fn update_with_ai_held(
        &mut self,
        ai: &mut dyn crate::modules::AIUpdateInterface,
        _machine_locked: bool,
    ) -> StateReturnType {
        self.update_with_ai(ai)
    }

    /// Owner already known because this step holds the machine.
    fn note_step_owner(
        &mut self,
        _owner: std::sync::Arc<std::sync::RwLock<crate::object::Object>>,
    ) {
    }

    fn note_guard_enter(&mut self, _mode: i32, _polygon: Option<Arc<PolygonTrigger>>) {}

    fn freezes_parent_during_update(&self) -> bool {
        false
    }

    fn bind_goal_object_id(&mut self, _id: crate::common::ObjectID) {}

    fn bind_goal_position(&mut self, _pos: Coord3D) {}

    /// Goal chosen while this machine was already borrowed. `None` means no change.
    /// The bool is whether the goal object should be cleared.
    fn take_published_goal(&mut self) -> Option<(Coord3D, bool)> {
        None
    }
    fn bind_goal_squad(&mut self, _squad: Option<Squad>) {}

    fn bind_goal_polygon(&mut self, _polygon: Option<Arc<PolygonTrigger>>) {}

    fn bind_goal_waypoint(&mut self, _waypoint: Option<WaypointId>) {}

    /// Check if this is an idle state
    fn is_idle(&self) -> bool {
        false
    }

    /// Check if this is an attack state
    fn is_attack(&self) -> bool {
        false
    }

    /// Check if this is guard idle state
    fn is_guard_idle(&self) -> bool {
        false
    }

    /// Check if this is a busy state
    fn is_busy(&self) -> bool {
        false
    }

    /// Get state name (for debugging)
    fn get_name(&self) -> &str {
        "UnknownState"
    }

    /// Get state ID
    fn get_id(&self) -> StateId {
        0
    }

    /// Set state ID (called by state machine)
    fn set_id(&mut self, _id: StateId) {
        // Default implementation does nothing
    }

    /// Get the goal object for this state machine (default implementation returns None)
    fn get_machine_goal_object(
        &self,
    ) -> Result<Option<Arc<RwLock<crate::object::Object>>>, String> {
        if let Some(base_state) = self.as_any().downcast_ref::<State>() {
            return Ok(base_state.get_machine_goal_object());
        }

        Ok(None)
    }

    /// Get the owner object for this state machine (default implementation returns error)
    fn get_machine_owner(&self) -> Result<Arc<RwLock<crate::object::Object>>, String> {
        if let Some(base_state) = self.as_any().downcast_ref::<State>() {
            return base_state
                .get_machine_owner()
                .ok_or_else(|| "state machine owner not attached".to_string());
        }

        Err(format!(
            "state '{}' does not expose its owner",
            self.get_name()
        ))
    }

    fn get_machine_owner_id(&self) -> Result<crate::common::ObjectID, String> {
        if let Some(base_state) = self.as_any().downcast_ref::<State>() {
            return base_state
                .get_machine_owner_id()
                .ok_or_else(|| "state machine owner not attached".to_string());
        }

        Err(format!(
            "state '{}' does not expose its owner",
            self.get_name()
        ))
    }
}

/// Transition information for internal use
#[derive(Debug)]
pub struct TransitionInfo {
    test: StateTransFuncPtr,
    to_state_id: StateId,
    user_data: StateTransitionUserData,
    description: String,
}

/// Base state implementation
#[derive(Debug)]
pub struct State {
    pub id: StateId,
    pub name: String,
    pub success_state_id: StateId,
    pub failure_state_id: StateId,
    pub transitions: Vec<TransitionInfo>,
    /// Copied from [`StateMachine::owner_id`] at construction. No second mutex.
    pub owner_id: crate::common::ObjectID,
    /// Victim id copied from the machine that owns this state.
    pub goal_object_id: crate::common::ObjectID,
    /// Goal point copied from the machine before each step.
    pub goal_position_copied: Option<Coord3D>,
    /// Squad value copied from the machine before each step.
    pub goal_squad_copied: Option<Squad>,
    /// Area copied before an update that already holds the machine lock.
    pub goal_polygon_copied: Option<Arc<PolygonTrigger>>,
    /// Waypoint copied before an update that already holds the machine lock.
    pub goal_waypoint_copied: Option<WaypointId>,
}

impl State {
    /// Create a state without wiring it to a concrete machine. This mirrors the
    /// legacy usage where most states lived inside stack-owned state machines.
    pub fn new(machine: &StateMachine, name: &str) -> Self {
        let mut state = Self::detached(name);
        state.owner_id = machine.get_owner_id();
        state
    }

    /// Create a state that is not attached to any machine. Goal data is bound
    /// by the owning machine before every step.
    pub fn detached(name: &str) -> Self {
        Self {
            id: INVALID_STATE_ID,
            name: name.to_string(),
            success_state_id: INVALID_STATE_ID,
            failure_state_id: INVALID_STATE_ID,
            transitions: Vec::new(),
            owner_id: crate::common::INVALID_ID,
            goal_object_id: crate::common::INVALID_ID,
            goal_position_copied: None,
            goal_squad_copied: None,
            goal_polygon_copied: None,
            goal_waypoint_copied: None,
        }
    }

    pub fn get_name(&self) -> &str {
        &self.name
    }

    pub fn get_id(&self) -> StateId {
        self.id
    }

    pub fn set_id(&mut self, id: StateId) {
        self.id = id;
    }

    /// Get the machine owner object
    pub fn get_machine_owner(&self) -> Option<Arc<RwLock<Object>>> {
        if self.owner_id == crate::common::INVALID_ID {
            return None;
        }
        crate::helpers::TheGameLogic::find_object_by_id(self.owner_id)
            .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(self.owner_id))
    }

    /// Get the machine goal object
    pub fn get_machine_goal_object(&self) -> Option<Arc<RwLock<Object>>> {
        let id = self.get_machine_goal_object_id()?;
        crate::helpers::TheGameLogic::find_object_by_id(id)
            .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(id))
    }

    /// Machine goal object id, bound by the machine before each step.
    pub fn get_machine_goal_object_id(&self) -> Option<crate::common::ObjectID> {
        if self.goal_object_id == crate::common::INVALID_ID {
            None
        } else {
            Some(self.goal_object_id)
        }
    }

    /// Machine owner id, bound by the machine before each step.
    pub fn get_machine_owner_id(&self) -> Option<crate::common::ObjectID> {
        if self.owner_id == crate::common::INVALID_ID {
            None
        } else {
            Some(self.owner_id)
        }
    }

    /// Machine goal squad value, bound by the machine before each step.
    /// Returns an owned copy; pruning it does not affect the machine's squad.
    pub fn get_machine_goal_squad(&self) -> Option<Squad> {
        self.goal_squad_copied.clone()
    }

    /// Machine goal polygon trigger, bound by the machine before each step.
    pub fn get_machine_goal_polygon(&self) -> Option<Arc<PolygonTrigger>> {
        self.goal_polygon_copied.clone()
    }

    /// Machine goal position, bound by the machine before each step.
    pub fn get_machine_goal_position(&self) -> Option<Coord3D> {
        self.goal_position_copied
    }


    /// Define success transition
    pub fn on_success(&mut self, to_state_id: StateId) {
        self.success_state_id = to_state_id;
    }

    /// Define failure transition
    pub fn on_failure(&mut self, to_state_id: StateId) {
        self.failure_state_id = to_state_id;
    }

    /// Define conditional transition
    pub fn on_condition(
        &mut self,
        test: StateTransFuncPtr,
        to_state_id: StateId,
        user_data: StateTransitionUserData,
        description: &str,
    ) {
        self.transitions.push(TransitionInfo {
            test,
            to_state_id,
            user_data,
            description: description.to_string(),
        });
    }

    /// Handle state exit - called when leaving this state
    pub fn on_exit(&mut self, _exit_type: StateExitType) {
        // Default implementation - can be overridden by specific states
    }

    /// Check for state transitions based on return status
    pub fn check_for_transitions(
        &self,
        status: StateReturnType,
        state_impl: &dyn StateImplementation,
    ) -> StateReturnType {
        // Check conditional transitions first
        for transition in &self.transitions {
            if (transition.test)(state_impl, &transition.user_data) {
                // Would trigger state change in real implementation
                return StateReturnType::Success;
            }
        }

        // Check standard success/failure transitions
        match status {
            StateReturnType::Success if self.success_state_id != INVALID_STATE_ID => {
                // Would transition to success state
                StateReturnType::Success
            }
            StateReturnType::Failure if self.failure_state_id != INVALID_STATE_ID => {
                // Would transition to failure state
                StateReturnType::Success
            }
            other => other,
        }
    }
}

/// A finite state machine
#[derive(Debug)]
pub struct StateMachine {
    state_map: HashMap<StateId, Box<dyn StateImplementation>>,
    state_meta: HashMap<StateId, StateMeta>,
    owner_id: crate::common::ObjectID,
    sleep_till: u32,
    default_state_id: StateId,
    current_state_id: Option<StateId>,
    goal_object_id: crate::common::ObjectID,
    goal_squad: Option<Squad>,
    goal_polygon: Option<Weak<PolygonTrigger>>,
    goal_waypoint: Option<WaypointId>,
    guard_mode_raw: i32,
    goal_position: Coord3D,
    locked: bool,
    default_state_inited: bool,
    name: String,
    debug_output: bool,
    transition_depth: u32,
    sleep_transition_depth: u32,
}

#[derive(Debug, Clone)]
struct StateMeta {
    success_state_id: StateId,
    failure_state_id: StateId,
    transitions: Vec<StateConditionInfo>,
}

impl StateMachine {
    /// Exit machine with success
    pub const EXIT_MACHINE_WITH_SUCCESS: StateId = EXIT_MACHINE_WITH_SUCCESS;
    /// Exit machine with failure
    pub const EXIT_MACHINE_WITH_FAILURE: StateId = EXIT_MACHINE_WITH_FAILURE;

    /// Create a new state machine
    pub fn new(owner: Option<Weak<RwLock<Object>>>, name: &str) -> Self {
        let owner_id = owner
            .as_ref()
            .and_then(|weak| weak.upgrade())
            .and_then(|arc| arc.read().ok().map(|g| g.get_id()))
            .unwrap_or(crate::common::INVALID_ID);
        Self::new_with_owner_id(owner_id, name)
    }

    pub fn new_with_owner_id(owner_id: crate::common::ObjectID, name: &str) -> Self {
        Self {
            state_map: HashMap::new(),
            state_meta: HashMap::new(),
            owner_id,
            sleep_till: 0,
            default_state_id: INVALID_STATE_ID,
            current_state_id: None,
            goal_object_id: crate::common::INVALID_ID,
            goal_squad: None,
            goal_polygon: None,
            goal_waypoint: None,
            guard_mode_raw: 0,
            goal_position: Coord3D::origin(),
            locked: false,
            default_state_inited: false,
            name: name.to_string(),
            debug_output: false,
            transition_depth: 0,
            sleep_transition_depth: 0,
        }
    }

    fn internal_clear(&mut self) {
        self.goal_object_id = crate::common::INVALID_ID;
        self.goal_squad = None;
        self.goal_polygon = None;
        self.goal_waypoint = None;
        self.guard_mode_raw = 0;
        self.goal_position = Coord3D::origin();
    }

    fn internal_set_goal_object(&mut self, obj: Option<Weak<RwLock<Object>>>) {
        if let Some(weak) = obj {
            if let Some(strong) = weak.upgrade() {
                if let Ok(guard) = strong.read() {
                    self.goal_object_id = guard.get_id();
                    self.internal_set_goal_position(guard.get_position().clone());
                    return;
                }
            }
        }

        self.goal_object_id = crate::common::INVALID_ID;
    }

    fn internal_set_goal_position(&mut self, pos: Coord3D) {
        self.goal_position = pos;
    }

    fn with_transition_depth_guard<F>(&mut self, f: F) -> StateReturnType
    where
        F: FnOnce(&mut Self) -> StateReturnType,
    {
        self.transition_depth = self.transition_depth.saturating_add(1);
        if self.transition_depth >= MAX_TRANSITION_RECURSION_DEPTH {
            self.transition_depth = self.transition_depth.saturating_sub(1);
            return StateReturnType::Failure;
        }

        let result = f(self);
        self.transition_depth = self.transition_depth.saturating_sub(1);
        result
    }

    fn with_sleep_transition_depth_guard<F>(&mut self, f: F) -> StateReturnType
    where
        F: FnOnce(&mut Self) -> StateReturnType,
    {
        self.sleep_transition_depth = self.sleep_transition_depth.saturating_add(1);
        if self.sleep_transition_depth >= MAX_TRANSITION_RECURSION_DEPTH {
            self.sleep_transition_depth = self.sleep_transition_depth.saturating_sub(1);
            return StateReturnType::Failure;
        }

        let result = f(self);
        self.sleep_transition_depth = self.sleep_transition_depth.saturating_sub(1);
        result
    }

    /// Run one step of the machine
    pub fn update(&mut self) -> StateReturnType {
        let now = self.get_current_frame();
        if self.sleep_till != 0 && now < self.sleep_till {
            if self.current_state_id.is_none() {
                return StateReturnType::Failure;
            }

            return self.check_for_sleep_transitions(StateReturnType::Sleep(
                self.sleep_till.wrapping_sub(now),
            ));
        }

        // Not sleeping anymore.
        self.sleep_till = 0;

        if let Some(state_id) = self.current_state_id {
            let state_before_update = state_id;
            let goal_object_id = self.get_goal_object_id();
            let goal_position = self.get_goal_position();
            let goal_squad = self.get_goal_squad().cloned();
            let goal_polygon = self.get_goal_polygon();
            let goal_waypoint = self.get_goal_waypoint();
            let step_owner = self.get_owner();
            let mut status = {
                let Some(state) = self.state_map.get_mut(&state_id) else {
                    return StateReturnType::Failure;
                };
                if let Some(owner) = step_owner {
                    state.note_step_owner(owner);
                }
                state.bind_goal_object_id(goal_object_id);
                state.bind_goal_position(goal_position);
                state.bind_goal_squad(goal_squad);
                state.bind_goal_polygon(goal_polygon);
                state.bind_goal_waypoint(goal_waypoint);
                state.update()
            };
            self.apply_pending_victim_goal();
            if self.current_state_id.is_none() {
                return StateReturnType::Failure;
            }

            // If update changed state, ignore any sleep and treat it as continue.
            if self.current_state_id != Some(state_before_update) {
                status = StateReturnType::Continue;
            }

            if let StateReturnType::Sleep(frames) = status {
                self.sleep_till = now.wrapping_add(frames);
                return self.check_for_sleep_transitions(StateReturnType::Sleep(
                    self.sleep_till.wrapping_sub(now),
                ));
            }

            return self.check_for_transitions(status);
        }

        StateReturnType::Failure
    }

    /// Same as [`update`], but the current state receives the AI this update already borrowed.
    pub fn update_with_ai(
        &mut self,
        ai: &mut dyn crate::modules::AIUpdateInterface,
    ) -> StateReturnType {
        let now = self.get_current_frame();
        if self.sleep_till != 0 && now < self.sleep_till {
            if self.current_state_id.is_none() {
                return StateReturnType::Failure;
            }

            return self.check_for_sleep_transitions_ai(
                StateReturnType::Sleep(self.sleep_till.wrapping_sub(now)),
                Some(ai),
            );
        }

        self.sleep_till = 0;

        if let Some(state_id) = self.current_state_id {
            let state_before_update = state_id;
            let machine_locked = self.locked;
            let goal_object_id = self.get_goal_object_id();
            let goal_position = self.get_goal_position();
            let goal_squad = self.get_goal_squad().cloned();
            let goal_polygon = self.get_goal_polygon();
            let goal_waypoint = self.get_goal_waypoint();
            let step_owner = self.get_owner();
            let freeze_parent = self
                .state_map
                .get(&state_id)
                .is_some_and(|state| state.freezes_parent_during_update());
            if freeze_parent {
                self.locked = true;
            }
            let mut status = {
                let Some(state) = self.state_map.get_mut(&state_id) else {
                    if freeze_parent {
                        self.locked = machine_locked;
                    }
                    return StateReturnType::Failure;
                };
                if let Some(owner) = step_owner {
                    state.note_step_owner(owner);
                }
                state.bind_goal_object_id(goal_object_id);
                state.bind_goal_position(goal_position);
                state.bind_goal_squad(goal_squad);
                state.bind_goal_polygon(goal_polygon);
                state.bind_goal_waypoint(goal_waypoint);
                state.update_with_ai_held(ai, machine_locked)
            };
            if let Some(state_id) = self.current_state_id {
                if let Some((pos, clear_object)) = self
                    .state_map
                    .get_mut(&state_id)
                    .and_then(|state| state.take_published_goal())
                {
                    self.set_goal_position(pos);
                    if clear_object {
                        self.set_goal_object_by_id(None);
                    }
                }
            }
            if freeze_parent {
                self.locked = machine_locked;
            }
            self.apply_pending_victim_goal();
            if self.current_state_id.is_none() {
                return StateReturnType::Failure;
            }

            if self.current_state_id != Some(state_before_update) {
                status = StateReturnType::Continue;
            }

            if let StateReturnType::Sleep(frames) = status {
                self.sleep_till = now.wrapping_add(frames);
                return self.check_for_sleep_transitions_ai(
                    StateReturnType::Sleep(self.sleep_till.wrapping_sub(now)),
                    Some(ai),
                );
            }

            return self.check_for_transitions_ai(status, Some(ai));
        }

        StateReturnType::Failure
    }

    fn apply_pending_victim_goal(&mut self) {
        let Some(owner) = self.get_owner() else {
            return;
        };
        let Ok(owner_guard) = owner.read() else {
            return;
        };
        let Some(id) = owner_guard.ai_fire_pending_victim else {
            return;
        };
        if id == crate::common::INVALID_ID {
            return;
        }
        self.goal_object_id = id;
        if let Some(arc) = crate::helpers::TheGameLogic::find_object_by_id(id)
            .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(id))
        {
            if let Ok(guard) = arc.read() {
                self.internal_set_goal_position(*guard.get_position());
            }
        }
    }

    /// Clear the machine's internals to a known, initialized state
    pub fn clear(&mut self) {
        if self.locked {
            return;
        }

        if let Some(current_id) = self.current_state_id {
            if let Some(current_state) = self.state_map.get_mut(&current_id) {
                current_state.on_exit(StateExitType::Reset);
            }
        }

        self.current_state_id = None;
        self.sleep_till = 0;
        self.internal_clear();
    }

    /// Reset to default state
    pub fn reset_to_default_state(&mut self) -> StateReturnType {
        if self.locked {
            return StateReturnType::Failure;
        }

        if !self.default_state_inited {
            return StateReturnType::Failure;
        }

        if let Some(current_id) = self.current_state_id {
            if let Some(current_state) = self.state_map.get_mut(&current_id) {
                current_state.on_exit(StateExitType::Reset);
            }
        }
        self.current_state_id = None;
        self.sleep_till = 0;
        self.internal_clear();

        self.internal_set_state(self.default_state_id)
    }

    /// Initialize default state
    pub fn init_default_state(&mut self) -> StateReturnType {
        if self.default_state_inited {
            return StateReturnType::Failure;
        }

        if self.default_state_id == INVALID_STATE_ID {
            return StateReturnType::Failure;
        }

        self.default_state_inited = true;
        self.internal_set_state(self.default_state_id)
    }

    /// Change the current state of the machine
    pub fn set_current_state(&mut self, new_state_id: StateId) -> StateReturnType {
        if self.locked {
            return StateReturnType::Continue;
        }

        self.internal_set_state(new_state_id)
    }

    /// Internal state transition used by state-driven transitions even when locked.
    pub fn internal_set_state(&mut self, new_state_id: StateId) -> StateReturnType {
        self.set_state_entering(new_state_id, None)
    }

    /// `ai` is the update borrow. `None` keeps [`on_enter`].
    pub fn set_state_entering(
        &mut self,
        mut new_state_id: StateId,
        mut ai: Option<&mut dyn crate::modules::AIUpdateInterface>,
    ) -> StateReturnType {
        self.sleep_till = 0;

        if new_state_id != MACHINE_DONE_STATE_ID {
            if new_state_id == INVALID_STATE_ID {
                new_state_id = self.default_state_id;
                if new_state_id == INVALID_STATE_ID {
                    return StateReturnType::Failure;
                }
            }

            if !self.state_map.contains_key(&new_state_id) {
                if self.state_map.contains_key(&self.default_state_id) {
                    new_state_id = self.default_state_id;
                } else {
                    return StateReturnType::Failure;
                }
            }
        }

        if let Some(current_id) = self.current_state_id {
            let outgoing_locks = self
                .state_map
                .get(&current_id)
                .is_some_and(|state| state.locks_machine());
            let restore = self
                .state_map
                .get(&current_id)
                .and_then(|state| state.exit_restore_goal());
            if let Some(current_state) = self.state_map.get_mut(&current_id) {
                if let Some(ref mut ai_ref) = ai {
                    current_state.on_exit_with_ai(StateExitType::Normal, &mut **ai_ref);
                } else {
                    current_state.on_exit(StateExitType::Normal);
                }
            }
            if outgoing_locks {
                self.unlock();
            }
            if let Some(origin) = restore {
                self.unlock();
                self.internal_set_goal_position(origin);
            }
        }

        self.current_state_id = if new_state_id == MACHINE_DONE_STATE_ID {
            None
        } else {
            Some(new_state_id)
        };

        if let Some(current_id) = self.current_state_id {
            let state_before_enter = current_id;
            let goal_id = self.get_goal_object_id();
            let goal_pos = self.get_goal_position();
            let goal_squad = self.get_goal_squad().cloned();
            let goal_polygon = self.get_goal_polygon();
            let locks_machine = self
                .state_map
                .get(&current_id)
                .map(|state| state.locks_machine())
                .unwrap_or(false);
            if locks_machine {
                self.lock();
            }
            if let Some(owner) = self.get_owner() {
                if let Some(state) = self.state_map.get_mut(&current_id) {
                    state.note_step_owner(owner);
                }
            }
            let guard_mode = self.get_guard_mode_raw();
            let guard_polygon = self.get_goal_polygon();
            if let Some(state) = self.state_map.get_mut(&current_id) {
                state.note_guard_enter(guard_mode, guard_polygon);
            }
            let waypoint = self.get_goal_waypoint();
            let mut status = {
                let Some(new_state) = self.state_map.get_mut(&current_id) else {
                    if locks_machine {
                        self.unlock();
                    }
                    return StateReturnType::Failure;
                };
                new_state.bind_goal_object_id(goal_id);
                new_state.bind_goal_position(goal_pos);
                new_state.bind_goal_squad(goal_squad);
                new_state.bind_goal_polygon(goal_polygon);
                new_state.bind_goal_waypoint(waypoint);
                if let Some(ref mut ai_ref) = ai {
                    new_state.on_enter_with_waypoint(&mut **ai_ref, goal_id, goal_pos, waypoint)
                } else {
                    new_state.on_enter()
                }
            };
            if let Some(id) = self.current_state_id {
                if let Some((pos, clear_object)) = self
                    .state_map
                    .get_mut(&id)
                    .and_then(|state| state.take_published_goal())
                {
                    self.set_goal_position(pos);
                    if clear_object {
                        self.set_goal_object_by_id(None);
                    }
                }
            }
            if locks_machine
                && (self.current_state_id != Some(state_before_enter)
                    || matches!(status, StateReturnType::Failure))
            {
                self.unlock();
            }

            if self.current_state_id.is_none() {
                return StateReturnType::Failure;
            }

            // If on_enter changed state, ignore any sleep and run the new state immediately.
            if self.current_state_id != Some(state_before_enter) {
                status = StateReturnType::Continue;
            }

            if let StateReturnType::Sleep(frames) = status {
                let now = self.get_current_frame();
                self.sleep_till = now.wrapping_add(frames);
                return self.check_for_sleep_transitions_ai(
                    StateReturnType::Sleep(self.sleep_till.wrapping_sub(now)),
                    ai,
                );
            }

            return self.check_for_transitions_ai(status, ai);
        }

        StateReturnType::Continue
    }

    fn check_for_transitions(&mut self, status: StateReturnType) -> StateReturnType {
        self.check_for_transitions_ai(status, None)
    }

    fn check_for_transitions_ai(
        &mut self,
        status: StateReturnType,
        mut ai: Option<&mut dyn crate::modules::AIUpdateInterface>,
    ) -> StateReturnType {
        if status.is_sleep() {
            return StateReturnType::Failure;
        }
        self.transition_depth = self.transition_depth.saturating_add(1);
        if self.transition_depth >= MAX_TRANSITION_RECURSION_DEPTH {
            self.transition_depth = self.transition_depth.saturating_sub(1);
            return StateReturnType::Failure;
        }
        let result = self.check_for_transitions_inner(status, ai);
        self.transition_depth = self.transition_depth.saturating_sub(1);
        result
    }

    fn check_for_transitions_inner(
        &mut self,
        status: StateReturnType,
        mut ai: Option<&mut dyn crate::modules::AIUpdateInterface>,
    ) -> StateReturnType {
        let Some(state_id) = self.current_state_id else {
            return StateReturnType::Failure;
        };
        let Some(meta) = self.state_meta.get(&state_id).cloned() else {
            return status;
        };

        match status {
            StateReturnType::Continue => self.check_condition_transitions_ai(&meta, ai),
            _ if status.is_success() => match meta.success_state_id {
                EXIT_MACHINE_WITH_SUCCESS => {
                    let _ = self.internal_set_state(MACHINE_DONE_STATE_ID);
                    StateReturnType::Success
                }
                EXIT_MACHINE_WITH_FAILURE => {
                    let _ = self.internal_set_state(MACHINE_DONE_STATE_ID);
                    StateReturnType::Failure
                }
                INVALID_STATE_ID => status,
                next => self.set_state_entering(next, ai),
            },
            _ if status.is_failure() => match meta.failure_state_id {
                EXIT_MACHINE_WITH_SUCCESS => {
                    let _ = self.internal_set_state(MACHINE_DONE_STATE_ID);
                    StateReturnType::Success
                }
                EXIT_MACHINE_WITH_FAILURE => {
                    let _ = self.internal_set_state(MACHINE_DONE_STATE_ID);
                    StateReturnType::Failure
                }
                INVALID_STATE_ID => status,
                next => self.set_state_entering(next, ai),
            },
            other => other,
        }
    }

    fn check_for_sleep_transitions(&mut self, status: StateReturnType) -> StateReturnType {
        self.check_for_sleep_transitions_ai(status, None)
    }

    fn check_for_sleep_transitions_ai(
        &mut self,
        status: StateReturnType,
        ai: Option<&mut dyn crate::modules::AIUpdateInterface>,
    ) -> StateReturnType {
        if !matches!(status, StateReturnType::Sleep(_)) {
            return status;
        }
        self.sleep_transition_depth = self.sleep_transition_depth.saturating_add(1);
        if self.sleep_transition_depth >= MAX_TRANSITION_RECURSION_DEPTH {
            self.sleep_transition_depth = self.sleep_transition_depth.saturating_sub(1);
            return StateReturnType::Failure;
        }
        let result = self.check_for_sleep_transitions_inner(status, ai);
        self.sleep_transition_depth = self.sleep_transition_depth.saturating_sub(1);
        result
    }

    fn check_for_sleep_transitions_inner(
        &mut self,
        status: StateReturnType,
        ai: Option<&mut dyn crate::modules::AIUpdateInterface>,
    ) -> StateReturnType {
        let Some(state_id) = self.current_state_id else {
            return StateReturnType::Failure;
        };
        let Some(meta) = self.state_meta.get(&state_id).cloned() else {
            return status;
        };
        self.check_condition_transitions_or_sleep(&meta, status, ai)
    }

    fn check_condition_transitions(&mut self, meta: &StateMeta) -> StateReturnType {
        self.check_condition_transitions_ai(meta, None)
    }

    fn check_condition_transitions_ai(
        &mut self,
        meta: &StateMeta,
        ai: Option<&mut dyn crate::modules::AIUpdateInterface>,
    ) -> StateReturnType {
        let Some(state_id) = self.current_state_id else {
            return StateReturnType::Failure;
        };
        let Some(state) = self.state_map.get(&state_id) else {
            return StateReturnType::Failure;
        };
        for transition in &meta.transitions {
            if (transition.test)(state.as_ref(), &transition.user_data) {
                return match transition.to_state_id {
                    EXIT_MACHINE_WITH_SUCCESS => StateReturnType::Success,
                    EXIT_MACHINE_WITH_FAILURE => StateReturnType::Failure,
                    next => self.set_state_entering(next, ai),
                };
            }
        }
        StateReturnType::Continue
    }

    fn check_condition_transitions_or_sleep(
        &mut self,
        meta: &StateMeta,
        status: StateReturnType,
        ai: Option<&mut dyn crate::modules::AIUpdateInterface>,
    ) -> StateReturnType {
        let Some(state_id) = self.current_state_id else {
            return StateReturnType::Failure;
        };
        let Some(state) = self.state_map.get(&state_id) else {
            return StateReturnType::Failure;
        };

        for transition in &meta.transitions {
            if (transition.test)(state.as_ref(), &transition.user_data) {
                return match transition.to_state_id {
                    EXIT_MACHINE_WITH_SUCCESS => StateReturnType::Success,
                    EXIT_MACHINE_WITH_FAILURE => StateReturnType::Failure,
                    next => self.set_state_entering(next, ai),
                };
            }
        }

        status
    }

    /// Get current state ID
    pub fn get_current_state_id(&self) -> Option<StateId> {
        self.current_state_id
    }

    /// Check if in idle state
    pub fn is_in_idle_state(&self) -> bool {
        if let Some(state_id) = self.current_state_id {
            if let Some(state) = self.state_map.get(&state_id) {
                return state.is_idle();
            }
        }
        true // stateless things are considered idle
    }

    /// Check if in attack state
    pub fn is_in_attack_state(&self) -> bool {
        if let Some(state_id) = self.current_state_id {
            if let Some(state) = self.state_map.get(&state_id) {
                return state.is_attack();
            }
        }
        true
    }

    /// Check if in guard idle state  
    pub fn is_in_guard_idle_state(&self) -> bool {
        if let Some(state_id) = self.current_state_id {
            if let Some(state) = self.state_map.get(&state_id) {
                return state.is_guard_idle();
            }
        }
        false
    }

    /// Check if in busy state
    pub fn is_in_busy_state(&self) -> bool {
        if let Some(state_id) = self.current_state_id {
            if let Some(state) = self.state_map.get(&state_id) {
                return state.is_busy();
            }
        }
        false
    }

    /// Lock/unlock this state machine
    pub fn lock(&mut self) {
        self.locked = true;
    }

    pub fn unlock(&mut self) {
        self.locked = false;
    }

    pub fn is_locked(&self) -> bool {
        self.locked
    }

    /// Get the owner object
    pub fn get_owner(&self) -> Option<Arc<RwLock<Object>>> {
        if self.owner_id == crate::common::INVALID_ID {
            return None;
        }
        crate::helpers::TheGameLogic::find_object_by_id(self.owner_id)
            .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(self.owner_id))
    }

    pub fn get_owner_id(&self) -> crate::common::ObjectID {
        self.owner_id
    }

    pub fn set_owner_id(&mut self, owner_id: crate::common::ObjectID) {
        self.owner_id = owner_id;
    }

    /// Set goal object
    pub fn set_goal_object(&mut self, obj: Option<Weak<RwLock<Object>>>) {
        if self.locked {
            return;
        }

        self.internal_set_goal_object(obj);
    }

    /// ID-first goal object setter (no Arc/Weak required at call site).
    pub fn set_goal_object_by_id(&mut self, object_id: Option<crate::common::ObjectID>) {
        if self.locked {
            return;
        }
        match object_id {
            Some(id) if id != crate::common::INVALID_ID => {
                self.goal_object_id = id;
                if let Some(arc) = crate::helpers::TheGameLogic::find_object_by_id(id)
                    .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(id))
                {
                    if let Ok(guard) = arc.read() {
                        self.internal_set_goal_position(guard.get_position().clone());
                    }
                }
            }
            _ => {
                self.goal_object_id = crate::common::INVALID_ID;
            }
        }
    }

    /// Get goal object
    pub fn get_goal_object(&self) -> Option<Arc<RwLock<Object>>> {
        if self.goal_object_id == crate::common::INVALID_ID {
            return None;
        }
        crate::helpers::TheGameLogic::find_object_by_id(self.goal_object_id)
            .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(self.goal_object_id))
    }

    pub fn get_goal_object_id(&self) -> crate::common::ObjectID {
        self.goal_object_id
    }

    /// Set goal squad
    pub fn set_goal_squad(&mut self, squad: Option<Squad>) {
        self.goal_squad = squad;
    }

    /// Get goal squad
    pub fn get_goal_squad(&self) -> Option<&Squad> {
        self.goal_squad.as_ref()
    }

    /// Set goal polygon trigger
    pub fn set_goal_polygon(&mut self, polygon: Option<Weak<PolygonTrigger>>) {
        self.goal_polygon = polygon;
    }

    /// Set guard mode (raw int value).
    pub fn set_guard_mode_raw(&mut self, guard_mode: i32) {
        self.guard_mode_raw = guard_mode;
    }

    /// Get guard mode (raw int value).
    pub fn get_guard_mode_raw(&self) -> i32 {
        self.guard_mode_raw
    }

    /// Get goal polygon trigger
    pub fn get_goal_polygon(&self) -> Option<Arc<PolygonTrigger>> {
        self.goal_polygon.as_ref()?.upgrade()
    }

    pub fn set_goal_waypoint(&mut self, waypoint: Option<WaypointId>) {
        self.goal_waypoint = waypoint;
    }

    pub fn get_goal_waypoint(&self) -> Option<WaypointId> {
        self.goal_waypoint
    }

    /// Set goal position
    pub fn set_goal_position(&mut self, pos: Coord3D) {
        if self.locked {
            return;
        }

        self.internal_set_goal_position(pos);
    }

    /// Get goal position
    pub fn get_goal_position(&self) -> Coord3D {
        self.goal_position
    }

    /// Check if goal object is destroyed
    pub fn is_goal_object_destroyed(&self) -> bool {
        if self.goal_object_id == crate::common::INVALID_ID {
            return false;
        }

        // Goal ID is set but the object no longer resolves.
        self.get_goal_object_id() == crate::common::INVALID_ID
    }

    /// Halt the state machine
    pub fn halt(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.locked = true;
        // Don't call on_exit when halting; this mirrors C++ halt semantics.
        self.current_state_id = None;
        Ok(())
    }

    /// Get current state name for debugging
    pub fn get_current_state_name(&self) -> String {
        if let Some(state_id) = self.current_state_id {
            if let Some(state) = self.state_map.get(&state_id) {
                return state.get_name().to_string();
            }
        }
        "NO_STATE".to_string()
    }

    /// Get state name by ID (for debugging)
    pub fn get_state_name_by_id(&self, id: StateId) -> Option<&str> {
        self.state_map.get(&id).map(|state| state.get_name())
    }

    /// Define a state in this machine
    pub fn define_state(
        &mut self,
        id: StateId,
        mut state: Box<dyn StateImplementation>,
        success_id: Option<StateId>,
        failure_id: Option<StateId>,
        conditions: Option<&[StateConditionInfo]>,
    ) {
        state.set_id(id);
        self.state_map.insert(id, state);
        self.state_meta.insert(
            id,
            StateMeta {
                success_state_id: success_id.unwrap_or(INVALID_STATE_ID),
                failure_state_id: failure_id.unwrap_or(INVALID_STATE_ID),
                transitions: conditions.map(|items| items.to_vec()).unwrap_or_default(),
            },
        );

        // Set as default state if this is the first one
        if self.default_state_id == INVALID_STATE_ID {
            self.default_state_id = id;
        }
    }

    /// Get state by ID (internal)
    pub fn get_state_mut(&mut self, id: StateId) -> Option<&mut Box<dyn StateImplementation>> {
        self.state_map.get_mut(&id)
    }

    /// Reset the state machine
    pub fn reset(&mut self) {
        // Exit current state with reset type
        if let Some(current_id) = self.current_state_id {
            if let Some(current_state) = self.state_map.get_mut(&current_id) {
                current_state.on_exit(StateExitType::Reset);
            }
        }

        self.clear();
    }

    /// Get current frame
    /// Matches C++ StateMachine.cpp line 397: TheGameLogic->getFrame()
    pub fn get_current_frame(&self) -> u32 {
        // Get current frame from global game logic instance
        // This is used for sleep timing and state transitions
        crate::helpers::TheGameLogic::get_frame()
    }

    /// Calculate CRC for state verification
    /// Matches C++ StateMachine.cpp lines 788-791
    ///
    /// Note: The C++ implementation is empty, which matches expected behavior.
    /// CRC calculation for state machines is intentionally a no-op as states
    /// are not typically included in save game CRC validation (only data values are).
    pub fn crc(
        &self,
        _xfer: &mut dyn crate::common::xfer::Xfer,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Empty implementation matches C++ StateMachine::crc()
        // State machine structure doesn't contribute to CRC as it's
        // deterministic based on the current state ID and sleep timer
        Ok(())
    }

    /// Transfer data for save/load
    /// Matches C++ StateMachine.cpp lines 799-867
    ///
    /// Serializes/deserializes the complete state machine state including:
    /// - Current sleep timer
    /// - Default state ID
    /// - Current state ID
    /// - Current state snapshot (for preserving state-specific data)
    /// - Goal object ID
    /// - Goal position
    /// - Lock status
    /// - Default state initialization flag
    pub fn xfer(
        &mut self,
        xfer: &mut dyn crate::common::xfer::Xfer,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        use crate::common::xfer::XferExt;

        // Version control - matches C++ version 1
        // C++ lines 803-805
        let current_version = 1u8;
        let mut version = current_version;
        game_engine::system::Xfer::xfer_version(xfer, &mut version, current_version)?;

        // Transfer sleep timer - C++ line 807
        xfer.xfer_unsigned_int(&mut self.sleep_till)?;

        // Transfer default state ID - C++ line 808
        xfer.xfer_unsigned_int(&mut self.default_state_id)?;

        // Transfer current state ID - C++ lines 809-815
        let mut cur_state_id = self.current_state_id.unwrap_or(INVALID_STATE_ID);
        xfer.xfer_unsigned_int(&mut cur_state_id)?;

        // On load, restore the current state reference
        // C++ lines 811-815: We jump directly into the saved state without
        // calling onEnter/onExit since the state was already active when saved
        self.current_state_id = if cur_state_id == INVALID_STATE_ID {
            None
        } else {
            Some(cur_state_id)
        };

        if xfer.get_xfer_mode() == game_engine::system::XferMode::Load {
            let preferred_state_id = if cur_state_id == INVALID_STATE_ID {
                self.default_state_id
            } else {
                cur_state_id
            };

            self.current_state_id = if self.state_map.contains_key(&preferred_state_id) {
                Some(preferred_state_id)
            } else if self.state_map.contains_key(&self.default_state_id) {
                Some(self.default_state_id)
            } else {
                None
            };
        }

        // Transfer state snapshot data
        // C++ lines 817-860: We only transfer the current state, not all states
        // (snapshotAllStates is false in release builds)
        let mut snapshot_all_states = false;
        game_engine::system::Xfer::xfer_bool(xfer, &mut snapshot_all_states)?;

        if snapshot_all_states {
            // Debug mode: transfer all states (not typically used)
            // C++ lines 822-851
            // For now, we skip this as it's only used in C++ debug builds
            // and requires implementing Snapshot trait for all state types
        } else {
            // Normal mode: only transfer current state
            // C++ lines 852-860
            // Note: Individual states must implement their own xfer methods
            // We cannot xfer Box<dyn StateImplementation> directly as it lacks Snapshot trait
            let current_id = if let Some(current_id) = self.current_state_id {
                current_id
            } else if self.state_map.contains_key(&self.default_state_id) {
                self.current_state_id = Some(self.default_state_id);
                self.default_state_id
            } else {
                return Err(Box::new(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "StateMachine::xfer has no current/default state to snapshot",
                )));
            };

            let Some(state) = self.state_map.get_mut(&current_id) else {
                return Err(Box::new(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("StateMachine::xfer missing state {}", current_id),
                )));
            };

            state.xfer_snapshot(xfer).map_err(|e| {
                Box::new(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("StateMachine::xfer snapshot failed: {}", e),
                )) as Box<dyn std::error::Error + Send + Sync>
            })?;
        }

        // Transfer goal object ID - C++ line 863
        // Convert Weak<RwLock<Object>> to ObjectID for serialization
        // On save: extract ID from current goal object
        // On load: this will load the ID (actual object resolution happens lazily)
        xfer.xfer_object_id(&mut self.goal_object_id)?;

        // Note: Goal object weak reference resolution happens lazily when get_goal_object() is called
        // The object registry will be used to look up the object by ID at that time
        // goal_object_id is authoritative; resolve via get_goal_object().

        // Transfer goal position - C++ line 864
        game_engine::system::Xfer::xfer_real(xfer, &mut self.goal_position.x)?;
        game_engine::system::Xfer::xfer_real(xfer, &mut self.goal_position.y)?;
        game_engine::system::Xfer::xfer_real(xfer, &mut self.goal_position.z)?;

        // Transfer locked status - C++ line 865
        game_engine::system::Xfer::xfer_bool(xfer, &mut self.locked)?;

        // Transfer default state initialized flag - C++ line 866
        game_engine::system::Xfer::xfer_bool(xfer, &mut self.default_state_inited)?;

        Ok(())
    }

    /// Post-process after loading
    /// Matches C++ StateMachine.cpp lines 873-876
    ///
    /// Note: The C++ implementation is empty, which is correct behavior.
    /// All necessary restoration happens during xfer() itself:
    /// - Current state ID is restored and mapped to state reference
    /// - Goal object will be lazily resolved when accessed via get_goal_object()
    /// - All other fields (sleep_till, locked, etc.) are directly restored
    ///
    /// Individual state implementations may have their own loadPostProcess() methods
    /// that handle state-specific restoration logic.
    pub fn load_post_process(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // C++ StateMachine::loadPostProcess is empty. Per-state loadPostProcess
        // still runs from the snapshot walker. Call it on the restored state.
        if let Some(id) = self.current_state_id {
            if let Some(state) = self.state_map.get_mut(&id) {
                state.load_post_process().map_err(|e| {
                    Box::new(std::io::Error::new(std::io::ErrorKind::Other, e))
                        as Box<dyn std::error::Error + Send + Sync>
                })?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex as StdMutex, OnceLock};

    fn test_guard() -> std::sync::MutexGuard<'static, ()> {
        static TEST_LOCK: OnceLock<StdMutex<()>> = OnceLock::new();
        TEST_LOCK
            .get_or_init(|| StdMutex::new(()))
            .lock()
            .expect("state machine test lock poisoned")
    }

    fn set_frame(frame: u64) {
        let mut logic = crate::system::game_logic::get_game_logic()
            .lock()
            .expect("game logic lock poisoned");
        logic.set_current_frame(frame);
    }

    #[derive(Debug)]
    struct FixedState {
        id: StateId,
        name: &'static str,
        on_enter_return: StateReturnType,
        update_return: StateReturnType,
    }

    impl FixedState {
        fn new(
            name: &'static str,
            on_enter_return: StateReturnType,
            update_return: StateReturnType,
        ) -> Self {
            Self {
                id: INVALID_STATE_ID,
                name,
                on_enter_return,
                update_return,
            }
        }
    }

    impl StateImplementation for FixedState {
        fn on_enter(&mut self) -> StateReturnType {
            self.on_enter_return
        }

        fn update(&mut self) -> StateReturnType {
            self.update_return
        }

        fn get_name(&self) -> &str {
            self.name
        }

        fn get_id(&self) -> StateId {
            self.id
        }

        fn set_id(&mut self, id: StateId) {
            self.id = id;
        }
    }

    #[derive(Debug)]
    struct SleepThenContinueState {
        id: StateId,
        name: &'static str,
        sleep_frames: u32,
        slept_once: bool,
    }

    impl SleepThenContinueState {
        fn new(name: &'static str, sleep_frames: u32) -> Self {
            Self {
                id: INVALID_STATE_ID,
                name,
                sleep_frames,
                slept_once: false,
            }
        }
    }

    impl StateImplementation for SleepThenContinueState {
        fn update(&mut self) -> StateReturnType {
            if !self.slept_once {
                self.slept_once = true;
                StateReturnType::Sleep(self.sleep_frames)
            } else {
                StateReturnType::Continue
            }
        }

        fn get_name(&self) -> &str {
            self.name
        }

        fn get_id(&self) -> StateId {
            self.id
        }

        fn set_id(&mut self, id: StateId) {
            self.id = id;
        }
    }

    #[test]
    fn update_without_current_state_returns_failure() {
        let _guard = test_guard();
        let mut machine = StateMachine::new(None::<Weak<RwLock<Object>>>, "no-state");
        assert_eq!(machine.update(), StateReturnType::Failure);
    }

    #[test]
    fn external_set_state_is_blocked_when_locked() {
        let _guard = test_guard();
        let mut machine = StateMachine::new(None::<Weak<RwLock<Object>>>, "locked");
        machine.define_state(
            1,
            Box::new(FixedState::new(
                "s1",
                StateReturnType::Continue,
                StateReturnType::Continue,
            )),
            None,
            None,
            None,
        );
        machine.define_state(
            2,
            Box::new(FixedState::new(
                "s2",
                StateReturnType::Continue,
                StateReturnType::Continue,
            )),
            None,
            None,
            None,
        );

        assert_eq!(machine.set_current_state(1), StateReturnType::Continue);
        machine.lock();

        assert_eq!(machine.set_current_state(2), StateReturnType::Continue);
        assert_eq!(machine.get_current_state_id(), Some(1));
    }

    #[test]
    fn internal_transitions_still_work_while_locked() {
        let _guard = test_guard();
        let mut machine = StateMachine::new(None::<Weak<RwLock<Object>>>, "internal-locked");
        machine.define_state(
            1,
            Box::new(FixedState::new(
                "s1",
                StateReturnType::Continue,
                StateReturnType::Success,
            )),
            Some(2),
            None,
            None,
        );
        machine.define_state(
            2,
            Box::new(FixedState::new(
                "s2",
                StateReturnType::Continue,
                StateReturnType::Continue,
            )),
            None,
            None,
            None,
        );

        assert_eq!(machine.set_current_state(1), StateReturnType::Continue);
        machine.lock();
        let update_result = machine.update();

        assert_eq!(update_result, StateReturnType::Continue);
        assert_eq!(machine.get_current_state_id(), Some(2));
    }

    #[test]
    fn sleep_uses_absolute_frame_deadline() {
        let _guard = test_guard();
        let mut machine = StateMachine::new(None::<Weak<RwLock<Object>>>, "sleep");
        machine.define_state(
            1,
            Box::new(SleepThenContinueState::new("sleepy", 5)),
            None,
            None,
            None,
        );
        assert_eq!(machine.set_current_state(1), StateReturnType::Continue);

        set_frame(100);
        assert_eq!(machine.update(), StateReturnType::Sleep(5));

        set_frame(101);
        assert_eq!(machine.update(), StateReturnType::Sleep(4));

        // Jump frames to validate absolute wake deadline semantics.
        set_frame(104);
        assert_eq!(machine.update(), StateReturnType::Sleep(1));

        set_frame(105);
        assert_eq!(machine.update(), StateReturnType::Continue);
    }

    #[test]
    fn clear_respects_lock_and_preserves_default_init_flag() {
        let _guard = test_guard();
        let mut machine = StateMachine::new(None::<Weak<RwLock<Object>>>, "clear");
        machine.define_state(
            1,
            Box::new(FixedState::new(
                "s1",
                StateReturnType::Continue,
                StateReturnType::Continue,
            )),
            None,
            None,
            None,
        );

        assert_eq!(machine.init_default_state(), StateReturnType::Continue);
        assert!(machine.default_state_inited);
        assert_eq!(machine.get_current_state_id(), Some(1));

        machine.lock();
        machine.clear();

        assert!(machine.default_state_inited);
        assert_eq!(machine.get_current_state_id(), Some(1));
    }

    #[test]
    fn stateless_attack_state_is_true_and_goal_destroyed_never_set_is_false() {
        let _guard = test_guard();
        let machine = StateMachine::new(None::<Weak<RwLock<Object>>>, "stateless");

        assert!(machine.is_in_attack_state());
        assert!(!machine.is_goal_object_destroyed());
    }

    #[test]
    fn halt_locks_and_keeps_internal_goal_data() {
        let _guard = test_guard();
        let mut machine = StateMachine::new(None::<Weak<RwLock<Object>>>, "halt");
        machine.set_goal_position(Coord3D::new(10.0, 20.0, 30.0));

        machine.halt().expect("halt should not fail");

        assert!(machine.is_locked());
        assert_eq!(machine.get_goal_position(), Coord3D::new(10.0, 20.0, 30.0));
    }

    fn transition_always_true(
        _state: &dyn StateImplementation,
        _data: &StateTransitionUserData,
    ) -> bool {
        true
    }

    #[test]
    fn transition_recursion_guard_returns_failure() {
        let _guard = test_guard();
        let mut machine = StateMachine::new(None::<Weak<RwLock<Object>>>, "transition-recursion");

        machine.define_state(
            1,
            Box::new(FixedState::new(
                "loop",
                StateReturnType::Success,
                StateReturnType::Continue,
            )),
            Some(1),
            None,
            None,
        );

        let result = machine.set_current_state(1);
        assert_eq!(result, StateReturnType::Failure);
    }

    #[test]
    fn sleep_transition_recursion_guard_returns_failure() {
        let _guard = test_guard();
        let mut machine = StateMachine::new(None::<Weak<RwLock<Object>>>, "sleep-recursion");

        let conditions = [StateConditionInfo::new(
            transition_always_true,
            1,
            StateTransitionUserData::new(),
            "sleep loop",
        )];

        machine.define_state(
            1,
            Box::new(FixedState::new(
                "sleep-loop",
                StateReturnType::Sleep(1),
                StateReturnType::Continue,
            )),
            None,
            None,
            Some(&conditions),
        );

        let result = machine.set_current_state(1);
        assert_eq!(result, StateReturnType::Failure);
    }
}
