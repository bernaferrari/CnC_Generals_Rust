//! Host GameLogic tests — authored weapon set bindings.
#![allow(unused_imports, non_snake_case, unused_variables, dead_code)]
use super::*;

/// hq-2qedt.143: GLA Technical PRIMARY binds from the authored WeaponSet at
/// create (C++ WeaponSet::updateWeaponSet), not a template-name peel.
#[test]
fn technical_create_primary_comes_from_authored_weapon_set() {
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let mut game_logic = GameLogic::new();
    let mut tpl = crate::game_logic::ThingTemplate::new("GLAVehicleTechnical");
    tpl.add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(200.0)
        .set_primary_weapon_name("RangerFlashBangGrenadeWeapon");
    game_logic
        .templates
        .insert("GLAVehicleTechnical".to_string(), tpl);
    let id = game_logic
        .create_object("GLAVehicleTechnical", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("technical");
    let prim = game_logic
        .host_object(id)
        .and_then(|t| t.weapon.clone())
        .expect("authored primary");
    assert!(
        (prim.damage - crate::game_logic::host_technical::TECH_MG_DAMAGE).abs() > 0.01,
        "authored PRIMARY overwritten by technical name peel: {prim:?}"
    );
}

/// hq-2qedt.144: China Battlemaster PRIMARY binds from the authored WeaponSet at
/// create (C++ WeaponSet::updateWeaponSet), not a template-name peel.
#[test]
fn battlemaster_create_primary_comes_from_authored_weapon_set() {
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let mut game_logic = GameLogic::new();
    let mut tpl = crate::game_logic::ThingTemplate::new("ChinaTankBattleMaster");
    tpl.add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(200.0)
        .set_primary_weapon_name("RangerFlashBangGrenadeWeapon");
    game_logic
        .templates
        .insert("ChinaTankBattleMaster".to_string(), tpl);
    let id = game_logic
        .create_object(
            "ChinaTankBattleMaster",
            Team::China,
            Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("battlemaster");
    let prim = game_logic
        .host_object(id)
        .and_then(|t| t.weapon.clone())
        .expect("authored primary");
    assert!(
        (prim.damage - crate::game_logic::host_battlemaster::BATTLE_MASTER_DAMAGE).abs() > 0.01,
        "authored PRIMARY overwritten by battlemaster name peel: {prim:?}"
    );
}

/// hq-2qedt.298: Uranium Shells / Nationalism refresh recipients are the
/// objects whose authored PRIMARY is BattleMasterTankGun (C++ WeaponSet
/// upgrade bonuses ride the authored weapon), never the chassis name.
#[test]
fn battlemaster_refresh_recipient_keys_on_authored_primary_not_name() {
    use crate::game_logic::host_battlemaster::{
        BATTLE_MASTER_TANK_GUN, UPGRADE_CHINA_URANIUM_SHELLS,
    };
    let mut game_logic = GameLogic::new();
    for (name, primary) in [
        ("ArbitraryAuthoredGunTank", BATTLE_MASTER_TANK_GUN),
        ("ChinaTankBattleMaster", "RangerFlashBangGrenadeWeapon"),
    ] {
        let mut tpl = crate::game_logic::ThingTemplate::new(name);
        tpl.add_kind_of(KindOf::Vehicle)
            .set_health(400.0)
            .set_primary_weapon_name(primary);
        game_logic.templates.insert(name.to_string(), tpl);
    }
    let authored = game_logic
        .create_object("ArbitraryAuthoredGunTank", Team::China, Vec3::ZERO)
        .expect("authored gun tank");
    let named = game_logic
        .create_object("ChinaTankBattleMaster", Team::China, Vec3::ZERO)
        .expect("name-only tank");
    assert!(game_logic.apply_battlemaster_uranium_upgrade(authored));
    assert!(!game_logic.apply_battlemaster_uranium_upgrade(named));
    let has_uranium = |id| {
        game_logic
            .host_object(id)
            .unwrap()
            .applied_upgrades
            .contains(UPGRADE_CHINA_URANIUM_SHELLS)
    };
    assert!(has_uranium(authored));
    assert!(!has_uranium(named));
}

#[test]
fn overlord_uranium_recipient_uses_authored_primary_without_rebinding() {
    use crate::game_logic::host_overlord_gun::{OVERLORD_TANK_GUN, TANK_OVERLORD_TANK_GUN};
    let mut logic = GameLogic::new();
    for (name, gun) in [
        ("CustomGunCarrier", OVERLORD_TANK_GUN),
        ("CustomGeneralCarrier", TANK_OVERLORD_TANK_GUN),
        ("ChinaTankOverlord", "RangerFlashBangGrenadeWeapon"),
    ] {
        let mut template = crate::game_logic::ThingTemplate::new(name);
        template.add_kind_of(KindOf::Vehicle).set_health(400.0).set_primary_weapon_name(gun);
        logic.templates.insert(name.to_owned(), template);
    }
    for name in ["CustomGunCarrier", "CustomGeneralCarrier"] {
        let id = logic.create_object(name, Team::China, Vec3::ZERO).unwrap();
        let mut custom = crate::game_logic::Weapon::default();
        custom.damage = 37.0;
        logic.host_object_mut(id).unwrap().weapon = Some(custom);
        assert!(logic.apply_overlord_gun_uranium_upgrade(id));
        assert_eq!(logic.host_object(id).unwrap().weapon.as_ref().unwrap().damage, 37.0);
    }
    let named = logic.create_object("ChinaTankOverlord", Team::China, Vec3::ZERO).unwrap();
    assert!(!logic.apply_overlord_gun_uranium_upgrade(named));
}

/// hq-2qedt.145: GLA Marauder PRIMARY binds from the authored WeaponSet at
/// create (C++ WeaponSet::updateWeaponSet), not a template-name peel.
#[test]
fn marauder_create_primary_comes_from_authored_weapon_set() {
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let mut game_logic = GameLogic::new();
    let mut tpl = crate::game_logic::ThingTemplate::new("GLATankMarauder");
    tpl.add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(200.0)
        .set_primary_weapon_name("RangerFlashBangGrenadeWeapon");
    game_logic
        .templates
        .insert("GLATankMarauder".to_string(), tpl);
    let id = game_logic
        .create_object("GLATankMarauder", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("marauder");
    let prim = game_logic
        .host_object(id)
        .and_then(|t| t.weapon.clone())
        .expect("authored primary");
    assert!(
        (prim.damage - crate::game_logic::host_marauder::MARAUDER_DAMAGE).abs() > 0.01,
        "authored PRIMARY overwritten by marauder name peel: {prim:?}"
    );
}

/// hq-2qedt.146: USA Missile Defender PRIMARY binds from the authored WeaponSet at
/// create (C++ WeaponSet::updateWeaponSet), not a template-name peel.
#[test]
fn missile_defender_create_primary_comes_from_authored_weapon_set() {
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let mut game_logic = GameLogic::new();
    let mut tpl = crate::game_logic::ThingTemplate::new("AmericaInfantryMissileDefender");
    tpl.add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(200.0)
        .set_primary_weapon_name("RangerFlashBangGrenadeWeapon");
    game_logic
        .templates
        .insert("AmericaInfantryMissileDefender".to_string(), tpl);
    let id = game_logic
        .create_object(
            "AmericaInfantryMissileDefender",
            Team::USA,
            Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("missile_defender");
    let prim = game_logic
        .host_object(id)
        .and_then(|t| t.weapon.clone())
        .expect("authored primary");
    assert!(
        (prim.damage - crate::game_logic::host_missile_defender::MISSILE_DEFENDER_DAMAGE).abs()
            > 0.01,
        "authored PRIMARY overwritten by missile_defender name peel: {prim:?}"
    );
}

/// hq-2qedt.147: GLA Tunnel Network PRIMARY binds from the authored WeaponSet at
/// create (C++ WeaponSet::updateWeaponSet), not a template-name peel.
#[test]
fn tunnel_network_create_primary_comes_from_authored_weapon_set() {
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let mut game_logic = GameLogic::new();
    let mut tpl = crate::game_logic::ThingTemplate::new("GLATunnelNetwork");
    // Retail GLATunnelNetwork authors Body/Contain = TunnelContain.
    tpl.contain_module.kind = crate::game_logic::ContainModuleKind::Tunnel;
    tpl.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(200.0)
        .set_primary_weapon_name("RangerFlashBangGrenadeWeapon");
    game_logic
        .templates
        .insert("GLATunnelNetwork".to_string(), tpl);
    let id = game_logic
        .create_object("GLATunnelNetwork", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("tunnel_network");
    let prim = game_logic
        .host_object(id)
        .and_then(|t| t.weapon.clone())
        .expect("authored primary");
    assert!(
        (prim.damage - crate::game_logic::host_tunnel_network::TUNNEL_NETWORK_GUN_DAMAGE).abs()
            > 0.01,
        "authored PRIMARY overwritten by tunnel_network name peel: {prim:?}"
    );
}

/// hq-2qedt.148: GLA Combat Cycle (default Rebel rider) PRIMARY binds from the authored WeaponSet at
/// create (C++ WeaponSet::updateWeaponSet), not a template-name peel.
#[test]
fn combat_cycle_create_primary_comes_from_authored_weapon_set() {
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let mut game_logic = GameLogic::new();
    let mut tpl = crate::game_logic::ThingTemplate::new("GLAVehicleCombatBike");
    tpl.add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(200.0)
        .set_primary_weapon_name("RangerFlashBangGrenadeWeapon");
    game_logic
        .templates
        .insert("GLAVehicleCombatBike".to_string(), tpl);
    let id = game_logic
        .create_object("GLAVehicleCombatBike", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("combat_cycle");
    let prim = game_logic
        .host_object(id)
        .and_then(|t| t.weapon.clone())
        .expect("authored primary");
    assert!(
        (prim.damage - crate::game_logic::host_combat_cycle::REBEL_MG_DAMAGE).abs() > 0.01,
        "authored PRIMARY overwritten by combat_cycle name peel: {prim:?}"
    );
}

/// hq-2qedt.149: GLA RPG Trooper PRIMARY binds from the authored WeaponSet at
/// create (C++ WeaponSet::updateWeaponSet), not a template-name peel.
#[test]
fn rpg_trooper_create_primary_comes_from_authored_weapon_set() {
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let mut game_logic = GameLogic::new();
    let mut tpl = crate::game_logic::ThingTemplate::new("GLAInfantryTunnelDefender");
    tpl.add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(200.0)
        .set_primary_weapon_name("RangerFlashBangGrenadeWeapon");
    game_logic
        .templates
        .insert("GLAInfantryTunnelDefender".to_string(), tpl);
    let id = game_logic
        .create_object(
            "GLAInfantryTunnelDefender",
            Team::GLA,
            Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("rpg_trooper");
    let prim = game_logic
        .host_object(id)
        .and_then(|t| t.weapon.clone())
        .expect("authored primary");
    assert!(
        (prim.damage - crate::game_logic::host_rpg_trooper::RPG_TROOPER_DAMAGE).abs() > 0.01,
        "authored PRIMARY overwritten by rpg_trooper name peel: {prim:?}"
    );
}

/// hq-2qedt.213: object_hit routes FireBase residual by the authored weapon,
/// not a template-name gate; a FireBase-named unit with a foreign authored weapon
/// takes the generic path.
#[test]
fn fire_base_object_hit_residual_keys_on_authored_weapon_not_template_name() {
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let mut game_logic = GameLogic::new();
    let mut tpl = crate::game_logic::ThingTemplate::new("TestFireBase");
    tpl.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(200.0)
        .set_primary_weapon_name("RangerFlashBangGrenadeWeapon");
    game_logic.templates.insert("TestFireBase".to_string(), tpl);
    let mut victim = crate::game_logic::ThingTemplate::new("A17xVictim");
    victim
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(1000.0);
    game_logic
        .templates
        .insert("A17xVictim".to_string(), victim);
    let id = game_logic
        .create_object("TestFireBase", Team::China, Vec3::new(0.0, 0.0, 0.0))
        .expect("attacker");
    let enemy = game_logic
        .create_object("A17xVictim", Team::USA, Vec3::new(40.0, 0.0, 0.0))
        .expect("victim");
    {
        let (t, attack_events) = game_logic.host_object_and_attack_events_mut(id).unwrap();
        t.attack_target(
            enemy,
            0,
            attack_events,
            &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
        );
        if let Some(w) = t.weapon.as_mut() {
            w.last_fire_time = -10.0;
            w.min_range = 0.0;
        }
    }
    for f in 40..70 {
        game_logic.set_current_frame(f);
        game_logic.update_combat(&[id, enemy], LOGIC_FRAME_TIMESTEP);
    }
    assert_eq!(
        game_logic.fire_base_shells_spawned + game_logic.fire_base_residual_fires,
        0,
        "FireBase template-name residual fired for a foreign authored weapon"
    );
    let hp = game_logic
        .host_object(enemy)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    assert!(hp < 1000.0, "attacker never hit the victim");
}

/// hq-2qedt.225: Stealth Fighter jet lockon keys on the authored
/// StealthJetMissileWeapon, not the template name. (Innate stealth comes from
/// the authored StealthUpdate module; see stealth_update_tests.)
#[test]
fn stealth_fighter_lockon_keys_on_authored_weapon_not_template_name() {
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let mut game_logic = GameLogic::new();
    for (name, weapon) in [
        ("TestStealthFighter", "RangerFlashBangGrenadeWeapon"),
        ("A225xAuthoredJet", "StealthJetMissileWeapon"),
    ] {
        let mut tpl = crate::game_logic::ThingTemplate::new(name);
        tpl.add_kind_of(KindOf::Aircraft)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_health(200.0)
            .set_primary_weapon_name(weapon);
        game_logic.templates.insert(name.to_string(), tpl);
    }
    let foreign = game_logic
        .create_object("TestStealthFighter", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("foreign");
    let authored = game_logic
        .create_object("A225xAuthoredJet", Team::USA, Vec3::new(50.0, 0.0, 0.0))
        .expect("authored");
    game_logic.set_current_frame(5);
    game_logic.update_stealth_and_detection();
    let f = game_logic.host_object(foreign).unwrap();
    assert_eq!(f.jet_lockon_time_frames(), 0, "name-only lockon residual");
    let a = game_logic.host_object(authored).unwrap();
    assert_ne!(a.jet_lockon_time_frames(), 0, "authored weapon lockon");
}

/// hq-2qedt.266: Gattling Cannon structure factory dual-slot residual keys on
/// the authored GattlingBuildingGun, not the template name.
#[test]
fn gattling_cannon_factory_init_keys_on_authored_weapon_not_template_name() {
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let mut game_logic = GameLogic::new();
    for (name, weapon) in [
        ("TestGattlingCannon266", "RangerFlashBangGrenadeWeapon"),
        ("A266xAuthoredCannon", "GattlingBuildingGun"),
    ] {
        let mut tpl = crate::game_logic::ThingTemplate::new(name);
        tpl.add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_health(1000.0)
            .set_primary_weapon_name(weapon);
        game_logic.templates.insert(name.to_string(), tpl);
    }
    let foreign = game_logic
        .create_object(
            "TestGattlingCannon266",
            Team::China,
            Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("foreign");
    let authored = game_logic
        .create_object(
            "A266xAuthoredCannon",
            Team::China,
            Vec3::new(80.0, 0.0, 0.0),
        )
        .expect("authored");
    let f = game_logic.host_object(foreign).unwrap();
    assert!(
        f.secondary_weapon.is_none(),
        "name-only gattling AA residual"
    );
    let a = game_logic.host_object(authored).unwrap();
    assert!(
        a.secondary_weapon.is_some(),
        "authored gattling AA residual"
    );
}

/// hq-2qedt.102: C++ binds PRIMARY from the template WeaponSet at create,
/// not from a template-name host table. A Tomahawk-named template authoring a
/// different PRIMARY must keep that authored weapon.
#[test]
fn tomahawk_create_primary_comes_from_authored_weapon_set() {
    use crate::game_logic::host_tomahawk::TOMAHAWK_PRIMARY_DAMAGE;
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let mut game_logic = GameLogic::new();
    let mut tpl = crate::game_logic::ThingTemplate::new("AmericaVehicleTomahawk");
    tpl.add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(180.0)
        .set_primary_weapon_name("HumveeGun");
    game_logic
        .templates
        .insert("AmericaVehicleTomahawk".to_string(), tpl);
    let id = game_logic
        .create_object(
            "AmericaVehicleTomahawk",
            Team::USA,
            Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("tomahawk");
    let prim = game_logic
        .host_object(id)
        .and_then(|t| t.weapon.clone())
        .expect("authored primary");
    assert!(
        (prim.damage - TOMAHAWK_PRIMARY_DAMAGE).abs() > 0.01,
        "authored HumveeGun PRIMARY overwritten by tomahawk_weapon() name peel: {prim:?}"
    );
}

/// hq-2qedt.104: Crusader/Paladin (incl. Lazr_) PRIMARY binds from the
/// authored WeaponSet at create (C++ WeaponSet::updateWeaponSet), not the
/// usa_tanks template-name peel.
#[test]
fn crusader_paladin_create_primary_comes_from_authored_weapon_set() {
    use crate::game_logic::host_usa_tanks::{
        LAZR_CRUSADER_TANK_GUN_DAMAGE, LAZR_PALADIN_TANK_GUN_DAMAGE, USA_TANK_GUN_DAMAGE,
    };
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    for name in [
        "AmericaTankCrusader",
        "AmericaTankPaladin",
        "Lazr_AmericaTankCrusader",
        "Lazr_AmericaTankPaladin",
    ] {
        let mut game_logic = GameLogic::new();
        let mut tpl = crate::game_logic::ThingTemplate::new(name);
        tpl.add_kind_of(KindOf::Vehicle)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_health(400.0)
            .set_primary_weapon_name("HumveeGun");
        game_logic.templates.insert(name.to_string(), tpl);
        let id = game_logic
            .create_object(name, Team::USA, Vec3::new(0.0, 0.0, 0.0))
            .expect("tank");
        let prim = game_logic
            .host_object(id)
            .and_then(|t| t.weapon.clone())
            .expect("authored primary");
        for d in [
            USA_TANK_GUN_DAMAGE,
            LAZR_CRUSADER_TANK_GUN_DAMAGE,
            LAZR_PALADIN_TANK_GUN_DAMAGE,
        ] {
            assert!(
                (prim.damage - d).abs() > 0.01,
                "{name}: authored HumveeGun PRIMARY overwritten by usa_tanks name peel: {prim:?}"
            );
        }
    }
}

/// hq-2qedt.122: C++ fires the authored WeaponSet weapon; a Tomahawk-named
/// template authoring a non-Tomahawk PRIMARY must not route through the
/// Tomahawk residual missile path on object hit.
#[test]
fn tomahawk_named_non_tomahawk_weapon_skips_tomahawk_object_hit_residual() {
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    let mut tpl = crate::game_logic::ThingTemplate::new("AmericaVehicleTomahawk");
    tpl.add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(180.0)
        .set_primary_weapon_name("HumveeGun");
    game_logic
        .templates
        .insert("AmericaVehicleTomahawk".to_string(), tpl);
    let tom_id = game_logic
        .create_object(
            "AmericaVehicleTomahawk",
            Team::USA,
            Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("tomahawk");
    let enemy = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(40.0, 0.0, 0.0))
        .expect("enemy");
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
    let hp_before = game_logic
        .host_object(enemy)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    for f in 50..60 {
        game_logic.set_current_frame(f);
        step_authored_combat(&mut game_logic, &[tom_id, enemy]);
    }
    let hp_after = game_logic
        .host_object(enemy)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    assert!(
        hp_after < hp_before,
        "authored HumveeGun must land (before={hp_before} after={hp_after})"
    );
}

/// hq-2qedt.105: Fire Base (incl. AirF_/SupW_/Lazr_) PRIMARY binds from the
/// authored WeaponSet at create (C++ WeaponSet::updateWeaponSet), not the
/// is_fire_base_template name peel.
#[test]
fn fire_base_create_primary_comes_from_authored_weapon_set() {
    use crate::game_logic::host_fire_base::FIRE_BASE_DAMAGE;
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    for name in [
        "AmericaFireBase",
        "AirF_AmericaFireBase",
        "SupW_AmericaFireBase",
        "Lazr_AmericaFireBase",
    ] {
        let mut game_logic = GameLogic::new();
        let mut tpl = crate::game_logic::ThingTemplate::new(name);
        tpl.add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_health(1000.0)
            .set_primary_weapon_name("HumveeGun");
        game_logic.templates.insert(name.to_string(), tpl);
        let id = game_logic
            .create_object(name, Team::USA, Vec3::new(0.0, 0.0, 0.0))
            .expect("firebase");
        let prim = game_logic
            .host_object(id)
            .and_then(|t| t.weapon.clone())
            .expect("authored primary");
        assert!(
            (prim.damage - FIRE_BASE_DAMAGE).abs() > 0.01,
            "{name}: authored HumveeGun PRIMARY overwritten by fire_base_weapon() name peel: {prim:?}"
        );
    }
}

/// hq-2qedt.110: C++ binds PRIMARY from the template WeaponSet at create
/// (WeaponSet::updateWeaponSet), not a template-name host table. A
/// Stealth-Fighter-named template authoring a different PRIMARY must keep it.
#[test]
fn stealth_fighter_create_primary_comes_from_authored_weapon_set() {
    use crate::game_logic::host_stealth_fighter::STEALTH_FIGHTER_DAMAGE;
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let mut game_logic = GameLogic::new();
    let mut tpl = crate::game_logic::ThingTemplate::new("AmericaJetStealthFighter");
    tpl.add_kind_of(KindOf::Aircraft)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(150.0)
        .set_primary_weapon_name("HumveeGun");
    game_logic
        .templates
        .insert("AmericaJetStealthFighter".to_string(), tpl);
    let id = game_logic
        .create_object(
            "AmericaJetStealthFighter",
            Team::USA,
            Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("stealth fighter");
    let obj = game_logic.host_object(id).expect("stealth fighter");
    let prim = obj.weapon.clone().expect("authored primary");
    assert!(
        (prim.damage - STEALTH_FIGHTER_DAMAGE).abs() > 0.01,
        "authored HumveeGun PRIMARY overwritten by stealth_fighter_weapon() name peel: {prim:?}"
    );
}

/// hq-2qedt.109: C++ binds PRIMARY/SECONDARY from the template WeaponSet at
/// create (WeaponSet::updateWeaponSet), not a template-name host table. A
/// Comanche-named template authoring different weapons must keep them.
#[test]
fn comanche_create_weapons_come_from_authored_weapon_set() {
    use crate::game_logic::host_comanche_rocket_pods::{
        COMANCHE_AT_PRIMARY_DAMAGE, COMANCHE_CANNON_DAMAGE,
    };
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let mut game_logic = GameLogic::new();
    let mut tpl = crate::game_logic::ThingTemplate::new("AmericaVehicleComanche");
    tpl.add_kind_of(KindOf::Aircraft)
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(220.0)
        .set_primary_weapon_name("HumveeGun")
        .set_secondary_weapon_name("HumveeGun");
    game_logic
        .templates
        .insert("AmericaVehicleComanche".to_string(), tpl);
    let id = game_logic
        .create_object(
            "AmericaVehicleComanche",
            Team::USA,
            Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("comanche");
    let obj = game_logic.host_object(id).expect("comanche");
    let prim = obj.weapon.clone().expect("authored primary");
    assert!(
        (prim.damage - COMANCHE_CANNON_DAMAGE).abs() > 0.01,
        "authored HumveeGun PRIMARY overwritten by comanche_cannon_weapon() name peel: {prim:?}"
    );
    let sec = obj.secondary_weapon.clone().expect("authored secondary");
    assert!(
        (sec.damage - COMANCHE_AT_PRIMARY_DAMAGE).abs() > 0.01,
        "authored HumveeGun SECONDARY overwritten by comanche_antitank_weapon() name peel: {sec:?}"
    );
}

/// hq-2qedt.130: Battle Drone PRIMARY binds from the authored WeaponSet at
/// create (C++ WeaponSet::updateWeaponSet), not a template-name peel.
#[test]
fn battle_drone_create_primary_comes_from_authored_weapon_set() {
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let mut game_logic = GameLogic::new();
    let mut tpl = crate::game_logic::ThingTemplate::new("AmericaVehicleBattleDrone");
    tpl.add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(200.0)
        .set_primary_weapon_name("HumveeGun");
    game_logic
        .templates
        .insert("AmericaVehicleBattleDrone".to_string(), tpl);
    let id = game_logic
        .create_object(
            "AmericaVehicleBattleDrone",
            Team::USA,
            Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("battle_drone");
    let prim = game_logic
        .host_object(id)
        .and_then(|t| t.weapon.clone())
        .expect("authored primary");
    assert!(
        (prim.damage - crate::game_logic::host_slave_drones::BATTLE_DRONE_GUN_DAMAGE).abs() > 0.01,
        "authored HumveeGun PRIMARY overwritten by battle_drone name peel: {prim:?}"
    );
}

/// hq-2qedt.131: China Red Guard PRIMARY binds from the authored WeaponSet at
/// create (C++ WeaponSet::updateWeaponSet), not a template-name peel.
#[test]
fn red_guard_create_primary_comes_from_authored_weapon_set() {
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let mut game_logic = GameLogic::new();
    let mut tpl = crate::game_logic::ThingTemplate::new("ChinaInfantryRedguard");
    tpl.add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(200.0)
        .set_primary_weapon_name("HumveeGun");
    game_logic
        .templates
        .insert("ChinaInfantryRedguard".to_string(), tpl);
    let id = game_logic
        .create_object(
            "ChinaInfantryRedguard",
            Team::China,
            Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("red_guard");
    let prim = game_logic
        .host_object(id)
        .and_then(|t| t.weapon.clone())
        .expect("authored primary");
    assert!(
        (prim.damage - crate::game_logic::host_red_guard::REDGUARD_DAMAGE).abs() > 0.01,
        "authored HumveeGun PRIMARY overwritten by red_guard name peel: {prim:?}"
    );
}

/// hq-2qedt.132: China Tank Hunter PRIMARY binds from the authored WeaponSet at
/// create (C++ WeaponSet::updateWeaponSet), not a template-name peel.
#[test]
fn tank_hunter_create_primary_comes_from_authored_weapon_set() {
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let mut game_logic = GameLogic::new();
    let mut tpl = crate::game_logic::ThingTemplate::new("ChinaInfantryTankHunter");
    tpl.add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(200.0)
        .set_primary_weapon_name("HumveeGun");
    game_logic
        .templates
        .insert("ChinaInfantryTankHunter".to_string(), tpl);
    let id = game_logic
        .create_object(
            "ChinaInfantryTankHunter",
            Team::China,
            Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("tank_hunter");
    let prim = game_logic
        .host_object(id)
        .and_then(|t| t.weapon.clone())
        .expect("authored primary");
    assert!(
        (prim.damage - crate::game_logic::host_tank_hunter::TANK_HUNTER_DAMAGE).abs() > 0.01,
        "authored HumveeGun PRIMARY overwritten by tank_hunter name peel: {prim:?}"
    );
}

/// hq-2qedt.133: Colonel Burton PRIMARY binds from the authored WeaponSet at
/// create (C++ WeaponSet::updateWeaponSet), not a template-name peel.
#[test]
fn colonel_burton_create_primary_comes_from_authored_weapon_set() {
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let mut game_logic = GameLogic::new();
    let mut tpl = crate::game_logic::ThingTemplate::new("AmericaInfantryColonelBurton");
    tpl.add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(200.0)
        .set_primary_weapon_name("HumveeGun");
    game_logic
        .templates
        .insert("AmericaInfantryColonelBurton".to_string(), tpl);
    let id = game_logic
        .create_object(
            "AmericaInfantryColonelBurton",
            Team::USA,
            Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("colonel_burton");
    let prim = game_logic
        .host_object(id)
        .and_then(|t| t.weapon.clone())
        .expect("authored primary");
    assert!(
        (prim.damage - crate::game_logic::host_colonel_burton::BURTON_SNIPER_DAMAGE).abs() > 0.01,
        "authored HumveeGun PRIMARY overwritten by colonel_burton name peel: {prim:?}"
    );
}

/// hq-2qedt.134: C++ fires the authored WeaponSet weapon; a Fire Base-named
/// template authoring a non-howitzer PRIMARY must not route through the
/// Fire Base howitzer residual on object hit.
#[test]
fn fire_base_named_non_howitzer_weapon_skips_fire_base_object_hit_residual() {
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    let mut tpl = crate::game_logic::ThingTemplate::new("AmericaFireBase");
    // Non-structure fixture so the strategy_center base-defense path (out of
    // scope) does not fire; isolates the object_hit gate.
    tpl.add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(1000.0)
        .set_primary_weapon_name("HumveeGun");
    game_logic
        .templates
        .insert("AmericaFireBase".to_string(), tpl);
    let fb_id = game_logic
        .create_object("AmericaFireBase", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("fire base");
    let enemy = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(40.0, 0.0, 0.0))
        .expect("enemy");
    {
        let (t, attack_events) = game_logic.host_object_and_attack_events_mut(fb_id).unwrap();
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
    for f in 50..60 {
        game_logic.set_current_frame(f);
        game_logic.update_combat(&[fb_id, enemy], LOGIC_FRAME_TIMESTEP);
    }
    assert_eq!(
        game_logic.fire_base_residual_fires(),
        0,
        "Fire Base residual chosen by template-name peel instead of authored weapon"
    );
}

/// hq-2qedt.162: strategy_center base-defense path must key the Fire Base
/// howitzer residual on the authored WeaponSet slot weapon, not template name.
#[test]
fn fire_base_named_structure_non_howitzer_weapon_skips_base_defense_howitzer() {
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    let mut tpl = crate::game_logic::ThingTemplate::new("AmericaFireBase");
    tpl.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(1000.0)
        .set_primary_weapon_name("HumveeGun");
    game_logic
        .templates
        .insert("AmericaFireBase".to_string(), tpl);
    let fb_id = game_logic
        .create_object("AmericaFireBase", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("fire base");
    let _enemy = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(40.0, 0.0, 0.0))
        .expect("enemy");
    if let Some(w) = game_logic
        .objects
        .get_mut(&fb_id)
        .and_then(|o| o.weapon.as_mut())
    {
        w.last_fire_time = -20.0;
        w.reload_time = 0.1;
    }
    for f in 50..60 {
        game_logic.set_current_frame(f);
        game_logic.try_base_defense_residual_fire(fb_id);
    }
    assert_eq!(
        game_logic.fire_base_residual_fires(),
        0,
        "base-defense Fire Base howitzer chosen by template-name peel instead of authored weapon"
    );
}

/// hq-2qedt.163: Stinger Site PRIMARY/SECONDARY bind from the authored
/// WeaponSet at create (C++ WeaponSet::updateWeaponSet), not a name peel.
#[test]
fn stinger_site_create_weapons_come_from_authored_weapon_set() {
    use crate::game_logic::host_base_defense::{STINGER_AIR_DAMAGE, STINGER_GROUND_DAMAGE};
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let mut game_logic = GameLogic::new();
    let mut tpl = crate::game_logic::ThingTemplate::new("GLAStingerSite");
    tpl.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::SpawnsAreTheWeapons)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(1000.0)
        .set_primary_weapon_name("HumveeGun")
        .set_secondary_weapon_name("HumveeGun");
    game_logic
        .templates
        .insert("GLAStingerSite".to_string(), tpl);
    let id = game_logic
        .create_object("GLAStingerSite", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("stinger site");
    let obj = game_logic.host_object(id).expect("obj");
    let prim = obj.weapon.clone().expect("authored primary");
    let sec = obj.secondary_weapon.clone().expect("authored secondary");
    assert!(
        (prim.damage - STINGER_GROUND_DAMAGE).abs() > 0.01,
        "authored HumveeGun PRIMARY overwritten by stinger name peel: {prim:?}"
    );
    assert!(
        (sec.damage - STINGER_AIR_DAMAGE).abs() > 0.01,
        "authored HumveeGun SECONDARY overwritten by stinger name peel: {sec:?}"
    );
}

/// hq-2qedt.164: Overlord PRIMARY binds from the authored WeaponSet at
/// create (C++ WeaponSet::updateWeaponSet), not a template-name peel.
#[test]
fn overlord_create_primary_comes_from_authored_weapon_set() {
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let peel = crate::game_logic::host_overlord_gun::overlord_gun_weapon(false);
    let mut game_logic = GameLogic::new();
    let mut tpl = crate::game_logic::ThingTemplate::new("ChinaTankOverlord");
    tpl.add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(500.0)
        .set_primary_weapon_name("HumveeGun");
    game_logic
        .templates
        .insert("ChinaTankOverlord".to_string(), tpl);
    let id = game_logic
        .create_object("ChinaTankOverlord", Team::China, Vec3::new(0.0, 0.0, 0.0))
        .expect("overlord");
    let prim = game_logic
        .host_object(id)
        .and_then(|t| t.weapon.clone())
        .expect("authored primary");
    assert!(
        (prim.damage - peel.damage).abs() > 0.01 || (prim.range - peel.range).abs() > 0.01,
        "authored HumveeGun PRIMARY overwritten by overlord name peel: {prim:?}"
    );
}

/// hq-2qedt.165: Jarmen Kell PRIMARY binds from the authored WeaponSet at
/// create (C++ WeaponSet::updateWeaponSet), not a template-name peel.
#[test]
fn jarmen_kell_create_primary_comes_from_authored_weapon_set() {
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let peel = crate::game_logic::host_jarmen_kell::jarmen_kell_weapon(false);
    let mut game_logic = GameLogic::new();
    let mut tpl = crate::game_logic::ThingTemplate::new("GLAInfantryJarmenKell");
    tpl.add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(500.0)
        .set_primary_weapon_name("HumveeGun");
    game_logic
        .templates
        .insert("GLAInfantryJarmenKell".to_string(), tpl);
    let id = game_logic
        .create_object("GLAInfantryJarmenKell", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("jarmen_kell");
    let prim = game_logic
        .host_object(id)
        .and_then(|t| t.weapon.clone())
        .expect("authored primary");
    assert!(
        (prim.damage - peel.damage).abs() > 0.01 || (prim.range - peel.range).abs() > 0.01,
        "authored HumveeGun PRIMARY overwritten by jarmen_kell name peel: {prim:?}"
    );
}

/// hq-2qedt.166: MiniGunner PRIMARY binds from the authored WeaponSet at
/// create (C++ WeaponSet::updateWeaponSet), not a template-name peel.
#[test]
fn minigunner_create_primary_comes_from_authored_weapon_set() {
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let peel = crate::game_logic::host_minigunner::minigunner_ground_weapon(
        crate::game_logic::host_gattling_tank::GattlingFireLevel::Base,
        false,
        false,
        false,
    );
    let mut game_logic = GameLogic::new();
    let mut tpl = crate::game_logic::ThingTemplate::new("Infa_ChinaInfantryMiniGunner");
    tpl.add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(500.0)
        .set_primary_weapon_name("HumveeGun");
    game_logic
        .templates
        .insert("Infa_ChinaInfantryMiniGunner".to_string(), tpl);
    let id = game_logic
        .create_object(
            "Infa_ChinaInfantryMiniGunner",
            Team::China,
            Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("minigunner");
    let prim = game_logic
        .host_object(id)
        .and_then(|t| t.weapon.clone())
        .expect("authored primary");
    assert!(
        (prim.damage - peel.damage).abs() > 0.01 || (prim.range - peel.range).abs() > 0.01,
        "authored HumveeGun PRIMARY overwritten by minigunner name peel: {prim:?}"
    );
}

/// hq-2qedt.181: C++ fires the authored WeaponSet weapon; an Overlord-named
/// template authoring a non-OverlordTankGun PRIMARY must not route through
/// the Overlord main-gun residual (OverlordTankShell) on object hit.
#[test]
fn overlord_named_non_overlord_gun_skips_overlord_object_hit_residual() {
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    let mut tpl = crate::game_logic::ThingTemplate::new("ChinaTankOverlord");
    tpl.add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(1000.0)
        .set_primary_weapon_name("HumveeGun");
    game_logic
        .templates
        .insert("ChinaTankOverlord".to_string(), tpl);
    let ol_id = game_logic
        .create_object("ChinaTankOverlord", Team::China, Vec3::new(0.0, 0.0, 0.0))
        .expect("overlord");
    let enemy = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(40.0, 0.0, 0.0))
        .expect("enemy");
    {
        let (t, attack_events) = game_logic.host_object_and_attack_events_mut(ol_id).unwrap();
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
    let mut shells = 0;
    for f in 50..60 {
        game_logic.set_current_frame(f);
        game_logic.update_combat(&[ol_id, enemy], LOGIC_FRAME_TIMESTEP);
        shells = shells.max(
            game_logic
                .objects
                .registered_values()
                .filter(|o| o.template_name == "OverlordTankShell")
                .count(),
        );
    }
    assert_eq!(
        shells + game_logic.overlord_gun_residual_fires() as usize,
        0,
        "Overlord residual chosen by template-name peel instead of authored weapon"
    );
}
