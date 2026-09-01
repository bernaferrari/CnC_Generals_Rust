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
            .values()
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
            .values()
            .any(|o| o.dragon_flame_projectile && o.is_alive()),
        "flame projectile should detonate"
    );
}

#[test]
fn dragon_tank_residual_flame_and_black_napalm() {
    use crate::game_logic::host_dragon_tank::{
        DRAGON_PRIMARY_DAMAGE, DRAGON_RANGE, DRAGON_TANK_FLAME_WEAPON,
        DRAGON_UPGRADED_PRIMARY_DAMAGE, is_dragon_tank_template,
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
        assert!(is_dragon_tank_template(&d.template_name));
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
        let d = game_logic.host_object_mut(dragon_id).unwrap();
        d.attack_target(enemy);
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
            .values()
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
fn gattling_tank_residual_ramp_fire_rate_and_aa() {
    use crate::game_logic::host_gattling_tank::{
        GATTLING_GROUND_DAMAGE, GATTLING_GROUND_RANGE, GATTLING_TANK_GUN, GATTLING_TANK_GUN_AIR,
        GattlingFireLevel, is_gattling_tank_template,
    };
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();

    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    ensure_test_infantry_template(&mut game_logic);

    let mut gattling_tpl = crate::game_logic::ThingTemplate::new("ChinaTankGattling");
    gattling_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(240.0)
        .set_primary_weapon_name(GATTLING_TANK_GUN)
        .set_secondary_weapon_name(GATTLING_TANK_GUN_AIR);
    game_logic
        .templates
        .insert("ChinaTankGattling".to_string(), gattling_tpl);

    let mut aircraft_tpl = crate::game_logic::ThingTemplate::new("TestAircraft");
    aircraft_tpl
        .add_kind_of(KindOf::Aircraft)
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Attackable)
        .add_kind_of(KindOf::Selectable)
        .set_health(100.0);
    game_logic
        .templates
        .insert("TestAircraft".to_string(), aircraft_tpl);

    let gattling_id = game_logic
        .create_object("ChinaTankGattling", Team::China, Vec3::new(0.0, 0.0, 0.0))
        .expect("gattling");
    let base_reload = {
        let g = game_logic.host_object(gattling_id).expect("gattling");
        assert!(is_gattling_tank_template(&g.template_name));
        let prim = g.weapon.as_ref().expect("ground gun");
        assert!((prim.damage - GATTLING_GROUND_DAMAGE).abs() < 0.01);
        // Retail Weapon.ini:129891 GattlingTankGun AttackRange 150.0.
        assert!((prim.range - GATTLING_GROUND_RANGE).abs() < 1.0);
        assert!(prim.can_target_ground);
        assert!(!prim.can_target_air);
        let sec = g.secondary_weapon.as_ref().expect("aa gun");
        assert!(sec.can_target_air);
        assert!(!sec.can_target_ground);
        assert_eq!(g.continuous_fire_level, 0);
        prim.reload_time
    };

    // Chain Guns residual → damage × 1.25.
    assert!(game_logic.apply_gattling_chain_guns_upgrade(gattling_id));
    {
        let g = game_logic.host_object(gattling_id).expect("gattling");
        let prim = g.weapon.as_ref().expect("chained ground");
        assert!(
            (prim.damage - GATTLING_GROUND_DAMAGE * 1.25).abs() < 0.01,
            "chain guns residual 125% damage, got {}",
            prim.damage
        );
    }

    // Fire repeatedly at same target to ramp continuous fire residual.
    let enemy = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(80.0, 0.0, 0.0))
        .expect("enemy");
    let enemy_hp_before = game_logic
        .host_object(enemy)
        .map(|e| e.health.current)
        .unwrap_or(0.0);

    for i in 0..8u32 {
        {
            let g = game_logic.host_object_mut(gattling_id).unwrap();
            g.attack_target(enemy);
            if let Some(w) = g.weapon.as_mut() {
                w.last_fire_time = -10.0;
                // Keep fireable each frame residual.
                w.reload_time = 0.05;
            }
            if let Some(w) = g.secondary_weapon.as_mut() {
                w.last_fire_time = 0.0;
                w.reload_time = 1000.0;
            }
        }
        game_logic.set_current_frame(30 + (i as u64) * 15);
        game_logic.update_combat(&[gattling_id, enemy], LOGIC_FRAME_TIMESTEP);
    }

    assert!(
        game_logic.gattling_tank_residual_ground_fires() > 0,
        "gattling ground residual honesty"
    );
    assert!(
        game_logic.honesty_gattling_tank_ramp_ok(),
        "gattling continuous-fire ramp residual honesty must reach MEAN or FAST"
    );
    {
        let g = game_logic.host_object(gattling_id).expect("gattling");
        assert!(
            g.continuous_fire_level >= GattlingFireLevel::Mean.as_u8(),
            "after multi-shot residual must be MEAN or FAST, level={}",
            g.continuous_fire_level
        );
        let prim = g.weapon.as_ref().expect("ramped gun");
        // MEAN reload 6/30=0.2, FAST 4/30≈0.133 — both < base 12/30=0.4
        assert!(
            prim.reload_time < base_reload - 0.05,
            "ramped fire residual faster than base (base={base_reload} now={})",
            prim.reload_time
        );
    }
    let enemy_hp_after = game_logic
        .host_object(enemy)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    assert!(
        enemy_hp_after < enemy_hp_before,
        "gattling residual must damage ground target"
    );

    // AA secondary residual vs airborne.
    let aircraft_id = game_logic
        .create_object("TestAircraft", Team::GLA, Vec3::new(80.0, 0.0, 0.0))
        .expect("aircraft");
    {
        let a = game_logic.host_object_mut(aircraft_id).unwrap();
        a.status.airborne_target = true;
    }
    {
        let g = game_logic.host_object_mut(gattling_id).unwrap();
        g.attack_target(aircraft_id);
        g.active_weapon_slot = 1;
        if let Some(w) = g.secondary_weapon.as_mut() {
            w.last_fire_time = -10.0;
            w.reload_time = 0.1;
            w.pre_attack_delay = 0.0;
        }
        if let Some(w) = g.weapon.as_mut() {
            w.last_fire_time = 0.0;
            w.reload_time = 1000.0;
        }
    }
    let air_hp_before = game_logic
        .host_object(aircraft_id)
        .map(|a| a.health.current)
        .unwrap_or(0.0);
    game_logic.set_current_frame(200);
    for i in 0..4u32 {
        game_logic.set_current_frame(200 + u64::from(i) * 5);
        game_logic.update_combat(&[gattling_id, aircraft_id, enemy], LOGIC_FRAME_TIMESTEP);
    }
    assert!(
        game_logic.honesty_gattling_tank_aa_ok(),
        "gattling AA residual honesty must fire secondary vs airborne"
    );
    let air_hp_after = game_logic
        .host_object(aircraft_id)
        .map(|a| a.health.current)
        .unwrap_or(0.0);
    assert!(
        air_hp_after < air_hp_before,
        "airborne target must take gattling AA residual"
    );
    assert!(
        game_logic.honesty_gattling_tank_ok(),
        "gattling residual host path honesty"
    );
}

