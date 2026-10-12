use super::*;
use crate::common::Coord3D;
use crate::helpers::{TheGameLogic, TheThingFactory};
use crate::object::object_factory::{ObjectCreationFlags, ObjectFactory};
use game_engine::common::thing::module_factory::{get_module_factory, init_module_factory};
use game_engine::common::thing::thing_factory::{get_thing_factory, init_thing_factory};
use std::sync::{Arc, RwLock};

#[test]
fn registered_cpp_rappel_entry_resets_physics_with_same_owner_goal() {
    #[cfg(not(target_arch = "wasm32"))]
    if let crate::test_process::TestProcess::ParentVerified = crate::test_process::run_bounded(
        "ai::states::rappel::reset_tests::registered_cpp_rappel_entry_resets_physics_with_same_owner_goal",
        "GENERALS_CPP_RAPPEL_RESET_CHILD",
    ) {
        return;
    }

    if get_thing_factory().unwrap().is_none() {
        init_thing_factory().unwrap();
    }
    if get_module_factory().unwrap().is_none() {
        init_module_factory().unwrap();
    }
    crate::contain_module_overrides::ensure_module_overrides_installed().unwrap();
    assert_eq!(
        get_thing_factory().unwrap().as_mut().unwrap().load_ini_text(
            "Object RappelCppEntryResetRegression\n KindOf = INFANTRY CAN_RAPPEL\n Behavior = AIUpdateInterface RappelAI\n End\n Behavior = PhysicsBehavior RappelPhysics\n Mass = 1\n End\nEnd\n"
        ),
        1
    );
    let template = TheThingFactory::find_template("RappelCppEntryResetRegression").unwrap();
    assert!(
        template
            .as_ref()
            .get_behavior_module_info()
            .iter()
            .any(|entry| entry.name.as_str() == "PhysicsBehavior")
    );

    let mut factory = ObjectFactory::new();
    let id = factory
        .create_object(
            "RappelCppEntryResetRegression",
            Coord3D::new(20.0, 30.0, 50.0),
            None,
            ObjectCreationFlags::empty(),
        )
        .unwrap();
    let unit = factory.get_object(id).unwrap();
    let owner = unit.get_base_object().unwrap();
    assert!(Arc::ptr_eq(
        &owner,
        &TheGameLogic::find_object_by_id(id).expect("factory-admitted owner")
    ));

    let (physics, transform, proxy) = {
        let mut owner_guard = owner.write().unwrap();
        owner_guard.ai_fire_desired_speed = 12.0;
        let physics = owner_guard
            .get_physics()
            .expect("installed PhysicsBehavior");
        let mut updates = owner_guard.named_update_registrations("PhysicsBehavior");
        assert_eq!(updates.len(), 1, "one real authored Physics proxy");
        (
            physics,
            owner_guard.get_transform_matrix(),
            updates.remove(0),
        )
    };
    physics.set_velocity(&Coord3D::new(3.0, 4.0, -5.0));

    let mut machine = AIStateMachine::new(Arc::downgrade(&owner), "RappelResetRegression");
    // This deliberate self-target verifies the target lookup also happens
    // after the owner write guard is released.
    machine.base.set_goal_object_by_id(Some(id));
    machine
        .base
        .set_goal_position(Coord3D::new(20.0, 30.0, 50.0));
    let frame = TheGameLogic::get_frame();
    assert_eq!(
        machine.set_state(AIStateType::RappelInto.into()),
        StateReturnType::Continue,
        "registered CPP state entry completes synchronously"
    );
    assert_eq!(
        machine.get_current_state_id(),
        Some(AIStateType::RappelInto.into())
    );

    let owner_guard = owner.read().unwrap();
    assert_eq!(physics.get_velocity(), Coord3D::ZERO);
    assert_eq!(owner_guard.get_transform_matrix(), transform);
    assert!(owner_guard.ai_pending_rappel);
    assert_eq!(owner_guard.ai_pending_rappel_obj, Some(id));
    assert!(owner_guard.ai_pending_rappel_pos.is_some());
    assert!(
        owner_guard
            .get_drawable()
            .unwrap()
            .read()
            .unwrap()
            .get_model_conditions()
            .contains(crate::common::ModelConditionFlags::RAPPELLING)
    );
    drop(owner_guard);

    let logic = crate::system::game_logic::get_game_logic();
    let logic = logic.lock().unwrap();
    assert_eq!(
        logic.sleepy_entry_for(&proxy),
        Some((id, frame.saturating_add(1))),
        "CPP reset synchronously reschedules the installed proxy"
    );
}
