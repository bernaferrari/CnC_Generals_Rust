//! Canonical Main fire, damage, timers and AI state; projections only observe.

use super::*;

fn owner_damage(logic: &mut GameLogic, id: ObjectId, amount: f32) -> bool {
    let (object, health_events) = logic.host_object_and_health_events_mut(id).unwrap();
    object.take_damage(amount, health_events)
}

#[test]
fn fire_at_records_fire_intent_residual() {
    use crate::game_logic::host_fire_intent_log;
    use crate::game_logic::{KindOf, Team, ThingTemplate, Weapon};
    host_fire_intent_log::clear();
    let mut logic = GameLogic::new();

    let cfg = golden_skirmish_config("FireAtRec");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    logic.set_current_frame(77);
    if !logic.templates.contains_key("FrU") {
        let mut t = ThingTemplate::new("FrU");
        t.add_kind_of(KindOf::Infantry);
        logic.templates.insert("FrU".into(), t);
    }
    let oid = logic
        .create_object("FrU", Team::USA, glam::Vec3::new(10.0, 0.0, 10.0))
        .expect("id");
    let vid = logic
        .create_object("FrU", Team::China, glam::Vec3::new(12.0, 0.0, 10.0))
        .expect("v");
    {
        let _ = logic.host_object_mut(oid).expect("o");
        let o = logic.objects.get_mut(&oid).expect("o");
        o.weapon = Some(Weapon {
            damage: 15.0,
            range: 200.0,
            reload_time: 0.0,
            ..Weapon::default()
        });
        o.status.weapons_jammed = false;
        let fired = o.fire_at(
            vid,
            1.0,
            logic.frame,
            &mut logic.combat_system,
            &mut logic.attack_events,
        );
        assert!(fired, "close-range fire_at should discharge");
        // The shot commits its durable fields before any observer refresh.
        assert_eq!(o.last_fire_victim_host, vid.0);
        assert_eq!(o.last_fire_frame, 77);
        assert!(o.fire_intent_count >= 1, "counter still advances");
    }
    let evs = host_fire_intent_log::drain();
    assert!(
        evs.iter().any(|e| e.object == oid
            && e.last_fire_victim_host == vid.0
            && e.last_fire_frame == 77),
        "fire_at must log intent; got {evs:?}"
    );
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);

    let o = logic.host_objects().get(&oid).unwrap();
    assert_eq!(o.last_fire_victim_host, vid.0);
    assert_eq!(o.last_fire_frame, 77);
    assert!((o.last_fire_damage - 15.0).abs() < 1e-5);

    // A subsequent ordinary shot has the same immediate owner semantics.

    host_fire_intent_log::clear();
    {
        let _ = logic.host_object_mut(oid).expect("o");
        let o = logic.objects.get_mut(&oid).expect("o");
        o.last_fire_victim_host = 0;
        o.last_fire_frame = 0;
        o.last_fire_damage = 0.0;
        o.fire_intent_count = 0;
        // Ensure weapon ready again.
        if let Some(w) = o.weapon.as_mut() {
            w.last_fire_time = 0.0;
        }
        let fired = o.fire_at(
            vid,
            2.0,
            logic.frame,
            &mut logic.combat_system,
            &mut logic.attack_events,
        );
        assert!(fired);
        assert_eq!(o.last_fire_victim_host, vid.0);
        assert!(o.fire_intent_count >= 1);
    }
    assert!(!host_fire_intent_log::drain().is_empty());
}

#[test]
fn assign_unit_path_decision_authority() {
    use crate::game_logic::{AIState, KindOf, Team, ThingTemplate};

    let mut logic = GameLogic::new();

    let cfg = golden_skirmish_config("PathMv");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    if !logic.templates.contains_key("PmU") {
        let mut t = ThingTemplate::new("PmU");
        t.add_kind_of(KindOf::Infantry);
        t.add_kind_of(KindOf::Selectable);
        logic.templates.insert("PmU".into(), t);
    }
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
        logic.host_objects().get(&oid).unwrap().ai_state,
        AIState::Moving,
        "host Moving immediate"
    );
    assert!(
        logic.host_objects().get(&oid).unwrap().status.moving
            || logic
                .host_objects()
                .get(&oid)
                .unwrap()
                .movement
                .target_position
                .is_some()
            || !logic
                .host_objects()
                .get(&oid)
                .unwrap()
                .movement
                .path
                .is_empty(),
        "movement residual still on host"
    );
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);

    assert_eq!(
        logic.host_objects().get(&oid).unwrap().ai_state,
        AIState::Moving
    );
}

