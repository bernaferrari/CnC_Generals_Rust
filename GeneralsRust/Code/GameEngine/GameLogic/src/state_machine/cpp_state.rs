//! Native C++ `State` callbacks and their single dispatch adapter.
//!
//! `Common/StateMachine.h` defines the base state and callbacks. Concrete AI
//! states retain their C++ names and embedded base state; registration owns
//! them through the generic FSM. This module centralizes error mapping, goal
//! binding, snapshot dispatch, and concrete transition predicate identity.

use crate::object::Object;
use crate::state_machine as core;
use crate::state_machine::{
    StateConditionInfo, StateExitType, StateId, StateReturnType, StateTransitionUserData,
};
use std::any::Any;
#[cfg(test)]
use std::sync::Weak;
use std::sync::{Arc, Mutex, RwLock};

/// Callbacks of the translated C++ state, using its embedded base for identity.
///
/// Rust callback errors become FSM failure on enter/update and are logged on
/// exit. Nested movement operations retain their separate explicit arguments;
/// registration dispatches only these callbacks against the bound base state.
pub(crate) trait CppState: std::fmt::Debug + Send + Sync + Any {
    /// Immutable view of the embedded base state.
    fn base_state(&self) -> &core::State;
    /// Mutable view of the embedded base state.
    fn base_state_mut(&mut self) -> &mut core::State;

    /// Original `OnEnter` callback.
    fn cpp_on_enter(&mut self) -> Result<StateReturnType, String>;

    fn cpp_on_enter_with_ai(
        &mut self,
        _ai: &mut dyn crate::modules::AIUpdateInterface,
    ) -> Result<StateReturnType, String> {
        self.cpp_on_enter()
    }

    fn cpp_on_update_with_ai(
        &mut self,
        _ai: &mut dyn crate::modules::AIUpdateInterface,
    ) -> Result<StateReturnType, String> {
        self.cpp_on_update()
    }
    fn cpp_on_update_with_control(
        &mut self,
        _control: &mut core::StateMachineControl,
        ai: Option<&mut dyn crate::modules::AIUpdateInterface>,
        _machine_locked: bool,
    ) -> Result<StateReturnType, String> {
        match ai {
            Some(ai) => self.cpp_on_update_with_ai(ai),
            None => self.cpp_on_update(),
        }
    }

    /// Optional owner context for operation-local typed state outputs. Existing
    /// states retain the exact control callback unless they opt into this hook.
    fn cpp_on_update_with_context(
        &mut self,
        control: &mut core::StateMachineControl,
        ai: Option<&mut dyn crate::modules::AIUpdateInterface>,
        machine_locked: bool,
        owner: &mut dyn Any,
    ) -> Result<StateReturnType, String> {
        let _ = owner;
        self.cpp_on_update_with_control(control, ai, machine_locked)
    }

    fn cpp_on_update(&mut self) -> Result<StateReturnType, String>;
    /// Original `OnExit` callback.
    fn cpp_on_exit(&mut self, exit: StateExitType) -> Result<(), String>;

    fn cpp_on_exit_with_ai(
        &mut self,
        exit: StateExitType,
        _ai: &mut dyn crate::modules::AIUpdateInterface,
    ) -> Result<(), String> {
        self.cpp_on_exit(exit)
    }

    fn cpp_on_exit_with_control(
        &mut self,
        _control: &mut core::StateMachineControl,
        exit: StateExitType,
        ai: Option<&mut dyn crate::modules::AIUpdateInterface>,
    ) -> Result<(), String> {
        match ai {
            Some(ai) => self.cpp_on_exit_with_ai(exit, ai),
            None => self.cpp_on_exit(exit),
        }
    }

    /// Override when the state should be considered idle.
    fn cpp_is_idle(&self) -> bool {
        false
    }
    /// Override when the state represents an attack.
    fn cpp_is_attack(&self) -> bool {
        false
    }
    /// Override when the state is a guard idle.
    fn cpp_is_guard_idle(&self) -> bool {
        false
    }

    /// Optional hook for starting movement audio (C++ StartMoveSound).
    fn start_move_sound(&mut self, _owner_guard: &Object) {}
    /// Optional snapshot hook used by registered C++ states.
    fn cpp_xfer_snapshot(
        &mut self,
        _xfer: &mut dyn crate::common::xfer::Xfer,
    ) -> Result<(), String> {
        Ok(())
    }
    /// Override only for the explicit Busy state (Common/StateMachine.h:136).
    fn cpp_is_busy(&self) -> bool {
        false
    }

    fn cpp_locks_machine(&self) -> bool {
        false
    }

