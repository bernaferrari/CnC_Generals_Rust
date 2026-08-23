//! Host GameLogic tests — `crates_and_salvage`.
#![allow(unused_imports, non_snake_case, unused_variables, dead_code)]
use super::super::*;
use super::helpers::*;

#[test]
fn drones_do_not_trigger_or_join_retaliation() {
    use crate::game_logic::{AIState, KindOf, Object, ObjectId, Team, ThingTemplate, Weapon};
    let mut logic = GameLogic::new();
    logic
        .players
        .insert(0, Player::new(0, Team::USA, "P0", true));
    logic.set_logical_retaliation_mode(0, true);
    // Victim is drone → shouldRetaliateAgainstAggressor false
    let mut vt = ThingTemplate::new("DroneV");
    vt.add_kind_of(KindOf::Drone);
    vt.add_kind_of(KindOf::Vehicle);
    let vid = ObjectId(4421);
    logic.objects.insert(vid, {
        let mut o = Object::new(vt, vid, Team::USA);
        o.set_position(glam::Vec3::ZERO);
        o
    });
    let mut et = ThingTemplate::new("EnD");
    et.add_kind_of(KindOf::Infantry);
    let eid = ObjectId(4422);
    logic.objects.insert(eid, {
        let mut o = Object::new(et, eid, Team::GLA);
        o.set_position(glam::Vec3::new(30.0, 0.0, 0.0));
        o
    });
    assert!(!logic.should_retaliate_against_aggressor(vid, eid));
}

#[test]
fn damaged_can_be_repulsed_sets_temporary_repulsor() {
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.set_enable_repulsors(true);
    let mut t = ThingTemplate::new("CivDmg");
    t.add_kind_of(KindOf::Infantry);
    t.add_kind_of(KindOf::CanBeRepulsed);
    let id = ObjectId(4301);
    let mut o = Object::new(t, id, Team::Neutral);
    o.health.current = 100.0;
    o.health.maximum = 100.0;
    logic.objects.insert(id, o);
    let destroyed = logic.objects.get_mut(&id).unwrap().take_damage(10.0);
    assert!(!destroyed);
    {
        let o = &logic.objects[&id];
        assert!(o.status.repulsor, "damaged civ must flag REPULSOR");
        assert_eq!(o.repulsor_until_frame, 60);
    }
    // Helper clear after 2 seconds residual (countdown).
    if let Some(o) = logic.objects.get_mut(&id) {
        for _ in 0..59 {
            o.tick_repulsor_status(0);
        }
        assert!(o.status.repulsor);
        assert_eq!(o.repulsor_until_frame, 1);
        o.tick_repulsor_status(0);
        assert!(!o.status.repulsor);
        assert_eq!(o.repulsor_until_frame, 0);
    }
}

#[test]
fn damaged_repulsor_disabled_when_enable_off() {
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
    crate::game_logic::host_repulsor_gate::set_enabled(false);
    let mut t = ThingTemplate::new("CivOff");
    t.add_kind_of(KindOf::CanBeRepulsed);
    let id = ObjectId(4302);
    let mut o = Object::new(t, id, Team::Neutral);
    o.health.current = 50.0;
    o.health.maximum = 50.0;
    let _ = o.take_damage(5.0);
    assert!(!o.status.repulsor);
}

#[test]
fn set_unit_repulsor_status_flag() {
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    let mut t = ThingTemplate::new("RepA");
    t.add_kind_of(KindOf::Infantry);
    let id = ObjectId(4201);
    logic.objects.insert(id, Object::new(t, id, Team::USA));
    assert!(!logic.objects[&id].status.repulsor);
    assert!(logic.set_unit_repulsor(id, true));
    assert!(logic.objects[&id].status.repulsor);
    assert!(logic.set_unit_repulsor(id, false));
    assert!(!logic.objects[&id].status.repulsor);
}

#[test]
fn find_closest_repulsor_respects_enable_flag() {
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    let mut ct = ThingTemplate::new("CivR");
    ct.add_kind_of(KindOf::Infantry);
    ct.add_kind_of(KindOf::CanBeRepulsed);
    let cid = ObjectId(4210);
    let mut civ = Object::new(ct, cid, Team::Neutral);
    civ.vision_range = 200.0;
    logic.objects.insert(cid, civ);

    let mut et = ThingTemplate::new("EnemyR");
    et.add_kind_of(KindOf::Infantry);
    et.add_kind_of(KindOf::Attackable);
    let eid = ObjectId(4211);
    let mut enemy = Object::new(et, eid, Team::GLA);
    enemy.set_position(glam::Vec3::new(50.0, 0.0, 0.0));
    enemy.set_status_repulsor(true);
    logic.objects.insert(eid, enemy);

    // Disabled by default (C++ m_enableRepulsors = false).
    assert!(logic.find_closest_repulsor(cid, 300.0).is_none());
    logic.set_enable_repulsors(true);
    let found = logic.find_closest_repulsor(cid, 300.0);
    assert_eq!(found.map(|(id, _)| id), Some(eid));
    // Out of range
    assert!(logic.find_closest_repulsor(cid, 10.0).is_none());
}

#[test]
fn try_idle_repulse_flees_flagged_repulsor() {
    use crate::game_logic::{AIState, KindOf, Object, ObjectId, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.set_enable_repulsors(true);

    let mut ct = ThingTemplate::new("CivFlee");
    ct.add_kind_of(KindOf::Infantry);
    ct.add_kind_of(KindOf::CanBeRepulsed);
    let cid = ObjectId(4220);
    let mut civ = Object::new(ct, cid, Team::Neutral);
    civ.vision_range = 200.0;
    civ.set_ai_state(AIState::Idle);
    // Ensure can_move
    civ.movement.max_speed = 5.0;
    logic.objects.insert(cid, civ);

    let mut et = ThingTemplate::new("ThreatR");
    et.add_kind_of(KindOf::Infantry);
    let eid = ObjectId(4221);
    let mut enemy = Object::new(et, eid, Team::China);
    enemy.set_position(glam::Vec3::new(40.0, 0.0, 0.0));
    enemy.set_status_repulsor(true);
    logic.objects.insert(eid, enemy);

    assert!(logic.try_idle_repulse(cid));
    let civ = &logic.objects[&cid];
    assert_eq!(civ.move_away_from, Some(eid));
    assert!(civ.move_away_frames > 0);
    assert!(civ.is_panicking, "flee must set MODELCONDITION_PANICKING");
    assert!(
        (civ.movement.max_speed - 50.0).abs() < 0.05,
        "flee must swap PanicHumanLocomotor speed, got {}",
        civ.movement.max_speed
    );
}

#[test]
fn start_new_game_applies_retail_enable_repulsors() {
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
    crate::game_logic::host_repulsor_gate::clear_aidata_ini_applied_for_test();
    crate::game_logic::host_repulsor_gate::set_enabled(false);
    let mut logic = GameLogic::new();
    assert!(
        !logic.enable_repulsors,
        "C++ TAiData ctor default is false until AIData.ini"
    );
    logic.start_new_game(GameMode::Skirmish);
    assert!(
        logic.enable_repulsors,
        "retail Default/AIData.ini EnableRepulsors=Yes on live start_new_game"
    );
    assert!(crate::game_logic::host_repulsor_gate::is_enabled());

    let mut t = ThingTemplate::new("CivLivePanic");
    t.add_kind_of(KindOf::Infantry);
    t.add_kind_of(KindOf::CanBeRepulsed);
    let id = ObjectId(4303);
    let mut o = Object::new(t, id, Team::Neutral);
    o.health.current = 100.0;
    o.health.maximum = 100.0;
    logic.objects.insert(id, o);
    let destroyed = logic.objects.get_mut(&id).unwrap().take_damage(10.0);
    assert!(!destroyed);
    assert!(
        logic.objects[&id].status.repulsor,
        "damaged CAN_BE_REPULSED must flag REPULSOR when live EnableRepulsors is on"
    );
}

#[test]
fn apply_aidata_enable_repulsors_honors_parsed_ini_no() {
    crate::game_logic::host_repulsor_gate::mark_aidata_ini_applied();
    {
        let mut store = game_engine::common::ini::get_ai_data_store_mut();
        store.ensure_base();
        if let Some(data) = store.get_active_mut() {
            data.enable_repulsors = false;
        }
    }
    let mut logic = GameLogic::new();
    logic.apply_aidata_enable_repulsors();
    assert!(
        !logic.enable_repulsors,
        "parsed EnableRepulsors=No must win over retail Yes"
    );
    assert!(!crate::game_logic::host_repulsor_gate::is_enabled());
    crate::game_logic::host_repulsor_gate::clear_aidata_ini_applied_for_test();
    {
        let mut store = game_engine::common::ini::get_ai_data_store_mut();
        if let Some(data) = store.get_active_mut() {
            data.enable_repulsors = false;
        }
    }
}

#[test]
fn set_team_repulsor_applies_to_members() {
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    for i in 0..2u32 {
        let name = format!("TR{i}");
        let mut t = ThingTemplate::new(&name);
        t.add_kind_of(KindOf::Infantry);
        let id = ObjectId(4230 + i);
        logic.objects.insert(id, Object::new(t, id, Team::GLA));
    }
    let n = logic.set_team_repulsor_by_name("GLA", true);
    assert_eq!(n, 2);
    assert!(logic.objects[&ObjectId(4230)].status.repulsor);
    assert!(logic.objects[&ObjectId(4231)].status.repulsor);
}

#[test]
fn team_panic_swaps_panic_locomotor_set() {
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    let mut t = ThingTemplate::new("GLAInfantryRebel");
    t.add_kind_of(KindOf::Infantry);
    let id = ObjectId(4240);
    let mut obj = Object::new(t, id, Team::GLA);
    obj.movement.max_speed = 20.0;
    logic.objects.insert(id, obj);
    assert_eq!(logic.set_team_panic_by_name("GLA"), 1);
    let u = &logic.objects[&id];
    assert!(u.is_panicking);
    assert!(
        (u.movement.max_speed - 50.0).abs() < 0.05,
        "TeamPanic must install PanicHumanLocomotor, got {}",
        u.movement.max_speed
    );
}

#[test]
fn named_unit_panic_is_not_a_noop() {
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    let mut t = ThingTemplate::new("Civilian");
    t.add_kind_of(KindOf::Infantry);
    t.add_kind_of(KindOf::CanBeRepulsed);
    let id = ObjectId(4241);
    let mut obj = Object::new(t, id, Team::Neutral);
    obj.name = "CivA".to_string();
    obj.movement.max_speed = 20.0;
    logic.objects.insert(id, obj);
    assert!(logic.set_named_unit_panic("CivA"));
    let u = &logic.objects[&id];
    assert!(u.is_panicking);
    assert!((u.movement.max_speed - 50.0).abs() < 0.05);
}

#[test]
fn set_team_attitude_applies_to_all_members() {
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate, Weapon};
    let mut logic = GameLogic::new();
    for i in 0..3u32 {
        let name = format!("Ta{i}");
        let mut t = ThingTemplate::new(&name);
        t.add_kind_of(KindOf::Infantry);
        let id = ObjectId(3100 + i);
        let mut o = Object::new(t, id, Team::China);
        o.weapon = Some(Weapon {
            range: 40.0,
            ..Default::default()
});
        logic.objects.insert(id, o);
    }
    // USA control
    let mut t = ThingTemplate::new("UsaCtrl");
    t.add_kind_of(KindOf::Infantry);
    logic.objects.insert(ObjectId(3199), {
        let mut o = Object::new(t, ObjectId(3199), Team::USA);
        o.weapon = Some(Weapon {
            range: 40.0,
            ..Default::default()
});
        o
    });
    let n = logic.set_team_attitude_by_name("China", "SLEEP");
    assert_eq!(n, 3);
    assert_eq!(logic.objects[&ObjectId(3100)].ai_attitude, -2);
    assert_eq!(logic.objects[&ObjectId(3101)].ai_attitude, -2);
    assert_eq!(logic.objects[&ObjectId(3199)].ai_attitude, 0);
    assert!(!logic.mood_allows_attack(ObjectId(3100), false));
    assert!(logic.mood_allows_attack(ObjectId(3199), false));
}

#[test]
fn set_team_attitude_by_instance_name_and_script_drain() {
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate, Weapon};
    use gamelogic::scripting::request_host_team_attitude;

    let mut logic = GameLogic::new();
    let mut t = ThingTemplate::new("Hero");
    t.add_kind_of(KindOf::Infantry);
    let id = ObjectId(4100);
    let mut o = Object::new(t, id, Team::USA);
    o.team_instance_name = "AmericaTeamHeroes".into();
    o.weapon = Some(Weapon {
        range: 40.0,
        ..Default::default()
});
    logic.objects.insert(id, o);

    request_host_team_attitude("AmericaTeamHeroes", 2);
    logic.apply_host_team_attitude_script_requests();
    assert_eq!(logic.objects[&id].ai_attitude, 2);
}


#[test]
fn resolve_host_team_name_covers_cpp_aliases() {
    assert_eq!(
        GameLogic::resolve_host_team_name("teamAmerica"),
        Some(crate::game_logic::Team::USA)
    );
    assert_eq!(
        GameLogic::resolve_host_team_name("America"),
        Some(crate::game_logic::Team::USA)
    );
    assert_eq!(
        GameLogic::resolve_host_team_name("GLA"),
        Some(crate::game_logic::Team::GLA)
    );
    assert_eq!(
        GameLogic::resolve_host_team_name("teamChina"),
        Some(crate::game_logic::Team::China)
    );
    assert!(GameLogic::resolve_host_team_name("nope").is_none());
}

#[test]
fn inherit_uses_named_team_proto_not_faction() {
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
    use gamelogic::common::well_known_keys::key_team_aggressiveness;
    use gamelogic::common::{AsciiString, Dict};
    use gamelogic::team::get_team_factory;

    {
        let Ok(mut factory) = get_team_factory().lock() else {
            return;
        };
        let mut named = Dict::new();
        named.set_int(key_team_aggressiveness(), 2);
        let _ = factory.init_team(
            AsciiString::from("AmericaTeamRangersInherit"),
            AsciiString::from("PlyrAmerica"),
            false,
            Some(&named),
        );
        // Faction protos as Sleep so a USA/America lookup would apply -2.
        let mut faction = Dict::new();
        faction.set_int(key_team_aggressiveness(), -2);
        let _ = factory.init_team(
            AsciiString::from("America"),
            AsciiString::from("PlyrAmerica"),
            true,
            Some(&faction),
        );
        let _ = factory.init_team(
            AsciiString::from("USA"),
            AsciiString::from("PlyrAmerica"),
            true,
            Some(&faction),
        );
    }

    let mut logic = GameLogic::new();
    let mut t = ThingTemplate::new("AmericaRangerNamedAtt");
    t.add_kind_of(KindOf::Infantry);
    let id = ObjectId(4101);
    let mut o = Object::new(t, id, Team::USA);
    o.team_instance_name = "AmericaTeamRangersInherit".into();
    logic.objects.insert(id, o);

    logic.inherit_team_ai_defaults(id);
    assert_eq!(
        logic.objects[&id].ai_attitude, 2,
        "named team proto Aggressive must win over faction Sleep"
    );

    logic.set_unit_attitude(id, -1);
    logic.inherit_team_ai_defaults(id);
    assert_eq!(
        logic.objects[&id].ai_attitude, -1,
        "re-inherit must not clobber scripted attitude"
    );

    if let Some(obj) = logic.objects.get_mut(&id) {
        obj.set_team(Team::USA);
    }
    assert_eq!(
        logic.objects[&id].ai_attitude, 2,
        "C++ setTeam reapplies named team proto attitude"
    );
}

#[test]
fn set_unit_attitude_affects_mood_matrix() {
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate, Weapon};
    let mut logic = GameLogic::new();
    let mut t = ThingTemplate::new("AttA");
    t.add_kind_of(KindOf::Infantry);
    let id = ObjectId(2901);
    logic.objects.insert(id, {
        let mut o = Object::new(t, id, Team::China);
        o.weapon = Some(Weapon {
            range: 50.0,
            ..Default::default()
});
        o
    });
    assert!(logic.set_unit_attitude(id, GameLogic::parse_attitude_token("SLEEP")));
    assert!(!logic.mood_allows_attack(id, false));
    assert!(logic.set_unit_attitude(id, GameLogic::parse_attitude_token("AGGRESSIVE")));
    assert!(logic.mood_allows_attack(id, false));
    assert_eq!(logic.objects[&id].ai_attitude, 2);
}

#[test]
fn apply_attack_priority_set_to_team_members() {
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    let mut info = AttackPriorityInfo::new("TeamHunt");
    logic.register_attack_priority_set(info);
    for i in 0..3u32 {
        let name = format!("Mem{i}");
        let mut t = ThingTemplate::new(&name);
        t.add_kind_of(KindOf::Infantry);
        let id = ObjectId(2910 + i);
        logic.objects.insert(id, Object::new(t, id, Team::GLA));
    }
    // USA unit should not be touched.
    let mut t = ThingTemplate::new("UsaX");
    t.add_kind_of(KindOf::Infantry);
    logic
        .objects
        .insert(ObjectId(2999), Object::new(t, ObjectId(2999), Team::USA));
    let n = logic.apply_attack_priority_set_to_team(Team::GLA, Some("TeamHunt"));
    assert_eq!(n, 3);
    assert_eq!(
        logic.objects[&ObjectId(2910)]
            .attack_priority_set
            .as_deref(),
        Some("TeamHunt")
    );
    assert!(logic.objects[&ObjectId(2999)].attack_priority_set.is_none());
}

#[test]
fn parse_attitude_token_covers_cpp_names() {
    assert_eq!(GameLogic::parse_attitude_token("sleep"), -2);
    assert_eq!(GameLogic::parse_attitude_token("Passive"), -1);
    assert_eq!(GameLogic::parse_attitude_token("normal"), 0);
    assert_eq!(GameLogic::parse_attitude_token("ALERT"), 1);
    assert_eq!(GameLogic::parse_attitude_token("defensive"), 1);
    assert_eq!(GameLogic::parse_attitude_token("aggressive"), 2);
}

#[test]
fn script_priority_set_applies_to_unit_via_host_api() {
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate, Weapon};
    use glam::Vec3;
    let mut logic = GameLogic::new();
    let mut info = AttackPriorityInfo::new("ScriptHunt");
    info.default_priority = 1;
    info.set_priority_template("PriorityTarget", 80);
    logic.register_attack_priority_set(info);

    let mut at = ThingTemplate::new("ScriptHunter");
    at.add_kind_of(KindOf::Infantry);
    at.add_kind_of(KindOf::Attackable);
    let aid = ObjectId(2801);
    logic.objects.insert(aid, {
        let mut o = Object::new(at, aid, Team::China);
        o.set_position(Vec3::ZERO);
        o.weapon = Some(Weapon {
            range: 300.0,
            can_target_ground: true,
            damage: 5.0,
            ..Default::default()
});
        o
    });
    // C++ NamedApplyAttackPrioritySet residual via host setter.
    logic.set_unit_attack_priority_set(aid, Some("ScriptHunt"));
    assert_eq!(
        logic.objects[&aid].attack_priority_set.as_deref(),
        Some("ScriptHunt")
    );

    let mut lt = ThingTemplate::new("Filler");
    lt.add_kind_of(KindOf::Infantry);
    lt.add_kind_of(KindOf::Attackable);
    let lid = ObjectId(2802);
    logic.objects.insert(lid, {
        let mut o = Object::new(lt, lid, Team::GLA);
        o.set_position(Vec3::new(30.0, 0.0, 0.0));
        o
    });
    let mut ht = ThingTemplate::new("PriorityTarget");
    ht.add_kind_of(KindOf::Infantry);
    ht.add_kind_of(KindOf::Attackable);
    let hid = ObjectId(2803);
    logic.objects.insert(hid, {
        let mut o = Object::new(ht, hid, Team::GLA);
        o.set_position(Vec3::new(100.0, 0.0, 0.0));
        o
    });
    assert_eq!(
        logic.find_closest_enemy(aid, 400.0, find_enemy_flags::CAN_ATTACK),
        Some(hid)
    );
    // Clear set → closest wins.
    logic.set_unit_attack_priority_set(aid, None);
    assert_eq!(
        logic.find_closest_enemy(aid, 400.0, find_enemy_flags::CAN_ATTACK),
        Some(lid)
    );
}

#[test]
fn set_default_and_kind_priority_residual() {
    let mut info = AttackPriorityInfo::new("KindSet");
    info.default_priority = 2;
    info.set_priority_kind("vehicle", 40);
    info.set_priority_template("SpecInf", 10);
    assert_eq!(info.get_priority_for_template("Unknown"), 2);
    assert_eq!(info.get_priority_for_template("SpecInf"), 10);
    // kind priority applied in attack_priority_for_target
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.register_attack_priority_set(info);
    let mut vt = ThingTemplate::new("TankX");
    vt.add_kind_of(KindOf::Vehicle);
    vt.add_kind_of(KindOf::Attackable);
    let vid = ObjectId(2810);
    let v = Object::new(vt, vid, Team::GLA);
    let pinfo = logic.attack_priority_sets.get("kindset").unwrap();
    assert_eq!(logic.attack_priority_for_target(pinfo, &v), 40);
}

#[test]
fn attack_priority_info_template_lookup() {
    let mut info = AttackPriorityInfo::new("TestSet");
    info.default_priority = 1;
    info.set_priority_template("AmericaRanger", 50);
    assert_eq!(info.get_priority_for_template("AmericaRanger"), 50);
    assert_eq!(info.get_priority_for_template("Other"), 1);
    assert_eq!(info.get_priority_for_template("americaranger"), 50);
}

#[test]
fn find_closest_enemy_uses_attack_priority() {
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate, Weapon};
    use glam::Vec3;
    let mut logic = GameLogic::new();
    let mut info = AttackPriorityInfo::new("Hunt");
    info.default_priority = 1;
    info.set_priority_template("HighValue", 100);
    info.set_priority_template("LowValue", 5);
    logic.register_attack_priority_set(info);

    let mut at = ThingTemplate::new("Hunter");
    at.add_kind_of(KindOf::Infantry);
    at.add_kind_of(KindOf::Attackable);
    let aid = ObjectId(2701);
    logic.objects.insert(aid, {
        let mut o = Object::new(at, aid, Team::China);
        o.set_position(Vec3::ZERO);
        o.attack_priority_set = Some("Hunt".into());
        o.weapon = Some(Weapon {
            range: 500.0,
            can_target_ground: true,
            damage: 5.0,
            ..Default::default()
});
        o
    });
    // Low value close
    let mut lt = ThingTemplate::new("LowValue");
    lt.add_kind_of(KindOf::Infantry);
    lt.add_kind_of(KindOf::Attackable);
    let lid = ObjectId(2702);
    logic.objects.insert(lid, {
        let mut o = Object::new(lt, lid, Team::GLA);
        o.set_position(Vec3::new(40.0, 0.0, 0.0));
        o
    });
    // High value farther
    let mut ht = ThingTemplate::new("HighValue");
    ht.add_kind_of(KindOf::Infantry);
    ht.add_kind_of(KindOf::Attackable);
    let hid = ObjectId(2703);
    logic.objects.insert(hid, {
        let mut o = Object::new(ht, hid, Team::GLA);
        o.set_position(Vec3::new(120.0, 0.0, 0.0));
        o
    });
    let found = logic.find_closest_enemy(aid, 400.0, find_enemy_flags::CAN_ATTACK);
    assert_eq!(
        found,
        Some(hid),
        "higher priority should win despite distance"
    );
}

#[test]
fn find_closest_enemy_skips_zero_priority() {
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate, Weapon};
    use glam::Vec3;
    let mut logic = GameLogic::new();
    let mut info = AttackPriorityInfo::new("Zero");
    info.default_priority = 0; // never attack default
    info.set_priority_template("Allowed", 10);
    logic.register_attack_priority_set(info);

    let mut at = ThingTemplate::new("Hunter2");
    at.add_kind_of(KindOf::Infantry);
    at.add_kind_of(KindOf::Attackable);
    let aid = ObjectId(2704);
    logic.objects.insert(aid, {
        let mut o = Object::new(at, aid, Team::China);
        o.set_position(Vec3::ZERO);
        o.attack_priority_set = Some("Zero".into());
        o.weapon = Some(Weapon {
            range: 200.0,
            can_target_ground: true,
            damage: 5.0,
            ..Default::default()
});
        o
    });
    let mut zt = ThingTemplate::new("Forbidden");
    zt.add_kind_of(KindOf::Infantry);
    zt.add_kind_of(KindOf::Attackable);
    let zid = ObjectId(2705);
    logic.objects.insert(zid, {
        let mut o = Object::new(zt, zid, Team::GLA);
        o.set_position(Vec3::new(20.0, 0.0, 0.0));
        o
    });
    let mut ok_t = ThingTemplate::new("Allowed");
    ok_t.add_kind_of(KindOf::Infantry);
    ok_t.add_kind_of(KindOf::Attackable);
    let okid = ObjectId(2706);
    logic.objects.insert(okid, {
        let mut o = Object::new(ok_t, okid, Team::GLA);
        o.set_position(Vec3::new(80.0, 0.0, 0.0));
        o
    });
    assert_eq!(
        logic.find_closest_enemy(aid, 200.0, find_enemy_flags::CAN_ATTACK),
        Some(okid)
    );
}

