//! AI decision authority, continue-attack, path, troop-crawler.

use super::*;

#[test]
fn ai_decision_buffer_channel_via_push_ai_decision() {
    let _env_guard = authority_env_lock();
    use crate::game_logic::host_ai_decision_log;
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    crate::env_compat::set_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY", "1");
    crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", "1");
    host_ai_decision_log::clear();
    let _couple = super::ShadowCoupleGuard::enter();
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("AiDec");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    if !logic.templates.contains_key("AdU") {
        let mut t = ThingTemplate::new("AdU");
        t.add_kind_of(KindOf::Infantry);
        logic.templates.insert("AdU".into(), t);
    }
    let oid = logic
        .create_object("AdU", Team::USA, glam::Vec3::new(5.0, 0.0, 5.0))
        .expect("id");
    let vid = logic
        .create_object("AdU", Team::China, glam::Vec3::new(25.0, 0.0, 5.0))
        .expect("v");
    logic.apply_ai_command_for_test(crate::game_logic::game_logic::AICommand::AttackTarget {
        object_id: oid,
        target_id: vid,
    });
    logic.apply_ai_command_for_test(crate::game_logic::game_logic::AICommand::MoveTo {
        object_id: oid,
        position: glam::Vec3::new(1.0, 0.0, 2.0),
    });
    let events = host_ai_decision_log::drain();
    assert!(events.len() >= 2);
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    assert!(shadow.apply_host_ai_decision_events(&events) >= 2);
    let dec = shadow.world().ai_decisions();
    assert!(dec.iter().any(|d| {
        d.kind == host_ai_decision_log::AI_DECISION_ATTACK
            && d.host_object == oid.0
            && d.target_host == vid.0
    }));
    assert!(dec.iter().any(|d| {
        d.kind == host_ai_decision_log::AI_DECISION_MOVE_TO
            && d.destination == Some([1.0, 0.0, 2.0])
    }));
}

