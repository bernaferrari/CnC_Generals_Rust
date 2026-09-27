//! Compatibility adapters for legacy AI state implementations.
//!
//! The original C++ port exposes a `State` base class that many of the Rust
//! translations still rely on.  Newer systems expect implementors of the safe
//! [`crate::state_machine::StateImplementation`] trait.  This module provides a
//! lightweight adapter so we can bridge the two worlds without rewriting every
//! call site immediately.

use crate::object::Object;
use crate::state_machine as core;
use crate::state_machine::{
    StateConditionInfo, StateExitType, StateId, StateReturnType, StateTransitionUserData,
};
use std::any::Any;
use std::sync::{Arc, Mutex, RwLock, Weak};

/// Trait mirroring the legacy `State` base class semantics.
///
/// Implementors typically embed a translated version of the C++ `State` struct
/// and expose the familiar callback surface (`on_enter`, `update`, `on_exit`).
/// The adapter in this module turns such implementations into
/// [`core::StateImplementation`] values so the modern state-machine core can own
/// and execute them.
pub trait LegacyState: Send + Sync + Any + std::fmt::Debug {
    /// C++ `OnEnter`
    fn on_enter(&mut self) -> Result<StateReturnType, String>;
    /// C++ `OnUpdate`
    fn on_update(&mut self) -> Result<StateReturnType, String>;
    fn on_exit(&mut self, exit: StateExitType) -> Result<StateReturnType, String>;

    fn on_enter_with_ai(
        &mut self,
        _ai: &mut dyn crate::modules::AIUpdateInterface,
    ) -> Result<StateReturnType, String> {
        self.on_enter()
    }

    fn on_update_with_ai(
        &mut self,
        _ai: &mut dyn crate::modules::AIUpdateInterface,
    ) -> Result<StateReturnType, String> {
        self.on_update()
    }
    /// Debug-friendly state name.
    fn state_name(&self) -> &str;
    /// Unique state identifier assigned by the machine.
    fn state_id(&self) -> StateId;
    /// Mutating counterpart to [`state_id`].
    fn set_state_id(&mut self, id: StateId);

    /// Optional goal object pointer recorded by the state machine.
    fn machine_goal_object(&self) -> Result<Option<Arc<RwLock<Object>>>, String> {
        Ok(None)
    }
    /// Owning object for the state machine.
    fn machine_owner(&self) -> Result<Arc<RwLock<Object>>, String> {
        Err("machine owner not attached".to_string())
    }
    /// Owning state machine (used for locks / advanced coordination).
    fn machine(&self) -> Result<Arc<Mutex<core::StateMachine>>, String> {
        Err("state machine not attached".to_string())
    }

    /// Whether this state should be treated as "idle" by higher level systems.
    fn is_idle(&self) -> bool {
        false
    }

    /// Whether this state represents an active attack.
    fn is_attack(&self) -> bool {
        false
    }

    /// Whether this is a guard-idle state.
    fn is_guard_idle(&self) -> bool {
        false
    }

    /// Whether this state keeps the unit busy (used for heuristics).
    fn is_busy(&self) -> bool {
        false
    }

    /// Legacy helper for conditional transitions; equivalents from the C++
    /// implementation can call into this function pointer signature.
    fn transition_predicate(&self, _user_data: &StateTransitionUserData) -> Result<bool, String> {
        Ok(false)
    }

    /// Snapshot transfer hook for legacy state payload.
    fn xfer_snapshot(&mut self, _xfer: &mut dyn crate::common::xfer::Xfer) -> Result<(), String> {
        Ok(())
    }

    /// Callback used by the original system when a state needs access to a
    /// machine reference but only has a weak pointer.
    fn attach_machine(&mut self, _machine: Weak<Mutex<core::StateMachine>>) {}

    fn note_step_owner(&mut self, _owner: Arc<RwLock<Object>>) {}

    fn note_guard_enter(
        &mut self,
        _mode: i32,
        _polygon: Option<Arc<crate::polygon_trigger::PolygonTrigger>>,
    ) {
    }

    fn exit_restore_goal(&self) -> Option<crate::common::Coord3D> {
        None
    }