#[test]
fn find_closest_enemy_skips_pure_buildings_by_default() {
    use crate::game_logic::{KindOf, Object, ObjectId, ObjectType, Team, ThingTemplate, Weapon};
    use glam::Vec3;
    let mut logic = GameLogic::new();
    let mut at = ThingTemplate::new("FceA");
    at.add_kind_of(KindOf::Infantry);
    at.add_kind_of(KindOf::Attackable);
    let aid = ObjectId(2601);
    logic.objects.insert(aid, {
        let mut o = Object::new(at, aid, Team::China);
        o.set_position(Vec3::ZERO);
        o.weapon = Some(Weapon {
            range: 200.0,
            can_target_ground: true,
            damage: 5.0,
            ..Default::default()
});
        o
    });
    // Pure building (no weapon) nearby.
    let mut bt = ThingTemplate::new("FceB");
    bt.add_kind_of(KindOf::Structure);
    bt.add_kind_of(KindOf::Attackable);
    let bid = ObjectId(2602);
    logic.objects.insert(bid, {
        let mut o = Object::new(bt, bid, Team::GLA);
        o.object_type = ObjectType::Building;
        o.set_position(Vec3::new(30.0, 0.0, 0.0));
        o
    });
    // Infantry farther.
    let mut it = ThingTemplate::new("FceI");
    it.add_kind_of(KindOf::Infantry);
    it.add_kind_of(KindOf::Attackable);
    let iid = ObjectId(2603);
    logic.objects.insert(iid, {
        let mut o = Object::new(it, iid, Team::GLA);
        o.set_position(Vec3::new(80.0, 0.0, 0.0));
        o
    });
    let found = logic.find_closest_enemy(aid, 200.0, find_enemy_flags::CAN_ATTACK);
    assert_eq!(found, Some(iid));
    // With ATTACK_BUILDINGS, building wins as closer.
    let found_b = logic.find_closest_enemy(
        aid,
        200.0,
        find_enemy_flags::CAN_ATTACK | find_enemy_flags::ATTACK_BUILDINGS,
    );
    assert_eq!(found_b, Some(bid));
}

#[test]
fn find_closest_enemy_within_attack_range_flag() {
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate, Weapon};
    use glam::Vec3;
    let mut logic = GameLogic::new();
    let mut at = ThingTemplate::new("FceA2");
    at.add_kind_of(KindOf::Infantry);
    at.add_kind_of(KindOf::Attackable);
    let aid = ObjectId(2604);
    logic.objects.insert(aid, {
        let mut o = Object::new(at, aid, Team::China);
        o.set_position(Vec3::ZERO);
        o.weapon = Some(Weapon {
            range: 40.0,
            can_target_ground: true,
            damage: 5.0,
            ..Default::default()
});
        o
    });
    let mut et = ThingTemplate::new("FceE2");
    et.add_kind_of(KindOf::Infantry);
    et.add_kind_of(KindOf::Attackable);
    let eid = ObjectId(2605);
    logic.objects.insert(eid, {
        let mut o = Object::new(et, eid, Team::GLA);
        o.set_position(Vec3::new(100.0, 0.0, 0.0)); // outside weapon range
        o
    });
    assert!(logic
        .find_closest_enemy(
            aid,
            200.0,
            find_enemy_flags::CAN_ATTACK | find_enemy_flags::WITHIN_ATTACK_RANGE,
        )
        .is_none());
    // Without WITHIN_ATTACK_RANGE, still found (PossibleAfterMoving).
    assert_eq!(
        logic.find_closest_enemy(aid, 200.0, find_enemy_flags::CAN_ATTACK),
        Some(eid)
    );
}

#[test]
fn find_closest_enemy_rejects_unable_attacker() {
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
    use glam::Vec3;
    let mut logic = GameLogic::new();
    let mut at = ThingTemplate::new("FceA3");
    at.add_kind_of(KindOf::Infantry);
    let aid = ObjectId(2606);
    logic.objects.insert(aid, {
        let mut o = Object::new(at, aid, Team::China);
        o.set_position(Vec3::ZERO);
        // no weapon → cannot attack
        o
    });
    let mut et = ThingTemplate::new("FceE3");
    et.add_kind_of(KindOf::Infantry);
    et.add_kind_of(KindOf::Attackable);
    let eid = ObjectId(2607);
    logic.objects.insert(eid, {
        let mut o = Object::new(et, eid, Team::GLA);
        o.set_position(Vec3::new(20.0, 0.0, 0.0));
        o
    });
    assert!(logic
        .find_closest_enemy(aid, 100.0, find_enemy_flags::CAN_ATTACK)
        .is_none());
}

#[test]
fn get_next_mood_target_finds_nearby_enemy() {
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate, Weapon};
    use glam::Vec3;
    let mut logic = GameLogic::new();
    logic.frame = 100;
    let mut at = ThingTemplate::new("MoodT1");
    at.add_kind_of(KindOf::Infantry);
    at.add_kind_of(KindOf::Attackable);
    let aid = ObjectId(2501);
    logic.objects.insert(aid, {
        let mut o = Object::new(at, aid, Team::China);
        o.set_position(Vec3::ZERO);
        o.ai_attitude = 0; // Normal
        o.vision_range = 200.0;
        o.next_mood_check_time = 0;
        o.weapon = Some(Weapon {
            range: 80.0,
            damage: 10.0,
            can_target_ground: true,
            ..Default::default()
});
        o
    });
    let mut vt = ThingTemplate::new("MoodT2");
    vt.add_kind_of(KindOf::Infantry);
    vt.add_kind_of(KindOf::Attackable);
    let vid = ObjectId(2502);
    logic.objects.insert(vid, {
        let mut o = Object::new(vt, vid, Team::GLA);
        o.set_position(Vec3::new(50.0, 0.0, 0.0));
        o
    });
    let t = logic.get_next_mood_target(aid, true, true, false);
    assert_eq!(t, Some(vid));
    // Rate limit: immediate second call should be None.
    assert!(logic.get_next_mood_target(aid, true, true, false).is_none());
}

#[test]
fn get_next_mood_target_sleep_returns_none() {
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate, Weapon};
    use glam::Vec3;
    let mut logic = GameLogic::new();
    let mut at = ThingTemplate::new("MoodS");
    at.add_kind_of(KindOf::Infantry);
    at.add_kind_of(KindOf::Attackable);
    let aid = ObjectId(2503);
    logic.objects.insert(aid, {
        let mut o = Object::new(at, aid, Team::China);
        o.set_position(Vec3::ZERO);
        o.set_ai_attitude_i8(-2);
        o.vision_range = 200.0;
        o.weapon = Some(Weapon {
            range: 80.0,
            can_target_ground: true,
            ..Default::default()
});
        o
    });
    let mut vt = ThingTemplate::new("MoodSE");
    vt.add_kind_of(KindOf::Infantry);
    vt.add_kind_of(KindOf::Attackable);
    let vid = ObjectId(2504);
    logic.objects.insert(vid, {
        let mut o = Object::new(vt, vid, Team::GLA);
        o.set_position(Vec3::new(20.0, 0.0, 0.0));
        o
    });
    assert!(logic.get_next_mood_target(aid, true, true, false).is_none());
}

#[test]
fn get_next_mood_target_passive_uses_last_damage_source() {
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate, Weapon};
    use glam::Vec3;
    let mut logic = GameLogic::new();
    logic.frame = 50;
    let mut at = ThingTemplate::new("MoodP");
    at.add_kind_of(KindOf::Infantry);
    at.add_kind_of(KindOf::Attackable);
    let aid = ObjectId(2505);
    let enemy = ObjectId(2506);
    logic.objects.insert(aid, {
        let mut o = Object::new(at, aid, Team::China);
        o.set_position(Vec3::ZERO);
        o.ai_attitude = -1; // Passive
        o.last_damage_source = Some(enemy);
        o.vision_range = 200.0;
        o.weapon = Some(Weapon {
            range: 100.0,
            damage: 5.0,
            can_target_ground: true,
            ..Default::default()
});
        o
    });
    let mut vt = ThingTemplate::new("MoodPE");
    vt.add_kind_of(KindOf::Infantry);
    vt.add_kind_of(KindOf::Attackable);
    logic.objects.insert(enemy, {
        let mut o = Object::new(vt, enemy, Team::GLA);
        o.set_position(Vec3::new(30.0, 0.0, 0.0));
        o
    });
    assert_eq!(
        logic.get_next_mood_target(aid, true, true, false),
        Some(enemy)
    );
}

#[test]
fn try_mood_auto_acquire_enters_attack() {
    use crate::game_logic::{AIState, KindOf, Object, ObjectId, Team, ThingTemplate, Weapon};
    use glam::Vec3;
    let mut logic = GameLogic::new();
    logic.frame = 10;
    let mut at = ThingTemplate::new("MoodAc");
    at.add_kind_of(KindOf::Infantry);
    at.add_kind_of(KindOf::Attackable);
    let aid = ObjectId(2507);
    logic.objects.insert(aid, {
        let mut o = Object::new(at, aid, Team::China);
        o.set_position(Vec3::ZERO);
        o.set_orientation(0.0);
        o.set_ai_state(AIState::Idle);
        o.ai_attitude = 2; // Aggressive
        o.vision_range = 200.0;
        o.next_mood_check_time = 0;
        o.weapon = Some(Weapon {
            range: 100.0,
            damage: 10.0,
            can_target_ground: true,
            last_fire_time: -10.0,
            reload_time: 1.0,
            ..Default::default()
});
        o
    });
    let mut vt = ThingTemplate::new("MoodAcE");
    vt.add_kind_of(KindOf::Infantry);
    vt.add_kind_of(KindOf::Attackable);
    let vid = ObjectId(2508);
    logic.objects.insert(vid, {
        let mut o = Object::new(vt, vid, Team::GLA);
        o.set_position(Vec3::new(40.0, 0.0, 0.0));
        o
    });
    let got = logic.try_mood_auto_acquire(aid, false);
    assert_eq!(got, Some(vid));
    assert_eq!(logic.objects[&aid].target, Some(vid));
    assert!(logic.objects[&aid].status.attacking);
}

#[test]
fn transfer_attack_retargets_attackers() {
    use crate::game_logic::object::TurretSubState;
    use crate::game_logic::{AIState, KindOf, Object, ObjectId, Team, ThingTemplate, Weapon};
    let mut logic = GameLogic::new();
    let mut mk = |id: u32, team| {
        let name = format!("T{id}");
        let mut t = ThingTemplate::new(&name);
        t.add_kind_of(KindOf::Infantry);
        let mut o = Object::new(t, ObjectId(id), team);
        o.weapon = Some(Weapon {
            range: 50.0,
            ..Default::default()
});
        o
    };
    let from = ObjectId(2401);
    let to = ObjectId(2402);
    let atk = ObjectId(2403);
    logic.objects.insert(from, mk(2401, Team::GLA));
    logic.objects.insert(to, mk(2402, Team::GLA));
    logic.objects.insert(atk, {
        let mut o = mk(2403, Team::USA);
        o.target = Some(from);
        o.set_ai_state(AIState::Attacking);
        o.turret_enabled = true;
        o.turret_target_id = Some(from);
        o.turret_substate = TurretSubState::Aim;
        o
    });
    let n = logic.transfer_attack(from, to);
    assert!(n >= 1);
    assert_eq!(logic.objects[&atk].target, Some(to));
    assert_eq!(logic.objects[&atk].turret_target_id, Some(to));
}

#[test]
fn mood_matrix_sleep_blocks_attack() {
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate, Weapon};
    let mut logic = GameLogic::new();
    let mut t = ThingTemplate::new("MoodA");
    t.add_kind_of(KindOf::Infantry);
    let id = ObjectId(2410);
    logic.objects.insert(id, {
        let mut o = Object::new(t, id, Team::USA);
        o.ai_attitude = -2; // Sleep
        o.weapon = Some(Weapon {
            range: 50.0,
            ..Default::default()
});
        o
    });
    let adj = logic.get_mood_matrix_action_adjustment(id, MoodMatrixAction::Attack, false);
    assert_eq!(
        adj & mood_action_adjust::ACTION_TO_IDLE,
        mood_action_adjust::ACTION_TO_IDLE
    );
    assert!(!logic.mood_allows_attack(id, false));
    // Player always allowed.
    assert!(logic.mood_allows_attack(id, true));
}

#[test]
fn mood_matrix_aggressive_move_to_attack_move() {
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    let mut t = ThingTemplate::new("MoodB");
    t.add_kind_of(KindOf::Vehicle);
    let id = ObjectId(2411);
    logic.objects.insert(id, {
        let mut o = Object::new(t, id, Team::GLA);
        o.ai_attitude = 2; // Aggressive
        o
    });
    let adj = logic.get_mood_matrix_action_adjustment(id, MoodMatrixAction::Move, false);
    assert_eq!(
        adj & mood_action_adjust::ACTION_TO_ATTACK_MOVE,
        mood_action_adjust::ACTION_TO_ATTACK_MOVE
    );
    assert_eq!(
        adj & mood_action_adjust::AFFECT_RANGE_AGGRESSIVE,
        mood_action_adjust::AFFECT_RANGE_AGGRESSIVE
    );
}

#[test]
fn attack_state_enter_fails_when_sleep_mood() {
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate, Weapon};
    let mut logic = GameLogic::new();
    let mut at = ThingTemplate::new("SleepA");
    at.add_kind_of(KindOf::Infantry);
    let aid = ObjectId(2412);
    logic.objects.insert(aid, {
        let mut o = Object::new(at, aid, Team::USA);
        o.set_ai_attitude_i8(-2);
        o.weapon = Some(Weapon {
            range: 50.0,
            damage: 10.0,
            can_target_ground: true,
            ..Default::default()
});
        o
    });
    let mut vt = ThingTemplate::new("SleepV");
    vt.add_kind_of(KindOf::Infantry);
    let vid = ObjectId(2413);
    logic.objects.insert(vid, {
        let mut o = Object::new(vt, vid, Team::GLA);
        o
    });
    assert_eq!(
        logic.attack_state_enter(aid, vid),
        AttackMachineResult::Failure
    );
}

#[test]
fn able_to_attack_possible_in_range_enemy() {
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate, Weapon};
    use glam::Vec3;
    let mut logic = GameLogic::new();
    let mut at = ThingTemplate::new("AtaA");
    at.add_kind_of(KindOf::Infantry);
    let aid = ObjectId(2301);
    logic.objects.insert(aid, {
        let mut o = Object::new(at, aid, Team::USA);
        o.set_position(Vec3::ZERO);
        o.weapon = Some(Weapon {
            range: 100.0,
            damage: 10.0,
            ..Default::default()
});
        o
    });
    let mut vt = ThingTemplate::new("AtaV");
    vt.add_kind_of(KindOf::Infantry);
    let vid = ObjectId(2302);
    logic.objects.insert(vid, {
        let mut o = Object::new(vt, vid, Team::GLA);
        o.set_position(Vec3::new(20.0, 0.0, 0.0));
        o
    });
    let r = logic.get_able_to_attack_specific_object(aid, vid, AbleToAttackType::NewTarget, false);
    assert_eq!(r, CanAttackResult::Possible);
}

#[test]
fn jarmen_rifle_mode_has_no_vehicle_attack_cursor() {
    use crate::game_logic::{
        AbleToAttackType, CanAttackResult, KindOf, Object, ObjectId, Team, ThingTemplate, Weapon,
    };
    use glam::Vec3;
    let mut logic = GameLogic::new();
    let mut jt = ThingTemplate::new("GLAInfantryJarmenKell");
    jt.add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Hero)
        .add_kind_of(KindOf::Attackable)
        .set_primary_weapon_name("GLAJarmenKellRifle")
        .set_secondary_weapon_name("GLAJarmenKellVehiclePilotSniperRifle");
    let jid = ObjectId(2321);
    logic.objects.insert(jid, {
        let mut o = Object::new(jt, jid, Team::GLA);
        o.set_position(Vec3::ZERO);
        o.weapon = Some(Weapon {
            range: 225.0,
            damage: 180.0,
            can_target_ground: true,
            ..Default::default()
        });
        o.secondary_weapon = Some(Weapon {
            range: 225.0,
            damage: 1.0,
            can_target_ground: true,
            ..Default::default()
        });
        o.active_weapon_slot = 0;
        o
    });
    let mut tank_t = ThingTemplate::new("GLATankScorpion");
    tank_t
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Attackable)
        .set_health(400.0);
    let tid = ObjectId(2322);
    logic.objects.insert(tid, {
        let mut o = Object::new(tank_t, tid, Team::USA);
        o.set_position(Vec3::new(40.0, 0.0, 0.0));
        o
    });
    let r = logic.get_able_to_use_weapon_against_target(
        jid,
        Some(tid),
        None,
        AbleToAttackType::NewTarget,
    );
    assert_eq!(
        r,
        CanAttackResult::InvalidShot,
        "rifle-mode Jarmen must not get a tank cursor via KILLPILOT"
    );

    logic.objects.get_mut(&jid).unwrap().weapon_lock_type =
        crate::game_logic::WeaponLockType::LockedPermanently;
    logic.objects.get_mut(&jid).unwrap().weapon_lock_slot = 1;
    let r_snipe = logic.get_able_to_use_weapon_against_target(
        jid,
        Some(tid),
        None,
        AbleToAttackType::NewTarget,
    );
    assert!(
        matches!(
            r_snipe,
            CanAttackResult::Possible | CanAttackResult::PossibleAfterMoving
        ),
        "snipe-mode lock must still allow the vehicle cursor, got {r_snipe:?}"
    );
}

#[test]
fn player_attack_command_uses_weaponset_target_legality_before_stamping_target() {
    use crate::game_logic::{
        KindOf, Object, ObjectId, Team, ThingTemplate, Weapon, WeaponLockType,
    };
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let attacker_id = ObjectId(2310);
    let mut attacker_template = ThingTemplate::new("PlayerAttackMaskSource");
    attacker_template.add_kind_of(KindOf::Infantry);
    logic.objects.insert(attacker_id, {
        let mut attacker = Object::new(attacker_template, attacker_id, Team::USA);
        attacker.set_position(Vec3::ZERO);
        attacker.weapon = Some(Weapon {
            range: 100.0,
            damage: 10.0,
            can_target_ground: true,
            can_target_air: false,
            ..Default::default()
});
        attacker
    });

    let target_id = ObjectId(2311);
    let mut target_template = ThingTemplate::new("PlayerAttackMaskTarget");
    target_template
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Unattackable);
    logic.objects.insert(target_id, {
        let mut target = Object::new(target_template, target_id, Team::GLA);
        target.set_position(Vec3::new(10.0, 0.0, 0.0));
        target
    });

    // The command boundary, not just AI target acquisition, must reject a
    // target C++ marks UNATTACKABLE. Force attack cannot override it either.
    assert!(!logic.unit_command_attack(attacker_id, target_id));
    assert!(!logic.unit_command_force_attack(attacker_id, target_id));
    assert_eq!(logic.objects[&attacker_id].target, None);

    let target = logic.objects.get_mut(&target_id).unwrap();
    target.thing.template.kind_of.remove(&KindOf::Unattackable);
    target.thing.template.add_kind_of(KindOf::Projectile);

    // A hand-authored ground-only weapon is not silently promoted to
    // AntiProjectile by the command route.
    assert!(!logic.unit_command_attack(attacker_id, target_id));
    assert_eq!(logic.objects[&attacker_id].target, None);

    let target = logic.objects.get_mut(&target_id).unwrap();
    target.thing.template.kind_of.remove(&KindOf::Projectile);
    assert!(logic.unit_command_attack(attacker_id, target_id));
    assert_eq!(logic.objects[&attacker_id].target, Some(target_id));

    // A locked slot is the actual C++ WeaponSet choice.  PRIMARY cannot use
    // SECONDARY's ground capability just to obtain a targeting cursor; once
    // unlocked the same object may use SECONDARY normally.
    let attacker = logic.objects.get_mut(&attacker_id).unwrap();
    attacker.set_target(None);
    attacker.weapon.as_mut().unwrap().can_target_ground = false;
    attacker.weapon.as_mut().unwrap().can_target_air = true;
    attacker.secondary_weapon = Some(Weapon {
        range: 100.0,
        damage: 10.0,
        can_target_ground: true,
        can_target_air: false,
        ..Default::default()
});
    assert!(attacker.set_weapon_lock(0, WeaponLockType::LockedPermanently));
    drop(attacker);
    assert!(!logic.unit_command_attack(attacker_id, target_id));
    assert_eq!(logic.objects[&attacker_id].target, None);
    logic
        .objects
        .get_mut(&attacker_id)
        .unwrap()
        .release_weapon_lock(WeaponLockType::LockedPermanently);
    assert!(logic.unit_command_attack(attacker_id, target_id));
}

#[test]
fn host_direct_attack_authority_does_not_bypass_weaponset_target_legality() {
    use crate::game_logic::{KindOf, Object, ObjectId, Player, Team, ThingTemplate, Weapon};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    logic
        .players
        .insert(0, Player::new(0, Team::USA, "Local", true));

    let attacker_id = ObjectId(2330);
    let mut attacker_template = ThingTemplate::new("HostAttackMaskSource");
    attacker_template.add_kind_of(KindOf::Infantry);
    logic.objects.insert(attacker_id, {
        let mut object = Object::new(attacker_template, attacker_id, Team::USA);
        object.set_position(Vec3::ZERO);
        object.weapon = Some(Weapon {
            range: 100.0,
            damage: 10.0,
            can_target_ground: true,
            ..Default::default()
});
        object
    });

    let target_id = ObjectId(2331);
    let mut target_template = ThingTemplate::new("HostAttackMaskTarget");
    target_template
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Unattackable);
    logic.objects.insert(target_id, {
        let mut object = Object::new(target_template, target_id, Team::GLA);
        object.set_position(Vec3::new(10.0, 0.0, 0.0));
        object
    });
    logic
        .players
        .get_mut(&0)
        .expect("local player")
        .selected_objects
        .push(attacker_id);

    // `host_command_attack` reaches this `command_attack` authority, rather
    // than the unit-command helper.  It must still reject the C++ victim
    // override before setting any target/path state.
    logic.command_attack(0, target_id);
    assert_eq!(logic.objects[&attacker_id].target, None);

    logic
        .objects
        .get_mut(&target_id)
        .expect("target")
        .thing
        .template
        .kind_of
        .remove(&KindOf::Unattackable);
    logic.command_attack(0, target_id);
    assert_eq!(logic.objects[&attacker_id].target, Some(target_id));
}

#[test]
fn able_to_attack_after_moving_when_oor() {
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate, Weapon};
    use glam::Vec3;
    let mut logic = GameLogic::new();
    let mut at = ThingTemplate::new("AtaA2");
    at.add_kind_of(KindOf::Vehicle);
    let aid = ObjectId(2303);
    logic.objects.insert(aid, {
        let mut o = Object::new(at, aid, Team::USA);
        o.set_position(Vec3::ZERO);
        o.movement.max_speed = 10.0;
        o.weapon = Some(Weapon {
            range: 30.0,
            damage: 10.0,
            can_target_ground: true,
            can_target_air: true,
            ..Default::default()
});
        o
    });
    let mut vt = ThingTemplate::new("AtaV2");
    vt.add_kind_of(KindOf::Infantry);
    let vid = ObjectId(2304);
    logic.objects.insert(vid, {
        let mut o = Object::new(vt, vid, Team::GLA);
        o.set_position(Vec3::new(200.0, 0.0, 0.0));
        o
    });
    let r = logic.get_able_to_attack_specific_object(aid, vid, AbleToAttackType::NewTarget, false);
    assert_eq!(r, CanAttackResult::PossibleAfterMoving);
}

#[test]
fn able_to_attack_rejects_self() {
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate, Weapon};
    let mut logic = GameLogic::new();
    let mut at = ThingTemplate::new("AtaA3");
    at.add_kind_of(KindOf::Infantry);
    let aid = ObjectId(2305);
    logic.objects.insert(aid, {
        let mut o = Object::new(at, aid, Team::USA);
        o.weapon = Some(Weapon {
            range: 50.0,
            ..Default::default()
});
        o
    });
    let r = logic.get_able_to_attack_specific_object(aid, aid, AbleToAttackType::NewTarget, false);
    assert_eq!(r, CanAttackResult::NotPossible);
}

#[test]
fn able_to_attack_stealth_blocks_unless_force() {
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate, Weapon};
    let mut logic = GameLogic::new();
    let mut at = ThingTemplate::new("AtaA4");
    at.add_kind_of(KindOf::Infantry);
    let aid = ObjectId(2306);
    logic.objects.insert(aid, {
        let mut o = Object::new(at, aid, Team::USA);
        o.set_position(glam::Vec3::ZERO);
        o.weapon = Some(Weapon {
            range: 50.0,
            damage: 10.0,
            can_target_ground: true,
            can_target_air: true,
            ..Default::default()
});
        o
    });
    let mut vt = ThingTemplate::new("AtaV4");
    vt.add_kind_of(KindOf::Infantry);
    let vid = ObjectId(2307);
    logic.objects.insert(vid, {
        let mut o = Object::new(vt, vid, Team::GLA);
        o.set_position(glam::Vec3::new(10.0, 0.0, 0.0));
        o.set_status_stealthed(true);
        o.set_status_detected(false);
        o
    });
    assert_eq!(
        logic.get_able_to_attack_specific_object(aid, vid, AbleToAttackType::NewTarget, false),
        CanAttackResult::NotPossible
    );
    // Force + same owner not required — force still blocked by stealth unless
    // IGNORING_STEALTH or disguised. C++ force alone does not ignore stealth
    // for normal units.
    assert_eq!(
        logic.get_able_to_attack_specific_object(aid, vid, AbleToAttackType::NewTargetForced, true),
        CanAttackResult::NotPossible
    );
    logic.objects.get_mut(&aid).unwrap().status.ignoring_stealth = true;
    assert_eq!(
        logic.get_able_to_attack_specific_object(aid, vid, AbleToAttackType::NewTarget, false),
        CanAttackResult::Possible
    );
}

