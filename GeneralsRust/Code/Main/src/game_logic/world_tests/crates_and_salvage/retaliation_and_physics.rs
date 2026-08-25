//! Behavior suite extracted from `crates_and_salvage`.
use super::*;

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
    assert!(
        logic
            .find_closest_enemy(
                aid,
                200.0,
                find_enemy_flags::CAN_ATTACK | find_enemy_flags::WITHIN_ATTACK_RANGE,
            )
            .is_none()
    );
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
    assert!(
        logic
            .find_closest_enemy(aid, 100.0, find_enemy_flags::CAN_ATTACK)
            .is_none()
    );
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
        KindOf, MIN_RECOMPUTE_TIME_RESIDUAL, Object, ObjectId, Team, ThingTemplate,
        is_same_position_residual,
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
        assert!(
            (h.physics_get_mass() - 50.0).abs() < 1e-3,
            "cache stale until sync"
        );
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
        KindOf, MOTIVE_FRAMES_RESIDUAL, Object, ObjectId, Team, ThingTemplate,
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

#[test]
fn tick_physics_motion_step_flying_aircraft_without_lift_falls() {
    use crate::game_logic::{KindOf, LocomotorBehaviorZ, Object, ObjectId, Team, ThingTemplate};
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
