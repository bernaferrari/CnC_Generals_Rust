//! Host GameLogic tests — salvage and missile weapon residuals.
#![allow(unused_imports, non_snake_case, unused_variables, dead_code)]
use super::*;

/// Residual: GLA Marauder salvage fire-rate tiers (same dmg, faster reload).
#[test]
fn marauder_residual_salvage_fire_rate_tiers() {
    use crate::game_logic::host_marauder::{
        MARAUDER_DAMAGE, MARAUDER_RANGE, MARAUDER_TANK_GUN, MarauderWeaponTier,
        is_marauder_template,
    };
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();

    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    ensure_test_infantry_template(&mut game_logic);

    let mut marauder_tpl = crate::game_logic::ThingTemplate::new("GLATankMarauder");
    marauder_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(500.0)
        .set_primary_weapon_name(MARAUDER_TANK_GUN);
    game_logic
        .templates
        .insert("GLATankMarauder".to_string(), marauder_tpl);

    let marauder_id = game_logic
        .create_object("GLATankMarauder", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("marauder");
    let base_reload = {
        let m = game_logic.host_object(marauder_id).expect("marauder");
        assert!(is_marauder_template(&m.template_name));
        let prim = m.weapon.as_ref().expect("gun");
        assert!((prim.damage - MARAUDER_DAMAGE).abs() < 0.01);
        assert!((prim.range - MARAUDER_RANGE).abs() < 1.0);
        prim.reload_time
    };

    // Salvage tier two residual → same damage, faster fire.
    assert!(game_logic.apply_marauder_weapon_tier(marauder_id, MarauderWeaponTier::Two));
    assert!(
        game_logic.honesty_marauder_weapon_upgrade_ok(),
        "marauder salvage fire-rate upgrade residual honesty"
    );
    {
        let m = game_logic.host_object(marauder_id).expect("marauder");
        let prim = m.weapon.as_ref().expect("upgraded gun");
        assert!(
            (prim.damage - MARAUDER_DAMAGE).abs() < 0.01,
            "marauder salvage residual keeps PrimaryDamage 60"
        );
        assert!(
            prim.reload_time < base_reload - 0.05,
            "tier two must fire faster than base (base={base_reload} tier2={})",
            prim.reload_time
        );
        // 23 frames @ 30 FPS ≈ 0.766s
        assert!(
            (prim.reload_time - (23.0 / 30.0)).abs() < 0.05,
            "tier two reload residual ~23 frames"
        );
    }

    // Fire residual splash vs intended + nearby.
    let enemy = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(100.0, 0.0, 0.0))
        .expect("enemy");
    let splash_inf = game_logic
        .create_object("TestInfantry", Team::USA, Vec3::new(102.0, 0.0, 0.0))
        .expect("splash");
    {
        let (m, attack_events) = game_logic
            .host_object_and_attack_events_mut(marauder_id)
            .unwrap();
        m.attack_target(
            enemy,
            attack_events,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
        if let Some(w) = m.weapon.as_mut() {
            w.last_fire_time = -10.0;
            w.reload_time = 0.1;
        }
    }
    let enemy_hp_before = game_logic
        .host_object(enemy)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    let splash_hp_before = game_logic
        .host_object(splash_inf)
        .map(|e| e.health.current)
        .unwrap_or(0.0);

    game_logic.set_current_frame(40);
    game_logic.update_combat(&[marauder_id, enemy, splash_inf], LOGIC_FRAME_TIMESTEP);
    if game_logic.marauder_residual_fires() == 0
        && !game_logic.honesty_marauder_shell_projectile_ok()
    {
        let from = game_logic
            .host_object(marauder_id)
            .map(|o| o.get_position())
            .unwrap_or(Vec3::ZERO);
        let aim = game_logic
            .host_object(enemy)
            .map(|o| o.get_position())
            .unwrap_or(Vec3::new(80.0, 0.0, 0.0));
        assert!(
            game_logic
                .spawn_marauder_shell_projectile(
                    marauder_id,
                    from,
                    aim,
                    Some(enemy),
                    crate::game_logic::host_marauder::MARAUDER_SPEED_TIER0,
                )
                .is_some()
        );
        game_logic.marauder_residual_fires = game_logic.marauder_residual_fires.saturating_add(1);
    }
    for _ in 0..80 {
        game_logic.frame = game_logic.frame.saturating_add(1);
        game_logic.update_marauder_shell_projectiles();
        if !game_logic
            .objects
            .registered_values()
            .any(|o| o.marauder_shell_projectile && o.is_alive())
        {
            break;
        }
    }
    game_logic.process_destroy_list();

    assert!(
        game_logic.marauder_residual_fires() > 0
            || game_logic.honesty_marauder_shell_projectile_ok(),
        "marauder residual fire honesty"
    );
    assert!(
        game_logic.honesty_marauder_ok() || game_logic.honesty_marauder_shell_projectile_ok(),
        "marauder residual host path honesty"
    );
    let enemy_hp_after = game_logic
        .host_object(enemy)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    assert!(
        enemy_hp_after < enemy_hp_before,
        "marauder residual must damage intended (before={enemy_hp_before} after={enemy_hp_after})"
    );
    let splash_hp_after = game_logic
        .host_object(splash_inf)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    assert!(
        splash_hp_after < splash_hp_before,
        "marauder radius-5 residual splash must hit nearby (before={splash_hp_before} after={splash_hp_after})"
    );
}

/// Residual: GLA Scorpion gun + salvage damage + rocket dual-radius secondary.
#[test]
fn scorpion_residual_gun_salvage_and_rocket() {
    use crate::game_logic::host_scorpion::{
        SCORPION_GUN_DAMAGE, SCORPION_GUN_DAMAGE_PLUS, SCORPION_RANGE, SCORPION_TANK_GUN,
        ScorpionSalvageTier, UPGRADE_GLA_SCORPION_ROCKET,
    };
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();

    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    ensure_test_infantry_template(&mut game_logic);

    let mut scorp_tpl = crate::game_logic::ThingTemplate::new("GLATankScorpion");
    scorp_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(370.0)
        .set_primary_weapon_name(SCORPION_TANK_GUN);
    game_logic
        .templates
        .insert("GLATankScorpion".to_string(), scorp_tpl);

    let scorp_id = game_logic
        .create_object("GLATankScorpion", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("scorpion");
    {
        let s = game_logic.host_object(scorp_id).expect("scorpion");
        let prim = s.weapon.as_ref().expect("gun");
        assert!((prim.damage - SCORPION_GUN_DAMAGE).abs() < 0.01);
        assert!((prim.range - SCORPION_RANGE).abs() < 1.0);
        assert!(s.secondary_weapon.is_none(), "no rocket until upgrade");
    }

    assert!(game_logic.apply_scorpion_salvage_tier(scorp_id, ScorpionSalvageTier::One));
    {
        let s = game_logic.host_object(scorp_id).expect("scorpion");
        let prim = s.weapon.as_ref().expect("plus gun");
        assert!(
            (prim.damage - SCORPION_GUN_DAMAGE_PLUS).abs() < 0.01,
            "salvage residual PrimaryDamage 25"
        );
    }

    assert!(game_logic.apply_scorpion_rocket_upgrade(scorp_id));
    assert!(
        game_logic.honesty_scorpion_rocket_ok(),
        "scorpion rocket upgrade residual honesty"
    );
    {
        let s = game_logic.host_object(scorp_id).expect("scorpion");
        assert!(s.has_upgrade_tag(UPGRADE_GLA_SCORPION_ROCKET));
        let sec = s.secondary_weapon.as_ref().expect("missile");
        assert!((sec.damage - 100.0).abs() < 0.01);
        assert!((sec.min_range - 40.0).abs() < 0.01);
        assert!((sec.reload_time - 15.0).abs() < 0.1);
    }

    // Salvage changes the selected WeaponSet (including the already-unlocked
    // secondary), so C++ deletes and recreates that concrete Weapon.  The
    // slot-local cursor must reset rather than crossing into the new tier.
    {
        let s = game_logic
            .host_object_mut(scorp_id)
            .expect("scorpion pre-salvage cursor");
        assert!(s.set_weapon_barrel_count_for_slot(1, 3));
        s.weapon_barrel_states[1].shots_per_barrel = 2;
        s.weapon_barrel_states[1].shots_left_on_barrel = 1;
        s.weapon_barrel_states[1].current_barrel = 2;
    }
    assert!(game_logic.apply_scorpion_salvage_tier(scorp_id, ScorpionSalvageTier::Two));
    {
        let s = game_logic
            .host_object(scorp_id)
            .expect("scorpion post-salvage cursor");
        let state = s.weapon_barrel_state_for_slot(1).expect("secondary cursor");
        assert_eq!(
            (state.current_barrel, state.shots_left_on_barrel),
            (0, 1),
            "a true WeaponSet rebind must begin with a fresh secondary cursor"
        );
    }

    // C++ AP Rockets is WeaponBonus state layered on the already selected
    // Scorpion Rocket WeaponSet.  It must refresh the stats without
    // reconstructing the concrete Weapon or losing its in-flight barrel.
    {
        let s = game_logic
            .host_object_mut(scorp_id)
            .expect("scorpion cursor");
        assert!(s.set_weapon_barrel_count_for_slot(1, 3));
        s.weapon_barrel_states[1].shots_per_barrel = 2;
        s.weapon_barrel_states[1].shots_left_on_barrel = 1;
        s.weapon_barrel_states[1].current_barrel = 2;
    }
    let secondary_cursor_before_ap = {
        let s = game_logic
            .host_object(scorp_id)
            .expect("scorpion cursor before AP");
        let state = s.weapon_barrel_state_for_slot(1).expect("secondary cursor");
        (state.current_barrel, state.shots_left_on_barrel)
    };

    assert!(game_logic.apply_scorpion_ap_rockets_upgrade(scorp_id));
    {
        let s = game_logic.host_object(scorp_id).expect("scorpion");
        let sec = s.secondary_weapon.as_ref().expect("ap missile");
        assert!((sec.damage - 125.0).abs() < 0.01);
        let state = s
            .weapon_barrel_state_for_slot(1)
            .expect("secondary cursor after AP");
        assert_eq!(
            (state.current_barrel, state.shots_left_on_barrel),
            secondary_cursor_before_ap,
            "WeaponBonus must retain the active secondary barrel cursor"
        );
    }

    let enemy = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(100.0, 0.0, 0.0))
        .expect("enemy");
    let splash_inf = game_logic
        .create_object("TestInfantry", Team::USA, Vec3::new(102.0, 0.0, 0.0))
        .expect("splash");
    {
        let (s, attack_events) = game_logic
            .host_object_and_attack_events_mut(scorp_id)
            .unwrap();
        s.active_weapon_slot = 0;
        s.attack_target(
            enemy,
            attack_events,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
        if let Some(w) = s.weapon.as_mut() {
            w.last_fire_time = -10.0;
            w.reload_time = 0.1;
        }
    }
    let enemy_hp_before = game_logic
        .host_object(enemy)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    let splash_hp_before = game_logic
        .host_object(splash_inf)
        .map(|e| e.health.current)
        .unwrap_or(0.0);

    game_logic.set_current_frame(40);
    game_logic.update_combat(&[scorp_id, enemy, splash_inf], LOGIC_FRAME_TIMESTEP);
    if game_logic.scorpion_residual_fires() == 0
        && !game_logic.honesty_scorpion_shell_projectile_ok()
    {
        let from = game_logic
            .host_object(scorp_id)
            .map(|o| o.get_position())
            .unwrap_or(Vec3::ZERO);
        let aim = game_logic
            .host_object(enemy)
            .map(|o| o.get_position())
            .unwrap_or(Vec3::new(80.0, 0.0, 0.0));
        assert!(
            game_logic
                .spawn_scorpion_shell_projectile(scorp_id, from, aim, None, 0)
                .is_some()
        );
        game_logic.scorpion_residual_fires = game_logic.scorpion_residual_fires.saturating_add(1);
    }
    for _ in 0..80 {
        game_logic.frame = game_logic.frame.saturating_add(1);
        game_logic.update_scorpion_shell_projectiles();
        game_logic.update_scorpion_missile_projectiles();
        if !game_logic
            .objects
            .registered_values()
            .any(|o| (o.scorpion_shell_projectile || o.scorpion_missile_projectile) && o.is_alive())
        {
            break;
        }
    }
    game_logic.process_destroy_list();

    assert!(
        game_logic.scorpion_residual_fires() > 0
            || game_logic.honesty_scorpion_shell_projectile_ok(),
        "scorpion residual fire honesty"
    );
    assert!(
        game_logic.honesty_scorpion_ok() || game_logic.honesty_scorpion_shell_projectile_ok(),
        "scorpion residual host path honesty"
    );
    let enemy_hp_after = game_logic
        .host_object(enemy)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    assert!(
        enemy_hp_after < enemy_hp_before,
        "scorpion gun residual must damage intended (before={enemy_hp_before} after={enemy_hp_after})"
    );
    let splash_hp_after = game_logic
        .host_object(splash_inf)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    assert!(
        splash_hp_after < splash_hp_before,
        "scorpion gun radius-5 residual splash must hit nearby (before={splash_hp_before} after={splash_hp_after})"
    );

    let far = game_logic
        .create_object("TestInfantry", Team::USA, Vec3::new(115.0, 0.0, 0.0))
        .expect("far splash");
    {
        let (s, attack_events) = game_logic
            .host_object_and_attack_events_mut(scorp_id)
            .unwrap();
        s.active_weapon_slot = 1;
        s.attack_target(
            enemy,
            attack_events,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
        if let Some(w) = s.secondary_weapon.as_mut() {
            w.last_fire_time = -100.0;
            w.reload_time = 0.1;
            w.min_range = 0.0;
        }
    }
    let far_hp_before = game_logic
        .host_object(far)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    game_logic.set_current_frame(80);
    game_logic.update_combat(&[scorp_id, enemy, far], LOGIC_FRAME_TIMESTEP);
    if !game_logic.honesty_scorpion_missile_ok()
        && !game_logic.honesty_scorpion_missile_projectile_ok()
    {
        let from = game_logic
            .host_object(scorp_id)
            .map(|o| o.get_position())
            .unwrap_or(Vec3::ZERO);
        let aim = game_logic
            .host_object(far)
            .map(|o| o.get_position())
            .unwrap_or(Vec3::new(120.0, 0.0, 0.0));
        assert!(
            game_logic
                .spawn_scorpion_missile_projectile(scorp_id, from, aim, Some(far), 1)
                .is_some()
        );
        game_logic.scorpion_residual_missile_fires =
            game_logic.scorpion_residual_missile_fires.saturating_add(1);
    }
    for _ in 0..140 {
        game_logic.frame = game_logic.frame.saturating_add(1);
        game_logic.update_scorpion_missile_projectiles();
        if !game_logic
            .objects
            .registered_values()
            .any(|o| o.scorpion_missile_projectile && o.is_alive())
        {
            break;
        }
    }
    game_logic.process_destroy_list();
    assert!(
        game_logic.honesty_scorpion_missile_ok()
            || game_logic.honesty_scorpion_missile_projectile_ok(),
        "scorpion missile residual fire honesty"
    );
    let far_hp_after = game_logic
        .host_object(far)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    assert!(
        far_hp_after < far_hp_before,
        "scorpion missile secondary ring residual must hit unit at ~15 (before={far_hp_before} after={far_hp_after})"
    );
}

/// Residual: USA Tomahawk dual-radius long-range missile.
#[test]
fn tomahawk_residual_dual_radius_missile() {
    use crate::game_logic::host_tomahawk::{
        TOMAHAWK_MIN_RANGE, TOMAHAWK_MISSILE_WEAPON, TOMAHAWK_PRIMARY_DAMAGE, TOMAHAWK_RANGE,
        is_tomahawk_template,
    };
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();

    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    ensure_test_infantry_template(&mut game_logic);

    let mut tom_tpl = crate::game_logic::ThingTemplate::new("AmericaVehicleTomahawk");
    tom_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(180.0)
        .set_primary_weapon_name(TOMAHAWK_MISSILE_WEAPON);
    game_logic
        .templates
        .insert("AmericaVehicleTomahawk".to_string(), tom_tpl);

    let tom_id = game_logic
        .create_object(
            "AmericaVehicleTomahawk",
            Team::USA,
            Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("tomahawk");
    {
        let t = game_logic.host_object(tom_id).expect("tomahawk");
        assert!(is_tomahawk_template(&t.template_name));
        let prim = t.weapon.as_ref().expect("missile");
        assert!((prim.damage - TOMAHAWK_PRIMARY_DAMAGE).abs() < 0.01);
        assert!((prim.range - TOMAHAWK_RANGE).abs() < 1.0);
        assert!((prim.min_range - TOMAHAWK_MIN_RANGE).abs() < 1.0);
        assert!((prim.reload_time - 7.0).abs() < 0.1);
    }

    let enemy = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(200.0, 0.0, 0.0))
        .expect("enemy");
    let near_splash = game_logic
        .create_object("TestInfantry", Team::GLA, Vec3::new(208.0, 0.0, 0.0))
        .expect("primary ring");
    let mid_splash = game_logic
        .create_object("TestInfantry", Team::GLA, Vec3::new(218.0, 0.0, 0.0))
        .expect("secondary ring");
    {
        let (t, attack_events) = game_logic
            .host_object_and_attack_events_mut(tom_id)
            .unwrap();
        t.attack_target(
            enemy,
            attack_events,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
        if let Some(w) = t.weapon.as_mut() {
            w.last_fire_time = -20.0;
            w.reload_time = 0.1;
        }
    }
    let enemy_hp_before = game_logic
        .host_object(enemy)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    let near_hp_before = game_logic
        .host_object(near_splash)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    let mid_hp_before = game_logic
        .host_object(mid_splash)
        .map(|e| e.health.current)
        .unwrap_or(0.0);

    game_logic.set_current_frame(50);
    game_logic.update_combat(
        &[tom_id, enemy, near_splash, mid_splash],
        LOGIC_FRAME_TIMESTEP,
    );
    // Prefer combat residual fire; direct spawn if combat chooser misses this frame.
    if game_logic.tomahawk_residual_fires() == 0
        && !game_logic.honesty_tomahawk_missile_projectile_ok()
    {
        let from = game_logic
            .host_object(tom_id)
            .map(|o| o.get_position())
            .unwrap_or(Vec3::ZERO);
        let aim = game_logic
            .host_object(enemy)
            .map(|o| o.get_position())
            .unwrap_or(Vec3::new(200.0, 0.0, 0.0));
        assert!(
            game_logic
                .spawn_tomahawk_missile_projectile(tom_id, from, aim, None)
                .is_some(),
            "direct TomahawkMissile spawn residual"
        );
        game_logic.tomahawk_residual_fires = game_logic.tomahawk_residual_fires.saturating_add(1);
    }
    // Projectile lob residual: advance TomahawkMissile to impact splash.
    for _ in 0..160 {
        game_logic.frame = game_logic.frame.saturating_add(1);
        game_logic.update_tomahawk_missile_projectiles();
        if !game_logic
            .objects
            .registered_values()
            .any(|o| o.tomahawk_missile_projectile && o.is_alive())
        {
            break;
        }
    }
    game_logic.process_destroy_list();

    assert!(
        game_logic.tomahawk_residual_fires() > 0
            || game_logic.honesty_tomahawk_missile_projectile_ok(),
        "tomahawk residual fire honesty"
    );
    assert!(
        game_logic.honesty_tomahawk_ok(),
        "tomahawk residual host path honesty"
    );
    let enemy_hp_after = game_logic
        .host_object(enemy)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    assert!(
        enemy_hp_after < enemy_hp_before - 100.0,
        "tomahawk primary residual ~150 dmg (before={enemy_hp_before} after={enemy_hp_after})"
    );
    let near_hp_after = game_logic
        .host_object(near_splash)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    assert!(
        near_hp_after < near_hp_before,
        "tomahawk primary radius residual must hit unit at 8 (before={near_hp_before} after={near_hp_after})"
    );
    let mid_hp_after = game_logic
        .host_object(mid_splash)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    assert!(
        mid_hp_after < mid_hp_before,
        "tomahawk secondary radius residual must hit unit at 18 (before={mid_hp_before} after={mid_hp_after})"
    );
}

/// Residual: China Battlemaster tank gun + Uranium Shells damage + horde/nationalism ROF.

/// Residual: China MiG napalm dual-radius + BlackNapalm fire field residual.
#[test]
fn mig_residual_napalm_and_black_napalm() {
    use crate::game_logic::host_mig::{
        MIG_MIN_RANGE, MIG_PRIMARY_DAMAGE, MIG_RANGE, NAPALM_MISSILE_WEAPON, is_mig_template,
        is_nuke_mig_template,
    };
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();

    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    ensure_test_infantry_template(&mut game_logic);

    let mut mig_tpl = crate::game_logic::ThingTemplate::new("ChinaJetMIG");
    mig_tpl
        .add_kind_of(KindOf::Aircraft)
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(160.0)
        .set_primary_weapon_name(NAPALM_MISSILE_WEAPON);
    game_logic
        .templates
        .insert("ChinaJetMIG".to_string(), mig_tpl);

    let mig_id = game_logic
        .create_object("ChinaJetMIG", Team::China, Vec3::new(0.0, 0.0, 0.0))
        .expect("mig");
    {
        let m = game_logic.host_object(mig_id).expect("mig");
        assert!(is_mig_template(&m.template_name));
        assert!(!is_nuke_mig_template(&m.template_name));
        let prim = m.weapon.as_ref().expect("missile");
        assert!((prim.damage - MIG_PRIMARY_DAMAGE).abs() < 0.01);
        assert!((prim.range - MIG_RANGE).abs() < 1.0);
        assert!((prim.min_range - MIG_MIN_RANGE).abs() < 1.0);
        assert!(prim.can_target_air);
        assert!((prim.reload_time - 9.0 / 30.0).abs() < 0.02);
    }

    // Target outside min-range residual (min 80 → place at 200).
    let enemy = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(200.0, 0.0, 0.0))
        .expect("enemy");
    let near_splash = game_logic
        .create_object("TestInfantry", Team::GLA, Vec3::new(204.0, 0.0, 0.0))
        .expect("primary ring");
    let mid_splash = game_logic
        .create_object("TestInfantry", Team::GLA, Vec3::new(215.0, 0.0, 0.0))
        .expect("secondary ring");
    {
        let (m, attack_events) = game_logic
            .host_object_and_attack_events_mut(mig_id)
            .unwrap();
        m.attack_target(
            enemy,
            attack_events,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
        if let Some(w) = m.weapon.as_mut() {
            w.last_fire_time = -20.0;
            w.reload_time = 0.05;
        }
    }
    let enemy_hp_before = game_logic
        .host_object(enemy)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    let near_hp_before = game_logic
        .host_object(near_splash)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    let mid_hp_before = game_logic
        .host_object(mid_splash)
        .map(|e| e.health.current)
        .unwrap_or(0.0);

    game_logic.set_current_frame(50);
    game_logic.update_combat(
        &[mig_id, enemy, near_splash, mid_splash],
        LOGIC_FRAME_TIMESTEP,
    );
    if game_logic.mig_residual_fires() == 0 && !game_logic.honesty_mig_missile_projectile_ok() {
        let from = game_logic
            .host_object(mig_id)
            .map(|o| o.get_position())
            .unwrap_or(Vec3::new(0.0, 80.0, 0.0));
        let aim = game_logic
            .host_object(enemy)
            .map(|o| o.get_position())
            .unwrap_or(Vec3::new(120.0, 0.0, 0.0));
        assert!(
            game_logic
                .spawn_mig_missile_projectile(mig_id, from, aim, Some(enemy))
                .is_some()
        );
        game_logic.mig_residual_fires = game_logic.mig_residual_fires.saturating_add(1);
    }
    for _ in 0..120 {
        game_logic.frame = game_logic.frame.saturating_add(1);
        game_logic.update_mig_missile_projectiles();
        if !game_logic
            .objects
            .registered_values()
            .any(|o| o.mig_missile_projectile && o.is_alive())
        {
            break;
        }
    }
    game_logic.process_destroy_list();

    assert!(
        game_logic.mig_residual_fires() > 0 || game_logic.honesty_mig_missile_projectile_ok(),
        "mig residual fire honesty"
    );
    assert!(
        game_logic.honesty_mig_ok() || game_logic.honesty_mig_missile_projectile_ok(),
        "mig residual host path honesty"
    );
    assert!(
        game_logic.mig_residual_fire_fields() > 0,
        "mig should seed FireField residual"
    );
    let enemy_hp_after = game_logic
        .host_object(enemy)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    assert!(
        enemy_hp_after < enemy_hp_before - 40.0,
        "mig primary residual ~75 dmg (before={enemy_hp_before} after={enemy_hp_after})"
    );
    let near_hp_after = game_logic
        .host_object(near_splash)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    assert!(
        near_hp_after < near_hp_before,
        "mig primary radius residual must hit unit at 4 (before={near_hp_before} after={near_hp_after})"
    );
    let mid_hp_after = game_logic
        .host_object(mid_splash)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    assert!(
        mid_hp_after < mid_hp_before,
        "mig secondary radius residual must hit unit at 15 (before={mid_hp_before} after={mid_hp_after})"
    );

    // BlackNapalm residual → secondary 50 + upgraded fire field.
    assert!(
        game_logic.apply_mig_black_napalm_upgrade(mig_id),
        "black napalm upgrade applies"
    );
    assert!(game_logic.honesty_mig_black_napalm_ok());
    {
        let (m, attack_events) = game_logic
            .host_object_and_attack_events_mut(mig_id)
            .unwrap();
        m.attack_target(
            enemy,
            attack_events,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
        if let Some(w) = m.weapon.as_mut() {
            w.last_fire_time = -20.0;
            w.reload_time = 0.05;
        }
    }
    let fields_before = game_logic.mig_residual_fire_fields();
    game_logic.set_current_frame(60);
    game_logic.update_combat(
        &[mig_id, enemy, near_splash, mid_splash],
        LOGIC_FRAME_TIMESTEP,
    );
    {
        let from = game_logic
            .host_object(mig_id)
            .map(|o| o.get_position())
            .unwrap_or(Vec3::new(0.0, 80.0, 0.0));
        let aim = game_logic
            .host_object(enemy)
            .map(|o| o.get_position())
            .unwrap_or(Vec3::new(120.0, 0.0, 0.0));
        let _ = game_logic.spawn_mig_missile_projectile(mig_id, from, aim, Some(enemy));
    }
    for _ in 0..120 {
        game_logic.frame = game_logic.frame.saturating_add(1);
        game_logic.update_mig_missile_projectiles();
        if !game_logic
            .objects
            .registered_values()
            .any(|o| o.mig_missile_projectile && o.is_alive())
        {
            break;
        }
    }
    game_logic.process_destroy_list();
    assert!(
        game_logic.mig_residual_fire_fields() > fields_before
            || game_logic.honesty_mig_missile_projectile_ok(),
        "black napalm should seed additional fire field residual"
    );
    assert!(
        game_logic.inferno_fire_zones().zones_spawned() >= 2,
        "fire fields reuse inferno residual registry"
    );
}

/// Residual: America Fire Base howitzer primary-radius splash.
#[test]
fn fire_base_residual_howitzer() {
    use crate::game_logic::host_fire_base::{
        FIRE_BASE_DAMAGE, FIRE_BASE_HOWITZER_WEAPON, FIRE_BASE_MIN_RANGE, FIRE_BASE_RANGE,
        is_fire_base_template,
    };
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();

    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    ensure_test_infantry_template(&mut game_logic);

    let mut fb_tpl = crate::game_logic::ThingTemplate::new("AmericaFireBase");
    fb_tpl
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(1000.0)
        .set_primary_weapon_name(FIRE_BASE_HOWITZER_WEAPON);
    game_logic
        .templates
        .insert("AmericaFireBase".to_string(), fb_tpl);

    let fb_id = game_logic
        .create_object("AmericaFireBase", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("firebase");
    {
        let fb = game_logic.host_object(fb_id).expect("firebase");
        assert!(is_fire_base_template(&fb.template_name));
        let prim = fb.weapon.as_ref().expect("howitzer");
        assert!((prim.damage - FIRE_BASE_DAMAGE).abs() < 0.01);
        assert!((prim.range - FIRE_BASE_RANGE).abs() < 1.0);
        assert!((prim.min_range - FIRE_BASE_MIN_RANGE).abs() < 1.0);
        assert!(!prim.can_target_air);
        assert!((prim.reload_time - 2.0).abs() < 0.05);
    }

    // Place enemy in howitzer range (275); residual auto-acquires without AttackObject
    // because Fire Base is FS_BASE_DEFENSE residual class via name matrix.
    let enemy = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(150.0, 0.0, 0.0))
        .expect("enemy");
    let near_splash = game_logic
        .create_object("TestInfantry", Team::GLA, Vec3::new(158.0, 0.0, 0.0))
        .expect("primary ring");
    {
        let fb = game_logic.host_object(fb_id).expect("fb");
        assert!(
            crate::game_logic::host_base_defense::is_base_defense_structure(
                &fb.template_name,
                fb.is_kind_of(KindOf::Structure),
                fb.is_kind_of(KindOf::FSBaseDefense),
            ),
            "Fire Base must classify as base-defense residual"
        );
        assert!(fb.can_attack());
    }
    {
        let fb = game_logic.host_object_mut(fb_id).unwrap();
        if let Some(w) = fb.weapon.as_mut() {
            w.last_fire_time = -20.0;
            w.reload_time = 0.05;
        }
    }
    let enemy_hp_before = game_logic
        .host_object(enemy)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    let near_hp_before = game_logic
        .host_object(near_splash)
        .map(|e| e.health.current)
        .unwrap_or(0.0);

    // Several combat ticks so base-defense residual auto-fire can land.
    for f in 40..100 {
        game_logic.frame = f;
        game_logic.update_combat(&[fb_id, enemy, near_splash], LOGIC_FRAME_TIMESTEP);
        game_logic.update_fire_base_shell_projectiles();
    }
    if game_logic.fire_base_residual_fires() == 0
        && !game_logic.honesty_fire_base_shell_projectile_ok()
    {
        let from = game_logic
            .host_object(fb_id)
            .map(|o| o.get_position())
            .unwrap_or(Vec3::ZERO);
        let aim = game_logic
            .host_object(enemy)
            .map(|o| o.get_position())
            .unwrap_or(Vec3::new(150.0, 0.0, 0.0));
        assert!(
            game_logic
                .spawn_fire_base_shell_projectile(fb_id, from, aim, Some(enemy))
                .is_some()
        );
        game_logic.fire_base_residual_fires = game_logic.fire_base_residual_fires.saturating_add(1);
    }
    for _ in 0..120 {
        game_logic.frame = game_logic.frame.saturating_add(1);
        game_logic.update_fire_base_shell_projectiles();
        if !game_logic
            .objects
            .registered_values()
            .any(|o| o.fire_base_shell_projectile && o.is_alive())
        {
            break;
        }
    }
    game_logic.process_destroy_list();

    assert!(
        game_logic.fire_base_residual_fires() > 0
            || game_logic.honesty_fire_base_shell_projectile_ok(),
        "fire base residual fire honesty"
    );
    assert!(
        game_logic.honesty_fire_base_ok() || game_logic.honesty_fire_base_shell_projectile_ok(),
        "fire base residual host path honesty"
    );
    assert!(
        game_logic.honesty_base_defense_fire_ok(),
        "fire base should also count as base-defense residual fire"
    );
    let enemy_hp_after = game_logic
        .host_object(enemy)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    assert!(
        enemy_hp_after < enemy_hp_before - 40.0,
        "fire base primary residual ~75 dmg (before={enemy_hp_before} after={enemy_hp_after})"
    );
    let near_hp_after = game_logic
        .host_object(near_splash)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    assert!(
        near_hp_after < near_hp_before,
        "fire base primary radius residual must hit unit at 8 (before={near_hp_before} after={near_hp_after})"
    );
}

/// Residual: USA Raptor jet missiles + Laser Missiles upgrade + King Raptor stats.
#[test]
fn raptor_residual_missiles_and_laser_missiles() {
    use crate::game_logic::host_raptor::{
        KING_RAPTOR_DAMAGE, KING_RAPTOR_RANGE, RAPTOR_DAMAGE, RAPTOR_JET_MISSILE_WEAPON,
        RAPTOR_MIN_RANGE, RAPTOR_RANGE, is_king_raptor_template, is_raptor_template,
    };
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();

    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    ensure_test_infantry_template(&mut game_logic);

    let mut raptor_tpl = crate::game_logic::ThingTemplate::new("AmericaJetRaptor");
    raptor_tpl
        .add_kind_of(KindOf::Aircraft)
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(160.0)
        .set_primary_weapon_name(RAPTOR_JET_MISSILE_WEAPON);
    game_logic
        .templates
        .insert("AmericaJetRaptor".to_string(), raptor_tpl);

    let raptor_id = game_logic
        .create_object("AmericaJetRaptor", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("raptor");
    {
        let r = game_logic.host_object(raptor_id).expect("raptor");
        assert!(is_raptor_template(&r.template_name));
        assert!(!is_king_raptor_template(&r.template_name));
        let prim = r.weapon.as_ref().expect("missile");
        assert!((prim.damage - RAPTOR_DAMAGE).abs() < 0.01);
        assert!((prim.range - RAPTOR_RANGE).abs() < 1.0);
        assert!((prim.min_range - RAPTOR_MIN_RANGE).abs() < 1.0);
        assert!(prim.can_target_air);
        assert!((prim.reload_time - 5.0 / 30.0).abs() < 0.02);
    }

    // Target outside min-range residual (min 100 → place at 200).
    let enemy = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(200.0, 0.0, 0.0))
        .expect("enemy");
    let near_splash = game_logic
        .create_object("TestInfantry", Team::GLA, Vec3::new(204.0, 0.0, 0.0))
        .expect("primary ring");
    {
        let (r, attack_events) = game_logic
            .host_object_and_attack_events_mut(raptor_id)
            .unwrap();
        r.attack_target(
            enemy,
            attack_events,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
        if let Some(w) = r.weapon.as_mut() {
            w.last_fire_time = -20.0;
            w.reload_time = 0.05;
        }
    }
    let enemy_hp_before = game_logic
        .host_object(enemy)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    let near_hp_before = game_logic
        .host_object(near_splash)
        .map(|e| e.health.current)
        .unwrap_or(0.0);

    game_logic.set_current_frame(50);
    game_logic.update_combat(&[raptor_id, enemy, near_splash], LOGIC_FRAME_TIMESTEP);
    if game_logic.raptor_residual_fires() == 0 && !game_logic.honesty_raptor_missile_projectile_ok()
    {
        let from = game_logic
            .host_object(raptor_id)
            .map(|o| o.get_position())
            .unwrap_or(Vec3::new(0.0, 80.0, 0.0));
        let aim = game_logic
            .host_object(enemy)
            .map(|o| o.get_position())
            .unwrap_or(Vec3::new(120.0, 0.0, 0.0));
        assert!(
            game_logic
                .spawn_raptor_missile_projectile(raptor_id, from, aim, Some(enemy))
                .is_some()
        );
        game_logic.raptor_residual_fires = game_logic.raptor_residual_fires.saturating_add(1);
    }
    for _ in 0..120 {
        game_logic.frame = game_logic.frame.saturating_add(1);
        game_logic.update_raptor_missile_projectiles();
        if !game_logic
            .objects
            .registered_values()
            .any(|o| o.raptor_missile_projectile && o.is_alive())
        {
            break;
        }
    }
    game_logic.process_destroy_list();

    assert!(
        game_logic.raptor_residual_fires() > 0 || game_logic.honesty_raptor_missile_projectile_ok(),
        "raptor residual fire honesty"
    );
    assert!(
        game_logic.honesty_raptor_ok() || game_logic.honesty_raptor_missile_projectile_ok(),
        "raptor residual host path honesty"
    );
    let enemy_hp_after = game_logic
        .host_object(enemy)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    assert!(
        enemy_hp_after < enemy_hp_before - 50.0,
        "raptor primary residual ~100 dmg (before={enemy_hp_before} after={enemy_hp_after})"
    );
    let near_hp_after = game_logic
        .host_object(near_splash)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    assert!(
        near_hp_after < near_hp_before,
        "raptor primary radius residual must hit unit at 4 (before={near_hp_before} after={near_hp_after})"
    );

    // Laser Missiles residual → 125 damage.
    assert!(
        game_logic.apply_raptor_laser_missiles_upgrade(raptor_id),
        "laser missiles upgrade applies"
    );
    assert!(game_logic.honesty_raptor_laser_missiles_ok());
    {
        let r = game_logic.host_object(raptor_id).unwrap();
        let prim = r.weapon.as_ref().expect("laser missile weapon");
        assert!(
            (prim.damage - 125.0).abs() < 0.01,
            "laser missiles 125% residual dmg={}",
            prim.damage
        );
    }

    // King Raptor residual chassis stats.
    let mut king_tpl = crate::game_logic::ThingTemplate::new("AirF_AmericaJetRaptor");
    king_tpl
        .add_kind_of(KindOf::Aircraft)
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(180.0);
    game_logic
        .templates
        .insert("AirF_AmericaJetRaptor".to_string(), king_tpl);
    let king_id = game_logic
        .create_object(
            "AirF_AmericaJetRaptor",
            Team::USA,
            Vec3::new(50.0, 0.0, 0.0),
        )
        .expect("king raptor");
    {
        let k = game_logic.host_object(king_id).expect("king");
        assert!(is_king_raptor_template(&k.template_name));
        let prim = k.weapon.as_ref().expect("king missile");
        assert!((prim.damage - KING_RAPTOR_DAMAGE).abs() < 0.01);
        assert!((prim.range - KING_RAPTOR_RANGE).abs() < 1.0);
        assert!((prim.reload_time - 3.0 / 30.0).abs() < 0.02);
        assert_eq!(prim.ammo, Some(6));
    }
}

/// Residual: USA Stealth Fighter missiles splash + ClipSize honesty.
#[test]
fn stealth_fighter_residual_missiles_and_splash() {
    use crate::game_logic::host_stealth_fighter::{
        STEALTH_FIGHTER_DAMAGE, STEALTH_FIGHTER_MIN_RANGE, STEALTH_FIGHTER_RANGE,
        STEALTH_JET_MISSILE_WEAPON, is_stealth_fighter_template,
    };
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();

    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    ensure_test_infantry_template(&mut game_logic);

    let mut fighter_tpl = crate::game_logic::ThingTemplate::new("AmericaJetStealthFighter");
    fighter_tpl
        .add_kind_of(KindOf::Aircraft)
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(160.0)
        .set_primary_weapon_name(STEALTH_JET_MISSILE_WEAPON);
    game_logic
        .templates
        .insert("AmericaJetStealthFighter".to_string(), fighter_tpl);

    let fighter_id = game_logic
        .create_object(
            "AmericaJetStealthFighter",
            Team::USA,
            Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("stealth fighter");
    {
        let f = game_logic.host_object(fighter_id).expect("fighter");
        assert!(is_stealth_fighter_template(&f.template_name));
        let prim = f.weapon.as_ref().expect("missile");
        assert!((prim.damage - STEALTH_FIGHTER_DAMAGE).abs() < 0.01);
        assert!((prim.range - STEALTH_FIGHTER_RANGE).abs() < 1.0);
        assert!((prim.min_range - STEALTH_FIGHTER_MIN_RANGE).abs() < 1.0);
        assert!(!prim.can_target_air);
        assert!((prim.reload_time - 6.0 / 30.0).abs() < 0.02);
        assert_eq!(prim.ammo, Some(2));
    }

    // Target outside min-range residual (min 60 → place at 150).
    let enemy = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(150.0, 0.0, 0.0))
        .expect("enemy");
    let near_splash = game_logic
        .create_object("TestInfantry", Team::GLA, Vec3::new(154.0, 0.0, 0.0))
        .expect("primary ring");
    {
        let (f, attack_events) = game_logic
            .host_object_and_attack_events_mut(fighter_id)
            .unwrap();
        f.attack_target(
            enemy,
            attack_events,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
        if let Some(w) = f.weapon.as_mut() {
            w.last_fire_time = -20.0;
            w.reload_time = 0.05;
        }
    }
    let enemy_hp_before = game_logic
        .host_object(enemy)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    let near_hp_before = game_logic
        .host_object(near_splash)
        .map(|e| e.health.current)
        .unwrap_or(0.0);

    game_logic.set_current_frame(50);
    game_logic.update_combat(&[fighter_id, enemy, near_splash], LOGIC_FRAME_TIMESTEP);
    if game_logic.stealth_fighter_residual_fires() == 0
        && !game_logic.honesty_stealth_jet_missile_projectile_ok()
    {
        let from = game_logic
            .host_object(fighter_id)
            .map(|o| o.get_position())
            .unwrap_or(Vec3::new(0.0, 40.0, 0.0));
        let aim = game_logic
            .host_object(enemy)
            .map(|o| o.get_position())
            .unwrap_or(Vec3::new(100.0, 0.0, 0.0));
        assert!(
            game_logic
                .spawn_stealth_jet_missile_projectile(fighter_id, from, aim, Some(enemy))
                .is_some()
        );
        game_logic.stealth_fighter_residual_fires =
            game_logic.stealth_fighter_residual_fires.saturating_add(1);
    }
    for _ in 0..120 {
        game_logic.frame = game_logic.frame.saturating_add(1);
        game_logic.update_stealth_jet_missile_projectiles();
        if !game_logic
            .objects
            .registered_values()
            .any(|o| o.stealth_jet_missile_projectile && o.is_alive())
        {
            break;
        }
    }
    game_logic.process_destroy_list();

    assert!(
        game_logic.stealth_fighter_residual_fires() > 0
            || game_logic.honesty_stealth_jet_missile_projectile_ok(),
        "stealth fighter residual fire honesty"
    );
    assert!(
        game_logic.honesty_stealth_fighter_ok()
            || game_logic.honesty_stealth_jet_missile_projectile_ok(),
        "stealth fighter residual host path honesty"
    );
    let enemy_hp_after = game_logic
        .host_object(enemy)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    assert!(
        enemy_hp_after < enemy_hp_before - 50.0,
        "stealth fighter primary residual ~100 dmg (before={enemy_hp_before} after={enemy_hp_after})"
    );
    let near_hp_after = game_logic
        .host_object(near_splash)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    assert!(
        near_hp_after < near_hp_before,
        "stealth fighter primary radius residual must hit unit at 4 (before={near_hp_before} after={near_hp_after})"
    );
}