    fn freezes_parent_during_update(&self) -> bool {
        false
    }
    fn locks_machine(&self) -> bool {
        false
    }

    fn bind_goal_waypoint(&mut self, _waypoint: Option<crate::waypoint::WaypointId>) {}
    fn bind_goal_object_id(&mut self, _id: crate::common::ObjectID) {}

    fn bind_goal_position(&mut self, _pos: crate::common::Coord3D) {}

    fn bind_goal_squad(
        &mut self,
        _squad: Option<Arc<Mutex<crate::ai::squad::Squad>>>,
    ) {
    }

    fn bind_goal_polygon(
        &mut self,
        _polygon: Option<Arc<crate::polygon_trigger::PolygonTrigger>>,
    ) {
    }
}

/// Adapter turning any [`LegacyState`] implementation into a safe
/// [`core::StateImplementation`].
#[derive(Debug)]
pub struct LegacyStateAdapter<S: LegacyState> {
    inner: S,
}

struct TransitionThunk<S: LegacyState + 'static> {
    predicate: fn(&S, &StateTransitionUserData) -> Result<bool, String>,
    user_data: StateTransitionUserData,
}

impl<S: LegacyState> LegacyStateAdapter<S> {
    pub fn new(inner: S) -> Self {
        Self { inner }
    }

    fn map_result(label: &str, result: Result<StateReturnType, String>) -> StateReturnType {
        match result {
            Ok(value) => value,
            Err(err) => {
                log::warn!("legacy state {} returned error: {}", label, err);
                StateReturnType::Failure
            }
        }
    }

    pub fn into_box(self) -> Box<dyn core::StateImplementation> {
        Box::new(self)
    }
}

impl<S: LegacyState + 'static> core::StateImplementation for LegacyStateAdapter<S> {
    fn on_enter(&mut self) -> StateReturnType {
        let state_name = self.inner.state_name().to_string();
        Self::map_result(state_name.as_str(), self.inner.on_enter())
    }

    fn on_enter_with_ai(
        &mut self,
        ai: &mut dyn crate::modules::AIUpdateInterface,
        _goal_id: crate::common::ObjectID,
        _goal_pos: crate::common::Coord3D,
    ) -> StateReturnType {
        let state_name = self.inner.state_name().to_string();
        Self::map_result(state_name.as_str(), self.inner.on_enter_with_ai(ai))
    }

    fn on_exit(&mut self, exit: StateExitType) {
        let state_name = self.inner.state_name().to_string();
        let _ = Self::map_result(state_name.as_str(), self.inner.on_exit(exit));
    }

    fn update(&mut self) -> StateReturnType {
        let state_name = self.inner.state_name().to_string();
        Self::map_result(state_name.as_str(), self.inner.on_update())
    }

    fn update_with_ai(
        &mut self,
        ai: &mut dyn crate::modules::AIUpdateInterface,
    ) -> StateReturnType {
        let state_name = self.inner.state_name().to_string();
        Self::map_result(state_name.as_str(), self.inner.on_update_with_ai(ai))
    }
    fn is_idle(&self) -> bool {
        self.inner.is_idle()
    }

    fn is_attack(&self) -> bool {
        self.inner.is_attack()
    }

    fn is_guard_idle(&self) -> bool {
        self.inner.is_guard_idle()
    }

    fn is_busy(&self) -> bool {
        self.inner.is_busy()
    }

    fn get_name(&self) -> &str {
        self.inner.state_name()
    }

    fn get_id(&self) -> StateId {
        self.inner.state_id()
    }

    fn set_id(&mut self, id: StateId) {
        self.inner.set_state_id(id);
    }

    fn get_machine_goal_object(&self) -> Result<Option<Arc<RwLock<Object>>>, String> {
        self.inner.machine_goal_object()
    }

    fn get_machine_owner(&self) -> Result<Arc<RwLock<Object>>, String> {
        self.inner.machine_owner()
    }

    fn get_machine(&self) -> Result<Arc<Mutex<core::StateMachine>>, String> {
        self.inner.machine()
    }

    fn xfer_snapshot(&mut self, xfer: &mut dyn crate::common::xfer::Xfer) -> Result<(), String> {
        self.inner.xfer_snapshot(xfer)
    }

    fn bind_goal_object_id(&mut self, id: crate::common::ObjectID) {
        self.inner.bind_goal_object_id(id);
    }

    fn locks_machine(&self) -> bool {
        self.inner.locks_machine()
    }

    fn bind_goal_waypoint(&mut self, waypoint: Option<crate::waypoint::WaypointId>) {
        self.inner.bind_goal_waypoint(waypoint);
    }

    fn note_step_owner(&mut self, owner: Arc<RwLock<Object>>) {
        self.inner.note_step_owner(owner);
    }

    fn note_guard_enter(
        &mut self,
        mode: i32,
        polygon: Option<Arc<crate::polygon_trigger::PolygonTrigger>>,
    ) {
        self.inner.note_guard_enter(mode, polygon);
    }

    fn exit_restore_goal(&self) -> Option<crate::common::Coord3D> {
        self.inner.exit_restore_goal()
    }

    fn freezes_parent_during_update(&self) -> bool {
        self.inner.freezes_parent_during_update()
    }

    fn bind_goal_position(&mut self, pos: crate::common::Coord3D) {
        self.inner.bind_goal_position(pos);
    }

    fn bind_goal_squad(&mut self, squad: Option<Arc<Mutex<crate::ai::squad::Squad>>>) {
        self.inner.bind_goal_squad(squad);
    }

    fn bind_goal_polygon(
        &mut self,
        polygon: Option<Arc<crate::polygon_trigger::PolygonTrigger>>,
    ) {
        self.inner.bind_goal_polygon(polygon);
    }

    fn evaluate_transition_payload(&self, payload: &(dyn Any + Send + Sync)) -> Option<bool> {
        let thunk = payload.downcast_ref::<TransitionThunk<S>>()?;
        match (thunk.predicate)(&self.inner, &thunk.user_data) {
            Ok(result) => Some(result),
            Err(err) => {
                log::warn!("legacy transition predicate failed: {}", err);
                Some(false)
            }
        }
    }
}

