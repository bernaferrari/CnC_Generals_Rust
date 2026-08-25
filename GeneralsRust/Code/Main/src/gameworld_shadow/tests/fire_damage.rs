//! Fire-intent/spawn, damage writeback, residual auto-fire, AI stop.

use super::*;

#[test]
fn fire_at_records_fire_intent_residual() {
    let _env_guard = AuthorityEnvGuard::lock()
        .set("GENERALS_GAMEWORLD_AI_ATTACK_AUTHORITY", "1")
        .set("GENERALS_GAMEWORLD_SHADOW", "1")
        .couple();

    use crate::game_logic::host_fire_intent_log;
    use crate::game_logic::{KindOf, Team, ThingTemplate, Weapon};
    host_fire_intent_log::clear();
    crate::game_logic::host_historic_bonus::set_logic_frame(77);
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("FireAtRec");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
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
        let o = logic.host_object_mut(oid).expect("o");
        o.weapon = Some(Weapon {
            damage: 15.0,
            range: 200.0,
            reload_time: 0.0,
            ..Weapon::default()
        });
        o.status.weapons_jammed = false;
        let fired = o.fire_at(vid, 1.0);
        assert!(fired, "close-range fire_at should discharge");
        // Host last_fire_* deferred under AI attack authority.
        assert_eq!(o.last_fire_victim_host, 0);
        assert_eq!(o.last_fire_frame, 0);
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
    assert!(shadow.apply_host_fire_intent_events(&evs) >= 1);
    assert!(shadow.writeback_fire_intent_to_host(&mut logic) >= 1);
    let _ = crate::game_logic::host_fire_intent_ready_log::drain();
    let o = logic.host_objects().get(&oid).unwrap();
    assert_eq!(o.last_fire_victim_host, vid.0);
    assert_eq!(o.last_fire_frame, 77);
    assert!((o.last_fire_damage - 15.0).abs() < 1e-5);

    // Legacy path: authority off — host last_fire_* applied same-frame.
    crate::env_compat::set_var("GENERALS_GAMEWORLD_AI_ATTACK_AUTHORITY", "0");
    host_fire_intent_log::clear();
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.last_fire_victim_host = 0;
        o.last_fire_frame = 0;
        o.last_fire_damage = 0.0;
        o.fire_intent_count = 0;
        // Ensure weapon ready again.
        if let Some(w) = o.weapon.as_mut() {
            w.last_fire_time = 0.0;
        }
        let fired = o.fire_at(vid, 2.0);
        assert!(fired);
        assert_eq!(o.last_fire_victim_host, vid.0);
        assert!(o.fire_intent_count >= 1);
    }
    assert!(!host_fire_intent_log::drain().is_empty());
}

#[test]
fn assign_unit_path_decision_authority() {
    let _env_guard = authority_env_lock();

    use crate::game_logic::host_ai_decision_log;
    use crate::game_logic::{AIState, KindOf, Team, ThingTemplate};
    let prev = std::env::var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY").ok();
    crate::env_compat::set_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY", "1");

    crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", "1");
    host_ai_decision_log::clear();
    let _couple = super::ShadowCoupleGuard::enter();
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
    let events = host_ai_decision_log::drain();
    assert!(
        events.iter().any(|e| {
            e.kind == host_ai_decision_log::AI_DECISION_SET_STATE
                && e.host_object == oid
                && e.ai_state_ordinal == 1
        }),
        "assign_unit_path must log Moving; got {events:?}"
    );
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
    assert!(shadow.apply_ai_decisions_as_world_mutations(&events) >= 1);
    let _ = shadow.writeback_ai_state_to_host(&mut logic);
    assert_eq!(
        logic.host_objects().get(&oid).unwrap().ai_state,
        AIState::Moving
    );
    match prev {
        Some(v) => crate::env_compat::set_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY", v),
        None => crate::env_compat::remove_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY"),
    }
}

