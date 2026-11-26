//! Canonical Main fire, damage, timers and AI state; projections only observe.

use super::*;

fn owner_damage(logic: &mut GameLogic, id: ObjectId, amount: f32) -> bool {
    let (object, health_events) = logic.host_object_and_health_events_mut(id).unwrap();
    object.take_damage(amount, 0, health_events)
}

#[test]
fn assign_unit_path_decision_authority() {
    use crate::game_logic::{AIState, KindOf, Team, ThingTemplate};

    let mut logic = GameLogic::new();

    let cfg = golden_skirmish_config("PathMv");
    crate::fixture_content::admit_fixture_objects(
        &mut logic,
        &[
            crate::fixture_content::FixtureObject::new("PmU").kind_of("INFANTRY SELECTABLE").health(100.0).walker(),
        ],
    );
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    let oid = logic
        .create_object("PmU", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
        .expect("id");
    if let Some(o) = logic.host_object_mut(oid) {
        // Ensure mobile residual (max_speed > 0).
        o.movement.max_speed = 20.0;
    }
    let ok = logic.assign_unit_path_for_test(oid, glam::Vec3::new(50.0, 0.0, 0.0), &[]);
    assert!(ok, "path assign should succeed");

    assert_eq!(
        logic.host_objects().lookup(&oid).unwrap().ai_state(),
        AIState::Moving,
        "host Moving immediate"
    );
    assert!(
        logic.host_objects().lookup(&oid).unwrap().status.moving
            || logic
                .host_objects()
                .lookup(&oid)
                .unwrap()
                .movement
                .target_position
                .is_some()
            || !logic
                .host_objects()
                .lookup(&oid)
                .unwrap()
                .movement
                .path
                .is_empty(),
        "movement residual still on host"
    );
    let mut shadow = GameWorldShadow::new();
    shadow.sync_from_host(&logic);

    assert_eq!(
        logic.host_objects().lookup(&oid).unwrap().ai_state(),
        AIState::Moving
    );
}

#[test]
fn append_unit_waypoint_decision_authority() {
    use crate::game_logic::{AIState, KindOf, Team, ThingTemplate};

    let mut logic = GameLogic::new();

    let cfg = golden_skirmish_config("WpAuth");
    crate::fixture_content::admit_fixture_objects(
        &mut logic,
        &[
            crate::fixture_content::FixtureObject::new("WpU").kind_of("INFANTRY").health(100.0).walker(),
        ],
    );
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    let oid = logic
        .create_object("WpU", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
        .expect("id");
    if let Some(o) = logic.host_object_mut(oid) {
        o.movement.max_speed = 20.0;
    }
    assert!(logic.append_unit_waypoint_for_test(oid, glam::Vec3::new(30.0, 0.0, 0.0)));

    assert_eq!(
        logic.host_objects().lookup(&oid).unwrap().ai_state(),
        AIState::Moving,
        "host Moving immediate"
    );
    let mut shadow = GameWorldShadow::new();
    shadow.sync_from_host(&logic);

    assert_eq!(
        logic.host_objects().lookup(&oid).unwrap().ai_state(),
        AIState::Moving
    );
}

#[test]
fn set_ai_state_decision_aware_writeback() {
    use crate::game_logic::{AIState, KindOf, Team, ThingTemplate};

    let mut logic = GameLogic::new();

    let cfg = golden_skirmish_config("StateAw");
    logic.admit_object_ini_sources([("fixture-objects.ini".into(), "Object SaU\nEnd\n".into())]).expect("fixture Object content admitted before NewGame");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    {
        let mut t = ThingTemplate::new("SaU");
        t.add_kind_of(KindOf::Infantry);
        logic.templates.insert("SaU".into(), t);
    }
    let oid = logic
        .create_object("SaU", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
        .expect("id");
    logic.set_ai_state_decision_aware_for_test(oid, AIState::Gathering);

    // C++ AIUpdate applies this state synchronously to the receiving owner.
    assert_eq!(
        logic.host_objects().lookup(&oid).unwrap().ai_state(),
        AIState::Gathering
    );
    let mut shadow = GameWorldShadow::new();
    shadow.sync_from_host(&logic);

    assert_eq!(
        logic.host_objects().lookup(&oid).unwrap().ai_state(),
        AIState::Gathering
    );
}

#[test]
fn death_type_channel_via_set_death_type() {
    use crate::game_logic::host_usa_pilot::HostDeathType;
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("DeathTy");
    logic.admit_object_ini_sources([("fixture-objects.ini".into(), "Object DieUnit\nEnd\n".into())]).expect("fixture Object content admitted before NewGame");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    {
        let mut t = ThingTemplate::new("DieUnit");
        t.add_kind_of(KindOf::Infantry);
        logic.templates.insert("DieUnit".into(), t);
    }
    let oid = logic
        .create_object("DieUnit", Team::USA, glam::Vec3::new(7.0, 0.0, 7.0))
        .expect("id");
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.status.destroyed = true;
        o.status.death_type = HostDeathType::Burned;
    }
    let mut shadow = GameWorldShadow::new();
    shadow.sync_from_host(&logic);
    let eid = *shadow.host_to_entity.get(&oid.0).expect("map");

    assert_eq!(
        shadow.world().entity(eid).unwrap().death_type,
        HostDeathType::Burned.ordinal()
    );
    {
        let observer = shadow.world_mut().world_mut().entity_mut(eid).unwrap();
        observer.death_type = HostDeathType::Normal.ordinal();
    }

    shadow.sync_from_host(&logic);

    assert_eq!(
        logic.host_object(oid).unwrap().status.death_type,
        HostDeathType::Burned
    );
    assert_eq!(
        shadow.world().entity(eid).unwrap().death_type,
        HostDeathType::Burned.ordinal()
    );
}

#[test]
fn radar_extend_channel_via_set_radar_extend() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};

    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("RadarEx");
    logic.admit_object_ini_sources([("fixture-objects.ini".into(), "Object RadarB\nEnd\n".into())]).expect("fixture Object content admitted before NewGame");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    {
        let mut t = ThingTemplate::new("RadarB");
        t.add_kind_of(KindOf::Structure);
        logic.templates.insert("RadarB".into(), t);
    }
    let oid = logic
        .create_object("RadarB", Team::USA, glam::Vec3::new(8.0, 0.0, 8.0))
        .expect("id");
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.radar_extend_done_frame = 120;
        o.radar_extend_complete = false;
        o.radar_active = true;
    }

    let mut shadow = GameWorldShadow::new();
    shadow.sync_from_host(&logic);
    let eid = *shadow.host_to_entity.get(&oid.0).expect("map");

    let e = shadow.world().entity(eid).unwrap();
    assert_eq!(e.radar_extend_done_frame, 120);
    assert!(e.radar_active);
    assert!(!e.radar_extend_complete);
    {
        let observer = shadow.world_mut().world_mut().entity_mut(eid).unwrap();
        observer.radar_active = false;
        observer.radar_extend_done_frame = 0;
    }

    shadow.sync_from_host(&logic);

    let o = logic.host_objects().lookup(&oid).unwrap();
    assert!(o.radar_active);
    assert_eq!(o.radar_extend_done_frame, 120);
    assert_eq!(
        shadow.world().entity(eid).unwrap().radar_extend_done_frame,
        120
    );
}