/// Convenience helper so call sites can turn a legacy state into a boxed
/// implementation in one step.
pub fn adapt_legacy_state<S>(state: S) -> Box<dyn core::StateImplementation>
where
    S: LegacyState + 'static,
{
    LegacyStateAdapter::new(state).into_box()
}

/// Register a legacy state into the modern state machine.
pub fn register_legacy_state<S: LegacyState + 'static>(
    machine: &mut core::StateMachine,
    id: StateId,
    state: S,
    success_id: Option<StateId>,
    failure_id: Option<StateId>,
    conditions: &[StateConditionInfo],
) {
    machine.define_state(
        id,
        adapt_legacy_state(state),
        success_id,
        failure_id,
        Some(conditions),
    );
}

/// Build a `StateConditionInfo` using a legacy predicate.
pub fn legacy_transition<S: LegacyState + 'static>(
    predicate: fn(&S, &StateTransitionUserData) -> Result<bool, String>,
    to_state_id: StateId,
    user_data: StateTransitionUserData,
    description: &str,
) -> StateConditionInfo {
    fn invoke(state: &dyn core::StateImplementation, user_data: &StateTransitionUserData) -> bool {
        let Some(payload) = user_data.data.as_ref() else {
            return false;
        };
        state
            .evaluate_transition_payload(payload.as_ref())
            .unwrap_or(false)
    }

    let thunk = TransitionThunk::<S> {
        predicate,
        user_data,
    };

    let wrapped_user_data = StateTransitionUserData {
        data: Some(Arc::new(thunk)),
    };

    StateConditionInfo::new(invoke, to_state_id, wrapped_user_data, description)
}

/// Trait capturing states that embed the original C++ `State` helper.
///
/// Many legacy ports embed a `core::State` alongside bespoke behaviour.
/// Implementing this trait lets the adapter derive the rest of the
/// [`LegacyState`] contract automatically while keeping the original code
/// structure intact.
pub trait ClassicState: std::fmt::Debug + Send + Sync {
    /// Immutable view of the embedded base state.
    fn base_state(&self) -> &core::State;
    /// Mutable view of the embedded base state.
    fn base_state_mut(&mut self) -> &mut core::State;

