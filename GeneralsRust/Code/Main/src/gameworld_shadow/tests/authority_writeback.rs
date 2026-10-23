//! Movement/damage/heal/XP authority, spawn/destroy, attack log, engine bridge.

use super::*;

fn owner_damage(logic: &mut GameLogic, id: ObjectId, amount: f32) -> bool {
    let (object, health_events) = logic.host_object_and_health_events_mut(id).unwrap();
    object.take_damage(amount, health_events)
}

fn owner_heal(logic: &mut GameLogic, id: ObjectId, health: f32) {
    let body_frame = logic.frame;
    let (object, health_events) = logic.host_object_and_health_events_mut(id).unwrap();
    object.heal(health, health_events, body_frame);
}

#[test]
fn no_shadow_boundary_still_admits_projected_lethal_damage() {
    let mut logic = GameLogic::new();

    apply_skirmish_config(&mut logic, &golden_skirmish_config("PendingLethal")).expect("config");
    ensure_template(&mut logic, "PendingLethalTarget", 100.0);
    let id = logic
        .create_object("PendingLethalTarget", Team::USA, Vec3::ZERO)
        .expect("target");
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    logic.health_events.clear_damage();
    assert!(logic.host_lethal_finish_object(id, None));
    assert_eq!(logic.host_object(id).unwrap().health.current, 0.0);
    assert!(logic.host_object(id).unwrap().status.destroyed);
    assert!(logic.host_object(id).unwrap().status.effectively_dead);
    assert!(logic.health_events.snapshot_damage().is_empty());
    crate::gameworld_shadow::run_owned_host_boundary(None, &mut logic);
    assert_eq!(logic.host_object(id).unwrap().health.current, 0.0);
}

#[test]
fn gameworld_step_movement_advances_move_target() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    // Actual receiving locomotor drives movement; shadow only observes it.
    let mut logic = GameLogic::new();

    let cfg = golden_skirmish_config("MvAuth");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    if !logic.templates.contains_key("RangerMv") {
        let mut t = ThingTemplate::new("RangerMv");
        t.add_kind_of(KindOf::Infantry);
        t.add_kind_of(KindOf::Selectable);
        logic.templates.insert("RangerMv".into(), t);
    }
    let oid = logic
        .create_object("RangerMv", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
        .expect("id");
    {
        let (o, mut movement_events) = logic.host_object_and_movement_events_mut(oid).expect("o");
        let binding = crate::game_logic::locomotor_bootstrap::resolve_host_locomotor_binding(
            "BasicHumanLocomotor",
        )
        .expect("authored locomotor");
        o.locomotor_set_names = vec!["BasicHumanLocomotor".into()];
        o.cur_locomotor_name = Some("BasicHumanLocomotor".into());
        crate::game_logic::locomotor_bootstrap::apply_host_locomotor_binding(o, &binding);
        o.movement.max_speed = 60.0;
        o.movement.velocity = glam::Vec3::ZERO;
        o.move_to(
            glam::Vec3::new(100.0, 0.0, 0.0),
            &mut movement_events,
            &mut crate::game_logic::ObjectAudio::queue(&mut Vec::new()),
        );
    }
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
    let before = shadow.world().entity(eid).expect("e").transform.position.x;
    // Exercise the actual locomotor phase with original acceleration.
    // Match defeat/victory is independent of this isolated movement domain.
    for _ in 0..10 {
        logic.update_movement_for_test(&[oid], 1.0 / 30.0);
    }
    shadow.sync_from_host(&logic);
    let after = shadow.world().entity(eid).expect("e").transform.position.x;
    assert!(
        after > before + 0.1,
        "expected +X march before={before} after={after}"
    );
    // Immutable projection cannot replace the actual moved pose.
    shadow.sync_from_host(&logic);

    let host_x = logic
        .host_objects()
        .lookup(&oid)
        .expect("o")
        .get_position()
        .x;
    assert!(
        (host_x - after).abs() < 1e-3,
        "host pose writeback host={host_x} gw={after}"
    );
}

#[test]
fn damage_authority_defers_host_hp_until_writeback() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();

    let cfg = golden_skirmish_config("DmgAuth");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    if !logic.templates.contains_key("RangerDmg") {
        let mut t = ThingTemplate::new("RangerDmg");
        t.set_health(100.0);
        t.add_kind_of(KindOf::Infantry);
        t.add_kind_of(KindOf::Selectable);
        logic.templates.insert("RangerDmg".into(), t);
    }
    let oid = logic
        .create_object("RangerDmg", Team::USA, glam::Vec3::new(1.0, 0.0, 1.0))
        .expect("id");
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let before = logic.host_objects().lookup(&oid).expect("o").health.current;
    logic.health_events.clear_damage();

    {
        let (o, health_events) = logic.host_object_and_health_events_mut(oid).expect("o");
        let _ = o.take_damage(25.0, health_events);
    };

    // C++ ActiveBody::internalChangeHealth writes HP the same frame.
    let mid = logic.host_objects().lookup(&oid).expect("o").health.current;
    assert!(
        (mid - (before - 25.0)).abs() < 1e-5,
        "host HP same-frame before={before} mid={mid}"
    );

    let events = logic.health_events.drain_damage();
    assert!(
        events
            .iter()
            .any(|e| e.target == oid && (e.amount - 25.0).abs() < 1e-5),
        "events {:?}",
        events
    );
    // Re-record for session (drained above).
    logic.health_events.clear_damage();
    {
        let (o, health_events) = logic.host_object_and_health_events_mut(oid).expect("o");
        let _ = o.take_damage(25.0, health_events);
    }
    let _ = observe_completed_owner(&mut shadow, &mut logic);
    let after = logic.host_objects().lookup(&oid).expect("o").health.current;
    assert!(
        after < before - 20.0,
        "writeback must apply damage before={before} after={after}"
    );
}

#[test]
fn heal_authority_commits_host_hp_before_shadow_writeback() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();

    let cfg = golden_skirmish_config("HealAuth");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    if !logic.templates.contains_key("RangerHeal") {
        let mut t = ThingTemplate::new("RangerHeal");
        t.set_health(100.0);
        t.add_kind_of(KindOf::Infantry);
        logic.templates.insert("RangerHeal".into(), t);
    }
    let oid = logic
        .create_object("RangerHeal", Team::USA, glam::Vec3::new(2.0, 0.0, 2.0))
        .expect("id");
    // Seed wounded host HP without authority path (direct field for setup).
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.health.current = 40.0;
    }
    // CPP VictoryConditions87–95/168–196: skirmish requires a live
    // victory-counting structure. Otherwise the session defeats this player
    // and kills its army while this control is checking health writeback.
    let mut keep_alive = ThingTemplate::new("HealthVictoryKeepAlive");
    keep_alive
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::MpCountForVictory);
    logic
        .templates
        .insert("HealthVictoryKeepAlive".into(), keep_alive);
    logic
        .create_object("HealthVictoryKeepAlive", Team::USA, glam::Vec3::ZERO)
        .unwrap();
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    logic.health_events.clear_heal();

    {
        let body_frame = logic.frame;
        let (o, health_events) = logic.host_object_and_health_events_mut(oid).expect("o");
        o.heal(30.0, health_events, body_frame);
    };

    let mid = logic.host_objects().lookup(&oid).expect("o").health.current;
    assert!((mid - 70.0).abs() < 1e-5, "host heal committed mid={mid}");
    let events = logic.health_events.snapshot_heal();
    assert!(
        events
            .iter()
            .any(|e| e.target == oid && (e.health - 70.0).abs() < 1e-5),
        "events {:?}",
        events
    );
    let _ = observe_completed_owner(&mut shadow, &mut logic);
    let after = logic.host_objects().lookup(&oid).expect("o").health.current;
    assert!((after - 70.0).abs() < 1e-3, "writeback heal after={after}");
    assert!(logic.health_events.snapshot_heal().is_empty());
}

#[test]
fn experience_is_immediate_even_during_shadow_coupling() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();

    let mut template = ThingTemplate::new("RangerXp");
    template.set_health(100.0);
    template.add_kind_of(KindOf::Infantry);
    template.is_trainable = true;
    template.veterancy_xp_thresholds = [1000.0, 2000.0, 3000.0];
    logic.templates.insert(template.name.clone(), template);
    let id = logic
        .create_object("RangerXp", Team::USA, glam::Vec3::new(3.0, 0.0, 3.0))
        .unwrap();
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let entity = shadow.entity_for_host(id).unwrap();
    let before = logic.host_object(id).unwrap().experience.current;

    {
        logic.award_experience(id, 50.0);
        assert_eq!(
            logic.host_object(id).unwrap().experience.current,
            before + 50.0
        );
    };
    shadow
        .world_mut()
        .world_mut()
        .entity_mut(entity)
        .unwrap()
        .experience_points = 999.0;
    logic.award_experience(id, 50.0);
    assert_eq!(
        logic.host_object(id).unwrap().experience.current,
        before + 100.0
    );
    let _ = observe_completed_owner(&mut shadow, &mut logic);
    assert_eq!(
        logic.host_object(id).unwrap().experience.current,
        before + 100.0
    );
    assert_eq!(
        shadow.world().entity(entity).unwrap().experience_points,
        before + 100.0
    );
}

#[test]
fn host_update_movement_skips_when_gameworld_movement_authority() {
    let mut logic = GameLogic::new();

    // Session integrates then writebacks.
    let cfg = golden_skirmish_config("MvSkip");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    if !logic.templates.contains_key("RangerSk") {
        let mut t = crate::game_logic::ThingTemplate::new("RangerSk");
        t.add_kind_of(crate::game_logic::KindOf::Infantry);
        logic.templates.insert("RangerSk".into(), t);
    }
    let oid = logic
        .create_object(
            "RangerSk",
            crate::game_logic::Team::USA,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("id");
    {
        let (o, mut movement_events) = logic.host_object_and_movement_events_mut(oid).expect("o");
        o.movement.max_speed = 60.0;
        // The movement residual now carries the synced Locomotor acceleration
        // (C++ getMaxAcceleration) and the GameWorld integrator clamps its
        // velocity ramp to it. Arm a retail-scale acceleration so the ten
        // authority frames exercise a marching pose, not a ramp from standstill.
        o.movement.acceleration = 240.0;
        o.move_to(
            glam::Vec3::new(50.0, 0.0, 0.0),
            &mut movement_events,
            &mut crate::game_logic::ObjectAudio::queue(&mut Vec::new()),
        );
    }
    // A real logic step (not the trailing observational probe) evaluates
    // victory. Per C++
    // VictoryConditions.cpp:87-95/168-196 the skirmish NO_BUILDINGS rule
    // defeats a structure-less playable player on frame 0-1 and
    // kill_player_for_victory destroys its army — the unit under test would
    // be marked destroyed mid-test. Retail skirmish starts with a
    // MpCountForVictory structure; seed a keep-alive (sell_heal.rs /
    // economy_construction.rs precedent).
    if !logic.templates.contains_key("VictoryKeepAlive") {
        let mut t = ThingTemplate::new("VictoryKeepAlive");
        t.add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::MpCountForVictory);
        logic.templates.insert("VictoryKeepAlive".into(), t);
    }
    let _keep_alive = logic
        .create_object(
            "VictoryKeepAlive",
            Team::USA,
            glam::Vec3::new(30.0, 0.0, 0.0),
        )
        .expect("keep-alive structure");
    let before = logic
        .host_objects()
        .lookup(&oid)
        .expect("o")
        .get_position()
        .x;
    let mut shadow = GameWorldShadow::new(64);
    // Multiple authority frames (path integrate + pose writeback each session).
    for _ in 0..10 {
        logic.update();
        let _ = observe_completed_owner(&mut shadow, &mut logic);
    }
    let after = logic
        .host_objects()
        .lookup(&oid)
        .expect("o")
        .get_position()
        .x;
    assert!(
        after > before + 1.0,
        "shadow session movement authority must march host pose before={before} after={after}"
    );
}