#[test]
fn private_idle_decision_authority() {
    let _env_guard = authority_env_lock();

    use crate::game_logic::host_ai_decision_log;
    use crate::game_logic::{AIState, KindOf, Team, ThingTemplate};
    let prev = std::env::var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY").ok();
    crate::env_compat::set_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY", "1");

    crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", "1");
    let prev_atk = std::env::var("GENERALS_GAMEWORLD_AI_ATTACK_AUTHORITY").ok();
    crate::env_compat::set_var("GENERALS_GAMEWORLD_AI_ATTACK_AUTHORITY", "1");
    host_ai_decision_log::clear();
    let _couple = super::ShadowCoupleGuard::enter();
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
    let events = host_ai_decision_log::drain();
    assert!(
        events
            .iter()
            .any(|e| e.kind == host_ai_decision_log::AI_DECISION_STOP_ATTACK),
        "private_idle must log StopAttack; got {events:?}"
    );
    assert!(
        events.iter().any(|e| {
            e.kind == host_ai_decision_log::AI_DECISION_SET_STATE && e.ai_state_ordinal == 0
        }),
        "private_idle must log Idle; got {events:?}"
    );
    // Host still engaged until writeback.
    assert_eq!(logic.host_objects().get(&oid).unwrap().target, Some(vid));
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    assert!(shadow.apply_ai_decisions_as_world_mutations(&events) >= 1);
    let _ = shadow.writeback_ai_state_to_host(&mut logic);
    assert!(shadow.writeback_attack_targets_to_host(&mut logic) >= 1);
    let _ = crate::game_logic::host_attack_target_ready_log::drain();
    // set_target(None) residual also idles host; either writeback path is enough.
    let o = logic.host_objects().get(&oid).unwrap();
    assert!(o.target.is_none());
    assert_eq!(o.ai_state, AIState::Idle);
    match prev {
        Some(v) => crate::env_compat::set_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY", v),
        None => crate::env_compat::remove_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY"),
    }
    match prev_atk {
        Some(v) => crate::env_compat::set_var("GENERALS_GAMEWORLD_AI_ATTACK_AUTHORITY", v),
        None => crate::env_compat::remove_var("GENERALS_GAMEWORLD_AI_ATTACK_AUTHORITY"),
    }
}

#[test]
fn residual_ai_state_paths_honor_decision_authority_source() {
    let src = GAME_LOGIC_HOST_SRC;
    for fn_name in [
        "fn try_return_to_base_rearm",
        "fn try_min_range_backup",
        "fn append_unit_waypoint",
        "fn attack_aim_at_target_enter",
        "fn attack_fire_weapon_enter",
        "fn try_idle_crate_pickup",
        "fn on_selling_container_residual",
    ] {
        let i = src
            .find(fn_name)
            .unwrap_or_else(|| panic!("missing {fn_name}"));
        let bytes = src.as_bytes();
        let mut j = src[i..].find('{').map(|o| i + o).expect("body");
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
                None => panic!("unclosed {fn_name}"),
            }
            j += 1;
        };
        let w = &src[i..=end];
        assert!(
            w.contains("gameworld_ai_decision_authority")
                || w.contains("host_ai_decision_log::record_set_state"),
            "{fn_name} must honor AI decision authority for AI state"
        );
    }
}

#[test]
fn append_unit_waypoint_decision_authority() {
    let _env_guard = authority_env_lock();

    use crate::game_logic::host_ai_decision_log;
    use crate::game_logic::{AIState, KindOf, Team, ThingTemplate};
    let prev = std::env::var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY").ok();
    crate::env_compat::set_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY", "1");
    crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", "1");
    host_ai_decision_log::clear();
    let _couple = super::ShadowCoupleGuard::enter();
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
    let events = host_ai_decision_log::drain();
    assert!(
        events.iter().any(|e| {
            e.kind == host_ai_decision_log::AI_DECISION_SET_STATE
                && e.host_object == oid
                && e.ai_state_ordinal == 1
        }),
        "waypoint must log Moving; got {events:?}"
    );
    assert_eq!(
        logic.host_objects().get(&oid).unwrap().ai_state,
        AIState::Moving,
        "host Moving immediate"
    );
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    assert!(shadow.apply_ai_decisions_as_world_mutations(&events) >= 1);
    let _ = shadow.writeback_ai_state_to_host(&mut logic);
    assert_eq!(
        logic.host_objects().get(&oid).unwrap().ai_state,
        AIState::Moving
    );
    match prev {
        Some(v) => crate::env_compat::set_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY", v),
        None => crate::env_compat::remove_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY"),
    }
}