    /// Original `OnEnter` callback.
    fn classic_on_enter(&mut self) -> Result<StateReturnType, String>;

    fn classic_on_enter_with_ai(
        &mut self,
        _ai: &mut dyn crate::modules::AIUpdateInterface,
    ) -> Result<StateReturnType, String> {
        self.classic_on_enter()
    }

    fn classic_on_update_with_ai(
        &mut self,
        _ai: &mut dyn crate::modules::AIUpdateInterface,
    ) -> Result<StateReturnType, String> {
        self.classic_on_update()
    }
    fn classic_on_update(&mut self) -> Result<StateReturnType, String>;
    /// Original `OnExit` callback.
    fn classic_on_exit(&mut self, exit: StateExitType) -> Result<(), String>;

    /// Override when the state should be considered idle.
    fn classic_is_idle(&self) -> bool {
        false
    }
    /// Override when the state represents an attack.
    fn classic_is_attack(&self) -> bool {
        false
    }
    /// Override when the state is a guard idle.
    fn classic_is_guard_idle(&self) -> bool {
        false
    }

    /// Optional hook for starting movement audio (C++ StartMoveSound).
    fn start_move_sound(&mut self, _owner_guard: &Object) {}
    /// Optional snapshot hook used by adapter-backed classic states.
    fn classic_xfer_snapshot(
        &mut self,
        _xfer: &mut dyn crate::common::xfer::Xfer,
    ) -> Result<(), String> {
        Ok(())
    }
    /// Override when the state keeps the unit "busy".
    fn classic_is_busy(&self) -> bool {
        false
    }

    fn classic_locks_machine(&self) -> bool {
        false
    }

    fn classic_exit_restore_goal(&self) -> Option<crate::common::Coord3D> {
        None
    }

    fn classic_freezes_parent_during_update(&self) -> bool {
        false
    }

    fn classic_note_guard_enter(
        &mut self,
        _mode: i32,
        _polygon: Option<Arc<crate::polygon_trigger::PolygonTrigger>>,
    ) {
    }
}