#[test]
fn host_disable_timers_log_drives_set_disable_timers_channel() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("DtCh");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    if !logic.templates.contains_key("RangerDt") {
        let mut t = ThingTemplate::new("RangerDt");
        t.add_kind_of(KindOf::Infantry);
        t.add_kind_of(KindOf::Selectable);
        logic.templates.insert("RangerDt".into(), t);
    }
    let oid = logic
        .create_object("RangerDt", Team::USA, glam::Vec3::new(6.0, 0.0, 6.0))
        .expect("id");
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let eid = *shadow.host_to_entity.get(&oid.0).expect("map");

    {
        let (o, health_events) = logic.host_object_and_health_events_mut(oid).expect("o");
        o.apply_disabled_emp(
            500,
            health_events,
            &mut crate::game_logic::ObjectAudio::queue(&mut Vec::new()),
        );
        o.apply_disabled_hacked(
            600,
            health_events,
            &mut crate::game_logic::ObjectAudio::queue(&mut Vec::new()),
        );
        o.apply_disabled_paralyzed(
            700,
            &mut crate::game_logic::ObjectAudio::queue(&mut Vec::new()),
        );
    }
    let o = logic.host_object(oid).expect("canonical owner");
    assert_eq!(
        (
            o.status.disabled_emp_until_frame,
            o.status.disabled_hacked_until_frame,
            o.status.disabled_paralyzed_until_frame
        ),
        (500, 600, 700)
    );
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.disabled_emp_until_frame = 0;
        e.disabled_hacked_until_frame = 0;
        e.disabled_paralyzed_until_frame = 0;
    }
    shadow.sync_from_host(&logic);
    let e = shadow.world().entity(eid).expect("e");
    assert_eq!(e.disabled_emp_until_frame, 500);
    assert_eq!(e.disabled_hacked_until_frame, 600);
    assert_eq!(e.disabled_paralyzed_until_frame, 700);

    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.disabled_emp_until_frame = 11;
        e.disabled_hacked_until_frame = 22;
        e.disabled_paralyzed_until_frame = 33;
    }
    shadow.sync_from_host(&logic);
    let o = logic.host_object(oid).unwrap();
    assert_eq!(
        (
            o.status.disabled_emp_until_frame,
            o.status.disabled_hacked_until_frame,
            o.status.disabled_paralyzed_until_frame
        ),
        (500, 600, 700)
    );
}
#[test]
fn host_experience_is_canonical_and_shadow_only_observes() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    fn world() -> (GameLogic, crate::game_logic::ObjectId) {
        let mut logic = GameLogic::new();
        let mut template = ThingTemplate::new("XpU");
        template.set_health(100.0);
        template.add_kind_of(KindOf::Selectable);
        template.is_trainable = true;
        template.veterancy_xp_thresholds = [1000.0, 2000.0, 3000.0];
        logic.templates.insert(template.name.clone(), template);
        let id = logic
            .create_object("XpU", Team::USA, glam::Vec3::new(3.0, 0.0, 3.0))
            .unwrap();
        (logic, id)
    }
    let (mut logic, id) = world();
    let (mut peer, peer_id) = world();
    assert_eq!(id, peer_id);
    logic.award_experience(id, 42.0);
    peer.award_experience(peer_id, 7.0);
    assert_eq!(logic.host_object(id).unwrap().experience.current, 42.0);
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let entity = shadow.entity_for_host(id).unwrap();
    assert_eq!(
        shadow.world().entity(entity).unwrap().experience_points,
        42.0
    );
    shadow
        .world_mut()
        .world_mut()
        .entity_mut(entity)
        .unwrap()
        .experience_points = 9000.0;
    logic.award_experience(id, 1.0);
    shadow.sync_from_host(&logic);
    assert_eq!(logic.host_object(id).unwrap().experience.current, 43.0);
    assert_eq!(
        shadow.world().entity(entity).unwrap().experience_points,
        43.0
    );
    assert_eq!(peer.host_object(peer_id).unwrap().experience.current, 7.0);
    let builder = crate::save_load::SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&logic).unwrap();
    let mut restored = GameLogic::new();
    restored.templates = logic.templates.clone();
    builder
        .restore_from_snapshot(&snapshot, &mut restored)
        .unwrap();
    restored.award_experience(id, 1.0);
    logic.award_experience(id, 1.0);
    assert_eq!(restored.host_object(id).unwrap().experience.current, 44.0);
    assert_eq!(logic.host_object(id).unwrap().experience.current, 44.0);
    peer.reset();
    drop(peer);
    assert_eq!(logic.host_object(id).unwrap().experience.current, 44.0);
}

#[test]
fn canonical_max_health_is_projected_from_receiving_owner() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("MaxHealthCh");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    if !logic.templates.contains_key("MaxHU") {
        let mut t = ThingTemplate::new("MaxHU");
        t.set_health(100.0);
        t.add_kind_of(KindOf::Selectable);
        logic.templates.insert("MaxHU".into(), t);
    }
    let oid = logic
        .create_object("MaxHU", Team::USA, glam::Vec3::new(2.0, 0.0, 2.0))
        .expect("id");
    {
        let obj = logic.host_object_mut(oid).expect("o");
        obj.max_health = 250.0;
        obj.health.maximum = 250.0;
        obj.health.current = 200.0;
    }
    assert_eq!(logic.host_object(oid).unwrap().max_health, 250.0);
    assert_eq!(logic.host_object(oid).unwrap().health.current, 200.0);
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.max_health = 1.0;
    }
    shadow.sync_from_host(&logic);
    let e = shadow.world().entity(eid).expect("e");
    assert!((e.max_health - 250.0).abs() < 1e-3, "max {}", e.max_health);
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.health = 200.0;
        e.max_health = 250.0;
    }
    shadow.sync_from_host(&logic);
    let obj = logic.host_objects().lookup(&oid).expect("o");
    assert!(
        (obj.max_health - 250.0).abs() < 1e-3,
        "host max {}",
        obj.max_health
    );
    assert!((obj.health.maximum - 250.0).abs() < 1e-3);
}

#[test]
fn sync_from_host_copies_host_orientation() {
    let src = GAMEWORLD_SHADOW_SRC;
    let idx = src
        .find("pub fn sync_from_host_with")
        .expect("canonical observation sync");
    let window = &src[idx..idx + 2200];
    assert!(
        window.contains("obj.get_orientation()"),
        "sync implementation must copy host orientation into Transform"
    );
    assert!(
        !window.contains("Transform::new([pos.x, pos.y, pos.z], 0.0)"),
        "sync must not wipe orientation to 0.0"
    );
}

#[test]
fn apply_host_positions_uses_host_orientation_channel() {
    // Object::set_orientation may be masked by engine-bridge registry reads; the
    // production pose channel uses get_orientation() into SetTransform. Prove the
    // bulk path applies a non-zero orientation when the host reports one via the
    // same queue used when get_orientation returns a known value.
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("OrientPose");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    ensure_template(&mut logic, "OrientU", 100.0);
    let id = logic
        .create_object("OrientU", Team::USA, glam::Vec3::new(5.0, 0.0, 5.0))
        .expect("id");
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let pos = {
        let obj = logic.host_objects().lookup(&id).unwrap();
        let p = obj.get_position();
        [p.x, p.y, p.z]
    };
    {
        let o = logic.host_object_mut(id).unwrap();
        o.set_position(glam::Vec3::from_array(pos));
        o.set_orientation(0.75);
    }
    shadow.sync_from_host(&logic);
    let eid = shadow.entity_for_host(id).unwrap();
    assert!((shadow.world().entity(eid).unwrap().transform.orientation - 0.75).abs() < 0.01);
    // Second pose write with new facing (simulates host turn + position step).
    {
        let o = logic.host_object_mut(id).unwrap();
        o.set_position(glam::Vec3::from_array([6.0, 0.0, 5.0]));
        o.set_orientation(-0.25);
    }
    shadow.sync_from_host(&logic);
    let e = shadow.world().entity(eid).unwrap();
    assert!((e.transform.position.x - 6.0).abs() < 0.01);
    assert!((e.transform.orientation - (-0.25)).abs() < 0.01);
}

