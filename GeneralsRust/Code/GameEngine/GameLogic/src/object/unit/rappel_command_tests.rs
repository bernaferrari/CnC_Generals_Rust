//! Factory-admitted cached UnitAI commands. These exercise synchronous entry
//! and subsequent family updates, not a normal scheduler or retail rope flow.

use crate::ai::{AiCommandParams, AiCommandType, CommandSourceType};
use crate::common::{Coord3D, ModelConditionFlags};
use crate::helpers::{TheGameLogic, TheThingFactory};
use crate::modules::{AIUpdateInterface, FAST_AS_POSSIBLE, PhysicsBehaviorExt};
use crate::object::Object;
use crate::object::object_factory::{ObjectCreationFlags, ObjectFactory};
use game_engine::common::thing::module_factory::{get_module_factory, init_module_factory};
use game_engine::common::thing::thing_factory::{get_thing_factory, init_thing_factory};
use std::sync::{Arc, Mutex, RwLock};

fn child(name: &str) -> bool {
    #[cfg(not(target_arch = "wasm32"))]
    {
        matches!(
            crate::test_process::run_bounded(
                name.strip_prefix("gamelogic::").unwrap_or(name),
                "GENERALS_UNITAI_RAPPEL_COMMAND_CHILD",
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

struct Fixture {
    // The fresh bounded child retains the actual factory-owned Unit and exact
    // Object admission. No separate UNIT_REGISTRY entry or fake AI is installed.
    _factory: ObjectFactory,
    owner: Arc<RwLock<Object>>,
    ai: Arc<Mutex<dyn AIUpdateInterface>>,
}

impl Fixture {
    fn new() -> Self {
        if get_thing_factory().unwrap().is_none() {
            init_thing_factory().unwrap();
        }
        if get_module_factory().unwrap().is_none() {
            init_module_factory().unwrap();
        }
        crate::contain_module_overrides::ensure_module_overrides_installed().unwrap();
        assert_eq!(
            get_thing_factory().unwrap().as_mut().unwrap().load_ini_text(
                "Object RappelCommandPassenger\n KindOf = INFANTRY CAN_RAPPEL\n Behavior = AIUpdateInterface RappelAI\n End\n Behavior = PhysicsBehavior RappelPhysics\n Mass = 1\n End\nEnd\n"
            ),
            1
        );
        let template = TheThingFactory::find_template("RappelCommandPassenger").unwrap();
        assert!(
            template
                .as_ref()
                .get_behavior_module_info()
                .iter()
                .any(|entry| {
                    entry.name.as_str() == "AIUpdateInterface"
                && entry.data.as_ref().downcast_ref::<
                    crate::object::update::ai_update_interface::AIUpdateModuleData,
                >().is_some()
                })
        );
        let mut factory = ObjectFactory::new();
        let id = factory
            .create_object(
                "RappelCommandPassenger",
                Coord3D::new(20.0, 30.0, 50.0),
                None,
                ObjectCreationFlags::empty(),
            )
            .unwrap();
        let unit = factory.get_object(id).unwrap();
        assert!(unit.is_unit());
        let owner = unit.get_base_object().unwrap();
        assert!(Arc::ptr_eq(
            &owner,
            &TheGameLogic::find_object_by_id(id).unwrap()
        ));
        assert!(
            super::registry::get_unit_arc(id).is_none(),
            "no fabricated unit admission"
        );
        let ai = owner.read().unwrap().get_ai_update_interface().unwrap();
        assert!(owner.read().unwrap().get_physics().is_some());
        assert!(owner.read().unwrap().get_drawable().is_some());
        Self {
            _factory: factory,
            owner,
            ai,
        }
    }

    fn command(&self, speed: f32) {
        self.command_for_target(speed, None);
    }

    fn command_for_target(&self, speed: f32, target: Option<crate::common::ObjectID>) {
        let mut ai = self.ai.lock().unwrap();
        ai.set_desired_speed(speed);
        let mut params =
            AiCommandParams::new(AiCommandType::RappelInto, CommandSourceType::FromScript);
        params.pos = *self.owner.read().unwrap().get_position();
        params.obj = target;
        ai.execute_command(&params)
            .expect("actual cached UnitAI RappelInto must enter synchronously");
        assert!(ai.is_in_rappel_state());
        assert_eq!(ai.get_current_command(), Some(AiCommandType::RappelInto));
        assert_eq!(ai.get_last_command_source(), CommandSourceType::FromScript);
        assert_eq!(ai.get_desired_speed(), speed);
    }
}

#[test]
fn admitted_cached_ai_rappel_entry_resets_physics_without_reentering_owner() {
    if !child(concat!(
        module_path!(),
        "::admitted_cached_ai_rappel_entry_resets_physics_without_reentering_owner"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    let fixture = Fixture::new();
    let (transform, physics) = {
        let owner = fixture.owner.read().unwrap();
        (owner.get_transform_matrix(), owner.get_physics().unwrap())
    };
    physics.set_velocity(&Coord3D::new(3.0, 4.0, -5.0));
    fixture.command(12.0);
    // C++ AIStates.cpp:481-514: entry sets RAPPELLING and resets dynamic
    // physics before returning; it does not teleport or consume desired speed.
    assert_eq!(physics.get_velocity(), Coord3D::ZERO);
    let owner = fixture.owner.read().unwrap();
    assert_eq!(owner.get_transform_matrix(), transform);
    assert!(
        owner
            .get_drawable()
            .unwrap()
            .read()
            .unwrap()
            .get_model_conditions()
            .contains(ModelConditionFlags::RAPPELLING)
    );
}

#[test]
fn admitted_cached_ai_airborne_rappel_update_keeps_state_and_scrubs_velocity() {
    if !child(concat!(
        module_path!(),
        "::admitted_cached_ai_airborne_rappel_update_keeps_state_and_scrubs_velocity"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    let fixture = Fixture::new();
    fixture.command(12.0);
    let (transform, physics) = {
        let owner = fixture.owner.read().unwrap();
        (owner.get_transform_matrix(), owner.get_physics().unwrap())
    };
    physics.set_velocity(&Coord3D::new(3.0, 4.0, -100.0));
    fixture.ai.lock().unwrap().update().unwrap();
    // C++ AIStates.cpp:519-553: clamp descent rate and retain airborne state;
    // pose integration belongs to PhysicsBehavior rather than this AI callback.
    assert!(fixture.ai.lock().unwrap().is_in_rappel_state());
    assert_eq!(physics.get_velocity(), Coord3D::new(0.0, 0.0, -12.0));
    assert_eq!(
        fixture.owner.read().unwrap().get_transform_matrix(),
        transform
    );
}

#[test]
fn admitted_cached_ai_ground_rappel_finishes_and_clears_exact_owner_condition() {
    if !child(concat!(
        module_path!(),
        "::admitted_cached_ai_ground_rappel_finishes_and_clears_exact_owner_condition"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    let fixture = Fixture::new();
    fixture.command(12.0);
    fixture
        .owner
        .write()
        .unwrap()
        .set_position(&Coord3D::new(20.0, 30.0, -1.0))
        .unwrap();
    fixture.ai.lock().unwrap().update().unwrap();
    // C++ AIStates.cpp:553-559,626-632: settle to destination then clear
    // RAPPELLING and restore default desired speed on the same admitted owner.
    let ai = fixture.ai.lock().unwrap();
    assert!(!ai.is_in_rappel_state());
    assert_ne!(ai.get_current_command(), Some(AiCommandType::RappelInto));
    assert_eq!(ai.get_desired_speed(), FAST_AS_POSSIBLE);
    let owner = fixture.owner.read().unwrap();
    assert!(
        !owner
            .get_drawable()
            .unwrap()
            .read()
            .unwrap()
            .get_model_conditions()
            .contains(ModelConditionFlags::RAPPELLING)
    );
    assert_eq!(owner.get_position().z, 0.0);
}

#[test]
fn admitted_cached_ai_self_target_rappel_uses_same_owner_without_nested_lock() {
    if !child(concat!(
        module_path!(),
        "::admitted_cached_ai_self_target_rappel_uses_same_owner_without_nested_lock"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    let fixture = Fixture::new();
    let id = fixture.owner.read().unwrap().get_id();
    // C++ AIStates.cpp:496-500 rejects a non-structure goal for building
    // treatment. The same admitted infantry remains a valid ground rappeller.
    fixture.command_for_target(12.0, Some(id));
    assert!(fixture.ai.lock().unwrap().is_in_rappel_state());
    let owner = fixture.owner.read().unwrap();
    assert_eq!(*owner.get_position(), Coord3D::new(20.0, 30.0, 50.0));
    assert!(
        owner
            .get_drawable()
            .unwrap()
            .read()
            .unwrap()
            .get_model_conditions()
            .contains(ModelConditionFlags::RAPPELLING)
    );
}
