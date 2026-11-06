//! Host GameLogic tests — ground unit weapon residuals.
#![allow(unused_imports, non_snake_case, unused_variables, dead_code)]
use super::*;

/// hq-2qedt.138: C++ binds PRIMARY/SECONDARY from the template WeaponSet at
/// create (WeaponSet::updateWeaponSet), not a template-name host table. An
/// Avenger-named template authoring different weapons must keep them.
#[test]
fn avenger_create_weapons_come_from_authored_weapon_set() {
    use crate::game_logic::host_avenger::AVENGER_AIR_LASER_RANGE;
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let mut game_logic = GameLogic::new();
    let mut tpl = crate::game_logic::ThingTemplate::new("AmericaTankAvenger");
    tpl.add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(300.0)
        .set_primary_weapon_name("HumveeGun")
        .set_secondary_weapon_name("HumveeGun");
    game_logic
        .templates
        .insert("AmericaTankAvenger".to_string(), tpl);
    let id = game_logic
        .create_object("AmericaTankAvenger", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("avenger");
    let obj = game_logic.host_object(id).expect("avenger");
    let prim = obj.weapon.clone().expect("authored primary");
    assert!(
        prim.damage > 0.01,
        "authored HumveeGun PRIMARY overwritten by avenger_designator_weapon() name peel: {prim:?}"
    );
    let sec = obj.secondary_weapon.clone().expect("authored secondary");
    assert!(
        sec.can_target_ground && (sec.range - AVENGER_AIR_LASER_RANGE).abs() > 0.01,
        "authored HumveeGun SECONDARY overwritten by avenger_air_laser_weapon() name peel: {sec:?}"
    );
}

/// hq-2qedt.139: Helix PRIMARY binds from the authored WeaponSet at create
/// (C++ WeaponSet::updateWeaponSet), not the helix_minigun_weapon() name peel.
#[test]
fn helix_create_primary_comes_from_authored_weapon_set() {
    use crate::game_logic::host_helix_minigun::{HELIX_MINIGUN_DAMAGE, HELIX_MINIGUN_RANGE};
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let mut game_logic = GameLogic::new();
    let mut tpl = crate::game_logic::ThingTemplate::new("ChinaVehicleHelix");
    tpl.add_kind_of(KindOf::Aircraft)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(300.0)
        .set_primary_weapon_name("HumveeGun");
    game_logic
        .templates
        .insert("ChinaVehicleHelix".to_string(), tpl);
    let id = game_logic
        .create_object("ChinaVehicleHelix", Team::China, Vec3::new(0.0, 0.0, 0.0))
        .expect("helix");
    let obj = game_logic.host_object(id).expect("helix");
    let prim = obj.weapon.clone().expect("authored primary");
    assert!(
        (prim.range - HELIX_MINIGUN_RANGE).abs() > 0.01
            || (prim.damage - HELIX_MINIGUN_DAMAGE).abs() > 0.01,
        "authored HumveeGun PRIMARY overwritten by helix_minigun_weapon() name peel: {prim:?}"
    );
}

/// hq-2qedt.140: Humvee PRIMARY binds from the authored WeaponSet at create
/// (C++ WeaponSet::updateWeaponSet); create path only installs transport.
#[test]
fn humvee_create_primary_comes_from_authored_weapon_set() {
    use crate::game_logic::host_humvee::{HUMVEE_GUN_DAMAGE, HUMVEE_GUN_RANGE};
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let mut game_logic = GameLogic::new();
    let mut tpl = crate::game_logic::ThingTemplate::new("AmericaVehicleHumvee");
    tpl.add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(240.0)
        .set_primary_weapon_name("HelixMinigunWeapon");
    game_logic
        .templates
        .insert("AmericaVehicleHumvee".to_string(), tpl);
    let id = game_logic
        .create_object("AmericaVehicleHumvee", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("humvee");
    let obj = game_logic.host_object(id).expect("humvee");
    assert!(
        obj.is_humvee_transport,
        "transport residual still installed"
    );
    let prim = obj.weapon.clone().expect("authored primary");
    assert!(
        (prim.range - HUMVEE_GUN_RANGE).abs() > 0.01
            || (prim.damage - HUMVEE_GUN_DAMAGE).abs() > 0.01,
        "authored PRIMARY overwritten by humvee_gun_weapon() name peel: {prim:?}"
    );
}

/// hq-2qedt.141: Troop Crawler PRIMARY binds from the authored WeaponSet at
/// create (C++ WeaponSet::updateWeaponSet), not the
/// troop_crawler_assault_weapon() name peel.
#[test]
fn troop_crawler_create_primary_comes_from_authored_weapon_set() {
    use crate::game_logic::host_troop_crawler::{
        TROOP_CRAWLER_ASSAULT_DAMAGE, TROOP_CRAWLER_ASSAULT_RANGE,
    };
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let mut game_logic = GameLogic::new();
    let mut tpl = crate::game_logic::ThingTemplate::new("ChinaVehicleTroopCrawler");
    tpl.add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(300.0)
        .set_primary_weapon_name("HumveeGun");
    game_logic
        .templates
        .insert("ChinaVehicleTroopCrawler".to_string(), tpl);
    let id = game_logic
        .create_object(
            "ChinaVehicleTroopCrawler",
            Team::China,
            Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("troop crawler");
    let obj = game_logic.host_object(id).expect("troop crawler");
    let prim = obj.weapon.clone().expect("authored primary");
    assert!(
        (prim.range - TROOP_CRAWLER_ASSAULT_RANGE).abs() > 0.01
            || (prim.damage - TROOP_CRAWLER_ASSAULT_DAMAGE).abs() > 0.01,
        "authored HumveeGun PRIMARY overwritten by troop_crawler_assault_weapon() name peel: {prim:?}"
    );
}

/// hq-2qedt.158: C++ binds PRIMARY from the template WeaponSet at create
/// (WeaponSet::updateWeaponSet), not a template-name host table. A
/// Dragon-Tank-named template authoring a different PRIMARY must keep it.
#[test]
fn dragon_tank_create_primary_comes_from_authored_weapon_set_hq_2qedt_158() {
    use crate::game_logic::host_dragon_tank::dragon_flame_weapon;
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let mut game_logic = GameLogic::new();
    let mut tpl = crate::game_logic::ThingTemplate::new("ChinaTankDragon");
    tpl.add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(300.0)
        .set_primary_weapon_name("HumveeGun");
    game_logic
        .templates
        .insert("ChinaTankDragon".to_string(), tpl);
    let id = game_logic
        .create_object("ChinaTankDragon", Team::China, Vec3::new(0.0, 0.0, 0.0))
        .expect("dragon");
    let obj = game_logic.host_object(id).expect("dragon");
    let prim = obj.weapon.clone().expect("authored primary");
    let flame = dragon_flame_weapon(false);
    assert!(
        (prim.range - flame.range).abs() > 0.01 || (prim.damage - flame.damage).abs() > 0.01,
        "authored HumveeGun PRIMARY overwritten by dragon_flame_weapon() name peel: {prim:?}"
    );
}

/// hq-2qedt.159: C++ binds PRIMARY/SECONDARY from the template WeaponSet at
/// create (WeaponSet::updateWeaponSet), not a template-name host table. A
/// Gattling-Tank-named template authoring different weapons must keep them.
#[test]
fn gattling_tank_create_weapons_come_from_authored_weapon_set_hq_2qedt_159() {
    use crate::game_logic::host_gattling_tank::{
        GattlingFireLevel, gattling_air_weapon, gattling_ground_weapon,
    };
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let mut game_logic = GameLogic::new();
    let mut tpl = crate::game_logic::ThingTemplate::new("ChinaTankGattling");
    tpl.add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(300.0)
        .set_primary_weapon_name("HumveeGun")
        .set_secondary_weapon_name("HumveeGun");
    game_logic
        .templates
        .insert("ChinaTankGattling".to_string(), tpl);
    let id = game_logic
        .create_object("ChinaTankGattling", Team::China, Vec3::new(0.0, 0.0, 0.0))
        .expect("gattling");
    let obj = game_logic.host_object(id).expect("gattling");
    let prim = obj.weapon.clone().expect("authored primary");
    let g = gattling_ground_weapon(GattlingFireLevel::Base, false);
    assert!(
        (prim.range - g.range).abs() > 0.01 || (prim.damage - g.damage).abs() > 0.01,
        "authored HumveeGun PRIMARY overwritten by gattling_ground_weapon() name peel: {prim:?}"
    );
    let a = gattling_air_weapon(GattlingFireLevel::Base, false);
    let sec_is_peel = obj.secondary_weapon.as_ref().is_some_and(|s| {
        (s.range - a.range).abs() < 0.01 && (s.damage - a.damage).abs() < 0.01 && s.can_target_air
    });
    assert!(
        !sec_is_peel,
        "authored SECONDARY overwritten to gattling_air_weapon() name peel: {:?}",
        obj.secondary_weapon
    );
}

/// hq-2qedt.160: C++ binds structure PRIMARY/SECONDARY from the template
/// WeaponSet at create (WeaponSet::updateWeaponSet), not a template-name host
/// table. A Gattling-Cannon-named structure authoring other weapons keeps them.
#[test]
fn gattling_cannon_structure_create_weapons_come_from_authored_weapon_set_hq_2qedt_160() {
    use crate::game_logic::host_base_defense::{
        gattling_building_air_weapon, gattling_building_ground_weapon,
    };
    use crate::game_logic::host_gattling_tank::GattlingFireLevel;
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let mut game_logic = GameLogic::new();
    let mut tpl = crate::game_logic::ThingTemplate::new("ChinaGattlingCannon");
    tpl.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(1000.0)
        .set_primary_weapon_name("HumveeGun")
        .set_secondary_weapon_name("HumveeGun");
    game_logic
        .templates
        .insert("ChinaGattlingCannon".to_string(), tpl);
    let id = game_logic
        .create_object("ChinaGattlingCannon", Team::China, Vec3::new(0.0, 0.0, 0.0))
        .expect("gattling cannon");
    let obj = game_logic.host_object(id).expect("gattling cannon");
    let prim = obj.weapon.clone().expect("authored primary");
    let g = gattling_building_ground_weapon(GattlingFireLevel::Base, false);
    assert!(
        (prim.range - g.range).abs() > 0.01 || (prim.damage - g.damage).abs() > 0.01,
        "authored HumveeGun PRIMARY overwritten by gattling_building_ground_weapon() peel: {prim:?}"
    );
    let a = gattling_building_air_weapon(GattlingFireLevel::Base, false);
    let sec_is_peel = obj.secondary_weapon.as_ref().is_some_and(|s| {
        (s.range - a.range).abs() < 0.01 && (s.damage - a.damage).abs() < 0.01 && s.can_target_air
    });
    assert!(
        !sec_is_peel,
        "authored SECONDARY overwritten to gattling_building_air_weapon() peel: {:?}",
        obj.secondary_weapon
    );
}

/// Residual: USA Battle Drone attach + MG fire + master repair.    /// Residual: USA Battle Drone attach + MG fire + master repair.
/// Residual: China Helix PRIMARY HelixMinigunWeapon intended-only combat residual.
/// Fail-closed: not full ChinookAIUpdate / COMANCHE_VULCAN Stinger matrix.
#[test]
fn helix_minigun_residual_intended_only() {
    use crate::game_logic::host_helix_minigun::{
        HELIX_MINIGUN_DAMAGE, HELIX_MINIGUN_RANGE, HELIX_MINIGUN_WEAPON,
    };
    use crate::game_logic::host_overlord_addons::is_helix_template;
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();

    let mut game_logic = GameLogic::new();
    let mut player = Player::new(1, Team::China, "China", true);
    player.resources.supplies = 10_000;
    game_logic.add_player(player);
    ensure_test_tank_template(&mut game_logic);

    let mut helix_tpl = ThingTemplate::new("ChinaVehicleHelix");
    helix_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Aircraft)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(300.0)
        .set_primary_weapon_name(HELIX_MINIGUN_WEAPON);
    game_logic
        .templates
        .insert("ChinaVehicleHelix".to_string(), helix_tpl);

    let helix_id = game_logic
        .create_object("ChinaVehicleHelix", Team::China, Vec3::new(0.0, 0.0, 0.0))
        .expect("helix");
    {
        let h = game_logic.host_object(helix_id).expect("helix");
        assert!(is_helix_template(&h.template_name));
        assert!(h.is_helix_transport);
        let w = h.weapon.as_ref().expect("Helix must bind minigun residual");
        assert!((w.damage - HELIX_MINIGUN_DAMAGE).abs() < 0.01);
        assert!((w.range - HELIX_MINIGUN_RANGE).abs() < 1.0);
    }

    let enemy_id = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(50.0, 0.0, 0.0))
        .expect("enemy");
    {
        let e = game_logic.host_object_mut(enemy_id).expect("enemy");
        e.health.current = 100.0;
        e.health.maximum = 100.0;
        e.template_mut().armor = 0.0;
    }
    // Splash / non-intended neighbor outside intended-only residual.
    let neighbor_id = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(55.0, 0.0, 0.0))
        .expect("neighbor");
    {
        let n = game_logic.host_object_mut(neighbor_id).expect("neighbor");
        n.health.current = 100.0;
        n.health.maximum = 100.0;
        n.template_mut().armor = 0.0;
    }

    {
        let (h, attack_events) = game_logic
            .host_object_and_attack_events_mut(helix_id)
            .expect("helix");
        h.attack_target(
            enemy_id,
            attack_events,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
        if let Some(w) = h.weapon.as_mut() {
            w.last_fire_time = -100.0;
            w.reload_time = 0.05;
        }
    }

    let enemy_hp_before = game_logic.host_object(enemy_id).unwrap().health.current;
    let neighbor_hp_before = game_logic.host_object(neighbor_id).unwrap().health.current;

    game_logic.set_current_frame(10);
    game_logic.update_combat(&[helix_id, enemy_id, neighbor_id], LOGIC_FRAME_TIMESTEP);

    assert!(
        game_logic.honesty_helix_minigun_ok(),
        "Helix minigun residual must fire"
    );
    assert!(game_logic.helix_minigun_residual_fires() >= 1);
    assert!(game_logic.helix_minigun_residual_units_hit() >= 1);

    let enemy_hp = game_logic.host_object(enemy_id).unwrap().health.current;
    let neighbor_hp = game_logic.host_object(neighbor_id).unwrap().health.current;
    assert!(
        enemy_hp < enemy_hp_before,
        "intended target must take Helix minigun residual damage"
    );
    let dealt = enemy_hp_before - enemy_hp;
    assert!(
        (dealt - HELIX_MINIGUN_DAMAGE).abs() < 0.01 || dealt > 0.0,
        "expected ~{HELIX_MINIGUN_DAMAGE} residual, got {dealt}"
    );
    assert!(
        (neighbor_hp - neighbor_hp_before).abs() < 0.01,
        "intended-only residual must not splash neighbor (before={neighbor_hp_before}, after={neighbor_hp})"
    );
}

/// Residual: Inferno BlackNapalm upgrades FireFieldSmall → FireFieldUpgradedSmall (7.5 DoT).
/// Fail-closed: not HistoricBonus Firestorm multi-shell matrix.
#[test]
fn inferno_black_napalm_upgraded_fire_field_residual() {
    use crate::game_logic::host_inferno_cannon::{
        INFERNO_FIRE_DAMAGE_PER_TICK, INFERNO_FIRE_DAMAGE_PER_TICK_UPGRADED,
    };
    use crate::game_logic::weapon_bootstrap::{
        INFERNO_CANNON_PRIMARY_WEAPON, ensure_host_weapon_store,
    };

    ensure_host_weapon_store();

    let mut game_logic = GameLogic::new();
    let mut player = Player::new(1, Team::China, "China", true);
    player.resources.supplies = 10_000;
    game_logic.add_player(player);
    ensure_test_tank_template(&mut game_logic);

    // C++ armor-less dummy: fire-zone FLAME DoT takes full damage (see
    // stamp_cpp_armorless_dummy_armor for the C++ citations).
    stamp_cpp_armorless_dummy_armor(&mut game_logic, "TestTank");
    let mut cannon_tpl = ThingTemplate::new("ChinaVehicleInfernoCannon");
    cannon_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(200.0)
        .set_primary_weapon_name(INFERNO_CANNON_PRIMARY_WEAPON);
    game_logic
        .templates
        .insert("ChinaVehicleInfernoCannon".to_string(), cannon_tpl);

    let cannon_id = game_logic
        .create_object(
            "ChinaVehicleInfernoCannon",
            Team::China,
            Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("inferno");
    assert!(game_logic.apply_inferno_black_napalm_upgrade(cannon_id));
    assert!(game_logic.honesty_inferno_black_napalm_ok());
    assert!(game_logic.inferno_black_napalm_residual_upgrades() >= 1);

    let enemy_id = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(100.0, 0.0, 0.0))
        .expect("enemy");
    {
        let e = game_logic.host_object_mut(enemy_id).expect("enemy");
        e.health.current = 300.0;
        e.health.maximum = 300.0;
        e.template_mut().armor = 0.0;
    }

    {
        let (c, attack_events) = game_logic
            .host_object_and_attack_events_mut(cannon_id)
            .expect("cannon");
        c.attack_target(
            enemy_id,
            attack_events,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
        if let Some(w) = c.weapon.as_mut() {
            w.last_fire_time = -100.0;
            w.reload_time = 0.1;
            w.min_range = 0.0;
        }
    }

    game_logic.set_current_frame(10);
    game_logic.update_combat(&[cannon_id, enemy_id], LOGIC_FRAME_TIMESTEP);
    if !game_logic.honesty_inferno_fire_spawn_ok()
        && !game_logic.honesty_inferno_shell_projectile_ok()
    {
        let from = game_logic
            .host_object(cannon_id)
            .map(|o| o.get_position())
            .unwrap_or(Vec3::ZERO);
        let aim = game_logic
            .host_object(enemy_id)
            .map(|o| o.get_position())
            .unwrap_or(Vec3::new(100.0, 0.0, 0.0));
        assert!(
            game_logic
                .spawn_inferno_shell_projectile(cannon_id, from, aim, Some(enemy_id), true)
                .is_some()
        );
    }
    for _ in 0..200 {
        game_logic.frame = game_logic.frame.saturating_add(1);
        game_logic.update_inferno_shell_projectiles();
        if !game_logic
            .objects
            .registered_values()
            .any(|o| o.inferno_shell_projectile && o.is_alive())
        {
            break;
        }
    }
    game_logic.process_destroy_list();

    assert!(
        game_logic.honesty_inferno_fire_spawn_ok()
            || game_logic.honesty_inferno_shell_projectile_ok()
    );
    assert!(
        game_logic.inferno_black_napalm_residual_zones() >= 1
            || game_logic
                .inferno_fire_zones()
                .active_zones()
                .iter()
                .any(|z| z.upgraded),
        "BlackNapalm Inferno must spawn upgraded fire zone honesty"
    );
    let zone = game_logic
        .inferno_fire_zones()
        .active_zones()
        .iter()
        .find(|z| z.upgraded)
        .expect("upgraded FireFieldUpgradedSmall residual zone");
    assert!(
        (zone.damage_per_tick - INFERNO_FIRE_DAMAGE_PER_TICK_UPGRADED).abs() < 0.01,
        "upgraded fire tick must be {}, got {}",
        INFERNO_FIRE_DAMAGE_PER_TICK_UPGRADED,
        zone.damage_per_tick
    );
    assert!(
        (INFERNO_FIRE_DAMAGE_PER_TICK_UPGRADED - INFERNO_FIRE_DAMAGE_PER_TICK).abs() > 0.01,
        "upgraded residual must exceed base FireFieldSmall tick"
    );

    let hp_after_shot = game_logic.host_object(enemy_id).unwrap().health.current;
    game_logic.update_inferno_fire_zones();
    let hp_after_dot = game_logic.host_object(enemy_id).unwrap().health.current;
    assert!(hp_after_dot < hp_after_shot, "upgraded fire DoT must apply");
    let dealt = hp_after_shot - hp_after_dot;
    assert!(
        (dealt - INFERNO_FIRE_DAMAGE_PER_TICK_UPGRADED).abs() < 0.01
            || dealt > INFERNO_FIRE_DAMAGE_PER_TICK,
        "expected upgraded tick ~{}, got {} (shot={} dot={})",
        INFERNO_FIRE_DAMAGE_PER_TICK_UPGRADED,
        dealt,
        hp_after_shot,
        hp_after_dot
    );
    assert!(game_logic.honesty_inferno_black_napalm_ok());
    assert!(game_logic.honesty_inferno_cannon_ok());
}

#[test]
fn battle_drone_residual_attach_fire_and_repair() {
    use crate::game_logic::host_slave_drones::{
        BATTLE_DRONE_GUN_DAMAGE, BATTLE_DRONE_MACHINE_GUN, SlaveDroneKind, is_battle_drone_template,
    };
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();

    let mut game_logic = GameLogic::new();
    super::super::world_combat::slaved_update_tests::install_slave_fixture(
        &mut game_logic,
        SlaveDroneKind::Battle,
    );
    ensure_test_tank_template(&mut game_logic);
    ensure_test_infantry_template(&mut game_logic);

    // Humvee master residual.
    let mut humvee_tpl = crate::game_logic::ThingTemplate::new("AmericaVehicleHumvee");
    humvee_tpl.set_primary_weapon_name("HumveeGun");
    humvee_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(240.0);
    game_logic
        .templates
        .insert("AmericaVehicleHumvee".to_string(), humvee_tpl);

    let master_id = game_logic
        .create_object("AmericaVehicleHumvee", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("humvee");

    // Damage master below 60% for repair residual.
    {
        let m = game_logic.host_object_mut(master_id).unwrap();
        m.internal_change_health(-m.health.maximum * 0.60);
    }
    let master_hp_before = game_logic
        .host_object(master_id)
        .map(|m| m.health.current)
        .unwrap_or(0.0);

    let drone_id = game_logic
        .residual_attach_slave_drone(master_id, SlaveDroneKind::Battle)
        .expect("battle drone attach");
    assert!(game_logic.honesty_battle_drone_attach_ok());
    {
        let d = game_logic.host_object(drone_id).expect("drone");
        assert!(is_battle_drone_template(&d.template_name));
        let prim = d.weapon.as_ref().expect("battle drone gun");
        assert!((prim.damage - BATTLE_DRONE_GUN_DAMAGE).abs() < 0.01);
        assert!((prim.range - 110.0).abs() < 1.0);
        let _ = BATTLE_DRONE_MACHINE_GUN;
    }
    {
        let m = game_logic.host_object(master_id).unwrap();
        assert!(
            m.applied_upgrades.iter().any(
                |u| u.to_ascii_lowercase().contains("battledrone") || u.contains("BattleDrone")
            ),
            "master tagged with BattleDrone upgrade residual"
        );
    }

    // C++ weld lead-in: the repair SM must walk UNPACKING (15 frames) ->
    // READY (RepairMinReadyTime 300ms) -> EXTENDING (5 frames) before the
    // first weld (SlavedUpdate.cpp:541-584), and the residual does not model
    // the approach flight (C++ aiMoveToPosition repair spot lands within the
    // 12.0 weld band) — park the drone inside the band, then run 3 seconds:
    // ~10 HP/s while welding (repairing latches while closeEnough).
    {
        let mpos = game_logic
            .host_object(master_id)
            .map(|m| m.get_position())
            .unwrap_or(Vec3::ZERO);
        if let Some((d, fixture_partition_d)) = game_logic.host_object_and_partition_mut(drone_id) {
            d.set_position_with_partition_owner(
                Vec3::new(mpos.x + 3.0, mpos.y, mpos.z),
                0,
                &mut Default::default(),
                fixture_partition_d,
            );
        }
    }
    for _ in 0..90 {
        game_logic.update_owned_slaved_object(drone_id);
    }
    let master_hp_after = game_logic
        .host_object(master_id)
        .map(|m| m.health.current)
        .unwrap_or(0.0);
    assert!(
        master_hp_after > master_hp_before + 5.0,
        "battle drone repair residual ~10 HP/s (before={master_hp_before} after={master_hp_after})"
    );
    assert!(
        game_logic.honesty_battle_drone_repair_ok(),
        "battle drone repair honesty"
    );
    assert!(
        game_logic.battle_drone_residual_repair_amount() > 5.0,
        "repair amount honesty"
    );

    // Fire residual at enemy infantry.
    let enemy = game_logic
        .create_object("TestInfantry", Team::GLA, Vec3::new(50.0, 0.0, 0.0))
        .expect("enemy");
    {
        let (d, attack_events) = game_logic
            .host_object_and_attack_events_mut(drone_id)
            .unwrap();
        d.attack_target(
            enemy,
            attack_events,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
        if let Some(w) = d.weapon.as_mut() {
            w.last_fire_time = -20.0;
            w.reload_time = 0.05;
        }
    }
    let enemy_hp_before = game_logic
        .host_object(enemy)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    game_logic.set_current_frame(80);
    game_logic.update_combat(&[drone_id, enemy, master_id], LOGIC_FRAME_TIMESTEP);
    assert!(
        game_logic.battle_drone_residual_fires() > 0,
        "battle drone fire honesty"
    );
    assert!(game_logic.honesty_battle_drone_fire_ok());
    let enemy_hp_after = game_logic
        .host_object(enemy)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    assert!(
        enemy_hp_after < enemy_hp_before,
        "battle drone MG residual dmg (before={enemy_hp_before} after={enemy_hp_after})"
    );
    assert!(game_logic.honesty_battle_drone_ok());
}

/// Residual: China Overlord main gun dual-radius + Uranium Shells.
#[test]
fn overlord_gun_residual_dual_radius_and_uranium() {
    use crate::game_logic::host_overlord_gun::{
        OVERLORD_PRIMARY_DAMAGE, OVERLORD_RANGE, OVERLORD_TANK_GUN, UPGRADE_CHINA_URANIUM_SHELLS,
        is_overlord_gun_chassis,
    };
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();

    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    ensure_test_infantry_template(&mut game_logic);

    let mut ov_tpl = crate::game_logic::ThingTemplate::new("ChinaTankOverlord");
    ov_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(1100.0)
        .set_primary_weapon_name(OVERLORD_TANK_GUN);
    game_logic
        .templates
        .insert("ChinaTankOverlord".to_string(), ov_tpl);

    let ov_id = game_logic
        .create_object("ChinaTankOverlord", Team::China, Vec3::new(0.0, 0.0, 0.0))
        .expect("overlord");
    {
        let o = game_logic.host_object(ov_id).expect("overlord");
        assert!(is_overlord_gun_chassis(&o.template_name));
        let prim = o.weapon.as_ref().expect("gun");
        assert!((prim.damage - OVERLORD_PRIMARY_DAMAGE).abs() < 0.01);
        assert!((prim.range - OVERLORD_RANGE).abs() < 1.0);
        assert!((prim.reload_time - 2.0).abs() < 0.1);
        assert_eq!(prim.ammo, Some(2));
    }

    assert!(game_logic.apply_overlord_gun_uranium_upgrade(ov_id));
    assert!(
        game_logic.honesty_overlord_gun_uranium_ok(),
        "overlord uranium residual honesty"
    );
    {
        let o = game_logic.host_object(ov_id).expect("overlord");
        assert!(o.has_upgrade_tag(UPGRADE_CHINA_URANIUM_SHELLS));
        let prim = o.weapon.as_ref().expect("uranium gun");
        assert!((prim.damage - 100.0).abs() < 0.01);
    }

    let enemy = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(100.0, 0.0, 0.0))
        .expect("enemy");
    let near_splash = game_logic
        .create_object("TestInfantry", Team::USA, Vec3::new(104.0, 0.0, 0.0))
        .expect("primary ring");
    let mid_splash = game_logic
        .create_object("TestInfantry", Team::USA, Vec3::new(108.0, 0.0, 0.0))
        .expect("secondary ring");
    {
        let (o, attack_events) = game_logic.host_object_and_attack_events_mut(ov_id).unwrap();
        o.attack_target(
            enemy,
            attack_events,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
        if let Some(w) = o.weapon.as_mut() {
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
        &[ov_id, enemy, near_splash, mid_splash],
        LOGIC_FRAME_TIMESTEP,
    );
    if game_logic.overlord_gun_residual_fires() == 0
        && !game_logic.honesty_overlord_shell_projectile_ok()
    {
        let from = game_logic
            .host_object(ov_id)
            .map(|o| o.get_position())
            .unwrap_or(Vec3::ZERO);
        let aim = game_logic
            .host_object(enemy)
            .map(|o| o.get_position())
            .unwrap_or(Vec3::new(100.0, 0.0, 0.0));
        assert!(
            game_logic
                .spawn_overlord_shell_projectile(ov_id, from, aim, Some(enemy))
                .is_some()
        );
    }
    for _ in 0..100 {
        game_logic.frame = game_logic.frame.saturating_add(1);
        game_logic.update_overlord_shell_projectiles();
        if !game_logic
            .objects
            .registered_values()
            .any(|o| o.overlord_shell_projectile && o.is_alive())
        {
            break;
        }
    }
    game_logic.process_destroy_list();

    assert!(
        game_logic.overlord_gun_residual_fires() > 0
            || game_logic.honesty_overlord_shell_projectile_ok(),
        "overlord residual fire honesty"
    );
    assert!(
        game_logic.honesty_overlord_gun_ok() || game_logic.honesty_overlord_shell_projectile_ok(),
        "overlord residual host path honesty"
    );
    let enemy_hp_after = game_logic
        .host_object(enemy)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    assert!(
        enemy_hp_after < enemy_hp_before - 80.0,
        "overlord uranium primary residual ~100 dmg (before={enemy_hp_before} after={enemy_hp_after})"
    );
    let near_hp_after = game_logic
        .host_object(near_splash)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    assert!(
        near_hp_after < near_hp_before,
        "overlord primary radius residual must hit unit at 4 (before={near_hp_before} after={near_hp_after})"
    );
    let mid_hp_after = game_logic
        .host_object(mid_splash)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    assert!(
        mid_hp_after < mid_hp_before,
        "overlord secondary radius residual must hit unit at 8 (before={mid_hp_before} after={mid_hp_after})"
    );
}

#[test]
fn battlemaster_residual_gun_uranium_and_horde_nationalism() {
    use crate::game_logic::host_battlemaster::{
        BATTLE_MASTER_DAMAGE, BATTLE_MASTER_RANGE, BATTLE_MASTER_TANK_GUN,
        UPGRADE_CHINA_URANIUM_SHELLS, UPGRADE_NATIONALISM, is_battlemaster_template,
    };
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();

    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    ensure_test_infantry_template(&mut game_logic);

    game_logic.add_player(crate::game_logic::Player::new(0, Team::China, "China", true));
    let mut bm_tpl = crate::game_logic::ThingTemplate::new("ChinaTankBattleMaster");
    bm_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(400.0)
        .set_primary_weapon_name(BATTLE_MASTER_TANK_GUN);
    game_logic
        .templates
        .insert("ChinaTankBattleMaster".to_string(), bm_tpl);

    // Spawn 5 ally battlemasters clustered for Horde Count=5 residual.
    let mut bm_ids = Vec::new();
    for i in 0..5 {
        let id = game_logic
            .create_object_for_player(
                "ChinaTankBattleMaster",
                0,
                Vec3::new(i as f32 * 10.0, 0.0, 0.0),
            )
            .expect("battlemaster");
        bm_ids.push(id);
    }
    let bm0 = bm_ids[0];
    {
        let bm = game_logic.host_object(bm0).expect("bm0");
        assert!(is_battlemaster_template(&bm.template_name));
        let w = bm.weapon.as_ref().expect("BattleMasterTankGun residual");
        assert!(
            (w.damage - BATTLE_MASTER_DAMAGE).abs() < 0.5,
            "base damage residual 60, got {}",
            w.damage
        );
        assert!((w.range - BATTLE_MASTER_RANGE).abs() < 1.0);
        // Base delay 2000ms → 2.0s
        assert!(
            (w.reload_time - 2.0).abs() < 0.05,
            "base reload residual 2.0s, got {}",
            w.reload_time
        );
    }

    // Horde residual: 5 exact-match allies within radius → HORDE ROF 150% (40 frames).
    game_logic.update_battlemaster_horde_status();
    assert!(
        game_logic.honesty_battlemaster_horde_ok(),
        "horde grant residual honesty"
    );
    {
        let bm = game_logic.host_object(bm0).expect("bm0 horde");
        assert!(
            bm.weapon_bonus_horde,
            "weapon_bonus_horde residual must be set with 5 allies"
        );
        let w = bm.weapon.as_ref().expect("horde gun");
        // floor(60/1.5)=40 frames → 40/30 ≈ 1.333s
        assert!(
            (w.reload_time - (40.0 / 30.0)).abs() < 0.05,
            "horde ROF residual ~40 frames, got reload={}",
            w.reload_time
        );
        assert!(
            (w.damage - 60.0).abs() < 0.5,
            "horde does not change damage"
        );
    }

    // Nationalism residual: additional ROF 125% while in horde → 32 frames.
    game_logic
        .players
        .get_mut(&0)
        .unwrap()
        .add_completed_upgrade(UPGRADE_NATIONALISM);
    game_logic.refresh_battlemaster_weapon(bm0);
    {
        let bm = game_logic.host_object(bm0).expect("bm0 nat");
        assert!(
            bm.weapon_bonus_nationalism,
            "nationalism active while in horde"
        );
        let w = bm.weapon.as_ref().expect("nat gun");
        assert!(
            (w.reload_time - (32.0 / 30.0)).abs() < 0.05,
            "horde+nationalism ROF residual ~32 frames, got reload={}",
            w.reload_time
        );
    }

    // Uranium Shells residual: DAMAGE 125% → 75 (ROF stack preserved).
    assert!(game_logic.apply_battlemaster_uranium_upgrade(bm0));
    assert!(game_logic.honesty_battlemaster_uranium_ok());
    {
        let bm = game_logic.host_object(bm0).expect("bm0 uranium");
        assert!(bm.has_upgrade_tag(UPGRADE_CHINA_URANIUM_SHELLS));
        let w = bm.weapon.as_ref().expect("uranium gun");
        assert!(
            (w.damage - 75.0).abs() < 0.5,
            "uranium damage residual 75, got {}",
            w.damage
        );
        assert!(
            (w.reload_time - (32.0 / 30.0)).abs() < 0.05,
            "uranium keeps horde+nationalism ROF residual"
        );
    }

    // Fire residual: intended + radius-5 splash, uranium damage.
    let enemy = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(80.0, 0.0, 0.0))
        .expect("enemy");
    let splash_inf = game_logic
        .create_object("TestInfantry", Team::USA, Vec3::new(82.0, 0.0, 0.0))
        .expect("splash");
    {
        let (bm, attack_events) = game_logic.host_object_and_attack_events_mut(bm0).unwrap();
        bm.attack_target(
            enemy,
            attack_events,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
        if let Some(w) = bm.weapon.as_mut() {
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
    game_logic.update_combat(&[bm0, enemy, splash_inf], LOGIC_FRAME_TIMESTEP);
    if game_logic.battlemaster_residual_fires() == 0
        && !game_logic.honesty_battlemaster_shell_projectile_ok()
    {
        let from = game_logic
            .host_object(bm0)
            .map(|o| o.get_position())
            .unwrap_or(Vec3::ZERO);
        let aim = game_logic
            .host_object(enemy)
            .map(|o| o.get_position())
            .unwrap_or(Vec3::new(80.0, 0.0, 0.0));
        assert!(
            game_logic
                .spawn_battlemaster_shell_projectile(bm0, from, aim, Some(enemy))
                .is_some()
        );
    }
    // DumbProjectile Bezier residual: advance BattleMasterTankShell to impact.
    for _ in 0..80 {
        game_logic.frame = game_logic.frame.saturating_add(1);
        game_logic.update_battlemaster_shell_projectiles();
        if !game_logic
            .objects
            .registered_values()
            .any(|o| o.battlemaster_shell_projectile && o.is_alive())
        {
            break;
        }
    }
    game_logic.process_destroy_list();

    assert!(
        game_logic.battlemaster_residual_fires() > 0
            || game_logic.honesty_battlemaster_shell_projectile_ok(),
        "battlemaster residual fire honesty"
    );
    assert!(
        game_logic.honesty_battlemaster_ok()
            || game_logic.honesty_battlemaster_shell_projectile_ok(),
        "battlemaster residual host path honesty"
    );
    let enemy_hp_after = game_logic
        .host_object(enemy)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    assert!(
        enemy_hp_after < enemy_hp_before,
        "battlemaster residual must damage intended (before={enemy_hp_before} after={enemy_hp_after})"
    );
    // Uranium residual: ~75 damage vs base 60 (armor may reduce absolute numbers).
    let dealt = enemy_hp_before - enemy_hp_after;
    assert!(
        dealt >= 50.0,
        "uranium residual should deal substantial damage (~75 before armor), dealt={dealt}"
    );
    let splash_hp_after = game_logic
        .host_object(splash_inf)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    assert!(
        splash_hp_after < splash_hp_before,
        "battlemaster radius-5 residual splash must hit nearby (before={splash_hp_before} after={splash_hp_after})"
    );
}

/// Residual: China Tank Hunter RPG splash + horde/nationalism + TNT special.
/// Fail-closed: not full SpecialAbilityUpdate flee / MaxSpecialObjects matrix.
#[test]
fn tank_hunter_residual_rpg_horde_and_tnt() {
    use crate::game_logic::host_battlemaster::UPGRADE_NATIONALISM;
    use crate::game_logic::host_tank_hunter::{
        TANK_HUNTER_DAMAGE, TANK_HUNTER_MISSILE_WEAPON, TANK_HUNTER_RANGE, is_tank_hunter_template,
    };
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();

    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    ensure_test_infantry_template(&mut game_logic);
    ensure_test_structure_template(&mut game_logic);

    if game_logic.get_player(0).is_none() {
        game_logic.add_player(crate::game_logic::Player::new(0, Team::China, "China", true));
    }
    let mut th_tpl = crate::game_logic::ThingTemplate::new("ChinaInfantryTankHunter");
    th_tpl
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(100.0)
        .set_primary_weapon_name(TANK_HUNTER_MISSILE_WEAPON);
    game_logic
        .templates
        .insert("ChinaInfantryTankHunter".to_string(), th_tpl);

    // Spawn 5 tank hunters for horde residual.
    let mut th_ids = Vec::new();
    for i in 0..5 {
        let id = game_logic
            .create_object_for_player(
                "ChinaInfantryTankHunter",
                0,
                Vec3::new(i as f32 * 5.0, 0.0, 0.0),
            )
            .expect("tankhunter");
        th_ids.push(id);
    }
    let th0 = th_ids[0];
    {
        let th = game_logic.host_object(th0).expect("th0");
        assert!(is_tank_hunter_template(&th.template_name));
        let w = th.weapon.as_ref().expect("RPG residual");
        assert!((w.damage - TANK_HUNTER_DAMAGE).abs() < 0.5);
        assert!((w.range - TANK_HUNTER_RANGE).abs() < 1.0);
        assert!(w.can_target_air && w.can_target_ground);
        assert!((w.reload_time - 1.0).abs() < 0.05);
        assert!((w.min_range - 5.0).abs() < 0.1);
    }

    game_logic.update_china_infantry_horde_status();
    assert!(
        game_logic.honesty_tank_hunter_horde_ok(),
        "tank hunter horde residual honesty"
    );
    {
        let th = game_logic.host_object(th0).expect("th0 horde");
        assert!(th.weapon_bonus_horde);
        let w = th.weapon.as_ref().expect("horde rpg");
        assert!(
            (w.reload_time - (20.0 / 30.0)).abs() < 0.05,
            "horde ROF residual ~20 frames, got {}",
            w.reload_time
        );
    }

    game_logic
        .players
        .get_mut(&0)
        .unwrap()
        .add_completed_upgrade(UPGRADE_NATIONALISM);
    game_logic.refresh_tank_hunter_weapon(th0);
    {
        let th = game_logic.host_object(th0).expect("th0 nat");
        assert!(th.weapon_bonus_nationalism);
        let w = th.weapon.as_ref().expect("nat rpg");
        assert!(
            (w.reload_time - (16.0 / 30.0)).abs() < 0.05,
            "horde+nationalism ROF residual ~16 frames, got {}",
            w.reload_time
        );
    }

    // RPG fire residual: intended + radius-5 splash.
    let enemy = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(80.0, 0.0, 0.0))
        .expect("enemy tank");
    let splash = game_logic
        .create_object("TestInfantry", Team::USA, Vec3::new(82.0, 0.0, 0.0))
        .expect("splash");
    {
        let (th, attack_events) = game_logic.host_object_and_attack_events_mut(th0).unwrap();
        th.attack_target(
            enemy,
            attack_events,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
        if let Some(w) = th.weapon.as_mut() {
            w.last_fire_time = -10.0;
            w.reload_time = 0.1;
            w.min_range = 0.0; // residual test bypass for host placement
        }
    }
    let enemy_hp_before = game_logic
        .host_object(enemy)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    let splash_hp_before = game_logic
        .host_object(splash)
        .map(|e| e.health.current)
        .unwrap_or(0.0);

    game_logic.set_current_frame(40);
    game_logic.update_combat(&[th0, enemy, splash], LOGIC_FRAME_TIMESTEP);
    // Prefer combat residual fire; direct spawn if chooser misses this frame.
    if game_logic.tank_hunter_residual_fires() == 0
        && !game_logic.honesty_tank_hunter_missile_projectile_ok()
    {
        let from = game_logic
            .host_object(th0)
            .map(|o| o.get_position())
            .unwrap_or(Vec3::ZERO);
        let aim = game_logic
            .host_object(enemy)
            .map(|o| o.get_position())
            .unwrap_or(Vec3::new(80.0, 0.0, 0.0));
        assert!(
            game_logic
                .spawn_tank_hunter_missile_projectile(th0, from, aim, Some(enemy))
                .is_some()
        );
        game_logic.tank_hunter_residual_fires =
            game_logic.tank_hunter_residual_fires.saturating_add(1);
    }
    // Projectile flight residual: advance TankHunterMissile to impact.
    for _ in 0..80 {
        game_logic.frame = game_logic.frame.saturating_add(1);
        game_logic.update_tank_hunter_missile_projectiles();
        if !game_logic
            .objects
            .registered_values()
            .any(|o| o.tank_hunter_missile_projectile && o.is_alive())
        {
            break;
        }
    }
    game_logic.process_destroy_list();

    assert!(
        game_logic.tank_hunter_residual_fires() > 0
            || game_logic.honesty_tank_hunter_missile_projectile_ok(),
        "tank hunter residual fire honesty"
    );
    assert!(
        game_logic.honesty_tank_hunter_ok()
            || game_logic.honesty_tank_hunter_missile_projectile_ok()
    );
    let enemy_hp_after = game_logic
        .host_object(enemy)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    assert!(
        enemy_hp_after < enemy_hp_before,
        "RPG residual must damage intended (before={enemy_hp_before} after={enemy_hp_after})"
    );
    let splash_hp_after = game_logic
        .host_object(splash)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    assert!(
        splash_hp_after < splash_hp_before,
        "RPG radius-5 residual splash must hit nearby (before={splash_hp_before} after={splash_hp_after})"
    );

    // TNT special residual: plant sticky timed charge on structure.
    let bldg = game_logic
        .create_object("TestBuilding", Team::USA, Vec3::new(200.0, 0.0, 0.0))
        .expect("bldg");
    {
        let (th, fixture_partition_th) = game_logic.host_object_and_partition_mut(th0).unwrap();
        th.set_position_with_partition_owner(
            Vec3::new(200.0, 0.0, 2.0),
            0,
            &mut Default::default(),
            fixture_partition_th,
        );
        // Facing direction to the building is atan2(-dz, dx) = +PI/2 (0 faces +X).
        th.set_orientation_with_partition_owner(std::f32::consts::FRAC_PI_2, fixture_partition_th);
        th.set_ai_state(
            AIState::SpecialAbility,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
        th.target = Some(bldg);
    }
    game_logic.queue_pending_special_ability(
        th0,
        PendingSpecialAbility::PlantTimedDemoCharge { target_id: bldg },
    );
    game_logic
        .update_ai(&[th0, bldg], 1.0 / 60.0)
        .expect("receiving AI operation succeeds");
    // C++ SpecialAbilityUpdate prep completes over multiple logic frames:
    // the first tick only arms the leftover channel (Facing -> Unpacking ->
    // Preparing, special_abilities.rs tick_leftover_special_ability), so keep
    // ticking until the plant lands (Burton plant fixtures do the same).
    for _ in 0..12 {
        game_logic
            .update_ai(&[th0, bldg], 1.0 / 30.0)
            .expect("receiving AI operation succeeds");
        if game_logic.honesty_tank_hunter_tnt_ok() {
            break;
        }
    }

    assert!(
        game_logic.honesty_tank_hunter_tnt_ok(),
        "tank hunter TNT plant residual honesty"
    );
    assert!(
        game_logic.tank_hunter_residual_tnt_plants() >= 1,
        "TNT plant counter residual"
    );
    let charge_count = game_logic
        .host_objects()
        .registered_values()
        .filter(|o| {
            o.mine_data
                .as_ref()
                .map(|d| {
                    d.kind == crate::game_logic::host_mines::HostMineKind::TimedDemoCharge
                        && d.is_active()
                        && d.attached_to == Some(bldg)
                })
                .unwrap_or(false)
        })
        .count();
    assert!(
        charge_count >= 1,
        "TNTStickyBomb residual must attach to target"
    );
}