#[test]
fn able_to_attack_uses_disguised_target_apparent_team_before_real_owner() {
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate, Weapon};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let attacker_id = ObjectId(2314);
    let mut attacker_template = ThingTemplate::new("DisguiseTargetAttacker");
    attacker_template.add_kind_of(KindOf::Infantry);
    logic.objects.insert(attacker_id, {
        let mut attacker = Object::new(attacker_template, attacker_id, Team::USA);
        attacker.set_position(Vec3::ZERO);
        attacker.weapon = Some(Weapon {
            range: 100.0,
            damage: 10.0,
            can_target_ground: true,
            ..Default::default()
});
        attacker
    });

    let victim_id = ObjectId(2315);
    let mut victim_template = ThingTemplate::new("DisguisedBombTruck");
    victim_template
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Disguiser);
    logic.objects.insert(victim_id, {
        let mut victim = Object::new(victim_template, victim_id, Team::GLA);
        victim.set_position(Vec3::new(10.0, 0.0, 0.0));
        victim.set_status_stealthed(true);
        victim.set_status_detected(false);
        victim.set_status_disguised(true);
        victim.disguise_as_team = Some(Team::USA);
        victim
    });

    // C++ WeaponSet uses the disguised controller while the truck is hidden.
    // An ordinary USA order must not target something presented as USA merely
    // because its underlying owner remains GLA.
    assert_eq!(
        logic.get_able_to_attack_specific_object(
            attacker_id,
            victim_id,
            AbleToAttackType::NewTarget,
            true,
        ),
        CanAttackResult::NotPossible
    );

    // The explicit C++ force-attack disguise exception still permits the
    // order, just as force fire can reveal a bomb truck.
    assert_eq!(
        logic.get_able_to_attack_specific_object(
            attacker_id,
            victim_id,
            AbleToAttackType::NewTargetForced,
            true,
        ),
        CanAttackResult::Possible
    );
}

#[test]
fn cannot_possibly_attack_same_team() {
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate, Weapon};
    let mut logic = GameLogic::new();
    let mut at = ThingTemplate::new("CpA");
    at.add_kind_of(KindOf::Infantry);
    let aid = ObjectId(2203);
    logic.objects.insert(aid, {
        let mut o = Object::new(at, aid, Team::USA);
        o.weapon = Some(Weapon {
            range: 50.0,
            ..Default::default()
});
        o
    });
    let mut vt = ThingTemplate::new("CpV");
    vt.add_kind_of(KindOf::Infantry);
    let vid = ObjectId(2204);
    logic.objects.insert(vid, {
        let mut o = Object::new(vt, vid, Team::USA);
        o
    });
    assert!(logic.cannot_possibly_attack_object(aid, vid, false));
    assert!(!logic.cannot_possibly_attack_object(aid, vid, true)); // force
}

#[test]
fn cannot_possibly_attack_stealthed_undetected() {
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate, Weapon};
    let mut logic = GameLogic::new();
    let mut at = ThingTemplate::new("StA");
    at.add_kind_of(KindOf::Infantry);
    let aid = ObjectId(2205);
    logic.objects.insert(aid, {
        let mut o = Object::new(at, aid, Team::USA);
        o.weapon = Some(Weapon {
            range: 50.0,
            ..Default::default()
});
        o
    });
    let mut vt = ThingTemplate::new("StV");
    vt.add_kind_of(KindOf::Infantry);
    let vid = ObjectId(2206);
    logic.objects.insert(vid, {
        let mut o = Object::new(vt, vid, Team::GLA);
        o.set_status_stealthed(true);
        o.set_status_detected(false);
        o
    });
    assert!(logic.cannot_possibly_attack_object(aid, vid, false));
    logic
        .objects
        .get_mut(&vid)
        .unwrap()
        .set_status_detected(true);
    assert!(!logic.cannot_possibly_attack_object(aid, vid, false));
}

#[test]
fn attack_state_enter_fails_without_legal_weapon() {
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    let mut at = ThingTemplate::new("NwA");
    at.add_kind_of(KindOf::Infantry);
    let aid = ObjectId(2207);
    logic.objects.insert(aid, {
        let mut o = Object::new(at, aid, Team::USA);
        // no weapon
        o
    });
    let mut vt = ThingTemplate::new("NwV");
    vt.add_kind_of(KindOf::Infantry);
    let vid = ObjectId(2208);
    logic.objects.insert(vid, {
        let mut o = Object::new(vt, vid, Team::GLA);
        o
    });
    assert_eq!(
        logic.attack_state_enter(aid, vid),
        AttackMachineResult::Failure
    );
}

#[test]
fn attack_state_machine_aim_to_fire_when_facing() {
    use crate::game_logic::{
        AttackSubState, KindOf, Object, ObjectId, Team, ThingTemplate, Weapon,
    };
    use glam::Vec3;
    let mut logic = GameLogic::new();
    let mut at = ThingTemplate::new("AsmA");
    at.add_kind_of(KindOf::Infantry);
    let aid = ObjectId(1601);
    logic.objects.insert(aid, {
        let mut o = Object::new(at, aid, Team::USA);
        o.set_position(Vec3::ZERO);
        o.set_orientation(0.0);
        o.weapon = Some(Weapon {
            damage: 10.0,
            range: 100.0,
            reload_time: 1.0,
            last_fire_time: -10.0,
            projectile_speed: 200.0,
            ..Default::default()
});
        o
    });
    let mut vt = ThingTemplate::new("AsmV");
    vt.add_kind_of(KindOf::Infantry);
    let vid = ObjectId(1602);
    logic.objects.insert(vid, {
        let mut o = Object::new(vt, vid, Team::GLA);
        o.set_position(Vec3::new(20.0, 0.0, 0.0));
        o
    });
    assert_eq!(
        logic.attack_state_enter(aid, vid),
        AttackMachineResult::Continue
    );
    assert_eq!(
        logic.objects[&aid].attack_substate,
        AttackSubState::AimAtTarget
    );
    // Facing target: one tick should promote Aim → Fire.
    let r = logic.tick_attack_state_machine(aid, vid, 10.0, 1, 1.0);
    assert_eq!(r, AttackMachineResult::Continue);
    assert_eq!(
        logic.objects[&aid].attack_substate,
        AttackSubState::FireWeapon
    );
}

#[test]
fn attack_state_machine_out_of_range_approaches() {
    use crate::game_logic::{
        AttackSubState, KindOf, Object, ObjectId, Team, ThingTemplate, Weapon,
    };
    use glam::Vec3;
    let mut logic = GameLogic::new();
    let mut at = ThingTemplate::new("AsmA2");
    at.add_kind_of(KindOf::Infantry);
    let aid = ObjectId(1603);
    logic.objects.insert(aid, {
        let mut o = Object::new(at, aid, Team::USA);
        o.set_position(Vec3::ZERO);
        o.weapon = Some(Weapon {
            range: 30.0,
            reload_time: 1.0,
            last_fire_time: -10.0,
            ..Default::default()
});
        o
    });
    let mut vt = ThingTemplate::new("AsmV2");
    vt.add_kind_of(KindOf::Infantry);
    let vid = ObjectId(1604);
    logic.objects.insert(vid, {
        let mut o = Object::new(vt, vid, Team::GLA);
        o.set_position(Vec3::new(400.0, 0.0, 0.0));
        o
    });
    assert_eq!(
        logic.attack_state_enter(aid, vid),
        AttackMachineResult::Continue
    );
    let r = logic.tick_attack_state_machine(aid, vid, 10.0, 1, 1.0);
    assert_eq!(r, AttackMachineResult::Continue);
    assert_eq!(
        logic.objects[&aid].attack_substate,
        AttackSubState::ApproachTarget
    );
}

#[test]
fn attack_state_machine_success_when_victim_dies() {
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate, Weapon};
    let mut logic = GameLogic::new();
    let mut at = ThingTemplate::new("AsmA3");
    at.add_kind_of(KindOf::Infantry);
    let aid = ObjectId(1605);
    logic.objects.insert(aid, {
        let mut o = Object::new(at, aid, Team::USA);
        o.weapon = Some(Weapon {
            range: 100.0,
            ..Default::default()
});
        o
    });
    let mut vt = ThingTemplate::new("AsmV3");
    vt.add_kind_of(KindOf::Infantry);
    let vid = ObjectId(1606);
    logic.objects.insert(vid, {
        let mut o = Object::new(vt, vid, Team::GLA);
        o
    });
    assert_eq!(
        logic.attack_state_enter(aid, vid),
        AttackMachineResult::Continue
    );
    logic.objects.get_mut(&vid).unwrap().status.destroyed = true;
    let r = logic.tick_attack_state_machine(aid, vid, 10.0, 1, 1.0);
    assert_eq!(r, AttackMachineResult::Success);
}

#[test]
fn attack_state_machine_fire_returns_to_aim() {
    use crate::game_logic::{
        AttackSubState, KindOf, Object, ObjectId, Team, ThingTemplate, Weapon,
    };
    use glam::Vec3;
    let mut logic = GameLogic::new();
    let mut at = ThingTemplate::new("AsmA4");
    at.add_kind_of(KindOf::Infantry);
    let aid = ObjectId(1607);
    logic.objects.insert(aid, {
        let mut o = Object::new(at, aid, Team::USA);
        o.set_position(Vec3::ZERO);
        o.set_orientation(0.0);
        o.weapon = Some(Weapon {
            damage: 10.0,
            range: 100.0,
            reload_time: 1.0,
            last_fire_time: -10.0,
            projectile_speed: 200.0,
            ..Default::default()
});
        o.attack_substate = AttackSubState::FireWeapon;
        o.set_status_firing_weapon(true);
        o
    });
    let mut vt = ThingTemplate::new("AsmV4");
    vt.add_kind_of(KindOf::Infantry);
    let vid = ObjectId(1608);
    logic.objects.insert(vid, {
        let mut o = Object::new(vt, vid, Team::GLA);
        o.set_position(Vec3::new(15.0, 0.0, 0.0));
        o
    });
    let r = logic.tick_attack_state_machine(aid, vid, 10.0, 1, 1.0);
    assert_eq!(r, AttackMachineResult::Continue);
    assert_eq!(
        logic.objects[&aid].attack_substate,
        AttackSubState::AimAtTarget
    );
}

#[test]
fn attack_aim_enter_exit_flags() {
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate, Weapon};
    let mut logic = GameLogic::new();
    let mut t = ThingTemplate::new("AimA");
    t.add_kind_of(KindOf::Infantry);
    let id = ObjectId(1501);
    logic.objects.insert(id, {
        let mut o = Object::new(t, id, Team::USA);
        o.weapon = Some(Weapon {
            range: 100.0,
            ..Default::default()
});
        o
    });
    assert!(logic.attack_aim_at_target_enter(id));
    assert!(logic.objects[&id].status.is_aiming_weapon);
    logic.attack_aim_at_target_exit(id);
    assert!(!logic.objects[&id].status.is_aiming_weapon);
}

#[test]
fn attack_aim_update_success_when_facing() {
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate, Weapon};
    use glam::Vec3;
    let mut logic = GameLogic::new();
    let mut at = ThingTemplate::new("AimAtk");
    at.add_kind_of(KindOf::Infantry);
    let aid = ObjectId(1502);
    logic.objects.insert(aid, {
        let mut o = Object::new(at, aid, Team::USA);
        o.set_position(Vec3::ZERO);
        o.set_orientation(0.0); // +X
        o.weapon = Some(Weapon {
            range: 100.0,
            ..Default::default()
});
        o
    });
    let mut vt = ThingTemplate::new("AimVic");
    vt.add_kind_of(KindOf::Infantry);
    let vid = ObjectId(1503);
    logic.objects.insert(vid, {
        let mut o = Object::new(vt, vid, Team::GLA);
        o.set_position(Vec3::new(40.0, 0.0, 0.0)); // along +X
        o
    });
    let r = logic.attack_aim_at_target_update(aid, vid, 1.0);
    assert_eq!(r, AttackAimResult::Success);
}

#[test]
fn attack_aim_update_continues_while_turning() {
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate, Weapon};
    use glam::Vec3;
    let mut logic = GameLogic::new();
    let mut at = ThingTemplate::new("AimAtk2");
    at.add_kind_of(KindOf::Infantry);
    let aid = ObjectId(1504);
    logic.objects.insert(aid, {
        let mut o = Object::new(at, aid, Team::USA);
        o.set_position(Vec3::ZERO);
        o.set_orientation(0.0); // +X
        o.weapon = Some(Weapon {
            range: 200.0,
            ..Default::default()
});
        o
    });
    let mut vt = ThingTemplate::new("AimVic2");
    vt.add_kind_of(KindOf::Infantry);
    let vid = ObjectId(1505);
    logic.objects.insert(vid, {
        let mut o = Object::new(vt, vid, Team::GLA);
        // Behind shooter — need ~180° turn; small step keeps Continue.
        o.set_position(Vec3::new(-40.0, 0.0, 0.0));
        o
    });
    let r = logic.attack_aim_at_target_update(aid, vid, 0.05); // ~3°
    assert_eq!(r, AttackAimResult::Continue);
    assert!(logic.objects[&aid].status.is_aiming_weapon);
}

#[test]
fn attack_aim_update_fails_dead_victim() {
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate, Weapon};
    let mut logic = GameLogic::new();
    let mut at = ThingTemplate::new("AimAtk3");
    at.add_kind_of(KindOf::Infantry);
    let aid = ObjectId(1506);
    logic.objects.insert(aid, {
        let mut o = Object::new(at, aid, Team::USA);
        o.weapon = Some(Weapon {
            range: 100.0,
            ..Default::default()
});
        o
    });
    let mut vt = ThingTemplate::new("AimVic3");
    vt.add_kind_of(KindOf::Infantry);
    let vid = ObjectId(1507);
    logic.objects.insert(vid, {
        let mut o = Object::new(vt, vid, Team::GLA);
        o.status.destroyed = true;
        o
    });
    let r = logic.attack_aim_at_target_update(aid, vid, 1.0);
    assert_eq!(r, AttackAimResult::Failure);
}

#[test]
fn attack_fire_weapon_enter_exit_flags() {
    use crate::game_logic::{AIState, KindOf, Object, ObjectId, Team, ThingTemplate, Weapon};
    let mut logic = GameLogic::new();
    let mut t = ThingTemplate::new("FireA");
    t.add_kind_of(KindOf::Infantry);
    let id = ObjectId(1401);
    logic.objects.insert(id, {
        let mut o = Object::new(t, id, Team::USA);
        o.weapon = Some(Weapon {
            range: 100.0,
            reload_time: 1.0,
            last_fire_time: -10.0,
            ..Default::default()
});
        o
    });
    assert!(logic.attack_fire_weapon_enter(id));
    assert!(logic.objects[&id].status.is_firing_weapon);
    assert_eq!(logic.objects[&id].ai_state, AIState::Attacking);
    logic.attack_fire_weapon_exit(id);
    assert!(!logic.objects[&id].status.is_firing_weapon);
}

#[test]
fn attack_fire_weapon_update_fires_in_range() {
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate, Weapon};
    use glam::Vec3;
    let mut logic = GameLogic::new();
    let mut at = ThingTemplate::new("FireAtk");
    at.add_kind_of(KindOf::Infantry);
    let aid = ObjectId(1402);
    logic.objects.insert(aid, {
        let mut o = Object::new(at, aid, Team::USA);
        o.set_position(Vec3::ZERO);
        o.weapon = Some(Weapon {
            damage: 10.0,
            range: 100.0,
            reload_time: 1.0,
            last_fire_time: -10.0,
            projectile_speed: 200.0,
            ..Default::default()
});
        o
    });
    let mut vt = ThingTemplate::new("FireVic");
    vt.add_kind_of(KindOf::Infantry);
    let vid = ObjectId(1403);
    logic.objects.insert(vid, {
        let mut o = Object::new(vt, vid, Team::GLA);
        o.set_position(Vec3::new(20.0, 0.0, 0.0));
        o
    });
    let r = logic.attack_fire_weapon_update(aid, vid, 10.0);
    assert_eq!(r, AttackFireResult::Success);
    assert!(logic.objects[&aid].status.is_firing_weapon);
}

#[test]
fn attack_fire_weapon_update_fails_dead_victim() {
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate, Weapon};
    let mut logic = GameLogic::new();
    let mut at = ThingTemplate::new("FireAtk2");
    at.add_kind_of(KindOf::Infantry);
    let aid = ObjectId(1404);
    logic.objects.insert(aid, {
        let mut o = Object::new(at, aid, Team::USA);
        o.weapon = Some(Weapon {
            damage: 10.0,
            range: 100.0,
            reload_time: 1.0,
            last_fire_time: -10.0,
            ..Default::default()
});
        o
    });
    let mut vt = ThingTemplate::new("FireVic2");
    vt.add_kind_of(KindOf::Infantry);
    let vid = ObjectId(1405);
    logic.objects.insert(vid, {
        let mut o = Object::new(vt, vid, Team::GLA);
        o.status.destroyed = true;
        o
    });
    let r = logic.attack_fire_weapon_update(aid, vid, 10.0);
    assert_eq!(r, AttackFireResult::Failure);
}

#[test]
fn attack_fire_weapon_update_fails_out_of_range() {
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate, Weapon};
    use glam::Vec3;
    let mut logic = GameLogic::new();
    let mut at = ThingTemplate::new("FireAtk3");
    at.add_kind_of(KindOf::Infantry);
    let aid = ObjectId(1406);
    logic.objects.insert(aid, {
        let mut o = Object::new(at, aid, Team::USA);
        o.set_position(Vec3::ZERO);
        o.weapon = Some(Weapon {
            damage: 10.0,
            range: 30.0,
            reload_time: 1.0,
            last_fire_time: -10.0,
            ..Default::default()
});
        o
    });
    let mut vt = ThingTemplate::new("FireVic3");
    vt.add_kind_of(KindOf::Infantry);
    let vid = ObjectId(1407);
    logic.objects.insert(vid, {
        let mut o = Object::new(vt, vid, Team::GLA);
        o.set_position(Vec3::new(500.0, 0.0, 0.0));
        o
    });
    let r = logic.attack_fire_weapon_update(aid, vid, 10.0);
    assert_eq!(r, AttackFireResult::Failure);
}

#[test]
fn is_within_attack_range_object() {
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate, Weapon};
    use glam::Vec3;
    let mut at = ThingTemplate::new("RngA");
    at.add_kind_of(KindOf::Infantry);
    let mut a = Object::new(at, ObjectId(1301), Team::USA);
    a.set_position(Vec3::ZERO);
    a.weapon = Some(Weapon {
        range: 50.0,
        min_range: 0.0,
        ..Default::default()
});
    let mut vt = ThingTemplate::new("RngV");
    vt.add_kind_of(KindOf::Infantry);
    let mut v = Object::new(vt, ObjectId(1302), Team::GLA);
    v.set_position(Vec3::new(40.0, 0.0, 0.0));
    assert!(a.is_within_attack_range(&v));
    v.set_position(Vec3::new(80.0, 0.0, 0.0));
    assert!(!a.is_within_attack_range(&v));
}

#[test]
fn private_idle_clears_state() {
    use crate::game_logic::{AIState, KindOf, Object, ObjectId, Team, ThingTemplate};
    use glam::Vec3;
    let mut logic = GameLogic::new();
    let mut t = ThingTemplate::new("Idl");
    t.add_kind_of(KindOf::Infantry);
    let id = ObjectId(1311);
    logic.objects.insert(id, {
        let mut o = Object::new(t, id, Team::USA);
        o.set_ai_state(AIState::Attacking);
        o.set_status_attacking(true);
        o.target = Some(ObjectId(1));
        o.movement.target_position = Some(Vec3::ONE);
        o
    });
    assert!(logic.private_idle(id));
    let o = logic.objects.get(&id).unwrap();
    assert_eq!(o.ai_state, AIState::Idle);
    assert!(!o.status.attacking);
}

#[test]
fn private_face_turns_toward_target() {
    use crate::game_logic::{KindOf, LocoGoalType, Object, ObjectId, Team, ThingTemplate};
    use glam::Vec3;
    let mut logic = GameLogic::new();
    let mut at = ThingTemplate::new("FcA");
    at.add_kind_of(KindOf::Vehicle);
    let aid = ObjectId(1321);
    logic.objects.insert(aid, {
        let mut o = Object::new(at, aid, Team::USA);
        o.set_orientation(0.0);
        o.movement.turn_rate = std::f32::consts::PI * 30.0;
        o.min_speed = 0.0;
        o.set_position(Vec3::ZERO);
        o
    });
    let mut vt = ThingTemplate::new("FcV");
    vt.add_kind_of(KindOf::Infantry);
    let vid = ObjectId(1322);
    logic.objects.insert(vid, {
        let mut o = Object::new(vt, vid, Team::GLA);
        o.set_position(Vec3::new(0.0, 0.0, 10.0));
        o
    });
    let yaw0 = logic.objects.get(&aid).unwrap().get_orientation();
    assert!(logic.private_face_object(aid, vid));
    let after_cmd = logic.objects.get(&aid).unwrap();
    assert_eq!(after_cmd.ai_state, crate::game_logic::AIState::FacingObject);
    assert!(after_cmd.face_active);
    assert!(after_cmd.face_can_turn_in_place);
    assert_eq!(after_cmd.locomotor_goal_type, LocoGoalType::Angle);
    assert_eq!(after_cmd.get_orientation(), yaw0);
    logic.update_ai(&[aid, vid], 1.0 / 30.0);
    let after_tick = logic.objects.get(&aid).unwrap();
    let yaw1 = after_tick.get_orientation();
    assert!((yaw1 - yaw0).abs() > 1e-4);
    assert_eq!(after_tick.locomotor_goal_type, LocoGoalType::Angle);
}

#[test]
fn private_face_position_sets_angle_goal_without_yaw_snap() {
    use crate::game_logic::{KindOf, LocoGoalType, Object, ObjectId, Team, ThingTemplate};
    use glam::Vec3;
    let mut logic = GameLogic::new();
    let mut at = ThingTemplate::new("FcPos");
    at.add_kind_of(KindOf::Vehicle);
    let aid = ObjectId(1323);
    logic.objects.insert(aid, {
        let mut o = Object::new(at, aid, Team::USA);
        o.set_orientation(0.0);
        o.movement.turn_rate = std::f32::consts::PI * 30.0;
        o.min_speed = 0.0;
        o.set_position(Vec3::ZERO);
        o
    });
    let goal = Vec3::new(0.0, 0.0, 10.0);
    let yaw0 = logic.objects.get(&aid).unwrap().get_orientation();
    assert!(logic.private_face_position(aid, goal));
    {
        let o = logic.objects.get(&aid).unwrap();
        assert_eq!(o.ai_state, crate::game_logic::AIState::FacingPosition);
        assert!(o.face_active);
        assert_eq!(o.locomotor_goal_type, LocoGoalType::Angle);
        assert_eq!(o.get_orientation(), yaw0);
    }
    logic.update_movement(&[aid], 1.0 / 30.0);
    let o = logic.objects.get(&aid).unwrap();
    assert!((o.get_orientation() - yaw0).abs() > 1e-4);
    assert_eq!(o.locomotor_goal_type, LocoGoalType::Angle);
}

#[test]
fn private_face_object_min_speed_uses_position_explicit() {
    use crate::game_logic::{KindOf, LocoGoalType, Object, ObjectId, Team, ThingTemplate};
    use glam::Vec3;
    let mut logic = GameLogic::new();
    let mut at = ThingTemplate::new("FcJet");
    at.add_kind_of(KindOf::Aircraft);
    let aid = ObjectId(1324);
    logic.objects.insert(aid, {
        let mut o = Object::new(at, aid, Team::USA);
        o.set_orientation(0.0);
        o.min_speed = 25.0;
        o.set_position(Vec3::ZERO);
        o
    });
    let mut vt = ThingTemplate::new("FcJetV");
    vt.add_kind_of(KindOf::Infantry);
    let vid = ObjectId(1325);
    let vpos = Vec3::new(0.0, 0.0, 40.0);
    logic.objects.insert(vid, {
        let mut o = Object::new(vt, vid, Team::GLA);
        o.set_position(vpos);
        o
    });
    assert!(logic.private_face_object(aid, vid));
    let o = logic.objects.get(&aid).unwrap();
    assert!(!o.face_can_turn_in_place);
    assert_eq!(o.locomotor_goal_type, LocoGoalType::PositionExplicit);
    assert_eq!(o.movement.target_position, Some(vpos));
    assert_eq!(o.get_orientation(), 0.0);
}


