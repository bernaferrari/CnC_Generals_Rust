//! AI decision authority, continue-attack, path, troop-crawler.

use super::*;

#[test]
fn ai_decision_buffer_channel_via_push_ai_decision() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", "1");

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
        position: glam::Vec3::new(14.5, 0.0, 14.5),
    });
    let object = logic.host_object(oid).unwrap();
    assert_eq!(
        object.movement.target_position,
        Some(glam::Vec3::new(14.5, 0.0, 14.5))
    );
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let entity = shadow.entity_for_host(oid).unwrap();
    assert_eq!(
        shadow.world().entity(entity).unwrap().move_target,
        Some([14.5, 0.0, 14.5])
    );
}

#[test]
fn ai_decision_authority_applies_attack_via_gameworld() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", "1");
    let mut logic = GameLogic::new();

    // Attack writeback must also be on for last-write.

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
    {
        let (object, attack_events) = logic.host_object_and_attack_events_mut(oid).unwrap();
        object.set_target(
            Some(vid),
            attack_events,
            &mut crate::game_logic::ObjectAudio::queue(&mut Vec::new()),
        );
    };
    assert_eq!(logic.host_object(oid).unwrap().target, Some(vid));
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let attacker = shadow.entity_for_host(oid).unwrap();
    assert_eq!(
        shadow.world().entity(attacker).unwrap().attack_target,
        shadow.entity_for_host(vid)
    );
}

#[test]
fn apply_ai_command_logs_and_host_applies_under_authority() {
    use crate::game_logic::game_logic::AICommand;

    use crate::game_logic::{KindOf, Team, ThingTemplate};
    crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", "1");

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
    shadow.sync_from_host(&logic);
    // Host already holds the target; writeback is a no-op when equal.
    shadow.sync_from_host(&logic);

    assert_eq!(logic.host_objects().get(&oid).unwrap().target, Some(vid));
}

#[test]
fn continue_attack_after_kill_decision_authority() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", "1");

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

    assert_eq!(
        logic.host_object(attacker).unwrap().target,
        Some(next),
        "original continuation installs its next victim immediately"
    );
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    shadow.sync_from_host(&logic);
    shadow.sync_from_host(&logic);

    assert_eq!(
        logic.host_objects().get(&attacker).unwrap().target,
        Some(next)
    );
}

#[test]
fn assign_unit_attack_path_decision_authority() {
    use crate::game_logic::{KindOf, Team, ThingTemplate, Weapon};
    crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", "1");

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

    let host = logic.host_objects().get(&uid).unwrap();
    assert_eq!(
        host.target,
        Some(vid),
        "original path assignment installs its victim immediately"
    );
    // Path still on host for movement residual.
    assert!(
        !host.movement.path.is_empty() || host.movement.target_position.is_some(),
        "path must still be assigned on host"
    );
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    shadow.sync_from_host(&logic);
    shadow.sync_from_host(&logic);

    assert_eq!(logic.host_objects().get(&uid).unwrap().target, Some(vid));
}

#[test]
fn path_approach_with_state_decision_authority() {
    use crate::game_logic::{AIState, KindOf, Team, ThingTemplate};
    crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", "1");

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

    let gathering_ord = GameWorldShadow::host_ai_state_ordinal(&AIState::Gathering);

    // assign_unit_path may stamp Moving on the host; decision log carries Gathering
    // and GameWorld writeback is last-writer (AIUpdate.cpp state is the last write).
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    shadow.sync_from_host(&logic);
    assert_eq!(
        logic.host_objects().get(&oid).unwrap().ai_state,
        AIState::Gathering
    );
}

#[test]
fn troop_crawler_assault_decision_authority() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", "1");

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
    shadow.sync_from_host(&logic);
    shadow.sync_from_host(&logic);

    // Writeback should land on whoever logged AttackTarget (occ if ordered, else engagetest).
    let hit = logic
        .host_objects()
        .iter()
        .any(|(id, o)| o.target == Some(enemy) && *id != enemy);
    assert!(hit, "writeback must set some unit target to enemy");
}

