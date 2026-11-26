//! Transport, garrison, and attached-object state.

use super::*;

#[test]
fn garrison_publication_uses_both_live_ends_of_the_relationship() {
    use crate::game_logic::{BuildingData, BuildingType};
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("ContainCh");
    crate::fixture_content::admit_fixture_objects(
        &mut logic,
        &[
            crate::fixture_content::FixtureObject::new("BunkC")
                .kind_of("SELECTABLE STRUCTURE")
                .health(200.0),
            crate::fixture_content::FixtureObject::new("InfC")
                .kind_of("SELECTABLE STRUCTURE")
                .health(200.0),
        ],
    );
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    for name in ["BunkC", "InfC"] {
        {
            let mut t = ThingTemplate::new(name);
            t.set_health(200.0);
            t.add_kind_of(KindOf::Selectable);
            if name == "BunkC" {
                t.add_kind_of(KindOf::Structure);
            }
            logic.templates.insert(name.into(), t);
        }
    }
    let bunker = logic
        .create_object("BunkC", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
        .expect("b");
    let inf = logic
        .create_object("InfC", Team::USA, glam::Vec3::new(1.0, 0.0, 0.0))
        .expect("i");
    {
        let o = logic.host_object_mut(bunker).expect("b");
        o.building_data = Some(BuildingData::new(BuildingType::Bunker));
        if let Some(bd) = o.building_data.as_mut() {
            bd.max_garrison = 5;
        }
    }

    {
        let o = logic.host_object_mut(bunker).expect("b");
        assert!(o.add_occupant(inf));
    }
    {
        let (o, fixture_partition) = logic.host_object_and_partition_mut(inf).expect("i");
        o.set_contained_by_with_partition_owner(Some(bunker), true, fixture_partition);
    }
    let frame = publish(&mut logic);
    assert_eq!(actor(&frame, inf).contained_by, Some(bunker));
    assert!(actor(&frame, bunker).garrisoned_units.contains(&inf));
    assert!(actor(&frame, bunker).occupant_count >= 1);
    assert_eq!(logic.host_object(inf).unwrap().contained_by, Some(bunker));
    let data = logic
        .host_object(bunker)
        .unwrap()
        .building_data
        .as_ref()
        .unwrap();
    assert!(data.garrisoned_units.contains(&inf));
}

#[test]
fn docked_is_published_for_the_receiving_hijacker() {
    use crate::game_logic::AIState;

    let mut logic = GameLogic::new();

    let cfg = golden_skirmish_config("HjAuth");
    logic
        .admit_object_ini_sources([("fixture-objects.ini".into(), "Object HjU\nEnd\n".into())])
        .expect("fixture Object content admitted before NewGame");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    {
        let mut t = ThingTemplate::new("HjU");
        t.add_kind_of(KindOf::Infantry);
        logic.templates.insert("HjU".into(), t);
    }
    let oid = logic
        .create_object("HjU", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
        .expect("id");
    logic.set_ai_state_decision_aware_for_test(oid, AIState::Docked);

    assert_eq!(
        logic.host_objects().lookup(&oid).unwrap().ai_state(),
        AIState::Docked,
        "host AI state applies immediately"
    );
    let frame = publish(&mut logic);
    let row = actor(&frame, oid);
    assert_eq!(row.ai_state_ordinal, AIState::Docked.to_ordinal());
}

#[test]
fn portable_bunker_capacity_publishes_on_its_receiving_carrier() {
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("OlCh");
    crate::fixture_content::admit_fixture_objects(
        &mut logic,
        &[
            // Retail Overlord: OverlordContain with one PORTABLE_STRUCTURE slot.
            crate::fixture_content::FixtureObject::new("OlU")
                .kind_of("SELECTABLE VEHICLE")
                .health(400.0)
                .walker()
                .module(
                    "Behavior = OverlordContain FixtureContain\n Slots = 1\n AllowInsideKindOf = PORTABLE_STRUCTURE\n PassengersAllowedToFire = Yes\nEnd",
                ),
            crate::fixture_content::FixtureObject::new("FramePortableBunker")
                .kind_of("PORTABLE_STRUCTURE")
                .health(100.0)
                .module("Behavior = TransportContain FixtureContain\n Slots = 4\nEnd"),
        ],
    );
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    let oid = logic
        .create_object("OlU", Team::China, glam::Vec3::new(21.0, 0.0, 21.0))
        .expect("id");

    let rider = logic
        .create_object("FramePortableBunker", Team::China, glam::Vec3::ZERO)
        .unwrap();
    logic.attach_overlord_portable_occupant(oid, rider);
    logic.host_object_mut(oid).unwrap().is_helix_transport = true;
    let frame = publish(&mut logic);
    let row = actor(&frame, oid);

    assert_eq!(row.overlord_bunker_capacity, 4);
    assert!(row.is_helix_transport);
    assert_eq!(
        logic.host_object(oid).unwrap().overlord_portable_occupant,
        Some(rider)
    );
    assert_eq!(logic.host_object(rider).unwrap().transport_capacity(), 4);
}

#[test]
fn hive_frame_freezes_its_slave_count_and_health() {
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("HiveCh");
    logic
        .admit_object_ini_sources([("fixture-objects.ini".into(), "Object HiveU\nEnd\n".into())])
        .expect("fixture Object content admitted before NewGame");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    {
        let mut t = ThingTemplate::new("HiveU");
        t.set_health(200.0);
        t.add_kind_of(KindOf::Selectable);
        t.add_kind_of(KindOf::Structure);
        logic.templates.insert("HiveU".into(), t);
    }
    let oid = logic
        .create_object("HiveU", Team::GLA, glam::Vec3::new(19.0, 0.0, 19.0))
        .expect("id");

    {
        let o = logic.host_object_mut(oid).expect("o");
        o.hive_slave_count = 3;
        o.hive_slave_hp = 55.0;
    }
    let frame = publish(&mut logic);
    let row = actor(&frame, oid);

    assert_eq!((row.hive_slave_count, row.hive_slave_hp), (3, 55.0));
    let object = logic.host_object_mut(oid).unwrap();
    object.hive_slave_count = 2;
    object.hive_slave_hp = 30.0;
    let current = publish(&mut logic);
    let latest = actor(&current, oid);
    assert_eq!((latest.hive_slave_count, latest.hive_slave_hp), (2, 30.0));
    assert_eq!((row.hive_slave_count, row.hive_slave_hp), (3, 55.0));
}

#[test]
fn transport_and_garrison_capacity_publish_from_the_object() {
    use crate::game_logic::buildings::{BuildingData, BuildingType};
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("CapCh");
    logic
        .admit_object_ini_sources([("fixture-objects.ini".into(), "Object CapU\nEnd\n".into())])
        .expect("fixture Object content admitted before NewGame");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    {
        let mut t = ThingTemplate::new("CapU");
        t.set_health(100.0);
        t.add_kind_of(KindOf::Selectable);
        logic.templates.insert("CapU".into(), t);
    }
    let oid = logic
        .create_object("CapU", Team::USA, glam::Vec3::new(18.0, 0.0, 18.0))
        .expect("id");

    {
        let o = logic.host_object_mut(oid).expect("o");
        o.max_transport = 5;
        let mut bd = BuildingData::new(BuildingType::Bunker);
        bd.max_garrison = 8;
        o.building_data = Some(bd);
    }
    let frame = publish(&mut logic);
    let row = actor(&frame, oid);

    assert_eq!(row.max_transport, 5);
    assert_eq!(row.max_garrison, 8);
    let owner = logic.host_object(oid).unwrap();
    assert_eq!(owner.max_transport, 5);
    assert_eq!(owner.building_data.as_ref().unwrap().max_garrison, 8);
}

#[test]
fn publishing_keeps_the_hijacker_vehicle_and_ejection_state_on_the_object() {
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("Hijack");
    logic
        .admit_object_ini_sources([("fixture-objects.ini".into(), "Object HjU\nEnd\n".into())])
        .expect("fixture Object content admitted before NewGame");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    {
        let mut t = ThingTemplate::new("HjU");
        t.add_kind_of(KindOf::Infantry);
        logic.templates.insert("HjU".into(), t);
    }
    let oid = logic
        .create_object("HjU", Team::USA, glam::Vec3::new(180.0, 0.0, 180.0))
        .expect("id");
    let vehicle = logic
        .create_object("HjU", Team::China, glam::Vec3::new(190.0, 0.0, 180.0))
        .expect("v");
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.hijack_vehicle_id = Some(vehicle);
        o.hijacker_in_vehicle = true;
        o.hijacker_update_active = true;
        o.hijacker_was_airborne = true;
        o.hijacker_eject_pos = Some(glam::Vec3::new(3.0, 1.0, 4.0));
        o.hive_slave_respawn_frame = 250;
        o.next_detection_scan_frame = 33;
    }
    let _frame = publish(&mut logic);
    let o = logic.host_object(oid).unwrap();

    assert_eq!(o.hijack_vehicle_id, Some(vehicle));
    assert!(o.hijacker_in_vehicle && o.hijacker_update_active && o.hijacker_was_airborne);
    assert_eq!(o.hijacker_eject_pos, Some(Vec3::new(3.0, 1.0, 4.0)));
    assert_eq!(
        (o.hive_slave_respawn_frame, o.next_detection_scan_frame),
        (250, 33)
    );
}
