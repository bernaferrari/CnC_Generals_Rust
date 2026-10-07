//! StateMachine.cpp:413-435: a callback changes state before its Sleep result.
use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Debug)]
struct RequestState {
    requested: Option<StateId>,
}
impl StateImplementation for RequestState {
    fn update(&mut self) -> StateReturnType {
        self.requested = Some(7);
        StateReturnType::Sleep(2000)
    }
    fn take_requested_state_change(&mut self) -> Option<StateId> {
        self.requested.take()
    }
}
#[derive(Debug)]
struct EnterState(Arc<AtomicUsize>);
impl StateImplementation for EnterState {
    fn on_enter(&mut self) -> StateReturnType {
        self.0.fetch_add(1, Ordering::SeqCst);
        StateReturnType::Continue
    }
    fn update(&mut self) -> StateReturnType {
        StateReturnType::Continue
    }
}
#[derive(Debug)]
struct TestAI;
impl crate::modules::AIUpdateInterface for TestAI {
    fn update(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }
    fn is_moving(&self) -> bool {
        false
    }
    fn is_idle(&self) -> bool {
        true
    }
    fn set_movement_target(&mut self, _: &Coord3D) -> Result<(), String> {
        Ok(())
    }
}
fn machine() -> (StateMachine, Arc<AtomicUsize>) {
    let mut core = StateMachine::new(None, "callback state change");
    let enters = Arc::new(AtomicUsize::new(0));
    core.define_state(
        5,
        Box::new(RequestState { requested: None }),
        None,
        None,
        None,
    );
    core.define_state(7, Box::new(EnterState(enters.clone())), None, None, None);
    assert_eq!(core.init_default_state(), StateReturnType::Continue);
    (core, enters)
}
fn assert_changed(core: &StateMachine, enters: &AtomicUsize, result: StateReturnType) {
    assert_eq!(
        result,
        StateReturnType::Continue,
        "C++ discards the outgoing state's Sleep"
    );
    assert_eq!(core.get_current_state_id(), Some(7));
    assert_eq!(
        core.sleep_till, 0,
        "new state must not inherit the outgoing sleep"
    );
    assert_eq!(
        enters.load(Ordering::SeqCst),
        1,
        "entry completes in this call"
    );
}
#[test]
fn owner_callback_change_precedes_sleep_processing() {
    let _serial = crate::test_sync::lock();
    let (mut core, enters) = machine();
    let result = core.update_with_owner(&mut ());
    assert_changed(&core, &enters, result);
}
#[test]
fn borrowed_ai_callback_change_precedes_sleep_processing() {
    let _serial = crate::test_sync::lock();
    let (mut core, enters) = machine();
    let result = core.update_with_ai(&mut TestAI);
    assert_changed(&core, &enters, result);
}
#[test]
fn locked_machine_rejects_callback_request_and_preserves_sleep() {
    let _serial = crate::test_sync::lock();
    let (mut core, enters) = machine();
    core.lock();
    assert_eq!(
        core.update_with_ai(&mut TestAI),
        StateReturnType::Sleep(2000)
    );
    assert_eq!(core.get_current_state_id(), Some(5));
    assert_eq!(enters.load(Ordering::SeqCst), 0);
}
