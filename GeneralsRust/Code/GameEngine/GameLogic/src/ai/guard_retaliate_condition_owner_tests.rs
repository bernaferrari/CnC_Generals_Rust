use super::*;
use crate::ai::object_registry::{register_legacy_object, unregister_legacy_object};

// The same assertions run against the old shared cell and the owned value.
// These adapters are fixture access only, never production dispatch.
trait TestConditionAccess<C> {
    fn edit(&mut self, f: impl FnOnce(&mut C));
    fn inspect<R>(&self, f: impl FnOnce(&C) -> R) -> R;
}
impl<C> TestConditionAccess<C> for Arc<Mutex<C>> {
    fn edit(&mut self, f: impl FnOnce(&mut C)) {
        f(&mut self.lock().unwrap());
    }
    fn inspect<R>(&self, f: impl FnOnce(&C) -> R) -> R {
        f(&self.lock().unwrap())
    }
}
impl TestConditionAccess<GuardRetaliateExitConditions> for GuardRetaliateExitConditions {
    fn edit(&mut self, f: impl FnOnce(&mut GuardRetaliateExitConditions)) {
        f(self);
    }
    fn inspect<R>(&self, f: impl FnOnce(&GuardRetaliateExitConditions) -> R) -> R {
        f(self)
    }
}

// Native guard entry consumes the existing Common RNG. Restore it after
// fixture objects are dropped so unrelated tests see their original stream.
struct PreserveRng([u32; 6]);
impl PreserveRng {
    fn new() -> Self {
        Self(game_engine::common::random_value::get_game_logic_random_seed_state())
    }
}
impl Drop for PreserveRng {
    fn drop(&mut self) {
        game_engine::common::random_value::set_game_logic_random_seed_state(self.0);
    }
}

