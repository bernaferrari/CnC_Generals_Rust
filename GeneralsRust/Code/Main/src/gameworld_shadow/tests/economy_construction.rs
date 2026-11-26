//! Economy authority, construction/production progress, rebuild/producer channels.

use super::*;

#[test]
fn canonical_front_crushed_is_projected_from_receiving_owner() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};

    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("CrushFl");
    logic.admit_object_ini_sources([("fixture-objects.ini".into(), "Object CrushMe\nEnd\n".into())]).expect("fixture Object content admitted before NewGame");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    {
        let mut t = ThingTemplate::new("CrushMe");
        t.add_kind_of(KindOf::Infantry);
        logic.templates.insert("CrushMe".into(), t);
    }
    let oid = logic
        .create_object("CrushMe", Team::USA, glam::Vec3::new(4.0, 0.0, 4.0))
        .expect("id");
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.front_crushed = true;
        o.back_crushed = false;
        o.crusher_level = 1;
        o.crushable_level = 1;
    }

    let mut shadow = GameWorldShadow::new();
    shadow.sync_from_host(&logic);
    let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
    shadow.sync_from_host(&logic);
    let e = shadow.world().entity(eid).unwrap();
    assert!(e.front_crushed);
    assert!(!e.back_crushed);

    shadow.sync_from_host(&logic);

    assert!(
        logic.host_objects().lookup(&oid).unwrap().front_crushed,
        "front crushed writeback"
    );
}

#[test]
fn waiting_for_path_channel_via_set_movement() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};

    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("WaitPath");
    logic.admit_object_ini_sources([("fixture-objects.ini".into(), "Object WaitUnit\nEnd\n".into())]).expect("fixture Object content admitted before NewGame");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    {
        let mut t = ThingTemplate::new("WaitUnit");
        t.add_kind_of(KindOf::Infantry);
        logic.templates.insert("WaitUnit".into(), t);
    }
    let oid = logic
        .create_object("WaitUnit", Team::USA, glam::Vec3::new(6.0, 0.0, 6.0))
        .expect("id");
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.waiting_for_path = true;
        o.movement.max_speed = 12.0;
    }

    let mut shadow = GameWorldShadow::new();
    shadow.sync_from_host(&logic);
    let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
    shadow.sync_from_host(&logic);
    assert!(shadow.world().entity(eid).unwrap().waiting_for_path);

    shadow.sync_from_host(&logic);

    assert!(
        logic.host_objects().lookup(&oid).unwrap().waiting_for_path,
        "waiting_for_path writeback"
    );
}

#[test]
fn locomotor_path_flags_channel_via_set_movement() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};

    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("LocoPath");
    logic.admit_object_ini_sources([("fixture-objects.ini".into(), "Object LocoU\nEnd\n".into())]).expect("fixture Object content admitted before NewGame");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    {
        let mut t = ThingTemplate::new("LocoU");
        t.add_kind_of(KindOf::Vehicle);
        logic.templates.insert("LocoU".into(), t);
    }
    let oid = logic
        .create_object("LocoU", Team::USA, glam::Vec3::new(9.0, 0.0, 9.0))
        .expect("id");
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.locomotor_surfaces = 0b101; // ground|cliff
        o.is_attack_path = true;
        o.is_braking = true;
        o.is_blocked_and_stuck = false;
        o.is_safe_path = true;
        o.queue_for_path_frame = 3;
        o.path_timestamp = 42;
        o.waiting_for_path = true;
        o.movement.max_speed = 15.0;
    }

    let mut shadow = GameWorldShadow::new();
    shadow.sync_from_host(&logic);
    let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
    shadow.sync_from_host(&logic);
    let e = shadow.world().entity(eid).unwrap();
    assert_eq!(e.locomotor_surfaces, 0b101);
    assert!(e.is_attack_path);
    assert!(e.is_braking);
    assert!(e.is_safe_path);
    assert_eq!(e.queue_for_path_frame, 3);
    assert_eq!(e.path_timestamp, 42);

    shadow.sync_from_host(&logic);

    let o = logic.host_objects().lookup(&oid).unwrap();
    assert_eq!(o.locomotor_surfaces, 0b101);
    assert!(o.is_attack_path);
    assert!(o.is_braking);
    assert_eq!(o.queue_for_path_frame, 3);
    assert_eq!(o.path_timestamp, 42);
}

#[test]
fn blocked_path_channel_via_set_movement() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};

    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("BlockP");
    logic.admit_object_ini_sources([("fixture-objects.ini".into(), "Object BlockU\nEnd\n".into())]).expect("fixture Object content admitted before NewGame");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    {
        let mut t = ThingTemplate::new("BlockU");
        t.add_kind_of(KindOf::Infantry);
        logic.templates.insert("BlockU".into(), t);
    }
    let oid = logic
        .create_object("BlockU", Team::USA, glam::Vec3::new(12.0, 0.0, 12.0))
        .expect("id");
    let other = logic
        .create_object("BlockU", Team::USA, glam::Vec3::new(14.0, 0.0, 12.0))
        .expect("other");
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.cur_max_blocked_speed = 3.5;
        o.num_frames_blocked = 7;
        o.is_blocked = true;
        o.move_away_from = Some(other);
        o.requested_victim_id = Some(other);
        o.movement.max_speed = 10.0;
    }

    let mut shadow = GameWorldShadow::new();
    shadow.sync_from_host(&logic);
    let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
    shadow.sync_from_host(&logic);
    let e = shadow.world().entity(eid).unwrap();
    assert!((e.cur_max_blocked_speed - 3.5).abs() < 1e-5);
    assert_eq!(e.num_frames_blocked, 7);
    assert!(e.is_blocked);
    assert_eq!(e.move_away_from_id, Some(other.0));
    assert_eq!(e.requested_victim_id, Some(other.0));

    shadow.sync_from_host(&logic);

    let o = logic.host_objects().lookup(&oid).unwrap();
    assert!((o.cur_max_blocked_speed - 3.5).abs() < 1e-5);
    assert_eq!(o.num_frames_blocked, 7);
    assert!(o.is_blocked);
    assert_eq!(o.move_away_from, Some(other));
    assert_eq!(o.requested_victim_id, Some(other));
}
