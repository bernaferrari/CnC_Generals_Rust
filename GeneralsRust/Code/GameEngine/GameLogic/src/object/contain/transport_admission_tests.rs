//! Canonical ObjectFactory admission queries for an authored TransportContain.
//! These cover C++ TransportContain.cpp:136-193, not Main host gameplay.

use crate::common::{Coord3D, DisabledType, INVALID_ID, ModelConditionFlags};
use crate::object::object_factory::{ObjectCreationFlags, ObjectFactory};
use crate::player::{Player, ThePlayerList};
use crate::team::Team;
use game_engine::common::system::xfer_save::XferSave;
use game_engine::common::thing::module_factory::{get_module_factory, init_module_factory};
use game_engine::common::thing::thing_factory::{get_thing_factory, init_thing_factory};
use std::io::Cursor;
use std::sync::{Arc, RwLock};

#[test]
fn factory_transport_admission_returns_cpp_predicate_without_mutation() {
    #[cfg(not(target_arch = "wasm32"))]
    if !matches!(
        crate::test_process::run_bounded(
            concat!(
                module_path!(),
                "::factory_transport_admission_returns_cpp_predicate_without_mutation"
            )
            .strip_prefix("gamelogic::")
            .unwrap(),
            "GENERALS_TRANSPORT_ADMISSION_CHILD",
        ),
        crate::test_process::TestProcess::Child
    ) {
        return;
    }
    let _serial = crate::test_sync::lock();
    if get_thing_factory().unwrap().is_none() {
        init_thing_factory().unwrap();
    }
    if get_module_factory().unwrap().is_none() {
        init_module_factory().unwrap();
    }
    crate::contain_module_overrides::ensure_module_overrides_installed().unwrap();
    assert_eq!(
        get_thing_factory().unwrap().as_mut().unwrap().load_ini_text(
            "Object AdmissionTransport\n KindOf = VEHICLE\n Behavior = TransportContain Cargo\n Slots = 3\n AllowInsideKindOf = INFANTRY\n NumberOfExitPaths = 0\n ResetMoodCheckTimeOnExit = No\n End\nEnd\nObject AdmissionTwoSlot\n KindOf = INFANTRY\n TransportSlotCount = 2\nEnd\nObject AdmissionOneSlot\n KindOf = INFANTRY\n TransportSlotCount = 1\nEnd\nObject AdmissionZeroSlot\n KindOf = INFANTRY\n TransportSlotCount = 0\nEnd\nObject AdmissionWrongKind\n KindOf = VEHICLE\n TransportSlotCount = 1\nEnd\n"
        ),
        5,
    );
    {
        let mut players = ThePlayerList().write().unwrap();
        players.clear();
        players.add_player(Arc::new(RwLock::new(Player::new(0))));
        players.add_player(Arc::new(RwLock::new(Player::new(1))));
    }
    let own_team = Arc::new(RwLock::new(Team::new("AdmissionOwn".into(), 9101)));
    own_team.write().unwrap().set_controlling_player_id(Some(0));
    let foreign_team = Arc::new(RwLock::new(Team::new("AdmissionForeign".into(), 9102)));
    foreign_team
        .write()
        .unwrap()
        .set_controlling_player_id(Some(1));
    let mut factory = ObjectFactory::new();
    let carrier_id = factory
        .create_object(
            "AdmissionTransport",
            Coord3D::new(20.0, 30.0, 0.0),
            Some(Arc::clone(&own_team)),
            ObjectCreationFlags::NO_AI,
        )
        .unwrap();
    let mut passenger_ids = Vec::new();
    for (name, team) in [
        ("AdmissionTwoSlot", &own_team),
        ("AdmissionTwoSlot", &own_team),
        ("AdmissionOneSlot", &own_team),
        ("AdmissionZeroSlot", &own_team),
        ("AdmissionTwoSlot", &foreign_team),
        ("AdmissionWrongKind", &own_team),
    ] {
        passenger_ids.push(
            factory
                .create_object(
                    name,
                    Coord3D::new(100.0, 110.0, 0.0),
                    Some(Arc::clone(team)),
                    ObjectCreationFlags::NO_AI,
                )
                .unwrap(),
        );
    }
    let carrier = factory
        .get_object(carrier_id)
        .unwrap()
        .get_base_object()
        .unwrap();
    let passenger = factory
        .get_object(passenger_ids[0])
        .unwrap()
        .get_base_object()
        .unwrap();
    let (contain, drawable) = {
        let carrier = carrier.read().unwrap();
        (
            carrier
                .get_contain()
                .expect("actual authored cargo interface"),
            carrier.get_drawable().expect("factory drawable"),
        )
    };
    let flags_before = drawable.read().unwrap().get_model_conditions();
    let save_contain = || {
        let mut bytes = Vec::new();
        contain
            .lock()
            .unwrap()
            .snapshot_xfer(&mut XferSave::new(Cursor::new(&mut bytes), 1))
            .unwrap();
        bytes
    };
    let state_before = save_contain();
    assert_eq!(
        contain.lock().unwrap().get_container_pips_to_show(),
        (3, 0, true)
    );

    // C++ accepts same-player positive slots after kind/relationship checks.
    // The old trait implementation discards this true predicate and returns false.
    assert!(
        contain.lock().unwrap().can_contain(passenger_ids[0]),
        "authored transport accepts a same-player two-slot rider in three slots"
    );
    assert!(contain.lock().unwrap().can_contain(passenger_ids[2]));
    assert!(!contain.lock().unwrap().can_contain(INVALID_ID));
    assert!(
        !contain.lock().unwrap().can_contain(passenger_ids[3]),
        "zero slots are not transportable"
    );
    assert!(
        !contain.lock().unwrap().can_contain(passenger_ids[4]),
        "transport rejects another controlling player"
    );
    assert!(
        !contain.lock().unwrap().can_contain(passenger_ids[5]),
        "base kind restriction is preserved"
    );

    // Querying must not execute containment or removal side effects.
    assert_eq!(
        contain.lock().unwrap().get_container_pips_to_show(),
        (3, 0, true)
    );
    assert_eq!(
        save_contain(),
        state_before,
        "query preserves serialized runtime fields including ExitBusy"
    );
    assert_eq!(
        drawable.read().unwrap().get_model_conditions(),
        flags_before
    );
    assert_eq!(passenger.read().unwrap().get_contained_by(), None);
    assert!(
        !passenger
            .read()
            .unwrap()
            .is_disabled_by_type(DisabledType::Held)
    );
    assert!(!carrier.read().unwrap().is_transporting());

    contain
        .lock()
        .unwrap()
        .contain_object(passenger_ids[0])
        .unwrap();
    assert_eq!(
        contain.lock().unwrap().get_container_pips_to_show(),
        (3, 2, true)
    );
    assert!(
        !contain.lock().unwrap().can_contain(passenger_ids[1]),
        "extra slots plus count plus rider slots exceed capacity"
    );
    assert!(
        contain.lock().unwrap().can_contain(passenger_ids[2]),
        "exact remaining capacity is accepted"
    );
    contain
        .lock()
        .unwrap()
        .contain_object(passenger_ids[2])
        .unwrap();
    assert_eq!(
        contain.lock().unwrap().get_container_pips_to_show(),
        (3, 3, true)
    );
    assert!(
        !contain.lock().unwrap().can_contain(passenger_ids[2]),
        "full capacity rejects another one-slot admission"
    );
    assert!(
        drawable
            .read()
            .unwrap()
            .get_model_conditions()
            .contains(ModelConditionFlags::LOADED)
    );
}
