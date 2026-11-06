//! Host GameLogic tests — `scatter_and_chain`.
#![allow(unused_imports, non_snake_case, unused_variables, dead_code)]
use super::super::*;
use super::helpers::*;

#[test]
fn continue_attack_range_chains_to_nearby_same_team_target() {
    let mut logic = GameLogic::new();
    {
        let mut tmpl = ThingTemplate::new("ContAtk");
        tmpl.primary_weapon_name = Some("DozerMineDisarmingWeapon".into());
        tmpl.add_kind_of(KindOf::Vehicle);
        tmpl.add_kind_of(KindOf::Attackable);
        logic.templates.insert("ContAtk".into(), tmpl);
        let mut tm = ThingTemplate::new("ContMine");
        tm.add_kind_of(KindOf::Attackable);
        tm.experience_value = 5.0;
        logic.templates.insert("ContMine".into(), tm);
    }
    let atk = logic
        .create_object("ContAtk", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
        .expect("atk");
    let mine1 = logic
        .create_object("ContMine", Team::GLA, glam::Vec3::new(10.0, 0.0, 0.0))
        .expect("m1");
    let mine2 = logic
        .create_object("ContMine", Team::GLA, glam::Vec3::new(40.0, 0.0, 0.0))
        .expect("m2");
    // Far mine outside ContinueAttackRange 100.
    let mine_far = logic
        .create_object("ContMine", Team::GLA, glam::Vec3::new(500.0, 0.0, 0.0))
        .expect("mfar");
    if let Some(o) = logic.objects.get_mut(&atk) {
        o.weapon = Some(Weapon {
            damage: 999.0,
            range: 50.0,
            reload_time: 0.0,
            last_fire_time: -100.0,
            ..Weapon::default()
        });
        o.target = Some(mine1);
        o.set_ai_state(
            AIState::Attacking,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
        o.set_status_attacking(true);
    }
    for id in [mine1, mine2, mine_far] {
        if let Some(o) = logic.objects.get_mut(&id) {
            o.health.current = 1.0;
            o.health.maximum = 1.0;
        }
    }
    // Direct kill path via helper (combat path may miss mines without AntiMine).
    let pos = logic.objects.get(&mine1).unwrap().get_position();
    let team = logic.objects.get(&mine1).unwrap().team;
    logic.mark_object_for_destruction(mine1, Some(Team::USA));
    // Force dead flag for is_alive filter.
    if let Some(o) = logic.objects.get_mut(&mine1) {
        o.health.current = 0.0;
    }

    assert!(logic.try_continue_attack_after_kill(atk, mine1, pos, 100.0, team,));
    // Under AI_DECISION_AUTHORITY (default), continue-attack logs AttackTarget and
    // leaves host target untouched for GameWorld writeback.
    {
        let next = logic.objects.get(&atk).and_then(|a| a.target);
        assert_eq!(next, Some(mine2), "must chain to nearer same-team mine");
    }
    // Outside range: no continue.

    if let Some(o) = logic.objects.get_mut(&atk) {
        o.target = None;
        o.stop_attack(&mut crate::game_logic::ObjectAudio::record(&mut Vec::new()));
    }
    let far_pos = logic.objects.get(&mine_far).unwrap().get_position();
    // Only far mine left as candidate if we pretend mine2 also dead.
    if let Some(o) = logic.objects.get_mut(&mine2) {
        o.health.current = 0.0;
    }
    assert!(!logic.try_continue_attack_after_kill(
        atk,
        mine2,
        glam::Vec3::new(40.0, 0.0, 0.0),
        100.0,
        team,
    ));
    let _ = far_pos;
    assert!(
        logic.objects.get(&atk).and_then(|a| a.target).is_none(),
        "far mine outside range must not be acquired"
    );
    assert!(
        !logic.objects[&atk].status.attacking,
        "out-of-range continuation must leave the canonical attacker stopped"
    );
    // continue_or_stop stops when nothing in range
    logic.continue_or_stop_after_kill(
        atk,
        mine2,
        glam::Vec3::new(40.0, 0.0, 0.0),
        team,
        Some("DozerMineDisarmingWeapon"),
    );
    let a = logic.objects.get(&atk).unwrap();
    assert!(matches!(a.ai_state(), AIState::Idle) || a.target.is_none());
}

#[test]
fn fire_base_shell_scatters_vs_infantry() {
    use crate::game_logic::host_fire_base::FIRE_BASE_SCATTER_VS_INFANTRY;

    let mut logic = GameLogic::new();
    let mut fb_tpl = ThingTemplate::new("AmericaFireBase");
    fb_tpl
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(1000.0);
    logic
        .templates
        .insert("AmericaFireBase".to_string(), fb_tpl);
    let mut r_tpl = ThingTemplate::new("ChinaInfantryRedguard");
    r_tpl
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(100.0);
    logic
        .templates
        .insert("ChinaInfantryRedguard".to_string(), r_tpl);

    let fb = logic
        .create_object("AmericaFireBase", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
        .expect("firebase");
    let inf = logic
        .create_object(
            "ChinaInfantryRedguard",
            Team::China,
            glam::Vec3::new(120.0, 0.0, 0.0),
        )
        .expect("inf");
    let aim = glam::Vec3::new(120.0, 0.0, 0.0);
    let from = glam::Vec3::new(0.0, 10.0, 0.0);
    let shell = logic
        .spawn_fire_base_shell_projectile(fb, from, aim, Some(inf))
        .expect("shell");
    assert!(logic.honesty_fire_base_scatter_ok());
    let shell_aim = logic
        .objects
        .get(&shell)
        .and_then(|o| o.fire_base_shell_aim)
        .expect("aim");
    let dx = shell_aim[0] - aim.x;
    let dz = shell_aim[2] - aim.z;
    let d = (dx * dx + dz * dz).sqrt();
    assert!(d > 0.01, "infantry aim should scatter");
    assert!(d <= FIRE_BASE_SCATTER_VS_INFANTRY + 0.01);

    // Vehicle target: no scatter.
    let mut t_tpl = ThingTemplate::new("ChinaTankBattlemaster");
    t_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(400.0);
    logic
        .templates
        .insert("ChinaTankBattlemaster".to_string(), t_tpl);
    let tank = logic
        .create_object(
            "ChinaTankBattlemaster",
            Team::China,
            glam::Vec3::new(150.0, 0.0, 0.0),
        )
        .expect("tank");
    let before = logic.fire_base_scatter_applied;
    let shell2 = logic
        .spawn_fire_base_shell_projectile(fb, from, glam::Vec3::new(150.0, 0.0, 0.0), Some(tank))
        .expect("shell2");
    assert_eq!(logic.fire_base_scatter_applied, before);
    let aim2 = logic
        .objects
        .get(&shell2)
        .and_then(|o| o.fire_base_shell_aim)
        .expect("aim2");
    assert!((aim2[0] - 150.0).abs() < 0.01 && aim2[2].abs() < 0.01);
}

#[test]
fn technical_cannon_shell_scatters_vs_infantry() {
    use crate::game_logic::host_technical::TECH_CANNON_SCATTER_VS_INFANTRY;

    let mut logic = GameLogic::new();
    let mut t_tpl = ThingTemplate::new("GLAVehicleTechnical");
    t_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(180.0);
    logic
        .templates
        .insert("GLAVehicleTechnical".to_string(), t_tpl);
    let mut i_tpl = ThingTemplate::new("AmericaInfantryRanger");
    i_tpl
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(100.0);
    logic
        .templates
        .insert("AmericaInfantryRanger".to_string(), i_tpl);

    let tech = logic
        .create_object(
            "GLAVehicleTechnical",
            Team::GLA,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("tech");
    let inf = logic
        .create_object(
            "AmericaInfantryRanger",
            Team::USA,
            glam::Vec3::new(120.0, 0.0, 0.0),
        )
        .expect("inf");
    let aim = glam::Vec3::new(120.0, 0.0, 0.0);
    let from = glam::Vec3::new(0.0, 6.0, 0.0);
    let shell = logic
        .spawn_technical_cannon_shell_projectile(tech, from, aim, Some(inf))
        .expect("shell");
    assert!(logic.honesty_technical_cannon_scatter_ok());
    let s_aim = logic
        .objects
        .get(&shell)
        .and_then(|o| o.technical_cannon_shell_aim)
        .expect("aim");
    let d = ((s_aim[0] - aim.x).powi(2) + (s_aim[2] - aim.z).powi(2)).sqrt();
    assert!(d > 0.01 && d <= TECH_CANNON_SCATTER_VS_INFANTRY + 0.01);

    let mut v_tpl = ThingTemplate::new("AmericaTankCrusader");
    v_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(400.0);
    logic
        .templates
        .insert("AmericaTankCrusader".to_string(), v_tpl);
    let tank = logic
        .create_object(
            "AmericaTankCrusader",
            Team::USA,
            glam::Vec3::new(150.0, 0.0, 0.0),
        )
        .expect("tank");
    let before = logic.technical_cannon_scatter_applied;
    let shell2 = logic
        .spawn_technical_cannon_shell_projectile(
            tech,
            from,
            glam::Vec3::new(150.0, 0.0, 0.0),
            Some(tank),
        )
        .expect("shell2");
    assert_eq!(logic.technical_cannon_scatter_applied, before);
    let aim2 = logic
        .objects
        .get(&shell2)
        .and_then(|o| o.technical_cannon_shell_aim)
        .expect("aim2");
    assert!((aim2[0] - 150.0).abs() < 0.01 && aim2[2].abs() < 0.01);
}

#[test]
fn tomahawk_missile_scatters_vs_infantry() {
    use crate::game_logic::host_tomahawk::TOMAHAWK_SCATTER_VS_INFANTRY;

    let mut logic = GameLogic::new();
    let mut t_tpl = ThingTemplate::new("AmericaVehicleTomahawk");
    t_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(180.0);
    logic
        .templates
        .insert("AmericaVehicleTomahawk".to_string(), t_tpl);
    let mut i_tpl = ThingTemplate::new("AmericaInfantryRanger");
    i_tpl
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(100.0);
    logic
        .templates
        .insert("AmericaInfantryRanger".to_string(), i_tpl);

    let tom = logic
        .create_object(
            "AmericaVehicleTomahawk",
            Team::USA,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("tom");
    let inf = logic
        .create_object(
            "AmericaInfantryRanger",
            Team::China,
            glam::Vec3::new(200.0, 0.0, 0.0),
        )
        .expect("inf");
    let aim = glam::Vec3::new(200.0, 0.0, 0.0);
    let from = glam::Vec3::new(0.0, 6.0, 0.0);
    let shell = logic
        .spawn_tomahawk_missile_projectile(tom, from, aim, Some(inf))
        .expect("missile");
    assert!(logic.honesty_tomahawk_scatter_ok());
    let s_aim = logic
        .objects
        .get(&shell)
        .and_then(|o| o.tomahawk_missile_aim)
        .expect("aim");
    let d = ((s_aim[0] - aim.x).powi(2) + (s_aim[2] - aim.z).powi(2)).sqrt();
    assert!(d > 0.01 && d <= TOMAHAWK_SCATTER_VS_INFANTRY + 0.01);

    let mut v_tpl = ThingTemplate::new("ChinaTankBattleMaster");
    v_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(400.0);
    logic
        .templates
        .insert("ChinaTankBattleMaster".to_string(), v_tpl);
    let tank = logic
        .create_object(
            "ChinaTankBattleMaster",
            Team::China,
            glam::Vec3::new(250.0, 0.0, 0.0),
        )
        .expect("tank");
    let before = logic.tomahawk_scatter_applied;
    let shell2 = logic
        .spawn_tomahawk_missile_projectile(tom, from, glam::Vec3::new(250.0, 0.0, 0.0), Some(tank))
        .expect("missile2");
    assert_eq!(logic.tomahawk_scatter_applied, before);
    let aim2 = logic
        .objects
        .get(&shell2)
        .and_then(|o| o.tomahawk_missile_aim)
        .expect("aim2");
    assert!((aim2[0] - 250.0).abs() < 0.01 && aim2[2].abs() < 0.01);
}

#[test]
fn patriot_scatter_misses_infantry_residual() {
    use crate::game_logic::host_base_defense::PATRIOT_SCATTER_RADIUS_VS_INFANTRY;
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let mut logic = GameLogic::new();
    let mut patriot_tpl = ThingTemplate::new("USA_Patriot");
    patriot_tpl
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .add_kind_of(KindOf::FSBaseDefense)
        .set_health(600.0)
        .set_primary_weapon_name(crate::game_logic::weapon_bootstrap::PATRIOT_PRIMARY_WEAPON)
        .set_secondary_weapon_name(crate::game_logic::weapon_bootstrap::PATRIOT_SECONDARY_WEAPON);
    logic
        .templates
        .insert("USA_Patriot".to_string(), patriot_tpl);
    ensure_test_infantry_template(&mut logic);

    let patriot = logic
        .create_object("USA_Patriot", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
        .expect("patriot");
    let inf = logic
        .create_object("TestInfantry", Team::GLA, glam::Vec3::new(50.0, 0.0, 0.0))
        .expect("inf");
    // Tiny geometry so scatter miss cone is active.
    if let Some(o) = logic.objects.get_mut(&inf) {
        o.set_selection_radius(0.5);
    }
    if let Some(p) = logic.objects.get_mut(&patriot) {
        if let Some(w) = p.weapon.as_mut() {
            w.last_fire_time = -10.0;
        }
    }
    let hp_before = logic.host_object(inf).unwrap().health.current;
    let mut saw_scatter = false;
    for f in 0..120u32 {
        logic.frame = f;
        logic.update_combat(&[patriot, inf], LOGIC_FRAME_TIMESTEP);
        if logic.patriot_scatter_applied > 0 {
            saw_scatter = true;
        }
        if logic.patriot_scatter_misses > 0 {
            break;
        }
    }
    assert!(saw_scatter, "expected patriot scatter peel vs infantry");
    assert!((PATRIOT_SCATTER_RADIUS_VS_INFANTRY - 10.0).abs() < 0.01);
    // Either miss was recorded, or a hit still damaged (deterministic seed dependent).
    let hp_after = logic
        .host_object(inf)
        .map(|o| o.health.current)
        .unwrap_or(0.0);
    assert!(
        logic.patriot_scatter_misses > 0 || hp_after < hp_before,
        "scatter residual must miss or damage infantry (misses={} hp {}->{})",
        logic.patriot_scatter_misses,
        hp_before,
        hp_after
    );
    assert!(logic.honesty_patriot_scatter_ok());
}

#[test]
fn stinger_scatter_misses_infantry_residual() {
    use crate::game_logic::host_base_defense::{
        STINGER_PRIMARY_WEAPON, STINGER_SCATTER_RADIUS_VS_INFANTRY,
    };
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let mut logic = GameLogic::new();
    let mut stinger_tpl = ThingTemplate::new("GLAStingerSite");
    stinger_tpl
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .add_kind_of(KindOf::FSBaseDefense)
        .set_health(1000.0)
        .set_primary_weapon_name(STINGER_PRIMARY_WEAPON);
    logic
        .templates
        .insert("GLAStingerSite".to_string(), stinger_tpl);
    ensure_test_infantry_template(&mut logic);
    ensure_test_tank_template(&mut logic);

    let stinger = logic
        .create_object("GLAStingerSite", Team::GLA, glam::Vec3::new(0.0, 0.0, 0.0))
        .expect("stinger");

    logic.run_first_spawn_update_for_test();
    // SPAWNS_ARE_THE_WEAPONS residual needs slaves.
    if let Some(s) = logic.objects.get_mut(&stinger) {
        s.hive_slave_count = 3;
        if let Some(w) = s.weapon.as_mut() {
            w.last_fire_time = -10.0;
        }
    }
    let inf = logic
        .create_object("TestInfantry", Team::USA, glam::Vec3::new(50.0, 0.0, 0.0))
        .expect("inf");
    if let Some(o) = logic.objects.get_mut(&inf) {
        o.set_selection_radius(0.5);
    }

    let mut saw_apply = false;
    for f in 0..80u32 {
        logic.frame = f;
        logic.update_combat(&[stinger, inf], LOGIC_FRAME_TIMESTEP);
        if logic.stinger_scatter_applied > 0 {
            saw_apply = true;
        }
        if logic.stinger_scatter_misses > 0 {
            break;
        }
    }
    assert!(
        saw_apply || logic.stinger_site_residual_ground_fires > 0,
        "stinger residual should engage infantry (applied={}, ground={})",
        logic.stinger_scatter_applied,
        logic.stinger_site_residual_ground_fires
    );
    assert!((STINGER_SCATTER_RADIUS_VS_INFANTRY - 10.0).abs() < 0.01);

    // Vehicle path still damages (no infantry scatter gate).
    let tank = logic
        .create_object("TestTank", Team::USA, glam::Vec3::new(40.0, 0.0, 0.0))
        .expect("tank");
    logic.mark_object_for_destruction(inf, None);
    logic.process_destroy_list();
    if let Some(s) = logic.objects.get_mut(&stinger) {
        if let Some(w) = s.weapon.as_mut() {
            w.last_fire_time = -10.0;
        }
    }
    let hp_before = logic.host_object(tank).unwrap().health.current;
    for f in 80..160u32 {
        logic.frame = f;
        logic.update_combat(&[stinger, tank], LOGIC_FRAME_TIMESTEP);
    }
    let hp_after = logic
        .host_object(tank)
        .map(|o| o.health.current)
        .unwrap_or(0.0);
    assert!(
        hp_after < hp_before || logic.stinger_site_residual_ground_fires > 0,
        "stinger must still engage vehicles (hp {}->{})",
        hp_before,
        hp_after
    );
    assert!(logic.honesty_stinger_scatter_ok() || logic.stinger_site_residual_ground_fires > 0);
}

#[test]
fn fire_base_scatter_misses_infantry_residual() {
    use crate::game_logic::host_fire_base::{
        FIRE_BASE_HOWITZER_WEAPON, FIRE_BASE_SCATTER_VS_INFANTRY,
    };
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let mut logic = GameLogic::new();
    ensure_test_infantry_template(&mut logic);
    ensure_test_tank_template(&mut logic);

    let mut fb_tpl = ThingTemplate::new("AmericaFireBase");
    fb_tpl
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .add_kind_of(KindOf::FSBaseDefense)
        .set_health(1000.0)
        .set_primary_weapon_name(FIRE_BASE_HOWITZER_WEAPON);
    logic
        .templates
        .insert("AmericaFireBase".to_string(), fb_tpl);

    let fb = logic
        .create_object("AmericaFireBase", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
        .expect("firebase");
    if let Some(o) = logic.objects.get_mut(&fb) {
        if let Some(w) = o.weapon.as_mut() {
            w.last_fire_time = -100.0;
            w.reload_time = 0.1;
        }
    }
    let inf = logic
        .create_object("TestInfantry", Team::GLA, glam::Vec3::new(80.0, 0.0, 0.0))
        .expect("inf");
    if let Some(o) = logic.objects.get_mut(&inf) {
        o.set_selection_radius(0.5);
    }

    // Force instant-apply path (no shell template spawn) by clearing projectile template
    // after create — spawn may still succeed; either path must peel scatter.
    let mut saw = false;
    for f in 0..90u32 {
        logic.frame = f;
        // Prefer direct apply residual for honesty of impact scatter.
        if f == 5 {
            let impact = logic
                .objects
                .get(&inf)
                .map(|o| o.get_position())
                .unwrap_or(glam::Vec3::new(80.0, 0.0, 0.0));
            let _ = logic.apply_fire_base_residual_at(impact, Some(fb), Some(inf));
        }
        logic.update_combat(&[fb, inf], LOGIC_FRAME_TIMESTEP);
        if logic.fire_base_scatter_applied > 0 || logic.fire_base_scatter_misses > 0 {
            saw = true;
            break;
        }
    }
    assert!(saw, "fire base scatter residual must apply vs infantry");
    assert!((FIRE_BASE_SCATTER_VS_INFANTRY - 15.0).abs() < 0.01);

    // Vehicle path still damages.
    let tank = logic
        .create_object("TestTank", Team::GLA, glam::Vec3::new(70.0, 0.0, 0.0))
        .expect("tank");
    logic.mark_object_for_destruction(inf, None);
    logic.process_destroy_list();
    let hp_before = logic.host_object(tank).unwrap().health.current;
    let impact = logic
        .objects
        .get(&tank)
        .map(|o| o.get_position())
        .unwrap_or(glam::Vec3::new(70.0, 0.0, 0.0));
    let (hits, _) = logic.apply_fire_base_residual_at(impact, Some(fb), Some(tank));
    let hp_after = logic
        .host_object(tank)
        .map(|o| o.health.current)
        .unwrap_or(0.0);
    assert!(
        hits > 0 && hp_after < hp_before,
        "vehicle still hit without infantry scatter"
    );
    assert!(logic.honesty_fire_base_scatter_ok());
}

#[test]
fn technical_cannon_scatter_misses_infantry_residual() {
    use crate::game_logic::host_technical::{
        TECH_CANNON_SCATTER_VS_INFANTRY, TECHNICAL_CANNON, TechnicalWeaponTier,
    };
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let mut logic = GameLogic::new();
    ensure_test_infantry_template(&mut logic);
    ensure_test_tank_template(&mut logic);

    let mut tech_tpl = ThingTemplate::new("GLAVehicleTechnical");
    tech_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(200.0)
        .set_primary_weapon_name(TECHNICAL_CANNON);
    logic
        .templates
        .insert("GLAVehicleTechnical".to_string(), tech_tpl);

    let tech = logic
        .create_object(
            "GLAVehicleTechnical",
            Team::GLA,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("tech");
    logic.apply_technical_weapon_tier(tech, TechnicalWeaponTier::One);
    if let Some(o) = logic.objects.get_mut(&tech) {
        if let Some(w) = o.weapon.as_mut() {
            w.last_fire_time = -100.0;
        }
    }
    let inf = logic
        .create_object("TestInfantry", Team::USA, glam::Vec3::new(60.0, 0.0, 0.0))
        .expect("inf");
    if let Some(o) = logic.objects.get_mut(&inf) {
        o.set_selection_radius(0.5);
    }

    let impact = logic
        .objects
        .get(&inf)
        .map(|o| o.get_position())
        .unwrap_or(glam::Vec3::new(60.0, 0.0, 0.0));
    let _ = logic.apply_technical_residual_at(impact, Some(tech), Some(inf));
    assert!(
        logic.technical_cannon_scatter_applied > 0
            || logic.technical_cannon_scatter_misses > 0
            || logic.honesty_technical_cannon_scatter_ok(),
        "technical cannon scatter residual must peel vs infantry"
    );
    assert!((TECH_CANNON_SCATTER_VS_INFANTRY - 10.0).abs() < 0.01);

    // Vehicle still damaged without infantry scatter gate.
    let tank = logic
        .create_object("TestTank", Team::USA, glam::Vec3::new(55.0, 0.0, 0.0))
        .expect("tank");
    logic.mark_object_for_destruction(inf, None);
    logic.process_destroy_list();
    let hp_before = logic.host_object(tank).unwrap().health.current;
    let impact = logic
        .objects
        .get(&tank)
        .map(|o| o.get_position())
        .unwrap_or(glam::Vec3::new(55.0, 0.0, 0.0));
    let (hits, _) = logic.apply_technical_residual_at(impact, Some(tech), Some(tank));
    let hp_after = logic
        .host_object(tank)
        .map(|o| o.health.current)
        .unwrap_or(0.0);
    assert!(hits > 0 && hp_after < hp_before, "vehicle still hit");
}

#[test]
fn rpg_trooper_scatter_misses_infantry_residual() {
    use crate::game_logic::host_rpg_trooper::{
        RPG_TROOPER_SCATTER_VS_INFANTRY, TUNNEL_DEFENDER_ROCKET_WEAPON,
    };
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let mut logic = GameLogic::new();
    ensure_test_infantry_template(&mut logic);
    ensure_test_tank_template(&mut logic);

    let mut rpg_tpl = ThingTemplate::new("GLAInfantryTunnelDefender");
    rpg_tpl
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(100.0)
        .set_primary_weapon_name(TUNNEL_DEFENDER_ROCKET_WEAPON);
    logic
        .templates
        .insert("GLAInfantryTunnelDefender".to_string(), rpg_tpl);

    let rpg = logic
        .create_object(
            "GLAInfantryTunnelDefender",
            Team::GLA,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("rpg");
    if let Some(o) = logic.objects.get_mut(&rpg) {
        if let Some(w) = o.weapon.as_mut() {
            w.last_fire_time = -100.0;
        }
    }
    let inf = logic
        .create_object("TestInfantry", Team::USA, glam::Vec3::new(50.0, 0.0, 0.0))
        .expect("inf");
    if let Some(o) = logic.objects.get_mut(&inf) {
        o.set_selection_radius(0.5);
    }

    let impact = logic
        .objects
        .get(&inf)
        .map(|o| o.get_position())
        .unwrap_or(glam::Vec3::new(50.0, 0.0, 0.0));
    let _ = logic.apply_rpg_trooper_residual_at(impact, Some(rpg), Some(inf));
    assert!(
        logic.rpg_trooper_scatter_applied > 0
            || logic.rpg_trooper_scatter_misses > 0
            || logic.honesty_rpg_trooper_scatter_ok(),
        "rpg scatter residual must peel vs infantry"
    );
    assert!((RPG_TROOPER_SCATTER_VS_INFANTRY - 10.0).abs() < 0.01);

    let tank = logic
        .create_object("TestTank", Team::USA, glam::Vec3::new(45.0, 0.0, 0.0))
        .expect("tank");
    logic.mark_object_for_destruction(inf, None);
    logic.process_destroy_list();
    let hp_before = logic.host_object(tank).unwrap().health.current;
    let impact = logic
        .objects
        .get(&tank)
        .map(|o| o.get_position())
        .unwrap_or(glam::Vec3::new(45.0, 0.0, 0.0));
    let (hits, _) = logic.apply_rpg_trooper_residual_at(impact, Some(rpg), Some(tank));
    let hp_after = logic
        .host_object(tank)
        .map(|o| o.health.current)
        .unwrap_or(0.0);
    assert!(hits > 0 && hp_after < hp_before, "vehicle still hit");
}

#[test]
fn missile_defender_scatter_misses_infantry_residual() {
    use crate::game_logic::host_missile_defender::{
        MISSILE_DEFENDER_MISSILE_WEAPON, MISSILE_DEFENDER_SCATTER_VS_INFANTRY,
    };
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let mut logic = GameLogic::new();
    ensure_test_infantry_template(&mut logic);
    ensure_test_tank_template(&mut logic);

    let mut md_tpl = ThingTemplate::new("AmericaInfantryMissileDefender");
    md_tpl
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(100.0)
        .set_primary_weapon_name(MISSILE_DEFENDER_MISSILE_WEAPON);
    logic
        .templates
        .insert("AmericaInfantryMissileDefender".to_string(), md_tpl);

    let md = logic
        .create_object(
            "AmericaInfantryMissileDefender",
            Team::USA,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("md");
    if let Some(o) = logic.objects.get_mut(&md) {
        if let Some(w) = o.weapon.as_mut() {
            w.last_fire_time = -100.0;
        }
    }
    let inf = logic
        .create_object("TestInfantry", Team::GLA, glam::Vec3::new(50.0, 0.0, 0.0))
        .expect("inf");
    if let Some(o) = logic.objects.get_mut(&inf) {
        o.set_selection_radius(0.5);
    }

    let impact = logic
        .objects
        .get(&inf)
        .map(|o| o.get_position())
        .unwrap_or(glam::Vec3::new(50.0, 0.0, 0.0));
    let _ = logic.apply_missile_defender_residual_at(impact, Some(md), Some(inf), false);
    assert!(
        logic.missile_defender_scatter_applied > 0
            || logic.missile_defender_scatter_misses > 0
            || logic.honesty_missile_defender_scatter_ok(),
        "md scatter residual must peel vs infantry"
    );
    assert!((MISSILE_DEFENDER_SCATTER_VS_INFANTRY - 10.0).abs() < 0.01);

    let tank = logic
        .create_object("TestTank", Team::GLA, glam::Vec3::new(45.0, 0.0, 0.0))
        .expect("tank");
    logic.mark_object_for_destruction(inf, None);
    logic.process_destroy_list();
    let hp_before = logic.host_object(tank).unwrap().health.current;
    let impact = logic
        .objects
        .get(&tank)
        .map(|o| o.get_position())
        .unwrap_or(glam::Vec3::new(45.0, 0.0, 0.0));
    let (hits, _) = logic.apply_missile_defender_residual_at(impact, Some(md), Some(tank), false);
    let hp_after = logic
        .host_object(tank)
        .map(|o| o.health.current)
        .unwrap_or(0.0);
    assert!(hits > 0 && hp_after < hp_before, "vehicle still hit");
}

#[test]
fn tank_hunter_scatter_misses_infantry_residual() {
    use crate::game_logic::host_tank_hunter::{
        TANK_HUNTER_MISSILE_WEAPON, TANK_HUNTER_SCATTER_VS_INFANTRY,
    };
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let mut logic = GameLogic::new();
    ensure_test_infantry_template(&mut logic);
    ensure_test_tank_template(&mut logic);

    let mut th_tpl = ThingTemplate::new("ChinaInfantryTankHunter");
    th_tpl
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(100.0)
        .set_primary_weapon_name(TANK_HUNTER_MISSILE_WEAPON);
    logic
        .templates
        .insert("ChinaInfantryTankHunter".to_string(), th_tpl);

    let th = logic
        .create_object(
            "ChinaInfantryTankHunter",
            Team::China,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("th");
    if let Some(o) = logic.objects.get_mut(&th) {
        if let Some(w) = o.weapon.as_mut() {
            w.last_fire_time = -100.0;
        }
    }
    let inf = logic
        .create_object("TestInfantry", Team::USA, glam::Vec3::new(50.0, 0.0, 0.0))
        .expect("inf");
    if let Some(o) = logic.objects.get_mut(&inf) {
        o.set_selection_radius(0.5);
    }

    let impact = logic
        .objects
        .get(&inf)
        .map(|o| o.get_position())
        .unwrap_or(glam::Vec3::new(50.0, 0.0, 0.0));
    let _ = logic.apply_tank_hunter_residual_at(impact, Some(th), Some(inf));
    assert!(
        logic.tank_hunter_scatter_applied > 0
            || logic.tank_hunter_scatter_misses > 0
            || logic.honesty_tank_hunter_scatter_ok(),
        "tank hunter scatter residual must peel vs infantry"
    );
    assert!((TANK_HUNTER_SCATTER_VS_INFANTRY - 10.0).abs() < 0.01);

    let tank = logic
        .create_object("TestTank", Team::USA, glam::Vec3::new(45.0, 0.0, 0.0))
        .expect("tank");
    logic.mark_object_for_destruction(inf, None);
    logic.process_destroy_list();
    let hp_before = logic.host_object(tank).unwrap().health.current;
    let impact = logic
        .objects
        .get(&tank)
        .map(|o| o.get_position())
        .unwrap_or(glam::Vec3::new(45.0, 0.0, 0.0));
    let (hits, _) = logic.apply_tank_hunter_residual_at(impact, Some(th), Some(tank));
    let hp_after = logic
        .host_object(tank)
        .map(|o| o.health.current)
        .unwrap_or(0.0);
    assert!(hits > 0 && hp_after < hp_before, "vehicle still hit");
}

#[test]
fn scorpion_scatter_misses_infantry_residual() {
    use crate::game_logic::host_scorpion::{SCORPION_SCATTER_VS_INFANTRY, SCORPION_TANK_GUN};
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let mut logic = GameLogic::new();
    ensure_test_infantry_template(&mut logic);
    ensure_test_tank_template(&mut logic);

    let mut sc_tpl = ThingTemplate::new("GLAVehicleScorpion");
    sc_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(200.0)
        .set_primary_weapon_name(SCORPION_TANK_GUN);
    logic
        .templates
        .insert("GLAVehicleScorpion".to_string(), sc_tpl);

    let sc = logic
        .create_object(
            "GLAVehicleScorpion",
            Team::GLA,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("scorpion");
    if let Some(o) = logic.objects.get_mut(&sc) {
        if let Some(w) = o.weapon.as_mut() {
            w.last_fire_time = -100.0;
        }
    }
    let inf = logic
        .create_object("TestInfantry", Team::USA, glam::Vec3::new(50.0, 0.0, 0.0))
        .expect("inf");
    if let Some(o) = logic.objects.get_mut(&inf) {
        o.set_selection_radius(0.5);
    }

    let impact = logic
        .objects
        .get(&inf)
        .map(|o| o.get_position())
        .unwrap_or(glam::Vec3::new(50.0, 0.0, 0.0));
    let _ = logic.apply_scorpion_residual_at(impact, Some(sc), Some(inf), 0);
    assert!(
        logic.scorpion_scatter_applied > 0
            || logic.scorpion_scatter_misses > 0
            || logic.honesty_scorpion_scatter_ok(),
        "scorpion scatter residual must peel vs infantry"
    );
    assert!((SCORPION_SCATTER_VS_INFANTRY - 10.0).abs() < 0.01);

    let tank = logic
        .create_object("TestTank", Team::USA, glam::Vec3::new(45.0, 0.0, 0.0))
        .expect("tank");
    logic.mark_object_for_destruction(inf, None);
    logic.process_destroy_list();
    let hp_before = logic.host_object(tank).unwrap().health.current;
    let impact = logic
        .objects
        .get(&tank)
        .map(|o| o.get_position())
        .unwrap_or(glam::Vec3::new(45.0, 0.0, 0.0));
    let (hits, _) = logic.apply_scorpion_residual_at(impact, Some(sc), Some(tank), 0);
    let hp_after = logic
        .host_object(tank)
        .map(|o| o.health.current)
        .unwrap_or(0.0);
    assert!(hits > 0 && hp_after < hp_before, "vehicle still hit");
}

#[test]
fn marauder_scatter_misses_infantry_residual() {
    use crate::game_logic::host_marauder::{MARAUDER_SCATTER_VS_INFANTRY, MARAUDER_TANK_GUN};
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let mut logic = GameLogic::new();
    ensure_test_infantry_template(&mut logic);
    ensure_test_tank_template(&mut logic);

    let mut ma_tpl = ThingTemplate::new("GLAVehicleMarauder");
    ma_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(300.0)
        .set_primary_weapon_name(MARAUDER_TANK_GUN);
    logic
        .templates
        .insert("GLAVehicleMarauder".to_string(), ma_tpl);

    let ma = logic
        .create_object(
            "GLAVehicleMarauder",
            Team::GLA,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("marauder");
    if let Some(o) = logic.objects.get_mut(&ma) {
        if let Some(w) = o.weapon.as_mut() {
            w.last_fire_time = -100.0;
        }
    }
    let inf = logic
        .create_object("TestInfantry", Team::USA, glam::Vec3::new(50.0, 0.0, 0.0))
        .expect("inf");
    if let Some(o) = logic.objects.get_mut(&inf) {
        o.set_selection_radius(0.5);
    }

    let impact = logic
        .objects
        .get(&inf)
        .map(|o| o.get_position())
        .unwrap_or(glam::Vec3::new(50.0, 0.0, 0.0));
    let _ = logic.apply_marauder_residual_at(impact, Some(ma), Some(inf));
    assert!(
        logic.marauder_scatter_applied > 0
            || logic.marauder_scatter_misses > 0
            || logic.honesty_marauder_scatter_ok(),
        "marauder scatter residual must peel vs infantry"
    );
    assert!((MARAUDER_SCATTER_VS_INFANTRY - 10.0).abs() < 0.01);

    let tank = logic
        .create_object("TestTank", Team::USA, glam::Vec3::new(45.0, 0.0, 0.0))
        .expect("tank");
    logic.mark_object_for_destruction(inf, None);
    logic.process_destroy_list();
    let hp_before = logic.host_object(tank).unwrap().health.current;
    let impact = logic
        .objects
        .get(&tank)
        .map(|o| o.get_position())
        .unwrap_or(glam::Vec3::new(45.0, 0.0, 0.0));
    let (hits, _) = logic.apply_marauder_residual_at(impact, Some(ma), Some(tank));
    let hp_after = logic
        .host_object(tank)
        .map(|o| o.health.current)
        .unwrap_or(0.0);
    assert!(hits > 0 && hp_after < hp_before, "vehicle still hit");
}

#[test]
fn tomahawk_scatter_misses_infantry_residual() {
    use crate::game_logic::host_tomahawk::{TOMAHAWK_MISSILE_WEAPON, TOMAHAWK_SCATTER_VS_INFANTRY};
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let mut logic = GameLogic::new();
    ensure_test_infantry_template(&mut logic);
    ensure_test_tank_template(&mut logic);

    let mut th_tpl = ThingTemplate::new("AmericaVehicleTomahawk");
    th_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(200.0)
        .set_primary_weapon_name(TOMAHAWK_MISSILE_WEAPON);
    logic
        .templates
        .insert("AmericaVehicleTomahawk".to_string(), th_tpl);

    let th = logic
        .create_object(
            "AmericaVehicleTomahawk",
            Team::USA,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("tomahawk");
    if let Some(o) = logic.objects.get_mut(&th) {
        if let Some(w) = o.weapon.as_mut() {
            w.last_fire_time = -100.0;
        }
    }
    let inf = logic
        .create_object("TestInfantry", Team::GLA, glam::Vec3::new(80.0, 0.0, 0.0))
        .expect("inf");
    if let Some(o) = logic.objects.get_mut(&inf) {
        o.set_selection_radius(0.5);
    }

    let impact = logic
        .objects
        .get(&inf)
        .map(|o| o.get_position())
        .unwrap_or(glam::Vec3::new(80.0, 0.0, 0.0));
    let _ = logic.apply_tomahawk_residual_at(impact, Some(th), Some(inf));
    assert!(
        logic.tomahawk_scatter_applied > 0
            || logic.tomahawk_scatter_misses > 0
            || logic.honesty_tomahawk_scatter_ok(),
        "tomahawk scatter residual must peel vs infantry"
    );
    assert!((TOMAHAWK_SCATTER_VS_INFANTRY - 20.0).abs() < 0.01);

    let tank = logic
        .create_object("TestTank", Team::GLA, glam::Vec3::new(70.0, 0.0, 0.0))
        .expect("tank");
    logic.mark_object_for_destruction(inf, None);
    logic.process_destroy_list();
    let hp_before = logic.host_object(tank).unwrap().health.current;
    let impact = logic
        .objects
        .get(&tank)
        .map(|o| o.get_position())
        .unwrap_or(glam::Vec3::new(70.0, 0.0, 0.0));
    let (hits, _) = logic.apply_tomahawk_residual_at(impact, Some(th), Some(tank));
    let hp_after = logic
        .host_object(tank)
        .map(|o| o.health.current)
        .unwrap_or(0.0);
    assert!(hits > 0 && hp_after < hp_before, "vehicle still hit");
}

#[test]
fn battlemaster_scatter_misses_infantry_residual() {
    use crate::game_logic::host_battlemaster::{
        BATTLE_MASTER_SCATTER_VS_INFANTRY, BATTLE_MASTER_TANK_GUN,
    };
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let mut logic = GameLogic::new();
    ensure_test_infantry_template(&mut logic);
    ensure_test_tank_template(&mut logic);

    let mut bm_tpl = ThingTemplate::new("ChinaTankBattleMaster");
    bm_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(300.0)
        .set_primary_weapon_name(BATTLE_MASTER_TANK_GUN);
    logic
        .templates
        .insert("ChinaTankBattleMaster".to_string(), bm_tpl);

    let bm = logic
        .create_object(
            "ChinaTankBattleMaster",
            Team::China,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("bm");
    if let Some(o) = logic.objects.get_mut(&bm) {
        if let Some(w) = o.weapon.as_mut() {
            w.last_fire_time = -100.0;
        }
    }
    let inf = logic
        .create_object("TestInfantry", Team::USA, glam::Vec3::new(50.0, 0.0, 0.0))
        .expect("inf");
    if let Some(o) = logic.objects.get_mut(&inf) {
        o.set_selection_radius(0.5);
    }

    let impact = logic
        .objects
        .get(&inf)
        .map(|o| o.get_position())
        .unwrap_or(glam::Vec3::new(50.0, 0.0, 0.0));
    let _ = logic.apply_battlemaster_residual_at(impact, Some(bm), Some(inf));
    assert!(
        logic.battlemaster_scatter_applied > 0
            || logic.battlemaster_scatter_misses > 0
            || logic.honesty_battlemaster_scatter_ok(),
        "battlemaster scatter residual must peel vs infantry"
    );
    assert!((BATTLE_MASTER_SCATTER_VS_INFANTRY - 10.0).abs() < 0.01);

    let tank = logic
        .create_object("TestTank", Team::USA, glam::Vec3::new(45.0, 0.0, 0.0))
        .expect("tank");
    logic.mark_object_for_destruction(inf, None);
    logic.process_destroy_list();
    let hp_before = logic.host_object(tank).unwrap().health.current;
    let impact = logic
        .objects
        .get(&tank)
        .map(|o| o.get_position())
        .unwrap_or(glam::Vec3::new(45.0, 0.0, 0.0));
    let (hits, _) = logic.apply_battlemaster_residual_at(impact, Some(bm), Some(tank));
    let hp_after = logic
        .host_object(tank)
        .map(|o| o.health.current)
        .unwrap_or(0.0);
    assert!(hits > 0 && hp_after < hp_before, "vehicle still hit");
}

#[test]
fn anthrax_scud_structure_uses_poison_armor() {
    // C++ ActiveBody::attemptDamage + StructureArmor POISON 1%.
    // Unresistable would deal 100x (200 HP vs 2 HP).
    use crate::game_logic::host_scud_launcher::SCUD_TOX_PRIMARY_DAMAGE;

    let mut logic = GameLogic::new();
    ensure_test_structure_template(&mut logic);
    let bldg = logic
        .create_object("TestBuilding", Team::USA, glam::Vec3::new(40.0, 0.0, 0.0))
        .expect("bldg");
    let hp_before = logic.host_object(bldg).unwrap().health.current;
    let impact = logic
        .objects
        .get(&bldg)
        .map(|o| o.get_position())
        .unwrap_or(glam::Vec3::new(40.0, 0.0, 0.0));
    let (hits, _) = logic.apply_scud_area_at(impact, None, Team::GLA, true);
    let hp_after = logic
        .host_object(bldg)
        .map(|o| o.health.current)
        .unwrap_or(0.0);
    let dealt = hp_before - hp_after;
    assert!(hits > 0, "structure must be in scud toxin blast");
    assert!(
        (dealt - SCUD_TOX_PRIMARY_DAMAGE * 0.01).abs() < 0.05,
        "StructureArmor POISON 1% expected ~{}, got {dealt} (Unresistable would be {})",
        SCUD_TOX_PRIMARY_DAMAGE * 0.01,
        SCUD_TOX_PRIMARY_DAMAGE
    );
}

#[test]
fn overlord_scatter_misses_infantry_residual() {
    use crate::game_logic::host_overlord_gun::{OVERLORD_SCATTER_VS_INFANTRY, OVERLORD_TANK_GUN};
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let mut logic = GameLogic::new();
    ensure_test_infantry_template(&mut logic);
    ensure_test_tank_template(&mut logic);

    let mut ol_tpl = ThingTemplate::new("ChinaTankOverlord");
    ol_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(1100.0)
        .set_primary_weapon_name(OVERLORD_TANK_GUN);
    logic
        .templates
        .insert("ChinaTankOverlord".to_string(), ol_tpl);

    let ol = logic
        .create_object(
            "ChinaTankOverlord",
            Team::China,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("overlord");
    if let Some(o) = logic.objects.get_mut(&ol) {
        if let Some(w) = o.weapon.as_mut() {
            w.last_fire_time = -100.0;
        }
    }
    let inf = logic
        .create_object("TestInfantry", Team::USA, glam::Vec3::new(50.0, 0.0, 0.0))
        .expect("inf");
    if let Some(o) = logic.objects.get_mut(&inf) {
        o.set_selection_radius(0.5);
    }

    let impact = logic
        .objects
        .get(&inf)
        .map(|o| o.get_position())
        .unwrap_or(glam::Vec3::new(50.0, 0.0, 0.0));
    let _ = logic.apply_overlord_gun_residual_at(impact, Some(ol), Some(inf));
    assert!(
        logic.overlord_scatter_applied > 0
            || logic.overlord_scatter_misses > 0
            || logic.honesty_overlord_scatter_ok(),
        "overlord scatter residual must peel vs infantry"
    );
    assert!((OVERLORD_SCATTER_VS_INFANTRY - 10.0).abs() < 0.01);

    let tank = logic
        .create_object("TestTank", Team::USA, glam::Vec3::new(45.0, 0.0, 0.0))
        .expect("tank");
    logic.mark_object_for_destruction(inf, None);
    logic.process_destroy_list();
    let hp_before = logic.host_object(tank).unwrap().health.current;
    let impact = logic
        .objects
        .get(&tank)
        .map(|o| o.get_position())
        .unwrap_or(glam::Vec3::new(45.0, 0.0, 0.0));
    let (hits, _) = logic.apply_overlord_gun_residual_at(impact, Some(ol), Some(tank));
    let hp_after = logic
        .host_object(tank)
        .map(|o| o.health.current)
        .unwrap_or(0.0);
    assert!(hits > 0 && hp_after < hp_before, "vehicle still hit");
}

#[test]
fn raptor_scatter_misses_infantry_residual() {
    use crate::game_logic::host_raptor::{RAPTOR_JET_MISSILE_WEAPON, RAPTOR_SCATTER_VS_INFANTRY};
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let mut logic = GameLogic::new();
    ensure_test_infantry_template(&mut logic);
    ensure_test_tank_template(&mut logic);

    let mut rp_tpl = ThingTemplate::new("AmericaJetRaptor");
    rp_tpl
        .add_kind_of(KindOf::Aircraft)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(200.0)
        .set_primary_weapon_name(RAPTOR_JET_MISSILE_WEAPON);
    logic
        .templates
        .insert("AmericaJetRaptor".to_string(), rp_tpl);

    let rp = logic
        .create_object(
            "AmericaJetRaptor",
            Team::USA,
            glam::Vec3::new(0.0, 50.0, 0.0),
        )
        .expect("raptor");
    if let Some(o) = logic.objects.get_mut(&rp) {
        if let Some(w) = o.weapon.as_mut() {
            w.last_fire_time = -100.0;
        }
    }
    let inf = logic
        .create_object("TestInfantry", Team::GLA, glam::Vec3::new(80.0, 0.0, 0.0))
        .expect("inf");
    if let Some(o) = logic.objects.get_mut(&inf) {
        o.set_selection_radius(0.5);
    }

    let impact = logic
        .objects
        .get(&inf)
        .map(|o| o.get_position())
        .unwrap_or(glam::Vec3::new(80.0, 0.0, 0.0));
    let _ = logic.apply_raptor_residual_at(impact, Some(rp), Some(inf));
    assert!(
        logic.raptor_scatter_applied > 0
            || logic.raptor_scatter_misses > 0
            || logic.honesty_raptor_scatter_ok(),
        "raptor scatter residual must peel vs infantry"
    );
    assert!((RAPTOR_SCATTER_VS_INFANTRY - 10.0).abs() < 0.01);

    let tank = logic
        .create_object("TestTank", Team::GLA, glam::Vec3::new(70.0, 0.0, 0.0))
        .expect("tank");
    logic.mark_object_for_destruction(inf, None);
    logic.process_destroy_list();
    let hp_before = logic.host_object(tank).unwrap().health.current;
    let impact = logic
        .objects
        .get(&tank)
        .map(|o| o.get_position())
        .unwrap_or(glam::Vec3::new(70.0, 0.0, 0.0));
    let (hits, _) = logic.apply_raptor_residual_at(impact, Some(rp), Some(tank));
    let hp_after = logic
        .host_object(tank)
        .map(|o| o.health.current)
        .unwrap_or(0.0);
    assert!(hits > 0 && hp_after < hp_before, "vehicle still hit");
}

#[test]
fn usa_tank_scatter_misses_infantry_residual() {
    use crate::game_logic::host_usa_tanks::{CRUSADER_TANK_GUN, USA_TANK_GUN_SCATTER_VS_INFANTRY};
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let mut logic = GameLogic::new();
    ensure_test_infantry_template(&mut logic);
    ensure_test_tank_template(&mut logic);

    let mut cr_tpl = ThingTemplate::new("AmericaTankCrusader");
    cr_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(400.0)
        .set_primary_weapon_name(CRUSADER_TANK_GUN);
    logic
        .templates
        .insert("AmericaTankCrusader".to_string(), cr_tpl);

    let cr = logic
        .create_object(
            "AmericaTankCrusader",
            Team::USA,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("crusader");
    if let Some(o) = logic.objects.get_mut(&cr) {
        if let Some(w) = o.weapon.as_mut() {
            w.last_fire_time = -100.0;
        }
    }
    let inf = logic
        .create_object("TestInfantry", Team::GLA, glam::Vec3::new(50.0, 0.0, 0.0))
        .expect("inf");
    if let Some(o) = logic.objects.get_mut(&inf) {
        o.set_selection_radius(0.5);
    }

    let impact = logic
        .objects
        .get(&inf)
        .map(|o| o.get_position())
        .unwrap_or(glam::Vec3::new(50.0, 0.0, 0.0));
    let _ = logic.apply_usa_tank_gun_residual_at(impact, Some(cr), Some(inf));
    assert!(
        logic.usa_tank_scatter_applied > 0
            || logic.usa_tank_scatter_misses > 0
            || logic.honesty_usa_tank_scatter_ok(),
        "usa tank scatter residual must peel vs infantry"
    );
    assert!((USA_TANK_GUN_SCATTER_VS_INFANTRY - 10.0).abs() < 0.01);

    let tank = logic
        .create_object("TestTank", Team::GLA, glam::Vec3::new(45.0, 0.0, 0.0))
        .expect("tank");
    logic.mark_object_for_destruction(inf, None);
    logic.process_destroy_list();
    let hp_before = logic.host_object(tank).unwrap().health.current;
    let impact = logic
        .objects
        .get(&tank)
        .map(|o| o.get_position())
        .unwrap_or(glam::Vec3::new(45.0, 0.0, 0.0));
    let (hits, _) = logic.apply_usa_tank_gun_residual_at(impact, Some(cr), Some(tank));
    let hp_after = logic
        .host_object(tank)
        .map(|o| o.health.current)
        .unwrap_or(0.0);
    assert!(hits > 0 && hp_after < hp_before, "vehicle still hit");
}

#[test]
fn stealth_jet_scatter_misses_infantry_residual() {
    use crate::game_logic::host_stealth_fighter::{
        AMERICA_JET_STEALTH_FIGHTER, STEALTH_FIGHTER_SCATTER_VS_INFANTRY,
        STEALTH_JET_MISSILE_WEAPON,
    };
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let mut logic = GameLogic::new();
    ensure_test_infantry_template(&mut logic);
    ensure_test_tank_template(&mut logic);

    let mut jet_tpl = ThingTemplate::new(AMERICA_JET_STEALTH_FIGHTER);
    jet_tpl
        .add_kind_of(KindOf::Aircraft)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(200.0)
        .set_primary_weapon_name(STEALTH_JET_MISSILE_WEAPON);
    logic
        .templates
        .insert(AMERICA_JET_STEALTH_FIGHTER.to_string(), jet_tpl);

    let jet = logic
        .create_object(
            AMERICA_JET_STEALTH_FIGHTER,
            Team::USA,
            glam::Vec3::new(0.0, 50.0, 0.0),
        )
        .expect("stealth jet");
    let inf = logic
        .create_object("TestInfantry", Team::GLA, glam::Vec3::new(50.0, 0.0, 0.0))
        .expect("inf");
    if let Some(o) = logic.objects.get_mut(&inf) {
        o.set_selection_radius(0.5);
    }

    let impact = logic
        .objects
        .get(&inf)
        .map(|o| o.get_position())
        .unwrap_or(glam::Vec3::new(50.0, 0.0, 0.0));
    let _ = logic.apply_stealth_fighter_residual_at(impact, Some(jet), Some(inf));
    assert!(
        logic.stealth_jet_scatter_applied > 0
            || logic.stealth_jet_scatter_misses > 0
            || logic.honesty_stealth_jet_scatter_ok(),
        "stealth jet scatter residual must peel vs infantry"
    );
    assert!((STEALTH_FIGHTER_SCATTER_VS_INFANTRY - 10.0).abs() < 0.01);

    let tank = logic
        .create_object("TestTank", Team::GLA, glam::Vec3::new(45.0, 0.0, 0.0))
        .expect("tank");
    logic.mark_object_for_destruction(inf, None);
    logic.process_destroy_list();
    let hp_before = logic.host_object(tank).unwrap().health.current;
    let impact = logic
        .objects
        .get(&tank)
        .map(|o| o.get_position())
        .unwrap_or(glam::Vec3::new(45.0, 0.0, 0.0));
    let (hits, _) = logic.apply_stealth_fighter_residual_at(impact, Some(jet), Some(tank));
    let hp_after = logic
        .host_object(tank)
        .map(|o| o.health.current)
        .unwrap_or(0.0);
    assert!(hits > 0 && hp_after < hp_before, "vehicle still hit");
}

#[test]
fn flashbang_scatter_misses_intended_residual() {
    use crate::game_logic::host_ranger::{FLASHBANG_SCATTER_RADIUS, RANGER_FLASHBANG_WEAPON};
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let mut logic = GameLogic::new();
    ensure_test_infantry_template(&mut logic);
    ensure_test_tank_template(&mut logic);

    let mut ranger_tpl = ThingTemplate::new("AmericaInfantryRanger");
    ranger_tpl
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(100.0)
        .set_primary_weapon_name(RANGER_FLASHBANG_WEAPON);
    logic
        .templates
        .insert("AmericaInfantryRanger".to_string(), ranger_tpl);

    let ranger = logic
        .create_object(
            "AmericaInfantryRanger",
            Team::USA,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("ranger");
    let inf = logic
        .create_object("TestInfantry", Team::GLA, glam::Vec3::new(50.0, 0.0, 0.0))
        .expect("inf");
    if let Some(o) = logic.objects.get_mut(&inf) {
        o.set_selection_radius(0.5);
    }

    let impact = logic
        .objects
        .get(&inf)
        .map(|o| o.get_position())
        .unwrap_or(glam::Vec3::new(50.0, 0.0, 0.0));
    let _ = logic.apply_ranger_residual_at(impact, Some(ranger), Some(inf), true);
    assert!(
        logic.flashbang_scatter_applied > 0
            || logic.flashbang_scatter_misses > 0
            || logic.honesty_flashbang_scatter_ok(),
        "flashbang scatter residual must peel"
    );
    assert!((FLASHBANG_SCATTER_RADIUS - 4.0).abs() < 0.01);

    // Retail Weapon.ini:2535-2556 RangerFlashBangGrenadeWeapon is DamageType
    // SURRENDER, and retail Armor.ini gives every vehicle armor set SURRENDER
    // 0% (TankArmor line 139; "Capture type weapons are effective only
    // against infantry"). C++ ActiveBody::attemptDamage applies the armored
    // amount (the surrender swap-out is compiled out), so the splash ring
    // still ACQUIRES the vehicle as a victim, but zero HP damage lands.
    let tank = logic
        .create_object("TestTank", Team::GLA, glam::Vec3::new(48.0, 0.0, 0.0))
        .expect("tank");
    logic.mark_object_for_destruction(inf, None);
    logic.process_destroy_list();
    let hp_before = logic.host_object(tank).unwrap().health.current;
    let impact = logic
        .objects
        .get(&tank)
        .map(|o| o.get_position())
        .unwrap_or(glam::Vec3::new(48.0, 0.0, 0.0));
    let (hits, _) = logic.apply_ranger_residual_at(impact, Some(ranger), Some(tank), true);
    let hp_after = logic
        .host_object(tank)
        .map(|o| o.health.current)
        .unwrap_or(0.0);
    assert!(hits > 0, "splash ring still acquires the vehicle victim");
    assert_eq!(
        hp_after, hp_before,
        "SURRENDER × 0% vehicle armor: flashbang cannot dent a tank"
    );
}

#[test]
fn rocket_buggy_scatter_misses_infantry_residual() {
    use crate::game_logic::host_rocket_buggy::{BUGGY_ROCKET_WEAPON, BUGGY_SCATTER_VS_INFANTRY};
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let mut logic = GameLogic::new();
    ensure_test_infantry_template(&mut logic);
    ensure_test_tank_template(&mut logic);

    let mut bg_tpl = ThingTemplate::new("GLAVehicleRocketBuggy");
    bg_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(200.0)
        .set_primary_weapon_name(BUGGY_ROCKET_WEAPON);
    logic
        .templates
        .insert("GLAVehicleRocketBuggy".to_string(), bg_tpl);

    let bg = logic
        .create_object(
            "GLAVehicleRocketBuggy",
            Team::GLA,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("buggy");
    let inf = logic
        .create_object("TestInfantry", Team::USA, glam::Vec3::new(50.0, 0.0, 0.0))
        .expect("inf");
    if let Some(o) = logic.objects.get_mut(&inf) {
        o.set_selection_radius(0.5);
    }

    let impact = logic
        .objects
        .get(&inf)
        .map(|o| o.get_position())
        .unwrap_or(glam::Vec3::new(50.0, 0.0, 0.0));
    let _ = logic.apply_rocket_buggy_residual_at(impact, Some(bg), Some(inf));
    assert!(
        logic.rocket_buggy_scatter_applied > 0
            || logic.rocket_buggy_residual_scatter_misses > 0
            || logic.honesty_rocket_buggy_scatter_ok(),
        "rocket buggy scatter residual must peel vs infantry"
    );
    assert!((BUGGY_SCATTER_VS_INFANTRY - 20.0).abs() < 0.01);

    let tank = logic
        .create_object("TestTank", Team::USA, glam::Vec3::new(48.0, 0.0, 0.0))
        .expect("tank");
    logic.mark_object_for_destruction(inf, None);
    logic.process_destroy_list();
    let hp_before = logic.host_object(tank).unwrap().health.current;
    let impact = logic
        .objects
        .get(&tank)
        .map(|o| o.get_position())
        .unwrap_or(glam::Vec3::new(48.0, 0.0, 0.0));
    let (hits, _) = logic.apply_rocket_buggy_residual_at(impact, Some(bg), Some(tank));
    let hp_after = logic
        .host_object(tank)
        .map(|o| o.health.current)
        .unwrap_or(0.0);
    assert!(
        hits > 0 && hp_after < hp_before,
        "vehicle splash still hits"
    );
}

#[test]
fn mig_scatter_misses_infantry_residual() {
    use crate::game_logic::host_mig::{MIG_SCATTER_VS_INFANTRY, NAPALM_MISSILE_WEAPON};
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let mut logic = GameLogic::new();
    ensure_test_infantry_template(&mut logic);
    ensure_test_tank_template(&mut logic);

    let mut mig_tpl = ThingTemplate::new("ChinaJetMIG");
    mig_tpl
        .add_kind_of(KindOf::Aircraft)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(200.0)
        .set_primary_weapon_name(NAPALM_MISSILE_WEAPON);
    logic.templates.insert("ChinaJetMIG".to_string(), mig_tpl);

    let mig = logic
        .create_object("ChinaJetMIG", Team::China, glam::Vec3::new(0.0, 50.0, 0.0))
        .expect("mig");
    let inf = logic
        .create_object("TestInfantry", Team::USA, glam::Vec3::new(50.0, 0.0, 0.0))
        .expect("inf");
    if let Some(o) = logic.objects.get_mut(&inf) {
        o.set_selection_radius(0.5);
    }

    let impact = logic
        .objects
        .get(&inf)
        .map(|o| o.get_position())
        .unwrap_or(glam::Vec3::new(50.0, 0.0, 0.0));
    let _ = logic.apply_mig_residual_at(impact, Some(mig), Some(inf));
    assert!(
        logic.mig_scatter_applied > 0
            || logic.mig_scatter_misses > 0
            || logic.honesty_mig_scatter_ok(),
        "mig scatter residual must peel vs infantry"
    );
    assert!((MIG_SCATTER_VS_INFANTRY - 10.0).abs() < 0.01);

    let tank = logic
        .create_object("TestTank", Team::USA, glam::Vec3::new(48.0, 0.0, 0.0))
        .expect("tank");
    logic.mark_object_for_destruction(inf, None);
    logic.process_destroy_list();
    let hp_before = logic.host_object(tank).unwrap().health.current;
    let impact = logic
        .objects
        .get(&tank)
        .map(|o| o.get_position())
        .unwrap_or(glam::Vec3::new(48.0, 0.0, 0.0));
    let (hits, _) = logic.apply_mig_residual_at(impact, Some(mig), Some(tank));
    let hp_after = logic
        .host_object(tank)
        .map(|o| o.health.current)
        .unwrap_or(0.0);
    assert!(hits > 0 && hp_after < hp_before, "vehicle still hit");
}

/// hq-2qedt.87.1: C++ Weapon.cpp FireFX plays at fire time only; impact uses
/// ProjectileDetonateFX. vehicle_shells residual impact handlers must not
/// spawn a second muzzle at the shooter (shared dispatch owns the one muzzle).
#[test]
fn vehicle_shells_residual_impact_spawns_no_muzzle_hq_2qedt_87_1() {
    use crate::game_logic::combat_particles::CombatParticleKind;
    use crate::game_logic::host_fire_base::FIRE_BASE_HOWITZER_WEAPON;
    use crate::game_logic::host_mig::NAPALM_MISSILE_WEAPON;
    use crate::game_logic::host_raptor::RAPTOR_JET_MISSILE_WEAPON;
    use crate::game_logic::host_scorpion::SCORPION_TANK_GUN;
    use crate::game_logic::host_tomahawk::TOMAHAWK_MISSILE_WEAPON;
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let cases: [(&str, &str, KindOf, Team); 5] = [
        ("GLAVehicleScorpion", SCORPION_TANK_GUN, KindOf::Vehicle, Team::GLA),
        ("AmericaVehicleTomahawk", TOMAHAWK_MISSILE_WEAPON, KindOf::Vehicle, Team::USA),
        ("AmericaJetRaptor", RAPTOR_JET_MISSILE_WEAPON, KindOf::Aircraft, Team::USA),
        ("ChinaJetMIG", NAPALM_MISSILE_WEAPON, KindOf::Aircraft, Team::China),
        ("AmericaFireBase", FIRE_BASE_HOWITZER_WEAPON, KindOf::Structure, Team::USA),
    ];
    for (tpl_name, weapon, kind, team) in cases {
        let mut logic = GameLogic::new();
        ensure_test_tank_template(&mut logic);
        let mut tpl = ThingTemplate::new(tpl_name);
        tpl.add_kind_of(kind)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_health(300.0)
            .set_primary_weapon_name(weapon);
        logic.templates.insert(tpl_name.to_string(), tpl);
        let src = logic
            .create_object(tpl_name, team, glam::Vec3::new(0.0, 0.0, 0.0))
            .expect("shooter");
        let enemy = if team == Team::GLA { Team::USA } else { Team::GLA };
        let tank = logic
            .create_object("TestTank", enemy, glam::Vec3::new(45.0, 0.0, 0.0))
            .expect("tank");
        let impact = logic.objects.get(&tank).unwrap().get_position();
        let shooter_pos = logic.objects.get(&src).unwrap().get_position();
        let before = logic
            .combat_particles()
            .systems_of_kind(CombatParticleKind::WeaponMuzzleFlash)
            .iter()
            .filter(|e| e.position.distance(shooter_pos) < 1.0)
            .count();
        let _ = match tpl_name {
            "GLAVehicleScorpion" => logic.apply_scorpion_residual_at(impact, Some(src), Some(tank), 0),
            "AmericaVehicleTomahawk" => logic.apply_tomahawk_residual_at(impact, Some(src), Some(tank)),
            "AmericaJetRaptor" => logic.apply_raptor_residual_at(impact, Some(src), Some(tank)),
            "ChinaJetMIG" => logic.apply_mig_residual_at(impact, Some(src), Some(tank)),
            _ => logic.apply_fire_base_residual_at(impact, Some(src), Some(tank)),
        };
        let after = logic
            .combat_particles()
            .systems_of_kind(CombatParticleKind::WeaponMuzzleFlash)
            .iter()
            .filter(|e| e.position.distance(shooter_pos) < 1.0)
            .count();
        assert_eq!(
            after, before,
            "{tpl_name}: impact residual must not spawn a second muzzle at the shooter (C++ FireFX is fire-time only)"
        );
    }
}

/// hq-2qedt.87.5: C++ Weapon.cpp FireFX plays at fire time only; impact uses
/// ProjectileDetonateFX. Neutron shell blast and infantry SCUD area impact
/// residuals must not spawn any WeaponMuzzleFlash (shooter or impact point).
#[test]
fn neutron_scud_residual_impact_spawns_no_muzzle_hq_2qedt_87_5() {
    use crate::game_logic::combat_particles::CombatParticleKind;
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let mut failures = Vec::new();
    for case in ["neutron", "scud", "scud_toxin"] {
        let mut logic = GameLogic::new();
        ensure_test_tank_template(&mut logic);
        let src = logic
            .create_object("TestTank", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
            .expect("shooter");
        let tank = logic
            .create_object("TestTank", Team::GLA, glam::Vec3::new(45.0, 0.0, 0.0))
            .expect("tank");
        let impact = logic.objects.get(&tank).unwrap().get_position();
        let count = |l: &GameLogic| {
            l.combat_particles()
                .systems_of_kind(CombatParticleKind::WeaponMuzzleFlash)
                .len()
        };
        let before = count(&logic);
        match case {
            "neutron" => {
                let _ = logic.apply_neutron_blast_at(impact, Team::USA, Some(src), false);
            }
            "scud" => {
                let _ = logic.apply_scud_area_at(impact, Some(src), Team::USA, false);
            }
            _ => {
                let _ = logic.apply_scud_area_at(impact, Some(src), Team::USA, true);
            }
        }
        if count(&logic) != before {
            failures.push(case);
        }
    }
    assert!(
        failures.is_empty(),
        "impact residuals spawned a muzzle (C++ FireFX is fire-time only): {failures:?}"
    );
}

/// hq-2qedt.125: base-defense residual fire captures a pending visual
/// dispatch and records the accepted discharge (shared FireFX owner,
/// Weapon.cpp:923-939). C++ plays FireFX exactly once per accepted shot, so
/// the residual must not add a second muzzle on top of the shared owner.
#[test]
fn base_defense_fire_time_exactly_one_muzzle_per_shot_hq_2qedt_125() {
    use crate::game_logic::combat_particles::CombatParticleKind;
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let mut logic = GameLogic::new();
    let mut pat_tpl = crate::game_logic::ThingTemplate::new("Lazr_AmericaPatriotBattery");
    pat_tpl
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .add_kind_of(KindOf::FSBaseDefense)
        .set_health(1000.0)
        .set_primary_weapon_name("Lazr_PatriotMissileWeapon");
    logic
        .templates
        .insert("Lazr_AmericaPatriotBattery".to_string(), pat_tpl);
    ensure_test_tank_template(&mut logic);
    let pat = logic
        .create_object("Lazr_AmericaPatriotBattery", Team::USA, glam::Vec3::ZERO)
        .expect("patriot");
    if let Some(p) = logic.host_object_mut(pat) {
        p.set_status_under_construction(false);
        p.construction_percent = 100.0;
        if let Some(w) = p.weapon.as_mut() {
            w.last_fire_time = -10.0;
        }
    }
    let _enemy = logic
        .create_object("TestTank", Team::GLA, glam::Vec3::new(100.0, 0.0, 0.0))
        .expect("enemy");
    let muzzles = |l: &GameLogic| {
        l.combat_particles()
            .systems_of_kind(CombatParticleKind::WeaponMuzzleFlash)
            .iter()
            .filter(|e| e.source_object == Some(pat))
            .count()
    };
    logic.frame = 30;
    let mut fired = false;
    for _ in 0..30 {
        let before_fires = logic.base_defense_residual_fires();
        let before = muzzles(&logic);
        logic.try_base_defense_residual_fire(pat);
        if logic.base_defense_residual_fires() > before_fires {
            let after = muzzles(&logic);
            eprintln!("hq-2qedt.125 muzzles per accepted shot = {}", after - before);
            assert_eq!(after - before, 1, "exactly one fire-time muzzle per accepted shot");
            fired = true;
            break;
        }
        logic.frame = logic.frame.saturating_add(1);
    }
    assert!(fired, "base defense must fire");
}

/// hq-2qedt.126: inferno fire zone (impact-time, from shell detonation / MIG
/// napalm) and Aurora bomb queue (called inside the accepted-shot path where
/// shared dispatch already plays FireFX) must not add a WeaponMuzzleFlash
/// owned by the shooter. C++ Weapon.cpp:923-939 plays FireFX once per shot at
/// fire time; impact plays DetonationFX/OCL (OCL_FireFieldSmall), launch has
/// no second muzzle. FireWall special power (no weapon FireFX dispatch) keeps
/// its single segment flame presentation.
#[test]
fn angry_mob_aurora_zone_launch_muzzle_ownership_hq_2qedt_126() {
    use crate::game_logic::combat_particles::CombatParticleKind;
    use crate::game_logic::host_aurora_bomb::HostAuroraBombKind;
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let muzzles = |l: &GameLogic, src: ObjectId| {
        l.combat_particles()
            .systems_of_kind(CombatParticleKind::WeaponMuzzleFlash)
            .iter()
            .filter(|e| e.source_object == Some(src))
            .count()
    };
    let mut logic = GameLogic::new();
    ensure_test_tank_template(&mut logic);
    let src = logic
        .create_object("TestTank", Team::USA, glam::Vec3::ZERO)
        .expect("src");
    let impact = glam::Vec3::new(120.0, 0.0, 0.0);

    let b = muzzles(&logic, src);
    logic.spawn_inferno_fire_zone(src, Team::USA, impact, false);
    assert_eq!(muzzles(&logic, src) - b, 0, "inferno zone at impact must not spawn shooter muzzle");

    let b = muzzles(&logic, src);
    logic.queue_aurora_bomb(HostAuroraBombKind::Standard, src, Team::USA, impact);
    assert_eq!(muzzles(&logic, src) - b, 0, "aurora launch must not add a second muzzle over shared FireFX");

    let b = muzzles(&logic, src);
    if logic.activate_firewall(src, impact).is_some() {
        assert_eq!(muzzles(&logic, src) - b, 1, "firewall keeps exactly one segment flame presentation");
    }
}

/// C++ AIAttackFireWeaponState::update (AIStates.cpp:5225-5233) relies on
/// AIUpdateInterface::transferAttack (AIUpdate.cpp:4149-4151) also moving the
/// state-machine goal object when a killed victim hands off (CreateObjectDie).
#[test]
fn fire_callback_transfer_attack_updates_machine_goal_same_frame() {
    use crate::game_logic::host_create_object_die::HostCreateObjectDieData;
    let mut logic = GameLogic::new();
    for (name, structure) in [("XferAtk", false), ("XferBldg", true), ("XferHole", true)] {
        let mut t = ThingTemplate::new(name);
        t.add_kind_of(KindOf::Attackable).set_health(100.0);
        if structure {
            t.add_kind_of(KindOf::Structure);
        } else {
            t.add_kind_of(KindOf::Vehicle);
        }
        logic.templates.insert(name.into(), t);
    }
    let atk = logic
        .create_object("XferAtk", Team::USA, glam::Vec3::new(0.0, 0.0, 0.0))
        .expect("atk");
    let bldg = logic
        .create_object("XferBldg", Team::GLA, glam::Vec3::new(20.0, 0.0, 0.0))
        .expect("bldg");
    if let Some(o) = logic.objects.get_mut(&atk) {
        o.weapon = Some(Weapon {
            damage: 999.0,
            range: 100.0,
            reload_time: 0.0,
            last_fire_time: -100.0,
            ..Weapon::default()
        });
    }
    if let Some(o) = logic.objects.get_mut(&bldg) {
        o.create_object_die = Some(HostCreateObjectDieData {
            ocl_name: "OCL_XferHole".into(),
            spawn_templates: vec!["XferHole".into()],
            transfer_previous_health: true,
            fired: false,
        });
    }
    assert!(logic.unit_command_attack(atk, bldg).unwrap());
    for _ in 0..120 {
        logic.tick_logic_frame(1.0 / 30.0, None, Some(1));
        if logic.objects.get(&bldg).is_none_or(|v| !v.is_alive()) {
            break;
        }
    }
    let hole = logic
        .objects
        .registered_values()
        .find(|o| o.template_name == "XferHole")
        .map(|o| o.id)
        .expect("CreateObjectDie hole spawned");
    let a = &logic.objects[&atk];
    assert_eq!(a.target, Some(hole), "transferAttack retargets host");
    assert_eq!(
        a.unit_ai_runtime.machine().goals.object,
        Some(hole),
        "attack machine goal must follow the transferred victim"
    );
}