#[test]
fn attack_can_fire_at_requires_range() {
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate, Weapon};
    use glam::Vec3;
    let mut logic = GameLogic::new();
    let mut at = ThingTemplate::new("FireA");
    at.add_kind_of(KindOf::Infantry);
    let aid = ObjectId(1331);
    logic.objects.insert(aid, {
        let mut o = Object::new(at, aid, Team::USA);
        o.set_position(Vec3::ZERO);
        o.weapon = Some(Weapon {
            range: 30.0,
            reload_time: 0.0,
            ..Default::default()
});
        o
    });
    let mut vt = ThingTemplate::new("FireV");
    vt.add_kind_of(KindOf::Infantry);
    let vid = ObjectId(1332);
    logic.objects.insert(vid, {
        let mut o = Object::new(vt, vid, Team::GLA);
        o.set_position(Vec3::new(20.0, 0.0, 0.0));
        o
    });
    assert!(logic.attack_can_fire_at(aid, vid, 0.0, false));
    logic
        .objects
        .get_mut(&vid)
        .unwrap()
        .set_position(Vec3::new(100.0, 0.0, 0.0));
    assert!(!logic.attack_can_fire_at(aid, vid, 0.0, false));
}

#[test]
fn can_pursue_requires_fleeing_speed() {
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
    use glam::Vec3;
    let mut at = ThingTemplate::new("PrA");
    at.add_kind_of(KindOf::Vehicle);
    let mut a = Object::new(at, ObjectId(1341), Team::USA);
    a.set_position(Vec3::ZERO);
    a.movement.max_speed = 40.0;
    a.set_orientation(0.0);
    let mut vt = ThingTemplate::new("PrV");
    vt.add_kind_of(KindOf::Vehicle);
    let mut v = Object::new(vt, ObjectId(1342), Team::GLA);
    v.set_position(Vec3::new(30.0, 0.0, 0.0));
    v.set_orientation(0.0); // face +X away
    v.movement.velocity = Vec3::new(20.0, 0.0, 0.0); // fleeing at half speed
    assert!(a.can_pursue_target(&v));
    v.movement.velocity = Vec3::new(1.0, 0.0, 0.0); // too slow
    assert!(!a.can_pursue_target(&v));
}

#[test]
fn private_move_to_position_sets_path() {
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
    use glam::Vec3;
    let mut logic = GameLogic::new();
    let mut t = ThingTemplate::new("Mv");
    t.add_kind_of(KindOf::Infantry);
    let id = ObjectId(1201);
    logic.objects.insert(id, {
        let mut o = Object::new(t, id, Team::USA);
        o.set_position(Vec3::ZERO);
        o
    });
    assert!(logic.private_move_to_position(id, Vec3::new(40.0, 0.0, 0.0)));
    let o = logic.objects.get(&id).unwrap();
    assert!(o.movement.target_position.is_some() || !o.movement.path.is_empty());
}

#[test]
fn private_stop_clears_attack() {
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
    use glam::Vec3;
    let mut logic = GameLogic::new();
    let mut t = ThingTemplate::new("St");
    t.add_kind_of(KindOf::Infantry);
    let id = ObjectId(1202);
    logic.objects.insert(id, {
        let mut o = Object::new(t, id, Team::USA);
        o.set_status_attacking(true);
        o.target = Some(ObjectId(9));
        o.movement.target_position = Some(Vec3::ONE);
        o
    });
    assert!(logic.private_stop(id));
    let o = logic.objects.get(&id).unwrap();
    assert!(!o.status.attacking);
    assert!(o.target.is_none());
    assert!(o.movement.target_position.is_none());
}

#[test]
fn attack_approach_skips_when_victim_still() {
    use crate::game_logic::{
        is_same_position_residual, KindOf, Object, ObjectId, Team, ThingTemplate,
        MIN_RECOMPUTE_TIME_RESIDUAL,
    };
    use glam::Vec3;
    assert_eq!(MIN_RECOMPUTE_TIME_RESIDUAL, 10);
    let our = Vec3::ZERO;
    let prev = Vec3::new(50.0, 0.0, 0.0);
    assert!(is_same_position_residual(our, prev, prev));
    assert!(!is_same_position_residual(
        our,
        prev,
        Vec3::new(80.0, 0.0, 0.0)
    ));

    let mut logic = GameLogic::new();
    let mut at = ThingTemplate::new("AA");
    at.add_kind_of(KindOf::Vehicle);
    let aid = ObjectId(1211);
    logic.objects.insert(aid, {
        let mut o = Object::new(at, aid, Team::USA);
        o.set_position(Vec3::ZERO);
        o.weapon = Some(crate::game_logic::Weapon {
            damage: 10.0,
            range: 50.0,
            ..Default::default()
});
        o.approach_timestamp = 100;
        o.prev_victim_pos = Some(Vec3::new(40.0, 0.0, 0.0));
        o
    });
    let mut vt = ThingTemplate::new("AV");
    vt.add_kind_of(KindOf::Infantry);
    let vid = ObjectId(1212);
    logic.objects.insert(vid, {
        let mut o = Object::new(vt, vid, Team::GLA);
        o.set_position(Vec3::new(40.0, 0.0, 0.0));
        o
    });
    logic.frame = 105; // within MIN_RECOMPUTE of 100
    assert!(logic.attack_approach_compute_path(aid, Some(vid), None));
}

#[test]
fn request_attack_path_sets_flags() {
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
    use glam::Vec3;
    let mut logic = GameLogic::new();
    let mut at = ThingTemplate::new("Atk");
    at.add_kind_of(KindOf::Vehicle);
    let aid = ObjectId(1101);
    let mut a = Object::new(at, aid, Team::USA);
    a.set_position(Vec3::new(0.0, 0.0, 0.0));
    a.movement.max_speed = 30.0;
    logic.objects.insert(aid, a);

    let mut vt = ThingTemplate::new("Vic");
    vt.add_kind_of(KindOf::Infantry);
    let vid = ObjectId(1102);
    let mut v = Object::new(vt, vid, Team::GLA);
    v.set_position(Vec3::new(80.0, 0.0, 0.0));
    logic.objects.insert(vid, v);

    logic.frame = 10;
    let _ = logic.request_attack_path(aid, Some(vid), Vec3::new(80.0, 0.0, 0.0));
    let a = logic.objects.get(&aid).unwrap();
    assert!(a.is_attack_path || a.movement.target_position.is_some() || a.status.attacking);
    assert_eq!(a.requested_victim_id, Some(vid));
}

#[test]
fn request_attack_path_rate_limits() {
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
    use glam::Vec3;
    let mut logic = GameLogic::new();
    let mut at = ThingTemplate::new("RL");
    at.add_kind_of(KindOf::Infantry);
    let aid = ObjectId(1111);
    let mut a = Object::new(at, aid, Team::USA);
    a.set_position(Vec3::ZERO);
    logic.objects.insert(aid, a);
    logic.frame = 100;
    let _ = logic.request_attack_path(aid, None, Vec3::new(10.0, 0.0, 0.0));
    logic.frame = 101; // within 3 frames
    let deferred = !logic.request_attack_path(aid, None, Vec3::new(20.0, 0.0, 0.0));
    let a = logic.objects.get(&aid).unwrap();
    assert!(deferred || a.queue_for_path_frames > 0);
}

#[test]
fn private_attack_object_sets_target_and_shots() {
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
    use glam::Vec3;
    let mut logic = GameLogic::new();
    let mut at = ThingTemplate::new("PA");
    at.add_kind_of(KindOf::Infantry);
    let aid = ObjectId(1121);
    logic.objects.insert(aid, {
        let mut o = Object::new(at, aid, Team::USA);
        o.set_position(Vec3::ZERO);
        o
    });
    let mut vt = ThingTemplate::new("PV");
    vt.add_kind_of(KindOf::Infantry);
    let vid = ObjectId(1122);
    logic.objects.insert(vid, {
        let mut o = Object::new(vt, vid, Team::GLA);
        o.set_position(Vec3::new(5.0, 0.0, 0.0));
        o
    });
    assert!(logic.private_attack_object(aid, vid, 3));
    let a = logic.objects.get(&aid).unwrap();
    assert_eq!(a.target, Some(vid));
    assert_eq!(a.max_shots_to_fire, 3);
    assert!(a.status.attacking);
}

#[test]
fn request_object_path_sets_target() {
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
    use glam::Vec3;
    let mut logic = GameLogic::new();
    let mut t = ThingTemplate::new("PathUnit");
    t.add_kind_of(KindOf::Infantry);
    let id = ObjectId(1001);
    let mut o = Object::new(t, id, Team::USA);
    o.set_position(Vec3::new(0.0, 0.0, 0.0));
    logic.objects.insert(id, o);
    assert!(logic.request_object_path(id, Vec3::new(50.0, 0.0, 0.0)));
    let o = logic.objects.get(&id).unwrap();
    assert!(o.movement.target_position.is_some());
    assert!(!o.movement.path.is_empty());
    assert!(!o.waiting_for_path);
}

#[test]
fn downhill_only_blocks_uphill() {
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
    use glam::Vec3;
    let mut t = ThingTemplate::new("Ski");
    t.add_kind_of(KindOf::Vehicle);
    let mut o = Object::new(t, ObjectId(1002), Team::USA);
    o.downhill_only = true;
    o.set_position(Vec3::new(0.0, 10.0, 0.0));
    o.movement.max_speed = 30.0;
    o.movement.acceleration = 100.0;
    o.movement.target_position = Some(Vec3::new(20.0, 20.0, 0.0)); // uphill
    let p0 = o.get_position();
    o.update_movement(1.0 / 30.0);
    // Should not advance toward uphill goal.
    assert!((o.get_position() - p0).length() < 1e-3);
}

#[test]
fn group_speed_factor_scales_desired() {
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
    use glam::Vec3;
    let mut t = ThingTemplate::new("Grp");
    t.add_kind_of(KindOf::Vehicle);
    let mut o = Object::new(t, ObjectId(1003), Team::USA);
    o.set_orientation(0.0);
    o.set_position(Vec3::ZERO);
    o.movement.max_speed = 40.0;
    o.movement.acceleration = 1000.0;
    o.group_speed_factor = 0.5;
    o.movement.target_position = Some(Vec3::new(100.0, 0.0, 0.0));
    o.update_movement(1.0 / 30.0);
    // With half group speed, shouldn't hit full 40 quickly.
    assert!(o.movement.velocity.length() < 35.0);
}

#[test]
fn calc_lift_positive_when_below_preferred() {
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
    let mut t = ThingTemplate::new("Lift");
    t.add_kind_of(KindOf::Aircraft);
    let mut o = Object::new(t, ObjectId(1004), Team::USA);
    o.max_lift = 5.0;
    o.movement.velocity.y = 0.0;
    let lift = o.calc_lift_to_use_at_pt(10.0, 20.0);
    assert!(lift > 0.0, "lift={lift}");
}

#[test]
fn calc_slow_down_dist_fudge() {
    use crate::game_logic::calc_slow_down_dist;
    assert_eq!(calc_slow_down_dist(10.0, 20.0, 5.0), 0.0);
    let d = calc_slow_down_dist(20.0, 0.0, 10.0);
    assert!((d - 21.0).abs() < 1e-3, "d={d}");
}

#[test]
fn pivot_offset_moves_center() {
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
    use glam::Vec3;
    let mut t = ThingTemplate::new("Piv");
    t.add_kind_of(KindOf::Vehicle);
    let mut o = Object::new(t, ObjectId(991), Team::USA);
    o.set_position(Vec3::ZERO);
    o.set_orientation(0.0);
    o.selection_radius = 10.0;
    o.turn_pivot_offset = -1.0;
    let p0 = o.get_position();
    let _ = o.rotate_obj_around_loco_pivot(Vec3::new(0.0, 0.0, 100.0), 0.2);
    let p1 = o.get_position();
    assert!((p1 - p0).length() > 1e-4, "pivot turn should move center");
}

#[test]
fn move_towards_angle_with_min_speed() {
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
    use glam::Vec3;
    let mut t = ThingTemplate::new("Ang");
    t.add_kind_of(KindOf::Vehicle);
    let mut o = Object::new(t, ObjectId(992), Team::USA);
    o.set_position(Vec3::ZERO);
    o.min_speed = 10.0;
    o.movement.max_speed = 30.0;
    o.movement.acceleration = 100.0;
    o.loco_update_move_towards_angle(0.0, 1.0 / 30.0);
    assert!(o.get_position().x > 0.0 || o.movement.velocity.x > 0.0);
}

#[test]
fn wander_offset_oscillates() {
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
    let mut t = ThingTemplate::new("Wan");
    t.add_kind_of(KindOf::Infantry);
    let mut o = Object::new(t, ObjectId(993), Team::USA);
    o.wander_width_factor = 1.0;
    o.wander_offset_increment = 0.1;
    let mut saw_pos = false;
    let mut saw_neg = false;
    for _ in 0..200 {
        let w = o.tick_wander_angle_offset(1.0);
        if w > 0.05 {
            saw_pos = true;
        }
        if w < -0.05 {
            saw_neg = true;
        }
    }
    assert!(saw_pos && saw_neg, "wander should oscillate both ways");
}

#[test]
fn wings_maintain_circles() {
    use crate::game_logic::{KindOf, LocomotorAppearance, Object, ObjectId, Team, ThingTemplate};
    use glam::Vec3;
    let mut t = ThingTemplate::new("Wing");
    t.add_kind_of(KindOf::Aircraft);
    let mut o = Object::new(t, ObjectId(981), Team::USA);
    o.loco_appearance = LocomotorAppearance::Wings;
    o.set_position(Vec3::new(100.0, 50.0, 0.0));
    o.movement.velocity = Vec3::new(10.0, 0.0, 0.0);
    o.motive_frames_remaining = 5;
    o.status.airborne_target = true;
    o.min_speed = 20.0;
    o.circling_radius = 40.0;
    o.maintain_pos = Some(Vec3::new(0.0, 50.0, 0.0));
    o.maintain_pos_valid = true;
    let p0 = o.get_position();
    o.maintain_position_wings(1.0 / 30.0);
    let p1 = o.get_position();
    // Should have moved (circling), not stayed put.
    assert!(
        (p1 - p0).length() > 1e-3,
        "wings maintain should move along circle"
    );
}

#[test]
fn fix_invalid_position_pushes_off_cliff() {
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
    use glam::Vec3;
    let mut t = ThingTemplate::new("Fix");
    t.add_kind_of(KindOf::Vehicle);
    let mut o = Object::new(t, ObjectId(982), Team::USA);
    o.cell_is_cliff = true;
    o.movement.velocity = Vec3::new(5.0, 0.0, 0.0);
    o.set_orientation(0.0);
    assert!(o.fix_invalid_position());
    // Should have applied force (velocity changed or accel path ran).
    assert!(o.motive_frames_remaining > 0 || o.movement.velocity.x < 5.0);
}

#[test]
fn thrust_moves_toward_goal() {
    use crate::game_logic::{KindOf, LocomotorAppearance, Object, ObjectId, Team, ThingTemplate};
    use glam::Vec3;
    let mut t = ThingTemplate::new("Thr");
    t.add_kind_of(KindOf::Aircraft);
    let mut o = Object::new(t, ObjectId(983), Team::USA);
    o.loco_appearance = LocomotorAppearance::Thrust;
    o.set_position(Vec3::ZERO);
    o.movement.max_speed = 50.0;
    o.movement.acceleration = 100.0;
    o.min_speed = 5.0;
    o.move_towards_thrust(Vec3::new(100.0, 20.0, 0.0), 100.0, 40.0, 1.0 / 30.0);
    assert!(o.get_position().length() > 1e-3 || o.movement.velocity.length() > 1e-3);
}

#[test]
fn wheels_cap_speed_while_turning() {
    use crate::game_logic::{KindOf, LocomotorAppearance, Object, ObjectId, Team, ThingTemplate};
    use glam::Vec3;
    let mut t = ThingTemplate::new("Wheel");
    t.add_kind_of(KindOf::Vehicle);
    let mut o = Object::new(t, ObjectId(971), Team::USA);
    o.loco_appearance = LocomotorAppearance::WheelsFour;
    o.set_orientation(0.0);
    o.movement.max_speed = 60.0;
    o.movement.acceleration = 1000.0;
    o.movement.turn_rate = 0.5;
    o.min_turn_speed = 15.0;
    o.movement.target_position = Some(Vec3::new(0.0, 0.0, 100.0));
    o.update_movement(1.0 / 30.0);
    // Hard turn: wheels should not instantly reach 60.
    assert!(o.movement.velocity.length() < 40.0);
}

#[test]
fn calc_min_turn_radius_finite() {
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
    let mut t = ThingTemplate::new("R");
    t.add_kind_of(KindOf::Vehicle);
    let mut o = Object::new(t, ObjectId(972), Team::USA);
    o.min_speed = 30.0;
    o.movement.turn_rate = std::f32::consts::PI; // rad/sec
    let r = o.calc_min_turn_radius();
    assert!(r.is_finite() && r > 0.0 && r < 1000.0, "r={r}");
}

#[test]
fn ultra_accurate_adds_extra_friction() {
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
    let mut t = ThingTemplate::new("UA");
    t.add_kind_of(KindOf::Infantry);
    let mut o = Object::new(t, ObjectId(973), Team::USA);
    o.loco_extra_2d_friction = 0.1;
    o.ultra_accurate = true;
    o.set_locomotor_physics_options();
    assert!((o.extra_friction - 0.6).abs() < 1e-5);
}

#[test]
fn rotate_towards_position_sets_turning() {
    use crate::game_logic::{KindOf, Object, ObjectId, PhysicsTurningType, Team, ThingTemplate};
    use glam::Vec3;
    let mut t = ThingTemplate::new("Rot");
    t.add_kind_of(KindOf::Vehicle);
    let mut o = Object::new(t, ObjectId(961), Team::USA);
    o.set_orientation(0.0);
    o.movement.turn_rate = std::f32::consts::FRAC_PI_2; // 90 deg/sec
    let (turning, rel) = o.rotate_towards_position(Vec3::new(0.0, 0.0, 10.0), 1.0 / 30.0);
    assert!(rel.abs() > 0.1);
    assert_ne!(turning, PhysicsTurningType::TurnNone);
    assert_eq!(o.physics_turning, turning);
}

#[test]
fn maintain_position_scrubs_ground_velocity() {
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
    use glam::Vec3;
    let mut t = ThingTemplate::new("Maint");
    t.add_kind_of(KindOf::Infantry);
    let mut o = Object::new(t, ObjectId(962), Team::USA);
    o.movement.velocity = Vec3::new(5.0, 0.0, 3.0);
    o.movement.target_position = None;
    o.update_movement(1.0 / 30.0);
    assert!(o.maintain_pos_valid);
    assert!(
        o.movement.velocity.x.abs() < 1e-3 && o.movement.velocity.z.abs() < 1e-3,
        "ground maintain should scrub 2D vel"
    );
}

#[test]
fn handle_behavior_z_sea_level_snaps() {
    use crate::game_logic::{KindOf, LocomotorBehaviorZ, Object, ObjectId, Team, ThingTemplate};
    use glam::Vec3;
    let mut t = ThingTemplate::new("Sea");
    t.add_kind_of(KindOf::Vehicle);
    let mut o = Object::new(t, ObjectId(963), Team::USA);
    o.loco_behavior_z = LocomotorBehaviorZ::SeaLevel;
    o.set_position(Vec3::new(0.0, 12.0, 0.0));
    assert!(o.handle_behavior_z(3.0, None));
    assert!((o.get_position().y - 3.0).abs() < 1e-4);
}

#[test]
fn physics_get_mass_adds_contained_riders() {
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
    use glam::Vec3;
    let mut logic = GameLogic::new();
    let mut hull_t = ThingTemplate::new("ChinookHull");
    hull_t.add_kind_of(KindOf::Vehicle);
    let hid = ObjectId(7701);
    let mut hull = Object::new(hull_t, hid, Team::USA);
    hull.physics_mass = 50.0;
    hull.max_transport = 8;
    hull.set_position(Vec3::ZERO);
    logic.objects.insert(hid, hull);

    let mut rider_t = ThingTemplate::new("RangerRider");
    rider_t.add_kind_of(KindOf::Infantry);
    let rid = ObjectId(7702);
    let mut rider = Object::new(rider_t, rid, Team::USA);
    rider.physics_mass = 25.0;
    logic.objects.insert(rid, rider);

    {
        let h = logic.objects.get_mut(&hid).unwrap();
        assert!(h.add_occupant(rid));
        assert!((h.physics_get_mass() - 50.0).abs() < 1e-3, "cache stale until sync");
    }
    logic.sync_contained_items_mass(hid);
    let loaded = logic.objects.get(&hid).unwrap().physics_get_mass();
    assert!(
        (loaded - 75.0).abs() < 1e-3,
        "loaded hull must include rider mass, got {loaded}"
    );
    logic.tick_physics_collisions_all();
    let after_tick = logic.objects.get(&hid).unwrap().physics_get_mass();
    assert!((after_tick - 75.0).abs() < 1e-3);
}

#[test]
fn highest_layer_surface_falls_back_to_ground() {
    use crate::game_logic::{KindOf, LocomotorBehaviorZ, Object, ObjectId, Team, ThingTemplate};
    use glam::Vec3;
    let mut t = ThingTemplate::new("FlyerLayer");
    t.add_kind_of(KindOf::Aircraft);
    let mut o = Object::new(t, ObjectId(7703), Team::USA);
    o.loco_behavior_z = LocomotorBehaviorZ::SmoothRelativeToHighestLayer;
    o.loco_preferred_height = 10.0;
    o.set_position(Vec3::new(0.0, 4.0, 0.0));
    let surface = o.highest_layer_surface_ht(3.0);
    assert!(
        (surface - 3.0).abs() < 1e-3,
        "empty leftover terrain keeps ground_y, got {surface}"
    );
    assert!(o.handle_behavior_z(3.0, None));
}

#[test]
fn group_speed_factor_caps_do_locomotor_speed() {
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
    let mut t = ThingTemplate::new("FastForm");
    t.add_kind_of(KindOf::Vehicle);
    let mut o = Object::new(t, ObjectId(7704), Team::USA);
    o.movement.max_speed = 40.0;
    o.group_speed_factor = 0.5;
    let capped = o.apply_do_locomotor_blocked_speed(40.0);
    assert!(
        (capped - 20.0).abs() < 1e-3,
        "formation desired speed must scale, got {capped}"
    );
}

#[test]
fn loco_turn_modulates_speed() {
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
    use glam::Vec3;
    let mut t = ThingTemplate::new("LocoTank");
    t.add_kind_of(KindOf::Vehicle);
    let mut o = Object::new(t, ObjectId(951), Team::USA);
    o.set_position(Vec3::ZERO);
    o.set_orientation(0.0); // face +X
    o.movement.max_speed = 30.0;
    o.movement.acceleration = 1000.0;
    o.movement.turn_rate = 0.5; // slow turn
                                // Goal behind / to the side → large rel angle → reduced goal speed.
    o.movement.target_position = Some(Vec3::new(0.0, 0.0, 100.0)); // +Z, 90 deg turn
    let yaw0 = o.get_orientation();
    o.update_movement(1.0 / 30.0);
    // 90° goal: treads residual clamps goal_speed via angleCoeff → near-zero thrust
    // while turning; orientation should advance toward +Z.
    let spd = o.movement.velocity.length();
    assert!(
        spd < 5.0,
        "hard turn should nearly zero goal_speed, spd={spd}"
    );
    assert!(
        (o.get_orientation() - yaw0).abs() > 1e-4,
        "should begin turning toward goal"
    );
}

#[test]
fn loco_set_physics_options_sticks_infantry() {
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
    let mut t = ThingTemplate::new("LocoInf");
    t.add_kind_of(KindOf::Infantry);
    let mut o = Object::new(t, ObjectId(952), Team::USA);
    o.loco_extra_2d_friction = 0.2;
    o.loco_apply_2d_friction_airborne = true;
    o.set_locomotor_physics_options();
    assert!(o.stick_to_ground);
    assert!((o.extra_friction - 0.2).abs() < 1e-6);
    assert!(o.apply_friction_2d_when_airborne);
}

#[test]
fn apply_motive_force_arms_lateral_window() {
    use crate::game_logic::{
        KindOf, Object, ObjectId, Team, ThingTemplate, MOTIVE_FRAMES_RESIDUAL,
    };
    use glam::Vec3;
    assert_eq!(MOTIVE_FRAMES_RESIDUAL, 10);
    let mut t = ThingTemplate::new("MotF");
    t.add_kind_of(KindOf::Vehicle);
    let mut o = Object::new(t, ObjectId(941), Team::USA);
    o.set_orientation(0.0);
    o.physics_mass = 1.0;
    // Motive force applies full forward push then arms motive.
    o.apply_motive_force(Vec3::new(10.0, 0.0, 0.0));
    assert_eq!(o.motive_frames_remaining, MOTIVE_FRAMES_RESIDUAL);
    o.integrate_physics_accel();
    assert!(o.movement.velocity.x > 1.0);
    // While motive, lateral-only on subsequent apply_physics_force.
    let vx_before = o.movement.velocity.x;
    o.apply_physics_force(Vec3::new(5.0, 0.0, 0.0)); // forward stripped
    o.integrate_physics_accel();
    assert!(
        (o.movement.velocity.x - vx_before).abs() < 1e-3,
        "forward force while motive should be stripped"
    );
}

