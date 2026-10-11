//! StateMachine.cpp:397-435 absolute, unsigned sleep timing on an explicit
//! operation clock. Uses the actual machine and snapshot wire, without a
//! published clock, object registry, AI mock, or test serialization lock.
use super::tests::FixedState;
use super::*;
use game_engine::common::system::{xfer_load::XferLoad, xfer_save::XferSave};
use std::io::Cursor;

fn machine() -> StateMachine {
    let mut machine = StateMachine::new_with_owner_id(crate::common::INVALID_ID, "operation clock");
    machine.define_state(
        1,
        Box::new(FixedState::new(
            "sleeper",
            StateReturnType::Continue,
            StateReturnType::Sleep(5),
        )),
        None,
        None,
        None,
    );
    assert_eq!(machine.set_current_state(1), StateReturnType::Continue);
    machine
}

#[test]
fn actual_machine_uses_operation_frame_and_preserves_exact_wake_boundary() {
    let mut first = machine();
    let mut peer = machine();
    assert_eq!(
        first.update_with_owner_at_frame(&mut (), 100),
        StateReturnType::Sleep(5)
    );
    assert_eq!(first.control.sleep_till, 105);
    // A different operation's clock cannot change this owner's deadline.
    assert_eq!(
        peer.update_with_owner_at_frame(&mut (), 400),
        StateReturnType::Sleep(5)
    );
    assert_eq!(
        first.update_with_owner_at_frame(&mut (), 104),
        StateReturnType::Sleep(1)
    );
    assert_eq!(first.control.sleep_till, 105);
    assert_eq!(
        first.update_with_owner_at_frame(&mut (), 105),
        StateReturnType::Sleep(5)
    );
    assert_eq!(first.control.sleep_till, 110);
    first.clear();
    assert_eq!(first.control.sleep_till, 0);
    assert_eq!(
        first.update_with_owner_at_frame(&mut (), 105),
        StateReturnType::Failure
    );
}

#[test]
fn saved_absolute_deadline_continues_on_operation_clock_without_readmitting_entry() {
    let mut first = machine();
    first.update_with_owner_at_frame(&mut (), 100);
    let mut bytes = Vec::new();
    first
        .xfer(&mut XferSave::new(Cursor::new(&mut bytes), 1))
        .unwrap();
    let mut restored = machine();
    restored
        .xfer(&mut XferLoad::new(&mut Cursor::new(bytes), 1))
        .unwrap();
    assert_eq!(restored.control.sleep_till, 105);
    for frame in [101, 104, 105, 108, 110] {
        assert_eq!(
            restored.update_with_owner_at_frame(&mut (), frame),
            first.update_with_owner_at_frame(&mut (), frame)
        );
        assert_eq!(restored.control.sleep_till, first.control.sleep_till);
    }
}

#[test]
fn operation_sleep_preserves_cpp_wrapping_and_zero_deadline_sentinel() {
    let mut machine = machine();
    assert_eq!(
        machine.update_with_owner_at_frame(&mut (), u32::MAX - 4),
        StateReturnType::Sleep(5)
    );
    assert_eq!(machine.control.sleep_till, 0);
    // Zero is the original not-sleeping sentinel; a wrapped positive deadline
    // is compared as unsigned, rather than using a new modular-time policy.
    assert_eq!(
        machine.update_with_owner_at_frame(&mut (), u32::MAX - 3),
        StateReturnType::Sleep(5)
    );
    assert_eq!(machine.control.sleep_till, 1);
    assert_eq!(
        machine.update_with_owner_at_frame(&mut (), 0),
        StateReturnType::Sleep(1)
    );
    assert_eq!(
        machine.update_with_owner_at_frame(&mut (), 1),
        StateReturnType::Sleep(5)
    );
    assert_eq!(machine.control.sleep_till, 6);
}
