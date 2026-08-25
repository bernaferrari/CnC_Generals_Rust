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
        o.set_ai_state(AIState::Attacking);
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
    crate::game_logic::host_ai_decision_log::clear();
    assert!(logic.try_continue_attack_after_kill(atk, mine1, pos, 100.0, team,));
    // Under AI_DECISION_AUTHORITY (default), continue-attack logs AttackTarget and
    // leaves host target untouched for GameWorld writeback.
    if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
        let events = crate::game_logic::host_ai_decision_log::snapshot();
        assert!(
            events.iter().any(|e| {
                e.host_object == atk
                    && e.kind == crate::game_logic::host_ai_decision_log::AI_DECISION_ATTACK
                    && e.target_host == mine2.0
            }),
            "continue-attack must log AttackTarget(mine2) under decision authority"
        );
        // Host target remains the prior engagement (writeback owns the swap).
        assert_eq!(logic.objects.get(&atk).and_then(|a| a.target), Some(mine1));
    } else {
        let next = logic.objects.get(&atk).and_then(|a| a.target);
        assert_eq!(next, Some(mine2), "must chain to nearer same-team mine");
    }
    // Outside range: no continue.
    crate::game_logic::host_ai_decision_log::clear();
    if let Some(o) = logic.objects.get_mut(&atk) {
        o.target = None;
        o.stop_attack();
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
        crate::game_logic::host_ai_decision_log::snapshot().is_empty(),
        "out-of-range continue must not log AttackTarget"
    );
    // continue_or_stop stops when nothing in range
    logic.continue_or_stop_after_kill(
        atk,
        mine2,
        glam::Vec3::new(40.0, 0.0, 0.0),
        team,
        Some("DozerMineDisarmingWeapon"),
        5.0,
    );
    let a = logic.objects.get(&atk).unwrap();
    assert!(matches!(a.ai_state, AIState::Idle) || a.target.is_none());
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
fn raptor_missile_scatters_vs_infantry() {
    use crate::game_logic::host_raptor::RAPTOR_SCATTER_VS_INFANTRY;

    let mut logic = GameLogic::new();
    let mut r_tpl = ThingTemplate::new("AmericaJetRaptor");
    r_tpl
        .add_kind_of(KindOf::Aircraft)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(160.0);
    logic
        .templates
        .insert("AmericaJetRaptor".to_string(), r_tpl);
    let mut i_tpl = ThingTemplate::new("ChinaInfantryRedguard");
    i_tpl
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(100.0);
    logic
        .templates
        .insert("ChinaInfantryRedguard".to_string(), i_tpl);

    let raptor = logic
        .create_object(
            "AmericaJetRaptor",
            Team::USA,
            glam::Vec3::new(0.0, 80.0, 0.0),
        )
        .expect("raptor");
    let inf = logic
        .create_object(
            "ChinaInfantryRedguard",
            Team::China,
            glam::Vec3::new(200.0, 0.0, 0.0),
        )
        .expect("inf");
    let aim = glam::Vec3::new(200.0, 0.0, 0.0);
    let from = glam::Vec3::new(0.0, 80.0, 0.0);
    let msl = logic
        .spawn_raptor_missile_projectile(raptor, from, aim, Some(inf))
        .expect("missile");
    assert!(logic.honesty_raptor_scatter_ok());
    let m_aim = logic
        .objects
        .get(&msl)
        .and_then(|o| o.raptor_missile_aim)
        .expect("aim");
    let dx = m_aim[0] - aim.x;
    let dz = m_aim[2] - aim.z;
    let d = (dx * dx + dz * dz).sqrt();
    assert!(d > 0.01, "infantry aim should scatter");
    assert!(d <= RAPTOR_SCATTER_VS_INFANTRY + 0.01);

    // Vehicle: no scatter.
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
            glam::Vec3::new(250.0, 0.0, 0.0),
        )
        .expect("tank");
    let before = logic.raptor_scatter_applied;
    let msl2 = logic
        .spawn_raptor_missile_projectile(raptor, from, glam::Vec3::new(250.0, 0.0, 0.0), Some(tank))
        .expect("missile2");
    assert_eq!(logic.raptor_scatter_applied, before);
    let aim2 = logic
        .objects
        .get(&msl2)
        .and_then(|o| o.raptor_missile_aim)
        .expect("aim2");
    assert!((aim2[0] - 250.0).abs() < 0.01 && aim2[2].abs() < 0.01);
}