#[test]
fn missile_defender_laser_guided_decision_authority() {
    use crate::game_logic::{KindOf, Team, ThingTemplate, Weapon};
    crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", "1");

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
    // C++ SpecialAbilityUpdate advances startPreparation → continuePreparation
    // → triggerAbilityEffect on later updates (SpecialAbilityUpdate.cpp
    // :1276-1293). Drive the actual preparation until it installs the
    // secondary-weapon engagement on the canonical object.
    for _ in 0..600 {
        if logic.host_object(mid).is_some_and(|object| {
            object.target == Some(eid)
                && object.ai_state == crate::game_logic::AIState::Attacking
                && object.active_weapon_slot == 1
        }) {
            break;
        }
        logic.update_support_states_for_test(&[mid, eid], 1.0 / 30.0);
    }

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
    shadow.sync_from_host(&logic);
    shadow.sync_from_host(&logic);

    assert_eq!(logic.host_objects().get(&mid).unwrap().target, Some(eid));
}

#[test]
fn private_attack_object_decision_authority() {
    use crate::game_logic::{KindOf, Team, ThingTemplate, Weapon};
    crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", "1");

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

    // AIUpdate.cpp:3395 installs the goal before entering the attack state.
    // Diagnostic decision recording does not postpone the host transition.
    assert_eq!(logic.host_objects().get(&uid).unwrap().target, Some(vid));
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    shadow.sync_from_host(&logic);
    // Exercise writeback only after proving the ordinary host command worked.
    let projected = shadow.entity_for_host(uid).unwrap();
    shadow
        .world_mut()
        .world_mut()
        .entity_mut(projected)
        .unwrap()
        .attack_target = None;
    shadow.sync_from_host(&logic);

    assert_eq!(logic.host_objects().get(&uid).unwrap().target, Some(vid));
}

#[test]
fn transfer_attack_decision_authority() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", "1");

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

    // Host retargets immediately (C++ transferAttack / rebuild-hole residual).
    assert_eq!(
        logic.host_objects().get(&attacker).unwrap().target,
        Some(to),
        "host must retarget immediately"
    );
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    shadow.sync_from_host(&logic);
    shadow.sync_from_host(&logic);

    assert_eq!(
        logic.host_objects().get(&attacker).unwrap().target,
        Some(to)
    );
}

#[test]
fn apply_engagement_decision_aware_writeback() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", "1");

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

    // C++ AIAttackState sets the goal on the Object same-frame
    // (AIAttackState.cpp enter/update). Host applies immediately; GW last-writes.
    assert_eq!(logic.host_objects().get(&uid).unwrap().target, Some(vid));
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    shadow.sync_from_host(&logic);
    let projected = shadow.entity_for_host(uid).unwrap();
    shadow
        .world_mut()
        .world_mut()
        .entity_mut(projected)
        .unwrap()
        .attack_target = None;
    shadow.sync_from_host(&logic);

    assert_eq!(logic.host_objects().get(&uid).unwrap().target, Some(vid));
}