#[test]
fn set_transform_mutation_moves_shadow_entity() {
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("MoveMut");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    ensure_template(&mut logic, "MoveUnit", 50.0);
    let id = logic
        .create_object("MoveUnit", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
        .expect("u");
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    {
        let o = logic.host_object_mut(id).unwrap();
        o.set_position(glam::Vec3::from_array([10.0, 0.0, 5.0]));
        o.set_orientation(1.5);
    }
    shadow.sync_from_host(&logic);
    let eid = shadow.entity_for_host(id).unwrap();
    let e = shadow.world().entity(eid).unwrap();
    assert!((e.transform.position.x - 10.0).abs() < 0.01);
    assert!((e.transform.position.z - 5.0).abs() < 0.01);
    assert!((e.transform.orientation - 1.5).abs() < 0.01);
}

#[test]
fn mark_for_destruction_logs_on_remove() {
    crate::game_logic::host_destroy_log::clear();
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("DesLog");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    ensure_template(&mut logic, "DesU", 50.0);
    let id = logic
        .create_object("DesU", Team::USA, glam::Vec3::ZERO)
        .expect("id");
    crate::game_logic::host_destroy_log::clear();
    logic.apply_host_object_id_op(crate::game_logic::HostObjectIdOp::MarkForDestruction {
        id: id,
        team: None,
    });
    logic.update_with_dt(1.0 / 30.0);
    let ev = crate::game_logic::host_destroy_log::drain();
    assert!(
        ev.iter().any(|e| e.id == id),
        "destroy process must log host_destroy: {ev:?}"
    );
    assert!(logic.host_objects().lookup(&id).is_none());
}

#[test]
fn spawn_is_committed_to_receiving_host_before_shadow_observation() {
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("SpawnMut");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    ensure_template(&mut logic, "SpMut", 80.0);
    let id = logic
        .create_object("SpMut", Team::USA, glam::Vec3::new(3.0, 0.0, 4.0))
        .expect("id");
    let object = logic
        .host_object(id)
        .expect("created object belongs to receiving host");
    assert_eq!(object.template_name, "SpMut");
    assert_eq!(object.team, Team::USA);
    assert_eq!(object.get_position(), glam::Vec3::new(3.0, 0.0, 4.0));
    let mut shadow = GameWorldShadow::new(4096);
    shadow.sync_from_host(&logic); // may already map
    assert!(shadow.entity_for_host(id).is_some());
    let entity = shadow.entity_for_host(id).unwrap();
    assert_eq!(
        shadow.world().entity(entity).unwrap().transform.position,
        logic.host_object(id).unwrap().get_position().into()
    );
}

#[test]
fn spawn_and_destroy_channel_maps_ids() {
    crate::game_logic::host_destroy_log::clear();
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("SpawnDestroy");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    ensure_template(&mut logic, "SpawnUnit", 80.0);
    let id = logic
        .create_object("SpawnUnit", Team::USA, glam::Vec3::new(3.0, 0.0, 0.0))
        .expect("spawn");
    let object = logic
        .host_object(id)
        .expect("successful factory creation is immediately observable");
    assert_eq!(object.template_name, "SpawnUnit");
    assert_eq!(object.team, Team::USA);
    assert_eq!(object.get_position(), glam::Vec3::new(3.0, 0.0, 0.0));

    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    // apply_spawn should be no-op (already mapped)
    shadow.sync_from_host(&logic);
    assert!(shadow.entity_for_host(id).is_some());

    logic.destroy_object(id);
    for _ in 0..3 {
        logic.update();
    }
    assert!(
        logic.host_object(id).is_none(),
        "physical retirement removes the actual Object"
    );
    let eid_before = shadow.entity_for_host(id);
    assert!(eid_before.is_some());
    shadow.sync_from_host(&logic);
    assert!(
        shadow.entity_for_host(id).is_none(),
        "entity unmapped after destroy"
    );
}

#[test]
fn production_authority_defaults_off_host_sole_writer() {
    // The production last-writer is a per-GameLogic context field (hq-e84zk
    // retired the GENERALS_GAMEWORLD_PRODUCTION_AUTHORITY env flag):
    // tick/authority.rs keeps the host GameLogic sole writer by default and
    // the wave177 residual pins the default-off gate. A fresh instance
    // publishes an all-off barrier; the setter opts this instance in and out.
    let mut logic = GameLogic::new();
}

#[test]
fn attack_target_logs_host_attack_event() {
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("AtkLog");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    ensure_template(&mut logic, "AtkA", 100.0);
    ensure_template(&mut logic, "AtkB", 100.0);
    if let Some(t) = logic.templates.get_mut("AtkA") {
        t.add_kind_of(KindOf::Infantry);
    }
    let a = logic
        .create_object("AtkA", Team::USA, glam::Vec3::ZERO)
        .expect("a");
    let b = logic
        .create_object("AtkB", Team::GLA, glam::Vec3::new(10.0, 0.0, 0.0))
        .expect("b");
    {
        let (o, attack_events) = logic.host_object_and_attack_events_mut(a).unwrap();
        // Ensure can_attack path: weapon or kind
        o.attack_target(
            b,
            attack_events,
            &mut crate::game_logic::ObjectAudio::queue(&mut Vec::new()),
        );
    }
    logic.attack_events.complete_boundary();
    let events = logic.attack_events.take_completed();
    assert!(
        events.iter().any(|e| e.attacker == a && e.target == b),
        "attack_target must log host_attack event: {events:?}"
    );
    {
        let o = logic.host_object_mut(a).unwrap();
        o.stop_attack(&mut crate::game_logic::ObjectAudio::queue(&mut Vec::new()));
    }
    logic.attack_events.complete_boundary();
    let clears = logic.attack_events.take_completed();
    assert!(
        clears.is_empty()
            && logic.host_object(a).unwrap().target.is_none()
            && !logic.host_object(a).unwrap().status.attacking,
        "stop_attack must clear the canonical target without a targeting receipt: {clears:?}"
    );
}

#[test]
fn attack_log_feeds_set_attack_target_mutation() {
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("AtkLog");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    ensure_template(&mut logic, "LogA", 100.0);
    ensure_template(&mut logic, "LogB", 100.0);
    let a = logic
        .create_object("LogA", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
        .expect("a");
    let b = logic
        .create_object("LogB", Team::GLA, glam::Vec3::new(15.0, 0.0, 0.0))
        .expect("b");
    if let Some((obj, attack_events)) = logic.host_object_and_attack_events_mut(a) {
        obj.set_target(
            Some(b),
            attack_events,
            &mut crate::game_logic::ObjectAudio::queue(&mut Vec::new()),
        );
    }
    logic.attack_events.complete_boundary();
    let evs = logic.attack_events.take_completed();
    assert_eq!(evs.len(), 1);
    assert_eq!(evs[0].attacker, a);
    assert_eq!(evs[0].target, b);

    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    // Clear then re-apply via log channel
    let ea = shadow.entity_for_host(a).unwrap();
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(ea) {
        e.attack_target = None;
    }
    for ev in &evs {
        assert_eq!(
            logic.host_object(ev.attacker).unwrap().target,
            Some(ev.target)
        );
    }
    shadow.sync_from_host(&logic);
    let eb = shadow.entity_for_host(b).unwrap();
    assert_eq!(shadow.world().entity(ea).unwrap().attack_target, Some(eb));
}

#[test]
fn shadow_session_defaults_on() {
    // Session defaults on when SHADOW unset (process may have gate env from other tests).
    if std::env::var_os("GENERALS_GAMEWORLD_SHADOW").is_none() {
        assert!(
            gameworld_shadow_enabled(),
            "shadow session should default on when env unset"
        );
    } else {
        // If explicitly set, respect the helper's parse.
        let _ = gameworld_shadow_enabled();
    }
}

#[test]
fn attack_target_syncs_to_shadow_entity() {
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("AtkTarget");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    ensure_template(&mut logic, "AtkA", 100.0);
    ensure_template(&mut logic, "AtkB", 100.0);
    let a = logic
        .create_object("AtkA", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
        .expect("a");
    let b = logic
        .create_object("AtkB", Team::GLA, glam::Vec3::new(20.0, 0.0, 0.0))
        .expect("b");
    if let Some((obj, attack_events)) = logic.host_object_and_attack_events_mut(a) {
        obj.set_target(
            Some(b),
            attack_events,
            &mut crate::game_logic::ObjectAudio::queue(&mut Vec::new()),
        );
    }
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let ea = shadow.entity_for_host(a).unwrap();
    let eb = shadow.entity_for_host(b).unwrap();
    assert_eq!(shadow.world().entity(ea).unwrap().attack_target, Some(eb));
    logic
        .host_object_mut(a)
        .unwrap()
        .clear_target(&mut crate::game_logic::ObjectAudio::queue(&mut Vec::new()));
    shadow.sync_from_host(&logic);
    assert_eq!(shadow.world().entity(ea).unwrap().attack_target, None);
}

#[test]
fn attack_target_writeback_updates_host() {
    // writeback_attack_targets_to_host is the AI-attack last-writer channel
    // (writeback_core.rs:295 gate); it exists only under the per-instance
    // opt-in GameLogic::set_ai_attack_authority(true) (hq-e84zk retired the
    // GENERALS_GAMEWORLD_AI_ATTACK_AUTHORITY env flag), matching the C++
    // GameLogic-sole-writer default.
    let mut logic = GameLogic::new();

    let cfg = golden_skirmish_config("AtkWb");
    ensure_template(&mut logic, "AtkWA", 100.0);
    ensure_template(&mut logic, "AtkWB", 100.0);
    let a = logic
        .create_object("AtkWA", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
        .expect("a");
    let b = logic
        .create_object("AtkWB", Team::GLA, glam::Vec3::new(20.0, 0.0, 0.0))
        .expect("b");
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    {
        let (object, attack_events) = logic.host_object_and_attack_events_mut(a).unwrap();
        object.set_target(
            Some(b),
            attack_events,
            &mut crate::game_logic::ObjectAudio::queue(&mut Vec::new()),
        );
    };
    shadow.sync_from_host(&logic);
    shadow.sync_from_host(&logic);

    shadow.sync_from_host(&logic);

    assert_eq!(logic.host_objects().lookup(&a).unwrap().target, Some(b));
    // Clear via shadow mutation + writeback
    logic
        .host_object_mut(a)
        .unwrap()
        .clear_target(&mut crate::game_logic::ObjectAudio::queue(&mut Vec::new()));
    shadow.sync_from_host(&logic);
    shadow.sync_from_host(&logic);

    shadow.sync_from_host(&logic);

    assert_eq!(logic.host_objects().lookup(&a).unwrap().target, None);
}

#[test]
fn probe_includes_host_victory_fields() {
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("VicProbe");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    let mut shadow = GameWorldShadow::new(4096);
    shadow.sync_from_host(&logic);
    let probe = shadow.probe(&mut logic);
    // Fresh skirmish: match not over; fields must still be populated honestly.
    assert!(!probe.host_match_over || probe.victory_label.is_some());
    let _ = probe.format_report(); // includes victory_over=
    assert!(
        probe.format_report().contains("victory_over="),
        "probe report must expose victory residual"
    );
}

#[test]
fn path_helpers_log_final_move_destination() {
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("PathLog");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    ensure_template(&mut logic, "PathU", 100.0);
    if let Some(t) = logic.templates.get_mut("PathU") {
        t.add_kind_of(KindOf::Infantry);
    }
    let id = logic
        .create_object("PathU", Team::USA, glam::Vec3::ZERO)
        .expect("id");
    {
        let o = logic.host_object_mut(id).unwrap();
        o.movement.max_speed = 20.0;
    }
    let dest = glam::Vec3::new(40.0, 0.0, 10.0);
    assert!(
        logic.append_unit_waypoint(id, dest),
        "append waypoint should succeed for mobile unit"
    );
    let events = logic.movement_events.drain_for_host_boundary();
    assert!(
        events.iter().any(|e| {
            e.unit == id
                && e.destination
                    .map(|d| (d[0] - 40.0).abs() < 0.5 && (d[2] - 10.0).abs() < 0.5)
                    .unwrap_or(false)
        }),
        "append_unit_waypoint must log final dest: {events:?}"
    );
}

#[test]
fn move_to_logs_destination_for_mobile_unit() {
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("MoveLog");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    ensure_template(&mut logic, "MoveLogU", 100.0);
    if let Some(tmpl) = logic.templates.get_mut("MoveLogU") {
        tmpl.add_kind_of(KindOf::Infantry);
    }
    let a = logic
        .create_object("MoveLogU", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
        .expect("a");
    assert!(
        logic.host_objects().lookup(&a).unwrap().is_mobile(),
        "template Infantry should make object mobile"
    );
    let (object, mut events) = logic.host_object_and_movement_events_mut(a).unwrap();
    object.set_destination(
        glam::Vec3::new(10.0, 0.0, 0.0),
        &mut events,
        &mut crate::game_logic::ObjectAudio::queue(&mut Vec::new()),
    );
    let ev = logic.movement_events.drain_for_host_boundary();
    assert_eq!(ev.len(), 1);
    assert_eq!(ev[0].unit, a);
    assert_eq!(ev[0].destination, Some([10.0, 0.0, 0.0]));
}

#[test]
fn move_target_writeback_updates_host() {
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("MoveWb");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    ensure_template(&mut logic, "MoveWA", 100.0);
    logic
        .templates
        .get_mut("MoveWA")
        .unwrap()
        .add_kind_of(KindOf::Infantry);
    let a = logic
        .create_object("MoveWA", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
        .expect("a");
    {
        let (object, mut writer) = logic.host_object_and_movement_events_mut(a).unwrap();
        object.set_destination(
            glam::Vec3::new(50.0, 0.0, 25.0),
            &mut writer,
            &mut crate::game_logic::ObjectAudio::queue(&mut Vec::new()),
        );
    }
    let events = logic.movement_events.drain_for_host_boundary();
    assert!(!events.is_empty(), "move log should hold destination");
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    shadow.sync_from_host(&logic);
    shadow.sync_from_host(&logic);
    let ea = shadow.entity_for_host(a).unwrap();
    assert_eq!(
        shadow.world().entity(ea).unwrap().move_target,
        Some([50.0, 0.0, 25.0])
    );
    logic
        .host_object_mut(a)
        .unwrap()
        .stop_moving(&mut crate::game_logic::ObjectAudio::queue(&mut Vec::new()));
    shadow.sync_from_host(&logic);
    assert!(
        logic
            .host_objects()
            .lookup(&a)
            .unwrap()
            .movement
            .target_position
            .is_none()
    );
}

#[test]
fn production_complete_applies_spawn_map_when_missing() {
    use crate::game_logic::host_production_log::HostProductionEvent;
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("ProdMap");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    ensure_template(&mut logic, "PMapU", 90.0);
    let id = logic
        .create_object("PMapU", Team::USA, glam::Vec3::new(1.0, 0.0, 2.0))
        .expect("id");
    let mut shadow = GameWorldShadow::new(64);
    // Do not sync — only Complete path should map.
    let ev = [HostProductionEvent::Complete {
        producer: ObjectId(1),
        template_name: "PMapU".into(),
        spawned: id,
    }];
    assert!(shadow.entity_for_host(id).is_none());
    shadow.sync_from_host(&logic);
    assert!(shadow.entity_for_host(id).is_some());
}

#[test]
fn production_complete_identifies_the_actual_created_object() {
    use crate::game_logic::{BuildingData, BuildingType, Player, ProductionExitMetadata};
    let mut logic = GameLogic::new();
    let mut player = Player::new(0, Team::USA, "CompletionOwner", true);
    player.resources.supplies = 10_000;
    player.power_produced = 1;
    logic.add_player(player);
    let mut factory = ThingTemplate::new("CompletionFactory");
    factory
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::FSBarracks)
        .add_kind_of(KindOf::MpCountForVictory)
        .set_health(500.0);
    factory.production_exit_metadata = Some(ProductionExitMetadata {
        style: crate::game_logic::thing::ProductionExitStyle::Default,
        unit_create_point: [0.0; 3],
        natural_rally_point: [0.0; 3],
        exit_delay_frames: 0,
        allow_airborne_creation: false,
        initial_burst: 0,
        use_spawn_rally_point: false,
        grant_temporary_stealth_frames: 0,
    });
    logic.templates.insert(factory.name.clone(), factory);
    let mut unit = ThingTemplate::new("CompletionInfantry");
    unit.add_kind_of(KindOf::Infantry).set_health(100.0);
    unit.build_time = 0.05;
    unit.build_cost.supplies = 0;
    unit.build_cost.power = 0;
    logic.templates.insert(unit.name.clone(), unit);
    let producer = logic
        .create_object_for_player("CompletionFactory", 0, Vec3::ZERO)
        .unwrap();
    logic.host_object_mut(producer).unwrap().building_data =
        Some(BuildingData::new(BuildingType::Barracks));
    let before: std::collections::HashSet<_> =
        logic.host_objects().registered_keys().copied().collect();
    assert!(logic.enqueue_production(producer, "CompletionInfantry".into()));
    let mut completed = None;
    for _ in 0..300 {
        logic.update_with_dt(1.0 / 30.0);
        logic.production_events.complete_boundary();
        for event in logic.production_events.take_completed() {
            if let crate::game_logic::host_production_log::HostProductionEvent::Complete {
                producer: actual_producer,
                template_name,
                spawned,
            } = event
            {
                if template_name == "CompletionInfantry" {
                    assert_eq!(actual_producer, producer);
                    assert!(
                        completed.replace(spawned).is_none(),
                        "one queue entry completes once"
                    );
                }
            }
        }
        if completed.is_some() {
            break;
        }
    }
    let spawned =
        completed.expect("accepted production must complete within its bounded build time");
    assert!(!before.contains(&spawned));
    let object = logic
        .host_object(spawned)
        .expect("Complete names an actual live Object");
    assert_eq!(object.template_name, "CompletionInfantry");
    assert_eq!(object.owner_player_id, Some(0));
    assert_eq!(object.team, Team::USA);
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let entity = shadow
        .entity_for_host(spawned)
        .expect("observer maps actual completed Object");
    assert_eq!(
        shadow.world().entity(entity).unwrap().transform.position,
        object.get_position().into()
    );
}

#[test]
fn production_enqueue_commits_queue_before_any_completion_receipt() {
    use crate::game_logic::{BuildingData, BuildingType, Player, ProductionExitMetadata};
    let mut logic = GameLogic::new();
    let mut player = Player::new(0, Team::USA, "QueueOwner", true);
    player.resources.supplies = 10_000;
    player.power_produced = 1;
    logic.add_player(player);
    let mut barracks = ThingTemplate::new("ReceiptQueueFactory");
    barracks
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::FSBarracks)
        .set_health(500.0);
    barracks.production_exit_metadata = Some(ProductionExitMetadata {
        style: crate::game_logic::thing::ProductionExitStyle::Default,
        unit_create_point: [0.0; 3],
        natural_rally_point: [0.0; 3],
        exit_delay_frames: 0,
        allow_airborne_creation: false,
        initial_burst: 0,
        use_spawn_rally_point: false,
        grant_temporary_stealth_frames: 0,
    });
    logic.templates.insert(barracks.name.clone(), barracks);
    let mut unit = ThingTemplate::new("ReceiptQueueInfantry");
    unit.add_kind_of(KindOf::Infantry).set_health(100.0);
    unit.build_time = 1.0;
    unit.build_cost.supplies = 100;
    logic.templates.insert(unit.name.clone(), unit);
    let producer = logic
        .create_object_for_player("ReceiptQueueFactory", 0, Vec3::ZERO)
        .unwrap();
    logic.host_object_mut(producer).unwrap().building_data =
        Some(BuildingData::new(BuildingType::Barracks));
    assert!(logic.enqueue_production(producer, "ReceiptQueueInfantry".into()));
    let queue = &logic
        .host_object(producer)
        .unwrap()
        .building_data
        .as_ref()
        .unwrap()
        .production_queue;
    assert_eq!(queue.len(), 1);
    assert_eq!(queue[0].template_name, "ReceiptQueueInfantry");
    assert_eq!(logic.get_player(0).unwrap().resources.supplies, 9_900);
    logic.production_events.complete_boundary();
    assert!(
        logic.production_events.take_completed().is_empty(),
        "an accepted queue is not a completed unit"
    );
}

#[test]
fn stale_engine_id_does_not_skip_host_movement() {
    {}
    // Host-only update_with_dt (no shadow session): keep host integrator on.
    crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", "1");
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("MoveBridge");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    ensure_template(&mut logic, "MoveBrU", 100.0);
    if let Some(t) = logic.templates.get_mut("MoveBrU") {
        t.add_kind_of(KindOf::Infantry);
    }
    // update_with_dt evaluates the skirmish NO_BUILDINGS victory rule
    // (C++ VictoryConditions.cpp:87-95/168-196): a structure-less playable
    // player is defeated on frame 0-1 and kill_player_for_victory destroys
    // its army — the unit under test disappears from host_objects. Seed the
    // retail-style MpCountForVictory keep-alive (sell_heal.rs precedent).
    if !logic.templates.contains_key("VictoryKeepAlive") {
        let mut t = ThingTemplate::new("VictoryKeepAlive");
        t.add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::MpCountForVictory);
        logic.templates.insert("VictoryKeepAlive".into(), t);
    }
    let _keep_alive = logic
        .create_object(
            "VictoryKeepAlive",
            Team::USA,
            glam::Vec3::new(300.0, 0.0, 300.0),
        )
        .expect("keep-alive structure");
    let id = logic
        .create_object("MoveBrU", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
        .expect("id");
    {
        let o = logic.host_object_mut(id).unwrap();
        o.movement.path = vec![
            glam::Vec3::new(0.0, 0.0, 0.0),
            glam::Vec3::new(50.0, 0.0, 0.0),
        ];
        o.movement.max_speed = 20.0;
        // C++ Locomotor::getMaxAcceleration: retail locomotors author
        // Acceleration (infantry ~220); the host integrate velocity-ramps by
        // acceleration*dt, so a 0-accel fixture never builds velocity.
        o.movement.acceleration = 240.0;
        o.movement.target_position = Some(glam::Vec3::new(50.0, 0.0, 0.0));
        o.status.moving = true;
        o.movement.max_speed = 20.0;
    }
    for _ in 0..10 {
        logic.update_with_dt(1.0 / 30.0);
    }
    let p = logic.host_objects().lookup(&id).unwrap().get_position();
    assert!(
        p.x > 0.05,
        "host movement must advance despite stale engine_object_id when bridge off; pos={p:?}"
    );
}

#[test]
fn host_object_ignores_registry_when_bridge_off() {
    {}
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("BridgeIgnore");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    ensure_template(&mut logic, "BridgeIgnU", 50.0);
    let id = logic
        .create_object("BridgeIgnU", Team::USA, glam::Vec3::ZERO)
        .expect("id");
    {
        let o = logic.host_object_mut(id).unwrap();
        // Stale bridge id must not hijack host pose/HP when bridge off.
        o.health.current = 12.0;
        o.set_position(glam::Vec3::new(3.0, 0.0, 4.0));
    }
    let o = logic.host_objects().lookup(&id).unwrap();
    assert!((o.get_health_percentage() - (12.0 / 50.0)).abs() < 0.02 || o.health.current == 12.0);
    let p = o.get_position();
    assert!((p.x - 3.0).abs() < 0.01 && (p.z - 4.0).abs() < 0.01);
    assert!(o.is_alive());
}

#[test]
fn host_object_pose_hp_never_dual_read_registry() {
    // Even with a stamped engine_object_id, host properties stay local.
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("HostSolePose");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    ensure_template(&mut logic, "HostSoleU", 80.0);
    let id = logic
        .create_object("HostSoleU", Team::USA, glam::Vec3::new(1.0, 0.0, 2.0))
        .expect("id");
    {
        let o = logic.host_object_mut(id).unwrap();
        o.health.current = 33.0;
        o.health.maximum = 80.0;
        o.set_position(glam::Vec3::new(7.0, 0.0, 9.0));
        o.set_orientation(1.25);
    }
    let o = logic.host_objects().lookup(&id).unwrap();
    assert_eq!(o.get_position(), glam::Vec3::new(7.0, 0.0, 9.0));
    assert!((o.get_orientation() - 1.25).abs() < 1e-5);
    assert!((o.get_health_percentage() - (33.0 / 80.0)).abs() < 1e-5);
    assert!(o.is_alive());
    // Source honesty: no OBJECT_REGISTRY dual-read helpers on host Object.
    let src = GAME_LOGIC_OBJECT_SRC;
    assert!(
        !src.contains("read_engine_position")
            && !src.contains("read_engine_is_alive")
            && !src.contains("write_engine_position")
            && !src.contains("fn engine_bridge_active"),
        "host Object must not dual-read/write OBJECT_REGISTRY"
    );
}

#[test]
fn reset_skips_factory_when_bridge_off() {
    {}
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("ResetBridge");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    ensure_template(&mut logic, "RstU", 50.0);
    let _ = logic
        .create_object("RstU", Team::USA, glam::Vec3::ZERO)
        .expect("id");
    assert!(!logic.host_objects().registered_is_empty());
    // Must not panic / lock-poison on factory residual when bridge off.
    logic.reset();
    assert!(logic.host_objects().registered_is_empty());
    assert_eq!(logic.get_frame(), 0);
}

#[test]
fn engine_object_bridge_off_by_default() {
    // Default path: dual-object factory stamp retired; bridge env off.
    if std::env::var_os("GENERALS_ALLOW_DUAL_TICK").is_none()
        && std::env::var_os("GENERALS_BRIDGE_ENGINE_OBJECTS").is_none()
    {}
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("BridgeOff");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    ensure_template(&mut logic, "BridgeUnit", 50.0);
    let id = logic
        .create_object("BridgeUnit", Team::USA, glam::Vec3::ZERO)
        .expect("id");
    let _ = id;
    // create_object no longer dual-creates into ObjectFactory.
    let src = concat!(
        include_str!("../../game_logic/game_logic/crate_tick.rs"),
        include_str!("../../game_logic/game_logic/player.rs"),
        include_str!("../../game_logic/game_logic/host.rs"),
        include_str!("../../game_logic/game_logic/script_camera.rs"),
        include_str!("../../game_logic/game_logic/authority.rs"),
        include_str!("../../game_logic/game_logic/construct.rs"),
        include_str!("../../game_logic/game_logic/mod.rs"),
    );
    assert!(
        !src.contains("obj.engine_object_id = Some(engine_id)"),
        "create_object must not stamp dual-world engine ids"
    );
}

#[test]
fn owned_damage_and_move_do_not_implicitly_publish_to_comparison_world() {
    crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", "1");
    assert!(
        matches!(
            crate::authoritative_world::dual_tick_policy(),
            crate::authoritative_world::DualTickPolicy::AuthorityOnly
        ),
        "production dual_tick_policy stays AuthorityOnly"
    );
    assert!(gameworld_shadow_enabled());

    let mut logic = GameLogic::new();

    let cfg = golden_skirmish_config("GwStore");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    ensure_template(&mut logic, "GwStoreU", 100.0);
    let oid = logic
        .create_object("GwStoreU", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
        .expect("id");

    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);

    let before_gw = {
        let eid = shadow.entity_for_host(oid).expect("map");
        shadow.world().entity(eid).expect("e").health
    };
    {
        logic.with_host_object_mut(oid, |o| {
            o.health.current = (o.health.current - 25.0).max(0.0);
        });
        let (o, mut movement_events) = logic.host_object_and_movement_events_mut(oid).expect("o");
        o.move_to(
            glam::Vec3::new(40.0, 0.0, 0.0),
            &mut movement_events,
            &mut crate::game_logic::ObjectAudio::queue(&mut Vec::new()),
        );
    };

    let eid = shadow.entity_for_host(oid).expect("map");
    let gw_hp = shadow.world().entity(eid).expect("e").health;
    assert!(
        (gw_hp - before_gw).abs() < 0.01,
        "coupled read-view HashMap HP poke must not last-write GameWorld; gw={gw_hp} before={before_gw}"
    );
    let auth_hp = { logic.host_authoritative_health(oid).expect("hp") };
    assert!(
        (auth_hp - 75.0).abs() < 1e-4,
        "ordinary HP query must observe the canonical owned body; auth={auth_hp} gw={gw_hp}"
    );
    let auth_pose = { logic.host_authoritative_pose(oid).expect("pose") };
    let gw_pose = {
        let p = shadow.world().entity(eid).expect("e").transform.position;
        [p.x, p.y, p.z]
    };
    assert!(
        (auth_pose[0] - gw_pose[0]).abs() < 0.01,
        "authoritative pose must match GameWorld"
    );

    shadow.sync_from_host(&logic);
    shadow.sync_from_host(&logic);
    let host_hp = logic.host_objects().lookup(&oid).expect("o").health.current;
    assert!(
        (host_hp - 75.0).abs() < 1e-3,
        "after writeback host and GameWorld must share one HP; host={host_hp} gw={gw_hp}"
    );
}

#[test]
fn host_object_mut_borrows_owned_fields_without_observation_ingress() {
    crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", "1");
    assert!(gameworld_shadow_enabled());

    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("GwView");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    ensure_template(&mut logic, "GwViewU", 80.0);
    let oid = logic
        .create_object("GwViewU", Team::USA, glam::Vec3::new(1.0, 0.0, 2.0))
        .expect("id");

    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);

    let eid = shadow.entity_for_host(oid).expect("map");
    {
        shadow
            .world_mut()
            .world_mut()
            .entity_mut(eid)
            .unwrap()
            .health = 40.0;
        {
            let o = logic.host_object_mut(oid).expect("view");
            assert!(
                (o.health.current - 80.0).abs() < 1e-3,
                "host_object_mut must borrow the owned body; got {}",
                o.health.current
            );
            o.health.current = 33.0;
        }
    };
    let gw_hp = shadow.world().entity(eid).expect("e").health;
    assert!(
        (gw_hp - 40.0).abs() < 1e-3,
        "coupled read-view must not last-write HashMap HP into GameWorld; gw={gw_hp}"
    );
    let auth = { logic.host_authoritative_health(oid).expect("auth") };
    assert!((auth - 33.0).abs() < 1e-4);
}

#[test]
fn owned_fat_fields_remain_visible_without_implicit_shadow_publication() {
    crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", "1");
    assert!(
        matches!(
            crate::authoritative_world::dual_tick_policy(),
            crate::authoritative_world::DualTickPolicy::AuthorityOnly
        ),
        "dual crate tick stays off"
    );

    assert!(
        gamelogic::object::registry::OBJECT_REGISTRY.store_is_empty(),
        "host create/couple must not fill crate OBJECT_REGISTRY"
    );

    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("GwFat");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    ensure_template(&mut logic, "GwFatU", 90.0);
    ensure_template(&mut logic, "GwFatPax", 40.0);
    let carrier = logic
        .create_object("GwFatU", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
        .expect("carrier");
    let pax = logic
        .create_object("GwFatPax", Team::USA, glam::Vec3::new(2.0, 0.0, 0.0))
        .expect("pax");

    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);

    {
        logic.with_host_object_mut(carrier, |o| {
            let mut w = crate::game_logic::Weapon::default();
            w.clip_size = 8;
            w.ammo = Some(5);
            o.weapon = Some(w);
            o.occupants = vec![pax];
            o.movement.target_position = Some(glam::Vec3::new(30.0, 0.0, 4.0));
            o.movement.path = vec![
                glam::Vec3::new(10.0, 0.0, 0.0),
                glam::Vec3::new(30.0, 0.0, 4.0),
            ];
            o.movement.current_path_index = 1;
        });

        let frame = logic
            .host_stamp_attack_substate_at_frame(
                carrier,
                crate::game_logic::AttackSubState::FireWeapon,
            )
            .expect("split-borrow stamp");
        assert_eq!(
            frame, logic.frame,
            "stamp must read logic frame while mutating the object map field"
        );
    };

    let eid = shadow.entity_for_host(carrier).expect("map");
    let ent = shadow.world().entity(eid).expect("e");
    assert_ne!(
        ent.weapon_ammo, 5,
        "coupled read-view must not last-write HashMap ammo into GameWorld"
    );
    assert_ne!(ent.weapon_clip_size, 8);
    assert_ne!(
        ent.attack_substate_ordinal,
        crate::game_logic::AttackSubState::FireWeapon.to_ordinal()
    );
    assert_eq!(ent.occupant_count, 0);
    assert!(ent.garrisoned_host_ids.is_empty());
    assert_eq!(ent.move_target, None);
    assert!(ent.path_waypoints.is_empty());

    {
        logic
            .with_host_object_mut(carrier, |o| {
                o.movement.target_position = None;
            })
            .expect("clear dest");
    };
    let ent = shadow.world().entity(eid).expect("e");
    assert_eq!(
        ent.move_target, None,
        "read-view clear dest must not invent a GameWorld move target"
    );
    assert_eq!(
        logic.host_authoritative_move_dest(carrier),
        None,
        "authoritative dest is None after clear, not the HashMap leftover"
    );
    {
        if let Some(o) = logic.host_object_mut(carrier) {
            o.movement.target_position = Some(glam::Vec3::new(77.0, 0.0, 1.0));
        }
    };
    assert_eq!(
        shadow.world().entity(eid).expect("e").move_target,
        None,
        "poking HashMap dest without commit must not change GameWorld"
    );
    assert_eq!(
        logic.host_authoritative_move_dest(carrier),
        Some([77.0, 0.0, 1.0]),
        "ordinary dest query observes the owned mutation immediately"
    );

    // Comparison state remains independent of the canonical owned values.
    {
        if let Some(o) = logic.host_object_mut(carrier) {
            o.movement.target_position = Some(glam::Vec3::new(99.0, 0.0, 99.0));
            o.attack_substate = crate::game_logic::AttackSubState::AimAtTarget;
        }
    };
    let ent = shadow.world().entity(eid).expect("e");
    assert_eq!(
        ent.move_target, None,
        "poking HashMap without commit must not change GameWorld dest"
    );
    assert_eq!(
        logic.host_authoritative_move_dest(carrier),
        Some([99.0, 0.0, 99.0]),
        "ordinary dest query always observes the same owner"
    );
}

#[test]
fn world_tick_split_borrow_still_reads_frame() {
    // world_tick mutates via objects.get_mut (map field) while reading self.frame.
    let src = include_str!("../../game_logic/world_tick/attack.rs");
    assert!(
        src.contains("self.objects.get_mut(") && src.contains("self.frame"),
        "mid-frame attack tick must keep HashMap field borrow + frame read"
    );
    let src = include_str!("../../game_logic/world_objects/host_ops_writeback.rs");
    assert!(
        src.contains("let frame = self.frame") && src.contains("self.objects.get_mut(&id)"),
        "host_stamp_attack_substate_at_frame must split-borrow objects + frame"
    );
    let store = concat!(
        include_str!("../../game_logic/game_logic/crate_tick.rs"),
        include_str!("../../game_logic/game_logic/player.rs"),
        include_str!("../../game_logic/game_logic/host.rs"),
        include_str!("../../game_logic/game_logic/script_camera.rs"),
        include_str!("../../game_logic/game_logic/authority.rs"),
        include_str!("../../game_logic/game_logic/construct.rs"),
        include_str!("../../game_logic/game_logic/mod.rs"),
        include_str!("../../game_logic/game_logic/host_object_store.rs"),
    );
    let owner_module = include_str!("../../game_logic/game_logic/mod.rs");
    assert!(
        owner_module.contains("mod host_object_store;")
            && owner_module
                .contains("pub use host_object_store::{HostObjectRead, HostObjectStore};"),
        "the canonical GameLogic module must wire the actual owned store definition"
    );
    assert!(
        store.contains("pub objects: HostObjectStore")
            && store.contains("struct HostObjectStore")
            && store.contains("pub struct HostObjectRead")
            && store.contains("pub fn read(&self)")
            && !store.contains("impl std::ops::Deref for HostObjectStore"),
        "objects must be a HostObjectStore field (split-borrow), not a raw HashMap field"
    );
}

#[test]
fn host_object_store_mutation_is_canonical_without_shadow_writeback() {
    crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", "1");
    assert!(
        matches!(
            crate::authoritative_world::dual_tick_policy(),
            crate::authoritative_world::DualTickPolicy::AuthorityOnly
        ),
        "production dual_tick_policy stays AuthorityOnly"
    );

    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("HostStore");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    ensure_template(&mut logic, "HostStoreU", 80.0);
    let oid = logic
        .create_object("HostStoreU", Team::USA, glam::Vec3::new(2.0, 0.0, 3.0))
        .expect("id");
    let victim = logic
        .create_object("HostStoreU", Team::USA, glam::Vec3::new(8.0, 0.0, 1.0))
        .expect("victim");

    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);

    {
        logic
            .with_host_object_mut(oid, |o| {
                o.health.current = 60.0;
                let mut w = crate::game_logic::Weapon::default();
                w.clip_size = 6;
                w.ammo = Some(4);
                o.weapon = Some(w);
                o.movement.target_position = Some(glam::Vec3::new(11.0, 0.0, 5.0));
                o.target = Some(victim);
            })
            .expect("shipped mutate");
        let stamped = logic
            .host_stamp_attack_substate_at_frame(oid, crate::game_logic::AttackSubState::FireWeapon)
            .expect("split-borrow store + frame");
        assert_eq!(stamped, logic.frame);
    };

    let spawn_hp = logic.host_authoritative_health(oid);
    assert_eq!(
        spawn_hp,
        Some(60.0),
        "owned body mutation is immediately visible"
    );
    let eid = shadow.entity_for_host(oid).expect("map");
    let ent = shadow.world().entity(eid).expect("e");
    assert_ne!(ent.health, 60.0);
    assert_ne!(ent.move_target, Some([11.0, 0.0, 5.0]));
    assert_ne!(ent.attack_target, shadow.entity_for_host(victim));

    if let Some(o) = logic.objects.get_mut(&oid) {
        o.health.current = 1.0;
        if let Some(w) = o.weapon.as_mut() {
            w.ammo = Some(1);
        }
        o.movement.target_position = Some(glam::Vec3::new(99.0, 0.0, 99.0));
        o.target = Some(ObjectId(9999));
    }
    assert_eq!(
        logic.host_authoritative_health(oid),
        Some(1.0),
        "ordinary query must read the same canonical body"
    );
    assert_ne!(shadow.world().entity(eid).expect("e").health, 1.0);

    // Split-borrow: mutate store field while reading frame.
    let frame = logic.frame;
    let obj = logic.objects.get_mut(&oid).expect("store");
    obj.attack_substate = crate::game_logic::AttackSubState::AimAtTarget;
    assert_eq!(frame, logic.frame);
}

/// C++ Object HP is its BodyModule. A comparison session cannot replace it.
#[test]
fn is_alive_uses_owned_body_even_with_conflicting_comparison_health() {
    crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", "1");

    let mut logic = GameLogic::new();

    let cfg = golden_skirmish_config("AliveAuth");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    ensure_template(&mut logic, "AliveAuthU", 100.0);
    let oid = logic
        .create_object("AliveAuthU", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
        .expect("id");

    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);

    {
        let eid = shadow.entity_for_host(oid).expect("map");
        if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
            e.health = 0.0;
        }
    }
    // HashMap still shows a living unit.
    {
        assert!(
            logic
                .host_object(oid)
                .is_some_and(|o| o.health.current > 0.0)
        );
        assert!(
            logic.host_object(oid).expect("obj").is_alive(),
            "ordinary is_alive observes the canonical owned body"
        );
        assert!(
            (logic.host_object(oid).expect("obj").get_health_percentage() - 1.0).abs() < 1e-5,
            "health percentage observes the canonical owned body"
        );
    };
}

/// C++ Object::xfer writes getBodyModule()->getHealth(). Snapshot must do the same.
#[test]
fn snapshot_builder_uses_authoritative_health() {
    crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", "1");

    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("SnapAuth");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    ensure_template(&mut logic, "SnapAuthU", 100.0);
    let oid = logic
        .create_object("SnapAuthU", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
        .expect("id");

    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);

    {
        let eid = shadow.entity_for_host(oid).expect("map");
        if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
            e.health = 37.0;
        }
    }
    if let Some(o) = logic.objects.get_mut(&oid) {
        o.health.current = 99.0;
    }

    let snap = {
        crate::save_load::snapshot::SnapshotBuilder::new()
            .create_world_snapshot(&logic)
            .expect("snapshot")
    };
    let obj_snap = snap.objects.get(&oid).expect("snap obj");
    assert!(
        (obj_snap.health.current - 99.0).abs() < 1e-4,
        "snapshot must serialize the canonical owned body, got {}",
        obj_snap.health.current
    );
}