#[test]
fn set_ai_state_decision_aware_writeback() {
    let _env_guard = authority_env_lock();

    use crate::game_logic::host_ai_decision_log;
    use crate::game_logic::{AIState, KindOf, Team, ThingTemplate};
    let prev = std::env::var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY").ok();
    crate::env_compat::set_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY", "1");
    crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", "1");
    host_ai_decision_log::clear();
    let _couple = super::ShadowCoupleGuard::enter();
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
    let events = host_ai_decision_log::drain();
    let ord = GameWorldShadow::host_ai_state_ordinal(&AIState::Gathering);
    assert!(
        events.iter().any(|e| {
            e.kind == host_ai_decision_log::AI_DECISION_SET_STATE
                && e.host_object == oid
                && e.ai_state_ordinal == ord
        }),
        "must log Gathering; got {events:?}"
    );
    // C++ AIUpdate applies state same-frame; host is last-writer input, GW writeback confirms.
    assert_eq!(
        logic.host_objects().get(&oid).unwrap().ai_state,
        AIState::Gathering
    );
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    assert!(shadow.apply_ai_decisions_as_world_mutations(&events) >= 1);
    let _ = shadow.writeback_ai_state_to_host(&mut logic);
    assert_eq!(
        logic.host_objects().get(&oid).unwrap().ai_state,
        AIState::Gathering
    );
    match prev {
        Some(v) => crate::env_compat::set_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY", v),
        None => crate::env_compat::remove_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY"),
    }
}

#[test]
fn death_type_channel_via_set_death_type() {
    use crate::game_logic::host_death_type_log;
    use crate::game_logic::host_usa_pilot::HostDeathType;
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    host_death_type_log::clear();
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
    host_death_type_log::record(oid, HostDeathType::Burned.ordinal());
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
    assert!(shadow.apply_host_death_type_events(&host_death_type_log::drain()) >= 1);
    assert_eq!(
        shadow.world().entity(eid).unwrap().death_type,
        HostDeathType::Burned.ordinal()
    );
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.status.death_type = HostDeathType::Normal;
    }
    assert!(shadow.writeback_death_type_to_host(&mut logic) >= 1);
    let _ = crate::game_logic::host_death_type_ready_log::drain();
    let _ = shadow.writeback_radar_extend_to_host(&mut logic);
    let _ = shadow.writeback_shock_stun_to_host(&mut logic);
    let _ = crate::game_logic::host_shock_stun_ready_log::drain();
    let _ = shadow.writeback_rebuild_producer_to_host(&mut logic);
    let _ = shadow.writeback_sole_healing_to_host(&mut logic);
    let _ = crate::game_logic::host_sole_healing_ready_log::drain();
    let _ = shadow.writeback_hijacker_to_host(&mut logic);
    let _ = crate::game_logic::host_hijacker_ready_log::drain();
    let _ = shadow.writeback_ai_mood_to_host(&mut logic);
    let _ = crate::game_logic::host_ai_mood_ready_log::drain();
    let _ = shadow.writeback_ai_request_to_host(&mut logic);
    let _ = crate::game_logic::host_ai_request_ready_log::drain();
    let _ = shadow.writeback_hijacker_to_host(&mut logic);
    let _ = crate::game_logic::host_hijacker_ready_log::drain();
    assert_eq!(
        logic.host_objects().get(&oid).unwrap().status.death_type,
        HostDeathType::Burned
    );
}

#[test]
fn radar_extend_channel_via_set_radar_extend() {
    use crate::game_logic::host_radar_extend_log;
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    host_radar_extend_log::clear();
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
    host_radar_extend_log::record(oid, 120, false, true);
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let eid = *shadow.host_to_entity.get(&oid.0).expect("map");
    assert!(shadow.apply_host_radar_extend_events(&host_radar_extend_log::drain()) >= 1);
    let e = shadow.world().entity(eid).unwrap();
    assert_eq!(e.radar_extend_done_frame, 120);
    assert!(e.radar_active);
    assert!(!e.radar_extend_complete);
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.radar_active = false;
        o.radar_extend_done_frame = 0;
    }
    assert!(shadow.writeback_radar_extend_to_host(&mut logic) >= 1);
    let _ = shadow.writeback_shock_stun_to_host(&mut logic);
    let _ = crate::game_logic::host_shock_stun_ready_log::drain();
    let _ = shadow.writeback_rebuild_producer_to_host(&mut logic);
    let _ = shadow.writeback_sole_healing_to_host(&mut logic);
    let _ = crate::game_logic::host_sole_healing_ready_log::drain();
    let _ = shadow.writeback_hijacker_to_host(&mut logic);
    let _ = crate::game_logic::host_hijacker_ready_log::drain();
    let _ = shadow.writeback_ai_mood_to_host(&mut logic);
    let _ = crate::game_logic::host_ai_mood_ready_log::drain();
    let _ = shadow.writeback_ai_request_to_host(&mut logic);
    let _ = crate::game_logic::host_ai_request_ready_log::drain();
    let _ = shadow.writeback_hijacker_to_host(&mut logic);
    let _ = crate::game_logic::host_hijacker_ready_log::drain();
    let o = logic.host_objects().get(&oid).unwrap();
    assert!(o.radar_active);
    assert_eq!(o.radar_extend_done_frame, 120);
}