    fn cpp_exit_restore_goal(&self) -> Option<crate::common::Coord3D> {
        None
    }

    fn cpp_freezes_parent_during_update(&self) -> bool {
        false
    }

    fn cpp_note_guard_enter(
        &mut self,
        _mode: i32,
        _polygon: Option<Arc<crate::polygon_trigger::PolygonTrigger>>,
    ) {
    }
}

/// Owns a concrete C++ state behind generic FSM dispatch.
#[derive(Debug)]
pub(crate) struct CppStateAdapter<S: CppState> {
    inner: S,
}

struct TransitionThunk<S: CppState + 'static> {
    predicate: fn(&S, &StateTransitionUserData) -> Result<bool, String>,
    user_data: StateTransitionUserData,
}

impl<S: CppState> CppStateAdapter<S> {
    /// Narrow concrete access preserves the registered adapter and state identity.
    pub(crate) fn inner_mut(&mut self) -> &mut S {
        &mut self.inner
    }

    #[cfg(test)]
    pub(crate) fn inner_mut_for_test(&mut self) -> &mut S {
        &mut self.inner
    }

    pub fn new(inner: S) -> Self {
        Self { inner }
    }

    fn map_result(label: &str, result: Result<StateReturnType, String>) -> StateReturnType {
        match result {
            Ok(value) => value,
            Err(err) => {
                log::warn!("C++ state {} returned error: {}", label, err);
                StateReturnType::Failure
            }
        }
    }
}

impl<S: CppState + 'static> core::StateImplementation for CppStateAdapter<S> {
    fn on_exit_with_control(
        &mut self,
        control: &mut core::StateMachineControl,
        exit: StateExitType,
        ai: Option<&mut dyn crate::modules::AIUpdateInterface>,
        _owner: &mut dyn Any,
    ) {
        let name = self.inner.base_state().get_name().to_string();
        let _ = Self::map_result(
            &name,
            self.inner
                .cpp_on_exit_with_control(control, exit, ai)
                .map(|_| StateReturnType::Continue),
        );
    }
    fn on_enter(&mut self) -> StateReturnType {
        let state_name = self.inner.base_state().get_name().to_string();
        Self::map_result(state_name.as_str(), self.inner.cpp_on_enter())
    }

    fn on_enter_with_ai(
        &mut self,
        ai: &mut dyn crate::modules::AIUpdateInterface,
        _goal_id: crate::common::ObjectID,
        _goal_pos: crate::common::Coord3D,
    ) -> StateReturnType {
        let state_name = self.inner.base_state().get_name().to_string();
        Self::map_result(state_name.as_str(), self.inner.cpp_on_enter_with_ai(ai))
    }

    fn on_exit(&mut self, exit: StateExitType) {
        let state_name = self.inner.base_state().get_name().to_string();
        let _ = Self::map_result(
            state_name.as_str(),
            self.inner
                .cpp_on_exit(exit)
                .map(|_| StateReturnType::Continue),
        );
    }

    fn on_exit_with_ai(
        &mut self,
        exit: StateExitType,
        ai: &mut dyn crate::modules::AIUpdateInterface,
    ) {
        let name = self.inner.base_state().get_name().to_string();
        let _ = Self::map_result(
            &name,
            self.inner
                .cpp_on_exit_with_ai(exit, ai)
                .map(|_| StateReturnType::Continue),
        );
    }

    fn update(&mut self) -> StateReturnType {
        let state_name = self.inner.base_state().get_name().to_string();
        Self::map_result(state_name.as_str(), self.inner.cpp_on_update())
    }

    fn update_with_ai(
        &mut self,
        ai: &mut dyn crate::modules::AIUpdateInterface,
    ) -> StateReturnType {
        let state_name = self.inner.base_state().get_name().to_string();
        Self::map_result(state_name.as_str(), self.inner.cpp_on_update_with_ai(ai))
    }
    fn update_with_control(
        &mut self,
        control: &mut core::StateMachineControl,
        ai: Option<&mut dyn crate::modules::AIUpdateInterface>,
        machine_locked: bool,
        owner: &mut dyn Any,
    ) -> StateReturnType {
        let name = self.inner.base_state().get_name().to_string();
        Self::map_result(
            &name,
            self.inner
                .cpp_on_update_with_context(control, ai, machine_locked, owner),
        )
    }

    fn is_idle(&self) -> bool {
        self.inner.cpp_is_idle()
    }

    fn is_attack(&self) -> bool {
        self.inner.cpp_is_attack()
    }

    fn is_guard_idle(&self) -> bool {
        self.inner.cpp_is_guard_idle()
    }

    fn is_busy(&self) -> bool {
        self.inner.cpp_is_busy()
    }

    fn get_name(&self) -> &str {
        self.inner.base_state().get_name()
    }

    fn get_id(&self) -> StateId {
        self.inner.base_state().get_id()
    }

    fn set_id(&mut self, id: StateId) {
        self.inner.base_state_mut().set_id(id);
    }

    fn get_machine_goal_object(&self) -> Result<Option<Arc<RwLock<Object>>>, String> {
        Ok(self.inner.base_state().get_machine_goal_object())
    }

    fn get_machine_owner(&self) -> Result<Arc<RwLock<Object>>, String> {
        self.inner
            .base_state()
            .get_machine_owner()
            .ok_or_else(|| "state machine owner not attached".to_string())
    }

    fn get_machine(&self) -> Result<Arc<Mutex<core::StateMachine>>, String> {
        self.inner.base_state().get_machine()
    }

    fn xfer_snapshot(&mut self, xfer: &mut dyn crate::common::xfer::Xfer) -> Result<(), String> {
        self.inner.cpp_xfer_snapshot(xfer)
    }

    fn bind_goal_object_id(&mut self, id: crate::common::ObjectID) {
        self.inner.base_state_mut().goal_object_id = id;
    }

    fn locks_machine(&self) -> bool {
        self.inner.cpp_locks_machine()
    }

    fn bind_goal_waypoint(&mut self, waypoint: Option<crate::waypoint::WaypointId>) {
        self.inner.base_state_mut().goal_waypoint_copied = waypoint;
    }

    fn note_step_owner(&mut self, owner: Arc<RwLock<Object>>) {
        self.inner.base_state_mut().bind_owner(&owner);
    }

    fn note_guard_enter(
        &mut self,
        mode: i32,
        polygon: Option<Arc<crate::polygon_trigger::PolygonTrigger>>,
    ) {
        self.inner.cpp_note_guard_enter(mode, polygon);
    }

    fn exit_restore_goal(&self) -> Option<crate::common::Coord3D> {
        self.inner.cpp_exit_restore_goal()
    }

    fn freezes_parent_during_update(&self) -> bool {
        self.inner.cpp_freezes_parent_during_update()
    }

    fn bind_goal_position(&mut self, pos: crate::common::Coord3D) {
        self.inner.base_state_mut().goal_position_copied = Some(pos);
    }

    fn bind_goal_squad(&mut self, squad: Option<Arc<crate::ai::squad::Squad>>) {
        self.inner.base_state_mut().goal_squad_copied = squad;
    }

    fn bind_goal_polygon(&mut self, polygon: Option<Arc<crate::polygon_trigger::PolygonTrigger>>) {
        self.inner.base_state_mut().goal_polygon_copied = polygon;
    }

    fn evaluate_transition_payload(&self, payload: &(dyn Any + Send + Sync)) -> Option<bool> {
        let thunk = payload.downcast_ref::<TransitionThunk<S>>()?;
        match (thunk.predicate)(&self.inner, &thunk.user_data) {
            Ok(result) => Some(result),
            Err(err) => {
                log::warn!("C++ transition predicate failed: {}", err);
                Some(false)
            }
        }
    }
}