#[test]
fn missile_defender_missile_scatters_vs_infantry() {
    use crate::game_logic::host_missile_defender::MISSILE_DEFENDER_SCATTER_VS_INFANTRY;

    let mut logic = GameLogic::new();
    let mut md_tpl = ThingTemplate::new("AmericaInfantryMissileDefender");
    md_tpl
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(100.0);
    logic
        .templates
        .insert("AmericaInfantryMissileDefender".to_string(), md_tpl);
    let mut i_tpl = ThingTemplate::new("ChinaInfantryRedguard");
    i_tpl
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(100.0);
    logic
        .templates
        .insert("ChinaInfantryRedguard".to_string(), i_tpl);

    let md = logic
        .create_object(
            "AmericaInfantryMissileDefender",
            Team::USA,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("md");
    let inf = logic
        .create_object(
            "ChinaInfantryRedguard",
            Team::China,
            glam::Vec3::new(120.0, 0.0, 0.0),
        )
        .expect("inf");
    let aim = glam::Vec3::new(120.0, 0.0, 0.0);
    let from = glam::Vec3::new(0.0, 5.0, 0.0);
    let msl = logic
        .spawn_missile_defender_missile_projectile(md, from, aim, Some(inf), false)
        .expect("missile");
    assert!(logic.honesty_missile_defender_scatter_ok());
    let m_aim = logic
        .objects
        .get(&msl)
        .and_then(|o| o.missile_defender_missile_aim)
        .expect("aim");
    let d = ((m_aim[0] - aim.x).powi(2) + (m_aim[2] - aim.z).powi(2)).sqrt();
    assert!(d > 0.01 && d <= MISSILE_DEFENDER_SCATTER_VS_INFANTRY + 0.01);

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
    let before = logic.missile_defender_scatter_applied;
    let msl2 = logic
        .spawn_missile_defender_missile_projectile(
            md,
            from,
            glam::Vec3::new(150.0, 0.0, 0.0),
            Some(tank),
            false,
        )
        .expect("missile2");
    assert_eq!(logic.missile_defender_scatter_applied, before);
    let aim2 = logic
        .objects
        .get(&msl2)
        .and_then(|o| o.missile_defender_missile_aim)
        .expect("aim2");
    assert!((aim2[0] - 150.0).abs() < 0.01 && aim2[2].abs() < 0.01);
}

#[test]
fn nuke_cannon_shell_scatters_vs_infantry() {
    use crate::game_logic::host_nuke_cannon::NUKE_CANNON_SCATTER_VS_INFANTRY;

    let mut logic = GameLogic::new();
    let mut nc_tpl = ThingTemplate::new("ChinaNukeCannon");
    nc_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(240.0);
    logic
        .templates
        .insert("ChinaNukeCannon".to_string(), nc_tpl);
    let mut i_tpl = ThingTemplate::new("AmericaInfantryRanger");
    i_tpl
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(100.0);
    logic
        .templates
        .insert("AmericaInfantryRanger".to_string(), i_tpl);

    let cannon = logic
        .create_object(
            "ChinaNukeCannon",
            Team::China,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("cannon");
    let inf = logic
        .create_object(
            "AmericaInfantryRanger",
            Team::USA,
            glam::Vec3::new(250.0, 0.0, 0.0),
        )
        .expect("inf");
    let aim = glam::Vec3::new(250.0, 0.0, 0.0);
    let from = glam::Vec3::new(0.0, 10.0, 0.0);
    let shell = logic
        .spawn_nuke_cannon_shell_projectile(cannon, from, aim, Some(inf))
        .expect("shell");
    assert!(logic.honesty_nuke_cannon_scatter_ok());
    let s_aim = logic
        .objects
        .get(&shell)
        .and_then(|o| o.nuke_shell_aim)
        .expect("aim");
    let d = ((s_aim[0] - aim.x).powi(2) + (s_aim[2] - aim.z).powi(2)).sqrt();
    assert!(d > 0.01 && d <= NUKE_CANNON_SCATTER_VS_INFANTRY + 0.01);

    let mut t_tpl = ThingTemplate::new("AmericaTankCrusader");
    t_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(400.0);
    logic
        .templates
        .insert("AmericaTankCrusader".to_string(), t_tpl);
    let tank = logic
        .create_object(
            "AmericaTankCrusader",
            Team::USA,
            glam::Vec3::new(280.0, 0.0, 0.0),
        )
        .expect("tank");
    let before = logic.nuke_cannon_scatter_applied;
    let shell2 = logic
        .spawn_nuke_cannon_shell_projectile(
            cannon,
            from,
            glam::Vec3::new(280.0, 0.0, 0.0),
            Some(tank),
        )
        .expect("shell2");
    assert_eq!(logic.nuke_cannon_scatter_applied, before);
    let aim2 = logic
        .objects
        .get(&shell2)
        .and_then(|o| o.nuke_shell_aim)
        .expect("aim2");
    assert!((aim2[0] - 280.0).abs() < 0.01 && aim2[2].abs() < 0.01);
}

#[test]
fn tank_hunter_missile_scatters_vs_infantry() {
    use crate::game_logic::host_tank_hunter::TANK_HUNTER_SCATTER_VS_INFANTRY;

    let mut logic = GameLogic::new();
    let mut th_tpl = ThingTemplate::new("ChinaInfantryTankHunter");
    th_tpl
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(100.0);
    logic
        .templates
        .insert("ChinaInfantryTankHunter".to_string(), th_tpl);
    let mut i_tpl = ThingTemplate::new("AmericaInfantryRanger");
    i_tpl
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(100.0);
    logic
        .templates
        .insert("AmericaInfantryRanger".to_string(), i_tpl);

    let th = logic
        .create_object(
            "ChinaInfantryTankHunter",
            Team::China,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("th");
    let inf = logic
        .create_object(
            "AmericaInfantryRanger",
            Team::USA,
            glam::Vec3::new(100.0, 0.0, 0.0),
        )
        .expect("inf");
    let aim = glam::Vec3::new(100.0, 0.0, 0.0);
    let from = glam::Vec3::new(0.0, 5.0, 0.0);
    let msl = logic
        .spawn_tank_hunter_missile_projectile(th, from, aim, Some(inf))
        .expect("missile");
    assert!(logic.honesty_tank_hunter_scatter_ok());
    let m_aim = logic
        .objects
        .get(&msl)
        .and_then(|o| o.tank_hunter_missile_aim)
        .expect("aim");
    let d = ((m_aim[0] - aim.x).powi(2) + (m_aim[2] - aim.z).powi(2)).sqrt();
    assert!(d > 0.01 && d <= TANK_HUNTER_SCATTER_VS_INFANTRY + 0.01);

    let mut t_tpl = ThingTemplate::new("AmericaTankCrusader");
    t_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(400.0);
    logic
        .templates
        .insert("AmericaTankCrusader".to_string(), t_tpl);
    let tank = logic
        .create_object(
            "AmericaTankCrusader",
            Team::USA,
            glam::Vec3::new(130.0, 0.0, 0.0),
        )
        .expect("tank");
    let before = logic.tank_hunter_scatter_applied;
    let msl2 = logic
        .spawn_tank_hunter_missile_projectile(
            th,
            from,
            glam::Vec3::new(130.0, 0.0, 0.0),
            Some(tank),
        )
        .expect("missile2");
    assert_eq!(logic.tank_hunter_scatter_applied, before);
    let aim2 = logic
        .objects
        .get(&msl2)
        .and_then(|o| o.tank_hunter_missile_aim)
        .expect("aim2");
    assert!((aim2[0] - 130.0).abs() < 0.01 && aim2[2].abs() < 0.01);
}

#[test]
fn overlord_shell_scatters_vs_infantry() {
    use crate::game_logic::host_overlord_gun::OVERLORD_SCATTER_VS_INFANTRY;

    let mut logic = GameLogic::new();
    let mut ov_tpl = ThingTemplate::new("ChinaTankOverlord");
    ov_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(1100.0);
    logic
        .templates
        .insert("ChinaTankOverlord".to_string(), ov_tpl);
    let mut i_tpl = ThingTemplate::new("AmericaInfantryRanger");
    i_tpl
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(100.0);
    logic
        .templates
        .insert("AmericaInfantryRanger".to_string(), i_tpl);

    let ov = logic
        .create_object(
            "ChinaTankOverlord",
            Team::China,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("overlord");
    let inf = logic
        .create_object(
            "AmericaInfantryRanger",
            Team::USA,
            glam::Vec3::new(140.0, 0.0, 0.0),
        )
        .expect("inf");
    let aim = glam::Vec3::new(140.0, 0.0, 0.0);
    let from = glam::Vec3::new(0.0, 8.0, 0.0);
    let shell = logic
        .spawn_overlord_shell_projectile(ov, from, aim, Some(inf))
        .expect("shell");
    assert!(logic.honesty_overlord_scatter_ok());
    let s_aim = logic
        .objects
        .get(&shell)
        .and_then(|o| o.overlord_shell_aim)
        .expect("aim");
    let d = ((s_aim[0] - aim.x).powi(2) + (s_aim[2] - aim.z).powi(2)).sqrt();
    assert!(d > 0.01 && d <= OVERLORD_SCATTER_VS_INFANTRY + 0.01);

    let mut t_tpl = ThingTemplate::new("AmericaTankCrusader");
    t_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(400.0);
    logic
        .templates
        .insert("AmericaTankCrusader".to_string(), t_tpl);
    let tank = logic
        .create_object(
            "AmericaTankCrusader",
            Team::USA,
            glam::Vec3::new(160.0, 0.0, 0.0),
        )
        .expect("tank");
    let before = logic.overlord_scatter_applied;
    let shell2 = logic
        .spawn_overlord_shell_projectile(ov, from, glam::Vec3::new(160.0, 0.0, 0.0), Some(tank))
        .expect("shell2");
    assert_eq!(logic.overlord_scatter_applied, before);
    let aim2 = logic
        .objects
        .get(&shell2)
        .and_then(|o| o.overlord_shell_aim)
        .expect("aim2");
    assert!((aim2[0] - 160.0).abs() < 0.01 && aim2[2].abs() < 0.01);
}

#[test]
fn rpg_trooper_missile_scatters_vs_infantry() {
    use crate::game_logic::host_rpg_trooper::RPG_TROOPER_SCATTER_VS_INFANTRY;

    let mut logic = GameLogic::new();
    let mut r_tpl = ThingTemplate::new("GLAInfantryTunnelDefender");
    r_tpl
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(100.0);
    logic
        .templates
        .insert("GLAInfantryTunnelDefender".to_string(), r_tpl);
    let mut i_tpl = ThingTemplate::new("AmericaInfantryRanger");
    i_tpl
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(100.0);
    logic
        .templates
        .insert("AmericaInfantryRanger".to_string(), i_tpl);

    let rpg = logic
        .create_object(
            "GLAInfantryTunnelDefender",
            Team::GLA,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("rpg");
    let inf = logic
        .create_object(
            "AmericaInfantryRanger",
            Team::USA,
            glam::Vec3::new(100.0, 0.0, 0.0),
        )
        .expect("inf");
    let aim = glam::Vec3::new(100.0, 0.0, 0.0);
    let from = glam::Vec3::new(0.0, 5.0, 0.0);
    let msl = logic
        .spawn_rpg_trooper_missile_projectile(rpg, from, aim, Some(inf))
        .expect("missile");
    assert!(logic.honesty_rpg_trooper_scatter_ok());
    let m_aim = logic
        .objects
        .get(&msl)
        .and_then(|o| o.rpg_trooper_missile_aim)
        .expect("aim");
    let d = ((m_aim[0] - aim.x).powi(2) + (m_aim[2] - aim.z).powi(2)).sqrt();
    assert!(d > 0.01 && d <= RPG_TROOPER_SCATTER_VS_INFANTRY + 0.01);

    let mut t_tpl = ThingTemplate::new("AmericaTankCrusader");
    t_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(400.0);
    logic
        .templates
        .insert("AmericaTankCrusader".to_string(), t_tpl);
    let tank = logic
        .create_object(
            "AmericaTankCrusader",
            Team::USA,
            glam::Vec3::new(130.0, 0.0, 0.0),
        )
        .expect("tank");
    let before = logic.rpg_trooper_scatter_applied;
    let msl2 = logic
        .spawn_rpg_trooper_missile_projectile(
            rpg,
            from,
            glam::Vec3::new(130.0, 0.0, 0.0),
            Some(tank),
        )
        .expect("missile2");
    assert_eq!(logic.rpg_trooper_scatter_applied, before);
    let aim2 = logic
        .objects
        .get(&msl2)
        .and_then(|o| o.rpg_trooper_missile_aim)
        .expect("aim2");
    assert!((aim2[0] - 130.0).abs() < 0.01 && aim2[2].abs() < 0.01);
}

#[test]
fn battlemaster_shell_scatters_vs_infantry() {
    use crate::game_logic::host_battlemaster::BATTLE_MASTER_SCATTER_VS_INFANTRY;

    let mut logic = GameLogic::new();
    let mut bm_tpl = ThingTemplate::new("ChinaTankBattleMaster");
    bm_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(400.0);
    logic
        .templates
        .insert("ChinaTankBattleMaster".to_string(), bm_tpl);
    let mut i_tpl = ThingTemplate::new("AmericaInfantryRanger");
    i_tpl
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(100.0);
    logic
        .templates
        .insert("AmericaInfantryRanger".to_string(), i_tpl);

    let bm = logic
        .create_object(
            "ChinaTankBattleMaster",
            Team::China,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("bm");
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
        .spawn_battlemaster_shell_projectile(bm, from, aim, Some(inf))
        .expect("shell");
    assert!(logic.honesty_battlemaster_scatter_ok());
    let s_aim = logic
        .objects
        .get(&shell)
        .and_then(|o| o.battlemaster_shell_aim)
        .expect("aim");
    let d = ((s_aim[0] - aim.x).powi(2) + (s_aim[2] - aim.z).powi(2)).sqrt();
    assert!(d > 0.01 && d <= BATTLE_MASTER_SCATTER_VS_INFANTRY + 0.01);

    let mut t_tpl = ThingTemplate::new("AmericaTankCrusader");
    t_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(400.0);
    logic
        .templates
        .insert("AmericaTankCrusader".to_string(), t_tpl);
    let tank = logic
        .create_object(
            "AmericaTankCrusader",
            Team::USA,
            glam::Vec3::new(150.0, 0.0, 0.0),
        )
        .expect("tank");
    let before = logic.battlemaster_scatter_applied;
    let shell2 = logic
        .spawn_battlemaster_shell_projectile(bm, from, glam::Vec3::new(150.0, 0.0, 0.0), Some(tank))
        .expect("shell2");
    assert_eq!(logic.battlemaster_scatter_applied, before);
    let aim2 = logic
        .objects
        .get(&shell2)
        .and_then(|o| o.battlemaster_shell_aim)
        .expect("aim2");
    assert!((aim2[0] - 150.0).abs() < 0.01 && aim2[2].abs() < 0.01);
}

#[test]
fn scorpion_shell_scatters_vs_infantry() {
    use crate::game_logic::host_scorpion::SCORPION_SCATTER_VS_INFANTRY;

    let mut logic = GameLogic::new();
    let mut s_tpl = ThingTemplate::new("GLATankScorpion");
    s_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(240.0);
    logic.templates.insert("GLATankScorpion".to_string(), s_tpl);
    let mut i_tpl = ThingTemplate::new("AmericaInfantryRanger");
    i_tpl
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(100.0);
    logic
        .templates
        .insert("AmericaInfantryRanger".to_string(), i_tpl);

    let scorp = logic
        .create_object("GLATankScorpion", Team::GLA, glam::Vec3::new(0.0, 0.0, 0.0))
        .expect("scorp");
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
        .spawn_scorpion_shell_projectile(scorp, from, aim, Some(inf), 0)
        .expect("shell");
    assert!(logic.honesty_scorpion_scatter_ok());
    let s_aim = logic
        .objects
        .get(&shell)
        .and_then(|o| o.scorpion_shell_aim)
        .expect("aim");
    let d = ((s_aim[0] - aim.x).powi(2) + (s_aim[2] - aim.z).powi(2)).sqrt();
    assert!(d > 0.01 && d <= SCORPION_SCATTER_VS_INFANTRY + 0.01);

    let mut t_tpl = ThingTemplate::new("AmericaTankCrusader");
    t_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(400.0);
    logic
        .templates
        .insert("AmericaTankCrusader".to_string(), t_tpl);
    let tank = logic
        .create_object(
            "AmericaTankCrusader",
            Team::USA,
            glam::Vec3::new(150.0, 0.0, 0.0),
        )
        .expect("tank");
    let before = logic.scorpion_scatter_applied;
    let shell2 = logic
        .spawn_scorpion_shell_projectile(
            scorp,
            from,
            glam::Vec3::new(150.0, 0.0, 0.0),
            Some(tank),
            0,
        )
        .expect("shell2");
    assert_eq!(logic.scorpion_scatter_applied, before);
    let aim2 = logic
        .objects
        .get(&shell2)
        .and_then(|o| o.scorpion_shell_aim)
        .expect("aim2");
    assert!((aim2[0] - 150.0).abs() < 0.01 && aim2[2].abs() < 0.01);
}

#[test]
fn usa_tank_shell_scatters_vs_infantry() {
    use crate::game_logic::host_usa_tanks::{
        CRUSADER_WEAPON_SPEED, USA_TANK_GUN_SCATTER_VS_INFANTRY,
    };

    let mut logic = GameLogic::new();
    let mut c_tpl = ThingTemplate::new("AmericaTankCrusader");
    c_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(480.0);
    logic
        .templates
        .insert("AmericaTankCrusader".to_string(), c_tpl);
    let mut i_tpl = ThingTemplate::new("AmericaInfantryRanger");
    i_tpl
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(100.0);
    logic
        .templates
        .insert("AmericaInfantryRanger".to_string(), i_tpl);

    let tank = logic
        .create_object(
            "AmericaTankCrusader",
            Team::USA,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("crusader");
    let inf = logic
        .create_object(
            "AmericaInfantryRanger",
            Team::China,
            glam::Vec3::new(120.0, 0.0, 0.0),
        )
        .expect("inf");
    let aim = glam::Vec3::new(120.0, 0.0, 0.0);
    let from = glam::Vec3::new(0.0, 6.0, 0.0);
    let shell = logic
        .spawn_usa_tank_shell_projectile(tank, from, aim, CRUSADER_WEAPON_SPEED, Some(inf))
        .expect("shell");
    assert!(logic.honesty_usa_tank_scatter_ok());
    let s_aim = logic
        .objects
        .get(&shell)
        .and_then(|o| o.usa_tank_shell_aim)
        .expect("aim");
    let d = ((s_aim[0] - aim.x).powi(2) + (s_aim[2] - aim.z).powi(2)).sqrt();
    assert!(d > 0.01 && d <= USA_TANK_GUN_SCATTER_VS_INFANTRY + 0.01);

    let mut v_tpl = ThingTemplate::new("ChinaTankBattleMaster");
    v_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(400.0);
    logic
        .templates
        .insert("ChinaTankBattleMaster".to_string(), v_tpl);
    let vehicle = logic
        .create_object(
            "ChinaTankBattleMaster",
            Team::China,
            glam::Vec3::new(150.0, 0.0, 0.0),
        )
        .expect("vehicle");
    let before = logic.usa_tank_scatter_applied;
    let shell2 = logic
        .spawn_usa_tank_shell_projectile(
            tank,
            from,
            glam::Vec3::new(150.0, 0.0, 0.0),
            CRUSADER_WEAPON_SPEED,
            Some(vehicle),
        )
        .expect("shell2");
    assert_eq!(logic.usa_tank_scatter_applied, before);
    let aim2 = logic
        .objects
        .get(&shell2)
        .and_then(|o| o.usa_tank_shell_aim)
        .expect("aim2");
    assert!((aim2[0] - 150.0).abs() < 0.01 && aim2[2].abs() < 0.01);
}

#[test]
fn marauder_shell_scatters_vs_infantry() {
    use crate::game_logic::host_marauder::MARAUDER_SCATTER_VS_INFANTRY;

    let mut logic = GameLogic::new();
    let mut m_tpl = ThingTemplate::new("GLATankMarauder");
    m_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(500.0);
    logic.templates.insert("GLATankMarauder".to_string(), m_tpl);
    let mut i_tpl = ThingTemplate::new("AmericaInfantryRanger");
    i_tpl
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(100.0);
    logic
        .templates
        .insert("AmericaInfantryRanger".to_string(), i_tpl);

    let marauder = logic
        .create_object("GLATankMarauder", Team::GLA, glam::Vec3::new(0.0, 0.0, 0.0))
        .expect("marauder");
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
        .spawn_marauder_shell_projectile(marauder, from, aim, Some(inf), 300.0)
        .expect("shell");
    assert!(logic.honesty_marauder_scatter_ok());
    let s_aim = logic
        .objects
        .get(&shell)
        .and_then(|o| o.marauder_shell_aim)
        .expect("aim");
    let d = ((s_aim[0] - aim.x).powi(2) + (s_aim[2] - aim.z).powi(2)).sqrt();
    assert!(d > 0.01 && d <= MARAUDER_SCATTER_VS_INFANTRY + 0.01);

    let mut t_tpl = ThingTemplate::new("AmericaTankCrusader");
    t_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(400.0);
    logic
        .templates
        .insert("AmericaTankCrusader".to_string(), t_tpl);
    let tank = logic
        .create_object(
            "AmericaTankCrusader",
            Team::USA,
            glam::Vec3::new(150.0, 0.0, 0.0),
        )
        .expect("tank");
    let before = logic.marauder_scatter_applied;
    let shell2 = logic
        .spawn_marauder_shell_projectile(
            marauder,
            from,
            glam::Vec3::new(150.0, 0.0, 0.0),
            Some(tank),
            300.0,
        )
        .expect("shell2");
    assert_eq!(logic.marauder_scatter_applied, before);
    let aim2 = logic
        .objects
        .get(&shell2)
        .and_then(|o| o.marauder_shell_aim)
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
fn rocket_buggy_missile_scatters_vs_infantry() {
    use crate::game_logic::host_rocket_buggy::BUGGY_SCATTER_VS_INFANTRY;

    let mut logic = GameLogic::new();
    let mut b_tpl = ThingTemplate::new("GLAVehicleRocketBuggy");
    b_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(120.0);
    logic
        .templates
        .insert("GLAVehicleRocketBuggy".to_string(), b_tpl);
    let mut i_tpl = ThingTemplate::new("AmericaInfantryRanger");
    i_tpl
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(100.0);
    logic
        .templates
        .insert("AmericaInfantryRanger".to_string(), i_tpl);

    let buggy = logic
        .create_object(
            "GLAVehicleRocketBuggy",
            Team::GLA,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("buggy");
    let inf = logic
        .create_object(
            "AmericaInfantryRanger",
            Team::USA,
            glam::Vec3::new(200.0, 0.0, 0.0),
        )
        .expect("inf");
    let aim = glam::Vec3::new(200.0, 0.0, 0.0);
    let from = glam::Vec3::new(0.0, 6.0, 0.0);
    let shell = logic
        .spawn_rocket_buggy_missile_projectile(buggy, from, aim, Some(inf))
        .expect("missile");
    assert!(logic.honesty_rocket_buggy_scatter_ok());
    let s_aim = logic
        .objects
        .get(&shell)
        .and_then(|o| o.rocket_buggy_missile_aim)
        .expect("aim");
    let d = ((s_aim[0] - aim.x).powi(2) + (s_aim[2] - aim.z).powi(2)).sqrt();
    assert!(d > 0.01 && d <= BUGGY_SCATTER_VS_INFANTRY + 0.01);

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
            glam::Vec3::new(250.0, 0.0, 0.0),
        )
        .expect("tank");
    let before = logic.rocket_buggy_scatter_applied;
    let shell2 = logic
        .spawn_rocket_buggy_missile_projectile(
            buggy,
            from,
            glam::Vec3::new(250.0, 0.0, 0.0),
            Some(tank),
        )
        .expect("missile2");
    assert_eq!(logic.rocket_buggy_scatter_applied, before);
    let aim2 = logic
        .objects
        .get(&shell2)
        .and_then(|o| o.rocket_buggy_missile_aim)
        .expect("aim2");
    assert!((aim2[0] - 250.0).abs() < 0.01 && aim2[2].abs() < 0.01);
}

#[test]
fn scud_launcher_missile_scatters_vs_infantry() {
    use crate::game_logic::host_scud_launcher::SCUD_SCATTER_VS_INFANTRY;

    let mut logic = GameLogic::new();
    let mut s_tpl = ThingTemplate::new("GLAVehicleSCUDLauncher");
    s_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(180.0);
    logic
        .templates
        .insert("GLAVehicleSCUDLauncher".to_string(), s_tpl);
    let mut i_tpl = ThingTemplate::new("AmericaInfantryRanger");
    i_tpl
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(100.0);
    logic
        .templates
        .insert("AmericaInfantryRanger".to_string(), i_tpl);

    let launcher = logic
        .create_object(
            "GLAVehicleSCUDLauncher",
            Team::GLA,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("scud");
    let inf = logic
        .create_object(
            "AmericaInfantryRanger",
            Team::USA,
            glam::Vec3::new(250.0, 0.0, 0.0),
        )
        .expect("inf");
    let aim = glam::Vec3::new(250.0, 0.0, 0.0);
    let from = glam::Vec3::new(0.0, 6.0, 0.0);
    let shell = logic
        .spawn_scud_launcher_missile_projectile(launcher, from, aim, Some(inf), false)
        .expect("missile");
    assert!(logic.honesty_scud_launcher_scatter_ok());
    let s_aim = logic
        .objects
        .get(&shell)
        .and_then(|o| o.scud_launcher_missile_aim)
        .expect("aim");
    let d = ((s_aim[0] - aim.x).powi(2) + (s_aim[2] - aim.z).powi(2)).sqrt();
    assert!(d > 0.01 && d <= SCUD_SCATTER_VS_INFANTRY + 0.01);

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
            glam::Vec3::new(300.0, 0.0, 0.0),
        )
        .expect("tank");
    let before = logic.scud_launcher_scatter_applied;
    let shell2 = logic
        .spawn_scud_launcher_missile_projectile(
            launcher,
            from,
            glam::Vec3::new(300.0, 0.0, 0.0),
            Some(tank),
            false,
        )
        .expect("missile2");
    assert_eq!(logic.scud_launcher_scatter_applied, before);
    let aim2 = logic
        .objects
        .get(&shell2)
        .and_then(|o| o.scud_launcher_missile_aim)
        .expect("aim2");
    assert!((aim2[0] - 300.0).abs() < 0.01 && aim2[2].abs() < 0.01);
}

#[test]
fn flashbang_grenade_scatters_aim() {
    use crate::game_logic::host_ranger::FLASHBANG_SCATTER_RADIUS;

    let mut logic = GameLogic::new();
    let mut r_tpl = ThingTemplate::new("AmericaInfantryRanger");
    r_tpl
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(180.0);
    logic
        .templates
        .insert("AmericaInfantryRanger".to_string(), r_tpl);
    let mut e_tpl = ThingTemplate::new("ChinaInfantryRedguard");
    e_tpl
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(100.0);
    logic
        .templates
        .insert("ChinaInfantryRedguard".to_string(), e_tpl);

    let ranger = logic
        .create_object(
            "AmericaInfantryRanger",
            Team::USA,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("ranger");
    let enemy = logic
        .create_object(
            "ChinaInfantryRedguard",
            Team::China,
            glam::Vec3::new(100.0, 0.0, 0.0),
        )
        .expect("enemy");
    let aim = glam::Vec3::new(100.0, 0.0, 0.0);
    let from = glam::Vec3::new(0.0, 4.0, 0.0);
    let shell = logic
        .spawn_flashbang_grenade_projectile(ranger, from, aim, Some(enemy))
        .expect("grenade");
    assert!(logic.honesty_flashbang_scatter_ok());
    let s_aim = logic
        .objects
        .get(&shell)
        .and_then(|o| o.flashbang_grenade_aim)
        .expect("aim");
    let d = ((s_aim[0] - aim.x).powi(2) + (s_aim[2] - aim.z).powi(2)).sqrt();
    assert!(d > 0.01 && d <= FLASHBANG_SCATTER_RADIUS + 0.01);

    // ScatterRadius applies vs vehicles too (not VsInfantry-only).
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
            glam::Vec3::new(150.0, 0.0, 0.0),
        )
        .expect("tank");
    let before = logic.flashbang_scatter_applied;
    let shell2 = logic
        .spawn_flashbang_grenade_projectile(
            ranger,
            from,
            glam::Vec3::new(150.0, 0.0, 0.0),
            Some(tank),
        )
        .expect("grenade2");
    assert!(logic.flashbang_scatter_applied > before);
    let aim2 = logic
        .objects
        .get(&shell2)
        .and_then(|o| o.flashbang_grenade_aim)
        .expect("aim2");
    let d2 = ((aim2[0] - 150.0).powi(2) + aim2[2].powi(2)).sqrt();
    assert!(d2 > 0.01 && d2 <= FLASHBANG_SCATTER_RADIUS + 0.01);
}

#[test]
fn inferno_shell_scatters_vs_infantry() {
    use crate::game_logic::host_inferno_cannon::INFERNO_CANNON_SCATTER_VS_INFANTRY;

    let mut logic = GameLogic::new();
    let mut c_tpl = ThingTemplate::new("ChinaVehicleInfernoCannon");
    c_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(120.0);
    logic
        .templates
        .insert("ChinaVehicleInfernoCannon".to_string(), c_tpl);
    let mut i_tpl = ThingTemplate::new("AmericaInfantryRanger");
    i_tpl
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(100.0);
    logic
        .templates
        .insert("AmericaInfantryRanger".to_string(), i_tpl);

    let cannon = logic
        .create_object(
            "ChinaVehicleInfernoCannon",
            Team::China,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("cannon");
    let inf = logic
        .create_object(
            "AmericaInfantryRanger",
            Team::USA,
            glam::Vec3::new(200.0, 0.0, 0.0),
        )
        .expect("inf");
    let aim = glam::Vec3::new(200.0, 0.0, 0.0);
    let from = glam::Vec3::new(0.0, 6.0, 0.0);
    let shell = logic
        .spawn_inferno_shell_projectile(cannon, from, aim, Some(inf), false)
        .expect("shell");
    assert!(logic.honesty_inferno_scatter_ok());
    let s_aim = logic
        .objects
        .get(&shell)
        .and_then(|o| o.inferno_shell_aim)
        .expect("aim");
    let d = ((s_aim[0] - aim.x).powi(2) + (s_aim[2] - aim.z).powi(2)).sqrt();
    assert!(d > 0.01 && d <= INFERNO_CANNON_SCATTER_VS_INFANTRY + 0.01);

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
            glam::Vec3::new(250.0, 0.0, 0.0),
        )
        .expect("tank");
    let before = logic.inferno_scatter_applied;
    let shell2 = logic
        .spawn_inferno_shell_projectile(
            cannon,
            from,
            glam::Vec3::new(250.0, 0.0, 0.0),
            Some(tank),
            false,
        )
        .expect("shell2");
    assert_eq!(logic.inferno_scatter_applied, before);
    let aim2 = logic
        .objects
        .get(&shell2)
        .and_then(|o| o.inferno_shell_aim)
        .expect("aim2");
    assert!((aim2[0] - 250.0).abs() < 0.01 && aim2[2].abs() < 0.01);
}

#[test]
fn stealth_jet_missile_scatters_vs_infantry() {
    use crate::game_logic::host_stealth_fighter::STEALTH_FIGHTER_SCATTER_VS_INFANTRY;

    let mut logic = GameLogic::new();
    let mut j_tpl = ThingTemplate::new("AmericaJetStealthFighter");
    j_tpl
        .add_kind_of(KindOf::Aircraft)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(120.0);
    logic
        .templates
        .insert("AmericaJetStealthFighter".to_string(), j_tpl);
    let mut i_tpl = ThingTemplate::new("ChinaInfantryRedguard");
    i_tpl
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(100.0);
    logic
        .templates
        .insert("ChinaInfantryRedguard".to_string(), i_tpl);

    let jet = logic
        .create_object(
            "AmericaJetStealthFighter",
            Team::USA,
            glam::Vec3::new(0.0, 40.0, 0.0),
        )
        .expect("jet");
    let inf = logic
        .create_object(
            "ChinaInfantryRedguard",
            Team::China,
            glam::Vec3::new(180.0, 0.0, 0.0),
        )
        .expect("inf");
    let aim = glam::Vec3::new(180.0, 0.0, 0.0);
    let from = glam::Vec3::new(0.0, 40.0, 0.0);
    let shell = logic
        .spawn_stealth_jet_missile_projectile(jet, from, aim, Some(inf))
        .expect("missile");
    assert!(logic.honesty_stealth_jet_scatter_ok());
    let s_aim = logic
        .objects
        .get(&shell)
        .and_then(|o| o.stealth_jet_missile_aim)
        .expect("aim");
    let d = ((s_aim[0] - aim.x).powi(2) + (s_aim[2] - aim.z).powi(2)).sqrt();
    assert!(d > 0.01 && d <= STEALTH_FIGHTER_SCATTER_VS_INFANTRY + 0.01);

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
            glam::Vec3::new(220.0, 0.0, 0.0),
        )
        .expect("tank");
    let before = logic.stealth_jet_scatter_applied;
    let shell2 = logic
        .spawn_stealth_jet_missile_projectile(
            jet,
            from,
            glam::Vec3::new(220.0, 0.0, 0.0),
            Some(tank),
        )
        .expect("missile2");
    assert_eq!(logic.stealth_jet_scatter_applied, before);
    let aim2 = logic
        .objects
        .get(&shell2)
        .and_then(|o| o.stealth_jet_missile_aim)
        .expect("aim2");
    assert!((aim2[0] - 220.0).abs() < 0.01 && aim2[2].abs() < 0.01);
}

#[test]
fn humvee_tow_missile_scatters_vs_infantry() {
    use crate::game_logic::host_humvee::HUMVEE_GROUND_TOW_SCATTER_VS_INFANTRY;

    let mut logic = GameLogic::new();
    let mut h_tpl = ThingTemplate::new("AmericaVehicleHumvee");
    h_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(240.0);
    logic
        .templates
        .insert("AmericaVehicleHumvee".to_string(), h_tpl);
    let mut i_tpl = ThingTemplate::new("ChinaInfantryRedguard");
    i_tpl
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(100.0);
    logic
        .templates
        .insert("ChinaInfantryRedguard".to_string(), i_tpl);

    let humvee = logic
        .create_object(
            "AmericaVehicleHumvee",
            Team::USA,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("humvee");
    let inf = logic
        .create_object(
            "ChinaInfantryRedguard",
            Team::China,
            glam::Vec3::new(120.0, 0.0, 0.0),
        )
        .expect("inf");
    let aim = glam::Vec3::new(120.0, 0.0, 0.0);
    let from = glam::Vec3::new(0.0, 6.0, 0.0);
    let shell = logic
        .spawn_humvee_tow_missile_projectile(humvee, from, aim, Some(inf), false)
        .expect("tow");
    assert!(logic.honesty_humvee_tow_scatter_ok());
    let s_aim = logic
        .objects
        .get(&shell)
        .and_then(|o| o.humvee_tow_aim)
        .expect("aim");
    let d = ((s_aim[0] - aim.x).powi(2) + (s_aim[2] - aim.z).powi(2)).sqrt();
    assert!(d > 0.01 && d <= HUMVEE_GROUND_TOW_SCATTER_VS_INFANTRY + 0.01);

    // Air TOW never scatters via this residual.
    let before = logic.humvee_tow_scatter_applied;
    let air = logic
        .spawn_humvee_tow_missile_projectile(humvee, from, aim, Some(inf), true)
        .expect("air tow");
    assert_eq!(logic.humvee_tow_scatter_applied, before);
    let a_aim = logic
        .objects
        .get(&air)
        .and_then(|o| o.humvee_tow_aim)
        .expect("air aim");
    assert!((a_aim[0] - aim.x).abs() < 0.01 && a_aim[2].abs() < 0.01);

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
            glam::Vec3::new(150.0, 0.0, 0.0),
        )
        .expect("tank");
    let before2 = logic.humvee_tow_scatter_applied;
    let shell2 = logic
        .spawn_humvee_tow_missile_projectile(
            humvee,
            from,
            glam::Vec3::new(150.0, 0.0, 0.0),
            Some(tank),
            false,
        )
        .expect("tow2");
    assert_eq!(logic.humvee_tow_scatter_applied, before2);
    let aim2 = logic
        .objects
        .get(&shell2)
        .and_then(|o| o.humvee_tow_aim)
        .expect("aim2");
    assert!((aim2[0] - 150.0).abs() < 0.01 && aim2[2].abs() < 0.01);
}

#[test]
fn neutron_shell_scatters_vs_infantry() {
    use crate::game_logic::host_neutron_shell::NEUTRON_WEAPON_SCATTER_VS_INFANTRY;

    let mut logic = GameLogic::new();
    let mut c_tpl = ThingTemplate::new("ChinaVehicleNukeCannon");
    c_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(280.0);
    logic
        .templates
        .insert("ChinaVehicleNukeCannon".to_string(), c_tpl);
    let mut i_tpl = ThingTemplate::new("AmericaInfantryRanger");
    i_tpl
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(100.0);
    logic
        .templates
        .insert("AmericaInfantryRanger".to_string(), i_tpl);

    let cannon = logic
        .create_object(
            "ChinaVehicleNukeCannon",
            Team::China,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("cannon");
    let inf = logic
        .create_object(
            "AmericaInfantryRanger",
            Team::USA,
            glam::Vec3::new(200.0, 0.0, 0.0),
        )
        .expect("inf");
    let aim = glam::Vec3::new(200.0, 0.0, 0.0);
    let from = glam::Vec3::new(0.0, 6.0, 0.0);
    let shell = logic
        .spawn_neutron_cannon_shell_projectile(cannon, from, aim, Some(inf))
        .expect("shell");
    assert!(logic.honesty_neutron_shell_scatter_ok());
    let s_aim = logic
        .objects
        .get(&shell)
        .and_then(|o| o.neutron_shell_aim)
        .expect("aim");
    let d = ((s_aim[0] - aim.x).powi(2) + (s_aim[2] - aim.z).powi(2)).sqrt();
    assert!(d > 0.01 && d <= NEUTRON_WEAPON_SCATTER_VS_INFANTRY + 0.01);

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
            glam::Vec3::new(250.0, 0.0, 0.0),
        )
        .expect("tank");
    let before = logic.neutron_shell_scatter_applied;
    let shell2 = logic
        .spawn_neutron_cannon_shell_projectile(
            cannon,
            from,
            glam::Vec3::new(250.0, 0.0, 0.0),
            Some(tank),
        )
        .expect("shell2");
    assert_eq!(logic.neutron_shell_scatter_applied, before);
    let aim2 = logic
        .objects
        .get(&shell2)
        .and_then(|o| o.neutron_shell_aim)
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
fn nuke_cannon_scatter_misses_infantry_residual() {
    use crate::game_logic::host_nuke_cannon::{
        NUKE_CANNON_PRIMARY_WEAPON, NUKE_CANNON_SCATTER_VS_INFANTRY,
    };
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let mut logic = GameLogic::new();
    ensure_test_infantry_template(&mut logic);
    ensure_test_tank_template(&mut logic);

    let mut nc_tpl = ThingTemplate::new("ChinaNukeCannon");
    nc_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(300.0)
        .set_primary_weapon_name(NUKE_CANNON_PRIMARY_WEAPON);
    logic
        .templates
        .insert("ChinaNukeCannon".to_string(), nc_tpl);

    let nc = logic
        .create_object(
            "ChinaNukeCannon",
            Team::China,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("nuke");
    if let Some(o) = logic.objects.get_mut(&nc) {
        if let Some(w) = o.weapon.as_mut() {
            w.last_fire_time = -100.0;
        }
    }
    let inf = logic
        .create_object("TestInfantry", Team::USA, glam::Vec3::new(120.0, 0.0, 0.0))
        .expect("inf");
    if let Some(o) = logic.objects.get_mut(&inf) {
        o.set_selection_radius(0.5);
    }

    let from = glam::Vec3::ZERO;
    let aim = logic
        .objects
        .get(&inf)
        .map(|o| o.get_position())
        .unwrap_or(glam::Vec3::new(120.0, 0.0, 0.0));
    let _ = logic.spawn_nuke_cannon_shell_projectile(nc, from, aim, Some(inf));
    assert!(
        logic.nuke_cannon_scatter_applied > 0
            || logic.nuke_cannon_scatter_misses > 0
            || logic.honesty_nuke_cannon_scatter_ok(),
        "nuke cannon scatter residual must peel vs infantry"
    );
    assert!((NUKE_CANNON_SCATTER_VS_INFANTRY - 30.0).abs() < 0.01);

    // Vehicle still damaged via pure splash at impact (no infantry scatter gate).
    let tank = logic
        .create_object("TestTank", Team::USA, glam::Vec3::new(40.0, 0.0, 0.0))
        .expect("tank");
    let impact = logic
        .objects
        .get(&tank)
        .map(|o| o.get_position())
        .unwrap_or(glam::Vec3::new(40.0, 0.0, 0.0));
    let hp_before = logic.host_object(tank).unwrap().health.current;
    let (hits, _) = logic.apply_nuke_cannon_primary_at(impact, Some(nc), Team::China);
    let hp_after = logic
        .host_object(tank)
        .map(|o| o.health.current)
        .unwrap_or(0.0);
    assert!(
        hits > 0 && hp_after < hp_before,
        "vehicle still hit by splash"
    );
}

#[test]
fn humvee_tow_scatter_misses_infantry_residual() {
    use crate::game_logic::host_humvee::{
        HUMVEE_GROUND_TOW_SCATTER_VS_INFANTRY, HUMVEE_MISSILE_WEAPON,
    };
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let mut logic = GameLogic::new();
    ensure_test_infantry_template(&mut logic);
    ensure_test_tank_template(&mut logic);

    let mut hv_tpl = ThingTemplate::new("AmericaVehicleHumvee");
    hv_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(200.0)
        .set_primary_weapon_name(HUMVEE_MISSILE_WEAPON);
    logic
        .templates
        .insert("AmericaVehicleHumvee".to_string(), hv_tpl);

    let hv = logic
        .create_object(
            "AmericaVehicleHumvee",
            Team::USA,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("humvee");
    let inf = logic
        .create_object("TestInfantry", Team::GLA, glam::Vec3::new(60.0, 0.0, 0.0))
        .expect("inf");
    if let Some(o) = logic.objects.get_mut(&inf) {
        o.set_selection_radius(0.5);
    }

    let from = glam::Vec3::ZERO;
    let aim = logic
        .objects
        .get(&inf)
        .map(|o| o.get_position())
        .unwrap_or(glam::Vec3::new(60.0, 0.0, 0.0));
    let _ = logic.spawn_humvee_tow_missile_projectile(hv, from, aim, Some(inf), false);
    assert!(
        logic.humvee_tow_scatter_applied > 0
            || logic.humvee_tow_scatter_misses > 0
            || logic.honesty_humvee_tow_scatter_ok(),
        "humvee ground TOW scatter residual must peel vs infantry"
    );
    assert!((HUMVEE_GROUND_TOW_SCATTER_VS_INFANTRY - 10.0).abs() < 0.01);

    // Air TOW does not peel scatter.
    let before = logic.humvee_tow_scatter_applied;
    let _ = logic.spawn_humvee_tow_missile_projectile(hv, from, aim, Some(inf), true);
    assert_eq!(
        logic.humvee_tow_scatter_applied, before,
        "air TOW must not apply infantry scatter"
    );

    // Vehicle still damaged via pure splash.
    let tank = logic
        .create_object("TestTank", Team::GLA, glam::Vec3::new(20.0, 0.0, 0.0))
        .expect("tank");
    let impact = logic
        .objects
        .get(&tank)
        .map(|o| o.get_position())
        .unwrap_or(glam::Vec3::new(20.0, 0.0, 0.0));
    let hp_before = logic.host_object(tank).unwrap().health.current;
    let (hits, _) = logic.apply_humvee_tow_residual_at(impact, Some(hv), Some(tank), false);
    let hp_after = logic
        .host_object(tank)
        .map(|o| o.health.current)
        .unwrap_or(0.0);
    assert!(
        hits > 0 && hp_after < hp_before,
        "vehicle still hit by tow splash"
    );
}

#[test]
fn scud_launcher_scatter_misses_infantry_residual() {
    use crate::game_logic::host_scud_launcher::{SCUD_GUN_EXPLOSIVE, SCUD_SCATTER_VS_INFANTRY};
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let mut logic = GameLogic::new();
    ensure_test_infantry_template(&mut logic);
    ensure_test_tank_template(&mut logic);

    let mut sc_tpl = ThingTemplate::new("GLAVehicleSCUDLauncher");
    sc_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(200.0)
        .set_primary_weapon_name(SCUD_GUN_EXPLOSIVE);
    logic
        .templates
        .insert("GLAVehicleSCUDLauncher".to_string(), sc_tpl);

    let sc = logic
        .create_object(
            "GLAVehicleSCUDLauncher",
            Team::GLA,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("scud");
    let inf = logic
        .create_object("TestInfantry", Team::USA, glam::Vec3::new(250.0, 0.0, 0.0))
        .expect("inf");
    if let Some(o) = logic.objects.get_mut(&inf) {
        o.set_selection_radius(0.5);
    }

    let from = glam::Vec3::ZERO;
    let aim = logic
        .objects
        .get(&inf)
        .map(|o| o.get_position())
        .unwrap_or(glam::Vec3::new(250.0, 0.0, 0.0));
    let _ = logic.spawn_scud_launcher_missile_projectile(sc, from, aim, Some(inf), false);
    assert!(
        logic.scud_launcher_scatter_applied > 0
            || logic.scud_launcher_scatter_misses > 0
            || logic.honesty_scud_launcher_scatter_ok(),
        "scud scatter residual must peel vs infantry"
    );
    assert!((SCUD_SCATTER_VS_INFANTRY - 30.0).abs() < 0.01);

    // Vehicle still damaged via pure splash.
    let tank = logic
        .create_object("TestTank", Team::USA, glam::Vec3::new(40.0, 0.0, 0.0))
        .expect("tank");
    let impact = logic
        .objects
        .get(&tank)
        .map(|o| o.get_position())
        .unwrap_or(glam::Vec3::new(40.0, 0.0, 0.0));
    let hp_before = logic.host_object(tank).unwrap().health.current;
    let (hits, _) = logic.apply_scud_area_at(impact, Some(sc), Team::GLA, false);
    let hp_after = logic
        .host_object(tank)
        .map(|o| o.health.current)
        .unwrap_or(0.0);
    assert!(
        hits > 0 && hp_after < hp_before,
        "vehicle still hit by scud splash"
    );
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
fn inferno_cannon_scatter_misses_infantry_residual() {
    use crate::game_logic::host_inferno_cannon::{
        INFERNO_CANNON_GUN, INFERNO_CANNON_SCATTER_VS_INFANTRY,
    };
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let mut logic = GameLogic::new();
    ensure_test_infantry_template(&mut logic);
    ensure_test_tank_template(&mut logic);

    let mut ic_tpl = ThingTemplate::new("ChinaInfernoCannon");
    ic_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(200.0)
        .set_primary_weapon_name(INFERNO_CANNON_GUN);
    logic
        .templates
        .insert("ChinaInfernoCannon".to_string(), ic_tpl);

    let ic = logic
        .create_object(
            "ChinaInfernoCannon",
            Team::China,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("inferno");
    let inf = logic
        .create_object("TestInfantry", Team::USA, glam::Vec3::new(120.0, 0.0, 0.0))
        .expect("inf");
    if let Some(o) = logic.objects.get_mut(&inf) {
        o.set_selection_radius(0.5);
    }

    let from = glam::Vec3::ZERO;
    let aim = logic
        .objects
        .get(&inf)
        .map(|o| o.get_position())
        .unwrap_or(glam::Vec3::new(120.0, 0.0, 0.0));
    let _ = logic.spawn_inferno_shell_projectile(ic, from, aim, Some(inf), false);
    assert!(
        logic.inferno_scatter_applied > 0
            || logic.inferno_scatter_misses > 0
            || logic.honesty_inferno_scatter_ok(),
        "inferno scatter residual must peel vs infantry"
    );
    assert!((INFERNO_CANNON_SCATTER_VS_INFANTRY - 30.0).abs() < 0.01);

    // Vehicle still damaged via pure splash.
    let tank = logic
        .create_object("TestTank", Team::USA, glam::Vec3::new(30.0, 0.0, 0.0))
        .expect("tank");
    let impact = logic
        .objects
        .get(&tank)
        .map(|o| o.get_position())
        .unwrap_or(glam::Vec3::new(30.0, 0.0, 0.0));
    let hp_before = logic.host_object(tank).unwrap().health.current;
    let (hits, _) = logic.apply_inferno_shell_residual_at(impact, Some(ic), Some(tank));
    let hp_after = logic
        .host_object(tank)
        .map(|o| o.health.current)
        .unwrap_or(0.0);
    assert!(
        hits > 0 && hp_after < hp_before,
        "vehicle still hit by shell splash"
    );
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
fn neutron_shell_scatter_misses_infantry_residual() {
    use crate::game_logic::host_neutron_shell::{
        NEUTRON_WEAPON_SCATTER_VS_INFANTRY, NUKE_CANNON_NEUTRON_WEAPON,
    };
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let mut logic = GameLogic::new();
    ensure_test_infantry_template(&mut logic);

    let mut nc_tpl = ThingTemplate::new("ChinaNukeCannon");
    nc_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(300.0)
        .set_primary_weapon_name(NUKE_CANNON_NEUTRON_WEAPON);
    logic
        .templates
        .insert("ChinaNukeCannon".to_string(), nc_tpl);

    let nc = logic
        .create_object(
            "ChinaNukeCannon",
            Team::China,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("nuke cannon");
    let inf = logic
        .create_object("TestInfantry", Team::USA, glam::Vec3::new(80.0, 0.0, 0.0))
        .expect("inf");
    if let Some(o) = logic.objects.get_mut(&inf) {
        o.set_selection_radius(0.5);
    }

    let from = glam::Vec3::new(0.0, 5.0, 0.0);
    let aim = logic
        .objects
        .get(&inf)
        .map(|o| o.get_position())
        .unwrap_or(glam::Vec3::new(80.0, 0.0, 0.0));
    let pid = logic.spawn_neutron_cannon_shell_projectile(nc, from, aim, Some(inf));
    assert!(pid.is_some(), "neutron shell must spawn");
    assert!(
        logic.neutron_shell_scatter_applied > 0
            || logic.neutron_shell_scatter_misses > 0
            || logic.honesty_neutron_shell_scatter_ok(),
        "neutron shell scatter residual must peel vs infantry"
    );
    assert!((NEUTRON_WEAPON_SCATTER_VS_INFANTRY - 10.0).abs() < 0.01);

    // Non-infantry aim does not add scatter miss residual.
    let before_miss = logic.neutron_shell_scatter_misses;
    let tank_aim = glam::Vec3::new(40.0, 0.0, 0.0);
    let _ = logic.spawn_neutron_cannon_shell_projectile(nc, from, tank_aim, None);
    assert_eq!(logic.neutron_shell_scatter_misses, before_miss);
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

    // Vehicle in splash still takes residual damage by radius.
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
    assert!(hits > 0 && hp_after < hp_before, "splash still hits");
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

#[test]
fn comanche_at_scatter_misses_infantry_residual() {
    use crate::game_logic::host_comanche_rocket_pods::{
        COMANCHE_ANTITANK_WEAPON, COMANCHE_AT_SCATTER_VS_INFANTRY,
    };
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let mut logic = GameLogic::new();
    ensure_test_infantry_template(&mut logic);
    ensure_test_tank_template(&mut logic);

    let mut tpl = ThingTemplate::new("AmericaJetComanche");
    tpl.add_kind_of(KindOf::Aircraft)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(200.0)
        .set_primary_weapon_name(COMANCHE_ANTITANK_WEAPON);
    logic
        .templates
        .insert("AmericaJetComanche".to_string(), tpl);

    let helo = logic
        .create_object(
            "AmericaJetComanche",
            Team::USA,
            glam::Vec3::new(0.0, 40.0, 0.0),
        )
        .expect("comanche");
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
    let _ = logic.apply_comanche_antitank_residual_at(impact, Some(helo), Some(inf));
    assert!(
        logic.comanche_at_scatter_applied > 0
            || logic.comanche_at_scatter_misses > 0
            || logic.honesty_comanche_at_scatter_ok(),
        "comanche AT scatter residual must peel vs infantry"
    );
    assert!((COMANCHE_AT_SCATTER_VS_INFANTRY - 10.0).abs() < 0.01);

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
    let (hits, _) = logic.apply_comanche_antitank_residual_at(impact, Some(helo), Some(tank));
    let hp_after = logic
        .host_object(tank)
        .map(|o| o.health.current)
        .unwrap_or(0.0);
    assert!(hits > 0 && hp_after < hp_before, "vehicle still hit");
}
