//! Movement/weapon/stealth/turret/power host-log channels.

use super::*;

#[test]
fn canonical_movement_is_projected_from_receiving_owner() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    use glam::Vec3;
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("MvCh");
    logic.admit_object_ini_sources([("fixture-objects.ini".into(), "Object MvU\nEnd\n".into())]).expect("fixture Object content admitted before NewGame");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    {
        let mut t = ThingTemplate::new("MvU");
        t.set_health(200.0);
        t.add_kind_of(KindOf::Selectable);
        t.add_kind_of(KindOf::Vehicle);
        logic.templates.insert("MvU".into(), t);
    }
    let oid = logic
        .create_object("MvU", Team::USA, glam::Vec3::new(26.0, 0.0, 26.0))
        .expect("id");

    {
        let o = logic.host_object_mut(oid).expect("o");
        o.movement.velocity = Vec3::new(3.0, 0.0, 4.0);
        o.movement.max_speed = 12.5;
        o.movement.set_path_coords(vec![Vec3::new(1.0, 0.0, 1.0), Vec3::new(2.0, 0.0, 2.0)]);
        o.movement.path_cursor = o.movement.path.cursor_at_ordinal(1);
    }
    let mut shadow = GameWorldShadow::new();
    shadow.sync_from_host(&logic);
    let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.velocity = [0.0, 0.0, 0.0];
        e.move_max_speed = 1.0;
        e.path_index = 0;
        e.path_len = 0;
        e.path_waypoints.clear();
    }

    shadow.sync_from_host(&logic);
    let e = shadow.world().entity(eid).expect("e");
    assert!((e.velocity[0] - 3.0).abs() < 1e-5);
    assert!((e.move_max_speed - 12.5).abs() < 1e-5);
    assert_eq!(e.path_index, 1);
    assert_eq!(e.path_len, 2);
    assert_eq!(e.path_waypoints.len(), 2);

    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.velocity = [3.0, 0.0, 4.0];
        e.move_max_speed = 12.5;
        e.path_index = 1;
        e.path_len = 2;
        e.path_waypoints = vec![[1.0, 0.0, 1.0], [2.0, 0.0, 2.0]];
    }
    shadow.sync_from_host(&logic);

    let o = logic.host_objects().lookup(&oid).expect("o");
    assert!((o.movement.velocity.x - 3.0).abs() < 1e-5);
    assert!((o.movement.max_speed - 12.5).abs() < 1e-5);
    assert_eq!(o.movement.path.cursor_ordinal(&o.movement.path_cursor), 1);
    assert_eq!(o.movement.path.len(), 2);
}

#[test]
fn canonical_overlord_projects_without_writeback() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
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
            crate::fixture_content::FixtureObject::new("ShadowPortableBunker")
                .kind_of("PORTABLE_STRUCTURE")
                .health(100.0)
                .module("Behavior = TransportContain FixtureContain\n Slots = 4\nEnd"),
        ],
    );
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    let oid = logic
        .create_object("OlU", Team::China, glam::Vec3::new(21.0, 0.0, 21.0))
        .expect("id");

    let rider = logic.create_object("ShadowPortableBunker", Team::China, glam::Vec3::ZERO).unwrap();
    logic.attach_overlord_portable_occupant(oid, rider);
    logic.host_object_mut(oid).unwrap().is_helix_transport = true;
    let mut shadow = GameWorldShadow::new();
    shadow.sync_from_host(&logic);
    let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.overlord_bunker_capacity = u16::MAX;
        e.is_helix_transport = false;
    }

    shadow.sync_from_host(&logic);
    let e = shadow.world().entity(eid).expect("e");
    assert_eq!(e.overlord_bunker_capacity, 4);
    assert!(e.is_helix_transport);

    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.overlord_bunker_capacity = 4;
        e.is_helix_transport = true;
    }
    shadow.sync_from_host(&logic);

    let o = logic.host_objects().lookup(&oid).expect("o");
    assert_eq!(o.overlord_portable_occupant, Some(rider));
    assert_eq!(logic.host_object(rider).unwrap().transport_capacity(), 4);
    assert!(o.is_helix_transport);
}