/// Register a translated C++ state without an intermediate forwarding trait.
pub(crate) fn register_cpp_state<S: CppState + 'static>(
    machine: &mut core::StateMachine,
    id: StateId,
    state: S,
    success_id: Option<StateId>,
    failure_id: Option<StateId>,
    conditions: &[StateConditionInfo],
) {
    machine.define_state(
        id,
        Box::new(CppStateAdapter::new(state)),
        success_id,
        failure_id,
        Some(conditions),
    );
}

/// Preserve the concrete state type and user data when erasing a predicate.
pub(crate) fn cpp_transition<S: CppState + 'static>(
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

    impl CppState for DummyState {
        fn base_state(&self) -> &core::State {
            &self.base
        }

        fn base_state_mut(&mut self) -> &mut core::State {
            &mut self.base
        }

        fn cpp_on_enter(&mut self) -> Result<StateReturnType, String> {
            self.entered = true;
            Ok(StateReturnType::Success)
        }

        fn cpp_on_update(&mut self) -> Result<StateReturnType, String> {
            Ok(StateReturnType::Continue)
        }

        fn cpp_on_exit(&mut self, _exit: StateExitType) -> Result<(), String> {
            self.exited = true;
            Ok(())
        }

        fn cpp_is_idle(&self) -> bool {
            true
        }
    }

    #[test]
    fn cpp_adapter_registers_and_runs() {
        let mut machine = core::StateMachine::new(None::<Weak<RwLock<Object>>>, "cpp-test");

        register_cpp_state(
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
