//! AIStates.cpp:660-714 defines states; AIUpdate.cpp:603-612 activates them later.
use super::{AIStateMachine, AIStateType};
use crate::object::Object;
use crate::state_machine::StateReturnType;
use game_engine::common::system::{Snapshotable, xfer_load::XferLoad, xfer_save::XferSave};
use std::io::Cursor;
use std::sync::{Arc, RwLock};

fn owner() -> Arc<RwLock<Object>> {
    Arc::new(RwLock::new(Object::new_test(0x7AF10010, 100.0)))
}

fn machine(owner: &Arc<RwLock<Object>>) -> AIStateMachine {
    AIStateMachine::new(Arc::downgrade(owner), "canonical construction")
}

fn save(machine: &mut AIStateMachine) -> Vec<u8> {
    let mut bytes = Vec::new();
    machine
        .xfer(&mut XferSave::new(Cursor::new(&mut bytes), 1))
        .unwrap();
    bytes
}

#[test]
fn construction_defines_states_without_idle_entry() {
    let first = owner();
    let second = owner();
    let first_machine = machine(&first);
    let second_machine = machine(&second);
    assert_eq!(first_machine.get_current_state_id(), None);
    assert_eq!(second_machine.get_current_state_id(), None);
    assert!(!first.read().unwrap().ai_pending_reset_mood);
    assert!(!second.read().unwrap().ai_pending_reset_mood);
    assert!(Arc::ptr_eq(
        &first_machine.base.get_owner().unwrap(),
        &first
    ));
    assert!(Arc::ptr_eq(
        &second_machine.base.get_owner().unwrap(),
        &second
    ));
}

#[test]
fn explicit_default_entry_is_once_only_and_reset_uses_exact_owner() {
    let _serial = crate::test_sync::lock();
    let first = owner();
    let second = owner();
    let mut first_machine = machine(&first);
    let second_machine = machine(&second);
    assert_eq!(first_machine.get_current_state_id(), None);
    assert_eq!(
        first_machine.base.init_default_state(),
        StateReturnType::Continue
    );
    assert_eq!(
        first_machine.get_current_state_id(),
        Some(AIStateType::Idle as u32)
    );
    assert!(first.read().unwrap().ai_pending_reset_mood);
    assert!(!second.read().unwrap().ai_pending_reset_mood);
    assert_eq!(second_machine.get_current_state_id(), None);
    first.write().unwrap().ai_pending_reset_mood = false;
    assert_eq!(
        first_machine.base.init_default_state(),
        StateReturnType::Failure
    );
    assert!(!first.read().unwrap().ai_pending_reset_mood);
    assert_eq!(
        first_machine.base.reset_to_default_state(),
        StateReturnType::Continue
    );
    assert!(first.read().unwrap().ai_pending_reset_mood);
    assert!(!second.read().unwrap().ai_pending_reset_mood);
}

#[test]
fn restore_into_unentered_parent_preserves_bytes_without_idle_reentry() {
    let _serial = crate::test_sync::lock();
    let first = owner();
    let second = owner();
    let mut source = machine(&first);
    assert_eq!(source.base.init_default_state(), StateReturnType::Continue);
    let bytes = save(&mut source);
    let mut restored = machine(&second);
    assert_eq!(restored.get_current_state_id(), None);
    restored
        .xfer(&mut XferLoad::new(&mut Cursor::new(bytes.clone()), 1))
        .unwrap();
    assert!(!second.read().unwrap().ai_pending_reset_mood);
    assert!(Arc::ptr_eq(&restored.base.get_owner().unwrap(), &second));
    assert_eq!(
        restored.get_current_state_id(),
        Some(AIStateType::Idle as u32)
    );
    assert_eq!(restored.base.init_default_state(), StateReturnType::Failure);
    assert_eq!(save(&mut restored), bytes);
    assert!(!second.read().unwrap().ai_pending_reset_mood);
}