#[test]
fn special_power_tick_updates_receiving_owner_timer() {
    // Host-only advance residual: disable SP sole-tick authority for this probe.

    use crate::game_logic::{KindOf, Team, ThingTemplate};

    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("SpTick");
    logic.admit_object_ini_sources([("fixture-objects.ini".into(), "Object SpUnit\nEnd\n".into())]).expect("fixture Object content admitted before NewGame");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    {
        let mut t = ThingTemplate::new("SpUnit");
        t.add_kind_of(KindOf::Infantry);
        logic.templates.insert("SpUnit".into(), t);
    }
    let oid = logic
        .create_object("SpUnit", Team::USA, glam::Vec3::new(1.0, 0.0, 1.0))
        .expect("id");
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.special_power_cooldown = 10.0;
        o.special_power_cooldown_remaining = 5.0;
        o.set_special_power_ready(false);
        let became = o.tick_timers(
            1.0,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
        let _ = became;
    }
    let owner = logic.host_object(oid).unwrap();
    assert!((owner.special_power_cooldown_remaining - 4.0).abs() < 1e-3);
    assert!(!owner.special_power_ready);
}

#[test]
fn special_power_timer_belongs_to_owner_not_projection() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};

    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("SpWb");
    logic.admit_object_ini_sources([("fixture-objects.ini".into(), "Object SpWbU\nEnd\n".into())]).expect("fixture Object content admitted before NewGame");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    {
        let mut t = ThingTemplate::new("SpWbU");
        t.add_kind_of(KindOf::Infantry);
        logic.templates.insert("SpWbU".into(), t);
    }
    let oid = logic
        .create_object("SpWbU", Team::USA, glam::Vec3::new(2.0, 0.0, 2.0))
        .expect("id");
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.special_power_cooldown = 10.0;
        o.special_power_cooldown_remaining = 2.0;
        o.set_special_power_ready(false);
    }
    let mut shadow = GameWorldShadow::new();
    shadow.sync_from_host(&logic);
    let projected = shadow
        .world()
        .entity(shadow.entity_for_host(oid).unwrap())
        .unwrap();
    assert_eq!(projected.special_power_cooldown_remaining, 2.0);
    // A stale or corrupt observation cannot change the owner timer.
    {
        let entity = shadow.entity_for_host(oid).unwrap();
        shadow
            .world_mut()
            .world_mut()
            .entity_mut(entity)
            .unwrap()
            .special_power_cooldown_remaining = 9.0;
    }

    let o = logic.host_objects().lookup(&oid).expect("o");
    assert!((o.special_power_cooldown_remaining - 2.0).abs() < 1e-3);
    shadow.sync_from_host(&logic);
    let entity = shadow.entity_for_host(oid).unwrap();
    assert_eq!(
        shadow
            .world()
            .entity(entity)
            .unwrap()
            .special_power_cooldown_remaining,
        2.0
    );
}

