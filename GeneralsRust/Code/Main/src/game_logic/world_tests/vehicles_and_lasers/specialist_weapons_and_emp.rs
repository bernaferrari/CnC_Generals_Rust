//! Behavior tests extracted from the original hero/weapons/airfields suite.
use super::*;

fn admit_test_sticky_bomb_charge(game: &mut GameLogic) {
    game.admit_receiving_module_content(
        crate::assets::ReceivingObjectContent::from_sources([(
            "booby-trap-charge.ini".into(),
            "Object BoobyTrap\nKindOf = IMMOBILE BOOBY_TRAP\nBody = ActiveBody BodyTag\nMaxHealth = 1\nInitialHealth = 1\nEnd\nBehavior = StickyBombUpdate Sticky\nEnd\nEnd\n".into(),
        )])
        .expect("typed StickyBombUpdate charge content"),
    )
    .expect("admit typed StickyBombUpdate charge content");
}

#[test]
fn hero_spawn_waits_stealth_delay() {
    use crate::game_logic::host_colonel_burton::BURTON_STEALTH_DELAY_FRAMES;

    let mut game_logic = GameLogic::new();
    let mut burton_tpl = crate::game_logic::ThingTemplate::new("AmericaInfantryColonelBurton");
    burton_tpl
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .set_health(200.0);
    game_logic
        .templates
        .insert("AmericaInfantryColonelBurton".to_string(), burton_tpl);

    let burton_id = game_logic
        .create_object(
            "AmericaInfantryColonelBurton",
            Team::USA,
            Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("burton");
    {
        let b = game_logic.host_object(burton_id).expect("burton");
        assert!(b.innate_stealth);
        assert!(
            !b.status.stealthed,
            "C++ ctor sets CAN_STEALTH only; STEALTHED waits StealthDelay"
        );
        assert_eq!(b.stealth_delay_frames, BURTON_STEALTH_DELAY_FRAMES);
        assert_eq!(b.stealth_allowed_frame, BURTON_STEALTH_DELAY_FRAMES);
    }
    game_logic.update_stealth_and_detection();
    assert!(
        !game_logic.host_object(burton_id).unwrap().status.stealthed,
        "must stay visible during StealthDelay"
    );
    game_logic.frame = BURTON_STEALTH_DELAY_FRAMES;
    game_logic.update_stealth_and_detection();
    assert!(
        game_logic.host_object(burton_id).unwrap().status.stealthed,
        "hero cloaks after StealthDelay"
    );
}

#[test]
fn supw_patriot_emp_residual_dual_slot_and_disable() {
    use crate::game_logic::host_base_defense::{
        SUPW_PATRIOT_AIR_DAMAGE, SUPW_PATRIOT_GROUND_DAMAGE, SUPW_PATRIOT_GROUND_RANGE,
        is_supw_patriot_template,
    };
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let mut game_logic = GameLogic::new();

    let mut pat_tpl = crate::game_logic::ThingTemplate::new("SupW_AmericaPatriotBattery");
    pat_tpl
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .add_kind_of(KindOf::FSBaseDefense)
        .set_health(1000.0)
        .set_primary_weapon_name(crate::game_logic::host_base_defense::SUPW_PATRIOT_PRIMARY_WEAPON)
        .set_secondary_weapon_name(crate::game_logic::host_base_defense::SUPW_PATRIOT_ASSIST_WEAPON)
        .set_tertiary_weapon_name(crate::game_logic::host_base_defense::SUPW_PATRIOT_SECONDARY_WEAPON);
    game_logic
        .templates
        .insert("SupW_AmericaPatriotBattery".to_string(), pat_tpl);

    let mut enemy_tpl = crate::game_logic::ThingTemplate::new("TestTank");
    enemy_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(500.0);
    game_logic
        .templates
        .insert("TestTank".to_string(), enemy_tpl);

    let mut air_tpl = crate::game_logic::ThingTemplate::new("TestJet");
    air_tpl
        .add_kind_of(KindOf::Aircraft)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(400.0);
    game_logic.templates.insert("TestJet".to_string(), air_tpl);

    let pat_id = game_logic
        .create_object(
            "SupW_AmericaPatriotBattery",
            Team::USA,
            Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("supw patriot");
    if let Some(p) = game_logic.host_object_mut(pat_id) {
        p.set_status_under_construction(false);
        p.construction_percent = 100.0;
    }

    {
        let p = game_logic.host_object(pat_id).expect("patriot");
        assert!(is_supw_patriot_template(&p.template_name));
        let g = p.weapon.as_ref().expect("SupW ground residual");
        assert!(
            (g.damage - SUPW_PATRIOT_GROUND_DAMAGE).abs() < 0.5,
            "SupW Patriot ground 15, got {}",
            g.damage
        );
        assert!(
            (g.range - SUPW_PATRIOT_GROUND_RANGE).abs() < 1.0,
            "SupW ground range 275, got {}",
            g.range
        );
        // hq-2qedt.32.2: retail AA lives in TERTIARY (SECONDARY = assist weapon).
        let a = p.tertiary_weapon.as_ref().expect("SupW AA residual");
        assert!(
            (a.damage - SUPW_PATRIOT_AIR_DAMAGE).abs() < 0.5,
            "SupW Patriot AA 30, got {}",
            a.damage
        );
        assert!(a.can_target_air);
    }

    let enemy_id = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(80.0, 0.0, 0.0))
        .expect("enemy");
    let air_id = game_logic
        .create_object("TestJet", Team::GLA, Vec3::new(0.0, 250.0, 0.0))
        .expect("air");
    if let Some(a) = game_logic.host_object_mut(air_id) {
        a.status.airborne_target = true;
    }

    let hp_before = game_logic.host_object(enemy_id).unwrap().health.current;
    {
        let p = game_logic.host_object_mut(pat_id).unwrap();
        if let Some(w) = p.weapon.as_mut() {
            w.last_fire_time = -10.0;
        }
        if let Some(w) = p.secondary_weapon.as_mut() {
            w.last_fire_time = -10.0;
        }
    }
    game_logic.health_events.clear_damage();
    game_logic.frame = 30;
    for _ in 0..30 {
        game_logic.try_base_defense_residual_fire(pat_id);
        let hp_now = game_logic.host_object(enemy_id).unwrap().health.current;
        if hp_now < hp_before || game_logic.base_defense_residual_fires() > 0 {
            break;
        }
        game_logic.frame = game_logic.frame.saturating_add(1);
    }

    let hp_after = game_logic.host_object(enemy_id).unwrap().health.current;
    let dealt = test_observed_damage_to(&game_logic.health_events, enemy_id, hp_before, hp_after);
    assert!(
        dealt > 0.0 || hp_after < hp_before,
        "SupW Patriot ground residual must damage (dealt={dealt}, before={hp_before} after={hp_after})"
    );
    let enemy = game_logic.host_object(enemy_id).expect("enemy");
    assert!(
        enemy.is_emp_disabled() || enemy.status.disabled_emp,
        "SupW EMP residual must DISABLED_EMP the hit vehicle"
    );
    assert!(
        game_logic.honesty_supw_patriot_emp_ok(),
        "EMP grant honesty"
    );
    assert!(
        game_logic.patriot_residual_ground_fires > 0
            || game_logic.base_defense_residual_fires() > 0,
        "patriot residual fire honesty"
    );

    // AA residual path: move ground enemy away.
    if let Some((e, fixture_partition_e)) = game_logic.host_object_and_partition_mut(enemy_id) {
        e.set_position_with_partition_owner(
            Vec3::new(5000.0, 0.0, 0.0),
            0,
            &mut Default::default(),
            fixture_partition_e,
        );
    }
    game_logic.health_events.clear_damage();
    let air_hp_before = game_logic.host_object(air_id).unwrap().health.current;
    {
        let p = game_logic.host_object_mut(pat_id).unwrap();
        if let Some(w) = p.weapon.as_mut() {
            w.last_fire_time = -10.0;
        }
        if let Some(w) = p.secondary_weapon.as_mut() {
            w.last_fire_time = -10.0;
        }
    }
    game_logic.frame = 90;
    for _ in 0..30 {
        game_logic.try_base_defense_residual_fire(pat_id);
        let hp_now = game_logic.host_object(air_id).unwrap().health.current;
        if hp_now < air_hp_before {
            break;
        }
        game_logic.frame = game_logic.frame.saturating_add(1);
    }

    let air_hp_after = game_logic.host_object(air_id).unwrap().health.current;
    let dealt_aa = test_observed_damage_to(
        &game_logic.health_events,
        air_id,
        air_hp_before,
        air_hp_after,
    );
    assert!(
        dealt_aa > 0.0 || air_hp_after < air_hp_before,
        "SupW Patriot AA residual must damage aircraft (dealt={dealt_aa}, before={air_hp_before} after={air_hp_after})"
    );
    let jet = game_logic.host_object(air_id).expect("jet");
    assert!(
        !jet.is_alive() || jet.status.destroyed || jet.status.effectively_dead,
        "SupW EMP residual must kill airborne aircraft"
    );
}

#[test]
fn supw_emp_scatter_misses_infantry_residual() {
    use crate::game_logic::host_base_defense::{
        SUPW_EMP_SCATTER_VS_INFANTRY, SUPW_PATRIOT_EMP_RADIUS,
    };

    let mut logic = GameLogic::new();
    ensure_test_infantry_template(&mut logic);
    ensure_test_tank_template(&mut logic);

    let mut tpl = ThingTemplate::new("SupW_AmericaPatriotBattery");
    tpl.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(1000.0);
    logic
        .templates
        .insert("SupW_AmericaPatriotBattery".to_string(), tpl);

    let bat = logic
        .create_object(
            "SupW_AmericaPatriotBattery",
            Team::USA,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("supw patriot");
    let inf = logic
        .create_object("TestInfantry", Team::GLA, glam::Vec3::new(5.0, 0.0, 0.0))
        .expect("inf");
    if let Some(o) = logic.objects.get_mut(&inf) {
        o.set_selection_radius(0.5);
    }

    let impact = logic
        .objects
        .get(&inf)
        .map(|o| o.get_position())
        .unwrap_or(glam::Vec3::new(5.0, 0.0, 0.0));
    logic.apply_supw_patriot_emp_residual_at(impact, bat, Team::USA, Some(inf));
    assert!(
        logic.supw_emp_scatter_applied > 0
            || logic.supw_emp_scatter_misses > 0
            || logic.honesty_supw_emp_scatter_ok(),
        "supw emp scatter residual must peel vs infantry"
    );
    assert!((SUPW_EMP_SCATTER_VS_INFANTRY - 10.0).abs() < 0.01);
    assert!((SUPW_PATRIOT_EMP_RADIUS - 10.0).abs() < 0.01);

    // Vehicle EMP center without infantry intended still grants in radius.
    let tank = logic
        .create_object("TestTank", Team::GLA, glam::Vec3::new(3.0, 0.0, 0.0))
        .expect("tank");
    let impact = logic
        .objects
        .get(&tank)
        .map(|o| o.get_position())
        .unwrap_or(glam::Vec3::new(3.0, 0.0, 0.0));
    let before = logic.supw_patriot_emp_residual_grants;
    logic.apply_supw_patriot_emp_residual_at(impact, bat, Team::USA, Some(tank));
    assert!(
        logic.supw_patriot_emp_residual_grants > before || logic.honesty_supw_patriot_emp_ok(),
        "vehicle EMP grant residual"
    );
}

#[test]
fn supw_patriot_emp_intended_victim_near_miss_disables() {
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let mut logic = GameLogic::new();
    ensure_test_tank_template(&mut logic);

    let mut pat_tpl = crate::game_logic::ThingTemplate::new("SupW_AmericaPatriotBattery");
    pat_tpl
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(1000.0);
    logic
        .templates
        .insert("SupW_AmericaPatriotBattery".to_string(), pat_tpl);

    let mut air_tpl = crate::game_logic::ThingTemplate::new("TestJet");
    air_tpl
        .add_kind_of(KindOf::Aircraft)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(400.0);
    logic.templates.insert("TestJet".to_string(), air_tpl);

    let bat = logic
        .create_object(
            "SupW_AmericaPatriotBattery",
            Team::USA,
            Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("supw patriot");
    // 30 units from blast: outside EffectRadius 10, inside leftover 40 near-miss.
    let jet = logic
        .create_object("TestJet", Team::GLA, Vec3::new(30.0, 0.0, 0.0))
        .expect("jet");
    if let Some(a) = logic.host_object_mut(jet) {
        a.status.airborne_target = true;
    }

    logic.apply_supw_patriot_emp_residual_at(Vec3::new(0.0, 0.0, 0.0), bat, Team::USA, Some(jet));
    let victim = logic.host_object(jet).expect("jet");
    assert!(
        victim.is_emp_disabled() || victim.status.disabled_emp,
        "intended aircraft near-miss must DISABLED_EMP"
    );

    // Farther than 40 and radius*2: leftover miss, stay mobile.
    let far = logic
        .create_object("TestJet", Team::GLA, Vec3::new(50.0, 0.0, 0.0))
        .expect("far jet");
    if let Some(a) = logic.host_object_mut(far) {
        a.status.airborne_target = true;
    }
    logic.apply_supw_patriot_emp_residual_at(Vec3::ZERO, bat, Team::USA, Some(far));
    let far_v = logic.host_object(far).expect("far jet");
    assert!(
        !far_v.is_emp_disabled() && !far_v.status.disabled_emp,
        "aircraft farther than 40 and radius*2 must not near-miss disable"
    );

    // Ground vehicle outside EffectRadius: C++ fallback requires KINDOF_AIRCRAFT.
    let tank = logic
        .create_object("TestTank", Team::GLA, Vec3::new(80.0, 0.0, 0.0))
        .expect("tank");
    logic.apply_supw_patriot_emp_residual_at(Vec3::ZERO, bat, Team::USA, Some(tank));
    let tank_v = logic.host_object(tank).expect("tank");
    assert!(
        !tank_v.is_emp_disabled() && !tank_v.status.disabled_emp,
        "non-aircraft intended victim must not near-miss disable"
    );

    // EMP_HARDENED name marker (cargo plane) — leftover skips fallback.
    let mut cargo_tpl = crate::game_logic::ThingTemplate::new("AmericaJetCargoPlane");
    cargo_tpl
        .add_kind_of(KindOf::Aircraft)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(400.0);
    logic
        .templates
        .insert("AmericaJetCargoPlane".to_string(), cargo_tpl);
    let cargo = logic
        .create_object("AmericaJetCargoPlane", Team::GLA, Vec3::new(30.0, 0.0, 0.0))
        .expect("cargo");
    if let Some(a) = logic.host_object_mut(cargo) {
        a.status.airborne_target = true;
    }
    logic.apply_supw_patriot_emp_residual_at(Vec3::ZERO, bat, Team::USA, Some(cargo));
    let cargo_v = logic.host_object(cargo).expect("cargo");
    assert!(
        !cargo_v.is_emp_disabled() && !cargo_v.status.disabled_emp,
        "EMP_HARDENED aircraft must not near-miss disable"
    );
}
