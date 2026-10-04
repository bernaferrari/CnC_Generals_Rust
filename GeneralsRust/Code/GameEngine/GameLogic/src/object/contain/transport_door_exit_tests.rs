//! Actual authored TransportContain dispatch through Object's exit interface.
//! The no-art-path branch checks removal hooks and door timing, not movement AI.

use crate::common::{Coord3D, DisabledType, ModelConditionFlags};
use crate::modules::ExitDoorType;
use crate::object::object_factory::{ObjectCreationFlags, ObjectFactory};
use crate::player::{Player, ThePlayerList};
use crate::team::Team;
use game_engine::common::thing::module_factory::{get_module_factory, init_module_factory};
use game_engine::common::thing::thing_factory::{get_thing_factory, init_thing_factory};
use std::sync::{Arc, RwLock};

#[test]
fn authored_transport_stream_parses_inherited_and_derived_fields_once() {
    use super::TransportContainModuleData;
    use game_engine::common::ini::{INI, INIError};
    let mut data = TransportContainModuleData::default();
    // C++ TransportContain.cpp:72-100 installs parent and derived tables
    // together. One End closes the block; a final partial line is valid.
    INI::new()
        .with_inline_source(
            "Slots = 2\n DoorOpenTime = 100ms\n Slots = 3\n NumberOfExitPaths = 0\n ExitDelay = 500ms\n ResetMoodCheckTimeOnExit = No\n End",
            |ini| data.parse_from_ini(ini),
        )
        .expect("one authored block accepts both inherited tables");
    assert_eq!(data.slot_capacity, 3);
    assert_eq!(data.base.door_open_time, 3);
    assert_eq!(data.base.number_of_exit_paths, 0);
    assert_eq!(data.exit_delay, 15);
    assert!(!data.reset_mood_check_time_on_exit);
    let error = INI::new()
        .with_inline_source("UnknownTransportField = 1\n End", |ini| {
            data.parse_from_ini(ini)
        })
        .unwrap_err();
    assert!(matches!(error, INIError::UnknownToken));
}

