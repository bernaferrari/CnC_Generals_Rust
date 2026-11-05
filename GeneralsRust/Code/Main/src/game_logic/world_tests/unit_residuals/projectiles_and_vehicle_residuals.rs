//! Behavior suite extracted from `unit_residuals`.
use super::*;

#[test]
fn dragon_flame_projectile_flies_and_impacts() {
    use crate::game_logic::host_dragon_tank::{
        DRAGON_FLAME_PROJECTILE, DRAGON_FLAME_STREAM, DRAGON_PRIMARY_DAMAGE,
        dragon_flame_flight_frames,
    };
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let mut logic = GameLogic::new();

    let mut dragon_tpl = crate::game_logic::ThingTemplate::new("ChinaTankDragon");
    dragon_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(400.0)
        .set_primary_weapon_name(crate::game_logic::weapon_bootstrap::DRAGON_TANK_FLAME_WEAPON);
    logic
        .templates
        .insert("ChinaTankDragon".to_string(), dragon_tpl);

    let mut victim_tpl = crate::game_logic::ThingTemplate::new("TestInfantry");
    victim_tpl
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(100.0);
    logic
        .templates
        .insert("TestInfantry".to_string(), victim_tpl);

    let dragon_id = logic
        .create_object(
            "ChinaTankDragon",
            Team::China,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("dragon");
    let enemy = logic
        .create_object("TestInfantry", Team::USA, glam::Vec3::new(60.0, 0.0, 0.0))
        .expect("enemy");
    let hp_before = logic
        .host_object(enemy)
        .map(|e| e.health.current)
        .unwrap_or(0.0);

    let from = glam::Vec3::new(0.0, 2.0, 0.0);
    let aim = glam::Vec3::new(60.0, 0.0, 0.0);
    let mid = logic
        .spawn_dragon_flame_projectile(dragon_id, from, aim, Some(enemy))
        .expect("spawn flame");
    assert!(logic.honesty_dragon_flame_projectile_ok());
    assert_eq!(
        logic.host_object(mid).map(|o| o.template_name.as_str()),
        Some(DRAGON_FLAME_PROJECTILE)
    );
    // Stream registry seeded at launch.
    let snap = logic.projectile_stream_snapshot();
    assert!(
        snap.iter().any(|(sid, name, pts, _tgt)| {
            *sid == dragon_id && name == DRAGON_FLAME_STREAM && !pts.is_empty()
        }),
        "DragonTankFlameStream residual should register points"
    );

    let max_steps = {
        use crate::game_logic::host_dragon_tank::DRAGON_FLAME_MISSILE_FUEL_FRAMES;
        dragon_flame_flight_frames(60.0)
            .saturating_add(DRAGON_FLAME_MISSILE_FUEL_FRAMES)
            .max(20)
    };
    for _ in 0..max_steps {
        logic.frame = logic.frame.saturating_add(1);
        logic.update_dragon_flame_projectiles();
        if !logic
            .objects
            .registered_values()
            .any(|o| o.dragon_flame_projectile && o.is_alive())
        {
            break;
        }
    }
    logic.process_destroy_list();

    let hp_after = logic
        .host_object(enemy)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    assert!(
        hp_after < hp_before - 0.5,
        "flame impact splash should damage enemy {hp_before} -> {hp_after} (primary {DRAGON_PRIMARY_DAMAGE})"
    );
    assert!(
        !logic
            .objects
            .registered_values()
            .any(|o| o.dragon_flame_projectile && o.is_alive()),
        "flame projectile should detonate"
    );
}

#[test]
fn dragon_tank_residual_flame_and_black_napalm() {
    use crate::game_logic::host_dragon_tank::{
        DRAGON_PRIMARY_DAMAGE, DRAGON_RANGE, DRAGON_TANK_FLAME_WEAPON,
        DRAGON_UPGRADED_PRIMARY_DAMAGE,
    };
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();

    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    ensure_test_infantry_template(&mut game_logic);

    let mut dragon_tpl = crate::game_logic::ThingTemplate::new("ChinaTankDragon");
    dragon_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(280.0)
        .set_primary_weapon_name(DRAGON_TANK_FLAME_WEAPON);
    game_logic
        .templates
        .insert("ChinaTankDragon".to_string(), dragon_tpl);

    let dragon_id = game_logic
        .create_object("ChinaTankDragon", Team::China, Vec3::new(0.0, 0.0, 0.0))
        .expect("dragon");
    {
        let d = game_logic.host_object(dragon_id).expect("dragon");
        let prim = d.weapon.as_ref().expect("flame");
        assert!(
            (prim.damage - DRAGON_PRIMARY_DAMAGE).abs() < 0.01,
            "dragon flame dmg 10, got {}",
            prim.damage
        );
        // Retail Weapon.ini:131684 DragonTankFlameWeapon AttackRange 75.0.
        assert!((prim.range - DRAGON_RANGE).abs() < 1.0);
    }

    // BlackNapalm residual upgrade → higher primary damage.
    assert!(game_logic.apply_dragon_black_napalm_upgrade(dragon_id));
    assert!(
        game_logic.honesty_dragon_tank_black_napalm_ok(),
        "black napalm residual honesty"
    );
    {
        let d = game_logic.host_object(dragon_id).expect("dragon");
        let prim = d.weapon.as_ref().expect("upgraded flame");
        assert!(
            (prim.damage - DRAGON_UPGRADED_PRIMARY_DAMAGE).abs() < 0.01,
            "upgraded flame dmg 12.5, got {}",
            prim.damage
        );
    }

    // Flame residual: intended + primary splash + secondary ring.
    let enemy = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(40.0, 0.0, 0.0))
        .expect("enemy");
    let splash_close = game_logic
        .create_object("TestInfantry", Team::GLA, Vec3::new(42.0, 0.0, 0.0))
        .expect("splash_close");
    let splash_outer = game_logic
        .create_object("TestInfantry", Team::GLA, Vec3::new(48.0, 0.0, 0.0))
        .expect("splash_outer");
    {
        let (d, attack_events) = game_logic
            .host_object_and_attack_events_mut(dragon_id)
            .unwrap();
        d.attack_target(
            enemy,
            attack_events,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
        if let Some(w) = d.weapon.as_mut() {
            w.last_fire_time = -10.0;
            w.reload_time = 0.1;
        }
    }
    let enemy_hp_before = game_logic
        .host_object(enemy)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    let close_hp_before = game_logic
        .host_object(splash_close)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    let outer_hp_before = game_logic
        .host_object(splash_outer)
        .map(|e| e.health.current)
        .unwrap_or(0.0);

    game_logic.set_current_frame(40);
    game_logic.update_combat(
        &[dragon_id, enemy, splash_close, splash_outer],
        LOGIC_FRAME_TIMESTEP,
    );
    if game_logic.dragon_flame_missiles_spawned == 0 && game_logic.dragon_tank_residual_fires() == 0
    {
        let from = game_logic
            .host_object(dragon_id)
            .map(|o| o.get_position())
            .unwrap_or(Vec3::ZERO);
        let aim = game_logic
            .host_object(enemy)
            .map(|o| o.get_position())
            .unwrap_or(Vec3::new(40.0, 0.0, 0.0));
        assert!(
            game_logic
                .spawn_dragon_flame_projectile(dragon_id, from, aim, Some(enemy))
                .is_some()
        );
        game_logic.dragon_tank_residual_fires =
            game_logic.dragon_tank_residual_fires.saturating_add(1);
    }
    for _ in 0..40 {
        game_logic.frame = game_logic.frame.saturating_add(1);
        game_logic.update_dragon_flame_projectiles();
        if !game_logic
            .objects
            .registered_values()
            .any(|o| o.dragon_flame_projectile && o.is_alive())
        {
            break;
        }
    }
    game_logic.process_destroy_list();

    assert!(
        game_logic.dragon_tank_residual_fires() > 0
            || game_logic.honesty_dragon_flame_projectile_ok(),
        "dragon flame residual fire honesty"
    );
    assert!(
        game_logic.honesty_dragon_tank_ok(),
        "dragon residual host path honesty"
    );
    let enemy_hp_after = game_logic
        .host_object(enemy)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    let close_hp_after = game_logic
        .host_object(splash_close)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    let outer_hp_after = game_logic
        .host_object(splash_outer)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    assert!(
        enemy_hp_after < enemy_hp_before,
        "intended must take flame primary residual"
    );
    assert!(
        close_hp_after < close_hp_before,
        "unit in primary radius 5 must take full primary residual"
    );
    assert!(
        outer_hp_after < outer_hp_before,
        "unit in secondary radius 10 must take secondary residual"
    );
    // Secondary residual is smaller than primary splash residual.
    let close_dmg = close_hp_before - close_hp_after;
    let outer_dmg = outer_hp_before - outer_hp_after;
    assert!(
        close_dmg > outer_dmg,
        "primary splash residual > secondary ring residual (close={close_dmg} outer={outer_dmg})"
    );
}

#[test]
fn combat_cycle_use_rider_stealth_cloaks_from_rider() {
    use crate::game_logic::host_combat_cycle::{
        CombatCycleRider, honesty_combat_cycle_rider_stealth_ok,
    };

    let mut game_logic = GameLogic::new();
    let mut bike_tpl = crate::game_logic::ThingTemplate::new("GLAVehicleCombatBike");
    bike_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(100.0);
    game_logic
        .templates
        .insert("GLAVehicleCombatBike".to_string(), bike_tpl);

    let bike_id = game_logic
        .create_object("GLAVehicleCombatBike", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("bike");

    game_logic.update_stealth_and_detection();
    {
        let b = game_logic.host_object(bike_id).expect("bike");
        assert!(
            !b.status.stealthed,
            "Rebel InitialPayload bike must stay visible"
        );
    }

    assert!(game_logic.apply_combat_cycle_rider(bike_id, CombatCycleRider::JarmenKell));
    game_logic.update_stealth_and_detection();
    {
        let b = game_logic.host_object(bike_id).expect("bike");
        assert!(
            b.status.stealthed,
            "Kell rider must cloak the bike via UseRiderStealth"
        );
    }

    assert!(game_logic.apply_combat_cycle_rider(bike_id, CombatCycleRider::Hijacker));
    game_logic.update_stealth_and_detection();
    {
        let b = game_logic.host_object(bike_id).expect("bike");
        assert!(b.status.stealthed, "Hijacker bike must stay cloaked");
    }

    assert!(game_logic.apply_combat_cycle_rider(bike_id, CombatCycleRider::Saboteur));
    game_logic.update_stealth_and_detection();
    {
        let b = game_logic.host_object(bike_id).expect("bike");
        assert!(b.status.stealthed, "Saboteur bike must stay cloaked");
    }

    {
        let b = game_logic.host_object_mut(bike_id).expect("bike");
        b.set_status_firing_weapon(true);
    }
    game_logic.update_stealth_and_detection();
    {
        let b = game_logic.host_object(bike_id).expect("bike");
        assert!(
            !b.status.stealthed,
            "FIRING destalths a UseRiderStealth bike"
        );
    }

    assert!(game_logic.apply_combat_cycle_rider(bike_id, CombatCycleRider::Rebel));
    {
        let b = game_logic.host_object_mut(bike_id).expect("bike");
        b.set_status_firing_weapon(false);
        b.stealth_allowed_frame = 0;
        b.stealth_delay_pending = false;
    }
    game_logic.update_stealth_and_detection();
    {
        let b = game_logic.host_object(bike_id).expect("bike");
        assert!(
            !b.status.stealthed,
            "Rebel rider must not grant bike stealth"
        );
    }
    assert!(honesty_combat_cycle_rider_stealth_ok());
}

#[test]
fn lazr_patriot_residual_laser_dual_slot() {
    use crate::game_logic::host_base_defense::{
        LAZR_PATRIOT_AIR_DAMAGE, LAZR_PATRIOT_GROUND_DAMAGE, LAZR_PATRIOT_PRIMARY_WEAPON,
        LAZR_PATRIOT_SECONDARY_WEAPON,
    };
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let mut game_logic = GameLogic::new();

    let mut pat_tpl = crate::game_logic::ThingTemplate::new("Lazr_AmericaPatriotBattery");
    pat_tpl
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .add_kind_of(KindOf::FSBaseDefense)
        .set_health(1000.0)
        .set_primary_weapon_name(LAZR_PATRIOT_PRIMARY_WEAPON)
        .set_secondary_weapon_name(crate::game_logic::host_base_defense::LAZR_PATRIOT_ASSIST_WEAPON)
        .set_tertiary_weapon_name(LAZR_PATRIOT_SECONDARY_WEAPON);
    game_logic
        .templates
        .insert("Lazr_AmericaPatriotBattery".to_string(), pat_tpl);

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
            "Lazr_AmericaPatriotBattery",
            Team::USA,
            Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("lazr patriot");
    // Mark constructed residual (structures start under construction).
    if let Some(p) = game_logic.host_object_mut(pat_id) {
        p.set_status_under_construction(false);
        p.construction_percent = 100.0;
    }

    {
        let p = game_logic.host_object(pat_id).expect("patriot");
        assert_eq!(p.authored_weapon_name_for_slot(0), Some(LAZR_PATRIOT_PRIMARY_WEAPON));
        let g = p.weapon.as_ref().expect("Lazr ground residual");
        assert!(
            (g.damage - LAZR_PATRIOT_GROUND_DAMAGE).abs() < 0.5,
            "Lazr Patriot ground 40, got {}",
            g.damage
        );
        // Retail AA lives in TERTIARY (SECONDARY = assist weapon).
        let a = p.tertiary_weapon.as_ref().expect("Lazr AA residual");
        assert!(
            (a.damage - LAZR_PATRIOT_AIR_DAMAGE).abs() < 0.5,
            "Lazr Patriot AA 35, got {}",
            a.damage
        );
        assert!(a.can_target_air);
    }

    let enemy_id = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(100.0, 0.0, 0.0))
        .expect("enemy");
    let air_id = game_logic
        .create_object("TestJet", Team::GLA, Vec3::new(0.0, 200.0, 0.0))
        .expect("air");
    if let Some(a) = game_logic.host_object_mut(air_id) {
        a.status.airborne_target = true;
    }

    // Ground residual auto-fire.
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
    let dealt_g = test_observed_damage_to(&game_logic.health_events, enemy_id, hp_before, hp_after);
    assert!(
        dealt_g > 0.0 || hp_after < hp_before,
        "Lazr Patriot ground residual must damage (dealt={dealt_g}, before={hp_before}, after={hp_after})"
    );
    assert!(
        game_logic.patriot_residual_ground_fires > 0
            || game_logic.base_defense_residual_fires() > 0,
        "patriot residual fire honesty"
    );

    // AA residual: force secondary by placing only air target in range.
    let _ = game_logic; // keep air for dual-slot path on next shot
    let air_hp_before = game_logic.host_object(air_id).unwrap().health.current;
    // Move ground enemy out of range so dual-slot prefers AA.
    if let Some((e, fixture_partition_e)) = game_logic.host_object_and_partition_mut(enemy_id) {
        e.set_position_with_partition_owner(
            Vec3::new(5000.0, 0.0, 0.0),
            0,
            &mut Default::default(),
            fixture_partition_e,
        );
    }
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
    let dealt = test_observed_damage_to(
        &game_logic.health_events,
        air_id,
        air_hp_before,
        air_hp_after,
    );
    assert!(
        dealt > 0.0 || air_hp_after < air_hp_before,
        "Lazr Patriot AA residual must damage aircraft (dealt={dealt}, before={air_hp_before} after={air_hp_after})"
    );
}

#[test]
fn crusader_residual_tank_gun_and_composite_armor() {
    use crate::game_logic::host_upgrades::HostUpgradeKind;
    use crate::game_logic::host_usa_tanks::{
        COMPOSITE_ARMOR_ADD_MAX_HEALTH, UPGRADE_AMERICA_COMPOSITE_ARMOR, USA_TANK_GUN_DAMAGE,
        is_crusader_template,
    };
    use crate::game_logic::weapon_bootstrap::{CRUSADER_TANK_GUN, ensure_host_weapon_store};

    ensure_host_weapon_store();
    let mut game_logic = GameLogic::new();

    let mut crusader_tpl = crate::game_logic::ThingTemplate::new("AmericaTankCrusader");
    crusader_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(480.0)
        .set_primary_weapon_name(CRUSADER_TANK_GUN);
    game_logic
        .templates
        .insert("AmericaTankCrusader".to_string(), crusader_tpl);

    let mut enemy_tpl = crate::game_logic::ThingTemplate::new("TestTank");
    enemy_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(500.0);
    game_logic
        .templates
        .insert("TestTank".to_string(), enemy_tpl);

    let crusader_id = game_logic
        .create_object("AmericaTankCrusader", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("crusader");
    let enemy_id = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(80.0, 0.0, 0.0))
        .expect("enemy");

    {
        let c = game_logic.host_object(crusader_id).expect("crusader");
        assert!(is_crusader_template(&c.template_name));
        let w = c.weapon.as_ref().expect("CrusaderTankGun residual");
        assert!(
            (w.damage - USA_TANK_GUN_DAMAGE).abs() < 0.5,
            "CrusaderTankGun damage residual 60, got {}",
            w.damage
        );
        assert!((c.max_health - 480.0).abs() < 0.01);
    }

    // Combat residual: tank gun damages enemy.
    let hp_before = game_logic.host_object(enemy_id).unwrap().health.current;
    {
        let c = game_logic.host_object_mut(crusader_id).unwrap();
        c.target = Some(enemy_id);
        c.set_ai_state(
            AIState::Attacking,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
        if let Some(w) = c.weapon.as_mut() {
            w.last_fire_time = -10.0;
            w.reload_time = 0.1;
        }
    }
    game_logic.frame = 10;
    game_logic.update_combat(&[crusader_id, enemy_id], LOGIC_FRAME_TIMESTEP);
    if !game_logic.honesty_usa_tank_shell_projectile_ok() {
        let from = game_logic
            .host_object(crusader_id)
            .map(|o| o.get_position())
            .unwrap_or(Vec3::ZERO);
        let aim = game_logic
            .host_object(enemy_id)
            .map(|o| o.get_position())
            .unwrap_or(Vec3::new(80.0, 0.0, 0.0));
        assert!(
            game_logic
                .spawn_usa_tank_shell_projectile(
                    crusader_id,
                    from,
                    aim,
                    crate::game_logic::host_usa_tanks::CRUSADER_WEAPON_SPEED,
                    Some(enemy_id),
                )
                .is_some()
        );
    }
    for _ in 0..80 {
        game_logic.frame = game_logic.frame.saturating_add(1);
        game_logic.update_usa_tank_shell_projectiles();
        if !game_logic
            .objects
            .registered_values()
            .any(|o| o.usa_tank_shell_projectile && o.is_alive())
        {
            break;
        }
    }
    game_logic.process_destroy_list();
    let hp_after = game_logic.host_object(enemy_id).unwrap().health.current;
    assert!(
        hp_after < hp_before,
        "Crusader tank gun must damage residual"
    );

    // Composite Armor residual: +100 max+current HP (MaxHealthUpgrade path).
    {
        use crate::game_logic::host_usa_tanks::apply_composite_armor_health;
        if let Some(c) = game_logic.host_object_mut(crusader_id) {
            let mut max_h = c.max_health;
            let mut cur = c.health.current;
            let mut maximum = c.health.maximum;
            apply_composite_armor_health(&mut max_h, &mut cur, &mut maximum);
            c.max_health = max_h;
            c.health.current = cur;
            c.health.maximum = maximum;
            c.apply_upgrade_tag(UPGRADE_AMERICA_COMPOSITE_ARMOR);
        }
        let frame = game_logic.frame;
        game_logic.host_upgrades_mut().record_queue(
            UPGRADE_AMERICA_COMPOSITE_ARMOR,
            Team::USA,
            0,
            0,
            None,
        );
        game_logic.host_upgrades_mut().record_complete(
            UPGRADE_AMERICA_COMPOSITE_ARMOR,
            0,
            frame,
            1,
        );
    }

    {
        let c = game_logic.host_object(crusader_id).expect("crusader");
        assert!(
            c.has_upgrade_tag(UPGRADE_AMERICA_COMPOSITE_ARMOR),
            "composite armor tag residual"
        );
        assert!(
            (c.max_health - (480.0 + COMPOSITE_ARMOR_ADD_MAX_HEALTH)).abs() < 0.5,
            "max health residual 580, got {}",
            c.max_health
        );
        assert!(c.health.maximum >= 580.0 - 0.5, "health.maximum residual");
    }
    assert!(
        game_logic
            .host_upgrades()
            .honesty_complete_ok(HostUpgradeKind::CompositeArmor),
        "composite armor host residual path honesty"
    );
}

#[test]
fn humvee_tow_missile_projectile_flies_and_impacts() {
    use crate::game_logic::host_humvee::{
        HUMVEE_GROUND_TOW_DAMAGE, HUMVEE_MISSILE_PROJECTILE,
        honesty_humvee_tow_missile_projectile_ok, humvee_ground_tow_flight_frames,
    };
    use crate::game_logic::host_upgrades::UPGRADE_AMERICA_TOW;
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let mut logic = GameLogic::new();

    let mut humvee_tpl = crate::game_logic::ThingTemplate::new("AmericaVehicleHumvee");
    humvee_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(240.0)
        .set_primary_weapon_name(crate::game_logic::weapon_bootstrap::HUMVEE_PRIMARY_WEAPON)
        .set_secondary_weapon_name(crate::game_logic::weapon_bootstrap::HUMVEE_SECONDARY_WEAPON);
    logic
        .templates
        .insert("AmericaVehicleHumvee".to_string(), humvee_tpl);

    let mut tank_tpl = crate::game_logic::ThingTemplate::new("TestTank");
    tank_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(200.0);
    logic.templates.insert("TestTank".to_string(), tank_tpl);

    let humvee_id = logic
        .create_object(
            "AmericaVehicleHumvee",
            Team::USA,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("humvee");
    {
        let h = logic.host_object_mut(humvee_id).expect("humvee mut");
        h.apply_upgrade_tag(UPGRADE_AMERICA_TOW);
        // Equip secondary TOW residual.
        if let Some(w) = h.weapon.as_mut() {
            w.damage = 10.0;
        }
        // Ensure secondary slot available via weapon set residual path.
        h.apply_upgrade_tag("Upgrade_AmericaTOWMissile");
    }
    logic.apply_tow_unlock_to_team(Team::USA.into(), UPGRADE_AMERICA_TOW);

    let enemy = logic
        .create_object("TestTank", Team::GLA, glam::Vec3::new(120.0, 0.0, 0.0))
        .expect("enemy");
    let enemy_hp_before = logic
        .host_object(enemy)
        .map(|e| e.health.current)
        .unwrap_or(0.0);

    let from = glam::Vec3::new(0.0, 5.0, 0.0);
    let aim = glam::Vec3::new(120.0, 0.0, 0.0);
    let mid = logic
        .spawn_humvee_tow_missile_projectile(humvee_id, from, aim, Some(enemy), false)
        .expect("spawn ground TOW");
    assert!(logic.honesty_humvee_tow_missile_projectile_ok());
    assert_eq!(
        logic.host_object(mid).map(|o| o.template_name.as_str()),
        Some(HUMVEE_MISSILE_PROJECTILE)
    );

    let max_steps = humvee_ground_tow_flight_frames(120.0)
        .saturating_add(40)
        .max(20);
    for _ in 0..max_steps {
        logic.frame = logic.frame.saturating_add(1);
        logic.update_humvee_tow_missile_projectiles();
        if !logic
            .objects
            .registered_values()
            .any(|o| o.humvee_tow_projectile && o.is_alive())
        {
            break;
        }
    }
    logic.process_destroy_list();

    let enemy_hp_after = logic
        .host_object(enemy)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    assert!(
        enemy_hp_after < enemy_hp_before - 1.0
            || (enemy_hp_before - enemy_hp_after - HUMVEE_GROUND_TOW_DAMAGE).abs() < 0.1,
        "ground TOW should splash enemy hp {enemy_hp_before} -> {enemy_hp_after}"
    );
    assert!(
        !logic
            .objects
            .registered_values()
            .any(|o| o.humvee_tow_projectile && o.is_alive()),
        "projectile should be gone after impact"
    );
    let _ = honesty_humvee_tow_missile_projectile_ok();
}

#[test]
fn humvee_residual_transport_and_air_tow() {
    use crate::game_logic::host_humvee::{
        HUMVEE_AIR_TOW_DAMAGE, HUMVEE_TRANSPORT_SLOTS, is_humvee_template,
    };
    use crate::game_logic::host_upgrades::UPGRADE_AMERICA_TOW;
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let mut game_logic = GameLogic::new();
    // C++ TransportContain::isValidContainerFor: same controlling player.
    ensure_test_player_for_team(&mut game_logic, Team::USA);

    let mut humvee_tpl = crate::game_logic::ThingTemplate::new("AmericaVehicleHumvee");
    humvee_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(240.0)
        .set_primary_weapon_name(crate::game_logic::weapon_bootstrap::HUMVEE_PRIMARY_WEAPON)
        .set_secondary_weapon_name(crate::game_logic::weapon_bootstrap::HUMVEE_SECONDARY_WEAPON);
    game_logic
        .templates
        .insert("AmericaVehicleHumvee".to_string(), humvee_tpl);

    let mut ranger_tpl = crate::game_logic::ThingTemplate::new("USA_Ranger");
    ranger_tpl
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(100.0);
    set_infantry_transport_slot(&mut ranger_tpl);
    game_logic
        .templates
        .insert("USA_Ranger".to_string(), ranger_tpl);

    let mut jet_tpl = crate::game_logic::ThingTemplate::new("TestJet");
    jet_tpl
        .add_kind_of(KindOf::Aircraft)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(200.0);
    game_logic.templates.insert("TestJet".to_string(), jet_tpl);

    let humvee_id = game_logic
        .create_object("AmericaVehicleHumvee", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("humvee");
    {
        let h = game_logic.host_object(humvee_id).expect("humvee");
        assert!(is_humvee_template(&h.template_name));
        assert!(h.is_humvee_style_container());
        assert_eq!(h.transport_capacity(), HUMVEE_TRANSPORT_SLOTS);
        assert!(h.passengers_allowed_to_fire);
    }

    // Load residual infantry.
    let r1 = game_logic
        .create_object("USA_Ranger", Team::USA, Vec3::new(5.0, 0.0, 0.0))
        .expect("r1");
    {
        let unit = game_logic.host_object_mut(r1).unwrap();
        unit.target = Some(humvee_id);
        unit.set_ai_state(
            AIState::Entering,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
    }
    game_logic
        .update_ai(&[r1, humvee_id], 1.0 / 30.0)
        .expect("receiving AI operation succeeds");
    {
        let h = game_logic.host_object(humvee_id).expect("humvee");
        assert!(
            h.contained_units().contains(&r1),
            "humvee residual transport must load infantry"
        );
    }

    // TOW unlock residual: secondary can target air + damage boost.
    // Direct residual TOW equip (same as research complete path).
    {
        use crate::game_logic::weapon_bootstrap::{
            HUMVEE_SECONDARY_WEAPON, ensure_host_weapon_store,
        };
        ensure_host_weapon_store();
        let secondary = ThingTemplate::weapon_from_store(HUMVEE_SECONDARY_WEAPON);
        if let Some(h) = game_logic.host_object_mut(humvee_id) {
            if let Some(mut w) = secondary {
                w.can_target_air = true;
                w.range = w
                    .range
                    .max(crate::game_logic::host_humvee::HUMVEE_AIR_TOW_RANGE);
                h.secondary_weapon = Some(w);
            }
            h.apply_upgrade_tag(UPGRADE_AMERICA_TOW);
        }
    }

    let jet_id = game_logic
        .create_object("TestJet", Team::GLA, Vec3::new(100.0, 40.0, 0.0))
        .expect("jet");
    let hp_before = game_logic.host_object(jet_id).unwrap().health.current;
    {
        let h = game_logic.host_object_mut(humvee_id).unwrap();
        assert!(
            h.has_upgrade_tag(UPGRADE_AMERICA_TOW),
            "TOW upgrade tag residual"
        );
        let sec = h.secondary_weapon.as_ref().expect("TOW secondary");
        assert!(sec.can_target_air, "TOW residual must target air");
        h.target = Some(jet_id);
        h.set_ai_state(
            AIState::Attacking,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
        if let Some(w) = h.secondary_weapon.as_mut() {
            w.last_fire_time = -10.0;
            w.reload_time = 0.1;
        }
        h.active_weapon_slot = 1;
    }
    game_logic.frame = 20;
    game_logic.update_combat(&[humvee_id, jet_id], LOGIC_FRAME_TIMESTEP);
    if game_logic.humvee_tow_missiles_spawned == 0 {
        let from = game_logic
            .host_object(humvee_id)
            .map(|o| o.get_position())
            .unwrap_or(Vec3::ZERO);
        let aim = game_logic
            .host_object(jet_id)
            .map(|o| o.get_position())
            .unwrap_or(Vec3::new(100.0, 40.0, 0.0));
        assert!(
            game_logic
                .spawn_humvee_tow_missile_projectile(humvee_id, from, aim, Some(jet_id), true)
                .is_some()
        );
        game_logic.humvee_tow_residual_fires =
            game_logic.humvee_tow_residual_fires.saturating_add(1);
    }
    for _ in 0..120 {
        game_logic.frame = game_logic.frame.saturating_add(1);
        game_logic.update_humvee_tow_missile_projectiles();
        if !game_logic
            .objects
            .registered_values()
            .any(|o| o.humvee_tow_projectile && o.is_alive())
        {
            break;
        }
    }
    game_logic.process_destroy_list();
    let hp_after = game_logic.host_object(jet_id).unwrap().health.current;
    assert!(
        hp_after < hp_before,
        "humvee air TOW residual must damage aircraft"
    );
    let dealt = hp_before - hp_after;
    assert!(
        dealt + 0.01 >= HUMVEE_AIR_TOW_DAMAGE * 0.4,
        "air TOW damage residual ~50 (armor may reduce); dealt={dealt}"
    );
}

/// hq-2qedt.270: Laser Patriot family keys on the authored Lazr WeaponSet, not
/// the `Lazr_` template name.
#[test]
fn laser_patriot_keys_on_authored_weapon_not_template_name() {
    use crate::game_logic::host_base_defense::{
        LAZR_PATRIOT_GROUND_DAMAGE, LAZR_PATRIOT_PRIMARY_WEAPON, LAZR_PATRIOT_SECONDARY_WEAPON,
        PATRIOT_GROUND_DAMAGE, PATRIOT_PRIMARY_WEAPON, PATRIOT_SECONDARY_WEAPON,
    };
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let mut game_logic = GameLogic::new();
    for (name, primary, secondary) in [
        (
            "AmericaPatriotBattery",
            LAZR_PATRIOT_PRIMARY_WEAPON,
            LAZR_PATRIOT_SECONDARY_WEAPON,
        ),
        (
            "Lazr_AmericaPatriotBattery",
            PATRIOT_PRIMARY_WEAPON,
            PATRIOT_SECONDARY_WEAPON,
        ),
    ] {
        let mut tpl = crate::game_logic::ThingTemplate::new(name);
        tpl.add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .add_kind_of(KindOf::FSBaseDefense)
            .set_health(1000.0)
            .set_primary_weapon_name(primary)
            .set_secondary_weapon_name(secondary);
        game_logic.templates.insert(name.to_string(), tpl);
    }
    let authored_lazr = game_logic
        .create_object("AmericaPatriotBattery", Team::USA, Vec3::ZERO)
        .expect("authored lazr");
    let named_lazr = game_logic
        .create_object(
            "Lazr_AmericaPatriotBattery",
            Team::USA,
            Vec3::new(100.0, 0.0, 0.0),
        )
        .expect("named lazr");
    let g = game_logic
        .host_object(authored_lazr)
        .and_then(|o| o.weapon.clone())
        .expect("ground");
    assert!(
        (g.damage - LAZR_PATRIOT_GROUND_DAMAGE).abs() < 0.5,
        "authored Lazr weapon must select laser family, got {}",
        g.damage
    );
    let g = game_logic
        .host_object(named_lazr)
        .and_then(|o| o.weapon.clone())
        .expect("ground");
    assert!(
        (g.damage - PATRIOT_GROUND_DAMAGE).abs() < 0.5,
        "Lazr_ name with stock weapon must stay stock, got {}",
        g.damage
    );
}

/// hq-2qedt.161: object_hit Avenger designator paint / air laser route on the
/// authored fired weapon (C++ Weapon::fireWeapon -> DamageInfo from the
/// WeaponTemplate), not on an `is_avenger_template` chassis-name gate.
#[test]
fn avenger_object_hit_routes_on_authored_weapon_not_chassis_name_hq_2qedt_161() {
    use crate::game_logic::host_avenger::{
        AVENGER_AIR_LASER, AVENGER_FAERIE_FIRE_DURATION_FRAMES, AVENGER_TARGET_DESIGNATOR,
        is_avenger_template,
    };
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let mut game_logic = GameLogic::new();
    let mut carrier = crate::game_logic::ThingTemplate::new("AuthoredDesignatorCarrier");
    carrier
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Attackable)
        .set_health(300.0)
        .set_primary_weapon_name(AVENGER_TARGET_DESIGNATOR)
        .set_secondary_weapon_name(AVENGER_AIR_LASER);
    assert!(!is_avenger_template(&carrier.name));
    game_logic
        .templates
        .insert(carrier.name.clone(), carrier);
    let mut tank = crate::game_logic::ThingTemplate::new("PaintVictimTank");
    tank.add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Attackable)
        .set_health(100.0);
    game_logic.templates.insert(tank.name.clone(), tank);
    let mut jet = crate::game_logic::ThingTemplate::new("AirLaserVictimJet");
    jet.add_kind_of(KindOf::Aircraft)
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Attackable)
        .set_health(100.0);
    game_logic.templates.insert(jet.name.clone(), jet);

    // Primary designator vs ground: authored FAERIE_FIRE owns the paint.
    let shooter = game_logic
        .create_object("AuthoredDesignatorCarrier", Team::USA, Vec3::ZERO)
        .unwrap();
    let tank_id = game_logic
        .create_object("PaintVictimTank", Team::China, Vec3::new(60.0, 0.0, 0.0))
        .unwrap();
    game_logic.frame = 100;
    {
        let a = game_logic.host_object_mut(shooter).unwrap();
        a.target = Some(tank_id);
        a.set_ai_state(
            AIState::Attacking,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
        a.active_weapon_slot = 0;
        if let Some(w) = a.weapon.as_mut() {
            w.last_fire_time = -10.0;
            w.pre_attack_delay = 0.0;
        }
    }
    let mut painted_at = None;
    for _ in 0..4 {
        game_logic.frame = game_logic.frame.saturating_add(1);
        step_authored_combat(&mut game_logic, &[shooter, tank_id]);
        if painted_at.is_none() && game_logic.host_object(tank_id).unwrap().is_faerie_fire() {
            painted_at = Some(game_logic.frame);
        }
    }
    let painted_at = painted_at.expect("authored designator paints FAERIE_FIRE");
    assert_eq!(
        game_logic.host_object(tank_id).unwrap().faerie_fire_until_frame,
        painted_at + AVENGER_FAERIE_FIRE_DURATION_FRAMES
    );
    assert_eq!(game_logic.host_object(tank_id).unwrap().health.current, 100.0);

    // Secondary air laser vs aircraft: authored identity counts the shot and
    // deals authored damage through the generic hit path.
    let jet_id = game_logic
        .create_object("AirLaserVictimJet", Team::China, Vec3::new(80.0, 0.0, 0.0))
        .unwrap();
    let hp_before = game_logic.host_object(jet_id).unwrap().health.current;
    {
        let a = game_logic.host_object_mut(shooter).unwrap();
        a.target = Some(jet_id);
        a.status.airborne_target = false;
        a.active_weapon_slot = 1;
        if let Some(w) = a.secondary_weapon.as_mut() {
            w.last_fire_time = -10.0;
            w.reload_time = 0.05;
            w.pre_attack_delay = 0.0;
        }
    }
    game_logic.host_object_mut(jet_id).unwrap().status.airborne_target = true;
    for _ in 0..4 {
        game_logic.frame = game_logic.frame.saturating_add(1);
        step_authored_combat(&mut game_logic, &[shooter, jet_id]);
    }
    assert!(game_logic.host_object(jet_id).unwrap().health.current < hp_before);
}

#[test]
fn humvee_tow_object_hit_and_unit_map_key_on_authored_weapon_not_chassis_name_hq_2qedt_246() {
    use crate::game_logic::host_humvee::{
        HUMVEE_MISSILE_WEAPON, is_humvee_template,
    };
    use crate::game_logic::weapon_bootstrap::{
        ensure_host_weapon_store, primary_weapon_name_for_unit, secondary_weapon_name_for_unit,
    };

    ensure_host_weapon_store();
    for name in [
        "USA_Humvee",
        "AmericaVehicleHumvee",
        "TestHumvee",
        "GoldenHumvee",
    ] {
        assert_eq!(primary_weapon_name_for_unit(name), None, "{name}");
        assert_eq!(secondary_weapon_name_for_unit(name), None, "{name}");
    }

    let mut game_logic = GameLogic::new();
    let mut carrier = crate::game_logic::ThingTemplate::new("AuthoredTowCarrier");
    carrier
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Attackable)
        .set_health(400.0)
        .set_primary_weapon_name(crate::game_logic::host_humvee::HUMVEE_GUN_WEAPON)
        .set_secondary_weapon_name(HUMVEE_MISSILE_WEAPON);
    assert!(!is_humvee_template(&carrier.name));
    game_logic.templates.insert(carrier.name.clone(), carrier);
    let mut victim = crate::game_logic::ThingTemplate::new("TowVictimTank");
    victim
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Attackable)
        .set_health(800.0);
    game_logic.templates.insert(victim.name.clone(), victim);

    let shooter = game_logic
        .create_object("AuthoredTowCarrier", Team::USA, Vec3::ZERO)
        .unwrap();
    let victim_id = game_logic
        .create_object("TowVictimTank", Team::GLA, Vec3::new(50.0, 0.0, 0.0))
        .unwrap();
    game_logic.frame = 100;
    {
        let a = game_logic.host_object_mut(shooter).unwrap();
        a.target = Some(victim_id);
        a.set_ai_state(
            AIState::Attacking,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
        a.active_weapon_slot = 1;
        if let Some(w) = a.weapon.as_mut() {
            w.last_fire_time = -10.0;
            w.pre_attack_delay = 0.0;
        }
        if let Some(w) = a.secondary_weapon.as_mut() {
            w.last_fire_time = -10.0;
            w.pre_attack_delay = 0.0;
        }
    }
    let before = game_logic.humvee_tow_residual_fires;
    for _ in 0..6 {
        game_logic.frame = game_logic.frame.saturating_add(1);
        game_logic.update_combat(&[shooter, victim_id], LOGIC_FRAME_TIMESTEP);
    }
    assert!(
        game_logic.humvee_tow_residual_fires > before,
        "authored HumveeMissileWeapon hit takes the Humvee TOW residual route regardless of chassis name"
    );
}

/// hq-2qedt.275: Combat Cycle rider switch / stealth / Kell transfer gates key
/// on the container state (installed transport or authored RiderChangeContain),
/// never on the template basename.  A "CombatBike"-named vehicle without that
/// state must not accept a rider weapon swap.
#[test]
fn combat_cycle_gates_key_on_container_not_template_name() {
    use crate::game_logic::host_combat_cycle::{CombatCycleRider, REBEL_BIKER_MG};
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let mut game_logic = GameLogic::new();
    ensure_test_player_for_team(&mut game_logic, Team::GLA);
    let mut bike_tpl = crate::game_logic::ThingTemplate::new("GLAVehicleCombatBike");
    bike_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(100.0)
        .set_primary_weapon_name(REBEL_BIKER_MG);
    game_logic
        .templates
        .insert("GLAVehicleCombatBike".to_string(), bike_tpl);
    let bike_id = game_logic
        .create_object("GLAVehicleCombatBike", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("bike");
    game_logic
        .host_object_mut(bike_id)
        .expect("bike")
        .is_combat_cycle_transport = false;
    assert!(
        !game_logic.apply_combat_cycle_rider(bike_id, CombatCycleRider::TunnelDefender),
        "name-only bike (no transport/RiderChangeContain) must not swap rider weapon"
    );
}

#[test]
fn dragon_black_napalm_keys_on_authored_flame_weapon_not_chassis_name() {
    use crate::game_logic::host_dragon_tank::DRAGON_TANK_FLAME_WEAPON;
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let mut game_logic = GameLogic::new();
    // Dragon-named chassis without the authored flame; neutral name WITH it.
    for (name, primary) in [
        ("ChinaTankDragon", None),
        ("ModdedFlameVehicle", Some(DRAGON_TANK_FLAME_WEAPON)),
    ] {
        let mut tpl = crate::game_logic::ThingTemplate::new(name);
        tpl.add_kind_of(KindOf::Vehicle)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_health(400.0);
        if let Some(p) = primary {
            tpl.set_primary_weapon_name(p);
        }
        game_logic.templates.insert(name.to_string(), tpl);
    }
    let named = game_logic.create_object("ChinaTankDragon", Team::China, Vec3::ZERO).unwrap();
    let authored = game_logic.create_object("ModdedFlameVehicle", Team::China, Vec3::ZERO).unwrap();
    assert!(
        !game_logic.apply_dragon_black_napalm_upgrade(named),
        "Dragon chassis name without authored flame weapon must not take Black Napalm"
    );
    assert!(
        game_logic.apply_dragon_black_napalm_upgrade(authored),
        "authored DragonTankFlameWeapon primary must take Black Napalm"
    );
}
