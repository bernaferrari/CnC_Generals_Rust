//! Dispatch-contract preservation tests, separate from native game parity fixtures.
use super::*;
use crate::state_machine::cpp_state::{CppState, CppStateAdapter, cpp_transition};
use game_engine::common::system::{xfer_load::XferLoad, xfer_save::XferSave};
use std::io::Cursor;

#[derive(Debug)]
struct RecordingState {
    base: State,
    events: Vec<&'static str>,
    fail: bool,
    payload: u32,
}
impl RecordingState {
    fn new() -> Self {
        Self {
            base: State::with_machine(None, "contract"),
            events: vec![],
            fail: false,
            payload: 7,
        }
    }
    fn result(&self, status: StateReturnType) -> Result<StateReturnType, String> {
        if self.fail {
            Err("contract error".into())
        } else {
            Ok(status)
        }
    }
}
impl CppState for RecordingState {
    fn base_state(&self) -> &State {
        &self.base
    }
    fn base_state_mut(&mut self) -> &mut State {
        &mut self.base
    }
    fn cpp_on_enter(&mut self) -> Result<StateReturnType, String> {
        self.events.push("enter");
        self.result(StateReturnType::Success)
    }
    fn cpp_on_enter_with_ai(
        &mut self,
        ai: &mut dyn crate::modules::AIUpdateInterface,
    ) -> Result<StateReturnType, String> {
        self.events.push("enter-ai");
        ai.set_queue_for_path_time(11);
        self.result(StateReturnType::Sleep(7))
    }
    fn cpp_on_update(&mut self) -> Result<StateReturnType, String> {
        self.events.push("update");
        self.result(StateReturnType::Continue)
    }
    fn cpp_on_update_with_ai(
        &mut self,
        ai: &mut dyn crate::modules::AIUpdateInterface,
    ) -> Result<StateReturnType, String> {
        self.events.push("update-ai");
        ai.set_queue_for_path_time(13);
        self.result(StateReturnType::Sleep(13))
    }
    fn cpp_on_update_with_control(
        &mut self,
        control: &mut StateMachineControl,
        ai: Option<&mut dyn crate::modules::AIUpdateInterface>,
        locked: bool,
    ) -> Result<StateReturnType, String> {
        self.events.push("update-control");
        assert!(locked);
        control.set_goal_position(Coord3D::new(3.0, 5.0, 7.0));
        ai.expect("original borrowed AI")
            .set_queue_for_path_time(17);
        self.result(StateReturnType::Sleep(17))
    }
    fn cpp_on_exit(&mut self, exit: StateExitType) -> Result<(), String> {
        assert_eq!(exit, StateExitType::Reset);
        self.events.push("exit");
        if self.fail {
            Err("exit error".into())
        } else {
            Ok(())
        }
    }
    fn cpp_on_exit_with_ai(
        &mut self,
        exit: StateExitType,
        ai: &mut dyn crate::modules::AIUpdateInterface,
    ) -> Result<(), String> {
        assert_eq!(exit, StateExitType::Normal);
        self.events.push("exit-ai");
        ai.set_queue_for_path_time(19);
        if self.fail {
            Err("exit error".into())
        } else {
            Ok(())
        }
    }
    fn cpp_is_idle(&self) -> bool {
        true
    }
    fn cpp_is_attack(&self) -> bool {
        true
    }
    fn cpp_is_guard_idle(&self) -> bool {
        true
    }
    fn cpp_is_busy(&self) -> bool {
        true
    }
    fn cpp_locks_machine(&self) -> bool {
        true
    }
    fn cpp_freezes_parent_during_update(&self) -> bool {
        true
    }
    fn cpp_exit_restore_goal(&self) -> Option<Coord3D> {
        Some(Coord3D::new(11.0, 13.0, 17.0))
    }
    fn cpp_note_guard_enter(
        &mut self,
        mode: i32,
        _: Option<Arc<crate::polygon_trigger::PolygonTrigger>>,
    ) {
        self.payload = mode as u32;
    }
    fn cpp_xfer_snapshot(
        &mut self,
        xfer: &mut dyn crate::common::xfer::Xfer,
    ) -> Result<(), String> {
        xfer.xfer_unsigned_int(&mut self.payload)
            .map_err(|err| err.to_string())
    }
}
#[derive(Debug, Default)]
struct TestAI {
    path_time: u32,
}
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
    fn set_queue_for_path_time(&mut self, time: u32) {
        self.path_time = time;
    }
}

#[test]
fn callback_errors_map_to_failure_without_changing_success_statuses() {
    let mut state = CppStateAdapter::new(RecordingState::new());
    assert_eq!(state.on_enter(), StateReturnType::Success);
    assert_eq!(state.update(), StateReturnType::Continue);
    let mut failing = RecordingState::new();
    failing.fail = true;
    let mut state = CppStateAdapter::new(failing);
    assert_eq!(state.on_enter(), StateReturnType::Failure);
    assert_eq!(state.update(), StateReturnType::Failure);
    let mut ai = TestAI::default();
    assert_eq!(
        state.on_enter_with_ai(&mut ai, INVALID_ID, Coord3D::default()),
        StateReturnType::Failure
    );
    assert_eq!(state.update_with_ai(&mut ai), StateReturnType::Failure);
}

#[test]
fn ai_callbacks_receive_the_original_mutable_driver() {
    let mut state = CppStateAdapter::new(RecordingState::new());
    let mut ai = TestAI::default();
    assert_eq!(
        state.on_enter_with_ai(&mut ai, INVALID_ID, Coord3D::default()),
        StateReturnType::Sleep(7)
    );
    assert_eq!(ai.path_time, 11);
    assert_eq!(state.update_with_ai(&mut ai), StateReturnType::Sleep(13));
    assert_eq!(ai.path_time, 13);
}