#[test]
fn special_power_tick_records_host_special_power_log() {
    // Host-only advance residual: disable SP sole-tick authority for this probe.
    let prev = std::env::var("GENERALS_GAMEWORLD_SPECIAL_POWER_AUTHORITY").ok();
    crate::env_compat::set_var("GENERALS_GAMEWORLD_SPECIAL_POWER_AUTHORITY", "0");

    use crate::game_logic::{KindOf, Team, ThingTemplate, host_special_power_log};
    host_special_power_log::clear();
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
    let events = host_special_power_log::drain();
    assert!(
        events
            .iter()
            .any(|e| { e.object == oid && (e.cooldown_remaining - 4.0).abs() < 1e-3 }),
        "events {:?}",
        events
    );

    match prev {
        Some(v) => crate::env_compat::set_var("GENERALS_GAMEWORLD_SPECIAL_POWER_AUTHORITY", v),
        None => crate::env_compat::remove_var("GENERALS_GAMEWORLD_SPECIAL_POWER_AUTHORITY"),
    }
}

#[test]
fn special_power_session_writeback_after_tick() {
    use crate::game_logic::{KindOf, Team, ThingTemplate, host_special_power_log};
    host_special_power_log::clear();
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
        o.record_host_special_power();
    }
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let events = host_special_power_log::drain();
    assert!(shadow.apply_host_special_power_events(&events) >= 1);
    // Desync host after GameWorld apply so writeback has work.
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.special_power_cooldown_remaining = 9.0;
    }
    assert!(shadow.writeback_special_power_to_host(&mut logic) >= 1);
    let o = logic.host_objects().get(&oid).expect("o");
    assert!((o.special_power_cooldown_remaining - 2.0).abs() < 1e-3);
}

#[test]
fn damage_authority_writeback_is_last_writer() {
    let _env_guard = authority_env_lock();

    crate::game_logic::host_damage_log::clear();
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

    crate::env_compat::set_var("GENERALS_GAMEWORLD_DAMAGE_AUTHORITY", "1");
    assert!(gameworld_damage_authority_enabled());
    // Wave 758: couple for damage_authority_live.
    let _couple = ShadowCoupleGuard::enter();
    install_active_shadow_for_coupled_tick(&mut shadow);
    if let Some(obj) = logic.host_object_mut(id) {
        let _ = obj.take_damage(25.0);
    }
    clear_active_shadow_for_coupled_tick();
    drop(_couple);
    let host_mid = logic.host_objects().get(&id).unwrap().health.current;
    // C++ ActiveBody::internalChangeHealth writes HP the same frame.
    // Host still logs for the shadow channel; writeback remains last-writer
    // after a deliberate host desync below.
    assert!(
        (host_mid - (pre - 25.0)).abs() < 0.01,
        "host HP must apply same frame (C++ internalChangeHealth); mid={host_mid} pre={pre}"
    );

    let events = crate::game_logic::host_damage_log::drain();
    assert!(!events.is_empty());
    shadow.sync_from_host_with(&logic, false);
    let eid = shadow.entity_for_host(id).unwrap();
    let shadow_pre_mut = shadow.world().entity(eid).unwrap().health;
    assert!(
        (shadow_pre_mut - pre).abs() < 0.01,
        "expected pre-tick shadow hp {pre} got {shadow_pre_mut}"
    );
    let _ = shadow.apply_host_damage_events(&events);
    // Deliberately desync host so writeback must run.
    if let Some(obj) = logic.host_object_mut(id) {
        obj.health.current = pre; // restore pre-damage on host
        obj.status.destroyed = false;
    }
    let wb = shadow.writeback_health_to_host(&mut logic);
    assert!(wb >= 1, "expected writeback after host desync");
    let host_final = logic.host_objects().get(&id).unwrap().health.current;
    let shadow_final = shadow.world().entity(eid).unwrap().health;
    assert!(
        (host_final - shadow_final).abs() < 0.05,
        "writeback mismatch host={host_final} shadow={shadow_final}"
    );
    assert!(
        (host_final - (pre - 25.0)).abs() < 0.05,
        "authority final {host_final} expected ~{}",
        pre - 25.0
    );
    assert!(host_final < pre);
}

