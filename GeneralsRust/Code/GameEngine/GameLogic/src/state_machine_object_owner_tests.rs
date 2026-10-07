//! C++ StateMachine keeps its actual Object*, independently of numerical ID lookup.
use super::*;
use crate::ai::states::{AIIdleState, AIStateType};
use crate::common::Coord3D;
use game_engine::common::system::{xfer_load::XferLoad, xfer_save::XferSave};
use std::io::Cursor;

struct PreserveRng([u32; 6]);
impl Drop for PreserveRng {
    fn drop(&mut self) {
        game_engine::common::random_value::set_game_logic_random_seed_state(self.0);
    }
}

fn idle_machine(owner: &Arc<RwLock<Object>>) -> StateMachine {
    let mut machine = StateMachine::new(Some(Arc::downgrade(owner)), "idle owner witness");
    let state = AIIdleState::new(&machine, true);
    crate::compat::register_classic_state(
        &mut machine,
        AIStateType::Idle as u32,
        state,
        Some(AIStateType::Idle as u32),
        Some(AIStateType::Idle as u32),
        &[],
    );
    machine
}

fn object(id: u32, x: f32) -> Arc<RwLock<Object>> {
    let mut object = Object::new_test(id, 100.0);
    object.set_position(&Coord3D::new(x, 0.0, 0.0)).unwrap();
    Arc::new(RwLock::new(object))
}

struct Decoy(u32);
impl Drop for Decoy {
    fn drop(&mut self) {
        crate::object::registry::OBJECT_REGISTRY.unregister_object(self.0);
    }
}
fn publish_decoy(object: &Arc<RwLock<Object>>) -> Decoy {
    let id = object.read().unwrap().get_id();
    crate::object::registry::OBJECT_REGISTRY.register_object(id, object);
    Decoy(id)
}

#[test]
fn bound_machine_and_state_keep_same_id_objects_distinct() {
    let _serial = crate::test_sync::lock();
    let first = object(0x7AF10001, 10.0);
    let second = object(0x7AF10001, 90.0);
    let _decoy = publish_decoy(&second);
    let first_machine = StateMachine::new(Some(Arc::downgrade(&first)), "first");
    let second_machine = StateMachine::new(Some(Arc::downgrade(&second)), "second");
    let first_state = State::new(&first_machine, "first child");
    let second_state = State::new(&second_machine, "second child");
    assert!(Arc::ptr_eq(&first_machine.get_owner().unwrap(), &first));
    assert!(Arc::ptr_eq(&second_machine.get_owner().unwrap(), &second));
    assert!(Arc::ptr_eq(
        &first_state.get_machine_owner().unwrap(),
        &first
    ));
    assert!(Arc::ptr_eq(
        &second_state.get_machine_owner().unwrap(),
        &second
    ));
    first_state
        .get_machine_owner()
        .unwrap()
        .write()
        .unwrap()
        .set_position(&Coord3D::new(25.0, 0.0, 0.0))
        .unwrap();
    assert_eq!(first.read().unwrap().get_position().x, 25.0);
    assert_eq!(second.read().unwrap().get_position().x, 90.0);
}

#[test]
fn expired_bound_owner_never_redirects_to_reused_id() {
    let _serial = crate::test_sync::lock();
    let first = object(0x7AF10002, 10.0);
    let replacement = object(0x7AF10002, 90.0);
    let _decoy = publish_decoy(&replacement);
    let machine = StateMachine::new(Some(Arc::downgrade(&first)), "expired");
    let state = State::new(&machine, "expired child");
    assert_eq!(
        Arc::strong_count(&first),
        1,
        "machines must not retain Objects"
    );
    drop(first);
    assert!(machine.get_owner().is_none());
    assert!(state.get_machine_owner().is_none());
}

#[test]
fn bound_owner_lookup_does_not_acquire_global_game_logic() {
    #[cfg(not(target_arch = "wasm32"))]
    if let crate::test_process::TestProcess::ParentVerified = crate::test_process::run_bounded(
        "state_machine::object_owner_tests::bound_owner_lookup_does_not_acquire_global_game_logic",
        "GENERALS_STATE_OBJECT_OWNER_CHILD",
    ) {
        return;
    }
    let _serial = crate::test_sync::lock();
    let first = object(0x7AF10003, 10.0);
    let machine = StateMachine::new(Some(Arc::downgrade(&first)), "borrowed owner");
    let state = State::new(&machine, "borrowed child");
    // The actual global guard is held: owner lookup must still resolve immediately.
    let _logic = crate::system::game_logic::lock_game_logic().unwrap();
    assert!(Arc::ptr_eq(&machine.get_owner().unwrap(), &first));
    assert!(Arc::ptr_eq(&state.get_machine_owner().unwrap(), &first));
}

#[test]
fn idle_state_entry_reset_and_restore_keep_exact_owner() {
    let _serial = crate::test_sync::lock();
    let _rng = PreserveRng(game_engine::common::random_value::get_game_logic_random_seed_state());
    let first = object(0x7AF10004, 10.0);
    let second = object(0x7AF10004, 90.0);
    let _decoy = publish_decoy(&second);
    let mut machine = idle_machine(&first);
    // The actual native Idle callback is entered by the generic driving machine.
    // This is a child callback identity witness, not factory/scheduler activation.
    assert!(machine.get_current_state_id().is_none());
    assert!(!first.read().unwrap().ai_pending_reset_mood);
    assert!(!second.read().unwrap().ai_pending_reset_mood);
    assert_eq!(machine.init_default_state(), StateReturnType::Continue);
    assert!(first.read().unwrap().ai_pending_reset_mood);
    assert!(!second.read().unwrap().ai_pending_reset_mood);
    first.write().unwrap().ai_pending_reset_mood = false;
    assert_eq!(machine.reset_to_default_state(), StateReturnType::Continue);
    assert!(first.read().unwrap().ai_pending_reset_mood);
    assert!(!second.read().unwrap().ai_pending_reset_mood);

    let mut bytes = Cursor::new(Vec::new());
    machine.xfer(&mut XferSave::new(&mut bytes, 1)).unwrap();
    let saved = bytes.into_inner();
    let mut restored = idle_machine(&second);
    restored
        .xfer(&mut XferLoad::new(Cursor::new(saved.clone()), 1))
        .unwrap();
    assert!(Arc::ptr_eq(&restored.get_owner().unwrap(), &second));
    let mut resaved = Cursor::new(Vec::new());
    restored.xfer(&mut XferSave::new(&mut resaved, 1)).unwrap();
    assert_eq!(
        resaved.into_inner(),
        saved,
        "owner references do not enter snapshot bytes"
    );
    assert!(
        !second.read().unwrap().ai_pending_reset_mood,
        "load must not enter Idle"
    );
    assert_eq!(restored.reset_to_default_state(), StateReturnType::Continue);
    assert!(second.read().unwrap().ai_pending_reset_mood);
    assert!(Arc::ptr_eq(&machine.get_owner().unwrap(), &first));
}

#[test]
fn id_only_compatibility_constructor_remains_explicit() {
    let _serial = crate::test_sync::lock();
    let owner = object(0x7AF10005, 15.0);
    let _decoy = publish_decoy(&owner);
    let machine = StateMachine::new_with_owner_id(0x7AF10005, "legacy ID adapter");
    let state = State::new(&machine, "legacy child");
    assert!(Arc::ptr_eq(&machine.get_owner().unwrap(), &owner));
    assert!(Arc::ptr_eq(&state.get_machine_owner().unwrap(), &owner));
}