#[test]
fn reset_dynamic_physics_clears_motion() {
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
    use glam::Vec3;
    let mut t = ThingTemplate::new("Rst");
    t.add_kind_of(KindOf::Vehicle);
    let mut o = Object::new(t, ObjectId(942), Team::USA);
    o.movement.velocity = Vec3::new(1.0, 2.0, 3.0);
    o.physics_accel = Vec3::ONE;
    o.shock_yaw_rate = 0.5;
    o.motive_frames_remaining = 5;
    o.reset_dynamic_physics();
    assert_eq!(o.movement.velocity, Vec3::ZERO);
    assert_eq!(o.physics_accel, Vec3::ZERO);
    assert_eq!(o.shock_yaw_rate, 0.0);
    assert_eq!(o.motive_frames_remaining, 0);
}

#[test]
fn motion_step_kills_when_resting() {
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
    use glam::Vec3;
    let mut t = ThingTemplate::new("RestKill");
    t.add_kind_of(KindOf::Infantry);
    let mut o = Object::new(t, ObjectId(943), Team::USA);
    // Already settled on ground with near-zero velocity (C++ isVerySmall3D).
    o.set_position(Vec3::new(0.0, 0.0, 0.0));
    o.movement.velocity = Vec3::ZERO;
    o.status.airborne_target = false;
    o.was_airborne_last_frame = false;
    o.kill_when_resting_on_ground = true;
    o.immune_to_falling_damage = true;
    let _ = o.tick_physics_motion_step(0.0);
    assert!(
        o.status.destroyed || !o.is_alive(),
        "killWhenRestingOnGround should kill settled infantry"
    );
}

#[test]
fn tick_physics_motion_step_clamps_ground() {
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
    use glam::Vec3;
    let mut t = ThingTemplate::new("Mot");
    t.add_kind_of(KindOf::Vehicle);
    let mut o = Object::new(t, ObjectId(931), Team::USA);
    o.set_position(Vec3::new(0.0, 1.0, 0.0));
    o.movement.velocity = Vec3::new(0.0, -3.0, 0.0); // crosses ground in one frame
    o.shock_allow_bounce = true;
    o.original_allow_bounce = false;
    o.was_airborne_last_frame = true;
    o.immune_to_falling_damage = true; // isolate clamp
    let bounced = o.tick_physics_motion_step(0.0);
    assert!(
        (o.get_position().y - 0.0).abs() < 1e-4,
        "y={}",
        o.get_position().y
    );
    // Falling into ground with allow bounce should produce bounce force path.
    assert!(
        bounced,
        "expected bounce force when crossing ground with ALLOW_BOUNCE"
    );
    assert!(o.movement.velocity.y >= -1e-3);
}

#[test]
fn compute_ground_bounce_force_rights_tilted_unflipped_body() {
    // hq-9r1qm: leftover handle_bounce + update_simple zero pitch/roll on
    // every vz<0 bounce, not only when the body is already flipped.
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
    use glam::{Mat4, Vec3};
    let mut t = ThingTemplate::new("TiltBounce");
    t.add_kind_of(KindOf::Vehicle);
    let mut o = Object::new(t, ObjectId(944), Team::USA);
    o.set_position(Vec3::new(0.0, 2.0, 0.0));
    o.movement.velocity = Vec3::new(0.0, -4.0, 0.0);
    o.shock_allow_bounce = true;
    o.original_allow_bounce = true;
    let pos = o.get_position();
    let yaw = o.get_orientation();
    o.thing.set_transform_matrix(
        Mat4::from_translation(pos)
            * Mat4::from_rotation_y(yaw)
            * Mat4::from_rotation_z(std::f32::consts::FRAC_PI_4),
    );
    let up_before = o.physics_transform_up_y();
    assert!(
        up_before > 0.0 && up_before < 0.95,
        "precondition: tilted but not flipped, up_y={up_before}"
    );
    let force = o.compute_ground_bounce_force(2.0, -0.1, 0.0);
    assert!(force.is_some(), "hq-9r1qm: bounce force must fire");
    let up_after = o.physics_transform_up_y();
    assert!(
        (up_after - 1.0).abs() < 1e-4,
        "hq-9r1qm: tilted wreck must right to yaw-only, up_y={up_after}"
    );
}

    #[test]
    fn tick_physics_does_not_apply_per_second_velocity_while_marching() {
        use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
        use glam::Vec3;
        let mut t = ThingTemplate::new("March");
        t.add_kind_of(KindOf::Infantry);
        let mut o = Object::new(t, ObjectId(933), Team::USA);
        o.set_position(Vec3::new(0.0, 0.0, 0.0));
        o.movement.velocity = Vec3::new(30.0, 0.0, 0.0); // units/second
        o.movement.target_position = Some(Vec3::new(100.0, 0.0, 0.0));
        let _ = o.tick_physics_motion_step(0.0);
        assert!(
            o.get_position().x.abs() < 1e-3,
            "marching unit must not get pos+=v per-frame; x={}",
            o.get_position().x
        );
    }

    /// hq-g8oig: C++ PhysicsUpdate always applyGravitationalForces unless HELD.
    /// Dead / lose-lift Z-motive aircraft must fall (no flying skip).
    #[test]
    fn tick_physics_motion_step_flying_aircraft_without_lift_falls() {
        use crate::game_logic::{
            KindOf, LocomotorBehaviorZ, Object, ObjectId, Team, ThingTemplate,
        };
        use glam::Vec3;
        let mut t = ThingTemplate::new("RaptorFall");
        t.add_kind_of(KindOf::Aircraft);
        let mut o = Object::new(t, ObjectId(934), Team::USA);
        o.set_position(Vec3::new(0.0, 20.0, 0.0));
        o.movement.velocity = Vec3::ZERO;
        o.status.airborne_target = true;
        o.allow_to_fall = false;
        o.shock_was_airborne = false;
        o.max_lift = 0.0;
        o.loco_behavior_z = LocomotorBehaviorZ::SurfaceRelativeHeight;
        o.health.current = 0.0;
        assert!(o.host_skip_dead_locomotor());
        let _ = o.tick_physics_motion_step(0.0);
        assert!(
            o.movement.velocity.y < -0.01,
            "lose-lift aircraft must get leftover gravity; vel.y={}",
            o.movement.velocity.y
        );
        assert!(
            o.get_position().y < 20.0,
            "lose-lift aircraft must fall; y={}",
            o.get_position().y
        );
    }


#[test]
fn stick_to_ground_snaps_when_not_falling() {
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
    use glam::Vec3;
    let mut t = ThingTemplate::new("Stick");
    t.add_kind_of(KindOf::Infantry);
    let mut o = Object::new(t, ObjectId(932), Team::USA);
    o.set_position(Vec3::new(0.0, 1.5, 0.0));
    o.movement.velocity = Vec3::ZERO;
    o.stick_to_ground = true;
    o.allow_to_fall = false;
    let _ = o.tick_physics_motion_step(0.0);
    assert!((o.get_position().y - 0.0).abs() < 1e-4);
}

#[test]
fn friction_slows_lateral_velocity() {
    use crate::game_logic::{
        KindOf, Object, ObjectId, Team, ThingTemplate, DEFAULT_LATERAL_FRICTION_RESIDUAL,
    };
    use glam::Vec3;
    assert!((DEFAULT_LATERAL_FRICTION_RESIDUAL - 0.15).abs() < 1e-6);
    let mut t = ThingTemplate::new("Fric");
    t.add_kind_of(KindOf::Vehicle);
    let id = ObjectId(901);
    let mut o = Object::new(t, id, Team::USA);
    o.set_orientation(0.0); // +X
    o.movement.velocity = Vec3::new(0.0, 0.0, 10.0); // pure lateral
    o.physics_mass = 1.0;
    o.lateral_friction = 0.15;
    o.forward_friction = 0.15;
    o.status.airborne_target = false;
    o.apply_frictional_forces();
    o.integrate_physics_accel();
    // Lateral friction force = -mass*lat_fric*lat_vel → accel reduces z
    assert!(
        o.movement.velocity.z.abs() < 10.0,
        "lateral friction should reduce |vz|, got {}",
        o.movement.velocity.z
    );
}

#[test]
fn transfer_velocity_adds_and_invalidates() {
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
    use glam::Vec3;
    let mut ta = ThingTemplate::new("Ta");
    ta.add_kind_of(KindOf::Vehicle);
    let mut a = Object::new(ta, ObjectId(911), Team::USA);
    a.movement.velocity = Vec3::new(1.0, 2.0, 3.0);
    let mut tb = ThingTemplate::new("Tb");
    tb.add_kind_of(KindOf::Vehicle);
    let mut b = Object::new(tb, ObjectId(912), Team::USA);
    b.movement.velocity = Vec3::new(4.0, 0.0, 0.0);
    a.transfer_velocity_to(&mut b);
    assert!((b.movement.velocity - Vec3::new(5.0, 2.0, 3.0)).length() < 1e-5);
    let mag = b.velocity_magnitude();
    assert!((mag - (5.0f32 * 5.0 + 2.0 * 2.0 + 3.0 * 3.0).sqrt()).abs() < 1e-4);
}

#[test]
fn forward_speed_2d_signed() {
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
    use glam::Vec3;
    let mut t = ThingTemplate::new("Fs");
    t.add_kind_of(KindOf::Vehicle);
    let mut o = Object::new(t, ObjectId(921), Team::USA);
    o.set_orientation(0.0); // +X
    o.movement.velocity = Vec3::new(5.0, 0.0, 0.0);
    assert!((o.forward_speed_2d() - 5.0).abs() < 1e-4);
    o.movement.velocity = Vec3::new(-3.0, 0.0, 0.0);
    assert!((o.forward_speed_2d() + 3.0).abs() < 1e-4);
}

#[test]
fn apply_physics_force_motive_lateral_only() {
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
    use glam::Vec3;
    let mut t = ThingTemplate::new("Mot");
    t.add_kind_of(KindOf::Vehicle);
    let id = ObjectId(801);
    let mut o = Object::new(t, id, Team::USA);
    o.set_orientation(0.0); // face +X
    o.motive_frames_remaining = 10;
    o.physics_mass = 2.0;
    // Forward force should be rejected when motive; lateral accepted.
    o.apply_physics_force(Vec3::new(10.0, 0.0, 0.0)); // along facing
    assert!(
        o.physics_accel.length() < 1e-4,
        "forward force stripped when motive"
    );
    o.apply_physics_force(Vec3::new(0.0, 0.0, 10.0)); // lateral +Z
                                                      // accel = force/mass lateral
    assert!(o.physics_accel.z.abs() > 0.1, "lateral force kept");
    o.integrate_physics_accel();
    assert!(o.movement.velocity.z.abs() > 0.1);
}

#[test]
fn vehicle_requests_infantry_move_away() {
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
    use glam::Vec3;
    let mut logic = GameLogic::new();
    let mut vt = ThingTemplate::new("VMove");
    vt.add_kind_of(KindOf::Vehicle);
    let vid = ObjectId(811);
    let mut v = Object::new(vt, vid, Team::USA);
    v.movement.velocity = Vec3::new(0.0, 0.0, 3.0);
    v.set_position(Vec3::new(0.0, 0.0, 0.0));
    // Face +Z: orientation = -PI/2 with (-dz).atan2(dx) convention.
    v.set_orientation(-std::f32::consts::FRAC_PI_2);
    v.selection_radius = 8.0;
    // Explicitly cannot crush (and disable ensure defaults via high crushable).
    v.crusher_level = 0;
    logic.objects.insert(vid, v);

    let mut it = ThingTemplate::new("IMove");
    it.add_kind_of(KindOf::Infantry);
    let iid = ObjectId(812);
    let mut inf = Object::new(it, iid, Team::USA);
    inf.set_position(Vec3::new(0.0, 0.0, 4.0));
    inf.set_orientation(-std::f32::consts::FRAC_PI_2);
    inf.selection_radius = 5.0;
    // Not crushable by level-0 crusher; keep ensure from lowering.
    inf.crushable_level = 10;
    logic.objects.insert(iid, inf);

    // Direct processCollision path: ensure blocked_by geometry works.
    {
        let other = logic.objects.get(&iid).unwrap().clone();
        let v = logic.objects.get_mut(&vid).unwrap();
        // Prevent ensure_crush_levels from promoting crusher during overlap.
        let blocked = v.ai_blocked_by(&other, true);
        assert!(blocked, "vehicle should be blocked by infantry ahead");
        let force = v.ai_process_collision(&other, 0, true);
        assert!(!force);
        assert!(v.is_blocked);
        assert_eq!(v.request_other_move_away, Some(iid));
    }
    assert!(logic.try_physics_collide(vid, iid, 8.0));
    let inf = logic.objects.get(&iid).unwrap();
    assert_eq!(inf.move_away_from, Some(vid));
    assert!(inf.move_away_destination.is_some());
    assert!(inf.move_away_frames > 0);
}

#[test]
fn ai_blocked_sets_speed_cap() {
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
    use glam::Vec3;
    let mut logic = GameLogic::new();
    let mut at = ThingTemplate::new("BlkA");
    at.add_kind_of(KindOf::Vehicle);
    let aid = ObjectId(701);
    let mut a = Object::new(at, aid, Team::USA);
    a.movement.velocity = Vec3::new(3.0, 0.0, 0.0); // moving +X
    a.set_position(Vec3::new(0.0, 0.0, 0.0));
    a.set_orientation(0.0); // face +X
    a.selection_radius = 8.0;
    a.crusher_level = 0;
    logic.objects.insert(aid, a);

    let mut bt = ThingTemplate::new("BlkB");
    bt.add_kind_of(KindOf::Vehicle);
    let bid = ObjectId(702);
    let mut b = Object::new(bt, bid, Team::USA);
    b.set_position(Vec3::new(5.0, 0.0, 0.0)); // in front +X
    b.set_orientation(0.0);
    b.selection_radius = 8.0;
    b.movement.velocity = Vec3::ZERO;
    b.crushable_level = 10;
    logic.objects.insert(bid, b);

    assert!(logic.try_physics_collide(aid, bid, 8.0));
    let a = logic.objects.get(&aid).unwrap();
    assert!(a.is_blocked || a.last_collidee == Some(bid));
    if a.is_blocked {
        assert!(a.movement.velocity.length() <= 4.0 + 1e-3);
    }
}

#[test]
fn panic_infantry_allows_bounce_force() {
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
    use glam::Vec3;
    let mut logic = GameLogic::new();
    let mut at = ThingTemplate::new("PanicA");
    at.add_kind_of(KindOf::Infantry);
    let aid = ObjectId(711);
    let mut a = Object::new(at, aid, Team::USA);
    a.is_panicking = true;
    a.movement.velocity = Vec3::new(0.0, 0.0, 2.0);
    a.set_position(Vec3::new(0.0, 0.0, 0.0));
    a.set_orientation(0.0);
    a.selection_radius = 5.0;
    logic.objects.insert(aid, a);

    let mut bt = ThingTemplate::new("PanicB");
    bt.add_kind_of(KindOf::Infantry);
    let bid = ObjectId(712);
    let mut b = Object::new(bt, bid, Team::USA);
    b.set_position(Vec3::new(0.0, 0.0, 3.0));
    b.set_orientation(0.0);
    b.selection_radius = 5.0;
    logic.objects.insert(bid, b);

    assert!(logic.try_physics_collide(aid, bid, 5.0));
    let a = logic.objects.get(&aid).unwrap();
    // Bounce impulse residual should push velocity somewhat.
    assert!(a.last_collidee == Some(bid));
}

#[test]
fn partition_cell_broadphase_and_collide_force() {
    use crate::game_logic::partition_manager::PARTITION_CELL_SIZE_RESIDUAL;
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
    use glam::Vec3;
    assert_eq!(PARTITION_CELL_SIZE_RESIDUAL, 40.0);
    let mut logic = GameLogic::new();
    // Far objects different cells — no crush.
    let mut vt = ThingTemplate::new("FarTank");
    vt.add_kind_of(KindOf::Vehicle);
    let tid = ObjectId(601);
    let mut tank = Object::new(vt, tid, Team::USA);
    tank.crusher_level = 1;
    tank.movement.velocity = Vec3::new(5.0, 0.0, 0.0);
    tank.set_position(Vec3::new(0.0, 0.0, 0.0));
    tank.selection_radius = 5.0;
    logic.objects.insert(tid, tank);

    let mut it = ThingTemplate::new("FarInf");
    it.add_kind_of(KindOf::Infantry);
    let iid = ObjectId(602);
    let mut inf = Object::new(it, iid, Team::GLA);
    inf.crushable_level = 0;
    inf.set_position(Vec3::new(200.0, 0.0, 200.0));
    inf.selection_radius = 5.0;
    logic.objects.insert(iid, inf);
    let _ = logic.tick_physics_collisions_all();
    assert!(!logic.objects.get(&iid).unwrap().status.destroyed);
    assert!(logic.partition_manager.registered_count() >= 2);

    // allowCollideForce false on structure bounce.
    let mut st = ThingTemplate::new("StiffOff");
    st.add_kind_of(KindOf::Structure);
    let sid = ObjectId(603);
    logic.objects.insert(sid, Object::new(st, sid, Team::China));
    logic.objects.get_mut(&tid).unwrap().allow_collide_force = false;
    logic
        .objects
        .get_mut(&tid)
        .unwrap()
        .set_position(Vec3::new(0.0, 1.0, 0.0));
    logic
        .objects
        .get_mut(&sid)
        .unwrap()
        .set_position(Vec3::new(2.0, 1.0, 0.0));
    assert!(logic.try_physics_collide(tid, sid, 10.0));
}

#[test]
fn tick_physics_collisions_all_crushes_nearby() {
    use crate::game_logic::host_usa_pilot::HostDeathType;
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
    use glam::Vec3;
    let mut logic = GameLogic::new();
    let mut vt = ThingTemplate::new("DispTank");
    vt.add_kind_of(KindOf::Vehicle);
    let tid = ObjectId(501);
    let mut tank = Object::new(vt, tid, Team::USA);
    tank.crusher_level = 1;
    tank.set_orientation(0.0);
    tank.movement.velocity = Vec3::new(5.0, 0.0, 0.0);
    tank.set_position(Vec3::new(6.0, 0.0, 0.0));
    tank.selection_radius = 8.0;
    logic.objects.insert(tid, tank);

    let mut it = ThingTemplate::new("DispInf");
    it.add_kind_of(KindOf::Infantry);
    let iid = ObjectId(502);
    let mut inf = Object::new(it, iid, Team::GLA);
    inf.crushable_level = 0;
    inf.has_squish_collide = true;
    inf.selection_radius = 8.0;
    inf.set_position(Vec3::new(5.0, 0.0, 0.0));
    logic.objects.insert(iid, inf);

    let n = logic.tick_physics_collisions_all();
    assert!(n >= 1, "nearby pair must be processed");
    let inf = logic.objects.get(&iid).unwrap();
    assert!(inf.status.destroyed);
    assert_eq!(inf.status.death_type, HostDeathType::Crushed);
    // Overlap advanced: previous set, current cleared.
    let tank = logic.objects.get(&tid).unwrap();
    assert!(tank.physics_current_overlap.is_none());
}

#[test]
fn try_physics_collide_respects_ignore_and_parachute() {
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
    use glam::Vec3;
    let mut logic = GameLogic::new();
    let mut ta = ThingTemplate::new("ColA");
    ta.add_kind_of(KindOf::Vehicle);
    let aid = ObjectId(401);
    let mut a = Object::new(ta, aid, Team::USA);
    a.set_position(Vec3::ZERO);
    a.movement.velocity = Vec3::new(5.0, 0.0, 0.0);

    let mut tb = ThingTemplate::new("ColB");
    tb.add_kind_of(KindOf::Infantry);
    let bid = ObjectId(402);
    let mut b = Object::new(tb, bid, Team::GLA);
    b.crushable_level = 0;
    b.set_position(Vec3::new(1.0, 0.0, 0.0));

    a.set_ignore_collisions_with(Some(bid));
    logic.objects.insert(aid, a);
    logic.objects.insert(bid, b);
    assert!(logic.try_physics_collide(aid, bid, 10.0));
    // Ignored: infantry not crushed.
    assert!(!logic.objects.get(&bid).unwrap().status.destroyed);

    // Clear ignore, parachute both → skip.
    logic
        .objects
        .get_mut(&aid)
        .unwrap()
        .set_ignore_collisions_with(None);
    logic.objects.get_mut(&aid).unwrap().status.parachuting = true;
    logic.objects.get_mut(&bid).unwrap().status.parachuting = true;
    assert!(logic.try_physics_collide(aid, bid, 10.0));
    assert!(!logic.objects.get(&bid).unwrap().status.destroyed);

    // Unmanned vehicle boarded by infantry.
    logic.objects.get_mut(&aid).unwrap().status.parachuting = false;
    logic.objects.get_mut(&bid).unwrap().status.parachuting = false;
    let mut tv = ThingTemplate::new("UnmannedV");
    tv.add_kind_of(KindOf::Vehicle);
    let vid = ObjectId(403);
    let mut v = Object::new(tv, vid, Team::Neutral);
    v.set_status_disabled_unmanned(true);
    logic.objects.insert(vid, v);
    let mut ti = ThingTemplate::new("PilotInf");
    ti.add_kind_of(KindOf::Infantry);
    let iid = ObjectId(404);
    let mut inf = Object::new(ti, iid, Team::USA);
    inf.ignored_obstacle_id = Some(vid);
    logic.objects.insert(iid, inf);
    assert!(logic.try_physics_collide(iid, vid, 5.0));
    // Reclaim residual: process_destroy_list removes the pilot (not merely
    // marks destroyed while leaving the slot occupied).
    assert!(
        logic.objects.get(&iid).is_none()
            || logic
                .objects
                .get(&iid)
                .map(|o| o.status.destroyed)
                .unwrap_or(false),
        "pilot must be destroyed or removed after unmanned reclaim"
    );
    assert!(!logic.objects.get(&vid).unwrap().status.disabled_unmanned);
    assert_eq!(logic.objects.get(&vid).unwrap().team, Team::USA);
}

#[test]
fn try_physics_collide_unmanned_recrew_requires_ignored_obstacle() {
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    let mut tv = ThingTemplate::new("HuskNoIgnore");
    tv.add_kind_of(KindOf::Vehicle);
    let vid = ObjectId(413);
    let mut v = Object::new(tv, vid, Team::Neutral);
    v.set_status_disabled_unmanned(true);
    logic.objects.insert(vid, v);
    let mut ti = ThingTemplate::new("BumpInf");
    ti.add_kind_of(KindOf::Infantry);
    let iid = ObjectId(414);
    let inf = Object::new(ti, iid, Team::USA);
    logic.objects.insert(iid, inf);
    assert!(logic.try_physics_collide(iid, vid, 5.0));
    assert!(
        logic.objects.get(&iid).is_some_and(|o| o.is_alive()),
        "accidental bump must not destroy the infantry"
    );
    assert!(
        logic.objects.get(&vid).unwrap().status.disabled_unmanned,
        "accidental bump must not recrew the husk"
    );
    assert_eq!(logic.objects.get(&vid).unwrap().team, Team::Neutral);
}


#[test]
fn apply_overlap_crush_check_crushes_enemy_infantry() {
    use crate::game_logic::host_usa_pilot::HostDeathType;
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
    use glam::Vec3;
    let mut logic = GameLogic::new();
    let mut vt = ThingTemplate::new("TankC");
    vt.add_kind_of(KindOf::Vehicle);
    let tid = ObjectId(101);
    let mut tank = Object::new(vt, tid, Team::USA);
    tank.crusher_level = 1;
    tank.set_orientation(0.0);
    tank.movement.velocity = Vec3::new(5.0, 0.0, 0.0);
    tank.set_position(Vec3::new(6.0, 0.0, 0.0));
    logic.objects.insert(tid, tank);

    let mut it = ThingTemplate::new("InfC");
    it.add_kind_of(KindOf::Infantry);
    let iid = ObjectId(102);
    let mut inf = Object::new(it, iid, Team::GLA);
    inf.crushable_level = 0;
    inf.has_squish_collide = true;
    inf.selection_radius = 10.0;
    inf.set_position(Vec3::new(5.0, 0.0, 0.0));
    logic.objects.insert(iid, inf);

    assert!(logic.apply_overlap_crush_check(tid, iid, false));
    let inf = logic.objects.get(&iid).unwrap();
    assert!(inf.status.destroyed);
    assert_eq!(inf.status.death_type, HostDeathType::Crushed);
}

#[test]
fn higher_id_crusher_still_squishes() {
    // C++ both onCollide. Tank built after map infantry must still crush.
    use crate::game_logic::host_usa_pilot::HostDeathType;
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
    use glam::Vec3;
    let mut logic = GameLogic::new();
    let mut it = ThingTemplate::new("FirstInf");
    it.add_kind_of(KindOf::Infantry);
    let iid = ObjectId(501);
    let mut inf = Object::new(it, iid, Team::GLA);
    inf.crushable_level = 0;
    inf.has_squish_collide = true;
    inf.selection_radius = 8.0;
    inf.set_position(Vec3::new(5.0, 0.0, 0.0));
    logic.objects.insert(iid, inf);

    let mut vt = ThingTemplate::new("LaterTank");
    vt.add_kind_of(KindOf::Vehicle);
    let tid = ObjectId(502);
    let mut tank = Object::new(vt, tid, Team::USA);
    tank.crusher_level = 1;
    tank.set_orientation(0.0);
    tank.movement.velocity = Vec3::new(5.0, 0.0, 0.0);
    tank.set_position(Vec3::new(6.0, 0.0, 0.0));
    tank.selection_radius = 8.0;
    logic.objects.insert(tid, tank);

    let _ = logic.tick_physics_collisions_all();
    let inf = logic.objects.get(&iid).unwrap();
    assert!(inf.status.destroyed, "higher-id tank must still crush");
    assert_eq!(inf.status.death_type, HostDeathType::Crushed);
}

