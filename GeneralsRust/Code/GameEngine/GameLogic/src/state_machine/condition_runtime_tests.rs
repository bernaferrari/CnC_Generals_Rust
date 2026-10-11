//! Actual generic FSM condition ordering and native error propagation.
use super::tests::FixedState;
use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Debug)]
struct Probe {
    calls: Arc<AtomicUsize>,
    ordinal: usize,
    answer: Result<bool, String>,
}

fn legacy_unused(_: &dyn StateImplementation, _: &StateTransitionUserData) -> bool {
    panic!("native condition must use its supplied callback");
}
fn condition(
    _: &dyn StateImplementation,
    data: &StateTransitionUserData,
    _: Option<&mut dyn crate::modules::ai_state_runtime::AiStateRuntime>,
) -> Result<bool, String> {
    let probe = data.data.as_ref().unwrap().downcast_ref::<Probe>().unwrap();
    assert_eq!(probe.calls.fetch_add(1, Ordering::SeqCst), probe.ordinal);
    probe.answer.clone()
}
fn transition(
    calls: &Arc<AtomicUsize>,
    ordinal: usize,
    answer: Result<bool, String>,
    destination: StateId,
) -> StateConditionInfo {
    StateConditionInfo::new(
        legacy_unused,
        destination,
        StateTransitionUserData::with_data(Probe {
            calls: calls.clone(),
            ordinal,
            answer,
        }),
        "native probe",
    )
    .with_ai_test(condition)
}
fn machine(transitions: &[StateConditionInfo]) -> StateMachine {
    let mut machine = StateMachine::new(None::<Weak<RwLock<Object>>>, "native-conditions");
    // Success without a success destination admits state 1 without evaluating
    // its Continue/sleep predicates during onEnter.
    machine.define_state(
        1,
        Box::new(FixedState::new(
            "source",
            StateReturnType::Success,
            StateReturnType::Continue,
        )),
        None,
        None,
        Some(transitions),
    );
    for id in [2, 3] {
        machine.define_state(
            id,
            Box::new(FixedState::new(
                "destination",
                StateReturnType::Continue,
                StateReturnType::Continue,
            )),
            None,
            None,
            None,
        );
    }
    assert_eq!(machine.set_current_state(1), StateReturnType::Success);
    machine
}
fn evaluate(machine: &mut StateMachine, status: StateReturnType) -> StateReturnType {
    let mut owner = ();
    if matches!(status, StateReturnType::Sleep(_)) {
        machine.check_for_sleep_transitions_ai(status, None, &mut owner)
    } else {
        machine.check_for_transitions_ai(status, None, &mut owner)
    }
}

#[test]
fn native_condition_error_stops_ordered_continue_and_sleep_without_transition() {
    for status in [StateReturnType::Continue, StateReturnType::Sleep(5)] {
        let calls = Arc::new(AtomicUsize::new(0));
        let transitions = [
            transition(
                &calls,
                0,
                Err("required owner service unavailable".into()),
                2,
            ),
            transition(&calls, 1, Ok(true), 3),
        ];
        let mut machine = machine(&transitions);
        assert_eq!(evaluate(&mut machine, status), StateReturnType::Failure);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(machine.get_current_state_id(), Some(1));
    }
}

#[test]
fn native_condition_false_then_true_preserves_first_match_continue_and_sleep() {
    for status in [StateReturnType::Continue, StateReturnType::Sleep(5)] {
        let calls = Arc::new(AtomicUsize::new(0));
        let transitions = [
            transition(&calls, 0, Ok(false), 3),
            transition(&calls, 1, Ok(true), 2),
            transition(&calls, 2, Err("must not run".into()), 3),
        ];
        let mut machine = machine(&transitions);
        assert_eq!(evaluate(&mut machine, status), StateReturnType::Continue);
        assert_eq!(calls.load(Ordering::SeqCst), 2);
        assert_eq!(machine.get_current_state_id(), Some(2));
    }
}