#[test]
fn no_shadow_boundary_does_not_replay_committed_damage_and_is_idempotent() {
    let mut logic = GameLogic::new();
    apply_skirmish_config(&mut logic, &golden_skirmish_config("NoShadowDamage")).expect("config");
    ensure_template(&mut logic, "NoShadowDamageTarget", 100.0);
    logic
        .templates
        .get_mut("NoShadowDamageTarget")
        .unwrap()
        .authored_instant_death = Some(vec![Default::default()]);

    let ordinary = logic
        .create_object("NoShadowDamageTarget", Team::USA, Vec3::new(1.0, 0.0, 1.0))
        .expect("ordinary target");
    let zero = logic
        .create_object("NoShadowDamageTarget", Team::USA, Vec3::new(2.0, 0.0, 1.0))
        .expect("zero target");
    let lethal = logic
        .create_object("NoShadowDamageTarget", Team::USA, Vec3::new(3.0, 0.0, 1.0))
        .expect("lethal target");

    logic.health_events.clear_damage();

    // Production Object::take_damage path commits this owner mutation at once.
    {
        let (object, health_events) = logic
            .host_object_and_health_events_mut(ordinary)
            .expect("ordinary");
        assert!(!object.status.destroyed);
        assert!(!object.take_damage(20.0, health_events));
        assert_eq!(object.health.current, 80.0);
        assert_eq!(object.previous_health, 100.0);
        assert!(!object.status.destroyed);
    }
    let ordinary_event = logic.health_events.snapshot_damage();
    assert_eq!(ordinary_event.len(), 1);
    assert_eq!(ordinary_event[0].target, ordinary);
    assert_eq!(ordinary_event[0].amount, 20.0);
    assert!(!ordinary_event[0].destroyed);

    // Main's no-session branch calls this exact boundary. It must drain the
    // residual without applying already committed HP a second time.
    crate::gameworld_shadow::run_owned_host_boundary(None, &mut logic);
    {
        let object = logic
            .host_object(ordinary)
            .expect("ordinary after boundary");
        assert_eq!(object.health.current, 80.0);
        assert_eq!(object.previous_health, 100.0);
        assert!(!object.status.destroyed);
    }
    assert!(logic.health_events.snapshot_damage().is_empty());

    // A second no-session boundary is a no-op after the event has been drained.
    crate::gameworld_shadow::run_owned_host_boundary(None, &mut logic);
    assert_eq!(logic.host_object(ordinary).unwrap().health.current, 80.0);
    assert_eq!(logic.host_object(ordinary).unwrap().previous_health, 100.0);
    assert!(!logic.host_object(ordinary).unwrap().status.destroyed);

    // Zero damage produces no damage event and must leave health/history alone.
    let zero_previous = logic.host_object(zero).unwrap().previous_health;
    {
        let (object, health_events) = logic.host_object_and_health_events_mut(zero).expect("zero");
        assert!(!object.take_damage(0.0, health_events));
        assert_eq!(object.health.current, 100.0);
        assert_eq!(object.previous_health, zero_previous);
    }
    assert!(logic.health_events.snapshot_damage().is_empty());
    crate::gameworld_shadow::run_owned_host_boundary(None, &mut logic);
    assert_eq!(logic.host_object(zero).unwrap().health.current, 100.0);
    assert_eq!(
        logic.host_object(zero).unwrap().previous_health,
        zero_previous
    );
    assert!(!logic.host_object(zero).unwrap().status.destroyed);

    // The actual world's onDie dispatch executes the explicitly authored
    // InstantDeath callback before physical removal at the frame boundary.
    perform_owner_damage(&mut logic, lethal, 1000.0, None);
    let object = logic.host_object(lethal).unwrap();
    assert_eq!(object.health.current, 0.0);
    assert_eq!(object.previous_health, 100.0);
    assert!(object.status.destroyed);
    let lethal_event = logic.health_events.snapshot_damage();
    assert_eq!(lethal_event.len(), 1);
    assert_eq!(lethal_event[0].target, lethal);
    assert!(lethal_event[0].destroyed);
    crate::gameworld_shadow::run_owned_host_boundary(None, &mut logic);
    assert!(logic.host_object(lethal).unwrap().status.destroyed);
    assert_eq!(logic.host_object(lethal).unwrap().health.current, 0.0);
    assert!(logic.objects.ordered_ids().contains(&lethal));
    assert!(logic.has_pending_destroy_work());
    let completed = logic.health_events.snapshot_last_damage();
    assert_eq!(completed.len(), 2);
    assert_eq!(completed[0].target, ordinary);
    assert!(!completed[0].destroyed);
    assert_eq!(completed[1].target, lethal);
    assert!(completed[1].destroyed);
    // CPP GameLogic.cpp3762 is the physical endphase, before the next frame.
    logic
        .process_destroy_list_with_effects_checked(&mut Default::default())
        .unwrap();
    crate::gameworld_shadow::run_owned_host_boundary(None, &mut logic);
    assert!(logic.host_object(lethal).is_none());
    assert!(!logic.has_pending_destroy_work());
    assert_eq!(logic.health_events.snapshot_last_damage(), completed);
    crate::gameworld_shadow::run_owned_host_boundary(None, &mut logic);
    assert!(logic.host_object(lethal).is_none());
    assert!(!logic.has_pending_destroy_work());
    assert_eq!(logic.health_events.snapshot_last_damage(), completed);

    logic.health_events.clear_damage();
}

