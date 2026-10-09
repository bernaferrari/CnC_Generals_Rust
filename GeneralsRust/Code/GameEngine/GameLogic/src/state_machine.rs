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

/// Preserve the driver's concrete AI type while loaning its existing
/// interface to state callbacks. Trait objects use the same protocol.
pub(crate) trait StateMachineAI: crate::modules::ai_state_runtime::AiStateRuntime {
    fn as_state_runtime(&mut self) -> &mut dyn crate::modules::ai_state_runtime::AiStateRuntime;
}

impl<A: crate::modules::ai_state_runtime::AiStateRuntime> StateMachineAI for A {
    fn as_state_runtime(&mut self) -> &mut dyn crate::modules::ai_state_runtime::AiStateRuntime {
        self
    }
}

impl StateMachineAI for dyn crate::modules::ai_state_runtime::AiStateRuntime + '_ {
    fn as_state_runtime(&mut self) -> &mut dyn crate::modules::ai_state_runtime::AiStateRuntime {
        self
    }
}

/// A completed result or a body continuation tied to its driving context.
#[must_use]
pub(crate) enum StateUpdate<'step, A: StateMachineAI + ?Sized> {
    Complete(StateReturnType),
    Body(StateStep<'step, A>),
}

/// The body has returned; the machine, AI and owner stay exclusively borrowed
/// until completion. This context is operation-local, never stored or xferred.
#[must_use]
pub(crate) struct StateStep<'step, A: StateMachineAI + ?Sized> {
    machine: &'step mut StateMachine,
    ai: &'step mut A,
    owner: &'step mut dyn Any,
    now: UnsignedInt,
    state_before_update: StateId,
    status: StateReturnType,
}

impl<A: StateMachineAI + ?Sized> StateUpdate<'_, A> {
    /// Consume the result using the context borrowed when the body ran.
    pub(crate) fn finish(self) -> StateReturnType {
        match self {
            Self::Complete(result) => result,
            Self::Body(step) => step.finish(),
        }
    }
}

impl<A: StateMachineAI + ?Sized> StateStep<'_, A> {
    /// Loan the original context for synchronous terminal driver operations.
    /// The state-table body borrow has ended; these short reborrows cannot
    /// escape the operation or replace the context used by completion.
    pub(crate) fn with_driver<R>(
        &mut self,
        operation: impl FnOnce(&mut StateMachine, &mut A, &mut dyn Any) -> R,
    ) -> R {
        operation(&mut *self.machine, &mut *self.ai, &mut *self.owner)
    }

    fn finish(self) -> StateReturnType {
        let Self {
            machine,
            ai,
            owner,
            now,
            state_before_update,
            status,
        } = self;
        machine.finish_state_body(
            now,
            state_before_update,
            status,
            ai.as_state_runtime(),
            owner,
        )
    }
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
    owner_test:
        Option<fn(&dyn StateImplementation, &StateTransitionUserData, &mut dyn Any) -> bool>,
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
            owner_test: None,
            to_state_id,
            user_data,
            description: description.to_string(),
        }
    }
    /// Conditional transition reading the owner loaned to this same step.
    pub(crate) fn with_owner_test(
        mut self,
        test: fn(&dyn StateImplementation, &StateTransitionUserData, &mut dyn Any) -> bool,
    ) -> Self {
        self.owner_test = Some(test);
        self
    }

    fn evaluate(&self, state: &dyn StateImplementation, owner: &mut dyn Any) -> bool {
        match self.owner_test {
            Some(test) => test(state, &self.user_data, owner),
            None => (self.test)(state, &self.user_data),
        }
    }
}

/// State implementation trait - all AI states must implement this
pub trait StateImplementation: Any + AsAny + std::fmt::Debug + Send + Sync {
    /// Executed once when entering state
    fn on_enter(&mut self) -> StateReturnType {
        StateReturnType::Continue
    }

    /// Enter while the driving machine lends its live control for this callback.
    fn on_enter_with_control(
        &mut self,
        _control: &mut StateMachineControl,
        ai: Option<&mut dyn crate::modules::ai_state_runtime::AiStateRuntime>,
        goal_id: crate::common::ObjectID,
        goal_pos: Coord3D,
        waypoint: Option<WaypointId>,
        owner: &mut dyn Any,
    ) -> StateReturnType {
        match ai {
            Some(ai) => self.on_enter_with_ai_and_owner(ai, goal_id, goal_pos, waypoint, owner),
            None => self.on_enter_with_owner(owner),
        }
    }

    /// AI-machine enter. Default keeps the old [`on_enter`].
    fn on_enter_with_ai(
        &mut self,
        _ai: &mut dyn crate::modules::ai_state_runtime::AiStateRuntime,
        _goal_id: crate::common::ObjectID,
        _goal_pos: Coord3D,
    ) -> StateReturnType {
        self.on_enter()
    }

    /// Same enter, plus the waypoint this machine already stores.
    fn on_enter_with_waypoint(
        &mut self,
        ai: &mut dyn crate::modules::ai_state_runtime::AiStateRuntime,
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
        _ai: &mut dyn crate::modules::ai_state_runtime::AiStateRuntime,
    ) {
        self.on_exit(_status)
    }

    /// Exit can mutate the driving control without reentering its owner.
    fn on_exit_with_control(
        &mut self,
        _control: &mut StateMachineControl,
        status: StateExitType,
        ai: Option<&mut dyn crate::modules::ai_state_runtime::AiStateRuntime>,
        owner: &mut dyn Any,
    ) {
        match ai {
            Some(ai) => self.on_exit_with_ai_and_owner(status, ai, owner),
            None => self.on_exit_with_owner(status, owner),
        }
    }

    /// Evacuate freezes transitions. The step already holds this machine.
    fn locks_machine(&self) -> bool {
        false
    }

    /// A state may release a transition lock even if it did not acquire it.
    fn unlocks_machine_on_exit(&self) -> bool {
        self.locks_machine()
    }