#[test]
fn canonical_hive_state_is_projected_without_observer_writeback() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("HiveCh");
    logic.admit_object_ini_sources([("fixture-objects.ini".into(), "Object HiveU\nEnd\n".into())]).expect("fixture Object content admitted before NewGame");
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
    let mut shadow = GameWorldShadow::new();
    shadow.sync_from_host(&logic);
    let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.hive_slave_count = 0;
        e.hive_slave_hp = 0.0;
    }

    shadow.sync_from_host(&logic);
    let e = shadow.world().entity(eid).expect("e");
    assert_eq!(e.hive_slave_count, 3);
    assert!((e.hive_slave_hp - 55.0).abs() < 1e-3);

    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.hive_slave_count = 3;
        e.hive_slave_hp = 55.0;
    }
    shadow.sync_from_host(&logic);

    let o = logic.host_objects().lookup(&oid).expect("o");
    assert_eq!(o.hive_slave_count, 3);
    assert!((o.hive_slave_hp - 55.0).abs() < 1e-3);
}

#[test]
fn canonical_contain_capacity_projects_without_writeback() {
    use crate::game_logic::buildings::{BuildingData, BuildingType};
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("CapCh");
    logic.admit_object_ini_sources([("fixture-objects.ini".into(), "Object CapU\nEnd\n".into())]).expect("fixture Object content admitted before NewGame");
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
    let mut shadow = GameWorldShadow::new();
    shadow.sync_from_host(&logic);
    let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.max_transport = 0;
        e.max_garrison = 0;
    }

    shadow.sync_from_host(&logic);
    let e = shadow.world().entity(eid).expect("e");
    assert_eq!(e.max_transport, 5);
    assert_eq!(e.max_garrison, 8);

    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.max_transport = 5;
        e.max_garrison = 8;
    }
    shadow.sync_from_host(&logic);
    let o = logic.host_objects().lookup(&oid).expect("o");
    assert_eq!(o.max_transport, 5);
    assert_eq!(o.building_data.as_ref().map(|bd| bd.max_garrison), Some(8));
}

#[test]
fn canonical_overcharge_projects_without_writeback() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("OcCh");
    logic.admit_object_ini_sources([("fixture-objects.ini".into(), "Object OCU\nEnd\n".into())]).expect("fixture Object content admitted before NewGame");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    {
        let mut t = ThingTemplate::new("OCU");
        t.set_health(100.0);
        t.add_kind_of(KindOf::Selectable);
        t.add_kind_of(KindOf::Structure);
        logic.templates.insert("OCU".into(), t);
    }
    let oid = logic
        .create_object("OCU", Team::China, glam::Vec3::new(17.0, 0.0, 17.0))
        .expect("id");

    {
        let o = logic.host_object_mut(oid).expect("o");
        o.set_overcharge_enabled(true);
    }
    let mut shadow = GameWorldShadow::new();
    shadow.sync_from_host(&logic);
    let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.overcharge_enabled = false;
    }

    shadow.sync_from_host(&logic);
    let e = shadow.world().entity(eid).expect("e");
    assert!(e.overcharge_enabled);

    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.overcharge_enabled = true;
    }
    shadow.sync_from_host(&logic);

    let o = logic.host_objects().lookup(&oid).expect("o");
    assert!(o.overcharge_enabled);
}

