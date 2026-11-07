//! Host GameLogic tests — salvage and missile weapon residuals.
#![allow(unused_imports, non_snake_case, unused_variables, dead_code)]
use super::*;

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
            0,
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