    /// C++ docking exits notify the dock, unlock, then finish movement.
    fn on_exit_after_unlock(
        &mut self,
        _status: StateExitType,
        _ai: Option<&mut dyn crate::modules::ai_state_runtime::AiStateRuntime>,
        _owner: &mut dyn Any,
    ) {
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

    /// Owner-aware [`load_post_process`](Self::load_post_process). Machines that
    /// own their states outright (turret, guard) loan the owner so a restored
    /// state can rebuild its child against the owner's live fields — the same
    /// context C++ reached through the machine's `getOwnerAI()`. Default keeps
    /// [`Self::load_post_process`].
    fn load_post_process_with_owner(
        &mut self,
        owner: &mut dyn std::any::Any,
    ) -> Result<(), String> {
        let _ = owner;
        self.load_post_process()
    }
    /// Implements this state's behavior, decides when to change state
    fn update(&mut self) -> StateReturnType;

    /// Borrow the live control alongside this state body for one callback.
    /// Commands requiring other bodies remain the driving machine's work;
    /// this loan contains no copied classification or deferred command queue.
    fn update_with_control(
        &mut self,
        _control: &mut StateMachineControl,
        ai: Option<&mut dyn crate::modules::ai_state_runtime::AiStateRuntime>,
        machine_locked: bool,
        owner: &mut dyn Any,
    ) -> StateReturnType {
        match ai {
            Some(ai) => self.update_with_ai_and_owner(ai, machine_locked, owner),
            None => self.update_with_owner(owner),
        }
    }

    /// A callback that requests a state change as its final operation can hand
    /// it back here. The core enters that state before processing its status,
    /// preserving C++'s changed-state override of Sleep/Success/Failure.
    /// This is synchronous within this update, never a command for the next tick.
    fn take_requested_state_change(&mut self) -> Option<StateId> {
        None
    }

    /// Step for a machine that its AI owns outright (no `Arc<Mutex<_>>`).
    /// C++ states reach their owner through the machine pointer
    /// (`TurretAI.h:75` — `((TurretStateMachine*)getMachine())->getTurretAI()`).
    /// An owned machine inverts that arrow, so the owner is loaned to the step
    /// and the state downcasts it. Default keeps [`Self::update`].
    fn update_with_owner(&mut self, owner: &mut dyn std::any::Any) -> StateReturnType {
        let _ = owner;
        self.update()
    }

    /// Enter variant of [`Self::update_with_owner`]. Default keeps [`Self::on_enter`].
    fn on_enter_with_owner(&mut self, owner: &mut dyn std::any::Any) -> StateReturnType {
        let _ = owner;
        self.on_enter()
    }

    /// AI-machine step. Dock and turret keep [`update`]. Default ignores the borrow.
    fn update_with_ai(
        &mut self,
        _ai: &mut dyn crate::modules::ai_state_runtime::AiStateRuntime,
    ) -> StateReturnType {
        self.update()
    }

    /// Like [`update_with_ai`], but `machine_locked` is `StateMachine::is_locked`
    /// on the machine this step already holds. Do not `lock()` it again.
    fn update_with_ai_held(
        &mut self,
        ai: &mut dyn crate::modules::ai_state_runtime::AiStateRuntime,
        _machine_locked: bool,
    ) -> StateReturnType {
        self.update_with_ai(ai)
    }

    /// Combined borrow for an AI-owned child machine with additional local state.
    /// Defaults preserve existing AI-only dispatch, including waypoint handling.
    fn update_with_ai_and_owner(
        &mut self,
        ai: &mut dyn crate::modules::ai_state_runtime::AiStateRuntime,
        machine_locked: bool,
        _owner: &mut dyn Any,
    ) -> StateReturnType {
        self.update_with_ai_held(ai, machine_locked)
    }

    fn on_enter_with_ai_and_owner(
        &mut self,
        ai: &mut dyn crate::modules::ai_state_runtime::AiStateRuntime,
        goal_id: crate::common::ObjectID,
        goal_pos: Coord3D,
        waypoint: Option<WaypointId>,
        _owner: &mut dyn Any,
    ) -> StateReturnType {
        self.on_enter_with_waypoint(ai, goal_id, goal_pos, waypoint)
    }

    fn on_exit_with_owner(&mut self, status: StateExitType, _owner: &mut dyn Any) {
        self.on_exit(status);
    }

    fn on_exit_with_ai_and_owner(
        &mut self,
        status: StateExitType,
        ai: &mut dyn crate::modules::ai_state_runtime::AiStateRuntime,
        _owner: &mut dyn Any,
    ) {
        self.on_exit_with_ai(status, ai);
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
    fn bind_goal_squad(&mut self, _squad: Option<Arc<Squad>>) {}

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
        Ok(None)
    }

    /// Get the owner object for this state machine (default implementation returns error)
    fn get_machine_owner(&self) -> Result<Arc<RwLock<crate::object::Object>>, String> {
        if let Some(base_state) = self.as_any().downcast_ref::<State>() {
            return base_state
                .get_machine_owner()
                .ok_or_else(|| "state machine owner not attached".to_string());
        }

        let machine = self.get_machine()?;
        let guard = machine
            .lock()
            .map_err(|_| "failed to lock state machine".to_string())?;
        guard
            .get_owner()
            .ok_or_else(|| "state machine owner not attached".to_string())
    }

    fn get_machine_owner_id(&self) -> Result<crate::common::ObjectID, String> {
        if let Some(base_state) = self.as_any().downcast_ref::<State>() {
            return base_state
                .get_machine_owner_id()
                .ok_or_else(|| "state machine owner not attached".to_string());
        }
        let machine = self.get_machine()?;
        let guard = machine
            .lock()
            .map_err(|_| "failed to lock state machine".to_string())?;
        let id = guard.get_owner_id();
        if id == crate::common::INVALID_ID {
            Err("state machine owner not attached".to_string())
        } else {
            Ok(id)
        }
    }

    /// Get the state machine (default implementation returns error)
    fn get_machine(&self) -> Result<Arc<Mutex<StateMachine>>, String> {
        if let Some(base_state) = self.as_any().downcast_ref::<State>() {
            return base_state.get_machine();
        }

        Err(format!(
            "state '{}' does not expose machine reference",
            self.get_name()
        ))
    }

    /// Serialize state-specific snapshot data (default no-op).
    fn xfer_snapshot(&mut self, _xfer: &mut dyn crate::common::xfer::Xfer) -> Result<(), String> {
        Ok(())
    }

    /// Evaluate an opaque transition payload against this concrete state.
    ///
    /// Legacy adapters use this to run strongly typed transition predicates
    /// without requiring callers to downcast trait objects at each transition.
    fn evaluate_transition_payload(&self, _payload: &(dyn Any + Send + Sync)) -> Option<bool> {
        None
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
    pub machine: Option<Weak<Mutex<StateMachine>>>,
    /// Exact non-owning Object identity. An expired bound owner is never
    /// replaced by an unrelated object with the same numeric ID.
    owner: Option<Weak<RwLock<Object>>>,
    /// Copied from [`StateMachine::owner_id`] at construction. No second mutex.
    pub owner_id: crate::common::ObjectID,
    /// Victim id copied from the machine that owns this state. `machine` is often `None`.
    pub goal_object_id: crate::common::ObjectID,
    /// Goal point copied from the machine when `machine` is `None`.
    pub goal_position_copied: Option<Coord3D>,
    /// Squad copied before an update that already holds the machine lock.
    pub goal_squad_copied: Option<Arc<Squad>>,
    /// Area copied before an update that already holds the machine lock.
    pub goal_polygon_copied: Option<Arc<PolygonTrigger>>,
    /// Waypoint copied before an update that already holds the machine lock.
    pub goal_waypoint_copied: Option<WaypointId>,
}

impl State {
    /// Create a state without wiring it to a concrete machine. This mirrors the
    /// legacy usage where most states lived inside stack-owned state machines.
    pub fn new(machine: &StateMachine, name: &str) -> Self {
        let mut state = Self::with_machine(None, name);
        state.owner_id = machine.get_owner_id();
        state.owner = machine.owner_reference();
        state
    }

    /// Create a state that tracks the owning state machine through a `Weak`.
    pub fn with_machine(machine: Option<Weak<Mutex<StateMachine>>>, name: &str) -> Self {
        Self {
            id: INVALID_STATE_ID,
            name: name.to_string(),
            success_state_id: INVALID_STATE_ID,
            failure_state_id: INVALID_STATE_ID,
            transitions: Vec::new(),
            machine,
            owner: None,
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

    /// Bind the owner already known by the driving machine. This does not
    /// lock or publish the Object, and does not change serialized state.
    pub(crate) fn bind_owner(&mut self, owner: &Arc<RwLock<Object>>) {
        self.owner = Some(Arc::downgrade(owner));
    }

    /// Get the machine owner object
    pub fn get_machine_owner(&self) -> Option<Arc<RwLock<Object>>> {
        if let Some(owner) = &self.owner {
            return owner.upgrade();
        }
        if let Some(owner) = self
            .machine
            .as_ref()
            .and_then(|weak| weak.upgrade())
            .and_then(|arc| arc.try_lock().ok().and_then(|guard| guard.get_owner()))
        {
            return Some(owner);
        }
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

    pub fn get_machine_goal_object_id(&self) -> Option<crate::common::ObjectID> {
        if let Some(machine) = self.machine.as_ref().and_then(|weak| weak.upgrade()) {
            if let Ok(guard) = machine.try_lock() {
                let id = guard.get_goal_object_id();
                if id != crate::common::INVALID_ID {
                    return Some(id);
                }
            }
        }
        if self.goal_object_id == crate::common::INVALID_ID {
            None
        } else {
            Some(self.goal_object_id)
        }
    }

    pub fn get_machine_owner_id(&self) -> Option<crate::common::ObjectID> {
        if self.owner.is_some() {
            return (self.owner_id != crate::common::INVALID_ID).then_some(self.owner_id);
        }
        if let Some(machine) = self.machine.as_ref().and_then(|weak| weak.upgrade()) {
            if let Ok(guard) = machine.try_lock() {
                let id = guard.get_owner_id();
                if id != crate::common::INVALID_ID {
                    return Some(id);
                }
            }
        }
        if self.owner_id == crate::common::INVALID_ID {
            None
        } else {
            Some(self.owner_id)
        }
    }

    /// Get the machine goal squad
    pub fn get_machine_goal_squad(&self) -> Option<Arc<Squad>> {
        if let Some(squad) = self
            .machine
            .as_ref()
            .and_then(|weak| weak.upgrade())
            .and_then(|arc| arc.try_lock().ok().and_then(|guard| guard.get_goal_squad()))
        {
            return Some(squad);
        }
        self.goal_squad_copied.clone()
    }

    /// Get the machine goal polygon trigger
    pub fn get_machine_goal_polygon(&self) -> Option<Arc<PolygonTrigger>> {
        if let Some(polygon) = self
            .machine
            .as_ref()
            .and_then(|weak| weak.upgrade())
            .and_then(|arc| {
                arc.try_lock()
                    .ok()
                    .and_then(|guard| guard.get_goal_polygon())
            })
        {
            return Some(polygon);
        }
        self.goal_polygon_copied.clone()
    }

    /// Get the machine goal position
    pub fn get_machine_goal_position(&self) -> Option<Coord3D> {
        if let Some(pos) = self
            .machine
            .as_ref()
            .and_then(|weak| weak.upgrade())
            .and_then(|arc| arc.try_lock().ok().map(|guard| guard.get_goal_position()))
        {
            return Some(pos);
        }
        self.goal_position_copied
    }

    /// Get the state machine reference.
    pub fn get_machine(&self) -> Result<Arc<Mutex<StateMachine>>, String> {
        self.machine
            .as_ref()
            .and_then(|weak| weak.upgrade())
            .ok_or_else(|| "State machine reference not available".to_string())
    }

    /// Set machine goal object through the attached state machine.
    pub fn set_goal_object(&self, obj: Option<Weak<RwLock<Object>>>) {
        if let Some(machine) = self.machine.as_ref().and_then(|weak| weak.upgrade()) {
            if let Ok(mut guard) = machine.lock() {
                guard.set_goal_object(obj);
            }
        }
    }

    /// ID-first goal object through attached state machine.
    pub fn set_goal_object_by_id(&self, object_id: Option<crate::common::ObjectID>) {
        if let Some(machine) = self.machine.as_ref().and_then(|weak| weak.upgrade()) {
            if let Ok(mut guard) = machine.lock() {
                guard.set_goal_object_by_id(object_id);
            }
        }
    }

    /// Set machine goal position through the attached state machine.
    pub fn set_goal_position(&self, pos: Coord3D) {
        if let Some(machine) = self.machine.as_ref().and_then(|weak| weak.upgrade()) {
            if let Ok(mut guard) = machine.lock() {
                guard.set_goal_position(pos);
            }
        }
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
        if self.machine.is_some() {
            // Could notify state machine of exit if needed
        }
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

pub(crate) mod cpp_state;

#[path = "state_machine_control.rs"]
mod control;
pub use control::StateMachineControl;

/// A finite state machine
#[derive(Debug)]
pub struct StateMachine {
    state_map: HashMap<StateId, Box<dyn StateImplementation>>,
    state_meta: HashMap<StateId, StateMeta>,
    control: StateMachineControl,
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
        let mut machine = Self::new_with_owner_id(owner_id, name);
        machine.control.owner = owner;
        machine
    }

    pub fn new_with_owner_id(owner_id: crate::common::ObjectID, name: &str) -> Self {
        Self {
            state_map: HashMap::new(),
            state_meta: HashMap::new(),
            control: StateMachineControl::new(owner_id, name),
        }
    }

    /// An inert machine before its states are defined. Empty machines return
    /// `Failure` from update. This is not a replacement for a live parent.
    pub fn empty() -> Self {
        Self::new_with_owner_id(crate::common::INVALID_ID, "empty-machine")
    }

    /// True until [`define_state`](Self::define_state) first runs.
    pub fn is_empty(&self) -> bool {
        self.state_map.is_empty()
    }

    fn with_transition_depth_guard<F>(&mut self, f: F) -> StateReturnType
    where
        F: FnOnce(&mut Self) -> StateReturnType,
    {
        self.control.transition_depth = self.control.transition_depth.saturating_add(1);
        if self.control.transition_depth >= MAX_TRANSITION_RECURSION_DEPTH {
            self.control.transition_depth = self.control.transition_depth.saturating_sub(1);
            return StateReturnType::Failure;
        }

        let result = f(self);
        self.control.transition_depth = self.control.transition_depth.saturating_sub(1);
        result
    }

    fn with_sleep_transition_depth_guard<F>(&mut self, f: F) -> StateReturnType
    where
        F: FnOnce(&mut Self) -> StateReturnType,
    {
        self.control.sleep_transition_depth = self.control.sleep_transition_depth.saturating_add(1);
        if self.control.sleep_transition_depth >= MAX_TRANSITION_RECURSION_DEPTH {
            self.control.sleep_transition_depth =
                self.control.sleep_transition_depth.saturating_sub(1);
            return StateReturnType::Failure;
        }

        let result = f(self);
        self.control.sleep_transition_depth = self.control.sleep_transition_depth.saturating_sub(1);
        result
    }

    /// Run one step of the machine
    pub fn update(&mut self) -> StateReturnType {
        self.update_with_owner(&mut ())
    }

    /// Same step, with the owner AI loaned to the current state (see
    /// [`StateImplementation::update_with_owner`]).
    pub fn update_with_owner(&mut self, owner: &mut dyn std::any::Any) -> StateReturnType {
        let now = self.get_current_frame();
        if self.control.sleep_till != 0 && now < self.control.sleep_till {
            if self.control.current_state_id.is_none() {
                return StateReturnType::Failure;
            }

            return self.check_for_sleep_transitions(
                StateReturnType::Sleep(self.control.sleep_till.wrapping_sub(now)),
                owner,
            );
        }

        // Not sleeping anymore.
        self.control.sleep_till = 0;

        if let Some(state_id) = self.control.current_state_id {
            let state_before_update = state_id;
            let goal_object_id = self.get_goal_object_id();
            let goal_position = self.get_goal_position();
            let goal_squad = self.get_goal_squad();
            let goal_polygon = self.get_goal_polygon();
            let goal_waypoint = self.get_goal_waypoint();
            let step_owner = self.get_owner();
            let mut status = {
                let Some(state) = self.state_map.get_mut(&state_id) else {
                    return StateReturnType::Failure;
                };
                if let Some(step_owner) = step_owner {
                    state.note_step_owner(step_owner);
                }
                state.bind_goal_object_id(goal_object_id);
                state.bind_goal_position(goal_position);
                state.bind_goal_squad(goal_squad);
                state.bind_goal_polygon(goal_polygon);
                state.bind_goal_waypoint(goal_waypoint);
                let locked = self.control.locked;
                state.update_with_control(&mut self.control, None, locked, owner)
            };
            if let Some(next) = self
                .state_map
                .get_mut(&state_before_update)
                .and_then(|state| state.take_requested_state_change())
            {
                let _ = self.set_current_state_with_owner(next, owner);
            }
            self.apply_pending_victim_goal();
            if self.control.current_state_id.is_none() {
                return StateReturnType::Failure;
            }

            // If update changed state, ignore any sleep and treat it as continue.
            if self.control.current_state_id != Some(state_before_update) {
                status = StateReturnType::Continue;
            }

            if let StateReturnType::Sleep(frames) = status {
                self.control.sleep_till = now.wrapping_add(frames);
                return self.check_for_sleep_transitions(
                    StateReturnType::Sleep(self.control.sleep_till.wrapping_sub(now)),
                    owner,
                );
            }

            return self.check_for_transitions(status, owner);
        }

        StateReturnType::Failure
    }

    /// Same as [`update`], but the current state receives the AI this update already borrowed.
    pub fn update_with_ai(
        &mut self,
        ai: &mut dyn crate::modules::AIUpdateInterface,
    ) -> StateReturnType {
        let mut runtime = crate::modules::ai_state_runtime::AiUpdateRuntimeAdapter(ai);
        self.update_with_ai_and_owner(&mut runtime, &mut ())
    }

    pub(crate) fn update_with_ai_and_owner(
        &mut self,
        ai: &mut dyn crate::modules::ai_state_runtime::AiStateRuntime,
        owner: &mut dyn Any,
    ) -> StateReturnType {
        self.begin_update_with_ai_and_owner(ai, owner).finish()
    }

    /// Run only the body when awake. The driver can then perform synchronous
    /// terminal operations without borrowing an entry in the state table.
    pub(crate) fn begin_update_with_ai_and_owner<'step, A: StateMachineAI + ?Sized>(
        &'step mut self,
        ai: &'step mut A,
        owner: &'step mut dyn Any,
    ) -> StateUpdate<'step, A> {
        let now = self.get_current_frame();
        if self.control.sleep_till != 0 && now < self.control.sleep_till {
            if self.control.current_state_id.is_none() {
                return StateUpdate::Complete(StateReturnType::Failure);
            }

            return StateUpdate::Complete(self.check_for_sleep_transitions_ai(
                StateReturnType::Sleep(self.control.sleep_till.wrapping_sub(now)),
                Some(ai.as_state_runtime()),
                owner,
            ));
        }

        self.control.sleep_till = 0;

        if let Some(state_id) = self.control.current_state_id {
            let state_before_update = state_id;
            let machine_locked = self.control.locked;
            let goal_object_id = self.get_goal_object_id();
            let goal_position = self.get_goal_position();
            let goal_squad = self.get_goal_squad();
            let goal_polygon = self.get_goal_polygon();
            let goal_waypoint = self.get_goal_waypoint();
            let step_owner = self.get_owner();
            let freeze_parent = self
                .state_map
                .get(&state_id)
                .is_some_and(|state| state.freezes_parent_during_update());
            if freeze_parent {
                self.control.locked = true;
            }
            let status = {
                let Some(state) = self.state_map.get_mut(&state_id) else {
                    if freeze_parent {
                        self.control.locked = machine_locked;
                    }
                    return StateUpdate::Complete(StateReturnType::Failure);
                };
                if let Some(owner) = step_owner {
                    state.note_step_owner(owner);
                }
                state.bind_goal_object_id(goal_object_id);
                state.bind_goal_position(goal_position);
                state.bind_goal_squad(goal_squad);
                state.bind_goal_polygon(goal_polygon);
                state.bind_goal_waypoint(goal_waypoint);
                state.update_with_control(
                    &mut self.control,
                    Some(ai.as_state_runtime()),
                    machine_locked,
                    owner,
                )
            };
            if let Some(state_id) = self.control.current_state_id {
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
                self.control.locked = machine_locked;
            }
            return StateUpdate::Body(StateStep {
                machine: self,
                ai,
                owner,
                now,
                state_before_update,
                status,
            });
        }

        StateUpdate::Complete(StateReturnType::Failure)
    }

    /// Complete the same step after the driver has released any command loan.
    /// C++ StateMachine.cpp:413-435 checks changed state before using Sleep.
    fn finish_state_body(
        &mut self,
        now: UnsignedInt,
        state_before_update: StateId,
        mut status: StateReturnType,
        ai: &mut dyn crate::modules::ai_state_runtime::AiStateRuntime,
        owner: &mut dyn Any,
    ) -> StateReturnType {
        if let Some(next) = self
            .state_map
            .get_mut(&state_before_update)
            .and_then(|state| state.take_requested_state_change())
        {
            let _ = self.set_current_state_with_ai_and_owner(next, ai, owner);
        }
        self.apply_pending_victim_goal();
        if self.control.current_state_id.is_none() {
            return StateReturnType::Failure;
        }
        if self.control.current_state_id != Some(state_before_update) {
            status = StateReturnType::Continue;
        }
        if let StateReturnType::Sleep(frames) = status {
            self.control.sleep_till = now.wrapping_add(frames);
            return self.check_for_sleep_transitions_ai(
                StateReturnType::Sleep(self.control.sleep_till.wrapping_sub(now)),
                Some(ai),
                owner,
            );
        }
        self.check_for_transitions_ai(status, Some(ai), owner)
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
        self.control.goal_object_id = id;
        if let Some(arc) = crate::helpers::TheGameLogic::find_object_by_id(id)
            .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(id))
        {
            if let Ok(guard) = arc.read() {
                self.control
                    .internal_set_goal_position(*guard.get_position());
            }
        }
    }

    /// Clear the machine's internals to a known, initialized state
    pub fn clear(&mut self) {
        self.clear_impl(None);
    }

    pub(crate) fn clear_with_ai(
        &mut self,
        ai: &mut dyn crate::modules::ai_state_runtime::AiStateRuntime,
    ) {
        self.clear_impl(Some(ai));
    }

    fn clear_impl(
        &mut self,
        ai: Option<&mut dyn crate::modules::ai_state_runtime::AiStateRuntime>,
    ) {
        if self.control.locked {
            return;
        }

        if let Some(current_id) = self.control.current_state_id {
            if let Some(current_state) = self.state_map.get_mut(&current_id) {
                current_state.on_exit_with_control(
                    &mut self.control,
                    StateExitType::Reset,
                    ai,
                    &mut (),
                );
            }
        }

        self.control.current_state_id = None;
        self.control.sleep_till = 0;
        self.control.internal_clear();
    }

    /// Reset to default state
    pub fn reset_to_default_state(&mut self) -> StateReturnType {
        if self.control.locked {
            return StateReturnType::Failure;
        }

        if !self.control.default_state_inited {
            return StateReturnType::Failure;
        }

        if let Some(current_id) = self.control.current_state_id {
            if let Some(current_state) = self.state_map.get_mut(&current_id) {
                current_state.on_exit(StateExitType::Reset);
            }
        }
        self.control.current_state_id = None;
        self.control.sleep_till = 0;
        self.control.internal_clear();

        self.internal_set_state(self.control.default_state_id)
    }

    /// Initialize default state
    pub fn init_default_state(&mut self) -> StateReturnType {
        if self.control.default_state_inited {
            return StateReturnType::Failure;
        }

        if self.control.default_state_id == INVALID_STATE_ID {
            return StateReturnType::Failure;
        }

        self.control.default_state_inited = true;
        self.internal_set_state(self.control.default_state_id)
    }

    /// [`init_default_state`](Self::init_default_state) with the owner AI loaned
    /// to the entering default state (see
    /// [`StateImplementation::on_enter_with_owner`]).
    pub fn init_default_state_with_owner(
        &mut self,
        owner: &mut dyn std::any::Any,
    ) -> StateReturnType {
        if self.control.default_state_inited {
            return StateReturnType::Failure;
        }

        if self.control.default_state_id == INVALID_STATE_ID {
            return StateReturnType::Failure;
        }

        self.control.default_state_inited = true;
        self.set_state_entering_with_owner(self.control.default_state_id, owner)
    }

    pub(crate) fn init_default_state_with_ai_and_owner(
        &mut self,
        ai: &mut dyn crate::modules::ai_state_runtime::AiStateRuntime,
        owner: &mut dyn Any,
    ) -> StateReturnType {
        if self.control.default_state_inited || self.control.default_state_id == INVALID_STATE_ID {
            return StateReturnType::Failure;
        }
        self.control.default_state_inited = true;
        self.set_state_entering_impl(self.control.default_state_id, Some(ai), owner)
    }

    /// Change the current state of the machine
    pub fn set_current_state(&mut self, new_state_id: StateId) -> StateReturnType {
        if self.control.locked {
            return StateReturnType::Continue;
        }

        self.internal_set_state(new_state_id)
    }

    /// [`set_current_state`](Self::set_current_state) with the owner AI loaned to
    /// the entering state (see [`StateImplementation::on_enter_with_owner`]).
    pub fn set_current_state_with_owner(
        &mut self,
        new_state_id: StateId,
        owner: &mut dyn std::any::Any,
    ) -> StateReturnType {
        if self.control.locked {
            return StateReturnType::Continue;
        }

        self.set_state_entering_with_owner(new_state_id, owner)
    }

    /// Change state using the driving AI and the same owner loan as update.
    pub(crate) fn set_current_state_with_ai_and_owner(
        &mut self,
        new_state_id: StateId,
        ai: &mut dyn crate::modules::ai_state_runtime::AiStateRuntime,
        owner: &mut dyn Any,
    ) -> StateReturnType {
        if self.control.locked {
            return StateReturnType::Continue;
        }
        self.set_state_entering_impl(new_state_id, Some(ai), owner)
    }

    /// Internal state transition used by state-driven transitions even when locked.
    pub fn internal_set_state(&mut self, new_state_id: StateId) -> StateReturnType {
        self.set_state_entering(new_state_id, None)
    }

    /// `ai` is the update borrow. `None` keeps [`on_enter`].
    pub fn set_state_entering(
        &mut self,
        new_state_id: StateId,
        ai: Option<&mut dyn crate::modules::ai_state_runtime::AiStateRuntime>,
    ) -> StateReturnType {
        self.set_state_entering_impl(new_state_id, ai, &mut ())
    }

    /// Enter `new_state_id` with the owner AI loaned to the entering state (see
    /// [`StateImplementation::on_enter_with_owner`]).
    pub fn set_state_entering_with_owner(
        &mut self,
        new_state_id: StateId,
        owner: &mut dyn std::any::Any,
    ) -> StateReturnType {
        self.set_state_entering_impl(new_state_id, None, owner)
    }

    fn set_state_entering_impl(
        &mut self,
        mut new_state_id: StateId,
        mut ai: Option<&mut dyn crate::modules::ai_state_runtime::AiStateRuntime>,
        owner: &mut dyn std::any::Any,
    ) -> StateReturnType {
        self.control.sleep_till = 0;

        if new_state_id != MACHINE_DONE_STATE_ID {
            if new_state_id == INVALID_STATE_ID {
                new_state_id = self.control.default_state_id;
                if new_state_id == INVALID_STATE_ID {
                    return StateReturnType::Failure;
                }
            }

            if !self.state_map.contains_key(&new_state_id) {
                if self.state_map.contains_key(&self.control.default_state_id) {
                    new_state_id = self.control.default_state_id;
                } else {
                    return StateReturnType::Failure;
                }
            }
        }

        if let Some(current_id) = self.control.current_state_id {
            let outgoing_locks = self
                .state_map
                .get(&current_id)
                .is_some_and(|state| state.unlocks_machine_on_exit());
            let restore = self
                .state_map
                .get(&current_id)
                .and_then(|state| state.exit_restore_goal());
            if let Some(current_state) = self.state_map.get_mut(&current_id) {
                if let Some(ref mut ai_ref) = ai {
                    current_state.on_exit_with_control(
                        &mut self.control,
                        StateExitType::Normal,
                        Some(&mut **ai_ref),
                        owner,
                    );
                } else {
                    current_state.on_exit_with_control(
                        &mut self.control,
                        StateExitType::Normal,
                        None,
                        owner,
                    );
                }
            }
            if outgoing_locks {
                self.unlock();
            }
            if let Some(current_state) = self.state_map.get_mut(&current_id) {
                match ai.as_mut() {
                    Some(ai) => current_state.on_exit_after_unlock(
                        StateExitType::Normal,
                        Some(&mut **ai),
                        owner,
                    ),
                    None => current_state.on_exit_after_unlock(StateExitType::Normal, None, owner),
                }
            }
            if let Some(origin) = restore {
                self.unlock();
                self.control.internal_set_goal_position(origin);
            }
        }

        self.control.current_state_id = if new_state_id == MACHINE_DONE_STATE_ID {
            None
        } else {
            Some(new_state_id)
        };

        if let Some(current_id) = self.control.current_state_id {
            let state_before_enter = current_id;
            let goal_id = self.get_goal_object_id();
            let goal_pos = self.get_goal_position();
            let goal_squad = self.get_goal_squad();
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
                let ai_ref = ai.as_mut().map(|ai_ref| {
                    &mut **ai_ref as &mut dyn crate::modules::ai_state_runtime::AiStateRuntime
                });
                new_state.on_enter_with_control(
                    &mut self.control,
                    ai_ref,
                    goal_id,
                    goal_pos,
                    waypoint,
                    owner,
                )
            };
            if let Some(id) = self.control.current_state_id {
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
                && (self.control.current_state_id != Some(state_before_enter)
                    || matches!(status, StateReturnType::Failure))
            {
                self.unlock();
            }

            if self.control.current_state_id.is_none() {
                return StateReturnType::Failure;
            }

            // If on_enter changed state, ignore any sleep and run the new state immediately.
            if self.control.current_state_id != Some(state_before_enter) {
                status = StateReturnType::Continue;
            }

            if let StateReturnType::Sleep(frames) = status {
                let now = self.get_current_frame();
                self.control.sleep_till = now.wrapping_add(frames);
                return self.check_for_sleep_transitions_ai(
                    StateReturnType::Sleep(self.control.sleep_till.wrapping_sub(now)),
                    ai,
                    owner,
                );
            }

            return self.check_for_transitions_ai(status, ai, owner);
        }

        StateReturnType::Continue
    }

    fn check_for_transitions(
        &mut self,
        status: StateReturnType,
        owner: &mut dyn std::any::Any,
    ) -> StateReturnType {
        self.check_for_transitions_ai(status, None, owner)
    }

    fn check_for_transitions_ai(
        &mut self,
        status: StateReturnType,
        mut ai: Option<&mut dyn crate::modules::ai_state_runtime::AiStateRuntime>,
        owner: &mut dyn std::any::Any,
    ) -> StateReturnType {
        if status.is_sleep() {
            return StateReturnType::Failure;
        }
        self.control.transition_depth = self.control.transition_depth.saturating_add(1);
        if self.control.transition_depth >= MAX_TRANSITION_RECURSION_DEPTH {
            self.control.transition_depth = self.control.transition_depth.saturating_sub(1);
            return StateReturnType::Failure;
        }
        let result = self.check_for_transitions_inner(status, ai, owner);
        self.control.transition_depth = self.control.transition_depth.saturating_sub(1);
        result
    }

    fn check_for_transitions_inner(
        &mut self,
        status: StateReturnType,
        mut ai: Option<&mut dyn crate::modules::ai_state_runtime::AiStateRuntime>,
        owner: &mut dyn std::any::Any,
    ) -> StateReturnType {
        let Some(state_id) = self.control.current_state_id else {
            return StateReturnType::Failure;
        };
        let Some(meta) = self.state_meta.get(&state_id).cloned() else {
            return status;
        };

        match status {
            StateReturnType::Continue => self.check_condition_transitions_ai(&meta, ai, owner),
            _ if status.is_success() => match meta.success_state_id {
                EXIT_MACHINE_WITH_SUCCESS => {
                    let _ = self.set_state_entering_impl(MACHINE_DONE_STATE_ID, ai, owner);
                    StateReturnType::Success
                }
                EXIT_MACHINE_WITH_FAILURE => {
                    let _ = self.set_state_entering_impl(MACHINE_DONE_STATE_ID, ai, owner);
                    StateReturnType::Failure
                }
                INVALID_STATE_ID => status,
                next => self.set_state_entering_impl(next, ai, owner),
            },
            _ if status.is_failure() => match meta.failure_state_id {
                EXIT_MACHINE_WITH_SUCCESS => {
                    let _ = self.set_state_entering_impl(MACHINE_DONE_STATE_ID, ai, owner);
                    StateReturnType::Success
                }
                EXIT_MACHINE_WITH_FAILURE => {
                    let _ = self.set_state_entering_impl(MACHINE_DONE_STATE_ID, ai, owner);
                    StateReturnType::Failure
                }
                INVALID_STATE_ID => status,
                next => self.set_state_entering_impl(next, ai, owner),
            },
            other => other,
        }
    }

    fn check_for_sleep_transitions(
        &mut self,
        status: StateReturnType,
        owner: &mut dyn std::any::Any,
    ) -> StateReturnType {
        self.check_for_sleep_transitions_ai(status, None, owner)
    }

    fn check_for_sleep_transitions_ai(
        &mut self,
        status: StateReturnType,
        ai: Option<&mut dyn crate::modules::ai_state_runtime::AiStateRuntime>,
        owner: &mut dyn std::any::Any,
    ) -> StateReturnType {
        if !matches!(status, StateReturnType::Sleep(_)) {
            return status;
        }
        self.control.sleep_transition_depth = self.control.sleep_transition_depth.saturating_add(1);
        if self.control.sleep_transition_depth >= MAX_TRANSITION_RECURSION_DEPTH {
            self.control.sleep_transition_depth =
                self.control.sleep_transition_depth.saturating_sub(1);
            return StateReturnType::Failure;
        }
        let result = self.check_for_sleep_transitions_inner(status, ai, owner);
        self.control.sleep_transition_depth = self.control.sleep_transition_depth.saturating_sub(1);
        result
    }

    fn check_for_sleep_transitions_inner(
        &mut self,
        status: StateReturnType,
        ai: Option<&mut dyn crate::modules::ai_state_runtime::AiStateRuntime>,
        owner: &mut dyn std::any::Any,
    ) -> StateReturnType {
        let Some(state_id) = self.control.current_state_id else {
            return StateReturnType::Failure;
        };
        let Some(meta) = self.state_meta.get(&state_id).cloned() else {
            return status;
        };
        self.check_condition_transitions_or_sleep(&meta, status, ai, owner)
    }

    fn check_condition_transitions(
        &mut self,
        meta: &StateMeta,
        owner: &mut dyn std::any::Any,
    ) -> StateReturnType {
        self.check_condition_transitions_ai(meta, None, owner)
    }

    fn check_condition_transitions_ai(
        &mut self,
        meta: &StateMeta,
        ai: Option<&mut dyn crate::modules::ai_state_runtime::AiStateRuntime>,
        owner: &mut dyn std::any::Any,
    ) -> StateReturnType {
        let Some(state_id) = self.control.current_state_id else {
            return StateReturnType::Failure;
        };
        let Some(state) = self.state_map.get(&state_id) else {
            return StateReturnType::Failure;
        };
        for transition in &meta.transitions {
            if transition.evaluate(state.as_ref(), owner) {
                return match transition.to_state_id {
                    EXIT_MACHINE_WITH_SUCCESS => StateReturnType::Success,
                    EXIT_MACHINE_WITH_FAILURE => StateReturnType::Failure,
                    next => self.set_state_entering_impl(next, ai, owner),
                };
            }
        }
        StateReturnType::Continue
    }

    fn check_condition_transitions_or_sleep(
        &mut self,
        meta: &StateMeta,
        status: StateReturnType,
        ai: Option<&mut dyn crate::modules::ai_state_runtime::AiStateRuntime>,
        owner: &mut dyn std::any::Any,
    ) -> StateReturnType {
        let Some(state_id) = self.control.current_state_id else {
            return StateReturnType::Failure;
        };
        let Some(state) = self.state_map.get(&state_id) else {
            return StateReturnType::Failure;
        };

        for transition in &meta.transitions {
            if transition.evaluate(state.as_ref(), owner) {
                return match transition.to_state_id {
                    EXIT_MACHINE_WITH_SUCCESS => StateReturnType::Success,
                    EXIT_MACHINE_WITH_FAILURE => StateReturnType::Failure,
                    next => self.set_state_entering_impl(next, ai, owner),
                };
            }
        }

        status
    }

    /// Get current state ID
    pub fn get_current_state_id(&self) -> Option<StateId> {
        self.control.current_state_id
    }

    /// Check if in idle state
    pub fn is_in_idle_state(&self) -> bool {
        if let Some(state_id) = self.control.current_state_id {
            if let Some(state) = self.state_map.get(&state_id) {
                return state.is_idle();
            }
        }
        true // stateless things are considered idle
    }

    /// Check if in attack state
    pub fn is_in_attack_state(&self) -> bool {
        if let Some(state_id) = self.control.current_state_id {
            if let Some(state) = self.state_map.get(&state_id) {
                return state.is_attack();
            }
        }
        true
    }

    /// Check if in guard idle state  
    pub fn is_in_guard_idle_state(&self) -> bool {
        if let Some(state_id) = self.control.current_state_id {
            if let Some(state) = self.state_map.get(&state_id) {
                return state.is_guard_idle();
            }
        }
        false
    }

    /// Check if in busy state
    pub fn is_in_busy_state(&self) -> bool {
        if let Some(state_id) = self.control.current_state_id {
            if let Some(state) = self.state_map.get(&state_id) {
                return state.is_busy();
            }
        }
        false
    }

    /// Lock/unlock this state machine
    pub fn lock(&mut self) {
        self.control.lock()
    }

    pub fn unlock(&mut self) {
        self.control.unlock()
    }

    pub fn is_locked(&self) -> bool {
        self.control.is_locked()
    }

    /// Get the owner object
    pub fn get_owner(&self) -> Option<Arc<RwLock<Object>>> {
        self.control.get_owner()
    }

    pub(crate) fn owner_reference(&self) -> Option<Weak<RwLock<Object>>> {
        self.control.owner_reference()
    }

    pub fn get_owner_id(&self) -> crate::common::ObjectID {
        self.control.get_owner_id()
    }

    pub fn set_owner_id(&mut self, owner_id: crate::common::ObjectID) {
        self.control.set_owner_id(owner_id)
    }

    /// Set goal object
    pub fn set_goal_object(&mut self, obj: Option<Weak<RwLock<Object>>>) {
        self.control.set_goal_object(obj)
    }

    /// ID-first goal object setter (no Arc/Weak required at call site).
    pub fn set_goal_object_by_id(&mut self, object_id: Option<crate::common::ObjectID>) {
        self.control.set_goal_object_by_id(object_id)
    }

    /// Get goal object
    pub fn get_goal_object(&self) -> Option<Arc<RwLock<Object>>> {
        self.control.get_goal_object()
    }

    pub fn get_goal_object_id(&self) -> crate::common::ObjectID {
        self.control.get_goal_object_id()
    }

    /// Set goal squad
    pub fn set_goal_squad(&mut self, squad: Option<Weak<Squad>>) {
        self.control.set_goal_squad(squad)
    }

    /// Get goal squad
    pub fn get_goal_squad(&self) -> Option<Arc<Squad>> {
        self.control.get_goal_squad()
    }

    /// Set goal polygon trigger
    pub fn set_goal_polygon(&mut self, polygon: Option<Weak<PolygonTrigger>>) {
        self.control.set_goal_polygon(polygon)
    }

    /// Set guard mode (raw int value).
    pub fn set_guard_mode_raw(&mut self, guard_mode: i32) {
        self.control.set_guard_mode_raw(guard_mode)
    }

    /// Get guard mode (raw int value).
    pub fn get_guard_mode_raw(&self) -> i32 {
        self.control.get_guard_mode_raw()
    }

    /// Get goal polygon trigger
    pub fn get_goal_polygon(&self) -> Option<Arc<PolygonTrigger>> {
        self.control.get_goal_polygon()
    }

    pub fn set_goal_waypoint(&mut self, waypoint: Option<WaypointId>) {
        self.control.set_goal_waypoint(waypoint)
    }

    pub fn get_goal_waypoint(&self) -> Option<WaypointId> {
        self.control.get_goal_waypoint()
    }

    /// Set goal position
    pub fn set_goal_position(&mut self, pos: Coord3D) {
        self.control.set_goal_position(pos)
    }

    /// Get goal position
    pub fn get_goal_position(&self) -> Coord3D {
        self.control.get_goal_position()
    }

    /// Check if goal object is destroyed
    pub fn is_goal_object_destroyed(&self) -> bool {
        if self.control.goal_object_id == crate::common::INVALID_ID {
            return false;
        }

        // Goal ID is set but the object no longer resolves.
        self.get_goal_object_id() == crate::common::INVALID_ID
    }

    /// Run the current state's C++ destructor exit while its owner and AI
    /// are still available (StateMachine.cpp:263-268). Unlike halt, destruction
    /// exits even a locked state, before the state map is released.
    pub(crate) fn finish_with_ai_and_owner(
        &mut self,
        ai: &mut dyn crate::modules::ai_state_runtime::AiStateRuntime,
        owner: &mut dyn Any,
    ) {
        if let Some(current_id) = self.control.current_state_id {
            if let Some(state) = self.state_map.get_mut(&current_id) {
                state.on_exit_with_control(
                    &mut self.control,
                    StateExitType::Reset,
                    Some(ai),
                    owner,
                );
            }
        }
        self.control.current_state_id = None;
    }

    /// Halt the state machine
    pub fn halt(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.control.locked = true;
        // Don't call on_exit when halting; this mirrors C++ halt semantics.
        self.control.current_state_id = None;
        Ok(())
    }

    /// Get current state name for debugging
    pub fn get_current_state_name(&self) -> String {
        if let Some(state_id) = self.control.current_state_id {
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
        if self.control.default_state_id == INVALID_STATE_ID {
            self.control.default_state_id = id;
        }
    }

    /// Get state by ID (internal)
    /// Drive one registered temporary body with its disjoint live control.
    pub(crate) fn update_registered_with_control(
        &mut self,
        id: StateId,
        ai: &mut dyn crate::modules::ai_state_runtime::AiStateRuntime,
        owner: &mut dyn Any,
    ) -> Option<StateReturnType> {
        let locked = self.control.is_locked();
        self.state_map
            .get_mut(&id)
            .map(|state| state.update_with_control(&mut self.control, Some(ai), locked, owner))
    }

    pub fn get_state_mut(&mut self, id: StateId) -> Option<&mut Box<dyn StateImplementation>> {
        self.state_map.get_mut(&id)
    }

    /// Reset the state machine
    pub fn reset(&mut self) {
        // Exit current state with reset type
        if let Some(current_id) = self.control.current_state_id {
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
        xfer.xfer_unsigned_int(&mut self.control.sleep_till)?;

        // Transfer default state ID - C++ line 808
        xfer.xfer_unsigned_int(&mut self.control.default_state_id)?;

        // Transfer current state ID - C++ lines 809-815
        let mut cur_state_id = self.control.current_state_id.unwrap_or(INVALID_STATE_ID);
        xfer.xfer_unsigned_int(&mut cur_state_id)?;

        // On load, restore the current state reference
        // C++ lines 811-815: We jump directly into the saved state without
        // calling onEnter/onExit since the state was already active when saved
        if xfer.get_xfer_mode() == game_engine::system::XferMode::Load {
            let preferred_state_id = if cur_state_id == INVALID_STATE_ID {
                self.control.default_state_id
            } else {
                cur_state_id
            };

            self.control.current_state_id = if self.state_map.contains_key(&preferred_state_id) {
                Some(preferred_state_id)
            } else if self.state_map.contains_key(&self.control.default_state_id) {
                Some(self.control.default_state_id)
            } else {
                // C++ internalGetState throws when neither saved nor default
                // state exists, before reading the snapshot selector.
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "StateMachine::xfer has no current/default state to restore",
                )
                .into());
            };
        }

        // Transfer state snapshot data
        // C++ lines 817-860: We only transfer the current state, not all states
        // (snapshotAllStates is false in release builds)
        let mut snapshot_all_states = false;
        game_engine::system::Xfer::xfer_bool(xfer, &mut snapshot_all_states)?;

        if snapshot_all_states {
            // C++ uses std::map order, even when states were defined in a
            // different order. Read and validate the signed count before any
            // state payload; a malformed count never controls allocation.
            let mut state_ids: Vec<_> = self.state_map.keys().copied().collect();
            state_ids.sort_unstable();
            let expected_count = i32::try_from(state_ids.len()).map_err(|_| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "StateMachine::xfer state count exceeds signed Int",
                )
            })?;
            let mut saved_count = expected_count;
            game_engine::system::Xfer::xfer_int(xfer, &mut saved_count)?;
            if saved_count != expected_count {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!(
                        "StateMachine::xfer state count mismatch: expected {expected_count}, read {saved_count}"
                    ),
                ).into());
            }
            for map_id in state_ids {
                let state = self.state_map.get_mut(&map_id).ok_or_else(|| {
                    std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        format!("StateMachine::xfer missing state {map_id}"),
                    )
                })?;
                let expected_id = state.get_id();
                let mut saved_id = expected_id;
                game_engine::system::Xfer::xfer_unsigned_int(xfer, &mut saved_id)?;
                if saved_id != expected_id {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        format!(
                            "StateMachine::xfer state ID mismatch: expected {expected_id}, read {saved_id}"
                        ),
                    ).into());
                }
                state.xfer_snapshot(xfer).map_err(|error| {
                    std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        format!("StateMachine::xfer state {expected_id} snapshot failed: {error}"),
                    )
                })?;
            }
        } else {
            // Normal mode: only transfer current state
            // C++ lines 852-860
            // StateImplementation dispatches the concrete state's payload.
            let current_id = if let Some(current_id) = self.control.current_state_id {
                current_id
            } else if self.state_map.contains_key(&self.control.default_state_id) {
                self.control.current_state_id = Some(self.control.default_state_id);
                self.control.default_state_id
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
        xfer.xfer_object_id(&mut self.control.goal_object_id)?;

        // Note: Goal object weak reference resolution happens lazily when get_goal_object() is called
        // The object registry will be used to look up the object by ID at that time
        // goal_object_id is authoritative; resolve via get_goal_object().

        // Transfer goal position - C++ line 864
        game_engine::system::Xfer::xfer_real(xfer, &mut self.control.goal_position.x)?;
        game_engine::system::Xfer::xfer_real(xfer, &mut self.control.goal_position.y)?;
        game_engine::system::Xfer::xfer_real(xfer, &mut self.control.goal_position.z)?;

        // Transfer locked status - C++ line 865
        game_engine::system::Xfer::xfer_bool(xfer, &mut self.control.locked)?;

        // Transfer default state initialized flag - C++ line 866
        game_engine::system::Xfer::xfer_bool(xfer, &mut self.control.default_state_inited)?;

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
        if let Some(id) = self.control.current_state_id {
            if let Some(state) = self.state_map.get_mut(&id) {
                state.load_post_process().map_err(|e| {
                    Box::new(std::io::Error::new(std::io::ErrorKind::Other, e))
                        as Box<dyn std::error::Error + Send + Sync>
                })?;
            }
        }
        Ok(())
    }

    /// [`load_post_process`](Self::load_post_process) with the owner AI loaned to
    /// the restored state (see
    /// [`StateImplementation::load_post_process_with_owner`]).
    pub fn load_post_process_with_owner(
        &mut self,
        owner: &mut dyn std::any::Any,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Some(id) = self.control.current_state_id {
            if let Some(state) = self.state_map.get_mut(&id) {
                state.load_post_process_with_owner(owner).map_err(|e| {
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
        let _frame = crate::system::game_logic::enter_update_frame(0);
        let mut machine = StateMachine::new(None::<Weak<RwLock<Object>>>, "no-state");
        assert_eq!(machine.update(), StateReturnType::Failure);
    }

    #[test]
    fn external_set_state_is_blocked_when_locked() {
        let _frame = crate::system::game_logic::enter_update_frame(0);
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
        let _frame = crate::system::game_logic::enter_update_frame(0);
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
        let mut machine = StateMachine::new(None::<Weak<RwLock<Object>>>, "sleep");
        machine.define_state(
            1,
            Box::new(SleepThenContinueState::new("sleepy", 5)),
            None,
            None,
            None,
        );
        assert_eq!(machine.set_current_state(1), StateReturnType::Continue);

        // Each operation owns its clock scope, ending before the next
        // operation begins instead of mutating the shared singleton clock.
        let mut update_at = |frame| {
            let _frame = crate::system::game_logic::enter_update_frame(frame);
            machine.update()
        };
        assert_eq!(update_at(100), StateReturnType::Sleep(5));
        assert_eq!(update_at(101), StateReturnType::Sleep(4));

        // Jump frames to validate absolute wake deadline semantics.
        assert_eq!(update_at(104), StateReturnType::Sleep(1));
        assert_eq!(update_at(105), StateReturnType::Continue);
    }

    #[test]
    fn clear_respects_lock_and_preserves_default_init_flag() {
        let _frame = crate::system::game_logic::enter_update_frame(0);
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
        assert!(machine.control.default_state_inited);
        assert_eq!(machine.get_current_state_id(), Some(1));

        machine.lock();
        machine.clear();

        assert!(machine.control.default_state_inited);
        assert_eq!(machine.get_current_state_id(), Some(1));
    }

    #[test]
    fn stateless_attack_state_is_true_and_goal_destroyed_never_set_is_false() {
        let _frame = crate::system::game_logic::enter_update_frame(0);
        let machine = StateMachine::new(None::<Weak<RwLock<Object>>>, "stateless");

        assert!(machine.is_in_attack_state());
        assert!(!machine.is_goal_object_destroyed());
    }

    #[test]
    fn halt_locks_and_keeps_internal_goal_data() {
        let _frame = crate::system::game_logic::enter_update_frame(0);
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
        let _frame = crate::system::game_logic::enter_update_frame(0);
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
        let _frame = crate::system::game_logic::enter_update_frame(0);
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

#[cfg(test)]
#[path = "state_machine_owner_transition_tests.rs"]
mod owner_transition_tests;

#[cfg(test)]
#[path = "state_machine_object_owner_tests.rs"]
mod object_owner_tests;

#[cfg(test)]
#[path = "state_machine_snapshot_contract_tests.rs"]
mod snapshot_contract_tests;

#[cfg(test)]
#[path = "state_machine_callback_change_tests.rs"]
mod callback_change_tests;

#[cfg(test)]
#[path = "state_machine_live_control_tests.rs"]
mod live_control_tests;

#[cfg(test)]
#[path = "state_machine_idle_lock_tests.rs"]
mod idle_lock_tests;

#[cfg(test)]
#[path = "state_machine_cpp_contract_tests.rs"]
mod cpp_contract_tests;