#[test]
fn no_shadow_boundary_preserves_committed_heal_damage_order() {
    let mut logic = GameLogic::new();
    apply_skirmish_config(&mut logic, &golden_skirmish_config("HealDamageOrder")).expect("config");
    ensure_template(&mut logic, "HealDamageOrderTarget", 200.0);
    for heal_first in [true, false] {
        logic.health_events.clear_damage();
        logic.health_events.clear_heal();
        let id = logic
            .create_object("HealDamageOrderTarget", Team::USA, Vec3::ZERO)
            .expect("target");
        let previous;
        {
            let body_frame = logic.frame;
            let (object, health_events) =
                logic.host_object_and_health_events_mut(id).expect("target");
            object.health.current = 40.0;
            object.previous_health = 40.0;
            if heal_first {
                object.heal(50.0, health_events, body_frame);
                assert_eq!(object.health.current, 90.0);
                assert!(!object.take_damage(20.0, health_events));
                previous = 90.0;
            } else {
                assert!(!object.take_damage(20.0, health_events));
                assert_eq!(object.health.current, 20.0);
                object.heal(50.0, health_events, body_frame);
                previous = 20.0;
            }
            assert_eq!(object.health.current, 70.0);
            assert_eq!(object.previous_health, previous);
        }
        assert_eq!(logic.health_events.snapshot_damage().len(), 1);
        assert_eq!(logic.health_events.snapshot_heal().len(), 1);
        for _ in 0..2 {
            crate::gameworld_shadow::run_owned_host_boundary(None, &mut logic);
            let object = logic.host_object(id).expect("target after boundary");
            assert_eq!(object.health.current, 70.0, "heal_first={heal_first}");
            assert_eq!(object.previous_health, previous);
            assert!(!object.status.destroyed);
        }
        assert!(logic.health_events.snapshot_damage().is_empty());
        assert!(logic.health_events.snapshot_heal().is_empty());
    }
    logic.health_events.clear_damage();
    logic.health_events.clear_heal();
}