fn child(name: &str) -> bool {
    #[cfg(not(target_arch = "wasm32"))]
    {
        matches!(
            crate::test_process::run_bounded(
                name.strip_prefix("gamelogic::").unwrap_or(name),
                "GENERALS_AUTHORED_TRANSPORT_EXIT_CHILD",
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

#[test]
fn factory_transport_door_exit_runs_transport_removal_before_shared_exit() {
    if child(concat!(
        module_path!(),
        "::factory_transport_door_exit_runs_transport_removal_before_shared_exit"
    )) {
        verify_factory_exit(false);
    }
}

#[test]
fn factory_transport_hurry_exit_runs_transport_removal_before_shared_exit() {
    if child(concat!(
        module_path!(),
        "::factory_transport_hurry_exit_runs_transport_removal_before_shared_exit"
    )) {
        verify_factory_exit(true);
    }
}

fn verify_factory_exit(hurry: bool) {
    let _serial = crate::test_sync::lock();
    if get_thing_factory().unwrap().is_none() {
        init_thing_factory().unwrap();
    }
    if get_module_factory().unwrap().is_none() {
        init_module_factory().unwrap();
    }
    crate::contain_module_overrides::ensure_module_overrides_installed().unwrap();
    assert_eq!(get_thing_factory().unwrap().as_mut().unwrap().load_ini_text(
        "Object AuthoredDoorTransport\n KindOf = VEHICLE\n Behavior = TransportContain Cargo\n Slots = 3\n DoorOpenTime = 100ms\n ExitDelay = 500ms\n NumberOfExitPaths = 0\n ResetMoodCheckTimeOnExit = No\n End\nEnd\nObject AuthoredDoorPassenger\n KindOf = INFANTRY\n TransportSlotCount = 2\nEnd\n"
    ), 2);
    ThePlayerList()
        .write()
        .unwrap()
        .add_player(Arc::new(RwLock::new(Player::new(0))));
    let team = Arc::new(RwLock::new(Team::new("DoorTeam".into(), 9001)));
    team.write().unwrap().set_controlling_player_id(Some(0));
    let mut factory = ObjectFactory::new();
    let carrier_id = factory
        .create_object(
            "AuthoredDoorTransport",
            Coord3D::new(20.0, 30.0, 0.0),
            Some(Arc::clone(&team)),
            ObjectCreationFlags::NO_AI,
        )
        .unwrap();
    let passenger_id = factory
        .create_object(
            "AuthoredDoorPassenger",
            Coord3D::new(100.0, 110.0, 0.0),
            Some(team),
            ObjectCreationFlags::NO_AI,
        )
        .unwrap();
    let carrier = factory
        .get_object(carrier_id)
        .unwrap()
        .get_base_object()
        .unwrap();
    let passenger = factory
        .get_object(passenger_id)
        .unwrap()
        .get_base_object()
        .unwrap();
    let (contain, exit, drawable) = {
        let carrier = carrier.read().unwrap();
        (
            carrier.get_contain().expect("authored cargo module"),
            carrier
                .get_object_exit_interface()
                .expect("actual exit proxy"),
            carrier.get_drawable().expect("actual factory drawable"),
        )
    };
    contain
        .lock()
        .unwrap()
        .contain_object(passenger_id)
        .unwrap();
    let rally = Coord3D::new(80.0, 90.0, 0.0);
    contain.lock().unwrap().set_rally_point(rally);
    assert_eq!(contain.lock().unwrap().get_rally_point(), Some(rally));
    assert_eq!(
        contain.lock().unwrap().get_container_pips_to_show(),
        (3, 2, true)
    );
    assert!(
        passenger
            .read()
            .unwrap()
            .is_disabled_by_type(DisabledType::Held)
    );
    assert!(carrier.read().unwrap().is_transporting());
    assert!(
        drawable
            .read()
            .unwrap()
            .get_model_conditions()
            .contains(ModelConditionFlags::LOADED)
    );

    // C++ OpenContain.cpp:915-932/1036-1055 first calls virtual
    // removeFromContain, so Transport onRemoving must precede the shared door pulse.
    if hurry {
        exit.lock()
            .unwrap()
            .exit_object_in_a_hurry(passenger_id)
            .unwrap();
    } else {
        exit.lock()
            .unwrap()
            .exit_object_via_door(passenger_id, ExitDoorType::NoneAvailable)
            .unwrap();
        assert_eq!(contain.lock().unwrap().get_contained_count(), 1);
        exit.lock()
            .unwrap()
            .exit_object_via_door(passenger_id, ExitDoorType::Door1)
            .unwrap();
    }
    assert_eq!(contain.lock().unwrap().get_contained_count(), 0);
    assert_eq!(contain.lock().unwrap().get_rally_point(), Some(rally));
    assert_eq!(
        contain.lock().unwrap().get_container_pips_to_show(),
        (3, 0, true)
    );
    assert_eq!(passenger.read().unwrap().get_contained_by(), None);
    assert!(
        !passenger
            .read()
            .unwrap()
            .is_disabled_by_type(DisabledType::Held)
    );
    assert!(!carrier.read().unwrap().is_transporting());
    let flags = drawable.read().unwrap().get_model_conditions();
    assert!(!flags.contains(ModelConditionFlags::LOADED));
    assert!(flags.contains(ModelConditionFlags::DOOR_1_OPENING));
    assert!(!flags.contains(ModelConditionFlags::DOOR_1_CLOSING));
    // The authored 100ms is three original 30Hz logic frames. Retain both
    // update boundary and closing flag semantics, without wall-clock timing.
    for _ in 0..2 {
        contain.lock().unwrap().update().unwrap();
        assert!(
            drawable
                .read()
                .unwrap()
                .get_model_conditions()
                .contains(ModelConditionFlags::DOOR_1_OPENING)
        );
    }
    contain.lock().unwrap().update().unwrap();
    let flags = drawable.read().unwrap().get_model_conditions();
    assert!(!flags.contains(ModelConditionFlags::DOOR_1_OPENING));
    assert!(flags.contains(ModelConditionFlags::DOOR_1_CLOSING));
}