#[test]
fn host_ai_attitude_is_canonical_and_shadow_only_observes() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    fn world() -> (GameLogic, crate::game_logic::ObjectId) {
        let mut logic = GameLogic::new();
        let mut template = ThingTemplate::new("AttitudeOwner");
        template.set_health(100.0);
        template.add_kind_of(KindOf::Selectable);
        logic.templates.insert("AttitudeOwner".into(), template);
        let id = logic
            .create_object("AttitudeOwner", Team::USA, glam::Vec3::new(15.0, 0.0, 15.0))
            .unwrap();
        (logic, id)
    }
    let (mut first, id) = world();
    first.host_object_mut(id).unwrap().set_ai_attitude_i8(2);
    let (mut second, peer_id) = world();
    assert_eq!(id, peer_id);
    assert_eq!(
        first
            .host_objects()
            .lookup(&id)
            .expect("admitted host Object")
            .ai_attitude,
        2,
        "construction must not publish another owner"
    );
    second
        .host_object_mut(peer_id)
        .unwrap()
        .set_ai_attitude_i8(-2);
    let mut shadow = GameWorldShadow::new();
    shadow.sync_from_host(&first);
    let eid = shadow.host_to_entity[&id.0];
    assert_eq!(shadow.world().entity(eid).unwrap().ai_attitude, 2);
    shadow
        .world_mut()
        .world_mut()
        .entity_mut(eid)
        .unwrap()
        .ai_attitude = -1;
    first.host_object_mut(id).unwrap().set_ai_attitude_i8(1);
    assert_eq!(
        first
            .host_objects()
            .lookup(&id)
            .expect("admitted host Object")
            .ai_attitude,
        1
    );
    assert_eq!(
        second
            .host_objects()
            .lookup(&peer_id)
            .expect("admitted host Object")
            .ai_attitude,
        -2
    );
    shadow.sync_from_host(&first);
    assert_eq!(shadow.world().entity(eid).unwrap().ai_attitude, 1);
    let builder = crate::save_load::SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&first).unwrap();
    let mut restored = GameLogic::new();
    restored.templates = first.templates.clone();
    builder
        .restore_from_snapshot(&snapshot, &mut restored)
        .unwrap();
    assert_eq!(
        restored
            .host_objects()
            .lookup(&id)
            .expect("admitted host Object")
            .ai_attitude,
        1
    );
    restored.host_object_mut(id).unwrap().set_ai_attitude_i8(-2);
    assert_eq!(
        first
            .host_objects()
            .lookup(&id)
            .expect("admitted host Object")
            .ai_attitude,
        1
    );
    second.reset();
    drop(second);
    assert_eq!(
        first
            .host_objects()
            .lookup(&id)
            .expect("admitted host Object")
            .ai_attitude,
        1
    );
    first.host_object_mut(id).unwrap().set_ai_attitude_i8(127);
    assert_eq!(
        first
            .host_objects()
            .lookup(&id)
            .expect("admitted host Object")
            .ai_attitude,
        2,
        "preserve original attitude bounds"
    );
    first
        .host_object_mut(id)
        .unwrap()
        .set_ai_attitude(crate::game_logic::host_strategy_center::HostAiAttitude::Passive);
    assert_eq!(
        first
            .host_objects()
            .lookup(&id)
            .expect("admitted host Object")
            .ai_attitude,
        -1
    );
}

#[test]
fn canonical_detector_projects_without_writeback() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("DetCh");
    logic.admit_object_ini_sources([("fixture-objects.ini".into(), "Object DetU\nEnd\n".into())]).expect("fixture Object content admitted before NewGame");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    {
        let mut t = ThingTemplate::new("DetU");
        t.set_health(100.0);
        t.add_kind_of(KindOf::Selectable);
        logic.templates.insert("DetU".into(), t);
    }
    let oid = logic
        .create_object("DetU", Team::USA, glam::Vec3::new(10.0, 0.0, 10.0))
        .expect("id");

    {
        let o = logic.host_object_mut(oid).expect("o");
        o.set_detector_state(true, 175.0, 12);
    }
    let mut shadow = GameWorldShadow::new();
    shadow.sync_from_host(&logic);
    let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.is_detector = false;
        e.detection_range = 0.0;
        e.detection_rate_frames = 0;
    }

    shadow.sync_from_host(&logic);
    let e = shadow.world().entity(eid).expect("e");
    assert!(e.is_detector);
    assert!((e.detection_range - 175.0).abs() < 1e-3);
    assert_eq!(e.detection_rate_frames, 12);

    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.is_detector = true;
        e.detection_range = 175.0;
        e.detection_rate_frames = 12;
    }
    shadow.sync_from_host(&logic);

    let o = logic.host_objects().lookup(&oid).expect("o");
    assert!(o.is_detector);
    assert!((o.detection_range - 175.0).abs() < 1e-3);
    assert_eq!(o.detection_rate_frames, 12);
}

