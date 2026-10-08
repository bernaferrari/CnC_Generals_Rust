//! Borrowed execution of the dock's owned state and queue position.
//!
//! C++ child states reach these fields through AIDockMachine*. The Rust
//! machine lends one context for the complete enter/update/transition chain.

use super::*;
use crate::state_machine::StateImplementation;
use std::any::Any;

#[derive(Debug)]
pub(super) struct DockContext {
    pub(super) approach_position: i32,
}

impl Default for DockContext {
    fn default() -> Self {
        Self {
            approach_position: -1,
        }
    }
}

pub(super) trait DockState: std::fmt::Debug + Send + Sync + 'static {
    fn base_state(&self) -> &State;
    fn base_state_mut(&mut self) -> &mut State;
    fn move_helper(&mut self) -> Option<&mut AIInternalMoveToState> {
        None
    }
    fn dock_on_enter(
        &mut self,
        context: &mut DockContext,
        ai: Option<&mut dyn crate::modules::ai_state_runtime::AiStateRuntime>,
    ) -> Result<StateReturnType, String>;
    fn dock_on_update(
        &mut self,
        context: &mut DockContext,
        ai: Option<&mut dyn crate::modules::ai_state_runtime::AiStateRuntime>,
    ) -> Result<StateReturnType, String>;
    fn dock_on_exit(
        &mut self,
        exit: StateExitType,
        ai: Option<&mut dyn crate::modules::ai_state_runtime::AiStateRuntime>,
    ) -> Result<(), String>;
    fn dock_after_exit(
        &mut self,
        _exit: StateExitType,
        _ai: Option<&mut dyn crate::modules::ai_state_runtime::AiStateRuntime>,
    ) -> Result<(), String> {
        Ok(())
    }
    fn dock_unlocks_on_exit(&self) -> bool {
        self.dock_locks_machine()
    }
    fn dock_xfer_snapshot(&mut self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }
    fn dock_locks_machine(&self) -> bool {
        false
    }
}

#[derive(Debug)]
struct DockStateAdapter<S: DockState>(S);

impl<S: DockState> DockStateAdapter<S> {
    fn status(result: Result<StateReturnType, String>) -> StateReturnType {
        result.unwrap_or_else(|error| {
            log::warn!("dock state returned error: {}", error);
            StateReturnType::Failure
        })
    }
}

impl<S: DockState> StateImplementation for DockStateAdapter<S> {
    fn update(&mut self) -> StateReturnType {
        StateReturnType::Failure
    }
    fn on_enter_with_owner(&mut self, owner: &mut dyn Any) -> StateReturnType {
        let Some(context) = owner.downcast_mut::<DockContext>() else {
            return StateReturnType::Failure;
        };
        Self::status(self.0.dock_on_enter(context, None))
    }
    fn on_enter_with_ai_and_owner(
        &mut self,
        ai: &mut dyn crate::modules::ai_state_runtime::AiStateRuntime,
        _goal: ObjectID,
        _pos: Coord3D,
        _waypoint: Option<crate::waypoint::WaypointId>,
        owner: &mut dyn Any,
    ) -> StateReturnType {
        let Some(context) = owner.downcast_mut::<DockContext>() else {
            return StateReturnType::Failure;
        };
        Self::status(self.0.dock_on_enter(context, Some(ai)))
    }
    fn update_with_owner(&mut self, owner: &mut dyn Any) -> StateReturnType {
        let Some(context) = owner.downcast_mut::<DockContext>() else {
            return StateReturnType::Failure;
        };
        Self::status(self.0.dock_on_update(context, None))
    }
    fn update_with_ai_and_owner(
        &mut self,
        ai: &mut dyn crate::modules::ai_state_runtime::AiStateRuntime,
        _locked: bool,
        owner: &mut dyn Any,
    ) -> StateReturnType {
        let Some(context) = owner.downcast_mut::<DockContext>() else {
            return StateReturnType::Failure;
        };
        Self::status(self.0.dock_on_update(context, Some(ai)))
    }
    fn on_exit_with_owner(&mut self, status: StateExitType, _owner: &mut dyn Any) {
        if let Err(error) = self.0.dock_on_exit(status, None) {
            log::warn!("dock state exit: {}", error);
        }
    }
    fn on_exit_with_ai_and_owner(
        &mut self,
        status: StateExitType,
        ai: &mut dyn crate::modules::ai_state_runtime::AiStateRuntime,
        _owner: &mut dyn Any,
    ) {
        if let Err(error) = self.0.dock_on_exit(status, Some(ai)) {
            log::warn!("dock state exit: {}", error);
        }
    }
    fn unlocks_machine_on_exit(&self) -> bool {
        self.0.dock_unlocks_on_exit()
    }
    fn on_exit_after_unlock(
        &mut self,
        exit: StateExitType,
        ai: Option<&mut dyn crate::modules::ai_state_runtime::AiStateRuntime>,
        _owner: &mut dyn Any,
    ) {
        if let Err(error) = self.0.dock_after_exit(exit, ai) {
            log::warn!("dock movement cleanup: {}", error);
        }
    }
    fn get_name(&self) -> &str {
        self.0.base_state().get_name()
    }
    fn get_id(&self) -> crate::state_machine::StateId {
        self.0.base_state().get_id()
    }
    fn set_id(&mut self, id: crate::state_machine::StateId) {
        self.0.base_state_mut().set_id(id);
    }
    fn is_busy(&self) -> bool {
        true
    }
    fn locks_machine(&self) -> bool {
        self.0.dock_locks_machine()
    }
    fn xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.0.dock_xfer_snapshot(xfer)
    }
    fn note_step_owner(&mut self, owner: Arc<RwLock<Object>>) {
        self.0.base_state_mut().bind_owner(&owner);
        if let Some(helper) = self.0.move_helper() {
            helper.bind_owner(&owner);
        }
    }
    fn bind_goal_object_id(&mut self, id: ObjectID) {
        self.0.base_state_mut().goal_object_id = id;
        // A dock child moves to its own dock/queue position. The machine goal
        // identifies the dock; it is not the movement helper's tracking target.
    }
    fn bind_goal_position(&mut self, pos: Coord3D) {
        self.0.base_state_mut().goal_position_copied = Some(pos);
    }
}

pub(super) fn register_dock_state<S: DockState>(
    machine: &mut StateMachine,
    id: u32,
    mut state: S,
    success: Option<u32>,
    failure: Option<u32>,
    conditions: &[StateConditionInfo],
) {
    if let Some(owner) = machine.get_owner() {
        state.base_state_mut().bind_owner(&owner);
        if let Some(helper) = state.move_helper() {
            helper.bind_owner(&owner);
        }
    }
    machine.define_state(
        id,
        Box::new(DockStateAdapter(state)),
        success,
        failure,
        Some(conditions),
    );
}

pub(super) fn clearance_without_context(
    _: &dyn StateImplementation,
    _: &StateTransitionUserData,
) -> bool {
    false
}

pub(super) fn clearance_with_context(
    state: &dyn StateImplementation,
    _: &StateTransitionUserData,
    owner: &mut dyn Any,
) -> bool {
    let Some(state) = state
        .as_any()
        .downcast_ref::<DockStateAdapter<AIDockWaitForClearanceState>>()
    else {
        return false;
    };
    let Some(context) = owner.downcast_ref::<DockContext>() else {
        return false;
    };
    state
        .0
        .able_to_advance(context.approach_position)
        .unwrap_or(false)
}