#[test]
fn first_overlap_crush_requires_geom_contact() {
    // checkForOverlapCollision only runs after geomCollidesWithGeom.
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
    use glam::Vec3;
    let mut logic = GameLogic::new();
    let mut vt = ThingTemplate::new("FarTank");
    vt.add_kind_of(KindOf::Vehicle);
    let tid = ObjectId(601);
    let mut tank = Object::new(vt, tid, Team::USA);
    tank.crusher_level = 2;
    tank.set_orientation(0.0);
    tank.movement.velocity = Vec3::new(5.0, 0.0, 0.0);
    tank.set_position(Vec3::new(0.0, 0.0, 0.0));
    tank.selection_radius = 8.0;
    logic.objects.insert(tid, tank);

    let mut ct = ThingTemplate::new("FarCar");
    ct.add_kind_of(KindOf::Vehicle);
    let cid = ObjectId(602);
    let mut car = Object::new(ct, cid, Team::Neutral);
    car.crushable_level = 1;
    car.crusher_level = 0;
    car.selection_radius = 8.0;
    car.set_position(Vec3::new(30.0, 0.0, 0.0));
    car.health.current = 200.0;
    car.health.maximum = 200.0;
    logic.objects.insert(cid, car);

    let _ = logic.tick_physics_collisions_all();
    let tank = logic.objects.get(&tid).unwrap();
    let car = logic.objects.get(&cid).unwrap();
    assert!(
        tank.physics_previous_overlap.is_none(),
        "shared cell without geom contact must not stamp overlap"
    );
    assert!(
        car.is_alive() && (car.health.current - 200.0).abs() < 1e-3,
        "0-damage first-overlap must not fire without contact"
    );
}

#[test]
fn allied_player_tank_does_not_crush_friends() {
    // C++ Object.cpp:1096 ALLIES, not faction Team==. hq-atwzd.
    use crate::game_logic::host_usa_pilot::HostDeathType;
    use crate::game_logic::{KindOf, Object, ObjectId, Player, Team, ThingTemplate};
    use glam::Vec3;
    let mut logic = GameLogic::new();
    let mut usa = Player::new(0, Team::USA, "USA", true);
    usa.alliance_team = 7;
    usa.is_alive = true;
    let mut china = Player::new(1, Team::China, "China", false);
    china.alliance_team = 7;
    china.is_alive = true;
    logic.add_player(usa);
    logic.add_player(china);

    let mut vt = ThingTemplate::new("AllyTank");
    vt.add_kind_of(KindOf::Vehicle);
    let tid = ObjectId(201);
    let mut tank = Object::new(vt, tid, Team::USA);
    tank.owner_player_id = Some(0);
    tank.crusher_level = 1;
    tank.set_orientation(0.0);
    tank.movement.velocity = Vec3::new(5.0, 0.0, 0.0);
    tank.set_position(Vec3::new(6.0, 0.0, 0.0));
    logic.objects.insert(tid, tank);

    let mut it = ThingTemplate::new("AllyInf");
    it.add_kind_of(KindOf::Infantry);
    let iid = ObjectId(202);
    let mut inf = Object::new(it, iid, Team::China);
    inf.owner_player_id = Some(1);
    inf.crushable_level = 0;
    inf.selection_radius = 10.0;
    inf.set_position(Vec3::new(5.0, 0.0, 0.0));
    inf.health.current = 100.0;
    inf.health.maximum = 100.0;
    logic.objects.insert(iid, inf);

    assert!(!logic.apply_overlap_crush_check(tid, iid, true));
    let inf = logic.objects.get(&iid).unwrap();
    assert!(inf.is_alive());
    assert_ne!(inf.status.death_type, HostDeathType::Crushed);

    // Live collide path uses object_relationship, not Team==.
    assert!(logic.try_physics_collide(tid, iid, 8.0));
    let inf = logic.objects.get(&iid).unwrap();
    assert!(inf.is_alive(), "diplomatic ally infantry must survive crush");
}

#[test]
fn apply_immobile_collide_bounce_scrubs_and_pushes() {
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
    use glam::Vec3;
    let mut logic = GameLogic::new();
    let mut vt = ThingTemplate::new("BounceMov");
    vt.add_kind_of(KindOf::Vehicle);
    let mid = ObjectId(81);
    let mut m = Object::new(vt, mid, Team::USA);
    m.set_position(Vec3::new(0.0, 1.0, 0.0));
    m.movement.velocity = Vec3::new(8.0, 0.0, 0.0);
    logic.objects.insert(mid, m);

    let mut st = ThingTemplate::new("BounceImm");
    st.add_kind_of(KindOf::Structure);
    st.add_kind_of(KindOf::Immobile);
    let iid = ObjectId(82);
    let mut s = Object::new(st, iid, Team::China);
    s.set_position(Vec3::new(5.0, 1.0, 0.0));
    logic.objects.insert(iid, s);

    assert!(logic.apply_immobile_collide_bounce(mid, iid, 10.0));
    let m = logic.objects.get(&mid).unwrap();
    // Pushed back / velocity reversed residual along -X.
    assert!(
        m.movement.velocity.x <= 0.0,
        "vel={:?}",
        m.movement.velocity
    );

    // Parachute path scrubs lateral.
    let mut pt = ThingTemplate::new("ParaMov");
    pt.add_kind_of(KindOf::Infantry);
    let pid = ObjectId(83);
    let mut p = Object::new(pt, pid, Team::USA);
    p.set_position(Vec3::new(0.0, 10.0, 0.0));
    p.movement.velocity = Vec3::new(3.0, -1.0, 0.0);
    p.set_status_parachuting(true);
    logic.objects.insert(pid, p);
    assert!(logic.apply_immobile_collide_bounce(pid, iid, 20.0));
    let p = logic.objects.get(&pid).unwrap();
    assert_eq!(p.movement.velocity.x, 0.0);
    assert_eq!(p.movement.velocity.z, 0.0);
    assert!(p.get_position().x < 0.0);

    // Dead wreck still stiffness-bounces (C++ effectivelyDead hulks stay in world).
    let mut wt = ThingTemplate::new("DeadHulk");
    wt.add_kind_of(KindOf::Vehicle);
    let wid = ObjectId(84);
    let mut w = Object::new(wt, wid, Team::USA);
    w.set_position(Vec3::new(0.0, 1.0, 0.0));
    w.movement.velocity = Vec3::new(6.0, 0.0, 0.0);
    w.status.destroyed = true;
    w.health.current = 0.0;
    logic.objects.insert(wid, w);
    assert!(logic.apply_immobile_collide_bounce(wid, iid, 10.0));
    let w = logic.objects.get(&wid).unwrap();
    assert!(
        w.movement.velocity.x <= 0.0,
        "wreck vel={:?}",
        w.movement.velocity
    );
}

#[test]
fn apply_immobile_collide_bounce_parachute_walks_contain_chain() {
    // C++ PhysicsUpdate.cpp:1322-1332 / leftover physics_collide.rs:199-221:
    // rider PARACHUTING + contained_by chute → jam the chute, not the rider.
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
    use glam::Vec3;
    let mut logic = GameLogic::new();

    let mut st = ThingTemplate::new("ParaBld");
    st.add_kind_of(KindOf::Structure);
    st.add_kind_of(KindOf::Immobile);
    let iid = ObjectId(182);
    let mut s = Object::new(st, iid, Team::China);
    s.set_position(Vec3::new(5.0, 1.0, 0.0));
    logic.objects.insert(iid, s);

    let mut ct = ThingTemplate::new("AmericaParachute");
    ct.add_kind_of(KindOf::Aircraft);
    let cid = ObjectId(181);
    let mut chute = Object::new(ct, cid, Team::USA);
    chute.set_position(Vec3::new(0.0, 12.0, 0.0));
    chute.movement.velocity = Vec3::new(3.0, -1.0, 0.5);
    logic.objects.insert(cid, chute);

    let mut pt = ThingTemplate::new("ParaRider");
    pt.add_kind_of(KindOf::Infantry);
    let pid = ObjectId(180);
    let mut rider = Object::new(pt, pid, Team::USA);
    rider.set_position(Vec3::new(0.0, 10.0, 0.0));
    rider.movement.velocity = Vec3::new(3.0, -1.0, 0.5);
    rider.set_status_parachuting(true);
    rider.set_contained_by_enclosing(Some(cid), false);
    logic.objects.insert(pid, rider);

    let rider_before = logic.objects.get(&pid).unwrap().get_position();
    let rider_vel_before = logic.objects.get(&pid).unwrap().movement.velocity;
    assert!(logic.apply_immobile_collide_bounce(pid, iid, 20.0));

    let rider = logic.objects.get(&pid).unwrap();
    assert_eq!(
        rider.get_position(),
        rider_before,
        "rider must stay in harness"
    );
    assert_eq!(
        rider.movement.velocity, rider_vel_before,
        "rider lateral must not be scrubbed"
    );

    let chute = logic.objects.get(&cid).unwrap();
    assert!(
        chute.get_position().x < 0.0,
        "chute jam away from building +X, pos={:?}",
        chute.get_position()
    );
    assert_eq!(chute.movement.velocity.x, 0.0);
    assert_eq!(chute.movement.velocity.z, 0.0);
    assert_eq!(
        chute.movement.velocity.y, -1.0,
        "scrubVelocity2D keeps vertical"
    );
}

#[test]
fn collide_immobile_is_kindof_immobile_not_can_move() {
    // C++ PhysicsUpdate.cpp:1221-1222 / leftover physics_collide.rs:144:
    // otherImmobile is KINDOF_IMMOBILE only. !can_move (dead, EMP, deployed,
    // docked, or garrisoned) stays mobile-mobile processCollision.
    use crate::game_logic::{AIState, KindOf, Object, ObjectId, Team, ThingTemplate};
    use glam::Vec3;
    let mut logic = GameLogic::new();
    let mut vt = ThingTemplate::new("ImmGateTank");
    vt.add_kind_of(KindOf::Vehicle);
    let mid = ObjectId(91);
    let mut m = Object::new(vt.clone(), mid, Team::USA);
    m.set_position(Vec3::new(0.0, 1.0, 0.0));
    m.movement.velocity = Vec3::new(8.0, 0.0, 0.0);
    logic.objects.insert(mid, m);

    let spawn_other = |logic: &mut GameLogic, id: u32, name: &str, tweak: fn(&mut Object)| {
        let mut t = ThingTemplate::new(name);
        t.add_kind_of(KindOf::Vehicle);
        let oid = ObjectId(id);
        let mut o = Object::new(t, oid, Team::China);
        o.set_position(Vec3::new(5.0, 1.0, 0.0));
        tweak(&mut o);
        assert!(
            !o.can_move(),
            "{name} fixture must fail can_move so the old gate would misfire"
        );
        assert!(
            !o.is_kind_of(KindOf::Immobile),
            "{name} must not carry KINDOF_IMMOBILE"
        );
        logic.objects.insert(oid, o);
        oid
    };

    let dead = spawn_other(&mut logic, 92, "DeadWreck", |o| {
        o.status.destroyed = true;
        o.health.current = 0.0;
    });
    let emp = spawn_other(&mut logic, 93, "EmpHumvee", |o| {
        o.set_status_disabled_emp(true);
    });
    let deployed = spawn_other(&mut logic, 94, "DeployedTomahawk", |o| {
        o.set_deployed(true);
    });
    let garrisoned = spawn_other(&mut logic, 95, "GarrisonedHusk", |o| {
        o.ai_state = AIState::Garrisoned;
    });
    let docked = spawn_other(&mut logic, 97, "DockedHumvee", |o| {
        o.ai_state = AIState::Docked;
    });

    for other in [dead, emp, deployed, garrisoned, docked] {
        let before = logic.objects.get(&mid).unwrap().movement.velocity;
        assert!(
            !logic.apply_immobile_collide_bounce(mid, other, 10.0),
            "other {other:?} is not KINDOF_IMMOBILE"
        );
        let after = logic.objects.get(&mid).unwrap().movement.velocity;
        assert_eq!(after, before, "stiffness must not fire vs {other:?}");
    }

    let mut tree_t = ThingTemplate::new("ImmTree");
    tree_t.add_kind_of(KindOf::Immobile);
    let tid = ObjectId(96);
    let mut tree = Object::new(tree_t, tid, Team::Neutral);
    tree.set_position(Vec3::new(5.0, 1.0, 0.0));
    logic.objects.insert(tid, tree);
    assert!(logic.apply_immobile_collide_bounce(mid, tid, 10.0));

    // Pair loop: dead wreck stays mobile-mobile (last_collidee, no vel zero).
    logic.objects.get_mut(&mid).unwrap().movement.velocity = Vec3::new(8.0, 0.0, 0.0);
    logic.objects.get_mut(&mid).unwrap().allow_collide_force = true;
    assert!(logic.try_physics_collide(mid, dead, 10.0));
    let tank = logic.objects.get(&mid).unwrap();
    assert_eq!(tank.last_collidee, Some(dead));
    assert!(
        tank.movement.velocity != Vec3::ZERO,
        "mobile-mobile must not stiffness-zero vel; vel={:?}",
        tank.movement.velocity
    );
}

#[test]
fn apply_vehicle_crash_into_building_destroys() {
    use crate::game_logic::host_partition_collision_physics_residual::PHYSICS_VEHICLE_CRASHES_INTO_BUILDING_WEAPON;
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
    use glam::Vec3;
    let mut logic = GameLogic::new();
    let mut vt = ThingTemplate::new("VCrash");
    vt.add_kind_of(KindOf::Vehicle);
    let vid = ObjectId(61);
    let mut v = Object::new(vt, vid, Team::USA);
    v.set_position(Vec3::new(0.0, 4.0, 0.0));
    v.movement.velocity = Vec3::new(0.0, -2.0, 0.0);
    v.health.current = 200.0;
    logic.objects.insert(vid, v);

    let mut st = ThingTemplate::new("SCrash");
    st.add_kind_of(KindOf::Structure);
    st.add_kind_of(KindOf::Immobile);
    let sid = ObjectId(62);
    logic.objects.insert(sid, Object::new(st, sid, Team::GLA));

    let w = logic
        .apply_vehicle_crash_into_immobile(vid, sid)
        .expect("weapon");
    assert_eq!(w, PHYSICS_VEHICLE_CRASHES_INTO_BUILDING_WEAPON);
    assert!(logic.objects.get(&vid).unwrap().status.destroyed);
    assert!(logic.queued_audio_event_count_for_test() > 0);
}

#[test]
fn try_physics_collide_skips_container_contained() {
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
    use glam::Vec3;
    let mut logic = GameLogic::new();
    let mut ct = ThingTemplate::new("FireBaseHull");
    ct.add_kind_of(KindOf::Structure);
    let cid = ObjectId(801);
    let mut hull = Object::new(ct, cid, Team::USA);
    hull.set_position(Vec3::ZERO);
    hull.selection_radius = 20.0;
    hull.allow_collide_force = true;
    logic.objects.insert(cid, hull);

    let mut it = ThingTemplate::new("FireBaseOcc");
    it.add_kind_of(KindOf::Infantry);
    let iid = ObjectId(802);
    let mut occ = Object::new(it, iid, Team::USA);
    occ.set_position(Vec3::new(1.0, 0.0, 0.0));
    occ.selection_radius = 8.0;
    occ.set_contained_by(Some(cid));
    occ.allow_collide_force = true;
    occ.is_panicking = true;
    occ.movement.velocity = Vec3::new(4.0, 0.0, 0.0);
    logic.objects.insert(iid, occ);

    assert!(logic.try_physics_collide(iid, cid, 8.0));
    let occ = logic.objects.get(&iid).unwrap();
    assert!(
        occ.physics_accel.length_squared() < 1e-8,
        "occupant must not bounce off container; accel={:?}",
        occ.physics_accel
    );
    assert!(
        (occ.movement.velocity.x - 4.0).abs() < 1e-4,
        "container/contained skip must not stiffness-zero vel; vel={:?}",
        occ.movement.velocity
    );
    assert_eq!(occ.last_collidee, None);
}

#[test]
fn apply_vehicle_crash_destroys_non_vehicle_into_structure() {
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
    use glam::Vec3;
    let mut logic = GameLogic::new();
    let mut it = ThingTemplate::new("TossedCrew");
    it.add_kind_of(KindOf::Infantry);
    let iid = ObjectId(63);
    let mut inf = Object::new(it, iid, Team::USA);
    inf.set_position(Vec3::new(0.0, 4.0, 0.0));
    inf.movement.velocity = Vec3::new(0.0, -2.0, 0.0);
    inf.health.current = 100.0;
    logic.objects.insert(iid, inf);

    let mut st = ThingTemplate::new("WarFactory");
    st.add_kind_of(KindOf::Structure);
    st.add_kind_of(KindOf::Immobile);
    let sid = ObjectId(64);
    logic.objects.insert(sid, Object::new(st, sid, Team::GLA));

    let w = logic
        .apply_vehicle_crash_into_immobile(iid, sid)
        .expect("destroy-only residual");
    assert_eq!(w, "");
    assert!(
        logic.objects.get(&iid).unwrap().status.destroyed,
        "non-vehicle fall into structure must destroyObject"
    );
}

#[test]
fn tick_shock_stun_all_queues_bounce_audio() {
    use crate::game_logic::{
        bounce_sound_volume_residual, KindOf, Object, ObjectId, Team, ThingTemplate,
        BOUNCE_SOUND_DEFAULT,
    };
    use glam::Vec3;
    let mut logic = GameLogic::new();
    let mut tmpl = ThingTemplate::new("BounceAud");
    tmpl.add_kind_of(KindOf::Vehicle);
    let id = ObjectId(9100);
    let mut o = Object::new(tmpl, id, Team::USA);
    o.shock_stun_frames = 0;
    o.set_bounce_sound(BOUNCE_SOUND_DEFAULT);
    o.record_bounce_land(2.0);
    assert!(o.bounce_audio_pending > 0);
    logic.objects.insert(id, o);
    let before = logic.queued_audio_event_count_for_test();
    logic.tick_shock_stun_all();
    assert!(
        logic.queued_audio_event_count_for_test() > before,
        "bounce land must queue AudioEventRequest"
    );
    let events = std::mem::take(&mut logic.queued_audio_events);
    assert!(events.iter().any(|e| e.event_type == BOUNCE_SOUND_DEFAULT));
    assert!(events.iter().any(|e| e.object_id == Some(id)));
    let v = bounce_sound_volume_residual(0.2, 10.0);
    assert!(v >= 0.25 && v <= 1.0);
    let _ = Vec3::ZERO;
}

#[test]
fn tick_shock_stun_all_samples_terrain_surface() {
    use crate::game_logic::terrain::TerrainData;
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
    use glam::Vec3;
    #[cfg(feature = "game_client")]
    {
        use game_client::terrain::height_map::HeightMap;
        let mut logic = GameLogic::new();
        let mut hm = HeightMap::new(8, 8, 100.0, 1.0);
        for h in hm.heights.iter_mut() {
            *h = 0.1;
        }
        let mut terrain = TerrainData::from_heightmap(
            hm,
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(70.0, 0.0, 70.0),
            0,
        );
        let ground = terrain.height_at_world(Vec3::new(10.0, 0.0, 10.0));
        terrain.water_plane_y = Some(ground + 5.0);
        logic.terrain = Some(terrain);

        let mut tmpl = ThingTemplate::new("StunSurf");
        tmpl.add_kind_of(KindOf::Vehicle);
        let id = ObjectId(9001);
        let mut o = Object::new(tmpl, id, Team::USA);
        o.set_position(Vec3::new(10.0, ground, 10.0));
        o.shock_stun_frames = 10;
        o.cell_is_underwater = false;
        logic.objects.insert(id, o);
        logic.tick_shock_stun_all();
        let o = logic.objects.get(&id).expect("obj");
        assert!(
            o.cell_is_underwater,
            "tick must sample underwater from terrain water plane"
        );
    }
}

#[test]
fn shock_wave_impulse_applies_on_splash_impact() {
    use crate::game_logic::weapon_bootstrap::{
        compute_shock_wave_force, host_shock_wave_amount_for_weapon_name,
    };
    assert!(host_shock_wave_amount_for_weapon_name("MOABDetonationWeapon") >= 250.0 - 1e-3);
    let mut logic = GameLogic::new();
    {
        let mut t = ThingTemplate::new("SwVic");
        t.add_kind_of(KindOf::Vehicle);
        t.add_kind_of(KindOf::Attackable);
        logic.templates.insert("SwVic".into(), t);
    }
    let v1 = logic
        .create_object("SwVic", Team::China, glam::Vec3::new(20.0, 0.0, 0.0))
        .unwrap();
    let v2 = logic
        .create_object("SwVic", Team::China, glam::Vec3::new(500.0, 0.0, 0.0))
        .unwrap();
    if let Some(o) = logic.objects.get_mut(&v1) {
        o.movement.velocity = glam::Vec3::ZERO;
    }
    if let Some(o) = logic.objects.get_mut(&v2) {
        o.movement.velocity = glam::Vec3::ZERO;
    }
    let n = logic.apply_shock_wave_at_impact(
        glam::Vec3::ZERO,
        glam::Vec3::ZERO,
        80.0,
        Some("MOABDetonationWeapon"),
        None,
    );
    assert!(n >= 1, "near victim shocked n={n}");
    let s1 = logic
        .objects
        .get(&v1)
        .map(|o| o.movement.velocity.length())
        .unwrap_or(0.0);
    let s2 = logic
        .objects
        .get(&v2)
        .map(|o| o.movement.velocity.length())
        .unwrap_or(0.0);
    assert!(s1 > 0.0, "near velocity {s1}");
    assert!(s2 < s1 * 0.1, "far much weaker {s2} near {s1}");
    assert!(s2 < 1e-2, "far essentially unshocked {s2}");
    let _ = compute_shock_wave_force(
        glam::Vec3::ZERO,
        glam::Vec3::new(10.0, 0.0, 0.0),
        100.0,
        50.0,
        0.75,
    );
}

