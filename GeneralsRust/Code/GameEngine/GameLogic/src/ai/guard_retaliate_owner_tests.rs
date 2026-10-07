//! Regressions for the borrowed retaliate machine callback path.
use super::*;
use crate::common::{Coord3D, ObjectID};
use crate::modules::AIUpdateInterface;
use crate::object::Object;
use crate::state_machine::StateReturnType;
use std::sync::{Arc, Mutex, RwLock};

#[derive(Debug, Default)]
struct RetaliateTestAI {
    commands: Vec<crate::ai::AiCommandParams>,
    crate_id: ObjectID,
    guard_clears: usize,
}
impl AIUpdateInterface for RetaliateTestAI {
    fn update(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }
    fn clear_guard_target_type(&mut self) {
        self.guard_clears += 1;
    }
    fn is_moving(&self) -> bool {
        false
    }
    fn is_idle(&self) -> bool {
        true
    }
    fn set_movement_target(&mut self, _target: &Coord3D) -> Result<(), String> {
        Ok(())
    }
    fn get_crate_id(&self) -> ObjectID {
        self.crate_id
    }
    fn execute_command(
        &mut self,
        command: &crate::ai::AiCommandParams,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.commands.push(command.clone());
        Ok(())
    }
}

// This tiny facade is also the seam for the strict OLD replay: the assertion
// bodies stay identical while the baseline adapter uses the old public calls.
trait RetaliateTestAccess {
    fn current_id(&self) -> Option<u32>;
    fn goal_id(&self) -> ObjectID;
    fn owner_is(&self, owner: &Arc<RwLock<Object>>) -> bool;
    fn init_loaned(&mut self, ai: &mut dyn AIUpdateInterface) -> StateReturnType;
    fn set_loaned(
        &mut self,
        state: GuardRetaliateStateType,
        ai: &mut dyn AIUpdateInterface,
    ) -> StateReturnType;
    fn update_loaned(&mut self, ai: &mut dyn AIUpdateInterface) -> StateReturnType;
}
impl RetaliateTestAccess for AIGuardRetaliateMachine {
    fn current_id(&self) -> Option<u32> {
        self.state_machine.get_current_state_id()
    }
    fn goal_id(&self) -> ObjectID {
        self.state_machine.get_goal_object_id()
    }
    fn owner_is(&self, owner: &Arc<RwLock<Object>>) -> bool {
        self.state_machine
            .get_owner()
            .is_some_and(|actual| Arc::ptr_eq(&actual, owner))
    }
    fn init_loaned(&mut self, ai: &mut dyn AIUpdateInterface) -> StateReturnType {
        self.init_default_state_with_ai(ai)
    }
    fn set_loaned(
        &mut self,
        state: GuardRetaliateStateType,
        ai: &mut dyn AIUpdateInterface,
    ) -> StateReturnType {
        self.set_state_with_ai(state, ai)
    }
    fn update_loaned(&mut self, ai: &mut dyn AIUpdateInterface) -> StateReturnType {
        self.update_with_ai(ai)
    }
}

trait OuterTestUpdate {
    fn loaned_update(&mut self, ai: &mut dyn AIUpdateInterface) -> StateReturnType;
}
impl OuterTestUpdate for AIGuardRetaliateOuterState {
    fn loaned_update(&mut self, ai: &mut dyn AIUpdateInterface) -> StateReturnType {
        self.classic_update_with_ai(Some(ai))
    }
}

fn child(name: &str) -> bool {
    #[cfg(not(target_arch = "wasm32"))]
    {
        matches!(
            crate::test_process::run_bounded(
                name.strip_prefix("gamelogic::").unwrap_or(name),
                "GENERALS_RETALIATE_LOAN_CHILD"
            ),
            crate::test_process::TestProcess::Child
        )
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = name;
        true
    }
}