#[test]
fn private_idle_decision_authority() {
    use crate::game_logic::{AIState, KindOf, Team, ThingTemplate};

    let mut logic = GameLogic::new();

    let cfg = golden_skirmish_config("IdleAuth");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    for name in ["IdU", "IdE"] {
        if !logic.templates.contains_key(name) {
            let mut t = ThingTemplate::new(name);
            t.add_kind_of(KindOf::Infantry);
            t.add_kind_of(KindOf::Attackable);
            logic.templates.insert(name.into(), t);
        }
    }
    let oid = logic
        .create_object("IdU", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
        .expect("u");
    let vid = logic
        .create_object("IdE", Team::GLA, glam::Vec3::new(10.0, 0.0, 0.0))
        .expect("e");
    if let Some(o) = logic.host_object_mut(oid) {
        o.target = Some(vid);
        o.status.attacking = true;
        o.set_ai_state(AIState::Attacking);
    }
    assert!(logic.private_idle_for_test(oid));

    // AIUpdate.cpp:3067 clears the state machine and enters Idle immediately.
    let idle = logic.host_objects().get(&oid).unwrap();
    assert!(idle.target.is_none());
    assert_eq!(idle.ai_state, AIState::Idle);
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);

    let entity = shadow.entity_for_host(oid).unwrap();
    let target = shadow.entity_for_host(vid).unwrap();
    shadow
        .world_mut()
        .world_mut()
        .entity_mut(entity)
        .unwrap()
        .attack_target = Some(target);
    assert!(logic.host_object(oid).unwrap().target.is_none());
    shadow.sync_from_host(&logic);
    assert!(
        shadow
            .world()
            .entity(entity)
            .unwrap()
            .attack_target
            .is_none()
    );

    // The canonical command has already entered Idle.
    let o = logic.host_objects().get(&oid).unwrap();
    assert!(o.target.is_none());
    assert_eq!(o.ai_state, AIState::Idle);
}

#[test]
fn residual_ai_state_paths_use_owned_state_writers() {
    // C++ changes the driving object's state synchronously. Optional shadow
    // telemetry does not replace that writer, and wrappers may delegate it.
    fn method<'a>(source: &'a str, header: &str) -> &'a str {
        assert_eq!(source.matches(header).count(), 1, "exact method {header}");
        let i = source.find(header).expect("method header");
        let bytes = source.as_bytes();
        let mut j = source[i..].find('{').map(|o| i + o).expect("body");
        let mut depth = 0i32;
        let end = loop {
            match bytes.get(j) {
                Some(b'{') => depth += 1,
                Some(b'}') => {
                    depth -= 1;
                    if depth == 0 {
                        break j;
                    }
                }
                Some(_) => {}
                None => panic!("unclosed {header}"),
            }
            j += 1;
        };
        &source[i..=end]
    }

    let src = GAME_LOGIC_HOST_SRC;
    for (header, write) in [
        (
            "fn try_return_to_base_rearm(",
            "jet.set_ai_state(AIState::Moving);",
        ),
        (
            "fn try_min_range_backup_between(",
            "a.set_ai_state(AIState::Attacking);",
        ),
        (
            "fn append_unit_waypoint(",
            "unit.set_ai_state(AIState::Moving);",
        ),
        (
            "fn attack_aim_at_target_enter(",
            "u.set_ai_state(AIState::Attacking);",
        ),
        (
            "fn attack_fire_weapon_enter(",
            "u.set_ai_state(AIState::Attacking);",
        ),
        ("fn try_idle_crate_pickup(", "u.set_ai_state(parent_state);"),
        (
            "fn on_selling_container_residual(",
            "unit.set_ai_state(AIState::Idle);",
        ),
    ] {
        assert!(
            method(src, header).contains(write),
            "{header} must use its exact owned state writer {write}"
        );
    }
    // The public min-range wrapper reaches the checked implementation; crate
    // pickup installs its owned move path and restores an existing parent.
    assert!(
        method(src, "fn try_min_range_backup(").contains("self.try_min_range_backup_between("),
        "min-range wrapper must reach the concrete backup state writer"
    );
    assert!(
        method(src, "fn try_idle_crate_pickup(").contains("self.assign_unit_path("),
        "crate pickup must install its owned move path before parent restoration"
    );
}

