//! C++ AIUpdate owns LocomotorSet and its current pointer, independently of Unit registries.
//! These native child fixtures own their authored definition catalog until process exit.

use super::UnitAIUpdate;
use crate::common::{AsciiString, Coord3D, LocomotorSetType};
use crate::helpers::{TheGameLogic, TheThingFactory};
use crate::locomotor::{LOCOMOTOR_STORE, LocomotorTemplate};
use crate::modules::AIUpdateInterface;
use crate::object::object_factory::{ObjectCreationFlags, ObjectFactory};
use crate::object::update::AIUpdateModuleData;
use game_engine::common::system::{xfer_load::XferLoad, xfer_save::XferSave};
use game_engine::common::thing::module_factory::{get_module_factory, init_module_factory};
use game_engine::common::thing::thing_factory::{get_thing_factory, init_thing_factory};
use std::io::Cursor;

fn child(name: &str) -> bool {
    #[cfg(not(target_arch = "wasm32"))]
    {
        matches!(
            crate::test_process::run_bounded(
                name.strip_prefix("gamelogic::").unwrap_or(name),
                "GENERALS_OWNED_LOCOMOTOR_CHILD",
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

fn definitions() {
    if get_thing_factory().unwrap().is_none() {
        init_thing_factory().unwrap();
    }
    if get_module_factory().unwrap().is_none() {
        init_module_factory().unwrap();
    }
    crate::contain_module_overrides::ensure_module_overrides_installed().unwrap();
    for (name, height) in [("OwnedNormalLoco", 10.0), ("OwnedOtherLoco", 90.0)] {
        let mut definition = LocomotorTemplate::new(name.to_string());
        definition.preferred_height = height;
        LOCOMOTOR_STORE.register_template(definition);
    }
}

fn runtime(id: u32, name: &str) -> UnitAIUpdate {
    let mut runtime = UnitAIUpdate::new(
        id,
        None,
        None,
        None,
        None,
        None,
        #[cfg(feature = "allow_surrender")]
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    );
    let mut data = AIUpdateModuleData::default();
    data.set_locomotor_set_entries(LocomotorSetType::Normal, vec![AsciiString::from(name)]);
    runtime.apply_ai_update_module_data(&data);
    runtime
}

#[test]
fn authored_factory_ai_initializes_normal_without_unit_registry() {
    if !child(concat!(
        module_path!(),
        "::authored_factory_ai_initializes_normal_without_unit_registry"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    definitions();
    assert_eq!(get_thing_factory().unwrap().as_mut().unwrap().load_ini_text(
        "Object OwnedNormalUnit\n KindOf = VEHICLE\n Behavior = AIUpdateInterface OwnedAI\n End\n Locomotor = SET_NORMAL OwnedNormalLoco\nEnd\n"
    ), 1);
    let template = TheThingFactory::find_template("OwnedNormalUnit").unwrap();
    let data = template
        .as_ref()
        .get_behavior_module_info()
        .iter()
        .find_map(|entry| entry.data.as_ref().downcast_ref::<AIUpdateModuleData>())
        .unwrap();
    assert_eq!(
        data.locomotor_sets()[&LocomotorSetType::Normal][0].as_str(),
        "OwnedNormalLoco"
    );
    let mut factory = ObjectFactory::new();
    let id = factory
        .create_object(
            "OwnedNormalUnit",
            Coord3D::new(10.0, 20.0, 0.0),
            None,
            ObjectCreationFlags::empty(),
        )
        .unwrap();
    let owner = factory.get_object(id).unwrap().get_base_object().unwrap();
    assert!(std::sync::Arc::ptr_eq(
        &owner,
        &TheGameLogic::find_object_by_id(id).unwrap()
    ));
    assert!(
        super::registry::get_unit_arc(id).is_none(),
        "factory must not invent Unit admission"
    );
    let ai = owner.read().unwrap().get_ai_update_interface().unwrap();
    let ai = ai.lock().unwrap();
    let set = ai
        .get_locomotor_set_clone()
        .expect("actual cached AI must own authored Normal set immediately");
    assert_eq!(set.active_name(), Some("OwnedNormalLoco"));
    assert_eq!(set.len(), 1);
    assert_eq!(set.get_active().unwrap().preferred_height, 10.0);
}

#[test]
fn same_id_runtime_locomotors_are_independent() {
    if !child(concat!(
        module_path!(),
        "::same_id_runtime_locomotors_are_independent"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    definitions();
    // Exact mutable AI instances share only numeric identity and immutable definitions.
    // No unrelated admitted object supplies the locomotor state under test.
    let mut first = runtime(730021, "OwnedNormalLoco");
    let second = runtime(730021, "OwnedOtherLoco");
    assert!(super::registry::get_unit_arc(730021).is_none());
    first.with_cur_locomotor_mut(&mut |loco| loco.preferred_height = 35.0);
    let first = first
        .get_locomotor_set_clone()
        .expect("first AI owns its set");
    let second = second
        .get_locomotor_set_clone()
        .expect("second AI owns its set");
    assert_eq!(first.active_name(), Some("OwnedNormalLoco"));
    assert_eq!(first.get_active().unwrap().preferred_height, 35.0);
    assert_eq!(second.active_name(), Some("OwnedOtherLoco"));
    assert_eq!(second.get_active().unwrap().preferred_height, 90.0);
}

#[test]
fn same_id_restore_preserves_current_locomotor_without_unit_registry() {
    if !child(concat!(
        module_path!(),
        "::same_id_restore_preserves_current_locomotor_without_unit_registry"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    definitions();
    let mut saved = runtime(730022, "OwnedNormalLoco");
    saved.with_cur_locomotor_mut(&mut |loco| loco.preferred_height = 47.0);
    let mut bytes = Cursor::new(Vec::new());
    saved
        .xfer_locomotor_set_state(&mut XferSave::new(&mut bytes, 1))
        .unwrap();
    let mut restored = runtime(730022, "OwnedOtherLoco");
    // AIUpdate.cpp version4 clears the existing set before loading all runtime members.
    restored
        .xfer_locomotor_set_state(&mut XferLoad::new(Cursor::new(bytes.into_inner()), 1))
        .unwrap();
    let restored = restored
        .get_locomotor_set_clone()
        .expect("loaded AI owns saved set and current pointer");
    assert_eq!(restored.active_name(), Some("OwnedNormalLoco"));
    assert_eq!(restored.get_active().unwrap().preferred_height, 47.0);
    assert_eq!(
        saved
            .get_locomotor_set_clone()
            .unwrap()
            .get_active()
            .unwrap()
            .preferred_height,
        47.0
    );
}

#[test]
fn authored_generic_and_transport_ai_proxies_have_initial_phase() {
    if !child(concat!(
        module_path!(),
        "::authored_generic_and_transport_ai_proxies_have_initial_phase"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    definitions();
    assert_eq!(get_thing_factory().unwrap().as_mut().unwrap().load_ini_text(
        "Object OwnedPhaseGeneric\n KindOf = VEHICLE\n Behavior = AIUpdateInterface PhaseGenericAI\n End\n Locomotor = SET_NORMAL OwnedNormalLoco\nEnd\nObject OwnedPhaseTransport\n KindOf = VEHICLE\n Behavior = TransportAIUpdate PhaseTransportAI\n End\n Locomotor = SET_NORMAL OwnedOtherLoco\nEnd\n"
    ), 2);
    let mut factory = ObjectFactory::new();
    for (name, module) in [
        ("OwnedPhaseGeneric", "AIUpdateInterface"),
        ("OwnedPhaseTransport", "TransportAIUpdate"),
    ] {
        let id = factory
            .create_object(
                name,
                Coord3D::new(20.0, 30.0, 0.0),
                None,
                ObjectCreationFlags::empty(),
            )
            .unwrap();
        let owner = factory.get_object(id).unwrap().get_base_object().unwrap();
        assert!(super::registry::get_unit_arc(id).is_none());
        let owner = owner.read().unwrap();
        assert!(owner.get_ai_update_interface().is_some());
        let registrations: Vec<_> = owner
            .update_module_registrations
            .iter()
            .filter(|registration| registration.module_name.as_str() == module)
            .collect();
        assert_eq!(
            registrations.len(),
            1,
            "one existing authored AI scheduler proxy"
        );
        let registration = registrations[0];
        let index = registration
            .module_index
            .expect("registration refers to exact authored entry");
        assert_eq!(owner.modules[index].name().as_str(), module);
        // AIUpdate.h:570-580: AI locomotive force precedes Physics for each owner.
        assert_eq!(
            registration.module.read().unwrap().get_update_phase(),
            game_engine::common::thing::update_module::SleepyUpdatePhase::Initial
        );
    }
}