fn owner_with_ai(id: ObjectID, ai: Arc<Mutex<RetaliateTestAI>>) -> Arc<RwLock<Object>> {
    let owner = Arc::new(RwLock::new(Object::new_test(id, 100.0)));
    let interface: Arc<Mutex<dyn AIUpdateInterface>> = ai;
    owner
        .write()
        .unwrap()
        .set_ai_update_interface(Some(interface));
    owner
}

struct RegisteredPair(ObjectID, ObjectID);
impl Drop for RegisteredPair {
    fn drop(&mut self) {
        crate::ai::object_registry::unregister_legacy_object(self.0);
        crate::ai::object_registry::unregister_legacy_object(self.1);
        crate::object::registry::OBJECT_REGISTRY.unregister_object(self.0);
        crate::object::registry::OBJECT_REGISTRY.unregister_object(self.1);
    }
}

#[test]
fn constructor_is_inert_and_same_id_owners_keep_their_borrowed_callback() {
    if !child(concat!(
        module_path!(),
        "::constructor_is_inert_and_same_id_owners_keep_their_borrowed_callback"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    let id = 0x7A4E_1201;
    let ai_a = Arc::new(Mutex::new(RetaliateTestAI::default()));
    let ai_b = Arc::new(Mutex::new(RetaliateTestAI::default()));
    let owner_a = owner_with_ai(id, ai_a.clone());
    let owner_b = owner_with_ai(id, ai_b.clone());
    let mut machine_a = AIGuardRetaliateMachine::new(Arc::downgrade(&owner_a));
    let mut machine_b = AIGuardRetaliateMachine::new(Arc::downgrade(&owner_b));

    assert_eq!(machine_a.current_id(), None);
    assert_eq!(machine_b.current_id(), None);
    assert!(machine_a.owner_is(&owner_a));
    assert!(machine_b.owner_is(&owner_b));

    // Keep each installed handle locked across the actual callback. The
    // borrowed path must use that loan instead of reacquiring the same mutex.
    let mut ai_a = ai_a.lock().unwrap();
    assert_eq!(machine_a.init_loaned(&mut *ai_a), StateReturnType::Continue);
    assert_eq!(
        machine_a.current_id(),
        Some(GuardRetaliateStateType::Return as u32)
    );
    assert_eq!(ai_a.commands.len(), 1);
    drop(ai_a);

    let mut ai_b = ai_b.lock().unwrap();
    machine_b.set_target_position_to_guard(&Coord3D::new(13.0, 17.0, 0.0));
    assert_eq!(machine_b.init_loaned(&mut *ai_b), StateReturnType::Continue);
    assert_eq!(
        machine_b.current_id(),
        Some(GuardRetaliateStateType::Return as u32)
    );
    assert_eq!(ai_b.commands.len(), 1);
    assert_eq!(ai_b.commands[0].pos, Coord3D::new(13.0, 17.0, 0.0));
}

#[test]
fn idle_crate_transition_enters_pickup_synchronously_with_the_borrowed_ai() {
    if !child(concat!(
        module_path!(),
        "::idle_crate_transition_enters_pickup_synchronously_with_the_borrowed_ai"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    let id = 0x7A4E_1202;
    let crate_id = id + 1;
    let ai = Arc::new(Mutex::new(RetaliateTestAI {
        crate_id,
        ..Default::default()
    }));
    let owner = owner_with_ai(id, ai.clone());
    owner.write().unwrap().ai_fire_crate_id = crate_id;
    let crate_object = Arc::new(RwLock::new(Object::new_test(crate_id, 100.0)));
    crate::object::registry::OBJECT_REGISTRY.register_object(id, &owner);
    crate::object::registry::OBJECT_REGISTRY.register_object(crate_id, &crate_object);
    crate::ai::object_registry::register_legacy_object(&owner);
    crate::ai::object_registry::register_legacy_object(&crate_object);
    let _registered = RegisteredPair(id, crate_id);

    let mut machine = AIGuardRetaliateMachine::new(Arc::downgrade(&owner));
    let mut random = game_engine::common::random_value::RandomState::default();
    random.set_seed_words([0; 6]);
    game_engine::common::random_value::with_logic_rng_owner(&mut random, || {
        let mut ai = ai.lock().unwrap();
        assert_eq!(
            machine.set_loaned(GuardRetaliateStateType::Idle, &mut *ai),
            StateReturnType::Continue
        );
        assert_eq!(
            machine.update_loaned(&mut *ai),
            StateReturnType::Continue,
            "C++ overrides Idle's Sleep when the callback synchronously changes state"
        );
        assert_eq!(
            machine.current_id(),
            Some(GuardRetaliateStateType::GetCrate as u32)
        );
        assert_eq!(machine.goal_id(), crate_id);
        assert!(
            ai.commands.is_empty(),
            "crate pickup enters synchronously without issuing a movement command yet"
        );
    });
}

#[test]
fn native_parent_retaliation_forwards_held_ai_through_enter_update_and_exit() {
    if !child(concat!(
        module_path!(),
        "::native_parent_retaliation_forwards_held_ai_through_enter_update_and_exit"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    let id = 0x7A4E_1210;
    let actual_ai = Arc::new(Mutex::new(RetaliateTestAI::default()));
    let shadow_ai = Arc::new(Mutex::new(RetaliateTestAI::default()));
    let owner = owner_with_ai(id, actual_ai.clone());
    let shadow = owner_with_ai(id, shadow_ai.clone());
    crate::object::registry::OBJECT_REGISTRY.register_object(id, &shadow);
    let _registered = RegisteredPair(id, id + 1);
    let mut machine =
        crate::ai::states::AIStateMachine::new(Arc::downgrade(&owner), "native retaliation loan");
    machine.set_goal_position(Coord3D::new(23.0, 31.0, 0.0));
    let mut loan = actual_ai.lock().unwrap();
    assert_eq!(
        machine.set_state_with_ai(
            crate::ai::states::AIStateType::GuardRetaliate as u32,
            &mut *loan
        ),
        StateReturnType::Continue
    );
    assert_eq!(
        machine.get_current_state_id(),
        Some(crate::ai::states::AIStateType::GuardRetaliate as u32)
    );
    assert_eq!(loan.commands.len(), 1);
    assert_eq!(loan.commands[0].pos, Coord3D::new(23.0, 31.0, 0.0));
    let _ = machine.base.update_with_ai(&mut *loan);
    // Leave via the native registered CppState adapter. It must clear the
    // driving AI's guard target while its installed handle remains held.
    let _ = machine.set_state_with_ai(crate::state_machine::MACHINE_DONE_STATE_ID, &mut *loan);
    assert_eq!(loan.guard_clears, 1);
    assert_eq!(machine.get_current_state_id(), None);
    assert_eq!(shadow_ai.lock().unwrap().guard_clears, 0);
    assert!(shadow_ai.lock().unwrap().commands.is_empty());
}

#[test]
fn borrowed_outer_update_checks_exit_predicate_before_child_effects() {
    if !child(concat!(
        module_path!(),
        "::borrowed_outer_update_checks_exit_predicate_before_child_effects"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    let id = 0x7A4E_1220;
    let ai = Arc::new(Mutex::new(RetaliateTestAI::default()));
    let owner = owner_with_ai(id, ai.clone());
    crate::object::registry::OBJECT_REGISTRY.register_object(id, &owner);
    let _registered = RegisteredPair(id, id + 1);
    let core = StateMachine::new(Some(Arc::downgrade(&owner)), "outer exit order");
    let mut state = AIGuardRetaliateOuterState::new(&core);
    state
        .exit_conditions
        .set_conditions(guard_retaliate_exit_conditions::ATTACK_EXIT_IF_NO_UNIT_FOUND);
    state.attack_machine = Some(AttackStateMachine::new(
        Arc::downgrade(&owner),
        "outer attack",
        false,
        true,
        false,
    ));
    let mut loan = ai.lock().unwrap();
    assert_eq!(state.loaned_update(&mut *loan), StateReturnType::Success);
    assert!(
        loan.commands.is_empty(),
        "missing target exits before the attack child issues commands"
    );
}