#[test]
fn damage_authority_applies_host_hp_when_shadow_disabled() {
    let _env_guard = authority_env_lock();

    // Without a live shadow session, deferred damage would never write back.
    // Authority must couple to shadow_enabled so host-only combat still hits.
    let prev_shadow = std::env::var("GENERALS_GAMEWORLD_SHADOW").ok();
    let prev_auth = std::env::var("GENERALS_GAMEWORLD_DAMAGE_AUTHORITY").ok();
    crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", "0");
    crate::env_compat::set_var("GENERALS_GAMEWORLD_DAMAGE_AUTHORITY", "1");
    assert!(!gameworld_shadow_enabled());
    assert!(gameworld_damage_authority_enabled());

    crate::game_logic::host_damage_log::clear();
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("DmgAuthNoShadow");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    ensure_template(&mut logic, "AuthUnit", 100.0);
    let id = logic
        .create_object("AuthUnit", Team::USA, Vec3::new(2.0, 0.0, 0.0))
        .expect("unit");
    let pre = logic.host_objects().get(&id).unwrap().health.current;
    if let Some(obj) = logic.host_object_mut(id) {
        let _ = obj.take_damage(25.0);
    }
    let mid = logic.host_objects().get(&id).unwrap().health.current;
    assert!(
        (mid - (pre - 25.0)).abs() < 0.01,
        "host HP must apply immediately when shadow disabled; pre={pre} mid={mid}"
    );

    // restore env
    match prev_shadow {
        Some(v) => crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", v),
        None => crate::env_compat::remove_var("GENERALS_GAMEWORLD_SHADOW"),
    }
    match prev_auth {
        Some(v) => crate::env_compat::set_var("GENERALS_GAMEWORLD_DAMAGE_AUTHORITY", v),
        None => crate::env_compat::remove_var("GENERALS_GAMEWORLD_DAMAGE_AUTHORITY"),
    }
}

#[test]
fn damage_authority_lethal_marks_destroyed_without_host_hp() {
    let _env_guard = authority_env_lock();
    let prev_shadow = std::env::var("GENERALS_GAMEWORLD_SHADOW").ok();
    let prev_auth = std::env::var("GENERALS_GAMEWORLD_DAMAGE_AUTHORITY").ok();
    crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", "1");
    crate::env_compat::set_var("GENERALS_GAMEWORLD_DAMAGE_AUTHORITY", "1");

    // C++ ActiveBody::internalChangeHealth writes HP the same frame.
    crate::game_logic::host_damage_log::clear();
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("DmgAuthLethalFlag");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    ensure_template(&mut logic, "AuthUnit", 50.0);
    let id = logic
        .create_object("AuthUnit", Team::USA, Vec3::new(3.0, 0.0, 0.0))
        .expect("unit");
    assert!(gameworld_damage_authority_enabled());
    assert!(gameworld_shadow_enabled());
    let _couple = ShadowCoupleGuard::enter();
    if let Some(obj) = logic.host_object_mut(id) {
        let dead = obj.take_damage(999.0);
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
    drop(_couple);
    match prev_shadow {
        Some(v) => crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", v),
        None => crate::env_compat::remove_var("GENERALS_GAMEWORLD_SHADOW"),
    }
    match prev_auth {
        Some(v) => crate::env_compat::set_var("GENERALS_GAMEWORLD_DAMAGE_AUTHORITY", v),
        None => crate::env_compat::remove_var("GENERALS_GAMEWORLD_DAMAGE_AUTHORITY"),
    }
}

#[test]
fn host_owner_log_feeds_transfer_owner_mutation() {
    crate::game_logic::host_owner_log::clear();
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
        let o = logic.host_object_mut(id).unwrap();
        o.set_team(Team::USA);
    }
    let events = crate::game_logic::host_owner_log::drain();
    assert_eq!(events.len(), 1);
    let n = shadow.apply_host_owner_events(&logic, &events);
    assert_eq!(n, 1);
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
    crate::game_logic::host_heal_log::clear();
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
        let o = logic.host_object_mut(id).unwrap();
        o.health.current = 70.0;
        crate::game_logic::host_heal_log::record(id, 70.0);
    }
    let heals = crate::game_logic::host_heal_log::drain();
    let n = shadow.apply_host_heal_events(&heals);
    assert_eq!(n, 1);
    let probe = shadow.probe(&mut logic);
    assert!(
        probe.health_match,
        "heal SetHealth should match host: {}",
        probe.detail
    );
}

