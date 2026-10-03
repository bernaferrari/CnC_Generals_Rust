//! Actual factory admission and synchronous callback regressions. These drive
//! cached UnitAI directly; normal sleepy-scheduler integration is a separate contract.

use super::{ChinookAIUpdate, ChinookAIUpdateData, ChinookAIUpdateModuleData, ChinookFlightStatus};
use crate::common::{Coord3D, DisabledType, ModelConditionFlags};
use crate::helpers::{TheGameLogic, TheThingFactory};
use crate::modules::{
    AIUpdateInterface, ContainModuleInterfaceExt, ContainWant, SupplyTruckAIInterface,
};
use crate::object::Object;
use crate::object::drawable::Drawable;
use crate::object::object_factory::{ObjectCreationFlags, ObjectFactory};
use game_engine::common::thing::module_factory::{get_module_factory, init_module_factory};
use game_engine::common::thing::thing_factory::{get_thing_factory, init_thing_factory};
use std::sync::{Arc, Mutex, RwLock};

const NAME: &str = "CallbackChinook";

struct Fixture {
    // Keep the actual factory-owned Unit alive for the whole callback.
    factory: ObjectFactory,
    owner: Arc<RwLock<Object>>,
    ai: Arc<Mutex<dyn AIUpdateInterface>>,
    drawable: Arc<RwLock<Drawable>>,
}

impl Fixture {
    fn new() -> Self {
        assert!(TheGameLogic::find_object_by_id(1).is_none());
        if get_thing_factory().unwrap().is_none() {
            init_thing_factory().unwrap();
        }
        if get_module_factory().unwrap().is_none() {
            init_module_factory().unwrap();
        }
        crate::contain_module_overrides::ensure_module_overrides_installed().unwrap();
        assert_eq!(get_thing_factory().unwrap().as_mut().unwrap().load_ini_text(
            "Object CallbackChinook\n KindOf = AIRCRAFT\n Behavior = ChinookAIUpdate CallbackFlight\n MaxBoxes = 2\n NumRopes = 1\n End\n Behavior = TransportContain CallbackCargo\n Slots = 4\n End\nEnd\nObject CallbackRider\n KindOf = INFANTRY\nEnd\n"
        ), 2);
        let template = TheThingFactory::find_template(NAME).unwrap();
        let modules = template.as_ref().get_behavior_module_info();
        assert!(
            modules
                .iter()
                .any(|module| module.name.as_str() == "ChinookAIUpdate"),
            "authored module names: {:?}",
            modules
                .iter()
                .map(|module| module.name.as_str())
                .collect::<Vec<_>>()
        );
        assert!(
            modules
                .iter()
                .find(|module| module.name.as_str() == "ChinookAIUpdate")
                .unwrap()
                .data
                .as_ref()
                .downcast_ref::<ChinookAIUpdateModuleData>()
                .is_some(),
            "typed Chinook data"
        );
        let mut factory = ObjectFactory::new();
        let id = factory
            .create_object(
                NAME,
                Coord3D::new(20.0, 30.0, 50.0),
                None,
                ObjectCreationFlags::empty(),
            )
            .unwrap();
        let owner = factory.get_object(id).unwrap().get_base_object().unwrap();
        assert!(factory.get_object(id).unwrap().is_unit());
        assert!(Arc::ptr_eq(
            &owner,
            &TheGameLogic::find_object_by_id(id).unwrap()
        ));
        let (ai, drawable) = {
            let owner = owner.read().unwrap();
            assert!(
                owner.get_contain().is_some(),
                "authored TransportContain installed"
            );
            (
                owner.get_ai_update_interface().unwrap(),
                owner.get_drawable().unwrap(),
            )
        };
        assert_eq!(
            ai.lock()
                .unwrap()
                .get_supply_truck_ai_interface()
                .unwrap()
                .get_number_boxes(),
            0
        );
        Self {
            factory,
            owner,
            ai,
            drawable,
        }
    }

