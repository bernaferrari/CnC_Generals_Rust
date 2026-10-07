//! C++ AIStates.cpp:1370–1377: the actual parent lock gates target checks.
use super::*;
use crate::ai::states::AIIdleState;

struct RestoreRng([u32; 6]);
impl Drop for RestoreRng {
    fn drop(&mut self) {
        game_engine::common::random_value::set_game_logic_random_seed_state(self.0);
    }
}
struct Unpublish(u32);
impl Drop for Unpublish {
    fn drop(&mut self) {
        crate::object::registry::OBJECT_REGISTRY.unregister_object(self.0);
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
fn exercise(classic: bool, held_ai: bool, locked: bool) {
    let _serial = crate::test_sync::lock();
    let _rng = RestoreRng(game_engine::common::random_value::get_game_logic_random_seed_state());
    let id = 0x7af10102;
    let actual = Arc::new(RwLock::new(Object::new_test(id, 100.0)));
    let decoy = Arc::new(RwLock::new(Object::new_test(id, 100.0)));
    crate::object::registry::OBJECT_REGISTRY.register_object(id, &decoy);
    let _unpublish = Unpublish(id);
    actual.write().unwrap().ai_fire_crate_id = 123;
    decoy.write().unwrap().ai_fire_crate_id = 456;
    let mut core = StateMachine::new(Some(Arc::downgrade(&actual)), "idle locked parent");
    let state = AIIdleState::new(&core, true);
    if classic {
        crate::state_machine::cpp_state::register_cpp_state(&mut core, 5, state, None, None, &[]);
    } else {
        core.define_state(5, Box::new(state), None, None, None);
    }
    assert_eq!(core.init_default_state(), StateReturnType::Continue);
    if locked {
        core.lock();
    }
    let result = if held_ai {
        core.update_with_ai(&mut TestAI)
    } else {
        core.update_with_owner(&mut ())
    };
    if locked {
        assert!(
            matches!(result, StateReturnType::Sleep(60..=120)),
            "locked C++ Idle sleeps before crate scan: {result:?}"
        );
        assert_eq!(actual.read().unwrap().ai_pending_move_crate, None);
    } else {
        assert_eq!(result, StateReturnType::Continue);
        assert_eq!(actual.read().unwrap().ai_pending_move_crate, Some(123));
    }
    assert_eq!(decoy.read().unwrap().ai_pending_move_crate, None);
    assert_eq!(core.get_current_state_id(), Some(5));
    assert_eq!(core.is_locked(), locked);
}
#[test]
fn registered_classic_idle_reads_locked_parent_with_held_ai() {
    exercise(true, true, true);
}
#[test]
fn registered_classic_idle_reads_locked_parent_with_owner_only() {
    exercise(true, false, true);
}
#[test]
fn direct_idle_reads_locked_parent_with_owner_only() {
    exercise(false, false, true);
}
#[test]
fn unlocked_classic_idle_still_requests_exact_owner_crate() {
    exercise(true, true, false);
}
