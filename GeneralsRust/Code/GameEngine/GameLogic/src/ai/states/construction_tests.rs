//! AIStates.cpp:660-714 defines states; AIUpdate.cpp:603-612 activates them later.
use super::{AIStateMachine, AIStateType};
use crate::object::Object;
use crate::state_machine::StateReturnType;
use game_engine::common::random_value::{
    get_game_logic_random_seed_state, set_game_logic_random_seed_state,
};
use game_engine::common::system::{Snapshotable, xfer_load::XferLoad, xfer_save::XferSave};
use std::io::Cursor;
use std::sync::{Arc, RwLock};

struct PreserveRng([u32; 6]);
impl Drop for PreserveRng {
    fn drop(&mut self) {
        set_game_logic_random_seed_state(self.0);
    }
}

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
fn construction_defines_states_without_idle_entry_or_rng_consumption() {
    let _serial = crate::test_sync::lock();
    let _rng = PreserveRng(get_game_logic_random_seed_state());
    let first = owner();
    let second = owner();
    let before = get_game_logic_random_seed_state();
    let first_machine = machine(&first);
    let second_machine = machine(&second);
    assert_eq!(first_machine.get_current_state_id(), None);
    assert_eq!(second_machine.get_current_state_id(), None);
    assert_eq!(get_game_logic_random_seed_state(), before);
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
    let _rng = PreserveRng(get_game_logic_random_seed_state());
    let first = owner();
    let second = owner();
    let mut first_machine = machine(&first);
    let second_machine = machine(&second);
    assert_eq!(first_machine.get_current_state_id(), None);
    let before = get_game_logic_random_seed_state();
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
    let entered_rng = get_game_logic_random_seed_state();
    assert_ne!(
        entered_rng, before,
        "Idle entry consumes its original random offset"
    );
    first.write().unwrap().ai_pending_reset_mood = false;
    assert_eq!(
        first_machine.base.init_default_state(),
        StateReturnType::Failure
    );
    assert_eq!(get_game_logic_random_seed_state(), entered_rng);
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
    let _rng = PreserveRng(get_game_logic_random_seed_state());
    let first = owner();
    let second = owner();
    let mut source = machine(&first);
    assert_eq!(source.base.init_default_state(), StateReturnType::Continue);
    let bytes = save(&mut source);
    let before = get_game_logic_random_seed_state();
    let mut restored = machine(&second);
    assert_eq!(restored.get_current_state_id(), None);
    assert_eq!(get_game_logic_random_seed_state(), before);
    restored
        .xfer(&mut XferLoad::new(&mut Cursor::new(bytes.clone()), 1))
        .unwrap();
    assert_eq!(get_game_logic_random_seed_state(), before);
    assert!(!second.read().unwrap().ai_pending_reset_mood);
    assert!(Arc::ptr_eq(&restored.base.get_owner().unwrap(), &second));
    assert_eq!(
        restored.get_current_state_id(),
        Some(AIStateType::Idle as u32)
    );
    assert_eq!(restored.base.init_default_state(), StateReturnType::Failure);
    assert_eq!(save(&mut restored), bytes);
    assert_eq!(get_game_logic_random_seed_state(), before);
    assert!(!second.read().unwrap().ai_pending_reset_mood);
}