    fn carry_two_boxes(&self) {
        let mut ai = self.ai.lock().unwrap();
        let supply = ai.get_supply_truck_ai_interface_mut().unwrap();
        assert!(supply.gain_one_box(2));
        assert!(supply.gain_one_box(1));
        assert_eq!(supply.get_number_boxes(), 2);
        assert!(
            self.drawable
                .read()
                .unwrap()
                .get_model_conditions()
                .contains(ModelConditionFlags::CARRYING)
        );
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn child(name: &str) -> bool {
    matches!(
        crate::test_process::run_bounded(
            name.strip_prefix("gamelogic::").unwrap_or(name),
            "GENERALS_CHINOOK_CALLBACK_CHILD"
        ),
        crate::test_process::TestProcess::Child
    )
}

#[cfg(target_arch = "wasm32")]
fn child(_: &str) -> bool {
    true
}

/// C++ ChinookAIUpdate.cpp:1091 queries this AI's victim, without acquiring
/// the already executing AI interface a second time.
#[test]
fn cached_ai_frame_one_does_not_relock_itself() {
    if !child(concat!(
        module_path!(),
        "::cached_ai_frame_one_does_not_relock_itself"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    let fixture = Fixture::new();
    let _frame = crate::system::game_logic::enter_update_frame(1);
    assert_eq!(TheGameLogic::get_frame(), 1);
    fixture.ai.lock().unwrap().update().unwrap();
    assert_eq!(
        fixture
            .ai
            .lock()
            .unwrap()
            .get_supply_truck_ai_interface()
            .unwrap()
            .get_number_boxes(),
        0
    );
    assert!(
        !fixture
            .drawable
            .read()
            .unwrap()
            .get_model_conditions()
            .contains(ModelConditionFlags::CARRYING)
    );
}

/// C++ ChinookAIUpdate.cpp:1079 -> landing onEnter:222 discards every box
/// before the landing update returns, with immediate Drawable notification.
#[test]
fn cached_ai_auto_landing_discards_supplies_synchronously() {
    if !child(concat!(
        module_path!(),
        "::cached_ai_auto_landing_discards_supplies_synchronously"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    let mut fixture = Fixture::new();
    fixture.carry_two_boxes();
    let contain = fixture.owner.read().unwrap().get_contain().unwrap();
    let rider_id = fixture
        .factory
        .create_object(
            "CallbackRider",
            Coord3D::new(20.0, 30.0, 0.0),
            None,
            ObjectCreationFlags::NO_DRAWABLE | ObjectCreationFlags::NO_AI,
        )
        .unwrap();
    let rider = TheGameLogic::find_object_by_id(rider_id).unwrap();
    contain
        .lock()
        .unwrap()
        .on_object_wants_to_enter_or_exit(&rider.read().unwrap(), ContainWant::WantsToEnter);
    assert!(contain.has_objects_wanting_to_enter_or_exit());
    let _frame = crate::system::game_logic::enter_update_frame(0);
    fixture.ai.lock().unwrap().update().unwrap();
    assert_eq!(
        fixture
            .ai
            .lock()
            .unwrap()
            .get_supply_truck_ai_interface()
            .unwrap()
            .get_number_boxes(),
        0
    );
    assert!(
        !fixture
            .drawable
            .read()
            .unwrap()
            .get_model_conditions()
            .contains(ModelConditionFlags::CARRYING)
    );
}

/// C++ combat-drop onEnter:461-495 sets Held and discards supplies even when
/// missing rope bones subsequently cause STATE_FAILURE.
#[test]
fn combat_drop_missing_bones_discards_supplies_before_failure() {
    if !child(concat!(
        module_path!(),
        "::combat_drop_missing_bones_discards_supplies_before_failure"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    let fixture = Fixture::new();
    let template = TheThingFactory::find_template(NAME).unwrap();
    let module = template
        .as_ref()
        .get_behavior_module_info()
        .iter()
        .find(|entry| entry.name.as_str() == "ChinookAIUpdate")
        .unwrap();
    let data = module
        .data
        .as_ref()
        .downcast_ref::<ChinookAIUpdateModuleData>()
        .unwrap();
    let mut chinook = ChinookAIUpdate::new(
        ChinookAIUpdateData::from_module(data),
        fixture.owner.read().unwrap().get_id(),
        0,
    );
    assert!(chinook.gain_one_box(2));
    assert!(chinook.gain_one_box(1));
    assert!(
        fixture
            .drawable
            .read()
            .unwrap()
            .get_model_conditions()
            .contains(ModelConditionFlags::CARRYING)
    );
    assert!(!chinook.start_combat_drop());
    assert!(
        fixture
            .owner
            .read()
            .unwrap()
            .is_disabled_by_type(DisabledType::Held)
    );
    assert_eq!(chinook.flight_status, ChinookFlightStatus::DoingCombatDrop);
    assert_eq!(chinook.get_number_boxes(), 0);
    assert!(
        !fixture
            .drawable
            .read()
            .unwrap()
            .get_model_conditions()
            .contains(ModelConditionFlags::CARRYING)
    );
    assert!(chinook.combat_drop_state.is_none());
}