#[test]
fn canonical_turret_is_projected_from_receiving_owner() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("TurretCh");
    logic.admit_object_ini_sources([("fixture-objects.ini".into(), "Object TurU\nEnd\n".into())]).expect("fixture Object content admitted before NewGame");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    {
        let mut t = ThingTemplate::new("TurU");
        t.set_health(100.0);
        t.add_kind_of(KindOf::Selectable);
        logic.templates.insert("TurU".into(), t);
    }
    let oid = logic
        .create_object("TurU", Team::USA, glam::Vec3::new(8.0, 0.0, 8.0))
        .expect("id");

    {
        let o = logic.host_object_mut(oid).expect("o");
        o.turret_ai.turret_angle_deg = 33.0;
        o.turret_ai.turret_pitch_deg = 12.0;
        o.turret_ai.turret_holding = true;
        o.turret_ai.turret_idle_scanning = false;
    }
    let mut shadow = GameWorldShadow::new();
    shadow.sync_from_host(&logic);
    let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.turret_angle_deg = 0.0;
        e.turret_pitch_deg = 0.0;
        e.turret_holding = false;
    }

    shadow.sync_from_host(&logic);
    let e = shadow.world().entity(eid).expect("e");
    assert!((e.turret_angle_deg - 33.0).abs() < 1e-3);
    assert!((e.turret_pitch_deg - 12.0).abs() < 1e-3);
    assert!(e.turret_holding);

    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.turret_angle_deg = 33.0;
        e.turret_pitch_deg = 12.0;
        e.turret_holding = true;
    }
    shadow.sync_from_host(&logic);

    let o = logic.host_objects().lookup(&oid).expect("o");
    assert!((o.turret_ai.turret_angle_deg - 33.0).abs() < 1e-3);
    assert!((o.turret_ai.turret_pitch_deg - 12.0).abs() < 1e-3);
    assert!(o.turret_ai.turret_holding);
}

#[test]
fn canonical_repulsor_projects_without_writeback() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("RpCh");
    logic.admit_object_ini_sources([("fixture-objects.ini".into(), "Object RangerRp\nEnd\n".into())]).expect("fixture Object content admitted before NewGame");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    {
        let mut t = ThingTemplate::new("RangerRp");
        t.add_kind_of(KindOf::Infantry);
        t.add_kind_of(KindOf::Selectable);
        logic.templates.insert("RangerRp".into(), t);
    }
    let oid = logic
        .create_object("RangerRp", Team::USA, glam::Vec3::new(5.0, 0.0, 5.0))
        .expect("id");
    {
        let object = logic.host_object_mut(oid).unwrap();
        object.set_status_repulsor(true);
        object.arm_repulsor_countdown(60);
    }
    let mut shadow = GameWorldShadow::new();
    shadow.sync_from_host(&logic);
    let eid = *shadow.host_to_entity.get(&oid.0).expect("map");

    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.repulsor = false;
        e.repulsor_until_frame = 0;
    }

    shadow.sync_from_host(&logic);
    let e = shadow.world().entity(eid).expect("e");
    assert!(e.repulsor);
    assert_eq!(e.repulsor_until_frame, 60);

    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.repulsor = true;
        e.repulsor_until_frame = 12;
    }
    assert_eq!(logic.host_object(oid).unwrap().repulsor_until_frame, 60);
    logic
        .host_object_mut(oid)
        .unwrap()
        .arm_repulsor_countdown(12);
    shadow.sync_from_host(&logic);

    let o = logic.host_objects().lookup(&oid).expect("o");
    assert!(o.status.repulsor);
    assert_eq!(o.repulsor_until_frame, 12);
}