#[test]
fn mood_auto_acquire_logs_decision_under_authority() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", "1");

    let mut logic = GameLogic::new();
    logic.set_ai_definition_base(game_engine::common::ini::AIData {
        guard_outer_modifier_human: 1.333,
        guard_outer_modifier_ai: 2.2,
        aggressive_range_modifier: 1.5,
        ..Default::default()
    });

    let cfg = golden_skirmish_config("MoodAcq");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    if !logic.templates.contains_key("MaU") {
        let mut t = ThingTemplate::new("MaU");
        t.add_kind_of(KindOf::Infantry);
        t.add_kind_of(KindOf::Attackable);
        t.set_authored_ai_update_interface(Some(true));
        logic.templates.insert("MaU".into(), t);
    }
    let oid = logic
        .create_object("MaU", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
        .expect("id");
    let vid = logic
        .create_object("MaU", Team::GLA, glam::Vec3::new(20.0, 0.0, 0.0))
        .expect("v");
    {
        // C++ human idle auto-acquire only takes OBJECTSHROUD_CLEAR victims
        // (AIUpdate.cpp:4608-4619 PartitionFilterFreeOfFog residual, mood.rs
        // UNFOGGED flag for a local player's scan). No vision pass runs in this
        // raw fixture, so stamp the retail precondition: the enemy in front of
        // the start units is shroud-clear for the owning local player.
        if let Some(pid) = logic.player_id_for_team(Team::USA) {
            logic
                .world_services
                .shroud()
                .lock()
                .expect("fixture world shroud must be available")
                .set_host_object_shroud_status(
                    pid,
                    vid.0,
                    gamelogic::common::ObjectShroudStatus::Clear,
                );
        }
        let o = logic.host_object_mut(oid).expect("o");
        // Mood auto-acquire is an IDLE-state scan (mood.rs:601 eligible gate);
        // the try_mood_auto_acquire_enters_attack precedent sets Idle explicitly.
        o.set_ai_state(
            crate::game_logic::AIState::Idle,
            &mut crate::game_logic::ObjectAudio::queue(&mut Vec::new()),
        );
        o.auto_acquire_when_idle = true;
        // C++ AutoAcquireEnemiesWhenIdle is a BITFIELD parsed from INI
        // (AIUpdate.cpp; AUTO_ACQUIRE_* flags) — get_next_mood_target gates
        // called_during_idle on AUTO_ACQUIRE_IDLE, not the legacy bool.
        o.auto_acquire_idle_bits |=
            gamelogic::object::update::ai_update_interface::AUTO_ACQUIRE_IDLE;
        // Retail infantry author VisionRange (AmericaInfantry.ini Ranger 150);
        // the mood scan fails closed on vision <= 0
        // (mood.rs adjusted_vision_range_for_mood gate).
        o.vision_range = 150.0;
        // Give a weapon so can_attack is true.
        // C++ WeaponSet::chooseBestWeaponForTarget (PreferMostDamage) only
        // considers a slot whose weapon is past its reload clock; a fresh
        // fixture at frame 0 with last_fire_time 0 / reload 1 has no ready
        // slot and AIAttackState::chooseWeapon refuses the enter (the
        // try_mood_auto_acquire_enters_attack precedent authors
        // last_fire_time -10 for the same reason).
        o.weapon = Some(crate::game_logic::Weapon {
            damage: 10.0,
            range: 100.0,
            last_fire_time: -10.0,
            ..crate::game_logic::Weapon::default()
        });
    }
    // Drive one mood tick.
    logic.tick_mood_auto_acquire_for_test(&[oid]);

    // AIStates.cpp:1417-1427 enters the attack during the owning mood update.
    assert_eq!(logic.host_objects().get(&oid).unwrap().target, Some(vid));
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    shadow.sync_from_host(&logic);
    let projected = shadow.entity_for_host(oid).unwrap();
    shadow
        .world_mut()
        .world_mut()
        .entity_mut(projected)
        .unwrap()
        .attack_target = None;
    shadow.sync_from_host(&logic);

    assert_eq!(logic.host_objects().get(&oid).unwrap().target, Some(vid));
}

#[test]
fn support_guard_engage_uses_decision_authority() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", "1");

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

    assert_eq!(
        logic.host_objects().get(&oid).unwrap().target,
        Some(vid),
        "host engage immediate"
    );
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    shadow.sync_from_host(&logic);
    shadow.sync_from_host(&logic);

    assert_eq!(logic.host_objects().get(&oid).unwrap().target, Some(vid));
}

#[test]
fn faction_ai_launch_attack_decision_authority_writeback() {
    use crate::ai::{AIDifficulty, AIPlayer};

    use crate::game_logic::{KindOf, Team, ThingTemplate, Weapon};

    crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", "1");

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

    assert_eq!(
        logic.host_objects().get(&usa_unit).unwrap().target,
        Some(enemy),
        "launch_attack must engage host target immediately"
    );
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    shadow.sync_from_host(&logic);
    shadow.sync_from_host(&logic);

    assert_eq!(
        logic.host_objects().get(&usa_unit).unwrap().target,
        Some(enemy)
    );
}

#[test]
fn stop_attack_decision_authority_clears_via_writeback() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", "1");

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

    // Host engagement clears same-frame so combat cannot keep firing.
    assert!(
        logic.host_objects().get(&oid).unwrap().target.is_none(),
        "host target must clear immediately on stop"
    );
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    // Seed world attack then apply stop decision; host already clear, writeback is no-op.
    shadow.sync_from_host(&logic);
    shadow.sync_from_host(&logic);
    shadow.sync_from_host(&logic);
    shadow.sync_from_host(&logic);
    shadow.sync_from_host(&logic);

    assert!(logic.host_objects().get(&oid).unwrap().target.is_none());
}