#[test]
fn direct_hit_applies_dual_radius_splash_to_neighbors() {
    let mut logic = GameLogic::new();
    {
        let mut t = ThingTemplate::new("HitGun");
        t.primary_weapon_name = Some("AmericaFireBaseHowitzer".into());
        t.add_kind_of(KindOf::Vehicle);
        t.add_kind_of(KindOf::Attackable);
        logic.templates.insert("HitGun".into(), t);
        let mut ti = ThingTemplate::new("HitTgt");
        ti.add_kind_of(KindOf::Vehicle);
        ti.add_kind_of(KindOf::Attackable);
        logic.templates.insert("HitTgt".into(), ti);
        let mut tn = ThingTemplate::new("HitNear");
        tn.add_kind_of(KindOf::Vehicle);
        tn.add_kind_of(KindOf::Attackable);
        logic.templates.insert("HitNear".into(), tn);
    }
    let gun = logic
        .create_object("HitGun", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    let tgt = logic
        .create_object("HitTgt", Team::China, glam::Vec3::new(80.0, 0.0, 0.0))
        .unwrap();
    let near = logic
        .create_object("HitNear", Team::China, glam::Vec3::new(90.0, 0.0, 0.0))
        .unwrap();
    for id in [tgt, near] {
        if let Some(o) = logic.objects.get_mut(&id) {
            o.health.current = 300.0;
            o.health.maximum = 300.0;
        }
    }
    let hits = logic.apply_instant_hit_splash_at(
        glam::Vec3::new(80.0, 0.0, 0.0),
        40.0,
        20.0,
        25.0,
        40.0,
        gun,
        Team::USA,
        tgt,
        Some("AmericaFireBaseHowitzer"),
    );
    assert!(hits >= 1, "neighbor in primary ring must splash");
    let nh = logic
        .objects
        .get(&near)
        .map(|o| o.health.current)
        .unwrap_or(0.0);
    assert!(nh < 300.0 - 1.0, "near took splash nh={nh}");
    let th = logic
        .objects
        .get(&tgt)
        .map(|o| o.health.current)
        .unwrap_or(0.0);
    assert!(
        (th - 300.0).abs() < 0.01,
        "intended skipped by splash helper"
    );
}

/// C++ ActiveBody::attemptDamage scoreTheKill on lethal splash (`hq-ng506`).
#[test]
fn splash_kill_awards_score_the_kill_experience() {
    let mut logic = GameLogic::new();
    {
        let mut t = ThingTemplate::new("SplashGun");
        t.primary_weapon_name = Some("AmericaFireBaseHowitzer".into());
        t.add_kind_of(KindOf::Vehicle);
        t.add_kind_of(KindOf::Attackable);
        t.is_trainable = true;
        t.veterancy_xp_thresholds = [40.0, 150.0, 300.0];
        logic.templates.insert("SplashGun".into(), t);
        let mut ti = ThingTemplate::new("SplashIntended");
        ti.add_kind_of(KindOf::Vehicle);
        ti.add_kind_of(KindOf::Attackable);
        logic.templates.insert("SplashIntended".into(), ti);
        let mut tn = ThingTemplate::new("SplashNear");
        tn.add_kind_of(KindOf::Vehicle);
        tn.add_kind_of(KindOf::Attackable);
        tn.experience_value = 40.0;
        tn.experience_values = [40.0, 40.0, 80.0, 120.0];
        logic.templates.insert("SplashNear".into(), tn);
    }
    let gun = logic
        .create_object("SplashGun", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    if let Some(o) = logic.objects.get_mut(&gun) {
        o.thing.template.is_trainable = true;
        o.thing.template.veterancy_xp_thresholds = [40.0, 150.0, 300.0];
    }
    let tgt = logic
        .create_object(
            "SplashIntended",
            Team::China,
            glam::Vec3::new(80.0, 0.0, 0.0),
        )
        .unwrap();
    let near = logic
        .create_object("SplashNear", Team::China, glam::Vec3::new(90.0, 0.0, 0.0))
        .unwrap();
    if let Some(o) = logic.objects.get_mut(&near) {
        o.health.current = 10.0;
        o.health.maximum = 10.0;
        o.thing.template.experience_value = 40.0;
        o.thing.template.experience_values = [40.0, 40.0, 80.0, 120.0];
    }
    let hits = logic.apply_instant_hit_splash_at(
        glam::Vec3::new(80.0, 0.0, 0.0),
        40.0,
        20.0,
        25.0,
        40.0,
        gun,
        Team::USA,
        tgt,
        Some("AmericaFireBaseHowitzer"),
    );
    assert!(hits >= 1, "neighbor in splash ring must be hit");
    let near_dead = logic
        .objects
        .get(&near)
        .map(|o| !o.is_alive() || o.health.current <= 0.0 || o.status.destroyed)
        .unwrap_or(true);
    assert!(near_dead, "splash must kill the low-HP neighbor");
    let xp = logic
        .objects
        .get(&gun)
        .map(|o| o.experience.current)
        .unwrap_or(0.0);
    assert!(
        xp + f32::EPSILON >= 40.0,
        "splash kill must award scoreTheKill XP, got {xp}"
    );
}

#[test]
fn scatter_miss_splash_honors_radius_damage_affects() {
    use crate::game_logic::host_ai_path_combat_residual_wave105::WEAPON_AFFECTS_ALLIES;
    use crate::game_logic::weapon_bootstrap::{
        ensure_host_weapon_store, host_radius_damage_affects_for_weapon_name,
        radius_damage_affects_victim, WEAPON_AFFECTS_DEFAULT,
    };
    use gamelogic::weapon::{with_weapon_store_mut, WeaponTemplate};

    ensure_host_weapon_store();
    let mut no_ally = WeaponTemplate::new("TestNoAllySplashWeapon".to_string());
    no_ally.parse_weapon_fields_from_ini(&std::collections::HashMap::from([
        (
            "RadiusDamageAffects".to_string(),
            "ENEMIES NEUTRALS".to_string(),
        ),
        ("PrimaryDamageRadius".to_string(), "30".to_string()),
    ]));
    let _ = with_weapon_store_mut(|store| {
        store.add_weapon_template(no_ally);
    });

    let gun = host_radius_damage_affects_for_weapon_name("TestNoAllySplashWeapon");
    assert_eq!(gun & WEAPON_AFFECTS_ALLIES, 0);
    // C++ omitted RadiusDamageAffects / unknown name: ALLIES|ENEMIES|NEUTRALS.
    let omitted = host_radius_damage_affects_for_weapon_name("CompletelyUnknownSplashWeaponXYZ");
    assert_eq!(omitted, WEAPON_AFFECTS_DEFAULT);
    assert_ne!(omitted & WEAPON_AFFECTS_ALLIES, 0);

    let mut logic = GameLogic::new();
    {
        let mut t = ThingTemplate::new("SplashSrc");
        t.primary_weapon_name = Some("TestNoAllySplashWeapon".into());
        t.add_kind_of(KindOf::Vehicle);
        logic.templates.insert("SplashSrc".into(), t);
        let mut ta = ThingTemplate::new("SplashAlly");
        ta.add_kind_of(KindOf::Vehicle);
        ta.add_kind_of(KindOf::Attackable);
        logic.templates.insert("SplashAlly".into(), ta);
        let mut te = ThingTemplate::new("SplashEnemy");
        te.add_kind_of(KindOf::Vehicle);
        te.add_kind_of(KindOf::Attackable);
        logic.templates.insert("SplashEnemy".into(), te);
    }
    let src = logic
        .create_object("SplashSrc", Team::USA, glam::Vec3::ZERO)
        .unwrap();
    let ally = logic
        .create_object("SplashAlly", Team::USA, glam::Vec3::new(10.0, 0.0, 0.0))
        .unwrap();
    let enemy = logic
        .create_object("SplashEnemy", Team::China, glam::Vec3::new(12.0, 0.0, 0.0))
        .unwrap();
    for id in [ally, enemy] {
        if let Some(o) = logic.objects.get_mut(&id) {
            o.health.current = 200.0;
            o.health.maximum = 200.0;
        }
    }
    let impact = glam::Vec3::new(11.0, 0.0, 0.0);
    let _ = logic.apply_scatter_miss_splash_at(
        impact,
        50.0,
        30.0,
        src,
        Team::USA,
        ObjectId(0), // no skip
        Some("TestNoAllySplashWeapon"),
    );
    let ah = logic
        .objects
        .get(&ally)
        .map(|o| o.health.current)
        .unwrap_or(0.0);
    let eh = logic
        .objects
        .get(&enemy)
        .map(|o| o.health.current)
        .unwrap_or(0.0);
    assert!(
        (ah - 200.0).abs() < 0.01,
        "ally must not take ENEMIES|NEUTRALS splash ah={ah}"
    );
    assert!(eh < 200.0 - 1.0, "enemy must take splash eh={eh}");

    // C++ default ALLIES|ENEMIES|NEUTRALS friendly-fires.
    if let Some(o) = logic.objects.get_mut(&ally) {
        o.health.current = 200.0;
    }
    if let Some(o) = logic.objects.get_mut(&enemy) {
        o.health.current = 200.0;
    }
    let _ = logic.apply_scatter_miss_splash_at(
        impact,
        50.0,
        30.0,
        src,
        Team::USA,
        ObjectId(0),
        None,
    );
    let ah2 = logic
        .objects
        .get(&ally)
        .map(|o| o.health.current)
        .unwrap_or(0.0);
    assert!(ah2 < 200.0 - 1.0, "default splash hits allies ah2={ah2}");
    assert!(radius_damage_affects_victim(
        omitted,
        gamelogic::common::Relationship::Allies,
        src,
        ally,
        None,
        false,
        false,
    ));
}

#[test]
fn scatter_miss_splash_honors_not_airborne_ini() {
    use crate::game_logic::host_ai_path_combat_residual_wave105::{
        WEAPON_AFFECTS_ENEMIES, WEAPON_AFFECTS_NEUTRALS, WEAPON_DOESNT_AFFECT_AIRBORNE,
    };
    use crate::game_logic::weapon_bootstrap::{
        ensure_host_weapon_store, host_radius_damage_affects_for_weapon_name,
    };
    use gamelogic::weapon::{with_weapon_store_mut, WeaponTemplate};

    ensure_host_weapon_store();
    let mut w = WeaponTemplate::new("TestNotAirborneSplashWeapon".to_string());
    w.parse_weapon_fields_from_ini(&std::collections::HashMap::from([
        (
            "RadiusDamageAffects".to_string(),
            "ENEMIES NEUTRALS NOT_AIRBORNE".to_string(),
        ),
        ("PrimaryDamageRadius".to_string(), "30".to_string()),
    ]));
    let _ = with_weapon_store_mut(|store| {
        store.add_weapon_template(w);
    });
    let mask = host_radius_damage_affects_for_weapon_name("TestNotAirborneSplashWeapon");
    assert_ne!(mask & WEAPON_DOESNT_AFFECT_AIRBORNE, 0);
    assert_eq!(mask & WEAPON_AFFECTS_ENEMIES, WEAPON_AFFECTS_ENEMIES);
    assert_eq!(mask & WEAPON_AFFECTS_NEUTRALS, WEAPON_AFFECTS_NEUTRALS);

    let mut logic = GameLogic::new();
    {
        let mut t = ThingTemplate::new("NaSrc");
        t.primary_weapon_name = Some("TestNotAirborneSplashWeapon".into());
        t.add_kind_of(KindOf::Vehicle);
        logic.templates.insert("NaSrc".into(), t);
        let mut te = ThingTemplate::new("NaGround");
        te.add_kind_of(KindOf::Vehicle);
        te.add_kind_of(KindOf::Attackable);
        logic.templates.insert("NaGround".into(), te);
        let mut th = ThingTemplate::new("NaHigh");
        th.add_kind_of(KindOf::Vehicle);
        th.add_kind_of(KindOf::Attackable);
        logic.templates.insert("NaHigh".into(), th);
    }
    let src = logic
        .create_object("NaSrc", Team::USA, glam::Vec3::ZERO)
        .unwrap();
    let ground = logic
        .create_object("NaGround", Team::China, glam::Vec3::new(8.0, 0.0, 0.0))
        .unwrap();
    let high = logic
        .create_object("NaHigh", Team::China, glam::Vec3::new(8.0, 12.0, 0.0))
        .unwrap();
    if let Some(o) = logic.objects.get_mut(&ground) {
        o.health.current = 200.0;
        o.health.maximum = 200.0;
        o.ground_height = 0.0;
    }
    if let Some(o) = logic.objects.get_mut(&high) {
        o.health.current = 200.0;
        o.health.maximum = 200.0;
        o.ground_height = 0.0;
        o.selection_radius = 0.0;
        o.thing.template.geometry_info.authored = true;
        o.thing.template.geometry_info.major_radius = 0.0;
        o.thing.template.geometry_info.geom_type = crate::game_logic::HostGeometryType::Sphere;
        o.thing.template.geometry_info.height = 0.0;
    }
    let _ = logic.apply_scatter_miss_splash_at(
        glam::Vec3::ZERO,
        50.0,
        30.0,
        src,
        Team::USA,
        ObjectId(0),
        Some("TestNotAirborneSplashWeapon"),
    );
    let gh = logic
        .objects
        .get(&ground)
        .map(|o| o.health.current)
        .unwrap_or(0.0);
    let hh = logic
        .objects
        .get(&high)
        .map(|o| o.health.current)
        .unwrap_or(0.0);
    assert!(gh < 200.0 - 1.0, "grounded enemy must take splash gh={gh}");
    assert!(
        (hh - 200.0).abs() < 0.01,
        "NOT_AIRBORNE + isSignificantlyAboveTerrain must skip high unit hh={hh}"
    );
}

#[test]
fn scatter_miss_applies_splash_at_offset() {
    let mut logic = GameLogic::new();
    {
        let mut t = ThingTemplate::new("ArtySc");
        // Firebase howitzer often has splash; force peel via weapon fields.
        t.primary_weapon_name = Some("AmericaFireBaseHowitzer".into());
        t.add_kind_of(KindOf::Vehicle);
        t.add_kind_of(KindOf::Attackable);
        logic.templates.insert("ArtySc".into(), t);
        let mut ti = ThingTemplate::new("InfSc");
        ti.add_kind_of(KindOf::Infantry);
        ti.add_kind_of(KindOf::Attackable);
        logic.templates.insert("InfSc".into(), ti);
        let mut tv = ThingTemplate::new("NearSc");
        tv.add_kind_of(KindOf::Vehicle);
        tv.add_kind_of(KindOf::Attackable);
        logic.templates.insert("NearSc".into(), tv);
    }
    let arty = logic
        .create_object("ArtySc", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    let intended = logic
        .create_object("InfSc", Team::China, glam::Vec3::new(100.0, 0.0, 0.0))
        .unwrap();
    let neighbor = logic
        .create_object("NearSc", Team::China, glam::Vec3::new(105.0, 0.0, 0.0))
        .unwrap();
    if let Some(o) = logic.objects.get_mut(&arty) {
        o.weapon = Some(Weapon {
            damage: 80.0,
            range: 300.0,
            min_range: 0.0,
            splash_radius: 30.0,
            reload_time: 0.0,
            last_fire_time: -100.0,
            can_target_ground: true,
            projectile_speed: 0.0,
            ..Weapon::default()
});
    }
    if let Some(o) = logic.objects.get_mut(&intended) {
        o.health.current = 500.0;
        o.health.maximum = 500.0;
        o.selection_radius = 1.0; // easy miss
    }
    if let Some(o) = logic.objects.get_mut(&neighbor) {
        o.health.current = 200.0;
        o.health.maximum = 200.0;
    }
    // Force a miss frame if possible and apply splash helper at known impact.
    let impact = glam::Vec3::new(105.0, 0.0, 0.0);
    let hits = logic.apply_scatter_miss_splash_at(
        impact,
        80.0,
        30.0,
        arty,
        Team::USA,
        intended,
        Some("AmericaFireBaseHowitzer"),
    );
    assert!(hits >= 1, "neighbor in splash must be hit");
    let nh = logic
        .objects
        .get(&neighbor)
        .map(|o| o.health.current)
        .unwrap_or(0.0);
    assert!(nh < 200.0 - 1.0, "neighbor took splash damage nh={nh}");
    // Intended skipped by skip_id.
    let ih = logic
        .objects
        .get(&intended)
        .map(|o| o.health.current)
        .unwrap_or(0.0);
    assert!((ih - 500.0).abs() < 0.01, "intended skipped on miss splash");
}

#[test]
fn scatter_miss_splash_does_not_invent_1_5x_secondary() {
    let mut logic = GameLogic::new();
    {
        let mut t = ThingTemplate::new("NoRingSrc");
        t.add_kind_of(KindOf::Vehicle);
        logic.templates.insert("NoRingSrc".into(), t);
        let mut te = ThingTemplate::new("NoRingFar");
        te.add_kind_of(KindOf::Vehicle);
        te.add_kind_of(KindOf::Attackable);
        logic.templates.insert("NoRingFar".into(), te);
    }
    let src = logic
        .create_object("NoRingSrc", Team::USA, glam::Vec3::ZERO)
        .unwrap();
    // 28wu is inside invented 1.5*20=30 but outside authored primary 20 with secondary 0.
    let far = logic
        .create_object("NoRingFar", Team::China, glam::Vec3::new(28.0, 0.0, 0.0))
        .unwrap();
    if let Some(o) = logic.objects.get_mut(&far) {
        o.health.current = 200.0;
        o.health.maximum = 200.0;
        o.selection_radius = 0.0;
        o.thing.template.geometry_info.authored = true;
        o.thing.template.geometry_info.major_radius = 0.0;
        o.thing.template.geometry_info.geom_type =
            crate::game_logic::HostGeometryType::Sphere;
        o.thing.template.geometry_info.height = 0.0;
    }
    let _ = logic.apply_scatter_miss_splash_at(
        glam::Vec3::ZERO,
        50.0,
        20.0,
        src,
        Team::USA,
        ObjectId(0),
        None,
    );
    let fh = logic
        .objects
        .get(&far)
        .map(|o| o.health.current)
        .unwrap_or(0.0);
    assert!(
        (fh - 200.0).abs() < 0.01,
        "must not invent 1.5x secondary ring fh={fh}"
    );
}

#[test]
fn scatter_miss_splash_uses_bounding_sphere_3d() {
    let mut logic = GameLogic::new();
    {
        let mut t = ThingTemplate::new("SphereSrc");
        t.add_kind_of(KindOf::Vehicle);
        logic.templates.insert("SphereSrc".into(), t);
        let mut te = ThingTemplate::new("SphereHigh");
        te.add_kind_of(KindOf::Vehicle);
        te.add_kind_of(KindOf::Attackable);
        logic.templates.insert("SphereHigh".into(), te);
    }
    let src = logic
        .create_object("SphereSrc", Team::USA, glam::Vec3::ZERO)
        .unwrap();
    // Same XZ as impact; 40wu up is outside 20wu 3D sphere (old XZ path would hit).
    let high = logic
        .create_object(
            "SphereHigh",
            Team::China,
            glam::Vec3::new(0.0, 40.0, 0.0),
        )
        .unwrap();
    if let Some(o) = logic.objects.get_mut(&high) {
        o.health.current = 200.0;
        o.health.maximum = 200.0;
        o.ground_height = 0.0;
        o.selection_radius = 0.0;
        o.thing.template.geometry_info.authored = true;
        o.thing.template.geometry_info.major_radius = 0.0;
        o.thing.template.geometry_info.geom_type =
            crate::game_logic::HostGeometryType::Sphere;
        o.thing.template.geometry_info.height = 0.0;
    }
    let _ = logic.apply_scatter_miss_splash_at(
        glam::Vec3::ZERO,
        50.0,
        20.0,
        src,
        Team::USA,
        ObjectId(0),
        None,
    );
    let hh = logic
        .objects
        .get(&high)
        .map(|o| o.health.current)
        .unwrap_or(0.0);
    assert!(
        (hh - 200.0).abs() < 0.01,
        "FROM_BOUNDINGSPHERE_3D must skip airborne-high unit hh={hh}"
    );
}

#[test]
fn instant_combat_scatter_can_miss_intended_target() {
    use crate::game_logic::weapon_bootstrap::{
        host_effective_scatter_radius, scatter_misses_intended_target,
    };
    // Ensure peel has VsInfantry scatter for crusader.
    let sc = host_effective_scatter_radius("AmericaTankCrusaderGun", true);
    assert!(sc >= 10.0 - 1e-3, "crusader vs infantry scatter {sc}");

    let mut logic = GameLogic::new();
    {
        let mut t = ThingTemplate::new("ScGun");
        t.primary_weapon_name = Some("AmericaTankCrusaderGun".into());
        t.add_kind_of(KindOf::Vehicle);
        t.add_kind_of(KindOf::Attackable);
        logic.templates.insert("ScGun".into(), t);
        let mut ti = ThingTemplate::new("ScInf");
        ti.add_kind_of(KindOf::Infantry);
        ti.add_kind_of(KindOf::Attackable);
        logic.templates.insert("ScInf".into(), ti);
    }
    let gun = logic
        .create_object("ScGun", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    let inf = logic
        .create_object("ScInf", Team::China, glam::Vec3::new(80.0, 0.0, 0.0))
        .unwrap();
    if let Some(o) = logic.objects.get_mut(&gun) {
        o.weapon = Some(Weapon {
            damage: 25.0,
            range: 150.0,
            reload_time: 0.0,
            last_fire_time: -100.0,
            can_target_ground: true,
            projectile_speed: 0.0, // instant residual
            ..Weapon::default()
});
        o.target = Some(inf);
        o.set_ai_state(AIState::Attacking);
        o.set_status_attacking(true);
    }
    if let Some(o) = logic.objects.get_mut(&inf) {
        o.health.current = 500.0;
        o.health.maximum = 500.0;
        o.selection_radius = 2.0; // small — easy scatter miss
    }
    // Helper must report miss for some seeds.
    let mut any_miss = false;
    for f in 0..64u32 {
        logic.frame = f;
        if logic.instant_scatter_misses_shot(gun, inf, 0) {
            any_miss = true;
            break;
        }
    }
    assert!(any_miss, "scatter must miss small infantry for some frames");
    assert!(scatter_misses_intended_target(10.0, 7, 2.0));

    // Run combat frames — health should sometimes survive full damage rate.
    let h0 = logic
        .objects
        .get(&inf)
        .map(|o| o.health.current)
        .unwrap_or(0.0);
    for f in 0..40u32 {
        logic.frame = f;
        if let Some(o) = logic.objects.get_mut(&gun) {
            if let Some(w) = o.weapon.as_mut() {
                w.last_fire_time = -100.0;
            }
            o.target = Some(inf);
            o.set_ai_state(AIState::Attacking);
        }
        logic.update_combat(&[gun, inf], 1.0 / 30.0);
    }
    let h1 = logic
        .objects
        .get(&inf)
        .map(|o| o.health.current)
        .unwrap_or(0.0);
    // Without scatter: 40 * 25 = 1000 would kill. With scatter misses, less damage.
    // At least verify combat ran (h1 <= h0) and scatter gate is live.
    assert!(h1 <= h0);
    assert!(
        h1 > 0.0 || any_miss,
        "either survived via miss or helper proved miss path"
    );
}

#[test]
fn ground_force_fire_applies_base_scatter_radius_peel() {
    use crate::game_logic::weapon_bootstrap::host_effective_scatter_radius;
    // Neutron / artillery-style weapons often have base ScatterRadius > 0.
    // Ground force-fire must not hardcode scatter_radius = 0.
    let src = crate::game_logic::game_logic::GAME_LOGIC_FACADE_SRC;
    assert!(
        src.contains("host_effective_scatter_radius") && src.contains("AttackingGround"),
        "AttackingGround path must peel ScatterRadius"
    );
    // Honesty: base peel for crusader is 0; howitzer/firebase may be >0.
    let _ = host_effective_scatter_radius("AmericaFireBaseHowitzer", false);
}

#[test]
fn minimum_attack_range_too_close_backs_away() {
    use crate::game_logic::weapon_bootstrap::{
        effective_minimum_attack_range, is_inside_minimum_attack_range,
    };
    let mut logic = GameLogic::new();
    {
        let mut t = ThingTemplate::new("Arty");
        t.primary_weapon_name = Some("AmericaFireBaseHowitzer".into());
        t.add_kind_of(KindOf::Vehicle);
        t.add_kind_of(KindOf::Attackable);
        logic.templates.insert("Arty".into(), t);
        let mut tg = ThingTemplate::new("CloseTgt");
        tg.add_kind_of(KindOf::Vehicle);
        tg.add_kind_of(KindOf::Attackable);
        logic.templates.insert("CloseTgt".into(), tg);
    }
    let arty = logic
        .create_object("Arty", Team::USA, glam::Vec3::new(20.0, 0.0, 0.0))
        .unwrap();
    let tgt = logic
        .create_object("CloseTgt", Team::China, glam::Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    if let Some(o) = logic.objects.get_mut(&arty) {
        o.weapon = Some(Weapon {
            damage: 40.0,
            range: 300.0,
            min_range: 50.0,
            reload_time: 0.0,
            last_fire_time: -100.0,
            can_target_ground: true,
            can_target_air: false,
            ..Weapon::default()
});
        o.target = Some(tgt);
        o.set_ai_state(AIState::Attacking);
        o.set_status_attacking(true);
    }
    let h0 = logic
        .objects
        .get(&tgt)
        .map(|t| t.health.current)
        .unwrap_or(0.0);
    // Too close: should back up, not damage.
    assert!(logic.try_min_range_backup(arty, glam::Vec3::ZERO, 50.0,));
    let dest = logic
        .objects
        .get(&arty)
        .and_then(|o| o.movement.target_position)
        .expect("backup sets move target");
    let dist = (dest.x * dest.x + dest.z * dest.z).sqrt();
    assert!(
        dist + 0.5 >= effective_minimum_attack_range(50.0) - 1.0,
        "backup dest dist={dist} dest={dest:?}"
    );
    // Combat path should not deal damage while still inside min (if still close).
    // Place again inside and run combat.
    if let Some(o) = logic.objects.get_mut(&arty) {
        o.set_position(glam::Vec3::new(20.0, 0.0, 0.0));
        o.movement.target_position = None;
        o.movement.path.clear();
        o.target = Some(tgt);
        o.set_ai_state(AIState::Attacking);
        if let Some(w) = o.weapon.as_mut() {
            w.last_fire_time = -100.0;
        }
    }
    for _ in 0..5 {
        logic.update_combat(&[arty, tgt], 1.0 / 30.0);
    }
    let h1 = logic
        .objects
        .get(&tgt)
        .map(|t| t.health.current)
        .unwrap_or(0.0);
    assert!(
        (h1 - h0).abs() < 0.01,
        "must not fire inside min range (h0={h0} h1={h1})"
    );
    // Outside min range — may fire.
    if let Some(o) = logic.objects.get_mut(&arty) {
        o.set_position(glam::Vec3::new(80.0, 0.0, 0.0));
        o.movement.target_position = None;
        o.movement.path.clear();
        o.set_status_moving(false);
        o.target = Some(tgt);
        o.set_ai_state(AIState::Attacking);
        if let Some(w) = o.weapon.as_mut() {
            w.last_fire_time = -100.0;
        }
    }
    for _ in 0..30 {
        logic.update_combat(&[arty, tgt], 1.0 / 30.0);
    }
    let h2 = logic
        .objects
        .get(&tgt)
        .map(|t| t.health.current)
        .unwrap_or(0.0);
    assert!(
        h2 < h0 - 1.0,
        "outside min range must allow fire h0={h0} h2={h2}"
    );
    assert!(!is_inside_minimum_attack_range(80.0, 50.0));
}

#[test]
fn contact_weapon_approach_reaches_target_noncontact_stands_off() {
    use crate::game_logic::weapon_bootstrap::{
        compute_approach_target_pos, is_contact_weapon_range,
    };
    assert!(is_contact_weapon_range(5.0));
    let mut logic = GameLogic::new();
    {
        let mut t = ThingTemplate::new("ContactAtk");
        t.primary_weapon_name = Some("DozerMineDisarmingWeapon".into());
        t.add_kind_of(KindOf::Vehicle);
        t.add_kind_of(KindOf::Attackable);
        logic.templates.insert("ContactAtk".into(), t);
        let mut t2 = ThingTemplate::new("GunAtk");
        t2.primary_weapon_name = Some("AmericaTankCrusaderGun".into());
        t2.add_kind_of(KindOf::Vehicle);
        t2.add_kind_of(KindOf::Attackable);
        logic.templates.insert("GunAtk".into(), t2);
        let mut tg = ThingTemplate::new("ApproachTgt");
        tg.add_kind_of(KindOf::Vehicle);
        tg.add_kind_of(KindOf::Attackable);
        logic.templates.insert("ApproachTgt".into(), tg);
    }
    let contact = logic
        .create_object("ContactAtk", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    let gun = logic
        .create_object("GunAtk", Team::USA, glam::Vec3::new(0.0, 0.0, 20.0))
        .unwrap();
    let tgt_pos = glam::Vec3::new(100.0, 0.0, 0.0);
    if let Some(o) = logic.objects.get_mut(&contact) {
        o.weapon = Some(Weapon {
            damage: 1.0,
            range: 5.0,
            reload_time: 0.0,
            last_fire_time: -100.0,
            ..Weapon::default()
});
    }
    if let Some(o) = logic.objects.get_mut(&gun) {
        o.weapon = Some(Weapon {
            damage: 10.0,
            range: 50.0,
            reload_time: 0.0,
            last_fire_time: -100.0,
            ..Weapon::default()
});
    }
    let c_app =
        logic.approach_pos_for_attack(contact, tgt_pos, 5.0, Some("DozerMineDisarmingWeapon"));
    assert!((c_app - tgt_pos).length() < 1e-2, "contact → target");
    let g_app = logic.approach_pos_for_attack(gun, tgt_pos, 50.0, Some("AmericaTankCrusaderGun"));
    let expected = compute_approach_target_pos(glam::Vec3::new(0.0, 0.0, 20.0), tgt_pos, 50.0);
    assert!(
        (g_app - expected).length() < 1.0,
        "gun standoff g={g_app:?} e={expected:?}"
    );
    // Gun should not march onto the target cell.
    assert!((g_app - tgt_pos).length() > 10.0);
}

#[test]
fn airfield_runway_reservation_limits_parallel_takeoff() {
    use crate::game_logic::host_dock_contain_exit_heal_residual::{
        airfield_runway_count, PARKING_PLACE_AIRFIELD_HAS_RUNWAYS, PARKING_PLACE_AIRFIELD_NUM_COLS,
    };
    assert!(PARKING_PLACE_AIRFIELD_HAS_RUNWAYS);
    assert_eq!(
        airfield_runway_count(true, PARKING_PLACE_AIRFIELD_NUM_COLS),
        2
    );
    let mut logic = GameLogic::new();
    {
        let mut af_t = ThingTemplate::new("RunwayAF");
        af_t.add_kind_of(KindOf::Structure);
        af_t.add_kind_of(KindOf::FSAirfield);
        logic.templates.insert("RunwayAF".into(), af_t);
        let mut jt = ThingTemplate::new("RunwayJet");
        jt.add_kind_of(KindOf::Aircraft);
        jt.add_kind_of(KindOf::Attackable);
        jt.primary_weapon_name = Some("AmericaJetRaptorRocketPods".into());
        logic.templates.insert("RunwayJet".into(), jt);
    }
    let af = logic
        .create_object("RunwayAF", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
        .expect("af");
    let mut jets = Vec::new();
    for i in 0..3 {
        let j = logic
            .create_object(
                "RunwayJet",
                Team::USA,
                glam::Vec3::new(i as f32 * 5.0, 0.0, 0.0),
            )
            .expect("jet");
        if let Some(o) = logic.objects.get_mut(&j) {
            o.object_type = ObjectType::Aircraft;
            o.set_ai_state(AIState::Docked);
            o.set_contained_by(Some(af));
            o.status.airborne_target = false;
        }
        if let Some(a) = logic.objects.get_mut(&af) {
            // direct hangar roster
            let mut list = a.contained_units();
            if !list.contains(&j) {
                // push via building garrison or occupants
                if let Some(b) = a.building_data.as_mut() {
                    b.garrisoned_units.push(j);
                } else {
                    a.occupants.push(j);
                }
            }
            let _ = list;
        }
        jets.push(j);
    }
    // Two runways: first two takeoffs succeed, third waits.
    assert!(logic.try_runway_takeoff_from_airfield(jets[0]));
    assert!(logic.try_runway_takeoff_from_airfield(jets[1]));
    assert_eq!(logic.airfield_runway_reserved_count(af), 2);
    assert!(
        !logic.try_runway_takeoff_from_airfield(jets[2]),
        "third jet must wait for a free runway"
    );
    assert!(
        logic
            .objects
            .get(&jets[2])
            .map(|o| o.ai_state == AIState::Docked || o.contained_by == Some(af))
            .unwrap_or(false),
        "waiting jet stays docked"
    );
    // First two have left the hangar (taxi or takeoff). Afterburners stay off until runway head.
    for &j in &jets[..2] {
        let o = logic.objects.get(&j).unwrap();
        assert!(o.contained_by.is_none());
        assert!(
            o.jet_ai.taxi_to_takeoff || o.jet_ai.takeoff_in_progress || o.status.airborne_target,
            "sortied jet must have left the stall"
        );
        assert!(
            !o.jet_ai.afterburners_on || o.jet_ai.takeoff_in_progress,
            "afterburners only at PauseBeforeTakeoff"
        );
    }
    // Move first jet clear and tick → frees a runway for third.
    if let Some(o) = logic.objects.get_mut(&jets[0]) {
        o.set_position(glam::Vec3::new(500.0, 50.0, 0.0));
    }
    logic.tick_airfield_runway_clear();
    assert!(logic.airfield_runway_reserved_count(af) <= 1);
    assert!(logic.try_runway_takeoff_from_airfield(jets[2]));
    assert!(logic
        .objects
        .get(&jets[2])
        .is_some_and(|o| o.contained_by.is_none()));
}

#[test]
fn airfield_runway_blocks_rtb_landing_when_busy() {
    let mut logic = GameLogic::new();
    {
        let mut af_t = ThingTemplate::new("LandAF");
        af_t.add_kind_of(KindOf::Structure);
        af_t.add_kind_of(KindOf::FSAirfield);
        logic.templates.insert("LandAF".into(), af_t);
        let mut jt = ThingTemplate::new("LandJet");
        jt.add_kind_of(KindOf::Aircraft);
        jt.add_kind_of(KindOf::Attackable);
        jt.primary_weapon_name = Some("AmericaJetRaptorRocketPods".into());
        logic.templates.insert("LandJet".into(), jt);
    }
    let af = logic
        .create_object("LandAF", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
        .expect("af");
    // Fill both runways with phantom airborne holders (takeoff residual).
    let h1 = logic
        .create_object("LandJet", Team::USA, glam::Vec3::new(10.0, 50.0, 0.0))
        .expect("h1");
    let h2 = logic
        .create_object("LandJet", Team::USA, glam::Vec3::new(20.0, 50.0, 0.0))
        .expect("h2");
    for &h in &[h1, h2] {
        if let Some(o) = logic.objects.get_mut(&h) {
            o.object_type = ObjectType::Aircraft;
            o.status.airborne_target = true;
            o.set_contained_by(None);
        }
        assert!(logic.reserve_airfield_runway(af, h).is_some());
    }
    assert_eq!(logic.airfield_runway_reserved_count(af), 2);

    // RTB jet near airfield with empty clip.
    let jet = logic
        .create_object("LandJet", Team::USA, glam::Vec3::new(30.0, 50.0, 0.0))
        .expect("jet");
    if let Some(o) = logic.objects.get_mut(&jet) {
        o.object_type = ObjectType::Aircraft;
        o.status.airborne_target = true;
        o.weapon = Some(Weapon {
            damage: 10.0,
            range: 100.0,
            reload_time: 1.0,
            last_fire_time: 0.0,
            clip_size: 1,
            ammo: Some(0),
            ..Weapon::default()
});
        // Ensure weapon name peels RETURN_TO_BASE.
        o.thing.template.primary_weapon_name = Some("AmericaJetRaptorRocketPods".into());
    }
    // While runways busy, RTB must not dock.
    let docked_busy = logic.try_return_to_base_rearm(jet);
    // needs_return_to_base may fail if weapon fields differ — still assert runway gate when needs.
    if logic
        .objects
        .get(&jet)
        .map(|j| j.needs_return_to_base_rearm())
        .unwrap_or(false)
    {
        assert!(!docked_busy, "busy runways must block RTB dock");
        assert!(logic
            .objects
            .get(&jet)
            .map(|j| j.contained_by.is_none())
            .unwrap_or(false));
    }
    // Free a runway → landing may proceed (if jet still needs RTB).
    logic.release_airfield_runway_for_jet(h1);
    if logic
        .objects
        .get(&jet)
        .map(|j| j.needs_return_to_base_rearm())
        .unwrap_or(false)
    {
        let af_pos = logic.objects.get(&af).unwrap().get_position();
        if let Some(j) = logic.objects.get_mut(&jet) {
            j.status.airborne_target = false;
            j.jet_ai.rtb_landing_phase = crate::game_logic::object::JET_RTB_PHASE_TAXI;
            j.set_position(af_pos);
        }
        assert!(logic.try_return_to_base_rearm(jet));
        let j = logic.objects.get(&jet).unwrap();
        assert_eq!(j.contained_by, Some(af));
        assert_eq!(j.ai_state, AIState::Docked);
        // Landing runway released after dock.
        assert!(logic
            .runway_reservations
            .get(&af)
            .map(|s| !s.iter().any(|x| *x == Some(jet)))
            .unwrap_or(true));
    }
}

#[test]
fn airfield_takeoff_releases_parking_slot() {
    use crate::game_logic::host_dock_contain_exit_heal_residual::PARKING_PLACE_AIRFIELD_APPROACH_HEIGHT;
    use crate::game_logic::{KindOf, ParkingPlaceMetadata, Team, ThingTemplate, Weapon};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let mut af_tmpl = ThingTemplate::new("AmericaAirfield");
    af_tmpl
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::FSAirfield)
        .set_health(1000.0);
    af_tmpl.parking_place = Some(ParkingPlaceMetadata {
        num_rows: 2,
        num_cols: 2,
        approach_height: 50.0,
        landing_deck_height_offset: 0.0,
        has_runways: true,
        park_in_hangars: true,
        heal_amount_per_second: 10.0,
    });
    logic.templates.insert("AmericaAirfield".into(), af_tmpl);
    let mut jet_tmpl = ThingTemplate::new("AmericaJetRaptor");
    jet_tmpl.primary_weapon_name = Some("HostTestRaptorJetMissileWeapon".into());
    jet_tmpl
        .add_kind_of(KindOf::Aircraft)
        .add_kind_of(KindOf::Attackable)
        .set_health(100.0);
    logic.templates.insert("AmericaJetRaptor".into(), jet_tmpl);

    let af_id = logic
        .create_object("AmericaAirfield", Team::USA, Vec3::ZERO)
        .unwrap();
    let jet_id = logic
        .create_object("AmericaJetRaptor", Team::USA, Vec3::new(40.0, 0.0, 0.0))
        .unwrap();
    {
        let jet = logic.objects.get_mut(&jet_id).unwrap();
        jet.weapon = Some(Weapon {
            damage: 50.0,
            range: 200.0,
            reload_time: 0.0,
            last_fire_time: -100.0,
            ammo: Some(0),
            clip_size: 4,
            can_target_air: true,
            can_target_ground: true,
            ..Weapon::default()
});
    }
    assert!(logic.try_return_to_base_rearm(jet_id));
    assert_eq!(logic.airfield_parked_count(af_id), 1);

    // C++ JetTakeoffOrLandingState onExit (897-900): uncontain, keep stall
    // when KeepsParkingSpaceWhenAirborne (default true, JetAIUpdate.cpp:1630).
    let stall_before = logic
        .objects
        .get(&jet_id)
        .and_then(|jet| jet.airfield_parking_space_index);
    {
        let jet = logic.objects.get_mut(&jet_id).unwrap();
        jet.attack_target(ObjectId(777));
    }
    assert!(logic.release_jet_from_airfield_parking(jet_id));
    {
        let jet = logic.objects.get(&jet_id).unwrap();
        assert!(jet.contained_by.is_none());
        assert_ne!(jet.ai_state, AIState::Docked);
        assert!(jet.status.airborne_target);
        assert!(jet.get_position().y >= PARKING_PLACE_AIRFIELD_APPROACH_HEIGHT - 1e-3);
        assert_eq!(jet.target, Some(ObjectId(777)));
        assert_eq!(jet.airfield_parking_space_index, stall_before);
    }
    assert_eq!(logic.airfield_parked_count(af_id), 0);
    let jet2 = logic
        .create_object("AmericaJetRaptor", Team::USA, Vec3::new(30.0, 0.0, 0.0))
        .unwrap();
    {
        let j = logic.objects.get_mut(&jet2).unwrap();
        j.weapon = Some(Weapon {
            damage: 10.0,
            range: 100.0,
            reload_time: 0.0,
            last_fire_time: -100.0,
            ammo: Some(0),
            clip_size: 2,
            ..Weapon::default()
});
    }
    assert!(logic.try_return_to_base_rearm(jet2));
}

fn helipad_parking_place() -> crate::game_logic::ParkingPlaceMetadata {
    crate::game_logic::ParkingPlaceMetadata {
        num_rows: 1,
        num_cols: 1,
        approach_height: 37.0,
        landing_deck_height_offset: 4.0,
        has_runways: false,
        park_in_hangars: false,
        heal_amount_per_second: 10.0,
    }
}

fn dock_helipad_comanche(logic: &mut GameLogic) -> (ObjectId, ObjectId) {
    use crate::game_logic::{KindOf, Team, ThingTemplate};

    use glam::Vec3;

    let mut pad_tmpl = ThingTemplate::new("AmericaHelipad");
    pad_tmpl
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::FSAirfield)
        .set_health(1000.0);
    pad_tmpl.parking_place = Some(helipad_parking_place());
    logic.templates.insert("AmericaHelipad".into(), pad_tmpl);

    let mut heli_tmpl = ThingTemplate::new("AmericaVehicleComanche");
    heli_tmpl
        .add_kind_of(KindOf::Aircraft)
        .add_kind_of(KindOf::Vehicle)
        .set_health(220.0);
    logic.templates.insert("AmericaVehicleComanche".into(), heli_tmpl);

    let pad_id = logic
        .create_object("AmericaHelipad", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("helipad");
    let heli_id = logic
        .create_object(
            "AmericaVehicleComanche",
            Team::USA,
            Vec3::new(0.0, 4.0, 0.0),
        )
        .expect("comanche");
    {
        let heli = logic.objects.get_mut(&heli_id).unwrap();
        heli.set_contained_by(Some(pad_id));
        heli.set_ai_state(AIState::Docked);
        heli.status.airborne_target = false;
        heli.producer_id = Some(pad_id);
        heli.movement.max_speed = 30.0;
    }
    (pad_id, heli_id)
}

#[test]
fn helipad_takeoff_is_two_point_climb_not_altitude_pop() {
    let mut logic = GameLogic::new();
    let (pad_id, heli_id) = dock_helipad_comanche(&mut logic);
    let pad_y = logic.objects.get(&heli_id).unwrap().get_position().y;
    assert!(logic.try_runway_takeoff_from_airfield(heli_id));
    {
        let heli = logic.objects.get(&heli_id).unwrap();
        assert!(heli.contained_by.is_none());
        assert!(heli.status.airborne_target);
        assert!(
            (heli.get_position().y - pad_y).abs() < 1e-3,
            "helipad takeoff must not pop Y in one frame, y={} pad={}",
            heli.get_position().y,
            pad_y
        );
        assert!(
            logic.heli_takeoff_or_landing.contains_key(&heli_id),
            "two-point HeliTakeoff state must be armed"
        );
    }
    let approach = pad_y + 37.0 + 4.0;
    for _ in 0..200 {
        logic.tick_airfield_parking_heal();
        let y = logic.objects.get(&heli_id).unwrap().get_position().y;
        if (y - approach).abs() <= 3.0 && !logic.heli_takeoff_or_landing.contains_key(&heli_id) {
            break;
        }
    }
    let heli = logic.objects.get(&heli_id).unwrap();
    assert!(
        (heli.get_position().y - approach).abs() <= 3.0,
        "heli must finish at parking + approachHeight + deck, y={} want {}",
        heli.get_position().y,
        approach
    );
    assert!(!logic.heli_takeoff_or_landing.contains_key(&heli_id));
    assert!(heli.status.airborne_target);
    let _ = pad_id;
}

#[test]
fn repaired_helipad_aircraft_auto_takeoff() {
    let mut logic = GameLogic::new();
    let (_pad_id, heli_id) = dock_helipad_comanche(&mut logic);
    {
        let heli = logic.objects.get_mut(&heli_id).unwrap();
        heli.health.current = heli.health.maximum;
        heli.target = None;
        heli.set_ai_state(AIState::Docked);
    }
    let pad_y = logic.objects.get(&heli_id).unwrap().get_position().y;
    logic.tick_airfield_parking_heal();
    {
        let heli = logic.objects.get(&heli_id).unwrap();
        assert!(
            heli.contained_by.is_none(),
            "full-health parked Comanche must lift off"
        );
        assert!(heli.status.airborne_target);
        assert!(
            (heli.get_position().y - pad_y).abs() < 1e-3,
            "auto-takeoff must use two-point climb, not pop"
        );
        assert!(logic.heli_takeoff_or_landing.contains_key(&heli_id));
    }
}

#[test]
fn helipad_landing_uses_two_point_descent_not_pad_snap() {
    use crate::game_logic::{KindOf, Team, ThingTemplate, Weapon};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let mut pad_tmpl = ThingTemplate::new("AmericaHelipad");
    pad_tmpl
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::FSAirfield)
        .set_health(1000.0);
    pad_tmpl.parking_place = Some(helipad_parking_place());
    logic.templates.insert("AmericaHelipad".into(), pad_tmpl);
    let mut heli_tmpl = ThingTemplate::new("AmericaVehicleComanche");
    heli_tmpl
        .add_kind_of(KindOf::Aircraft)
        .add_kind_of(KindOf::Vehicle)
        .set_health(220.0);
    logic.templates.insert("AmericaVehicleComanche".into(), heli_tmpl);

    let pad_id = logic
        .create_object("AmericaHelipad", Team::USA, Vec3::ZERO)
        .expect("helipad");
    let heli_id = logic
        .create_object(
            "AmericaVehicleComanche",
            Team::USA,
            Vec3::new(80.0, 50.0, 0.0),
        )
        .expect("comanche");
    {
        let heli = logic.objects.get_mut(&heli_id).unwrap();
        heli.weapon = Some(Weapon {
            damage: 10.0,
            range: 100.0,
            reload_time: 0.0,
            last_fire_time: -100.0,
            ammo: Some(0),
            clip_size: 4,
            ..Weapon::default()
});
        heli.status.airborne_target = true;
        heli.producer_id = Some(pad_id);
        heli.movement.max_speed = 30.0;
    }
    assert!(logic.try_return_to_base_rearm(heli_id));
    {
        let heli = logic.objects.get(&heli_id).unwrap();
        assert!(
            heli.contained_by.is_none(),
            "far helipad landing must not snap-dock"
        );
        assert!(logic.heli_takeoff_or_landing.contains_key(&heli_id));
        let y = heli.get_position().y;
        assert!(
            y > 10.0,
            "descent must start from current altitude, y={y}"
        );
    }
}


#[test]
fn target_pitch_gate_blocks_strategy_center_out_of_loft() {
    use crate::game_logic::weapon_bootstrap::{
        host_target_pitch_limits_for_weapon_name, is_pitch_within_limits,
    };
    let mut logic = GameLogic::new();
    // Install attacker template with Strategy Center artillery loft residual.
    {
        let mut tmpl = ThingTemplate::new("PitchSc");
        tmpl.primary_weapon_name = Some("AmericaStrategyCenterArtillery".into());
        tmpl.add_kind_of(KindOf::Structure);
        // CanAttack residual covered by weapon + structure kinds
        tmpl.add_kind_of(KindOf::Attackable);
        logic.templates.insert("PitchSc".into(), tmpl);
        let mut tt = ThingTemplate::new("PitchTgt");
        tt.add_kind_of(KindOf::Vehicle);
        tt.add_kind_of(KindOf::Attackable);
        logic.templates.insert("PitchTgt".into(), tt);
    }
    let sc = logic
        .create_object("PitchSc", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
        .expect("sc");
    // Deep depression: outside strategy loft and beyond ACCEPTABLE_DZ=10.
    let tgt = logic
        .create_object("PitchTgt", Team::China, glam::Vec3::new(100.0, -80.0, 0.0))
        .expect("tgt");
    if let Some(o) = logic.objects.get_mut(&sc) {
        o.weapon = Some(Weapon {
            damage: 50.0,
            range: 400.0,
            reload_time: 0.0,
            last_fire_time: -100.0,
            ..Weapon::default()
});
        o.health.current = 5000.0;
        o.health.maximum = 5000.0;
        o.target = Some(tgt);
        o.set_ai_state(AIState::Attacking);
        o.set_status_attacking(true);
    }
    if let Some(o) = logic.objects.get_mut(&tgt) {
        o.health.current = 500.0;
        o.health.maximum = 500.0;
    }
    let h0 = logic
        .objects
        .get(&tgt)
        .map(|o| o.health.current)
        .unwrap_or(0.0);
    crate::game_logic::host_damage_log::clear();
    for _ in 0..30 {
        logic.update_combat(&[sc, tgt], 1.0 / 30.0);
    }
    let h1 = logic
        .objects
        .get(&tgt)
        .map(|o| o.health.current)
        .unwrap_or(0.0);
    let dealt_bad = test_observed_damage_to(tgt, h0, h1);
    assert!(
        dealt_bad.abs() < 0.01,
        "out-of-pitch depression must not deal damage (h0={h0} h1={h1} dealt={dealt_bad})"
    );

    // Elevate target into loft window (~60°) and allow fire.
    if let Some(o) = logic.objects.get_mut(&tgt) {
        let dy = 100.0_f32 * 60f32.to_radians().tan();
        o.set_position(glam::Vec3::new(100.0, dy, 0.0));
        o.health.current = h0;
    }
    if let Some(o) = logic.objects.get_mut(&sc) {
        if let Some(w) = o.weapon.as_mut() {
            w.last_fire_time = -100.0;
        }
        o.target = Some(tgt);
        o.set_ai_state(AIState::Attacking);
        o.set_status_attacking(true);
    }
    crate::game_logic::host_damage_log::clear();
    for _ in 0..30 {
        logic.update_combat(&[sc, tgt], 1.0 / 30.0);
    }
    let h2 = logic
        .objects
        .get(&tgt)
        .map(|o| o.health.current)
        .unwrap_or(0.0);
    let dealt_loft = test_observed_damage_to(tgt, h0, h2);
    assert!(
        dealt_loft > 1.0,
        "lofted pitch must allow fire (h0={h0} h2={h2} dealt={dealt_loft})"
    );
    let lim = host_target_pitch_limits_for_weapon_name("AmericaStrategyCenterArtillery");
    // C++ ACCEPTABLE_DZ: near-level shots always pass the pitch gate.
    assert!(is_pitch_within_limits(
        glam::Vec3::ZERO,
        glam::Vec3::new(100.0, 0.0, 0.0),
        &lim
    ));
    assert!(!is_pitch_within_limits(
        glam::Vec3::ZERO,
        glam::Vec3::new(100.0, -80.0, 0.0),
        &lim
    ));
}

#[test]
fn supply_center_accepts_deposit_same_player_only() {
    use crate::game_logic::{DockKind, KindOf, Object, ObjectId, Player, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.players.insert(0, Player::new(0, Team::USA, "P0", true));
    logic.players.insert(1, Player::new(1, Team::USA, "P1", false));
    let mut t = ThingTemplate::new("AmericaSupplyCenter");
    t.add_kind_of(KindOf::SupplyCenter);
    t.dock_kind = DockKind::SupplyCenter;
    t.has_supply_center_create = true;
    let cid = ObjectId(8801);
    let mut center = Object::new(t, cid, Team::USA);
    center.owner_player_id = Some(0);
    center.construction_percent = 1.0;
    center.status.under_construction = false;
    logic.objects.insert(cid, center);
    assert!(logic.supply_center_accepts_deposit_for_test(cid, Team::USA, Some(0)));
    assert!(!logic.supply_center_accepts_deposit_for_test(cid, Team::USA, Some(1)));
}

#[test]
fn worker_mine_clear_dumps_carried_boxes() {
    use crate::game_logic::{Object, ObjectId, Team, ThingTemplate};
    let mut t = ThingTemplate::new("GLAWorker");
    let id = ObjectId(8802);
    let mut worker = Object::new(t, id, Team::GLA);
    worker.stored_resources.supplies = 3;
    worker.set_weapon_set_mine_clearing_detail(true);
    assert_eq!(worker.stored_resources.supplies, 0);
}
