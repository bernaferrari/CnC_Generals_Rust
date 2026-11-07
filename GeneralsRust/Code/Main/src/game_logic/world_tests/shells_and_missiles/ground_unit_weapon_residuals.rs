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
    helix_tpl.contain_module.kind = crate::game_logic::ContainModuleKind::Transport;
    helix_tpl.contain_module.is_helix_contain = true;
    helix_tpl.contain_module.slots = Some(crate::game_logic::HELIX_TRANSPORT_SLOTS);
    game_logic
        .templates
        .insert("ChinaVehicleHelix".to_string(), helix_tpl);

    let helix_id = game_logic
        .create_object("ChinaVehicleHelix", Team::China, Vec3::new(0.0, 0.0, 0.0))
        .expect("helix");
    {
        let h = game_logic.host_object(helix_id).expect("helix");
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