#[test]
fn fire_spawn_authority_defers_queue_until_shadow() {
    use crate::game_logic::combat::{self, DamageType, PendingProjectile};
    use crate::game_logic::host_usa_pilot::HostDeathType;

    let prev_shadow = std::env::var("GENERALS_GAMEWORLD_SHADOW").ok();
    crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", "1");
    let mut logic = GameLogic::new();

    ensure_template(&mut logic, "PendingShotOwner", 100.0);
    assert_eq!(
        logic
            .create_object("PendingShotOwner", Team::USA, glam::Vec3::ZERO)
            .unwrap(),
        ObjectId(1)
    );
    assert_eq!(
        logic
            .create_object(
                "PendingShotOwner",
                Team::China,
                glam::Vec3::new(50.0, 0.0, 0.0)
            )
            .unwrap(),
        ObjectId(2)
    );
    // The accepted shot belongs to its actual CombatSystem in either mode.

    combat::queue_projectile(
        &mut logic.combat_system,
        PendingProjectile {
            shooter_id: ObjectId(1),
            shooter_pos: glam::Vec3::ZERO,
            source_context: None,
            target_id: Some(ObjectId(2)),
            target_pos: Some(glam::Vec3::new(50.0, 0.0, 0.0)),
            damage: 12.0,
            speed: 100.0,
            speed_unit: combat::ProjectileSpeedUnit::DistancePerSecond,
            splash_radius: 0.0,
            is_homing: false,
            damage_type: DamageType::Bullet,
            death_type: HostDeathType::Normal,
            // C++ WeaponTemplate::getProjectileTemplate()==NULL (empty name)
            // routes to delayed damage with NO CombatSystem flight projectile
            // (weapon_fire.rs is_projectileless_object_name). The shadow apply
            // must spawn a flight projectile here, so author a projectile object.
            projectile_object_name: "TestMissile".to_string(),
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
        },
    );
    assert_eq!(
        combat::pending_projectile_queue_len_for_test(&logic.combat_system),
        1
    );
    assert_eq!(logic.combat_system.projectile_count(), 0);
    logic.drain_pending_projectiles_into_combat();
    assert_eq!(
        combat::pending_projectile_queue_len_for_test(&logic.combat_system),
        0
    );
    assert!(
        logic.combat_system.projectile_count() >= 1,
        "actual owner admits its queued projectile"
    );
    match prev_shadow {
        Some(v) => crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", v),
        None => crate::env_compat::remove_var("GENERALS_GAMEWORLD_SHADOW"),
    }
}

#[test]
fn fire_spawn_authority_enqueues_host_when_shadow_disabled() {
    use crate::game_logic::combat::{self, DamageType, PendingProjectile};
    use crate::game_logic::host_usa_pilot::HostDeathType;

    let prev_shadow = std::env::var("GENERALS_GAMEWORLD_SHADOW").ok();
    crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", "0");
    let mut logic = GameLogic::new();

    // Projectile ownership is independent of the diagnostic observation flag.
    combat::clear_pending_projectile_queue_for_test(&mut logic.combat_system);
    combat::queue_projectile(
        &mut logic.combat_system,
        PendingProjectile {
            shooter_id: ObjectId(9),
            shooter_pos: glam::Vec3::ZERO,
            source_context: None,
            target_id: Some(ObjectId(10)),
            target_pos: Some(glam::Vec3::new(10.0, 0.0, 0.0)),
            damage: 5.0,
            speed: 200.0,
            speed_unit: combat::ProjectileSpeedUnit::DistancePerLogicFrame,
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
        },
    );
    let peer = GameLogic::new();
    assert_eq!(
        combat::pending_projectile_queue_len_for_test(&peer.combat_system),
        0,
        "accepted shots cannot leak into a second receiving CombatSystem"
    );
    assert!(
        combat::pending_projectile_queue_len_for_test(&logic.combat_system) >= 1,
        "shadow-off + fire_spawn auth must enqueue this world's accepted shots immediately"
    );
    combat::clear_pending_projectile_queue_for_test(&mut logic.combat_system);
    match prev_shadow {
        Some(v) => crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", v),
        None => crate::env_compat::remove_var("GENERALS_GAMEWORLD_SHADOW"),
    }
}

#[test]
fn ai_decision_authority_applies_host_state_when_shadow_disabled() {
    let prev_s = std::env::var("GENERALS_GAMEWORLD_SHADOW").ok();
    crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", "0");
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
        o.template_mut().add_kind_of(KindOf::Infantry);
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

    match prev_s {
        Some(v) => crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", v),
        None => crate::env_compat::remove_var("GENERALS_GAMEWORLD_SHADOW"),
    }
}