#[test]
fn owned_construction_events_map_completed_structure_without_consuming_output() {
    let mut logic = GameLogic::new();
    let mut t = ThingTemplate::new("USA_Barracks");
    t.set_health(1000.0);
    t.add_kind_of(KindOf::Structure);
    logic.templates.insert("USA_Barracks".into(), t);
    let id = logic
        .create_object("USA_Barracks", Team::USA, glam::Vec3::ZERO)
        .expect("barracks");
    // Simulate host recording construction complete without pre-sync map.
    let mut shadow = GameWorldShadow::new();
    // Do not sync first — apply construction should map via spawn residual.
    logic.construction_events.record(id, "USA_Barracks");
    let events = logic.construction_events.pending();
    assert_eq!(events.len(), 1);
    let completed = events.to_vec();
    shadow.sync_from_host(&logic);
    assert_eq!(
        logic.construction_events.pending(),
        completed,
        "observation cannot consume completion output"
    );
    assert!(
        shadow.entity_for_host(id).is_some(),
        "completed structure must be mapped in shadow"
    );
}

#[test]
fn dozer_bored_repair_state_decision_authority() {
    use crate::game_logic::{AIState, KindOf, Team, ThingTemplate};

    let mut logic = GameLogic::new();

    let cfg = golden_skirmish_config("DzAuth");
    logic.admit_object_ini_sources([("fixture-objects.ini".into(), "Object DzU\nEnd\n".into())]).expect("fixture Object content admitted before NewGame");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    {
        let mut t = ThingTemplate::new("DzU");
        t.add_kind_of(KindOf::Vehicle);
        t.add_kind_of(KindOf::Worker);
        logic.templates.insert("DzU".into(), t);
    }
    let oid = logic
        .create_object("DzU", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
        .expect("id");
    logic.set_ai_state_decision_aware_for_test(oid, AIState::Repairing);

    assert_eq!(
        logic.host_objects().lookup(&oid).unwrap().ai_state(),
        AIState::Repairing,
        "host AI state applies immediately"
    );
    let mut shadow = GameWorldShadow::new();
    shadow.sync_from_host(&logic);

    assert_eq!(
        logic.host_objects().lookup(&oid).unwrap().ai_state(),
        AIState::Repairing
    );
}

#[test]
fn hijacker_docked_state_decision_authority() {
    use crate::game_logic::{AIState, KindOf, Team, ThingTemplate};

    let mut logic = GameLogic::new();

    let cfg = golden_skirmish_config("HjAuth");
    logic.admit_object_ini_sources([("fixture-objects.ini".into(), "Object HjU\nEnd\n".into())]).expect("fixture Object content admitted before NewGame");
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
    let mut shadow = GameWorldShadow::new();
    shadow.sync_from_host(&logic);

    assert_eq!(
        logic.host_objects().lookup(&oid).unwrap().ai_state(),
        AIState::Docked
    );
}

#[test]
fn residual_auto_fire_damage_source_writeback_channel() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();

    let cfg = golden_skirmish_config("DmgSrc");
    logic.admit_object_ini_sources([("fixture-objects.ini".into(), "Object SrcA\nEnd\nObject SrcB\nEnd\n".into())]).expect("fixture Object content admitted before NewGame");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    for name in ["SrcA", "SrcB"] {
        {
            let mut t = ThingTemplate::new(name);
            t.add_kind_of(KindOf::Infantry);
            t.add_kind_of(KindOf::Attackable);
            t.set_health(100.0);
            logic.templates.insert(name.into(), t);
        }
    }
    let attacker = logic
        .create_object("SrcA", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
        .expect("a");
    let victim = logic
        .create_object("SrcB", Team::China, glam::Vec3::new(10.0, 0.0, 0.0))
        .expect("v");
    let kind = crate::game_logic::combat::DamageType::Unresistable;
    let context =
        crate::game_logic::object::DamageHitContext::new(logic.host_object(attacker), None, kind);
    logic
        .apply_owned_damage(
            victim,
            25.0,
            Some(attacker),
            kind,
            crate::game_logic::host_usa_pilot::HostDeathType::Normal,
            None,
            &context,
        )
        .unwrap();
    assert_eq!(
        logic.host_object(victim).unwrap().last_damage_source,
        Some(attacker)
    );
    // The actual receiving nonlethal owner write retains its source.
    let events = logic.health_events.drain_damage();
    assert!(
        events
            .iter()
            .any(|e| { e.target == victim && e.source == Some(attacker) && e.amount >= 20.0 }),
        "damage log must carry source; got {events:?}"
    );
    let mut shadow = GameWorldShadow::new();
    shadow.sync_from_host(&logic);
    assert_eq!(logic.host_object(victim).unwrap().health.current, 75.0);
    let entity = shadow.entity_for_host(victim).unwrap();
    assert_eq!(shadow.world().entity(entity).unwrap().health, 75.0);
}
