//! Owner/AI borrow regression controls for StateMachine state transitions.
//!
//! Included as a test-only sibling of the production StateMachine module.
//! Kept separate so the production API and existing tests remain untouched.

use super::*;
use std::any::Any;

#[derive(Default)]
struct OwnerContext {
    identity: &'static str,
    transition_ready: bool,
    events: Vec<&'static str>,
}

#[derive(Debug, Default)]
struct BorrowedAi {
    movement_targets: usize,
}

impl crate::modules::AIUpdateInterface for BorrowedAi {
    fn update(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    fn is_moving(&self) -> bool {
        true
    }

    fn is_idle(&self) -> bool {
        false
    }

    fn set_movement_target(&mut self, _target: &Coord3D) -> Result<(), String> {
        self.movement_targets += 1;
        Ok(())
    }
}

#[derive(Debug)]
struct OwnerState {
    id: StateId,
    label: &'static str,
    result: StateReturnType,
    locks: bool,
}

impl OwnerState {
    fn new(label: &'static str, result: StateReturnType, locks: bool) -> Self {
        Self {
            id: INVALID_STATE_ID,
            label,
            result,
            locks,
        }
    }

    fn record(owner: &mut dyn Any, event: &'static str) {
        let owner = owner
            .downcast_mut::<OwnerContext>()
            .expect("same owner context must reach every callback");
        assert_eq!(owner.identity, "driving owner");
        owner.events.push(event);
    }
}

impl StateImplementation for OwnerState {
    fn update(&mut self) -> StateReturnType {
        self.result
    }

    fn update_with_ai_and_owner(
        &mut self,
        ai: &mut dyn crate::modules::AIUpdateInterface,
        _machine_locked: bool,
        owner: &mut dyn Any,
    ) -> StateReturnType {
        assert!(ai.is_moving(), "the caller's AI borrow must be forwarded");
        Self::record(owner, self.label);
        self.result
    }

    fn on_enter_with_ai_and_owner(
        &mut self,
        ai: &mut dyn crate::modules::AIUpdateInterface,
        _goal: crate::common::ObjectID,
        _position: Coord3D,
        _waypoint: Option<crate::waypoint::WaypointId>,
        owner: &mut dyn Any,
    ) -> StateReturnType {
        assert!(ai.is_moving(), "entry receives the same AI borrow");
        Self::record(owner, self.label);
        StateReturnType::Continue
    }

    fn on_exit_with_ai_and_owner(
        &mut self,
        _status: StateExitType,
        ai: &mut dyn crate::modules::AIUpdateInterface,
        owner: &mut dyn Any,
    ) {
        assert!(ai.is_moving(), "exit receives the same AI borrow");
        Self::record(owner, "exit");
    }

    fn on_exit_after_unlock(
        &mut self,
        _status: StateExitType,
        ai: Option<&mut dyn crate::modules::AIUpdateInterface>,
        owner: &mut dyn Any,
    ) {
        assert!(ai.is_some_and(|ai| ai.is_moving()));
        Self::record(owner, "after_unlock");
    }

    fn locks_machine(&self) -> bool {
        self.locks
    }

    fn unlocks_machine_on_exit(&self) -> bool {
        self.locks
    }

    fn get_name(&self) -> &str {
        self.label
    }

    fn get_id(&self) -> StateId {
        self.id
    }

    fn set_id(&mut self, id: StateId) {
        self.id = id;
    }
}

fn owner_condition(
    state: &dyn StateImplementation,
    _data: &StateTransitionUserData,
    owner: &mut dyn Any,
) -> bool {
    assert_eq!(state.get_id(), 1);
    if !owner
        .downcast_ref::<OwnerContext>()
        .unwrap()
        .transition_ready
    {
        return false;
    }
    OwnerState::record(owner, "condition");
    true
}

#[test]
fn conditional_transition_keeps_one_owner_and_ai_borrow_through_exit_and_enter() {
    let mut machine = StateMachine::new_with_owner_id(INVALID_ID, "owner-transition");
    let conditions = [StateConditionInfo::new(
        always_false,
        2,
        StateTransitionUserData::new(),
        "owner condition",
    )
    .with_owner_test(owner_condition)];
    machine.define_state(
        1,
        Box::new(OwnerState::new("update", StateReturnType::Continue, true)),
        None,
        None,
        Some(&conditions),
    );
    machine.define_state(
        2,
        Box::new(OwnerState::new(
            "next_enter",
            StateReturnType::Continue,
            false,
        )),
        None,
        None,
        None,
    );

    let mut owner = OwnerContext {
        identity: "driving owner",
        ..Default::default()
    };
    let mut ai = BorrowedAi::default();
    assert_eq!(
        machine.init_default_state_with_ai_and_owner(&mut ai, &mut owner),
        StateReturnType::Continue
    );
    assert_eq!(machine.get_current_state_id(), Some(1));
    assert!(machine.is_locked());
    owner.transition_ready = true;
    assert_eq!(
        machine.update_with_ai_and_owner(&mut ai, &mut owner),
        StateReturnType::Continue
    );
    assert_eq!(machine.get_current_state_id(), Some(2));
    assert!(!machine.is_locked());
    assert_eq!(
        owner.events,
        [
            "update",
            "update",
            "condition",
            "exit",
            "after_unlock",
            "next_enter"
        ]
    );
}

#[test]
fn terminal_transition_runs_owner_ai_exit_cleanup_and_unlock_hook() {
    let mut machine = StateMachine::new_with_owner_id(INVALID_ID, "owner-terminal");
    machine.define_state(
        1,
        Box::new(OwnerState::new("update", StateReturnType::Success, true)),
        Some(StateMachine::EXIT_MACHINE_WITH_SUCCESS),
        None,
        None,
    );
    let mut owner = OwnerContext {
        identity: "driving owner",
        ..Default::default()
    };
    let mut ai = BorrowedAi::default();
    assert_eq!(
        machine.init_default_state_with_ai_and_owner(&mut ai, &mut owner),
        StateReturnType::Continue
    );
    assert_eq!(
        machine.update_with_ai_and_owner(&mut ai, &mut owner),
        StateReturnType::Success
    );
    assert_eq!(machine.get_current_state_id(), None);
    assert!(!machine.is_locked());
    assert_eq!(owner.events, ["update", "update", "exit", "after_unlock"]);
}

fn always_false(_: &dyn StateImplementation, _: &StateTransitionUserData) -> bool {
    false
}