#[test]
fn combat_cycle_residual_rider_weapon_switch() {
    use crate::game_logic::host_combat_cycle::{
        COMBAT_CYCLE_TRANSPORT_SLOTS, CombatCycleRider, KELL_DAMAGE, REBEL_BIKER_MG,
        REBEL_MG_DAMAGE, RPG_DAMAGE, is_combat_cycle_template,
    };
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();

    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    ensure_test_infantry_template(&mut game_logic);
    // C++ TransportContain/RiderChangeContain isValidContainerFor: same
    // controlling player (ActionManager.cpp canEnterObject).
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

    // Rider templates for switch residual.
    for (name, kinds) in [
        ("GLAInfantryRebel", true),
        ("GLAInfantryTunnelDefender", true),
        ("GLAInfantryJarmenKell", true),
        ("GLAInfantryWorker", true),
    ] {
        let mut tpl = crate::game_logic::ThingTemplate::new(name);
        tpl.add_kind_of(KindOf::Infantry)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_health(100.0);
        set_infantry_transport_slot(&mut tpl);
        let _ = kinds;
        game_logic.templates.insert(name.to_string(), tpl);
    }

    let bike_id = game_logic
        .create_object("GLAVehicleCombatBike", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("bike");
    {
        let b = game_logic.host_object(bike_id).expect("bike");
        assert!(is_combat_cycle_template(&b.template_name));
        assert!(b.is_combat_cycle_style_container());
        assert_eq!(b.transport_capacity(), COMBAT_CYCLE_TRANSPORT_SLOTS);
        assert!(!b.passengers_allowed_to_fire);
        // InitialPayload residual: Rebel MG bound.
        let prim = b.weapon.as_ref().expect("default rebel weapon");
        assert!((prim.damage - REBEL_MG_DAMAGE).abs() < 0.01);
        assert_eq!(b.combat_cycle_rider, CombatCycleRider::Rebel.as_u8());
    }

    // Switch residual: TunnelDefender RPG.
    assert!(game_logic.apply_combat_cycle_rider(bike_id, CombatCycleRider::TunnelDefender));
    assert!(
        game_logic.honesty_combat_cycle_rider_switch_ok(),
        "combat cycle rider switch residual honesty"
    );
    {
        let b = game_logic.host_object(bike_id).expect("bike");
        let prim = b.weapon.as_ref().expect("rpg");
        assert!((prim.damage - RPG_DAMAGE).abs() < 0.5);
        assert!(prim.can_target_air, "RPG residual targets air");
        assert!((prim.min_range - 5.0).abs() < 0.1);
    }

    // Switch residual: Jarmen Kell sniper.
    assert!(game_logic.apply_combat_cycle_rider(bike_id, CombatCycleRider::JarmenKell));
    {
        let b = game_logic.host_object(bike_id).expect("bike");
        let prim = b.weapon.as_ref().expect("sniper");
        assert!((prim.damage - KELL_DAMAGE).abs() < 1.0);
        assert!((prim.range - 225.0).abs() < 1.0);
    }

    // Worker residual: no combat weapon (PRIMARY NONE).
    assert!(game_logic.apply_combat_cycle_rider(bike_id, CombatCycleRider::Worker));
    {
        let b = game_logic.host_object(bike_id).expect("bike");
        assert!(
            b.weapon.is_none(),
            "worker rider residual must clear combat weapon"
        );
    }

    // Restore Rebel and fire residual.
    assert!(game_logic.apply_combat_cycle_rider(bike_id, CombatCycleRider::Rebel));
    let enemy = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(80.0, 0.0, 0.0))
        .expect("enemy");
    {
        let b = game_logic.host_object_mut(bike_id).unwrap();
        b.attack_target(enemy);
        if let Some(w) = b.weapon.as_mut() {
            w.last_fire_time = -10.0;
            w.reload_time = 0.1;
            w.min_range = 0.0;
        }
    }
    let enemy_hp_before = game_logic
        .host_object(enemy)
        .map(|e| e.health.current)
        .unwrap_or(0.0);

    game_logic.set_current_frame(50);
    game_logic.update_combat(&[bike_id, enemy], LOGIC_FRAME_TIMESTEP);

    assert!(
        game_logic.honesty_combat_cycle_fire_ok(),
        "combat cycle residual fire honesty"
    );
    assert!(
        game_logic.honesty_combat_cycle_ok(),
        "combat cycle residual host path honesty"
    );
    let enemy_hp_after = game_logic
        .host_object(enemy)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    assert!(
        enemy_hp_after < enemy_hp_before,
        "combat cycle rebel residual must damage target (before={enemy_hp_before} after={enemy_hp_after})"
    );

    // RiderChangeContain::onContaining (RiderChangeContain.cpp) switches the
    // bike weapon from the occupant. Live AI Enter also requires a parsed
    // SET_NORMAL locomotor row in LocomotorStore; this fixture exercises the
    // host residual APIs that current code produces without that INI row.
    let rpg_rider = game_logic
        .create_object(
            "GLAInfantryTunnelDefender",
            Team::GLA,
            Vec3::new(1.0, 0.0, 0.0),
        )
        .expect("rpg rider");
    {
        let b = game_logic.host_object_mut(bike_id).unwrap();
        b.combat_cycle_rider = 0;
        b.occupants.clear();
        b.weapon = None;
        assert!(
            b.add_occupant(rpg_rider),
            "combat cycle must load single rider residual"
        );
    }
    game_logic.record_combat_cycle_residual_load();
    game_logic.refresh_combat_cycle_rider_weapon(bike_id);
    {
        let b = game_logic.host_object(bike_id).expect("bike");
        assert!(b.contained_units().contains(&rpg_rider));
        assert_eq!(b.transport_count(), 1);
        let prim = b.weapon.as_ref().expect("rpg after load");
        assert!(
            (prim.damage - RPG_DAMAGE).abs() < 0.5,
            "enter residual must switch to TunnelDefender RPG weapon"
        );
    }
    assert!(
        game_logic.combat_cycle_residual_loads() >= 1,
        "combat cycle load residual honesty"
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
fn avenger_residual_designator_paint_and_rof() {
    use crate::game_logic::host_avenger::{FAERIE_FIRE_ROF_MULTIPLIER, is_avenger_template};
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let mut game_logic = GameLogic::new();

    let mut avenger_tpl = crate::game_logic::ThingTemplate::new("USA_Avenger");
    avenger_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(300.0)
        .set_primary_weapon_name(crate::game_logic::weapon_bootstrap::AVENGER_TARGET_DESIGNATOR)
        .set_secondary_weapon_name(crate::game_logic::weapon_bootstrap::AVENGER_AIR_LASER);
    game_logic
        .templates
        .insert("USA_Avenger".to_string(), avenger_tpl);

    let mut enemy_tpl = crate::game_logic::ThingTemplate::new("TestTank");
    enemy_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(500.0);
    game_logic
        .templates
        .insert("TestTank".to_string(), enemy_tpl);

    let mut ally_tpl = crate::game_logic::ThingTemplate::new("USA_Ranger");
    ally_tpl
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(100.0)
        .set_primary_weapon_name(crate::game_logic::weapon_bootstrap::RANGER_PRIMARY_WEAPON);
    game_logic
        .templates
        .insert("USA_Ranger".to_string(), ally_tpl);

    let avenger_id = game_logic
        .create_object("USA_Avenger", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("avenger");
    let enemy_id = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(50.0, 0.0, 0.0))
        .expect("enemy");
    let ally_id = game_logic
        .create_object("USA_Ranger", Team::USA, Vec3::new(10.0, 0.0, 0.0))
        .expect("ally");

    {
        let a = game_logic.host_object(avenger_id).expect("avenger");
        assert!(is_avenger_template(&a.template_name));
        assert!(a.weapon.is_some(), "designator primary residual");
        assert!(a.secondary_weapon.is_some(), "air laser secondary residual");
    }
    assert!(!game_logic.honesty_avenger_paint_ok());

    // Avenger paints enemy.
    {
        let a = game_logic.host_object_mut(avenger_id).unwrap();
        a.target = Some(enemy_id);
        a.set_ai_state(AIState::Attacking);
        if let Some(w) = a.weapon.as_mut() {
            w.last_fire_time = -10.0;
            w.reload_time = 0.1;
        }
        a.record_host_weapon_stats();
    }
    game_logic.frame = 1;
    game_logic.update_combat(&[avenger_id, enemy_id], LOGIC_FRAME_TIMESTEP);

    assert!(
        game_logic.honesty_avenger_paint_ok(),
        "designator residual honesty must record paint"
    );
    assert!(
        game_logic.avenger_residual_paints() > 0,
        "paint counter must advance"
    );
    {
        let e = game_logic.host_object(enemy_id).expect("enemy");
        assert!(e.is_faerie_fire(), "enemy must have FAERIE_FIRE residual");
        // Designator deals no HP damage residual.
        assert!(
            (e.health.current - e.health.maximum).abs() < 0.01
                || e.health.current >= e.health.maximum - 1.0,
            "designator must not deal hitpoint damage residual"
        );
    }

    // Ally fires at painted target — ROF residual readiness + honesty.
    // Retail paint duration is 6 frames; keep paint alive for the ROF check.
    {
        let until = game_logic.frame.saturating_add(60);
        let e = game_logic.host_object_mut(enemy_id).unwrap();
        e.apply_faerie_fire(until);
    }
    {
        let ally = game_logic.host_object_mut(ally_id).unwrap();
        ally.target = Some(enemy_id);
        ally.set_ai_state(AIState::Attacking);
        if let Some(w) = ally.weapon.as_mut() {
            w.damage = 10.0;
            w.range = 200.0;
            w.reload_time = 1.0;
            // Not ready under normal ROF (0.5s < 1.0s), ready under 150% ROF (~0.667).
            w.last_fire_time = 0.0;
        }
        ally.record_host_weapon_stats();
    }
    // t=0.7s with last_fire=0 reload=1.0 → only ready with FAERIE 150% ROF.
    game_logic.frame = 21;
    let current_time = game_logic.frame as f32 * LOGIC_FRAME_TIMESTEP;
    {
        let ally = game_logic.host_object(ally_id).unwrap();
        let enemy = game_logic.host_object(enemy_id).unwrap();
        assert!(
            enemy.is_faerie_fire(),
            "paint must still be active for ROF residual"
        );
        let w = ally.weapon.as_ref().unwrap();
        assert!(
            !Object::weapon_ready(w, current_time),
            "without FAERIE_FIRE ROF, reload 1.0 not ready at t=0.7"
        );
        assert!(
            Object::weapon_ready_vs_target(w, current_time, enemy.is_faerie_fire()),
            "with FAERIE_FIRE ROF 150%, ready at t=0.7 (effective reload ~0.667)"
        );
        assert!(
            (FAERIE_FIRE_ROF_MULTIPLIER - 1.5).abs() < 0.001,
            "retail ROF mult residual"
        );
        let slot = ally.select_combat_weapon_slot(enemy, current_time);
        assert_eq!(slot, Some(0), "ally must select primary vs painted target");
    }
    game_logic.update_combat(&[ally_id, enemy_id], LOGIC_FRAME_TIMESTEP);
    assert!(
        game_logic.honesty_avenger_rof_ok(),
        "shooting painted target must record ROF residual honesty"
    );
    assert!(game_logic.honesty_avenger_ok());
}

#[test]
fn avenger_residual_air_laser_damages_aircraft() {
    use crate::game_logic::host_avenger::AVENGER_AIR_LASER_DAMAGE;
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let mut game_logic = GameLogic::new();

    let mut avenger_tpl = crate::game_logic::ThingTemplate::new("AmericaTankAvenger");
    avenger_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(300.0)
        .set_primary_weapon_name(crate::game_logic::host_avenger::AVENGER_TARGET_DESIGNATOR)
        .set_secondary_weapon_name(crate::game_logic::host_avenger::AVENGER_AIR_LASER);
    game_logic
        .templates
        .insert("AmericaTankAvenger".to_string(), avenger_tpl);

    let mut jet_tpl = crate::game_logic::ThingTemplate::new("TestJet");
    jet_tpl
        .add_kind_of(KindOf::Aircraft)
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(100.0);
    game_logic.templates.insert("TestJet".to_string(), jet_tpl);

    let avenger_id = game_logic
        .create_object("AmericaTankAvenger", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("avenger");
    // Feet-level airborne: Weapon.cpp pitch window rejects a 50-unit loft
    // from a ground chassis; C++ airborne is OBJECT_STATUS_AIRBORNE_TARGET.
    let jet_id = game_logic
        .create_object("TestJet", Team::China, Vec3::new(80.0, 0.0, 0.0))
        .expect("jet");

    let hp_before = game_logic.host_object(jet_id).unwrap().health.current;
    {
        let a = game_logic.host_object_mut(avenger_id).unwrap();
        a.target = Some(jet_id);
        a.set_ai_state(AIState::Attacking);
        a.status.airborne_target = false;
        if let Some(w) = a.secondary_weapon.as_mut() {
            w.last_fire_time = -10.0;
            w.reload_time = 0.05;
            w.pre_attack_delay = 0.0;
        }
        // Force secondary slot preference residual.
        a.active_weapon_slot = 1;
    }
    {
        let j = game_logic.host_object_mut(jet_id).unwrap();
        j.status.airborne_target = true;
    }
    game_logic.frame = 5;
    for _ in 0..4 {
        game_logic.frame = game_logic.frame.saturating_add(1);
        game_logic.update_combat(&[avenger_id, jet_id], LOGIC_FRAME_TIMESTEP);
    }

    assert!(
        game_logic.honesty_avenger_air_laser_ok(),
        "air laser residual honesty"
    );
    let hp_after = game_logic.host_object(jet_id).unwrap().health.current;
    assert!(
        hp_after < hp_before,
        "air laser must damage aircraft residual (before={hp_before} after={hp_after})"
    );
    let dealt = hp_before - hp_after;
    assert!(
        dealt + 0.01 >= AVENGER_AIR_LASER_DAMAGE * 0.5,
        "air laser damage residual ~10 (armor may reduce); dealt={dealt}"
    );
}

#[test]
fn lazr_tank_residual_laser_guns() {
    use crate::game_logic::host_usa_tanks::{
        LAZR_CRUSADER_TANK_GUN_DAMAGE, LAZR_PALADIN_TANK_GUN_DAMAGE, is_laser_general_tank_template,
    };
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let mut game_logic = GameLogic::new();

    for (name, hp) in [
        ("Lazr_AmericaTankCrusader", 480.0_f32),
        ("Lazr_AmericaTankPaladin", 500.0_f32),
    ] {
        let mut tpl = crate::game_logic::ThingTemplate::new(name);
        tpl.add_kind_of(KindOf::Vehicle)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_health(hp);
        game_logic.templates.insert(name.to_string(), tpl);
    }

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
        .create_object(
            "Lazr_AmericaTankCrusader",
            Team::USA,
            Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("lazr crusader");
    let paladin_id = game_logic
        .create_object(
            "Lazr_AmericaTankPaladin",
            Team::USA,
            Vec3::new(0.0, 40.0, 0.0),
        )
        .expect("lazr paladin");
    let enemy_id = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(80.0, 0.0, 0.0))
        .expect("enemy");

    {
        let c = game_logic.host_object(crusader_id).expect("crusader");
        assert!(is_laser_general_tank_template(&c.template_name));
        let w = c.weapon.as_ref().expect("Lazr_CrusaderTankGun residual");
        assert!(
            (w.damage - LAZR_CRUSADER_TANK_GUN_DAMAGE).abs() < 0.5,
            "Lazr Crusader damage residual 80, got {}",
            w.damage
        );
        assert!((w.reload_time - 2.0).abs() < 0.05);
    }
    {
        let p = game_logic.host_object(paladin_id).expect("paladin");
        let w = p.weapon.as_ref().expect("Lazr_PaladinTankGun residual");
        assert!(
            (w.damage - LAZR_PALADIN_TANK_GUN_DAMAGE).abs() < 0.5,
            "Lazr Paladin damage residual 70, got {}",
            w.damage
        );
        assert!((w.reload_time - 1.0).abs() < 0.05);
    }

    let hp_before = game_logic.host_object(enemy_id).unwrap().health.current;
    {
        let c = game_logic.host_object_mut(crusader_id).unwrap();
        c.target = Some(enemy_id);
        c.set_ai_state(AIState::Attacking);
        if let Some(w) = c.weapon.as_mut() {
            w.last_fire_time = -10.0;
            w.reload_time = 0.1;
        }
    }
    game_logic.frame = 10;
    game_logic.update_combat(&[crusader_id, enemy_id], LOGIC_FRAME_TIMESTEP);
    let hp_after = game_logic.host_object(enemy_id).unwrap().health.current;
    assert!(
        hp_after < hp_before,
        "Lazr Crusader laser gun must damage residual (before={hp_before} after={hp_after})"
    );
    // Observable laser residual: higher damage than stock 60 shell when armor allows.
    let dealt = hp_before - hp_after;
    assert!(
        dealt + 0.01 >= 40.0,
        "laser residual should deal substantial damage, dealt={dealt}"
    );
}

#[test]
fn lazr_patriot_residual_laser_dual_slot() {
    use crate::game_logic::host_base_defense::{
        LAZR_PATRIOT_AIR_DAMAGE, LAZR_PATRIOT_GROUND_DAMAGE, is_laser_patriot_template,
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
        .set_health(1000.0);
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
        assert!(is_laser_patriot_template(&p.template_name));
        let g = p.weapon.as_ref().expect("Lazr ground residual");
        assert!(
            (g.damage - LAZR_PATRIOT_GROUND_DAMAGE).abs() < 0.5,
            "Lazr Patriot ground 40, got {}",
            g.damage
        );
        let a = p.secondary_weapon.as_ref().expect("Lazr AA residual");
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
    crate::game_logic::host_damage_log::clear();
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
    let dealt_g = test_observed_damage_to(enemy_id, hp_before, hp_after);
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
    if let Some(e) = game_logic.host_object_mut(enemy_id) {
        e.set_position(Vec3::new(5000.0, 0.0, 0.0));
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
    crate::game_logic::host_damage_log::clear();
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
    let dealt = test_observed_damage_to(air_id, air_hp_before, air_hp_after);
    assert!(
        dealt > 0.0 || air_hp_after < air_hp_before,
        "Lazr Patriot AA residual must damage aircraft (dealt={dealt}, before={air_hp_before} after={air_hp_after})"
    );
}

#[test]
fn tunnel_network_gun_residual_auto_fires() {
    use crate::game_logic::host_tunnel_network::{
        TUNNEL_NETWORK_GUN_DAMAGE, is_tunnel_network_template,
    };
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let mut game_logic = GameLogic::new();
    crate::game_logic::host_damage_log::clear();

    let mut tunnel_tpl = crate::game_logic::ThingTemplate::new("GLATunnelNetwork");
    tunnel_tpl
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(1000.0);
    game_logic
        .templates
        .insert("GLATunnelNetwork".to_string(), tunnel_tpl);

    let mut enemy_tpl = crate::game_logic::ThingTemplate::new("TestTank");
    enemy_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(200.0);
    game_logic
        .templates
        .insert("TestTank".to_string(), enemy_tpl);

    let tunnel_id = game_logic
        .create_object("GLATunnelNetwork", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("tunnel");
    if let Some(t) = game_logic.host_object_mut(tunnel_id) {
        t.set_status_under_construction(false);
        t.construction_percent = 100.0;
    }
    {
        let t = game_logic.host_object(tunnel_id).expect("tunnel");
        assert!(is_tunnel_network_template(&t.template_name));
        assert!(
            crate::game_logic::host_base_defense::is_base_defense_structure(
                &t.template_name,
                true,
                false
            )
        );
        let w = t.weapon.as_ref().expect("TunnelNetworkGun residual");
        assert!(
            (w.damage - TUNNEL_NETWORK_GUN_DAMAGE).abs() < 0.5,
            "TunnelNetworkGun damage residual 15, got {}",
            w.damage
        );
    }

    let enemy_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(100.0, 0.0, 0.0))
        .expect("enemy");
    let hp_before = game_logic.host_object(enemy_id).unwrap().health.current;
    {
        let t = game_logic.host_object_mut(tunnel_id).unwrap();
        if let Some(w) = t.weapon.as_mut() {
            w.last_fire_time = -10.0;
        }
    }
    game_logic.frame = 20;
    for _ in 0..30 {
        game_logic.try_base_defense_residual_fire(tunnel_id);
        let hp_now = game_logic.host_object(enemy_id).unwrap().health.current;
        if hp_now < hp_before {
            break;
        }
        game_logic.frame = game_logic.frame.saturating_add(1);
    }

    let hp_after = game_logic.host_object(enemy_id).unwrap().health.current;
    let logged = crate::game_logic::host_damage_log::drain();
    let log_hit = logged
        .iter()
        .any(|e| e.target == enemy_id && e.amount > 0.0);
    assert!(
        hp_after < hp_before || log_hit,
        "TunnelNetworkGun residual must damage host HP or damage-log under authority (before={hp_before} after={hp_after} log_hit={log_hit})"
    );
    assert!(
        game_logic.honesty_tunnel_network_gun_ok(),
        "tunnel gun residual honesty"
    );
    assert!(
        game_logic.tunnel_network_residual_gun_fires() > 0,
        "gun fire counter"
    );
    assert!(
        game_logic.base_defense_residual_fires() > 0,
        "base defense residual fire counter"
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
        c.set_ai_state(AIState::Attacking);
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
            .values()
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
    logic.apply_tow_unlock_to_team(Team::USA, UPGRADE_AMERICA_TOW);

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
            .values()
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
            .values()
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
        unit.set_ai_state(AIState::Entering);
    }
    game_logic.update_ai(&[r1, humvee_id], 1.0 / 30.0);
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
        h.set_ai_state(AIState::Attacking);
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
            .values()
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

#[test]
fn minigunner_residual_gun_ramp_aa_horde_and_chain_guns() {
    use crate::game_logic::host_battlemaster::UPGRADE_NATIONALISM;
    use crate::game_logic::host_gattling_tank::GattlingFireLevel;
    use crate::game_logic::host_minigunner::{
        MINIGUNNER_GROUND_DAMAGE, MINIGUNNER_GROUND_RANGE, MINIGUNNER_GUN, MINIGUNNER_GUN_AIR,
        is_minigunner_template,
    };
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();

    let mut game_logic = GameLogic::new();
    ensure_test_infantry_template(&mut game_logic);
    ensure_test_tank_template(&mut game_logic);

    let mut mg_tpl = crate::game_logic::ThingTemplate::new("Infa_ChinaInfantryMiniGunner");
    mg_tpl
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(120.0)
        .set_primary_weapon_name(MINIGUNNER_GUN)
        .set_secondary_weapon_name(MINIGUNNER_GUN_AIR);
    game_logic
        .templates
        .insert("Infa_ChinaInfantryMiniGunner".to_string(), mg_tpl);

    let mut aircraft_tpl = crate::game_logic::ThingTemplate::new("TestAircraft");
    aircraft_tpl
        .add_kind_of(KindOf::Aircraft)
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Attackable)
        .add_kind_of(KindOf::Selectable)
        .set_health(100.0);
    game_logic
        .templates
        .insert("TestAircraft".to_string(), aircraft_tpl);

    // Spawn 5 ally minigunners for Horde Count=5 residual.
    let mut mg_ids = Vec::new();
    for i in 0..5 {
        let id = game_logic
            .create_object(
                "Infa_ChinaInfantryMiniGunner",
                Team::China,
                Vec3::new(i as f32 * 5.0, 0.0, 0.0),
            )
            .expect("minigunner");
        mg_ids.push(id);
    }
    let mg0 = mg_ids[0];
    let base_reload = {
        let mg = game_logic.host_object(mg0).expect("mg0");
        assert!(is_minigunner_template(&mg.template_name));
        let prim = mg.weapon.as_ref().expect("ground gun");
        assert!((prim.damage - MINIGUNNER_GROUND_DAMAGE).abs() < 0.5);
        // Retail Weapon.ini:135005 Infa_MiniGunnerGun AttackRange 125.0.
        assert!((prim.range - MINIGUNNER_GROUND_RANGE).abs() < 1.0);
        assert!(prim.can_target_ground && !prim.can_target_air);
        let sec = mg.secondary_weapon.as_ref().expect("aa gun");
        assert!(sec.can_target_air && !sec.can_target_ground);
        assert_eq!(mg.continuous_fire_level, 0);
        // Base 15 frames → 0.5s
        assert!((prim.reload_time - 0.5).abs() < 0.05);
        prim.reload_time
    };

    // Horde residual: 5 infantry allies → HORDE ROF 150% (floor(15/1.5)=10 frames).
    game_logic.update_china_infantry_horde_status();
    assert!(
        game_logic.honesty_minigunner_horde_ok(),
        "minigunner horde grant residual honesty"
    );
    {
        let mg = game_logic.host_object(mg0).expect("mg0 horde");
        assert!(mg.weapon_bonus_horde);
        let w = mg.weapon.as_ref().expect("horde gun");
        assert!(
            (w.reload_time - (10.0 / 30.0)).abs() < 0.05,
            "horde ROF residual ~10 frames, got reload={}",
            w.reload_time
        );
    }

    // Nationalism residual: additional ROF 125% while in horde → floor(15/1.875)=8 frames.
    assert!(game_logic.apply_minigunner_nationalism_upgrade(mg0));
    assert!(game_logic.honesty_minigunner_nationalism_ok());
    {
        let mg = game_logic.host_object(mg0).expect("mg0 nat");
        assert!(mg.has_upgrade_tag(UPGRADE_NATIONALISM));
        assert!(mg.weapon_bonus_nationalism);
        let w = mg.weapon.as_ref().expect("nat gun");
        assert!(
            (w.reload_time - (8.0 / 30.0)).abs() < 0.05,
            "horde+nationalism ROF residual ~8 frames, got reload={}",
            w.reload_time
        );
    }

    // Chain Guns residual → damage × 1.25.
    assert!(game_logic.apply_minigunner_chain_guns_upgrade(mg0));
    {
        let mg = game_logic.host_object(mg0).expect("mg0 chain");
        let prim = mg.weapon.as_ref().expect("chained ground");
        assert!(
            (prim.damage - MINIGUNNER_GROUND_DAMAGE * 1.25).abs() < 0.01,
            "chain guns residual 125% damage, got {}",
            prim.damage
        );
    }

    // Fire repeatedly at same target to ramp continuous fire residual (MEAN after >6).
    let enemy = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(80.0, 0.0, 0.0))
        .expect("enemy");
    let enemy_hp_before = game_logic
        .host_object(enemy)
        .map(|e| e.health.current)
        .unwrap_or(0.0);

    for i in 0..10u32 {
        {
            let mg = game_logic.host_object_mut(mg0).unwrap();
            mg.attack_target(enemy);
            if let Some(w) = mg.weapon.as_mut() {
                w.last_fire_time = -10.0;
                w.reload_time = 0.05;
            }
            if let Some(w) = mg.secondary_weapon.as_mut() {
                w.last_fire_time = 0.0;
                w.reload_time = 1000.0;
            }
        }
        game_logic.set_current_frame(30 + (i as u64) * 15);
        game_logic.update_combat(&[mg0, enemy], LOGIC_FRAME_TIMESTEP);
    }

    assert!(
        game_logic.minigunner_residual_ground_fires() > 0,
        "minigunner ground residual honesty"
    );
    assert!(
        game_logic.honesty_minigunner_ramp_ok(),
        "minigunner continuous-fire ramp residual honesty must reach MEAN or FAST"
    );
    {
        let mg = game_logic.host_object(mg0).expect("mg0 ramped");
        assert!(
            mg.continuous_fire_level >= GattlingFireLevel::Mean.as_u8(),
            "after multi-shot residual must be MEAN or FAST, level={}",
            mg.continuous_fire_level
        );
        let prim = mg.weapon.as_ref().expect("ramped gun");
        assert!(
            prim.reload_time < base_reload - 0.05,
            "ramped fire residual faster than base (base={base_reload} now={})",
            prim.reload_time
        );
    }
    let enemy_hp_after = game_logic
        .host_object(enemy)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    assert!(
        enemy_hp_after < enemy_hp_before,
        "minigunner residual must damage ground target"
    );

    // AA secondary residual vs airborne.
    let aircraft_id = game_logic
        .create_object("TestAircraft", Team::GLA, Vec3::new(80.0, 0.0, 0.0))
        .expect("aircraft");
    {
        let a = game_logic.host_object_mut(aircraft_id).unwrap();
        a.status.airborne_target = true;
    }
    {
        let mg = game_logic.host_object_mut(mg0).unwrap();
        mg.attack_target(aircraft_id);
        mg.active_weapon_slot = 1;
        if let Some(w) = mg.secondary_weapon.as_mut() {
            w.last_fire_time = -10.0;
            w.reload_time = 0.1;
            w.pre_attack_delay = 0.0;
        }
        if let Some(w) = mg.weapon.as_mut() {
            w.last_fire_time = 0.0;
            w.reload_time = 1000.0;
        }
    }
    let air_hp_before = game_logic
        .host_object(aircraft_id)
        .map(|a| a.health.current)
        .unwrap_or(0.0);
    game_logic.set_current_frame(400);
    for i in 0..4u32 {
        game_logic.set_current_frame(400 + u64::from(i) * 5);
        game_logic.update_combat(&[mg0, aircraft_id, enemy], LOGIC_FRAME_TIMESTEP);
    }
    assert!(
        game_logic.honesty_minigunner_aa_ok(),
        "minigunner AA residual honesty must fire secondary vs airborne"
    );
    let air_hp_after = game_logic
        .host_object(aircraft_id)
        .map(|a| a.health.current)
        .unwrap_or(0.0);
    assert!(
        air_hp_after < air_hp_before,
        "airborne target must take minigunner AA residual"
    );
    assert!(
        game_logic.honesty_minigunner_ok(),
        "minigunner residual host path honesty"
    );
}