#[test]
fn no_shadow_boundary_preserves_completed_coupled_healing() {
    let mut logic = GameLogic::new();

    apply_skirmish_config(&mut logic, &golden_skirmish_config("PendingHealing")).expect("config");
    ensure_template(&mut logic, "PendingHealingTarget", 100.0);
    let id = logic
        .create_object("PendingHealingTarget", Team::USA, Vec3::ZERO)
        .expect("target");
    logic.host_object_mut(id).unwrap().health.current = 40.0;
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    logic.health_events.clear_heal();
    {
        {
            owner_heal(&mut logic, id, 30.0);
            assert_eq!(logic.host_object(id).unwrap().health.current, 70.0);
        };
    };
    let events = logic.health_events.snapshot_heal();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].health, 70.0);
    for _ in 0..2 {
        crate::gameworld_shadow::run_owned_host_boundary(None, &mut logic);
        assert_eq!(logic.host_object(id).unwrap().health.current, 70.0);
        assert!(!logic.host_object(id).unwrap().status.destroyed);
    }
    assert!(logic.health_events.snapshot_heal().is_empty());
    logic.health_events.clear_heal();
}

#[test]
fn coupled_tick_group_speed_reads_script_damage_from_host_owner_before_shadow_boundary() {
    use crate::command_executor::CommandExecutor;
    use crate::command_system::{CommandResult, CommandType, GameCommand, ModifierKeys};
    use crate::game_logic::{GameLogic, KindOf, Player, Team, ThingTemplate};
    use gamelogic::scripting::{
        HostScriptKillDeleteDamageRequest, request_host_script_kill_delete_damage,
    };
    use glam::Vec3;

    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "USA", true));
    logic.scripts_loaded = true;
    let mut template = ThingTemplate::new("OwnerGroupSpeedVehicle");
    template
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .set_health(100.0);
    logic.templates.insert(template.name.clone(), template);

    let fast = logic
        .create_object("OwnerGroupSpeedVehicle", Team::USA, Vec3::ZERO)
        .expect("fast member");
    let slow = logic
        .create_object(
            "OwnerGroupSpeedVehicle",
            Team::USA,
            Vec3::new(20.0, 0.0, 0.0),
        )
        .expect("slow member");
    {
        let object = logic.host_object_mut(fast).expect("fast");
        object.movement.max_speed = 20.0;
        object.movement.max_speed_damaged = 20.0;
        object.health.current = 100.0;
        object.health.maximum = 100.0;
        object.refresh_model_condition_bits();
    }
    {
        let object = logic.host_object_mut(slow).expect("slow");
        object.movement.max_speed = 10.0;
        object.movement.max_speed_damaged = 10.0;
        object.health.current = 100.0;
        object.health.maximum = 100.0;
        object.name = "SlowOwnerPhaseMember".into();
        object.refresh_model_condition_bits();
    }

    // Use the actual command executor to establish the authored formation
    // stamp; the later Move is processed by GameLogic::process_commands.
    let formation = CommandExecutor::new(&mut logic, 0).execute_create_formation(&[fast, slow]);
    assert_eq!(formation, CommandResult::Success);

    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let slow_eid = shadow.entity_for_host(slow).expect("slow shadow mapping");
    request_host_script_kill_delete_damage(HostScriptKillDeleteDamageRequest::NamedDamage {
        unit: "SlowOwnerPhaseMember".into(),
        amount: 70,
    });
    logic.queue_command(GameCommand {
        command_type: CommandType::Move {
            destination: Vec3::new(200.0, 0.0, 0.0),
        },
        player_id: 0,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![fast, slow],
        modifier_keys: ModifierKeys::default(),
    });

    {
        // One real Advanced update: begin sync → script damage → queued Move →
        // formation group_speed, before any Main post-step shadow boundary.
        logic.tick_logic_frame_with_boundary(1.0 / 30.0, None, None, |_| {});

        let fast_object = logic.host_objects().lookup(&fast).expect("fast after move");
        let slow_object = logic
            .host_objects()
            .lookup(&slow)
            .expect("slow after damage");
        assert_eq!(
            slow_object.health.current, 30.0,
            "named damage applied on host"
        );
        assert_eq!(
            fast_object.group_speed_factor, 1.0,
            "the really-damaged 10-speed member is excluded; healthy group speed is 20"
        );
        assert!(
            logic
                .health_events
                .snapshot_damage()
                .iter()
                .any(|event| event.target == slow),
            "the in-step script damage passed through the production damage channel"
        );

        // Ordinary body queries stay canonical after the step as well.
        // The independent comparison remains at its original 100 HP.
        assert_eq!(logic.host_authoritative_health(slow), Some(30.0));
        let shadow_hp = Some((|active: &GameWorldShadow| {
            active.world().entity(slow_eid).expect("slow entity").health
        })(&shadow))
        .expect("active shadow");
        assert_eq!(shadow_hp, 100.0);
    };
    logic.health_events.clear_damage();
}