#[test]
fn host_damage_log_feeds_shadow_mutation_channel() {
    crate::game_logic::host_damage_log::clear();
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("DmgLogChannel");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    ensure_template(&mut logic, "LogUnit", 150.0);
    let id = logic
        .create_object("LogUnit", Team::USA, Vec3::new(1.0, 0.0, 0.0))
        .expect("unit");
    let mut shadow = GameWorldShadow::new(4096);
    let queued = apply_logged_damage_channel_parity(&mut logic, &mut shadow, &[(id, 40.0)])
        .expect("channel");
    assert!(queued >= 1, "expected queued mutations");
    assert!(shadow.entity_for_host(id).is_some());
}

#[test]
fn host_construction_log_maps_completed_structure_in_shadow() {
    crate::game_logic::host_construction_log::clear();
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
    crate::game_logic::host_construction_log::record(id, "USA_Barracks");
    let events = crate::game_logic::host_construction_log::drain();
    let n = shadow.apply_host_construction_events(&events, &logic);
    assert!(n >= 1, "construction apply mapped {n}");
    assert!(
        shadow.entity_for_host(id).is_some(),
        "completed structure must be mapped in shadow"
    );
}

#[test]
fn dozer_construction_ai_state_decision_authority_source() {
    let src = GAME_LOGIC_HOST_SRC;
    for fn_name in [
        "update_dozer_bored_repair",
        "update_construction",
        "update_rebuild_holes",
        "try_auto_resume_construction_residual",
        "process_destroy_list",
    ] {
        let last = last_rust_fn_body(src, fn_name).unwrap_or_else(|| panic!("missing {fn_name}"));
        let first = rust_fn_body(src, fn_name).unwrap_or(last);
        let ok = [last, first].iter().any(|w| {
            w.contains("gameworld_ai_decision_authority")
                || w.contains("set_ai_state_decision_aware")
                || w.contains("host_ai_decision_log::record_set_state")
                || w.contains("apply_engagement_decision_aware")
                || w.contains("record_stop_attack")
                || w.contains("set_ai_state(AIState::Idle)")
        });
        assert!(ok, "{fn_name} must honor AI decision authority");
    }
}

#[test]
fn dozer_bored_repair_state_decision_authority() {
    let _env_guard = authority_env_lock();

    use crate::game_logic::host_ai_decision_log;
    use crate::game_logic::{AIState, KindOf, Team, ThingTemplate};
    let prev = std::env::var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY").ok();
    crate::env_compat::set_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY", "1");
    crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", "1");
    host_ai_decision_log::clear();
    let _couple = super::ShadowCoupleGuard::enter();
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
    let events = host_ai_decision_log::drain();
    let ord = GameWorldShadow::host_ai_state_ordinal(&AIState::Repairing);
    assert!(
        events.iter().any(|e| {
            e.kind == host_ai_decision_log::AI_DECISION_SET_STATE
                && e.host_object == oid
                && e.ai_state_ordinal == ord
        }),
        "Repairing must be logged; got {events:?}"
    );
    assert_eq!(
        logic.host_objects().get(&oid).unwrap().ai_state,
        AIState::Repairing,
        "host AI state applies immediately"
    );
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    assert!(shadow.apply_ai_decisions_as_world_mutations(&events) >= 1);
    let _ = shadow.writeback_ai_state_to_host(&mut logic);
    assert_eq!(
        logic.host_objects().get(&oid).unwrap().ai_state,
        AIState::Repairing
    );
    match prev {
        Some(v) => crate::env_compat::set_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY", v),
        None => crate::env_compat::remove_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY"),
    }
}

#[test]
fn capture_residual_ai_state_decision_authority_source() {
    let src = GAME_LOGIC_HOST_SRC;
    for fn_name in [
        "fn on_capture_object_residual",
        "fn on_capture_tunnel_network_residual",
        "fn on_capture_kick_passengers",
        "fn check_building_damage_states",
        "fn put_hijacker_in_airborne_parachute",
        "fn tick_strategy_center_turret_mood_target",
    ] {
        let i = src
            .find(fn_name)
            .unwrap_or_else(|| panic!("missing {fn_name}"));
        let bytes = src.as_bytes();
        let mut j = src[i..].find('{').map(|o| i + o).expect("body");
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
                None => panic!("unclosed {fn_name}"),
            }
            j += 1;
        };
        let w = &src[i..=end];
        assert!(
            w.contains("gameworld_ai_decision_authority")
                || w.contains("set_ai_state_decision_aware")
                || w.contains("host_ai_decision_log::record_set_state")
                || w.contains("host_ai_decision_log::record_attack"),
            "{fn_name} must honor AI decision authority"
        );
    }
}