struct Fixture {
    owner: Arc<RwLock<Object>>,
    target: Arc<RwLock<Object>>,
    id: ObjectID,
    // Declared last: restoration runs after the object handles are dropped.
    _rng: PreserveRng,
}
impl Fixture {
    fn new(id: ObjectID) -> Self {
        let rng = PreserveRng::new();
        let owner = Arc::new(RwLock::new(Object::new_test(id, 100.0)));
        let target = Arc::new(RwLock::new(Object::new_test(id + 1, 100.0)));
        crate::object::registry::OBJECT_REGISTRY.register_object(id, &owner);
        crate::object::registry::OBJECT_REGISTRY.register_object(id + 1, &target);
        register_legacy_object(&owner);
        register_legacy_object(&target);
        Self {
            owner,
            target,
            id,
            _rng: rng,
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        unregister_legacy_object(self.id);
        unregister_legacy_object(self.id + 1);
        crate::object::registry::OBJECT_REGISTRY.unregister_object(self.id);
        crate::object::registry::OBJECT_REGISTRY.unregister_object(self.id + 1);
    }
}

#[test]
fn aiguardretaliateinnerstate_borrows_conditions_without_mutex() {
    let fixture = Fixture::new(920000);
    let mut parent = AIGuardRetaliateMachine::new(Arc::downgrade(&fixture.owner));
    parent.set_nemesis_id(fixture.id + 1);
    parent.set_target_position_to_guard(&Coord3D::new(12.0, 34.0, 0.0));
    let mut state = AIGuardRetaliateInnerState::new(&parent.state_machine);
    let _ = state.classic_on_enter(&mut parent);
    assert!(
        state.attack_machine.is_some(),
        "actual parent entry must build the child"
    );
    assert_eq!(
        state.attack_machine.as_ref().unwrap().get_goal_object_id(),
        fixture.id + 1
    );
    state.exit_conditions.edit(|c| {
        c.set_conditions(guard_retaliate_exit_conditions::ATTACK_EXIT_IF_NO_UNIT_FOUND);
    });
    state.attack_machine.as_mut().unwrap().set_goal_object(None);
    assert_eq!(
        state.classic_update(),
        StateReturnType::Success,
        "the child must see the parent's latest condition immediately"
    );
    let acquisitions = state.exit_conditions.inspect(|c| c.test_exit_acquisitions);
    state.classic_on_exit(StateExitType::Reset);
    assert!(state.attack_machine.is_none());
    assert!(!state.is_attack());
    assert_eq!(
        state.exit_conditions.inspect(|c| c.test_exit_acquisitions),
        acquisitions
    );
    parent.set_target_position_to_guard(&Coord3D::new(56.0, 78.0, 0.0));
    let _ = state.classic_on_enter(&mut parent);
    assert!(
        state.attack_machine.is_some(),
        "re-entry must rebuild the child"
    );
    assert_eq!(
        state.attack_machine.as_ref().unwrap().get_goal_object_id(),
        fixture.id + 1
    );
    assert_eq!(
        acquisitions, 0,
        "synchronous child exit evaluation must not acquire a conditions mutex"
    );
}

#[test]
fn aiguardretaliateouterstate_borrows_conditions_without_mutex() {
    let fixture = Fixture::new(920010);
    let mut parent = AIGuardRetaliateMachine::new(Arc::downgrade(&fixture.owner));
    parent.set_nemesis_id(fixture.id + 1);
    parent.set_target_position_to_guard(&Coord3D::new(12.0, 34.0, 0.0));
    let mut state = AIGuardRetaliateOuterState::new(&parent.state_machine);
    let _ = state.classic_on_enter(&mut parent);
    assert!(
        state.attack_machine.is_some(),
        "actual parent entry must build the child"
    );
    assert_eq!(
        state.attack_machine.as_ref().unwrap().get_goal_object_id(),
        fixture.id + 1
    );
    state.exit_conditions.edit(|c| {
        c.set_conditions(guard_retaliate_exit_conditions::ATTACK_EXIT_IF_NO_UNIT_FOUND);
    });
    state.attack_machine.as_mut().unwrap().set_goal_object(None);
    assert_eq!(
        state.classic_update(),
        StateReturnType::Success,
        "the child must see the parent's latest condition immediately"
    );
    let acquisitions = state.exit_conditions.inspect(|c| c.test_exit_acquisitions);
    state.classic_on_exit(StateExitType::Reset);
    assert!(state.attack_machine.is_none());
    assert!(!state.is_attack());
    assert_eq!(
        state.exit_conditions.inspect(|c| c.test_exit_acquisitions),
        acquisitions
    );
    parent.set_target_position_to_guard(&Coord3D::new(56.0, 78.0, 0.0));
    let _ = state.classic_on_enter(&mut parent);
    assert!(
        state.attack_machine.is_some(),
        "re-entry must rebuild the child"
    );
    assert_eq!(
        state.attack_machine.as_ref().unwrap().get_goal_object_id(),
        fixture.id + 1
    );
    assert_eq!(
        acquisitions, 0,
        "synchronous child exit evaluation must not acquire a conditions mutex"
    );
}

#[test]
fn aiguardretaliateattackaggressorstate_borrows_conditions_without_mutex() {
    let fixture = Fixture::new(920020);
    let mut parent = AIGuardRetaliateMachine::new(Arc::downgrade(&fixture.owner));
    parent.set_nemesis_id(fixture.id + 1);
    parent.set_target_position_to_guard(&Coord3D::new(12.0, 34.0, 0.0));
    let mut state = AIGuardRetaliateAttackAggressorState::new(&parent.state_machine);
    let _ = state.classic_on_enter(&mut parent);
    assert!(
        state.attack_machine.is_some(),
        "actual parent entry must build the child"
    );
    assert_eq!(
        state.attack_machine.as_ref().unwrap().get_goal_object_id(),
        fixture.id + 1
    );
    state.exit_conditions.edit(|c| {
        c.set_conditions(guard_retaliate_exit_conditions::ATTACK_EXIT_IF_NO_UNIT_FOUND);
    });
    state.attack_machine.as_mut().unwrap().set_goal_object(None);
    assert_eq!(
        state.classic_update(),
        StateReturnType::Success,
        "the child must see the parent's latest condition immediately"
    );
    let acquisitions = state.exit_conditions.inspect(|c| c.test_exit_acquisitions);
    state.classic_on_exit(StateExitType::Reset);
    assert!(state.attack_machine.is_none());
    assert!(!state.is_attack());
    assert_eq!(
        state.exit_conditions.inspect(|c| c.test_exit_acquisitions),
        acquisitions
    );
    parent.set_target_position_to_guard(&Coord3D::new(56.0, 78.0, 0.0));
    let _ = state.classic_on_enter(&mut parent);
    assert!(
        state.attack_machine.is_some(),
        "re-entry must rebuild the child"
    );
    assert_eq!(
        state.attack_machine.as_ref().unwrap().get_goal_object_id(),
        fixture.id + 1
    );
    assert_eq!(
        acquisitions, 0,
        "synchronous child exit evaluation must not acquire a conditions mutex"
    );
}

#[test]
fn condition_values_remain_distinct_for_equal_owner_ids_and_absent_children() {
    let _rng = PreserveRng::new();
    let owner_a = Arc::new(RwLock::new(Object::new_test(929_999, 100.0)));
    let owner_b = Arc::new(RwLock::new(Object::new_test(929_999, 200.0)));
    let machine_a = StateMachine::new(Some(Arc::downgrade(&owner_a)), "guard-a");
    let machine_b = StateMachine::new(Some(Arc::downgrade(&owner_b)), "guard-b");
    let mut a = AIGuardRetaliateInnerState::new(&machine_a);
    let mut b = AIGuardRetaliateInnerState::new(&machine_b);
    assert!(Arc::ptr_eq(&a.get_machine_owner().unwrap(), &owner_a));
    assert!(Arc::ptr_eq(&b.get_machine_owner().unwrap(), &owner_b));
    assert!(a.attack_machine.is_none() && b.attack_machine.is_none());
    a.exit_conditions.edit(|c| c.set_attack_give_up_frame(37));
    b.exit_conditions.edit(|c| c.set_attack_give_up_frame(91));
    assert_eq!(a.exit_conditions.inspect(|c| c.attack_give_up_frame), 37);
    assert_eq!(b.exit_conditions.inspect(|c| c.attack_give_up_frame), 91);
    a.exit_conditions.edit(|c| c.set_attack_give_up_frame(123));
    assert_eq!(a.exit_conditions.inspect(|c| c.attack_give_up_frame), 123);
    assert_eq!(b.exit_conditions.inspect(|c| c.attack_give_up_frame), 91);
    assert!(a.attack_machine.is_none() && b.attack_machine.is_none());
    drop(owner_a);
    assert!(a.get_machine_owner().is_err());
}