#[test]
fn host_logic_step_active_keeps_repeated_object_mutations_and_getters_on_owner() {
    let mut logic = GameLogic::new();
    apply_skirmish_config(&mut logic, &golden_skirmish_config("OwnerPhase")).expect("config");
    ensure_template(&mut logic, "OwnerPhaseUnit", 80.0);

    let source = logic
        .create_object("OwnerPhaseUnit", Team::USA, Vec3::new(1.0, 0.0, 2.0))
        .expect("source");
    let target = logic
        .create_object("OwnerPhaseUnit", Team::USA, Vec3::new(5.0, 0.0, 6.0))
        .expect("target");
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let source_eid = shadow.entity_for_host(source).expect("source mapping");
    let owner_position = [17.0, 0.0, 19.0];
    let final_owner_position = [23.0, 0.0, 29.0];

    {
        assert_eq!(logic.host_authoritative_target(source), None);
        let before = Some((|active: &GameWorldShadow| {
            active
                .world()
                .entity(source_eid)
                .expect("source")
                .transform
                .position
        })(&shadow))
        .expect("active shadow");
        assert_eq!([before.x, before.y, before.z], [1.0, 0.0, 2.0]);

        logic
            .with_host_logic_step(|owner| {
                {
                    let (object, attack_events) = owner
                        .host_object_and_attack_events_mut(source)
                        .expect("first owner mutation");
                    object.set_order_target(
                        Some(target),
                        attack_events,
                        &mut crate::game_logic::ObjectAudio::queue(&mut Vec::new()),
                    );
                    object.set_position(glam::Vec3::from_array(owner_position));
                }

                // A second ordinary mutable borrow used to re-overlay the old
                // GameWorld pose/target, erasing the first host operation.
                owner
                    .with_host_object_mut(source, |object| {
                        assert_eq!(object.target, Some(target));
                        assert_eq!(
                            object.get_position().to_array(),
                            owner_position,
                            "the first host mutation must survive the second borrow"
                        );
                        object.set_position(glam::Vec3::from_array(final_owner_position));
                    })
                    .expect("second owner mutation");

                assert_eq!(
                    owner.host_authoritative_target(source),
                    Some(target),
                    "in-phase target reads must use the same live Object"
                );
                assert_eq!(
                    owner.host_authoritative_pose(source),
                    Some(final_owner_position),
                    "in-phase pose reads must use the same live Object"
                );
                assert_eq!(
                    owner.host_authoritative_pose(target),
                    Some([5.0, 0.0, 6.0]),
                    "unmodified owner data remains intact"
                );

                // A nested phase cannot replace/reset the active guard. It is
                // rejected while the original owner scope remains active.
                assert!(owner.with_host_logic_step(|_| ()).is_none());
                assert_eq!(owner.host_authoritative_target(source), Some(target));
            })
            .expect("enter host-owned post-sync phase");

        // Leaving the reentry guard never changes the source of ordinary reads.
        assert_eq!(logic.host_authoritative_target(source), Some(target));
        assert_eq!(
            logic.host_authoritative_pose(source),
            Some(final_owner_position)
        );
        let source_after = Some((|active: &GameWorldShadow| {
            active
                .world()
                .entity(source_eid)
                .expect("source after")
                .transform
                .position
        })(&shadow))
        .expect("active shadow after phase");
        assert_eq!(source_after, glam::Vec3::new(1.0, 0.0, 2.0));
    };
}