#[test]
fn control_callback_mutates_the_driving_control_and_ai() {
    let mut state = CppStateAdapter::new(RecordingState::new());
    let mut machine = StateMachine::new(None, "contract control");
    let mut ai = TestAI::default();
    let mut operation_owner = 23u32;
    assert_eq!(
        state.update_with_control(
            &mut machine.control,
            Some(&mut ai),
            true,
            &mut operation_owner
        ),
        StateReturnType::Sleep(17)
    );
    assert_eq!(machine.get_goal_position(), Coord3D::new(3.0, 5.0, 7.0));
    assert_eq!(ai.path_time, 17);
    assert_eq!(operation_owner, 23);
}

#[test]
fn exit_errors_do_not_become_transition_results() {
    for fail in [false, true] {
        let mut inner = RecordingState::new();
        inner.fail = fail;
        let mut state = CppStateAdapter::new(inner);
        let mut ai = TestAI::default();
        state.on_exit(StateExitType::Reset);
        state.on_exit_with_ai(StateExitType::Normal, &mut ai);
        assert_eq!(ai.path_time, 19);
    }
}

#[test]
fn bound_owner_and_goals_retain_exact_identity_without_owner_reentry() {
    let mut state = CppStateAdapter::new(RecordingState::new());
    let owner = Arc::new(RwLock::new(Object::new_test(0xC0_2026, 100.0)));
    let _held = owner.write().unwrap();
    state.note_step_owner(owner.clone());
    state.bind_goal_object_id(91);
    state.bind_goal_position(Coord3D::new(2.0, 3.0, 5.0));
    state.bind_goal_waypoint(Some(37));
    assert!(Arc::ptr_eq(&state.get_machine_owner().unwrap(), &owner));
    assert_eq!(state.get_name(), "contract");
    state.set_id(43);
    assert_eq!(state.get_id(), 43);
    // The predicate sees the exact concrete state and all pre-bound goal fields.
    fn bound(state: &RecordingState, _: &StateTransitionUserData) -> Result<bool, String> {
        Ok(state.base.goal_object_id == 91
            && state.base.goal_position_copied == Some(Coord3D::new(2.0, 3.0, 5.0))
            && state.base.goal_waypoint_copied == Some(37))
    }
    let condition = cpp_transition(bound, 59, StateTransitionUserData::new(), "bound");
    assert!((condition.test)(&state, &condition.user_data));
}

#[test]
fn classifications_and_guard_hooks_reach_the_concrete_state() {
    let mut state = CppStateAdapter::new(RecordingState::new());
    assert!(state.is_idle() && state.is_attack() && state.is_guard_idle() && state.is_busy());
    assert!(state.locks_machine() && state.freezes_parent_during_update());
    assert_eq!(
        state.exit_restore_goal(),
        Some(Coord3D::new(11.0, 13.0, 17.0))
    );
    state.note_guard_enter(29, None);
    fn guard(state: &RecordingState, _: &StateTransitionUserData) -> Result<bool, String> {
        Ok(state.payload == 29)
    }
    let condition = cpp_transition(guard, 59, StateTransitionUserData::new(), "guard");
    assert!((condition.test)(&state, &condition.user_data));
}

#[test]
fn typed_predicates_preserve_payload_and_fail_closed_on_errors_or_wrong_types() {
    fn check(state: &RecordingState, data: &StateTransitionUserData) -> Result<bool, String> {
        Ok(state.payload == *data.data.as_ref().unwrap().downcast_ref::<u32>().unwrap())
    }
    fn error(_: &RecordingState, _: &StateTransitionUserData) -> Result<bool, String> {
        Err("predicate error".into())
    }
    let state = CppStateAdapter::new(RecordingState::new());
    let condition = cpp_transition(check, 59, StateTransitionUserData::with_data(7u32), "typed");
    assert!((condition.test)(&state, &condition.user_data));
    let error = cpp_transition(error, 61, StateTransitionUserData::new(), "error");
    assert!(!(error.test)(&state, &error.user_data));
    assert!(!(condition.test)(&state, &StateTransitionUserData::new()));
    assert!(!(condition.test)(
        &state,
        &StateTransitionUserData::with_data(7u32)
    ));
}

#[test]
fn state_payload_xfer_consumes_the_original_exact_wire() {
    let mut state = CppStateAdapter::new(RecordingState::new());
    let mut bytes = Cursor::new(Vec::new());
    state
        .xfer_snapshot(&mut XferSave::new(&mut bytes, 1))
        .unwrap();
    assert_eq!(bytes.get_ref(), &7u32.to_le_bytes());
    let mut input = 31u32.to_le_bytes().to_vec();
    input.extend_from_slice(&0xCAFE1234u32.to_le_bytes());
    let mut load = XferLoad::new(Cursor::new(input), 1);
    state.xfer_snapshot(&mut load).unwrap();
    let mut sentinel = 0;
    game_engine::common::system::Xfer::xfer_unsigned_int(&mut load, &mut sentinel).unwrap();
    assert_eq!(sentinel, 0xCAFE1234);
    let mut bytes = Cursor::new(Vec::new());
    state
        .xfer_snapshot(&mut XferSave::new(&mut bytes, 1))
        .unwrap();
    assert_eq!(bytes.get_ref(), &31u32.to_le_bytes());
}