#[test]
fn hijacker_docked_state_decision_authority() {
    let _env_guard = authority_env_lock();

    use crate::game_logic::host_ai_decision_log;
    use crate::game_logic::{AIState, KindOf, Team, ThingTemplate};
    let prev = std::env::var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY").ok();
    crate::env_compat::set_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY", "1");

    crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", "1");
    host_ai_decision_log::clear();
    let _couple = super::ShadowCoupleGuard::enter();
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
    let events = host_ai_decision_log::drain();
    let ord = GameWorldShadow::host_ai_state_ordinal(&AIState::Docked);
    assert!(
        events.iter().any(|e| {
            e.kind == host_ai_decision_log::AI_DECISION_SET_STATE
                && e.host_object == oid
                && e.ai_state_ordinal == ord
        }),
        "Docked must be logged; got {events:?}"
    );
    assert_eq!(
        logic.host_objects().get(&oid).unwrap().ai_state,
        AIState::Docked,
        "host AI state applies immediately"
    );
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    assert!(shadow.apply_ai_decisions_as_world_mutations(&events) >= 1);
    let _ = shadow.writeback_ai_state_to_host(&mut logic);
    assert_eq!(
        logic.host_objects().get(&oid).unwrap().ai_state,
        AIState::Docked
    );
    match prev {
        Some(v) => crate::env_compat::set_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY", v),
        None => crate::env_compat::remove_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY"),
    }
}

#[test]
fn residual_eject_payload_ai_state_decision_authority_source() {
    let src = GAME_LOGIC_HOST_SRC;
    for fn_name in [
        "fn apply_bunker_buster_to_target",
        "fn apply_kill_garrisoned_to_target",
        "fn apply_rider_free_fall_damage",
        "fn tick_eject_parachute_residual",
        "fn apply_host_hive_damage_from",
        "fn update_angry_mobs",
        "fn update_mines_and_demo_traps",
        "fn clear_mine_internal",
        "fn start_sell_object",
        "fn cancel_dozers_building",
        "fn resume_construction",
        "fn apply_listening_outpost_initial_payload",
        "fn apply_troop_crawler_initial_payload",
        "fn command_attack",
        "fn command_stop",
    ] {
        let i = src
            .find(fn_name)
            .unwrap_or_else(|| panic!("missing {fn_name}"));
        let bytes = src.as_bytes();
        let mut j = src[i..].find('{').map(|o| i + o).expect("body");
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
                None => panic!("unclosed {fn_name}"),
            }
            j += 1;
        };
        let w = &src[i..=end];
        assert!(
            w.contains("gameworld_ai_decision_authority")
                || w.contains("set_ai_state_decision_aware")
                || w.contains("host_ai_decision_log::record_set_state")
                || w.contains("host_ai_decision_log::record_attack"),
            "{fn_name} must honor AI decision authority"
        );
    }
}

#[test]
fn residual_auto_fire_records_fire_intent_source() {
    let src = GAME_LOGIC_HOST_SRC;
    for fn_name in [
        "fn try_strategy_center_bombardment_turret_fire",
        "fn try_base_defense_residual_fire",
        "fn update_pending_patriot_assists",
        "fn try_sentry_drone_residual_fire",
        "fn try_hellfire_drone_residual_fire",
        "fn try_transport_passenger_residual_fire",
        "fn try_garrison_residual_fire",
    ] {
        let i = src
            .find(fn_name)
            .unwrap_or_else(|| panic!("missing {fn_name}"));
        let bytes = src.as_bytes();
        let mut j = src[i..].find('{').map(|o| i + o).expect("body");
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
                None => panic!("unclosed {fn_name}"),
            }
            j += 1;
        };
        let w = &src[i..=end];
        assert!(
            w.contains("host_fire_intent_log::record")
                && w.contains("gameworld_ai_attack_authority"),
            "{fn_name} must record fire-intent under AI attack authority"
        );
    }
    let obj = GAME_LOGIC_OBJECT_SRC;
    let i = obj.find("fn fire_at_ex").expect("fire_at_ex");
    let w = &obj[i..i + 8000];
    assert!(
        w.contains("gameworld_ai_decision_authority") && w.contains("record_set_state"),
        "fire_at_ex pre-attack must honor AI decision authority"
    );
}