#[test]
fn host_logic_step_active_guard_restores_on_return_and_unwind() {
    let mut logic = GameLogic::new();
    logic
        .with_host_logic_step(|_| ())
        .expect("first phase entry");
    assert!(logic.with_host_logic_step(|_| ()).is_some());

    let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = logic.with_host_logic_step(|_| panic!("exercise guard drop"));
    }));
    assert!(unwind.is_err());
    assert!(logic.with_host_logic_step(|_| ()).is_some());
}

#[test]
fn update_simulation_enters_owned_step_without_shadow_ingress() {
    let wrapper = rust_fn_body(GAME_LOGIC_HOST_SRC, "update_simulation").expect("step wrapper");
    assert!(wrapper.contains("self.update_simulation_with_script_effects(dt, Default::default())"));
    let with_effects = rust_fn_body(GAME_LOGIC_HOST_SRC, "update_simulation_with_script_effects")
        .expect("receiving-effects step delegate");
    assert!(with_effects.contains(
        "self.update_simulation_with_command_phase(dt, ui, CommandPhaseMode::Completed)"
    ));
    let command_phase = rust_fn_body(GAME_LOGIC_HOST_SRC, "update_simulation_with_command_phase")
        .expect("shared owned command-phase authority");
    assert!(command_phase.contains("self.with_host_logic_step"));
    assert!(command_phase.contains("logic.update_simulation_in_step(dt, ui, command_phase)"));
    for delegate in [wrapper, with_effects, command_phase] {
        assert!(
            !delegate.contains("sync_authoritative_view_from_gameworld"),
            "each actual step delegate must retain owned ingress without shadow recapture"
        );
    }
    let operations = include_str!("../../game_logic/world_objects/host_ops_writeback.rs");
    assert!(!operations.contains("overlay_object_from_gameworld"));
    assert!(!operations.contains("commit_dirty_host_objects_to_gameworld"));

    let body =
        rust_fn_body(GAME_LOGIC_HOST_SRC, "update_simulation_in_step").expect("owned phases");
    let scripts = body
        .find("evaluate_and_execute_scripts_with_effects(dt, ui.reborrow())")
        .expect("scripts using the receiving effects borrow");
    assert!(
        scripts
            < body
                .find("simulation_frozen_with_script_engine(None)")
                .expect("freeze using the restored receiving interpreter")
    );
}

#[test]
fn canonical_attack_receipts_keep_owner_fifo_across_same_id_peer_lifecycle() {
    fn world() -> (GameLogic, ObjectId, ObjectId, ObjectId) {
        let mut logic = GameLogic::new();
        logic.add_player(crate::game_logic::Player::new(
            0,
            Team::USA,
            "AttackOwner",
            true,
        ));
        logic.add_player(crate::game_logic::Player::new(1, Team::GLA, "Enemy", false));
        let mut template = ThingTemplate::new("OwnedAttackReceiptInfantry");
        template
            .add_kind_of(KindOf::Infantry)
            .add_kind_of(KindOf::Attackable)
            .set_health(100.0);
        logic.templates.insert(template.name.clone(), template);
        let attacker = logic
            .create_object_for_player("OwnedAttackReceiptInfantry", 0, Vec3::ZERO)
            .unwrap();
        let first = logic
            .create_object_for_player("OwnedAttackReceiptInfantry", 1, Vec3::new(10.0, 0.0, 0.0))
            .unwrap();
        let second = logic
            .create_object_for_player("OwnedAttackReceiptInfantry", 1, Vec3::new(20.0, 0.0, 0.0))
            .unwrap();
        (logic, attacker, first, second)
    }
    fn order(logic: &mut GameLogic, attacker: ObjectId, target: ObjectId) {
        let (object, output) = logic.host_object_and_attack_events_mut(attacker).unwrap();
        object.attack_target(
            target,
            output,
            &mut crate::game_logic::ObjectAudio::queue(&mut Vec::new()),
        );
        assert_eq!(object.target, Some(target));
        assert_eq!(object.ai_state, crate::game_logic::AIState::Attacking);
        assert!(object.status.attacking);
        logic.attack_events.complete_boundary();
    }
    let (mut owner, attacker, first, second) = world();
    let (mut peer, peer_attacker, peer_first, peer_second) = world();
    assert_eq!(
        (attacker, first, second),
        (peer_attacker, peer_first, peer_second)
    );
    order(&mut owner, attacker, first);
    order(&mut peer, peer_attacker, peer_second);
    assert_eq!(peer.attack_events.take_completed().len(), 1);
    peer.reset();
    order(&mut owner, attacker, second);
    let builder = crate::save_load::SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&owner).unwrap();
    let (mut peer, peer_attacker, peer_first, _) = world();
    order(&mut peer, peer_attacker, peer_first);
    builder.restore_from_snapshot(&snapshot, &mut peer).unwrap();
    assert!(peer.attack_events.take_completed().is_empty());
    assert_eq!(peer.host_object(attacker).unwrap().target, Some(second));
    owner
        .host_object_mut(attacker)
        .unwrap()
        .stop_attack(&mut crate::game_logic::ObjectAudio::queue(&mut Vec::new()));
    assert!(owner.host_object(attacker).unwrap().target.is_none());
    assert!(!owner.host_object(attacker).unwrap().status.attacking);
    owner.attack_events.complete_boundary();
    let frame = crate::presentation_frame::PresentationFrame::build_from_logic(&mut owner, 0);
    let targeted = frame
        .events
        .iter()
        .filter_map(|event| match event {
            generals_presentation::events::PresentationEvent::AttackTargeted {
                attacker,
                target,
            } => Some((*attacker, *target)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        targeted,
        vec![(attacker, Some(first)), (attacker, Some(second))]
    );
    let repeated = crate::presentation_frame::PresentationFrame::build_from_logic(&mut owner, 0);
    assert!(!repeated.events.iter().any(|event| matches!(
        event,
        generals_presentation::events::PresentationEvent::AttackTargeted { .. }
    )));
}

#[test]
fn interleaved_creation_and_peer_drop_stay_with_the_receiving_host() {
    let mut a = GameLogic::new();
    let mut b = GameLogic::new();
    ensure_template(&mut a, "OwnedSpawnA", 80.0);
    ensure_template(&mut b, "OwnedSpawnB", 90.0);
    let a_id = a
        .create_object("OwnedSpawnA", Team::USA, Vec3::new(11.0, 0.0, 12.0))
        .unwrap();
    let b_id = b
        .create_object("OwnedSpawnB", Team::China, Vec3::new(21.0, 0.0, 22.0))
        .unwrap();
    assert_eq!(
        a_id, b_id,
        "fixture deliberately overlaps session-local identities"
    );
    let mut a_shadow = GameWorldShadow::new(64);
    let mut b_shadow = GameWorldShadow::new(64);
    a_shadow.sync_from_host(&a);
    b_shadow.sync_from_host(&b);
    let a_entity = a_shadow.entity_for_host(a_id).unwrap();
    let b_entity = b_shadow.entity_for_host(b_id).unwrap();
    assert_eq!(
        a_shadow
            .world()
            .entity(a_entity)
            .unwrap()
            .transform
            .position,
        a.host_object(a_id).unwrap().get_position().into()
    );
    assert_eq!(
        b_shadow
            .world()
            .entity(b_entity)
            .unwrap()
            .transform
            .position,
        b.host_object(b_id).unwrap().get_position().into()
    );
    assert_eq!(a.host_object(a_id).unwrap().template_name, "OwnedSpawnA");
    assert_eq!(b.host_object(b_id).unwrap().template_name, "OwnedSpawnB");
    b.host_object_mut(b_id)
        .unwrap()
        .set_position(Vec3::new(31.0, 0.0, 32.0));
    b_shadow.sync_from_host(&b);
    a_shadow.sync_from_host(&a);
    assert_eq!(
        a.host_object(a_id).unwrap().get_position(),
        Vec3::new(11.0, 0.0, 12.0)
    );
    assert_eq!(
        a_shadow
            .world()
            .entity(a_entity)
            .unwrap()
            .transform
            .position,
        a.host_object(a_id).unwrap().get_position().into()
    );
    drop(b_shadow);
    drop(b);
    a_shadow.sync_from_host(&a);
    assert!(a.host_object(a_id).is_some());
    assert_eq!(a_shadow.entity_for_host(a_id), Some(a_entity));
}