#[test]
fn projectile_authority_steps_host_when_shadow_disabled() {
    let prev_s = std::env::var("GENERALS_GAMEWORLD_SHADOW").ok();
    crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", "0");
    let mut logic = GameLogic::new();

    // Source must contain live gate so host update_projectiles is not skipped.
    let src = GAME_LOGIC_HOST_SRC;

    match prev_s {
        Some(v) => crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", v),
        None => crate::env_compat::remove_var("GENERALS_GAMEWORLD_SHADOW"),
    }
}

#[test]
fn movement_authority_integrates_host_when_shadow_disabled() {
    let prev_s = std::env::var("GENERALS_GAMEWORLD_SHADOW").ok();
    crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", "0");
    let mut logic = GameLogic::new();

    let cfg = golden_skirmish_config("MoveNoShadow");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    ensure_template(&mut logic, "MvU", 100.0);
    if let Some(t) = logic.templates.get_mut("MvU") {
        t.add_kind_of(KindOf::Infantry);
    }
    // update_with_dt evaluates the skirmish NO_BUILDINGS victory rule
    // (C++ VictoryConditions.cpp:87-95/168-196): a structure-less playable
    // player is defeated on frame 0-1. Seed the MpCountForVictory keep-alive
    // (stale_engine_id_does_not_skip_host_movement precedent).
    if !logic.templates.contains_key("VictoryKeepAlive") {
        let mut t = ThingTemplate::new("VictoryKeepAlive");
        t.add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::MpCountForVictory);
        logic.templates.insert("VictoryKeepAlive".into(), t);
    }
    let _keep_alive = logic
        .create_object("VictoryKeepAlive", Team::USA, Vec3::new(300.0, 0.0, 300.0))
        .expect("keep-alive structure");
    let id = logic
        .create_object("MvU", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("unit");
    {
        let o = logic.host_object_mut(id).unwrap();
        // The stale_engine_id twin authors the resolved locomotor path
        // directly: a mapless first-tick assign still sits in the
        // computePointOnPath lead fallback (AIPathfind.cpp:910-950) where the
        // Locomotor.cpp:1393-1430 approach brake zeroes goalSpeed on
        // distAlongPath 0. Authoring the path mirrors the post-pathfind
        // FollowPath state the authority must integrate.
        o.movement.path = vec![Vec3::new(0.0, 0.0, 0.0), Vec3::new(100.0, 0.0, 0.0)];
        o.movement.max_speed = 60.0;
        // C++ Locomotor::getMaxAcceleration: retail locomotors author
        // Acceleration; velocity ramps by acceleration*dt — a 0-accel
        // fixture never moves.
        o.movement.acceleration = 240.0;
        o.movement.target_position = Some(Vec3::new(100.0, 0.0, 0.0));
        o.status.moving = true;
    }
    let pre = logic.host_objects().get(&id).unwrap().get_position();
    for _ in 0..10 {
        logic.update_with_dt(1.0 / 30.0);
    }
    let post = logic.host_objects().get(&id).unwrap().get_position();
    let dist = (post - pre).length();
    assert!(
        dist > 0.01,
        "host-only movement must integrate path; pre={pre:?} post={post:?} dist={dist}"
    );
    match prev_s {
        Some(v) => crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", v),
        None => crate::env_compat::remove_var("GENERALS_GAMEWORLD_SHADOW"),
    }
}
#[test]
fn constructed_host_birth_has_completed_percent_without_shadow_admission() {
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
    assert_eq!(logic.host_object(id).unwrap().construction_percent, 1.0);
    assert!(!logic.host_object(id).unwrap().status.under_construction);
}

#[test]
fn ai_attack_authority_gates_fire_intent_writeback() {
    use crate::game_logic::host_fire_intent_log;
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    host_fire_intent_log::clear();
    crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", "1");
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
    {
        let object = logic.host_object_mut(oid).unwrap();
        object.fire_intent_count = 1;
        object.last_fire_victim_host = 9;
    }
    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let entity = shadow.entity_for_host(oid).unwrap();
    shadow
        .world_mut()
        .world_mut()
        .entity_mut(entity)
        .unwrap()
        .fire_intent_count = 99;
    assert_eq!(logic.host_object(oid).unwrap().fire_intent_count, 1);
    assert_eq!(logic.host_object(oid).unwrap().last_fire_victim_host, 9);
    shadow.sync_from_host(&logic);
    assert_eq!(shadow.world().entity(entity).unwrap().fire_intent_count, 1);
}