#[test]
fn append_unit_waypoint_decision_authority() {
    use crate::game_logic::{AIState, KindOf, Team, ThingTemplate};

    let mut logic = GameLogic::new();

    let cfg = golden_skirmish_config("WpAuth");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    if !logic.templates.contains_key("WpU") {
        let mut t = ThingTemplate::new("WpU");
        t.add_kind_of(KindOf::Infantry);
        logic.templates.insert("WpU".into(), t);
    }
    let oid = logic
        .create_object("WpU", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
        .expect("id");
    if let Some(o) = logic.host_object_mut(oid) {
        o.movement.max_speed = 20.0;
    }
    assert!(logic.append_unit_waypoint_for_test(oid, glam::Vec3::new(30.0, 0.0, 0.0)));

    assert_eq!(
        logic.host_objects().get(&oid).unwrap().ai_state,
        AIState::Moving,
        "host Moving immediate"
    );
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);

    assert_eq!(
        logic.host_objects().get(&oid).unwrap().ai_state,
        AIState::Moving
    );
}

#[test]
fn set_ai_state_decision_aware_writeback() {
    use crate::game_logic::{AIState, KindOf, Team, ThingTemplate};

    let mut logic = GameLogic::new();

    let cfg = golden_skirmish_config("StateAw");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    if !logic.templates.contains_key("SaU") {
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
        logic.host_objects().get(&oid).unwrap().ai_state,
        AIState::Gathering
    );
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);

    assert_eq!(
        logic.host_objects().get(&oid).unwrap().ai_state,
        AIState::Gathering
    );
}