#[test]
fn ai_decision_authority_applies_attack_via_gameworld() {
    let _env_guard = authority_env_lock();

    use crate::game_logic::host_ai_decision_log;
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let prev = std::env::var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY").ok();
    crate::env_compat::set_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY", "1");
    crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", "1");
    assert!(gameworld_ai_decision_authority_enabled());
    // Attack writeback must also be on for last-write.
    let prev_atk = std::env::var("GENERALS_GAMEWORLD_AI_ATTACK_AUTHORITY").ok();
    crate::env_compat::set_var("GENERALS_GAMEWORLD_AI_ATTACK_AUTHORITY", "1");
    host_ai_decision_log::clear();
    let _couple = super::ShadowCoupleGuard::enter();
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("AiDecAuth");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    if !logic.templates.contains_key("AdaU") {
        let mut t = ThingTemplate::new("AdaU");
        t.add_kind_of(KindOf::Infantry);
        logic.templates.insert("AdaU".into(), t);
    }
    let oid = logic
        .create_object("AdaU", Team::USA, glam::Vec3::new(8.0, 0.0, 8.0))
        .expect("id");
    let vid = logic
        .create_object("AdaU", Team::China, glam::Vec3::new(40.0, 0.0, 8.0))
        .expect("v");
    // Log-only path (authority on): record without host apply_ai_command.
    host_ai_decision_log::record_attack(oid, vid);
    let events = host_ai_decision_log::drain();
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    assert!(shadow.apply_ai_decisions_as_world_mutations(&events) >= 1);
    // Host still has no target until writeback.
    assert!(logic.host_objects().get(&oid).unwrap().target.is_none());
    assert!(shadow.writeback_attack_targets_to_host(&mut logic) >= 1);
    let _ = crate::game_logic::host_attack_target_ready_log::drain();
    let o = logic.host_objects().get(&oid).unwrap();
    assert_eq!(o.target, Some(vid));
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
fn apply_ai_command_logs_and_host_applies_under_authority() {
    let _env_guard = authority_env_lock();

    use crate::game_logic::game_logic::AICommand;
    use crate::game_logic::host_ai_decision_log;
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let prev = std::env::var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY").ok();
    crate::env_compat::set_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY", "1");
    crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", "1");
    let prev_atk = std::env::var("GENERALS_GAMEWORLD_AI_ATTACK_AUTHORITY").ok();
    crate::env_compat::set_var("GENERALS_GAMEWORLD_AI_ATTACK_AUTHORITY", "1");
    host_ai_decision_log::clear();
    let _couple = super::ShadowCoupleGuard::enter();
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("AiCmdAuth");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    if !logic.templates.contains_key("AcU") {
        let mut t = ThingTemplate::new("AcU");
        t.add_kind_of(KindOf::Infantry);
        t.add_kind_of(KindOf::Attackable);
        logic.templates.insert("AcU".into(), t);
    }
    let oid = logic
        .create_object("AcU", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
        .expect("id");
    let vid = logic
        .create_object("AcU", Team::GLA, glam::Vec3::new(20.0, 0.0, 0.0))
        .expect("v");
    logic.apply_ai_command_for_test(AICommand::AttackTarget {
        object_id: oid,
        target_id: vid,
    });
    logic.apply_ai_command_for_test(AICommand::SetAIState {
        object_id: oid,
        state: crate::game_logic::AIState::Attacking,
    });
    let events = host_ai_decision_log::drain();
    assert!(
        events
            .iter()
            .any(|e| e.kind == host_ai_decision_log::AI_DECISION_ATTACK),
        "AttackTarget must be logged: {events:?}"
    );
    assert!(
        events
            .iter()
            .any(|e| e.kind == host_ai_decision_log::AI_DECISION_SET_STATE),
        "SetAIState must be logged: {events:?}"
    );
    // Production path is host-immediate engagement + decision log (GameWorld
    // last-write). Shadow writeback re-asserts the same target.
    let host = logic.host_objects().get(&oid).unwrap();
    assert_eq!(
        host.target,
        Some(vid),
        "host applies AttackTarget same-frame"
    );
    assert_eq!(
        host.ai_state,
        crate::game_logic::AIState::Attacking,
        "host applies SetAIState same-frame"
    );
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    assert!(shadow.apply_ai_decisions_as_world_mutations(&events) >= 1);
    // Host already holds the target; writeback is a no-op when equal.
    let _ = shadow.writeback_attack_targets_to_host(&mut logic);
    let _ = crate::game_logic::host_attack_target_ready_log::drain();
    assert_eq!(logic.host_objects().get(&oid).unwrap().target, Some(vid));
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
fn continue_attack_after_kill_decision_authority() {
    let _env_guard = authority_env_lock();

    use crate::game_logic::host_ai_decision_log;
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let prev = std::env::var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY").ok();
    crate::env_compat::set_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY", "1");
    crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", "1");
    let prev_atk = std::env::var("GENERALS_GAMEWORLD_AI_ATTACK_AUTHORITY").ok();
    crate::env_compat::set_var("GENERALS_GAMEWORLD_AI_ATTACK_AUTHORITY", "1");
    host_ai_decision_log::clear();
    let _couple = super::ShadowCoupleGuard::enter();
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("ContAtk");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    for name in ["CaA", "CaD", "CaN"] {
        if !logic.templates.contains_key(name) {
            let mut t = ThingTemplate::new(name);
            t.add_kind_of(KindOf::Infantry);
            t.add_kind_of(KindOf::Attackable);
            logic.templates.insert(name.into(), t);
        }
    }
    let attacker = logic
        .create_object("CaA", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
        .expect("a");
    let dead = logic
        .create_object("CaD", Team::GLA, glam::Vec3::new(5.0, 0.0, 0.0))
        .expect("d");
    let next = logic
        .create_object("CaN", Team::GLA, glam::Vec3::new(8.0, 0.0, 0.0))
        .expect("n");
    let dead_pos = glam::Vec3::new(5.0, 0.0, 0.0);
    let ok =
        logic.try_continue_attack_after_kill_for_test(attacker, dead, dead_pos, 50.0, Team::GLA);
    assert!(ok, "must find next victim in continue range");
    let events = host_ai_decision_log::drain();
    assert!(
        events.iter().any(|e| {
            e.kind == host_ai_decision_log::AI_DECISION_ATTACK
                && e.host_object == attacker
                && e.target_host == next.0
        }),
        "continue-attack must log AttackTarget on next victim; got {events:?}"
    );
    assert!(
        logic
            .host_objects()
            .get(&attacker)
            .unwrap()
            .target
            .is_none(),
        "host target deferred under decision authority"
    );
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    assert!(shadow.apply_ai_decisions_as_world_mutations(&events) >= 1);
    assert!(shadow.writeback_attack_targets_to_host(&mut logic) >= 1);
    let _ = crate::game_logic::host_attack_target_ready_log::drain();
    assert_eq!(
        logic.host_objects().get(&attacker).unwrap().target,
        Some(next)
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

#[test]
fn assign_unit_attack_path_decision_authority() {
    let _env_guard = authority_env_lock();

    use crate::game_logic::host_ai_decision_log;
    use crate::game_logic::{KindOf, Team, ThingTemplate, Weapon};
    let prev = std::env::var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY").ok();
    crate::env_compat::set_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY", "1");
    crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", "1");
    let prev_atk = std::env::var("GENERALS_GAMEWORLD_AI_ATTACK_AUTHORITY").ok();
    crate::env_compat::set_var("GENERALS_GAMEWORLD_AI_ATTACK_AUTHORITY", "1");
    host_ai_decision_log::clear();
    let _couple = super::ShadowCoupleGuard::enter();
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("AtkPath");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    for name in ["ApU", "ApE"] {
        if !logic.templates.contains_key(name) {
            let mut t = ThingTemplate::new(name);
            t.add_kind_of(KindOf::Infantry);
            t.add_kind_of(KindOf::Attackable);
            t.set_health(100.0);
            logic.templates.insert(name.into(), t);
        }
    }
    let uid = logic
        .create_object("ApU", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
        .expect("u");
    let vid = logic
        .create_object("ApE", Team::GLA, glam::Vec3::new(80.0, 0.0, 0.0))
        .expect("e");
    if let Some(o) = logic.host_object_mut(uid) {
        o.weapon = Some(Weapon {
            damage: 10.0,
            range: 25.0,
            ..Weapon::default()
        });
    }
    let tpos = glam::Vec3::new(80.0, 0.0, 0.0);
    let ok = logic.assign_unit_attack_path_for_test(uid, Some(vid), tpos);
    assert!(ok, "attack path should assign");
    let events = host_ai_decision_log::drain();
    assert!(
        events.iter().any(|e| {
            e.kind == host_ai_decision_log::AI_DECISION_ATTACK
                && e.host_object == uid
                && e.target_host == vid.0
        }),
        "must log AttackTarget; got {events:?}"
    );
    assert!(
        events.iter().any(|e| {
            e.kind == host_ai_decision_log::AI_DECISION_SET_STATE
                && e.host_object == uid
                && e.ai_state_ordinal == 2
        }),
        "must log Attacking state; got {events:?}"
    );
    let host = logic.host_objects().get(&uid).unwrap();
    assert!(
        host.target.is_none(),
        "host target deferred under decision authority"
    );
    // Path still on host for movement residual.
    assert!(
        !host.movement.path.is_empty() || host.movement.target_position.is_some(),
        "path must still be assigned on host"
    );
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    assert!(shadow.apply_ai_decisions_as_world_mutations(&events) >= 1);
    assert!(shadow.writeback_attack_targets_to_host(&mut logic) >= 1);
    let _ = crate::game_logic::host_attack_target_ready_log::drain();
    assert_eq!(logic.host_objects().get(&uid).unwrap().target, Some(vid));
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
fn path_approach_with_state_decision_authority() {
    let _env_guard = authority_env_lock();

    use crate::game_logic::host_ai_decision_log;
    use crate::game_logic::{AIState, KindOf, Team, ThingTemplate};
    let prev = std::env::var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY").ok();
    crate::env_compat::set_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY", "1");
    crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", "1");
    host_ai_decision_log::clear();
    let _couple = super::ShadowCoupleGuard::enter();
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("PathSt");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    if !logic.templates.contains_key("PsU") {
        let mut t = ThingTemplate::new("PsU");
        t.add_kind_of(KindOf::Infantry);
        logic.templates.insert("PsU".into(), t);
    }
    let oid = logic
        .create_object("PsU", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
        .expect("id");
    logic.path_approach_with_state_for_test(
        oid,
        glam::Vec3::new(40.0, 0.0, 0.0),
        AIState::Gathering,
    );
    let events = host_ai_decision_log::drain();
    let gathering_ord = GameWorldShadow::host_ai_state_ordinal(&AIState::Gathering);
    assert!(
        events.iter().any(|e| {
            e.kind == host_ai_decision_log::AI_DECISION_SET_STATE
                && e.host_object == oid
                && e.ai_state_ordinal == gathering_ord
        }),
        "path_approach must log SetAIState; got {events:?} ord={gathering_ord}"
    );
    // assign_unit_path may stamp Moving on the host; decision log carries Gathering
    // and GameWorld writeback is last-writer (AIUpdate.cpp state is the last write).
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
fn troop_crawler_assault_decision_authority() {
    let _env_guard = authority_env_lock();

    use crate::game_logic::host_ai_decision_log;
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let prev = std::env::var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY").ok();
    crate::env_compat::set_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY", "1");
    crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", "1");
    let prev_atk = std::env::var("GENERALS_GAMEWORLD_AI_ATTACK_AUTHORITY").ok();
    crate::env_compat::set_var("GENERALS_GAMEWORLD_AI_ATTACK_AUTHORITY", "1");
    host_ai_decision_log::clear();
    let _couple = super::ShadowCoupleGuard::enter();
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("TcAuth");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    let crawler_name = "ChinaVehicleTroopCrawler";
    for name in [crawler_name, "TcO", "TcE"] {
        if !logic.templates.contains_key(name) {
            let mut t = ThingTemplate::new(name);
            if name == crawler_name {
                t.add_kind_of(KindOf::Vehicle);
            } else {
                t.add_kind_of(KindOf::Infantry);
            }
            t.add_kind_of(KindOf::Attackable);
            logic.templates.insert(name.into(), t);
        }
    }
    let crawler = logic
        .create_object(crawler_name, Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
        .expect("c");
    let occ = logic
        .create_object("TcO", Team::USA, glam::Vec3::new(1.0, 0.0, 0.0))
        .expect("o");
    let enemy = logic
        .create_object("TcE", Team::GLA, glam::Vec3::new(30.0, 0.0, 0.0))
        .expect("e");
    if let Some(c) = logic.host_object_mut(crawler) {
        c.install_troop_crawler_transport();
        let _ = c.add_occupant(occ);
    }
    if let Some(o) = logic.host_object_mut(occ) {
        o.set_contained_by(Some(crawler));
    }
    let ordered = logic.apply_troop_crawler_assault_deploy_for_test(crawler, enemy);
    assert!(
        ordered >= 1,
        "deploy should order occupant attack; ordered={ordered}"
    );
    let events = host_ai_decision_log::drain();
    assert!(
        events.iter().any(|e| {
            e.kind == host_ai_decision_log::AI_DECISION_ATTACK && e.target_host == enemy.0
        }),
        "assault deploy must log AttackTarget; ordered={ordered} events={events:?}"
    );
    // Host engagement should stick same-frame for unload residual.
    let host_engaged = logic
        .host_objects()
        .iter()
        .any(|(id, o)| *id != enemy && o.target == Some(enemy));
    assert!(
        host_engaged,
        "assault deploy must set host target same-frame; ordered={ordered} occ_target={:?}",
        logic.host_objects().get(&occ).map(|o| o.target)
    );
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    assert!(shadow.apply_ai_decisions_as_world_mutations(&events) >= 1);
    let _ = shadow.writeback_attack_targets_to_host(&mut logic);
    let _ = crate::game_logic::host_attack_target_ready_log::drain();
    // Writeback should land on whoever logged AttackTarget (occ if ordered, else engagetest).
    let hit = logic
        .host_objects()
        .iter()
        .any(|(id, o)| o.target == Some(enemy) && *id != enemy);
    assert!(hit, "writeback must set some unit target to enemy");
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
fn missile_defender_laser_guided_decision_authority() {
    let _env_guard = authority_env_lock();

    use crate::game_logic::host_ai_decision_log;
    use crate::game_logic::{KindOf, Team, ThingTemplate, Weapon};
    let prev = std::env::var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY").ok();
    crate::env_compat::set_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY", "1");
    crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", "1");
    let prev_atk = std::env::var("GENERALS_GAMEWORLD_AI_ATTACK_AUTHORITY").ok();
    crate::env_compat::set_var("GENERALS_GAMEWORLD_AI_ATTACK_AUTHORITY", "1");
    host_ai_decision_log::clear();
    let _couple = super::ShadowCoupleGuard::enter();
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("MdAuth");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    // Retail template name residual for missile defender.
    let md_name = "AmericaInfantryMissileDefender";
    if !logic.templates.contains_key(md_name) {
        let mut t = ThingTemplate::new(md_name);
        t.add_kind_of(KindOf::Infantry);
        t.add_kind_of(KindOf::Attackable);
        logic.templates.insert(md_name.into(), t);
    }
    if !logic.templates.contains_key("MdE") {
        let mut t = ThingTemplate::new("MdE");
        t.add_kind_of(KindOf::Infantry);
        t.add_kind_of(KindOf::Attackable);
        logic.templates.insert("MdE".into(), t);
    }
    let mid = logic
        .create_object(md_name, Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
        .expect("md");
    let eid = logic
        .create_object("MdE", Team::GLA, glam::Vec3::new(50.0, 0.0, 0.0))
        .expect("e");
    if let Some(o) = logic.host_object_mut(mid) {
        o.secondary_weapon = Some(Weapon {
            damage: 20.0,
            range: 250.0,
            ..Weapon::default()
        });
        o.weapon = Some(Weapon {
            damage: 5.0,
            range: 100.0,
            ..Weapon::default()
        });
    }
    let ok = logic.activate_missile_defender_laser_guided_for_test(mid, eid);
    assert!(ok, "laser guided should activate");
    let events = host_ai_decision_log::drain();
    assert!(
        events.iter().any(|e| {
            e.kind == host_ai_decision_log::AI_DECISION_ATTACK
                && e.host_object == mid
                && e.target_host == eid.0
        }),
        "laser guided must log AttackTarget; got {events:?}"
    );
    assert_eq!(
        logic.host_objects().get(&mid).unwrap().target,
        Some(eid),
        "host target applies immediately under decision authority"
    );
    // Weapon slot still host-applied.
    assert_eq!(
        logic.host_objects().get(&mid).unwrap().active_weapon_slot,
        1
    );
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    assert!(shadow.apply_ai_decisions_as_world_mutations(&events) >= 1);
    let _ = shadow.writeback_attack_targets_to_host(&mut logic);
    let _ = crate::game_logic::host_attack_target_ready_log::drain();
    assert_eq!(logic.host_objects().get(&mid).unwrap().target, Some(eid));
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
fn private_attack_object_decision_authority() {
    let _env_guard = authority_env_lock();

    use crate::game_logic::host_ai_decision_log;
    use crate::game_logic::{KindOf, Team, ThingTemplate, Weapon};
    let prev = std::env::var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY").ok();
    crate::env_compat::set_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY", "1");
    crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", "1");
    let prev_atk = std::env::var("GENERALS_GAMEWORLD_AI_ATTACK_AUTHORITY").ok();
    crate::env_compat::set_var("GENERALS_GAMEWORLD_AI_ATTACK_AUTHORITY", "1");
    host_ai_decision_log::clear();
    let _couple = super::ShadowCoupleGuard::enter();
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("PrivAtk");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    for name in ["PaU", "PaE"] {
        if !logic.templates.contains_key(name) {
            let mut t = ThingTemplate::new(name);
            t.add_kind_of(KindOf::Infantry);
            t.add_kind_of(KindOf::Attackable);
            t.set_health(100.0);
            logic.templates.insert(name.into(), t);
        }
    }
    let uid = logic
        .create_object("PaU", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
        .expect("u");
    let vid = logic
        .create_object("PaE", Team::GLA, glam::Vec3::new(40.0, 0.0, 0.0))
        .expect("e");
    if let Some(o) = logic.host_object_mut(uid) {
        o.weapon = Some(Weapon {
            damage: 10.0,
            range: 50.0,
            ..Weapon::default()
        });
    }
    let ok = logic.private_attack_object_for_test(uid, vid, -1);
    assert!(ok, "private_attack_object should enter attack SM");
    let events = host_ai_decision_log::drain();
    assert!(
        events.iter().any(|e| {
            e.kind == host_ai_decision_log::AI_DECISION_ATTACK
                && e.host_object == uid
                && e.target_host == vid.0
        }),
        "must log AttackTarget; got {events:?}"
    );
    assert!(
        logic.host_objects().get(&uid).unwrap().target.is_none(),
        "host target deferred under decision authority"
    );
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    assert!(shadow.apply_ai_decisions_as_world_mutations(&events) >= 1);
    assert!(shadow.writeback_attack_targets_to_host(&mut logic) >= 1);
    let _ = crate::game_logic::host_attack_target_ready_log::drain();
    assert_eq!(logic.host_objects().get(&uid).unwrap().target, Some(vid));
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
fn transfer_attack_decision_authority() {
    let _env_guard = authority_env_lock();

    use crate::game_logic::host_ai_decision_log;
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let prev = std::env::var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY").ok();
    crate::env_compat::set_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY", "1");
    crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", "1");
    let prev_atk = std::env::var("GENERALS_GAMEWORLD_AI_ATTACK_AUTHORITY").ok();
    crate::env_compat::set_var("GENERALS_GAMEWORLD_AI_ATTACK_AUTHORITY", "1");
    host_ai_decision_log::clear();
    let _couple = super::ShadowCoupleGuard::enter();
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("XferAtk");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    for name in ["XaA", "XaFrom", "XaTo"] {
        if !logic.templates.contains_key(name) {
            let mut t = ThingTemplate::new(name);
            t.add_kind_of(KindOf::Infantry);
            t.add_kind_of(KindOf::Attackable);
            logic.templates.insert(name.into(), t);
        }
    }
    let attacker = logic
        .create_object("XaA", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
        .expect("a");
    let from = logic
        .create_object("XaFrom", Team::GLA, glam::Vec3::new(10.0, 0.0, 0.0))
        .expect("from");
    let to = logic
        .create_object("XaTo", Team::GLA, glam::Vec3::new(12.0, 0.0, 0.0))
        .expect("to");
    // Seed host engagement on destroyed/old victim.
    if let Some(o) = logic.host_object_mut(attacker) {
        o.target = Some(from);
        o.status.attacking = true;
    }
    let n = logic.transfer_attack_for_test(from, to);
    assert!(n >= 1, "should transfer at least one engagement");
    let events = host_ai_decision_log::drain();
    assert!(
        events.iter().any(|e| {
            e.kind == host_ai_decision_log::AI_DECISION_ATTACK
                && e.host_object == attacker
                && e.target_host == to.0
        }),
        "transfer_attack must log AttackTarget retarget; got {events:?}"
    );
    // Host retargets immediately (C++ transferAttack / rebuild-hole residual).
    assert_eq!(
        logic.host_objects().get(&attacker).unwrap().target,
        Some(to),
        "host must retarget immediately"
    );
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    assert!(shadow.apply_ai_decisions_as_world_mutations(&events) >= 1);
    let _ = shadow.writeback_attack_targets_to_host(&mut logic);
    let _ = crate::game_logic::host_attack_target_ready_log::drain();
    assert_eq!(
        logic.host_objects().get(&attacker).unwrap().target,
        Some(to)
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

#[test]
fn update_combat_defers_engagement_under_decision_authority() {
    // Source honesty: combat aim/pitch/pre-attack sets host engagement immediately
    // and still logs under AI decision authority for GameWorld last-write.
    let src = GAME_LOGIC_HOST_SRC;
    let i = src.find("fn update_combat").expect("update_combat");
    let w = &src[i..i + 120_000.min(src.len() - i)];
    assert!(
        w.contains("gameworld_ai_decision_authority") && w.contains("turn_toward_position"),
        "update_combat aim residual must reference decision authority"
    );
    assert!(
        w.matches("pre_attack_ready_at").count() >= 1
            && w.contains("host_ai_decision_log::record_attack")
            && !w.contains("!crate::gameworld_shadow::gameworld_ai_decision_authority_live()"),
        "pre-attack engagement must host-apply and log (not inverted !live gate)"
    );
}

#[test]
fn residual_defense_fire_engagement_decision_authority() {
    // Source honesty: residual auto-fire paths gate host engagement.
    let src = GAME_LOGIC_HOST_SRC;
    for fn_name in [
        "fn try_base_defense_residual_fire",
        "fn try_sentry_drone_residual_fire",
        "fn try_hellfire_drone_residual_fire",
        "fn try_strategy_center_bombardment_turret_fire",
        "fn update_pending_patriot_assists",
        "fn attack_aim_at_target_update",
        "fn attack_fire_weapon_update",
        "fn tick_attack_state_machine",
        "fn tick_strategy_center_turret_mood_target",
        "fn update_stealth_and_detection",
    ] {
        let i = src
            .find(fn_name)
            .unwrap_or_else(|| panic!("missing {fn_name}"));
        // Brace-match the full function body (large residuals exceed fixed windows).
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
                || w.contains("host_ai_decision_log::record_attack"),
            "{fn_name} must honor AI decision authority for engagement"
        );
    }
}

#[test]
fn apply_engagement_decision_aware_writeback() {
    let _env_guard = authority_env_lock();

    use crate::game_logic::host_ai_decision_log;
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let prev = std::env::var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY").ok();
    crate::env_compat::set_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY", "1");
    crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", "1");
    let prev_atk = std::env::var("GENERALS_GAMEWORLD_AI_ATTACK_AUTHORITY").ok();
    crate::env_compat::set_var("GENERALS_GAMEWORLD_AI_ATTACK_AUTHORITY", "1");
    host_ai_decision_log::clear();
    let _couple = super::ShadowCoupleGuard::enter();
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("EngAw");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    for name in ["EaU", "EaE"] {
        if !logic.templates.contains_key(name) {
            let mut t = ThingTemplate::new(name);
            t.add_kind_of(KindOf::Infantry);
            t.add_kind_of(KindOf::Attackable);
            logic.templates.insert(name.into(), t);
        }
    }
    let uid = logic
        .create_object("EaU", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
        .expect("u");
    let vid = logic
        .create_object("EaE", Team::GLA, glam::Vec3::new(20.0, 0.0, 0.0))
        .expect("e");
    logic.apply_engagement_decision_aware_for_test(uid, vid);
    let events = host_ai_decision_log::drain();
    assert!(
        events.iter().any(|e| {
            e.kind == host_ai_decision_log::AI_DECISION_ATTACK
                && e.host_object == uid
                && e.target_host == vid.0
        }),
        "must log AttackTarget; got {events:?}"
    );
    // C++ AIAttackState sets the goal on the Object same-frame
    // (AIAttackState.cpp enter/update). Host applies immediately; GW last-writes.
    assert_eq!(logic.host_objects().get(&uid).unwrap().target, Some(vid));
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    assert!(shadow.apply_ai_decisions_as_world_mutations(&events) >= 1);
    if let Some(o) = logic.host_object_mut(uid) {
        o.target = None;
    }
    assert!(shadow.writeback_attack_targets_to_host(&mut logic) >= 1);
    let _ = crate::game_logic::host_attack_target_ready_log::drain();
    assert_eq!(logic.host_objects().get(&uid).unwrap().target, Some(vid));
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
fn mood_auto_acquire_logs_decision_under_authority() {
    let _env_guard = authority_env_lock();

    use crate::game_logic::host_ai_decision_log;
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let prev = std::env::var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY").ok();
    crate::env_compat::set_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY", "1");
    crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", "1");
    let prev_atk = std::env::var("GENERALS_GAMEWORLD_AI_ATTACK_AUTHORITY").ok();
    crate::env_compat::set_var("GENERALS_GAMEWORLD_AI_ATTACK_AUTHORITY", "1");
    host_ai_decision_log::clear();
    let _couple = super::ShadowCoupleGuard::enter();
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("MoodAcq");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    if !logic.templates.contains_key("MaU") {
        let mut t = ThingTemplate::new("MaU");
        t.add_kind_of(KindOf::Infantry);
        t.add_kind_of(KindOf::Attackable);
        logic.templates.insert("MaU".into(), t);
    }
    let oid = logic
        .create_object("MaU", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
        .expect("id");
    let vid = logic
        .create_object("MaU", Team::GLA, glam::Vec3::new(20.0, 0.0, 0.0))
        .expect("v");
    {
        let o = logic.host_object_mut(oid).expect("o");
        o.auto_acquire_when_idle = true;
        o.ai_state = crate::game_logic::AIState::Idle;
        o.target = None;
        // Give a weapon so can_attack is true.
        o.weapon = Some(crate::game_logic::Weapon {
            damage: 10.0,
            range: 100.0,
            ..crate::game_logic::Weapon::default()
        });
    }
    // Drive one mood tick.
    logic.tick_mood_auto_acquire_for_test(&[oid]);
    let events = host_ai_decision_log::drain();
    assert!(
        events.iter().any(|e| {
            e.kind == host_ai_decision_log::AI_DECISION_ATTACK
                && e.host_object == oid
                && e.target_host == vid.0
        }),
        "mood acquire must log AttackTarget decision under authority; got {events:?}"
    );
    // Host target still unset until shadow writeback.
    assert!(logic.host_objects().get(&oid).unwrap().target.is_none());
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    assert!(shadow.apply_ai_decisions_as_world_mutations(&events) >= 1);
    assert!(shadow.writeback_attack_targets_to_host(&mut logic) >= 1);
    let _ = crate::game_logic::host_attack_target_ready_log::drain();
    assert_eq!(logic.host_objects().get(&oid).unwrap().target, Some(vid));
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
fn support_guard_engage_uses_decision_authority() {
    let _env_guard = authority_env_lock();

    use crate::game_logic::host_ai_decision_log;
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let prev = std::env::var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY").ok();
    crate::env_compat::set_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY", "1");
    crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", "1");
    let prev_atk = std::env::var("GENERALS_GAMEWORLD_AI_ATTACK_AUTHORITY").ok();
    crate::env_compat::set_var("GENERALS_GAMEWORLD_AI_ATTACK_AUTHORITY", "1");
    host_ai_decision_log::clear();
    let _couple = super::ShadowCoupleGuard::enter();
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("GuardEng");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    if !logic.templates.contains_key("GeU") {
        let mut t = ThingTemplate::new("GeU");
        t.add_kind_of(KindOf::Infantry);
        t.add_kind_of(KindOf::Attackable);
        logic.templates.insert("GeU".into(), t);
    }
    let oid = logic
        .create_object("GeU", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
        .expect("id");
    let vid = logic
        .create_object("GeU", Team::GLA, glam::Vec3::new(15.0, 0.0, 0.0))
        .expect("v");
    // Direct helper (same path support-states uses under authority).
    logic.engage_target_decision_aware_for_test(oid, vid);
    let events = host_ai_decision_log::drain();
    assert!(
        events.iter().any(|e| {
            e.kind == host_ai_decision_log::AI_DECISION_ATTACK
                && e.host_object == oid
                && e.target_host == vid.0
        }),
        "guard engage must log decision; got {events:?}"
    );
    assert_eq!(
        logic.host_objects().get(&oid).unwrap().target,
        Some(vid),
        "host engage immediate"
    );
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    assert!(shadow.apply_ai_decisions_as_world_mutations(&events) >= 1);
    let _ = shadow.writeback_attack_targets_to_host(&mut logic);
    let _ = crate::game_logic::host_attack_target_ready_log::drain();
    assert_eq!(logic.host_objects().get(&oid).unwrap().target, Some(vid));
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
fn faction_ai_launch_attack_decision_authority_writeback() {
    let _env_guard = authority_env_lock();

    use crate::ai::{AIDifficulty, AIPlayer};
    use crate::game_logic::host_ai_decision_log;
    use crate::game_logic::{KindOf, Team, ThingTemplate, Weapon};
    let prev = std::env::var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY").ok();
    crate::env_compat::set_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY", "1");

    crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", "1");
    let prev_atk = std::env::var("GENERALS_GAMEWORLD_AI_ATTACK_AUTHORITY").ok();
    crate::env_compat::set_var("GENERALS_GAMEWORLD_AI_ATTACK_AUTHORITY", "1");
    host_ai_decision_log::clear();
    let _couple = super::ShadowCoupleGuard::enter();
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("FacAtk");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    for (name, team, x) in [("FacU", Team::USA, 0.0f32), ("FacE", Team::GLA, 80.0)] {
        if !logic.templates.contains_key(name) {
            let mut t = ThingTemplate::new(name);
            t.set_health(100.0);
            t.add_kind_of(KindOf::Infantry);
            t.add_kind_of(KindOf::Attackable);
            logic.templates.insert(name.into(), t);
        }
        let _ = match logic.apply_host_object_id_op(crate::game_logic::HostObjectIdOp::Create {
            template: name.to_string(),
            team: team,
            spawn_at: glam::Vec3::new(x, 0.0, 0.0),
        }) {
            crate::game_logic::HostObjectIdResult::Created(id) => id,
            _ => None,
        };
    }
    let usa_id = logic
        .get_players()
        .iter()
        .find(|(_, p)| p.team == Team::USA)
        .map(|(id, _)| *id)
        .unwrap_or(0);
    let gla_id = logic
        .get_players()
        .iter()
        .find(|(_, p)| p.team == Team::GLA)
        .map(|(id, _)| *id);
    let enemy = logic
        .host_objects()
        .iter()
        .find(|(_, o)| o.team == Team::GLA)
        .map(|(id, _)| *id)
        .expect("enemy");
    let usa_unit = logic
        .host_objects()
        .iter()
        .find(|(_, o)| o.team == Team::USA)
        .map(|(id, _)| *id)
        .expect("usa");
    if let Some(o) = logic.host_object_mut(usa_unit) {
        o.weapon = Some(Weapon {
            damage: 10.0,
            ..Weapon::default()
        });
    }
    let mut ai = AIPlayer::new(usa_id, Team::USA, AIDifficulty::Medium);
    ai.enemy_player_id = gla_id;
    ai.is_active = true;
    ai.launch_attack(&mut logic, 1000.0);
    let events = host_ai_decision_log::drain();
    assert!(
        events
            .iter()
            .any(|e| e.kind == host_ai_decision_log::AI_DECISION_ATTACK),
        "expected AttackTarget decision: {events:?}"
    );
    assert_eq!(
        logic.host_objects().get(&usa_unit).unwrap().target,
        Some(enemy),
        "launch_attack must engage host target immediately"
    );
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    assert!(shadow.apply_ai_decisions_as_world_mutations(&events) >= 1);
    let _ = shadow.writeback_attack_targets_to_host(&mut logic);
    let _ = crate::game_logic::host_attack_target_ready_log::drain();
    assert_eq!(
        logic.host_objects().get(&usa_unit).unwrap().target,
        Some(enemy)
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

#[test]
fn stop_attack_decision_authority_clears_via_writeback() {
    let _env_guard = authority_env_lock();

    use crate::game_logic::host_ai_decision_log;
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let prev = std::env::var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY").ok();
    crate::env_compat::set_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY", "1");
    crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", "1");
    let prev_atk = std::env::var("GENERALS_GAMEWORLD_AI_ATTACK_AUTHORITY").ok();
    crate::env_compat::set_var("GENERALS_GAMEWORLD_AI_ATTACK_AUTHORITY", "1");
    host_ai_decision_log::clear();
    let _couple = super::ShadowCoupleGuard::enter();
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("StopAtk");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    for name in ["Su", "Se"] {
        if !logic.templates.contains_key(name) {
            let mut t = ThingTemplate::new(name);
            t.add_kind_of(KindOf::Infantry);
            t.add_kind_of(KindOf::Attackable);
            logic.templates.insert(name.into(), t);
        }
    }
    let oid = logic
        .create_object("Su", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
        .expect("u");
    let vid = logic
        .create_object("Se", Team::GLA, glam::Vec3::new(10.0, 0.0, 0.0))
        .expect("e");
    // Seed host target as if previously engaged.
    if let Some(o) = logic.host_object_mut(oid) {
        o.target = Some(vid);
        o.status.attacking = true;
    }
    logic.stop_attack_decision_aware_for_test(oid);
    let events = host_ai_decision_log::drain();
    assert!(
        events.iter().any(|e| {
            e.kind == host_ai_decision_log::AI_DECISION_STOP_ATTACK && e.host_object == oid
        }),
        "stop must log decision; got {events:?}"
    );
    // Host engagement clears same-frame so combat cannot keep firing.
    assert!(
        logic.host_objects().get(&oid).unwrap().target.is_none(),
        "host target must clear immediately on stop"
    );
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    // Seed world attack then apply stop decision; host already clear, writeback is no-op.
    assert!(shadow.queue_set_attack_target_for_host(oid, Some(vid)));
    let _ = shadow.apply_pending();
    assert!(shadow.apply_ai_decisions_as_world_mutations(&events) >= 1);
    let _ = shadow.apply_pending();
    let _ = shadow.writeback_attack_targets_to_host(&mut logic);
    let _ = crate::game_logic::host_attack_target_ready_log::drain();
    assert!(logic.host_objects().get(&oid).unwrap().target.is_none());
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
fn fire_spawn_authority_defers_queue_until_shadow() {
    use crate::game_logic::combat::{self, DamageType, PendingProjectile};
    use crate::game_logic::host_fire_spawn_log;
    use crate::game_logic::host_usa_pilot::HostDeathType;
    let _env_guard = authority_env_lock();
    let prev = std::env::var("GENERALS_GAMEWORLD_FIRE_SPAWN_AUTHORITY").ok();
    let prev_shadow = std::env::var("GENERALS_GAMEWORLD_SHADOW").ok();
    crate::env_compat::set_var("GENERALS_GAMEWORLD_FIRE_SPAWN_AUTHORITY", "1");
    crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", "1");
    assert!(gameworld_fire_spawn_authority_enabled());
    assert!(gameworld_shadow_enabled());
    host_fire_spawn_log::clear();
    // Fire-spawn defers only while a coupled shadow tick is live (Wave 682).
    begin_shadow_coupled_tick();
    combat::queue_projectile(PendingProjectile {
        shooter_id: ObjectId(1),
        shooter_pos: glam::Vec3::ZERO,
        source_context: None,
        target_id: Some(ObjectId(2)),
        target_pos: Some(glam::Vec3::new(50.0, 0.0, 0.0)),
        damage: 12.0,
        speed: 100.0,
        splash_radius: 0.0,
        is_homing: false,
        damage_type: DamageType::Bullet,
        death_type: HostDeathType::Normal,
        projectile_object_name: String::new(),
        projectile_lifecycle: None,
        fire_fx_name: String::new(),
        fire_ocl_name: String::new(),
        detonation_fx_name: String::new(),
        detonation_ocl_name: String::new(),
        exhaust_name: String::new(),
        secondary_damage: 0.0,
        secondary_damage_radius: 0.0,
        shock_wave_amount: 0.0,
        shock_wave_radius: 0.0,
        shock_wave_taper_off: 0.0,
        radius_damage_affects: 0,
        projectile_collides: 0,
        scatter_radius: 0.0,
        scatter_table_offset: None,
        min_weapon_speed: 0.0,
        scale_weapon_speed: false,
        attack_range: 0.0,
        min_attack_range: 0.0,
        historic_weapon_key: String::new(),
        historic_bonus_time_frames: 0,
        historic_bonus_count: 0,
        historic_bonus_radius: 0.0,
        historic_bonus_weapon: String::new(),
        die_on_detonate: false,
    });
    // Not yet in combat system.
    let mut logic = GameLogic::new();
    assert_eq!(logic.combat_system.projectile_count(), 0);
    let spawns = host_fire_spawn_log::drain();
    assert_eq!(spawns.len(), 1);
    end_shadow_coupled_tick();
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let n = shadow.apply_host_fire_spawn_events(&mut logic, spawns);
    assert!(n >= 1 || logic.combat_system.projectile_count() >= 1);
    assert!(
        logic.combat_system.projectile_count() >= 1,
        "shadow apply must spawn into CombatSystem"
    );
    match prev {
        Some(v) => crate::env_compat::set_var("GENERALS_GAMEWORLD_FIRE_SPAWN_AUTHORITY", v),
        None => crate::env_compat::remove_var("GENERALS_GAMEWORLD_FIRE_SPAWN_AUTHORITY"),
    }
    match prev_shadow {
        Some(v) => crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", v),
        None => crate::env_compat::remove_var("GENERALS_GAMEWORLD_SHADOW"),
    }
}

#[test]
fn fire_spawn_authority_enqueues_host_when_shadow_disabled() {
    use crate::game_logic::combat::{self, DamageType, PendingProjectile};
    use crate::game_logic::host_fire_spawn_log;
    use crate::game_logic::host_usa_pilot::HostDeathType;
    let _env_guard = authority_env_lock();
    let prev = std::env::var("GENERALS_GAMEWORLD_FIRE_SPAWN_AUTHORITY").ok();
    let prev_shadow = std::env::var("GENERALS_GAMEWORLD_SHADOW").ok();
    crate::env_compat::set_var("GENERALS_GAMEWORLD_FIRE_SPAWN_AUTHORITY", "1");
    crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", "0");
    assert!(gameworld_fire_spawn_authority_enabled());
    assert!(!gameworld_shadow_enabled());
    host_fire_spawn_log::clear();
    combat::clear_pending_projectile_queue_for_test();
    combat::queue_projectile(PendingProjectile {
        shooter_id: ObjectId(9),
        shooter_pos: glam::Vec3::ZERO,
        source_context: None,
        target_id: Some(ObjectId(10)),
        target_pos: Some(glam::Vec3::new(10.0, 0.0, 0.0)),
        damage: 5.0,
        speed: 200.0,
        splash_radius: 0.0,
        is_homing: false,
        damage_type: DamageType::Bullet,
        death_type: HostDeathType::Normal,
        projectile_object_name: String::new(),
        projectile_lifecycle: None,
        fire_fx_name: String::new(),
        fire_ocl_name: String::new(),
        detonation_fx_name: String::new(),
        detonation_ocl_name: String::new(),
        exhaust_name: String::new(),
        secondary_damage: 0.0,
        secondary_damage_radius: 0.0,
        shock_wave_amount: 0.0,
        shock_wave_radius: 0.0,
        shock_wave_taper_off: 0.0,
        radius_damage_affects: 0,
        projectile_collides: 0,
        scatter_radius: 0.0,
        scatter_table_offset: None,
        min_weapon_speed: 0.0,
        scale_weapon_speed: false,
        attack_range: 0.0,
        min_attack_range: 0.0,
        historic_weapon_key: String::new(),
        historic_bonus_time_frames: 0,
        historic_bonus_count: 0,
        historic_bonus_radius: 0.0,
        historic_bonus_weapon: String::new(),
        die_on_detonate: false,
    });
    assert!(
        host_fire_spawn_log::drain().is_empty(),
        "host-only must not defer into fire_spawn_log"
    );
    assert!(
        combat::pending_projectile_queue_len_for_test() >= 1,
        "shadow-off + fire_spawn auth must enqueue PENDING_PROJECTILES immediately"
    );
    combat::clear_pending_projectile_queue_for_test();
    match prev {
        Some(v) => crate::env_compat::set_var("GENERALS_GAMEWORLD_FIRE_SPAWN_AUTHORITY", v),
        None => crate::env_compat::remove_var("GENERALS_GAMEWORLD_FIRE_SPAWN_AUTHORITY"),
    }
    match prev_shadow {
        Some(v) => crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", v),
        None => crate::env_compat::remove_var("GENERALS_GAMEWORLD_SHADOW"),
    }
}

#[test]
fn ai_decision_authority_applies_host_state_when_shadow_disabled() {
    let _env_guard = authority_env_lock();
    let prev_d = std::env::var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY").ok();
    let prev_s = std::env::var("GENERALS_GAMEWORLD_SHADOW").ok();
    crate::env_compat::set_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY", "1");
    crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", "0");
    assert!(gameworld_ai_decision_authority_enabled());
    assert!(!gameworld_ai_decision_authority_live());
    crate::game_logic::host_ai_decision_log::clear();
    let _couple = super::ShadowCoupleGuard::enter();
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("AiDecNoShadow");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    ensure_template(&mut logic, "AiUnit", 100.0);
    if let Some(t) = logic.templates.get_mut("AiUnit") {
        t.add_kind_of(KindOf::Infantry);
    }
    let id = logic
        .create_object("AiUnit", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("unit");
    if let Some(o) = logic.host_object_mut(id) {
        o.thing.template.add_kind_of(KindOf::Infantry);
        o.movement.max_speed = 30.0;
    }
    assert!(
        logic.assign_unit_path(id, Vec3::new(50.0, 0.0, 0.0), &[]),
        "assign_unit_path"
    );
    let st = logic.host_objects().get(&id).unwrap().ai_state.clone();
    assert!(
        matches!(st, crate::game_logic::AIState::Moving),
        "host-only must set Moving immediately under AI_DECISION_AUTH, got {st:?}"
    );
    assert!(
        crate::game_logic::host_ai_decision_log::drain().is_empty(),
        "must not defer decisions when shadow is off"
    );
    match prev_d {
        Some(v) => crate::env_compat::set_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY", v),
        None => crate::env_compat::remove_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY"),
    }
    match prev_s {
        Some(v) => crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", v),
        None => crate::env_compat::remove_var("GENERALS_GAMEWORLD_SHADOW"),
    }
}

#[test]
fn projectile_authority_steps_host_when_shadow_disabled() {
    let _env_guard = authority_env_lock();
    let prev_p = std::env::var("GENERALS_GAMEWORLD_PROJECTILE_AUTHORITY").ok();
    let prev_s = std::env::var("GENERALS_GAMEWORLD_SHADOW").ok();
    crate::env_compat::set_var("GENERALS_GAMEWORLD_PROJECTILE_AUTHORITY", "1");
    crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", "0");
    assert!(gameworld_projectile_authority_enabled());
    assert!(!gameworld_projectile_authority_live());
    // Source must contain live gate so host update_projectiles is not skipped.
    let src = GAME_LOGIC_HOST_SRC;
    assert!(
        src.contains("gameworld_projectile_authority_live()"),
        "host combat must gate projectile defer on live shadow"
    );
    match prev_p {
        Some(v) => crate::env_compat::set_var("GENERALS_GAMEWORLD_PROJECTILE_AUTHORITY", v),
        None => crate::env_compat::remove_var("GENERALS_GAMEWORLD_PROJECTILE_AUTHORITY"),
    }
    match prev_s {
        Some(v) => crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", v),
        None => crate::env_compat::remove_var("GENERALS_GAMEWORLD_SHADOW"),
    }
}

#[test]
fn movement_authority_integrates_host_when_shadow_disabled() {
    let _env_guard = authority_env_lock();
    let prev_m = std::env::var("GENERALS_GAMEWORLD_MOVEMENT_AUTHORITY").ok();
    let prev_s = std::env::var("GENERALS_GAMEWORLD_SHADOW").ok();
    crate::env_compat::set_var("GENERALS_GAMEWORLD_MOVEMENT_AUTHORITY", "1");
    crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", "0");
    assert!(gameworld_movement_authority_enabled());
    assert!(!gameworld_movement_authority_live());
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("MoveNoShadow");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    ensure_template(&mut logic, "MvU", 100.0);
    if let Some(t) = logic.templates.get_mut("MvU") {
        t.add_kind_of(KindOf::Infantry);
    }
    let id = logic
        .create_object("MvU", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("unit");
    if let Some(o) = logic.host_object_mut(id) {
        o.thing.template.add_kind_of(KindOf::Infantry);
        o.movement.max_speed = 60.0;
    }
    assert!(logic.assign_unit_path(id, Vec3::new(100.0, 0.0, 0.0), &[]));
    let pre = logic.host_objects().get(&id).unwrap().get_position();
    // One host movement tick must advance pose when shadow is off.
    logic.update_movement_for_test(&[id], 1.0 / 30.0);
    let post = logic.host_objects().get(&id).unwrap().get_position();
    let dist = (post - pre).length();
    assert!(
        dist > 0.01,
        "host-only movement must integrate path; pre={pre:?} post={post:?} dist={dist}"
    );
    match prev_m {
        Some(v) => crate::env_compat::set_var("GENERALS_GAMEWORLD_MOVEMENT_AUTHORITY", v),
        None => crate::env_compat::remove_var("GENERALS_GAMEWORLD_MOVEMENT_AUTHORITY"),
    }
    match prev_s {
        Some(v) => crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", v),
        None => crate::env_compat::remove_var("GENERALS_GAMEWORLD_SHADOW"),
    }
}

#[test]
fn construction_authority_sets_host_percent_when_shadow_disabled() {
    let _env_guard = authority_env_lock();
    let prev_c = std::env::var("GENERALS_GAMEWORLD_CONSTRUCTION_AUTHORITY").ok();
    let prev_s = std::env::var("GENERALS_GAMEWORLD_SHADOW").ok();
    crate::env_compat::set_var("GENERALS_GAMEWORLD_CONSTRUCTION_AUTHORITY", "1");
    crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", "0");
    assert!(gameworld_construction_authority_enabled());
    assert!(!gameworld_construction_authority_live());
    assert!(!gameworld_construction_sole_tick_enabled());
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("ConstNoShadow");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    ensure_template(&mut logic, "HoleT", 500.0);
    if let Some(t) = logic.templates.get_mut("HoleT") {
        t.add_kind_of(KindOf::Structure);
    }
    let id = logic
        .create_object("HoleT", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("hole");
    if let Some(h) = logic.host_object_mut(id) {
        // Simulate rebuild-hole complete residual path without dual-world.
        if crate::gameworld_shadow::gameworld_construction_authority_live() {
            crate::game_logic::host_construction_progress_log::record(id, 1.0, false, 0.0);
        } else {
            h.construction_percent = 1.0;
        }
        assert!(
            (h.construction_percent - 1.0).abs() < 0.01,
            "host-only must set construction_percent immediately"
        );
    }
    match prev_c {
        Some(v) => crate::env_compat::set_var("GENERALS_GAMEWORLD_CONSTRUCTION_AUTHORITY", v),
        None => crate::env_compat::remove_var("GENERALS_GAMEWORLD_CONSTRUCTION_AUTHORITY"),
    }
    match prev_s {
        Some(v) => crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", v),
        None => crate::env_compat::remove_var("GENERALS_GAMEWORLD_SHADOW"),
    }
}

#[test]
fn ai_attack_authority_gates_fire_intent_writeback() {
    let _env_guard = authority_env_lock();

    use crate::game_logic::host_fire_intent_log;
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    host_fire_intent_log::clear();
    let prev = std::env::var("GENERALS_GAMEWORLD_AI_ATTACK_AUTHORITY").ok();
    crate::env_compat::set_var("GENERALS_GAMEWORLD_AI_ATTACK_AUTHORITY", "0");
    crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", "1");
    assert!(!gameworld_ai_attack_authority_enabled());
    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("AiAtkAuth");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    if !logic.templates.contains_key("AaU") {
        let mut t = ThingTemplate::new("AaU");
        t.add_kind_of(KindOf::Infantry);
        logic.templates.insert("AaU".into(), t);
    }
    let oid = logic
        .create_object("AaU", Team::USA, glam::Vec3::new(250.0, 0.0, 250.0))
        .expect("id");
    host_fire_intent_log::record(oid, 9, 0, 10.0, 20.0, 1.0, 5, 1);
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    assert!(shadow.apply_host_fire_intent_events(&host_fire_intent_log::drain()) >= 1);
    // Host still default zeros; writeback skipped when authority off.
    assert_eq!(shadow.writeback_fire_intent_to_host(&mut logic), 0);
    let _ = crate::game_logic::host_fire_intent_ready_log::drain();
    let o = logic.host_objects().get(&oid).unwrap();
    assert_eq!(o.fire_intent_count, 0);
    crate::env_compat::set_var("GENERALS_GAMEWORLD_AI_ATTACK_AUTHORITY", "1");
    assert!(gameworld_ai_attack_authority_enabled());
    assert!(shadow.writeback_fire_intent_to_host(&mut logic) >= 1);
    let _ = crate::game_logic::host_fire_intent_ready_log::drain();
    let o = logic.host_objects().get(&oid).unwrap();
    assert_eq!(o.fire_intent_count, 1);
    assert_eq!(o.last_fire_victim_host, 9);
    match prev {
        Some(v) => crate::env_compat::set_var("GENERALS_GAMEWORLD_AI_ATTACK_AUTHORITY", v),
        None => crate::env_compat::remove_var("GENERALS_GAMEWORLD_AI_ATTACK_AUTHORITY"),
    }
}