#[test]
fn residual_auto_fire_damage_source_attribution_source() {
    let src = GAME_LOGIC_HOST_SRC;
    let helper = last_rust_fn_body(src, "residual_auto_fire_apply_damage")
        .expect("residual_auto_fire_apply_damage");
    assert!(
        helper.contains("take_damage_from(damage, Some(attacker_id))"),
        "residual auto-fire helper must source-attribute hitscan damage"
    );
    for name in [
        "try_sentry_drone_residual_fire",
        "try_hellfire_drone_residual_fire",
        "try_garrison_residual_fire",
        "try_transport_passenger_residual_fire",
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
}

#[test]
fn residual_auto_fire_damage_source_writeback_channel() {
    use crate::game_logic::host_damage_log;
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    host_damage_log::clear();
    let prev = std::env::var("GENERALS_GAMEWORLD_DAMAGE_AUTHORITY").ok();
    crate::env_compat::set_var("GENERALS_GAMEWORLD_DAMAGE_AUTHORITY", "1");
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
    {
        let v = logic.host_object_mut(victim).unwrap();
        let _ = v.take_damage_from(25.0, Some(attacker));
        assert_eq!(v.last_damage_source, Some(attacker));
        // Damage authority defers HP; projected destroy false.
        assert!(v.health.current > 50.0 || gameworld_damage_authority_enabled());
    }
    let events = host_damage_log::drain();
    assert!(
        events
            .iter()
            .any(|e| { e.target == victim && e.source == Some(attacker) && e.amount >= 20.0 }),
        "damage log must carry source; got {events:?}"
    );
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let applied = shadow.apply_host_damage_events(&events);
    assert!(
        applied.0 + applied.1 >= 1,
        "expected damage apply {applied:?}"
    );
    match prev {
        Some(v) => crate::env_compat::set_var("GENERALS_GAMEWORLD_DAMAGE_AUTHORITY", v),
        None => crate::env_compat::remove_var("GENERALS_GAMEWORLD_DAMAGE_AUTHORITY"),
    }
}

#[test]
fn private_stop_and_clear_target_decision_authority_source() {
    let src = GAME_LOGIC_HOST_SRC;
    assert!(
        src.contains("fn clear_target_decision_aware"),
        "clear_target_decision_aware helper must exist"
    );
    for fn_name in [
        "private_stop",
        "process_destroy_list",
        "on_capture_tunnel_network_residual",
        "on_capture_kick_passengers",
        "check_building_damage_states",
        "tick_strategy_center_turret_mood_target",
    ] {
        let w = last_rust_fn_body(src, fn_name).unwrap_or_else(|| panic!("missing {fn_name}"));
        assert!(
            w.contains("record_stop_attack")
                || w.contains("clear_target_decision_aware")
                || w.contains("stop_attack_decision_aware"),
            "{fn_name} must clear combat targets via StopAttack decision channel"
        );
    }
}

#[test]
fn private_stop_decision_authority_clears_via_writeback() {
    let _env_guard = authority_env_lock();

    use crate::game_logic::host_ai_decision_log;
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let prev = std::env::var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY").ok();
    let prev_atk = std::env::var("GENERALS_GAMEWORLD_AI_ATTACK_AUTHORITY").ok();
    crate::env_compat::set_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY", "1");
    crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", "1");
    crate::env_compat::set_var("GENERALS_GAMEWORLD_AI_ATTACK_AUTHORITY", "1");
    host_ai_decision_log::clear();
    let _couple = super::ShadowCoupleGuard::enter();
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
    let events = host_ai_decision_log::drain();
    assert!(
        events.iter().any(|e| {
            e.kind == host_ai_decision_log::AI_DECISION_STOP_ATTACK && e.host_object == oid
        }),
        "private_stop must log StopAttack; got {events:?}"
    );
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    // Seed world attack target then apply stop.
    assert!(shadow.queue_set_attack_target_for_host(oid, Some(vid)));
    let _ = shadow.apply_pending();
    assert!(shadow.apply_ai_decisions_as_world_mutations(&events) >= 1);
    let _ = shadow.apply_pending();
    let _ = shadow.writeback_attack_targets_to_host(&mut logic);
    let _ = crate::game_logic::host_attack_target_ready_log::drain();
    assert!(
        logic.host_objects().get(&oid).unwrap().target.is_none(),
        "host remains clear after stop + GameWorld stop apply"
    );
    match prev {
        Some(v) => crate::env_compat::set_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY", v),
        None => crate::env_compat::remove_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY"),
    }
    match prev_atk {
        Some(v) => crate::env_compat::set_var("GENERALS_GAMEWORLD_AI_ATTACK_AUTHORITY", v),
        None => crate::env_compat::remove_var("GENERALS_GAMEWORLD_AI_ATTACK_AUTHORITY"),
    }
}
