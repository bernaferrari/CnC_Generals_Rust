//! Behavior suite extracted from `unit_residuals`.
use super::*;

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