#[test]
fn death_type_channel_via_set_death_type() {
    use crate::game_logic::host_usa_pilot::HostDeathType;
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("DeathTy");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    if !logic.templates.contains_key("DieUnit") {
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
    let mut shadow = GameWorldShadow::new(64);
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
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    if !logic.templates.contains_key("RadarB") {
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

    let mut shadow = GameWorldShadow::new(64);
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

    let o = logic.host_objects().get(&oid).unwrap();
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
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    if !logic.templates.contains_key("SpUnit") {
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
        let became = o.tick_timers(1.0);
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
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    if !logic.templates.contains_key("SpWbU") {
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
    let mut shadow = GameWorldShadow::new(64);
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

    let o = logic.host_objects().get(&oid).expect("o");
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
fn damage_is_immediate_and_projection_cannot_reapply_it() {
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("DmgAuthority");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    ensure_template(&mut logic, "AuthUnit", 100.0);
    let id = logic
        .create_object("AuthUnit", Team::USA, Vec3::new(2.0, 0.0, 0.0))
        .expect("unit");

    let mut shadow = GameWorldShadow::new(4096);
    shadow.sync_from_host(&logic);
    let pre = logic.host_objects().get(&id).unwrap().health.current;

    // Wave 758: couple for damage_authority_live.

    {
        if let Some((obj, health_events)) = logic.host_object_and_health_events_mut(id) {
            let _ = obj.take_damage(25.0, health_events);
        }
    };

    let host_mid = logic.host_objects().get(&id).unwrap().health.current;
    // C++ ActiveBody::internalChangeHealth writes HP the same frame.
    // The observer cannot change the result of this owner write.
    assert!(
        (host_mid - (pre - 25.0)).abs() < 0.01,
        "host HP must apply same frame (C++ internalChangeHealth); mid={host_mid} pre={pre}"
    );

    let events = logic.health_events.drain_damage();
    assert!(!events.is_empty());
    let eid = shadow.entity_for_host(id).unwrap();
    shadow
        .world_mut()
        .world_mut()
        .entity_mut(eid)
        .unwrap()
        .health = 999.0;
    for _ in 0..2 {
        shadow.sync_from_host(&logic);
        let host_final = logic.host_object(id).unwrap().health.current;
        let observed = shadow.world().entity(eid).unwrap().health;
        assert!((host_final - (pre - 25.0)).abs() < 0.01);
        assert!((observed - host_final).abs() < 0.01);
        assert!(host_final < pre);
    }
}

#[test]
fn damage_authority_applies_host_hp_when_shadow_disabled() {
    // Without a live shadow session, deferred damage would never write back.
    // Authority must couple to shadow_enabled so host-only combat still hits.
    let mut logic = GameLogic::new();

    logic.health_events.clear_damage();
    let cfg = golden_skirmish_config("DmgAuthNoShadow");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    ensure_template(&mut logic, "AuthUnit", 100.0);
    let id = logic
        .create_object("AuthUnit", Team::USA, Vec3::new(2.0, 0.0, 0.0))
        .expect("unit");
    let pre = logic.host_objects().get(&id).unwrap().health.current;
    if let Some((obj, health_events)) = logic.host_object_and_health_events_mut(id) {
        let _ = obj.take_damage(25.0, health_events);
    }
    let mid = logic.host_objects().get(&id).unwrap().health.current;
    assert!(
        (mid - (pre - 25.0)).abs() < 0.01,
        "host HP must apply immediately when shadow disabled; pre={pre} mid={mid}"
    );
}

#[test]
fn lethal_damage_marks_destroyed_and_zero_health_immediately() {
    // C++ ActiveBody::internalChangeHealth writes HP the same frame.
    let mut logic = GameLogic::new();

    let cfg = golden_skirmish_config("DmgAuthLethalFlag");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    ensure_template(&mut logic, "AuthUnit", 50.0);
    let id = logic
        .create_object("AuthUnit", Team::USA, Vec3::new(3.0, 0.0, 0.0))
        .expect("unit");

    if let Some((obj, health_events)) = logic.host_object_and_health_events_mut(id) {
        let dead = obj.take_damage(999.0, health_events);
        assert!(dead, "projected lethal");
        assert!(obj.status.destroyed, "destroyed flag must flip mid-frame");
        assert!(!obj.is_alive(), "is_alive must fail after lethal");
        assert!(
            obj.health.current <= 0.0,
            "same-frame death HP must match C++ internalChangeHealth; now={}",
            obj.health.current
        );
    } else {
        panic!("missing unit");
    }
}

#[test]
fn host_owner_log_feeds_transfer_owner_mutation() {
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("OwnerLog");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    ensure_template(&mut logic, "OwnT", 100.0);
    let id = logic
        .create_object("OwnT", Team::GLA, glam::Vec3::ZERO)
        .expect("id");
    let mut shadow = GameWorldShadow::new(4096);
    shadow.sync_from_host(&logic);
    {
        let (o, ownership_events) = logic.host_object_and_ownership_events_mut(id).unwrap();
        o.set_team(Team::USA, ownership_events);
    }
    logic.ownership_events.complete_boundary();
    let events = logic.ownership_events.take_completed();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].object, id);
    shadow.sync_from_host(&logic);
    let eid = shadow.entity_for_host(id).expect("map");
    let owner = shadow.world().entity(eid).unwrap().owner;
    let expected = shadow.owner_for_host_object(&logic, logic.host_object(id).unwrap());
    assert_eq!(
        owner, expected,
        "TransferOwner should map host team to shadow player"
    );
}

#[test]
fn host_heal_log_feeds_set_health_mutation() {
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("HealLog");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    ensure_template(&mut logic, "HealT", 100.0);
    let id = logic
        .create_object("HealT", Team::USA, glam::Vec3::ZERO)
        .expect("id");
    {
        let o = logic.host_object_mut(id).unwrap();
        o.health.current = 40.0;
    }
    let mut shadow = GameWorldShadow::new(4096);
    shadow.sync_from_host(&logic);
    {
        let body_frame = logic.frame;
        let (o, events) = logic.host_object_and_health_events_mut(id).unwrap();
        o.heal(30.0, events, body_frame);
    }
    let heals = logic.health_events.drain_heal();
    assert_eq!(heals.len(), 1);
    assert_eq!(logic.host_object(id).unwrap().health.current, 70.0);
    shadow.sync_from_host(&logic);
    let probe = shadow.probe(&logic);
    assert!(
        probe.health_match,
        "heal SetHealth should match host: {}",
        probe.detail
    );
}

#[test]
fn actual_damage_is_copied_into_projection() {
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("DmgLogChannel");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    ensure_template(&mut logic, "LogUnit", 150.0);
    let id = logic
        .create_object("LogUnit", Team::USA, Vec3::new(1.0, 0.0, 0.0))
        .expect("unit");
    let mut shadow = GameWorldShadow::new(4096);
    assert!(!owner_damage(&mut logic, id, 40.0));
    assert_eq!(logic.host_object(id).unwrap().health.current, 110.0);
    shadow.sync_from_host(&logic);
    let entity = shadow.entity_for_host(id).unwrap();
    assert_eq!(shadow.world().entity(entity).unwrap().health, 110.0);
    assert!(shadow.entity_for_host(id).is_some());
}

#[test]
fn owned_construction_events_map_completed_structure_without_consuming_output() {
    crate::game_logic::host_spawn_log::clear();
    let mut logic = GameLogic::new();
    let mut t = ThingTemplate::new("USA_Barracks");
    t.set_health(1000.0);
    t.add_kind_of(KindOf::Structure);
    logic.templates.insert("USA_Barracks".into(), t);
    let id = logic
        .create_object("USA_Barracks", Team::USA, glam::Vec3::ZERO)
        .expect("barracks");
    // Simulate host recording construction complete without pre-sync map.
    let mut shadow = GameWorldShadow::new(64);
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
fn physical_destructor_does_not_replay_containment_death_callbacks() {
    // Structural ordering guard only; actual lifecycle tests establish behavior.
    let physical =
        rust_fn_body(GAME_LOGIC_HOST_SRC, "process_destroy_list").expect("physical removal");
    assert!(!physical.contains("apply_transport_death_scatter"));
}

#[test]
fn dozer_bored_repair_state_decision_authority() {
    use crate::game_logic::{AIState, KindOf, Team, ThingTemplate};

    let mut logic = GameLogic::new();

    let cfg = golden_skirmish_config("DzAuth");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    if !logic.templates.contains_key("DzU") {
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
        logic.host_objects().get(&oid).unwrap().ai_state,
        AIState::Repairing,
        "host AI state applies immediately"
    );
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);

    assert_eq!(
        logic.host_objects().get(&oid).unwrap().ai_state,
        AIState::Repairing
    );
}

#[test]
fn hijacker_docked_state_decision_authority() {
    use crate::game_logic::{AIState, KindOf, Team, ThingTemplate};

    let mut logic = GameLogic::new();

    let cfg = golden_skirmish_config("HjAuth");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    if !logic.templates.contains_key("HjU") {
        let mut t = ThingTemplate::new("HjU");
        t.add_kind_of(KindOf::Infantry);
        logic.templates.insert("HjU".into(), t);
    }
    let oid = logic
        .create_object("HjU", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
        .expect("id");
    logic.set_ai_state_decision_aware_for_test(oid, AIState::Docked);

    assert_eq!(
        logic.host_objects().get(&oid).unwrap().ai_state,
        AIState::Docked,
        "host AI state applies immediately"
    );
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);

    assert_eq!(
        logic.host_objects().get(&oid).unwrap().ai_state,
        AIState::Docked
    );
}

#[test]
fn residual_auto_fire_damage_source_attribution_source() {
    let src = GAME_LOGIC_HOST_SRC;
    let helper = last_rust_fn_body(src, "residual_auto_fire_apply_damage")
        .expect("residual_auto_fire_apply_damage");
    let helper: String = helper.split_whitespace().collect();
    assert!(
        helper.contains("apply_residual_damage_owned(receiving_damage_id,damage,Some(attacker_id),crate::game_logic::combat::DamageType::Unresistable,crate::game_logic::host_usa_pilot::HostDeathType::from_host_damage_type(crate::game_logic::combat::DamageType::Unresistable,),None"),
        "residual auto-fire must forward actual damage and attacker through the receiving Body/Player operation"
    );
    for name in [
        "try_base_defense_residual_fire",
        "try_strategy_center_bombardment_turret_fire",
        "update_pending_patriot_assists",
    ] {
        let at = src
            .find(&format!("fn {name}"))
            .unwrap_or_else(|| panic!("missing {name}"));
        let body = &src[at..src.len().min(at + 14000)];
        assert!(
            body.contains("residual_auto_fire_apply_damage"),
            "{name} must use residual_auto_fire_apply_damage"
        );
    }
    // Contained firing now retains the actual selected weapon/shot receipt.
    for name in [
        "try_sentry_drone_residual_fire",
        "try_garrison_residual_fire",
        "try_transport_passenger_residual_fire",
    ] {
        let body = last_rust_fn_body(src, name).unwrap_or_else(|| panic!("missing {name}"));
        assert!(
            body.contains("fire_owned_passive_weapon"),
            "{name} must use the selected real shot"
        );
        assert!(
            !body.contains("residual_auto_fire_apply_damage"),
            "{name} must not also apply residual damage"
        );
    }
    let passive = include_str!("../../game_logic/world_combat/passive_weapon_fire.rs");
    let body =
        last_rust_fn_body(passive, "fire_owned_passive_weapon").expect("actual passive driver");
    assert!(
        body.contains("prepare_passive_weapon_shot")
            && body.contains("materialize_owned_passive_shot(shot)")
    );
    let attack = include_str!("../../game_logic/object/attack.rs");
    let shot = last_rust_fn_body(attack, "prepare_weapon_shot").expect("shared real shot");
    assert!(
        shot.contains("let shooter_id = self.id") && shot.contains("damage_type: weapon_dtype")
    );
    let damage = include_str!("../../game_logic/world_objects/damage_application.rs");
    let application =
        last_rust_fn_body(damage, "apply_owned_combat_damage").expect("actual shot damage owner");
    let application: String = application.split_whitespace().collect();
    assert!(
        application.contains(
            "apply_owned_damage(target_id,damage,Some(shooter_id),damage_type,death_type"
        ),
        "selected typed shot must preserve attacker at the receiving Body callback"
    );
}

#[test]
fn residual_auto_fire_damage_source_writeback_channel() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();

    let cfg = golden_skirmish_config("DmgSrc");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    for name in ["SrcA", "SrcB"] {
        if !logic.templates.contains_key(name) {
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
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    assert_eq!(logic.host_object(victim).unwrap().health.current, 75.0);
    let entity = shadow.entity_for_host(victim).unwrap();
    assert_eq!(shadow.world().entity(entity).unwrap().health, 75.0);
}

#[test]
fn private_stop_clears_owner_before_projection() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};

    let mut logic = GameLogic::new();

    let cfg = golden_skirmish_config("PrivStop");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    for name in ["PsU", "PsE"] {
        if !logic.templates.contains_key(name) {
            let mut t = ThingTemplate::new(name);
            t.add_kind_of(KindOf::Infantry);
            t.add_kind_of(KindOf::Attackable);
            logic.templates.insert(name.into(), t);
        }
    }
    let oid = logic
        .create_object("PsU", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
        .expect("u");
    let vid = logic
        .create_object("PsE", Team::GLA, glam::Vec3::new(10.0, 0.0, 0.0))
        .expect("e");
    if let Some(o) = logic.host_object_mut(oid) {
        o.target = Some(vid);
        o.status.attacking = true;
    }
    assert!(logic.private_stop(oid));
    // Host target clears same-frame; decision log still drives GameWorld.
    assert!(
        logic.host_objects().get(&oid).unwrap().target.is_none(),
        "private_stop must clear host target immediately"
    );

    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let entity = shadow.entity_for_host(oid).unwrap();
    let target = shadow.entity_for_host(vid).unwrap();
    shadow
        .world_mut()
        .world_mut()
        .entity_mut(entity)
        .unwrap()
        .attack_target = Some(target);
    assert!(logic.host_object(oid).unwrap().target.is_none());
    shadow.sync_from_host(&logic);
    assert!(
        shadow
            .world()
            .entity(entity)
            .unwrap()
            .attack_target
            .is_none()
    );
    assert!(
        logic.host_objects().get(&oid).unwrap().target.is_none(),
        "host remains clear after stop + GameWorld stop apply"
    );
}