impl<T> LegacyState for T
where
    T: ClassicState + Any,
{
    fn on_enter(&mut self) -> Result<StateReturnType, String> {
        self.classic_on_enter()
    }

    fn on_enter_with_ai(
        &mut self,
        ai: &mut dyn crate::modules::AIUpdateInterface,
    ) -> Result<StateReturnType, String> {
        self.classic_on_enter_with_ai(ai)
    }

    fn on_update_with_ai(
        &mut self,
        ai: &mut dyn crate::modules::AIUpdateInterface,
    ) -> Result<StateReturnType, String> {
        self.classic_on_update_with_ai(ai)
    }

    fn on_update(&mut self) -> Result<StateReturnType, String> {
        self.classic_on_update()
    }

    fn on_exit(&mut self, exit: StateExitType) -> Result<StateReturnType, String> {
        self.classic_on_exit(exit)
            .map(|_| StateReturnType::Continue)
    }

    fn state_name(&self) -> &str {
        self.base_state().get_name()
    }

    fn state_id(&self) -> StateId {
        self.base_state().get_id()
    }

    fn set_state_id(&mut self, id: StateId) {
        self.base_state_mut().set_id(id);
    }

    fn machine_goal_object(&self) -> Result<Option<Arc<RwLock<Object>>>, String> {
        Ok(self.base_state().get_machine_goal_object())
    }

    fn machine_owner(&self) -> Result<Arc<RwLock<Object>>, String> {
        self.base_state()
            .get_machine_owner()
            .ok_or_else(|| "state machine owner not attached".to_string())
    }

    fn machine(&self) -> Result<Arc<Mutex<core::StateMachine>>, String> {
        self.base_state().get_machine()
    }

    fn is_idle(&self) -> bool {
        self.classic_is_idle()
    }

    fn is_attack(&self) -> bool {
        self.classic_is_attack()
    }

    fn is_guard_idle(&self) -> bool {
        self.classic_is_guard_idle()
    }

    fn is_busy(&self) -> bool {
        self.classic_is_busy()
    }

    fn xfer_snapshot(&mut self, xfer: &mut dyn crate::common::xfer::Xfer) -> Result<(), String> {
        self.classic_xfer_snapshot(xfer)
    }

    fn bind_goal_object_id(&mut self, id: crate::common::ObjectID) {
        self.base_state_mut().goal_object_id = id;
    }

    fn locks_machine(&self) -> bool {
        self.classic_locks_machine()
    }

    fn bind_goal_waypoint(&mut self, waypoint: Option<crate::waypoint::WaypointId>) {
        self.base_state_mut().goal_waypoint_copied = waypoint;
    }

    fn note_step_owner(&mut self, owner: Arc<RwLock<Object>>) {
        if let Ok(guard) = owner.read() {
            self.base_state_mut().owner_id = guard.get_id();
        }
    }

    fn note_guard_enter(
        &mut self,
        mode: i32,
        polygon: Option<Arc<crate::polygon_trigger::PolygonTrigger>>,
    ) {
        self.classic_note_guard_enter(mode, polygon);
    }

    fn exit_restore_goal(&self) -> Option<crate::common::Coord3D> {
        self.classic_exit_restore_goal()
    }

    fn freezes_parent_during_update(&self) -> bool {
        self.classic_freezes_parent_during_update()
    }

    fn bind_goal_position(&mut self, pos: crate::common::Coord3D) {
        self.base_state_mut().goal_position_copied = Some(pos);
    }

    fn bind_goal_squad(&mut self, squad: Option<Arc<Mutex<crate::ai::squad::Squad>>>) {
        self.base_state_mut().goal_squad_copied = squad;
    }

    fn bind_goal_polygon(
        &mut self,
        polygon: Option<Arc<crate::polygon_trigger::PolygonTrigger>>,
    ) {
        self.base_state_mut().goal_polygon_copied = polygon;
    }
}

/// Convenience helper for registering C++-style states.
pub fn register_classic_state<S: ClassicState + 'static>(
    machine: &mut core::StateMachine,
    id: StateId,
    state: S,
    success_id: Option<StateId>,
    failure_id: Option<StateId>,
    conditions: &[StateConditionInfo],
) {
    register_legacy_state(machine, id, state, success_id, failure_id, conditions);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct DummyState {
        base: core::State,
        entered: bool,
        exited: bool,
    }

    impl DummyState {
        fn new(id: StateId, name: &'static str) -> Self {
            let mut base = core::State::with_machine(None, name);
            base.set_id(id);
            Self {
                base,
                entered: false,
                exited: false,
            }
        }
    }

    impl ClassicState for DummyState {
        fn base_state(&self) -> &core::State {
            &self.base
        }

        fn base_state_mut(&mut self) -> &mut core::State {
            &mut self.base
        }

        fn classic_on_enter(&mut self) -> Result<StateReturnType, String> {
            self.entered = true;
            Ok(StateReturnType::Success)
        }

        fn classic_on_update(&mut self) -> Result<StateReturnType, String> {
            Ok(StateReturnType::Continue)
        }

        fn classic_on_exit(&mut self, _exit: StateExitType) -> Result<(), String> {
            self.exited = true;
            Ok(())
        }

        fn classic_is_idle(&self) -> bool {
            true
        }
    }

    #[test]
    fn legacy_adapter_registers_and_runs() {
        let mut machine = core::StateMachine::new(None::<Weak<RwLock<Object>>>, "legacy-test");

        register_classic_state(
            &mut machine,
            1,
            DummyState::new(1, "Dummy"),
            None,
            None,
            &[],
        );

        let result = machine.set_current_state(1);
        assert!(matches!(result, StateReturnType::Success));
        assert!(machine.is_in_idle_state());

        let update = machine.update();
        assert!(matches!(update, StateReturnType::Continue));

        let result = machine.set_current_state(1);
        assert!(matches!(result, StateReturnType::Success));
    }
}
