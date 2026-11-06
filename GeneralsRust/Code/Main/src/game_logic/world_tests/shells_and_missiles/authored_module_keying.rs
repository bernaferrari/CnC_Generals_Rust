//! Host GameLogic tests — authored module keying.
#![allow(unused_imports, non_snake_case, unused_variables, dead_code)]
use super::*;

/// hq-2qedt.220: Raptor / King Raptor PRIMARY binds from the authored
/// WeaponSet at create (C++ WeaponSet::updateWeaponSet), not a name peel.
#[test]
fn raptor_create_primary_comes_from_authored_weapon_set() {
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    for (name, king) in [("AmericaJetRaptor", false), ("AirF_AmericaJetRaptor", true)] {
        let peel = crate::game_logic::host_raptor::raptor_weapon(king, false);
        let mut game_logic = GameLogic::new();
        let mut tpl = crate::game_logic::ThingTemplate::new(name);
        tpl.add_kind_of(KindOf::Aircraft)
            .add_kind_of(KindOf::Vehicle)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_health(160.0)
            .set_primary_weapon_name("HumveeGun");
        game_logic.templates.insert(name.to_string(), tpl);
        let id = game_logic
            .create_object(name, Team::USA, Vec3::new(0.0, 0.0, 0.0))
            .expect("raptor");
        let prim = game_logic
            .host_object(id)
            .and_then(|t| t.weapon.clone())
            .expect("authored primary");
        assert!(
            (prim.damage - peel.damage).abs() > 0.01 || (prim.range - peel.range).abs() > 0.01,
            "{name}: authored HumveeGun PRIMARY overwritten by raptor name peel: {prim:?}"
        );
    }
}

/// hq-2qedt.255: Comanche rocket-pod TERTIARY at create keys on the authored
/// WeaponSet (Comanche20mmCannonWeapon chassis), not the template name.
#[test]
fn comanche_rocket_pods_create_key_on_authored_weapon_set() {
    use crate::game_logic::host_comanche_rocket_pods::{
        COMANCHE_PRIMARY_WEAPON, UPGRADE_COMANCHE_ROCKET_PODS,
    };
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let run = |name: &str, weapon: &str| -> bool {
        let mut game_logic = GameLogic::new();
        let mut player = Player::new(0, Team::USA, "USA", true);
        player.add_completed_upgrade(UPGRADE_COMANCHE_ROCKET_PODS);
        game_logic.add_player(player);
        let mut tpl = ThingTemplate::new(name);
        tpl.add_kind_of(KindOf::Vehicle)
            .add_kind_of(KindOf::Aircraft)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_health(220.0)
            .set_primary_weapon_name(weapon);
        game_logic.templates.insert(name.to_string(), tpl);
        let id = game_logic
            .create_object(name, Team::USA, Vec3::new(0.0, 0.0, 0.0))
            .expect("unit");
        game_logic
            .host_object(id)
            .unwrap()
            .tertiary_weapon
            .is_some()
    };
    assert!(
        run("USA_Gunship", COMANCHE_PRIMARY_WEAPON),
        "authored Comanche WeaponSet must receive rocket pods TERTIARY"
    );
    assert!(
        !run("AmericaVehicleComanche", "HumveeGun"),
        "Comanche-named chassis with authored HumveeGun must not receive pods"
    );
}

/// hq-2qedt.256: Emperor innate propaganda keys on the authored
/// PropagandaTowerBehavior + OverlordContain modules, not the template name.
#[test]
fn emperor_innate_propaganda_keys_on_authored_modules() {
    let run = |name: &str, propaganda: bool| -> (bool, bool) {
        let mut world = GameLogic::new();
        let tower = if propaganda {
            "Behavior = PropagandaTowerBehavior ModulePropaganda_04\nRadius = 150.0\nDelayBetweenUpdates = 2000\nHealPercentEachSecond = 1%\nEnd\n"
        } else {
            ""
        };
        let source = format!(
            "Object {name}\nKindOf = VEHICLE SELECTABLE\nBody = ActiveBody Body\nMaxHealth = 100\nInitialHealth = 100\nEnd\nBehavior = OverlordContain ModuleTag_06\nSlots = 1\nEnd\n{tower}End\n"
        );
        world
            .admit_receiving_module_content(
                crate::assets::ReceivingObjectContent::from_sources([(
                    "emperor.ini".into(),
                    source,
                )])
                .unwrap(),
            )
            .unwrap();
        world.add_player(Player::new(1, Team::China, "owner", true));
        let id = world
            .create_object_for_player_with_effects_checked(
                name,
                1,
                Vec3::ZERO,
                None,
                &mut Default::default(),
            )
            .unwrap()
            .unwrap();
        let o = world.host_object(id).unwrap();
        (
            o.has_overlord_propaganda_addon,
            o.authors_innate_propaganda_tower(),
        )
    };
    assert_eq!(
        run("TestHullWithTower", true),
        (true, true),
        "authored innate PropagandaTowerBehavior on OverlordContain host is Emperor-style"
    );
    assert_eq!(
        run("Tank_ChinaTankEmperor", false),
        (false, false),
        "Emperor-named hull without authored PropagandaTowerBehavior gets no innate tower"
    );
}

/// hq-2qedt.257: Emperor addon-conflict orders key on the authored innate
/// PropagandaTowerBehavior, not the template name.
#[test]
fn emperor_addon_conflicts_key_on_authored_innate_tower() {
    let run = |name: &str, propaganda: bool| -> (bool, bool) {
        let mut world = GameLogic::new();
        let tower = if propaganda {
            "Behavior = PropagandaTowerBehavior ModulePropaganda_04\nRadius = 150.0\nEnd\n"
        } else {
            ""
        };
        let source = format!(
            "Object {name}\nKindOf = VEHICLE SELECTABLE\nBody = ActiveBody Body\nMaxHealth = 100\nInitialHealth = 100\nEnd\nBehavior = OverlordContain ModuleTag_06\nSlots = 1\nEnd\n{tower}End\n"
        );
        world
            .admit_receiving_module_content(
                crate::assets::ReceivingObjectContent::from_sources([(
                    "emperor.ini".into(),
                    source,
                )])
                .unwrap(),
            )
            .unwrap();
        world.add_player(Player::new(1, Team::China, "owner", true));
        let id = world
            .create_object_for_player_with_effects_checked(
                name,
                1,
                Vec3::ZERO,
                None,
                &mut Default::default(),
            )
            .unwrap()
            .unwrap();
        let o = world.host_object_mut(id).unwrap();
        o.has_overlord_propaganda_addon = true;
        o.install_overlord_gattling_addon();
        (
            o.has_overlord_propaganda_addon,
            o.has_overlord_propaganda_residual(),
        )
    };
    assert_eq!(
        run("TestHullWithTower", true),
        (true, true),
        "authored innate tower survives the gattling ConflictsWith clear"
    );
    assert_eq!(
        run("Tank_ChinaTankEmperor", false),
        (false, false),
        "Emperor-named hull without authored tower takes the normal conflict clear"
    );
}

/// hq-2qedt.258: Patriot dual ground/AA create binding keys on the authored
/// WeaponSet PRIMARY (PatriotMissileWeapon family), not the template name.
#[test]
fn patriot_create_binding_keys_on_authored_weapon_set() {
    use crate::game_logic::host_base_defense::{
        PATRIOT_GROUND_DAMAGE, PATRIOT_PRIMARY_WEAPON, PATRIOT_SECONDARY_WEAPON,
    };
    crate::game_logic::weapon_bootstrap::ensure_host_weapon_store();
    let run = |name: &str, primary: &str, secondary: Option<&str>| -> bool {
        let mut world = GameLogic::new();
        let mut tpl = crate::game_logic::ThingTemplate::new(name);
        tpl.add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::Attackable)
            .add_kind_of(KindOf::FSBaseDefense)
            .set_health(600.0)
            .set_primary_weapon_name(primary);
        if let Some(s) = secondary {
            tpl.set_secondary_weapon_name(s);
        }
        world.templates.insert(name.to_string(), tpl);
        let id = world
            .create_object(name, Team::USA, Vec3::ZERO)
            .expect("defense");
        let o = world.host_object(id).unwrap();
        o.weapon
            .as_ref()
            .is_some_and(|w| (w.damage - PATRIOT_GROUND_DAMAGE).abs() < 0.01)
    };
    assert!(
        run(
            "TestAuthoredBattery",
            PATRIOT_PRIMARY_WEAPON,
            Some(PATRIOT_SECONDARY_WEAPON)
        ),
        "authored PatriotMissileWeapon WeaponSet binds the Patriot ground primary"
    );
    assert!(
        !run("USA_Patriot", "HumveeGun", None),
        "Patriot-named hull without authored Patriot WeaponSet gets no Patriot ground peel"
    );
}

/// hq-2qedt.259: Stinger Site hive spawn keys on the authored
/// KINDOF_SPAWNS_ARE_THE_WEAPONS bit, not the template name.
#[test]
fn stinger_hive_spawn_keys_on_authored_spawns_are_the_weapons() {
    let run = |name: &str, hive: bool| -> usize {
        let mut logic = GameLogic::new();
        logic
            .players
            .insert(0, crate::game_logic::Player::new(0, Team::GLA, "GLA", true));
        let mut site = crate::game_logic::ThingTemplate::new(name);
        site.add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::FSBaseDefense)
            .set_health(1000.0);
        if hive {
            site.add_kind_of(KindOf::SpawnsAreTheWeapons);
        }
        logic.templates.insert(name.into(), site);
        let mut soldier = crate::game_logic::ThingTemplate::new("GLAInfantryStingerSoldier");
        soldier
            .add_kind_of(KindOf::Infantry)
            .add_kind_of(KindOf::Selectable)
            .set_health(100.0);
        logic
            .templates
            .insert("GLAInfantryStingerSoldier".into(), soldier);
        let site_id = logic
            .create_object(name, Team::GLA, Vec3::ZERO)
            .expect("site");
        logic
            .objects
            .registered_values()
            .filter(|o| o.producer_id == Some(site_id))
            .count()
    };
    assert_eq!(
        run("TestHiveDefense", true),
        3,
        "authored SPAWNS_ARE_THE_WEAPONS hive spawns its Stinger soldiers"
    );
    assert_eq!(
        run("GLAStingerSite", false),
        0,
        "Stinger-named structure without the authored hive KindOf spawns nothing"
    );
}

/// hq-2qedt.260: Black Lotus innate hero stealth keys on the authored
/// SpecialAbilityBlackLotusCaptureBuilding capture power, not the template name.
#[test]
fn black_lotus_innate_stealth_keys_on_authored_capture_power() {
    let run = |name: &str, lotus: bool| -> bool {
        let mut logic = GameLogic::new();
        logic.players.insert(
            0,
            crate::game_logic::Player::new(0, Team::China, "China", true),
        );
        let mut t = crate::game_logic::ThingTemplate::new(name);
        t.add_kind_of(KindOf::Infantry)
            .add_kind_of(KindOf::Selectable)
            .set_health(200.0);
        if lotus {
            t.capture_power = crate::game_logic::CapturePowerKind::BlackLotus;
        }
        logic.templates.insert(name.into(), t);
        let id = logic
            .create_object(name, Team::China, Vec3::ZERO)
            .expect("lotus");
        logic.host_object(id).unwrap().innate_stealth
    };
    assert!(
        run("TestHeroCaptureInfantry", true),
        "authored Black Lotus capture power gets innate hero stealth"
    );
    assert!(
        !run("ChinaInfantryBlackLotus", false),
        "Lotus-named infantry without the authored capture power gets no hero stealth"
    );
}

/// hq-2qedt.261: Pathfinder reveal response (OrderIdleEnemiesToAttackMeUponReveal)
/// keys on the authored USAPathfinderSniperRifle WeaponSet slot, not the name.
#[test]
fn pathfinder_reveal_response_keys_on_authored_sniper_weapon() {
    let run = |name: &str, weapon: &str| -> bool {
        let mut logic = GameLogic::new();
        logic
            .players
            .insert(0, crate::game_logic::Player::new(0, Team::USA, "USA", true));
        logic.players.insert(
            1,
            crate::game_logic::Player::new(1, Team::China, "China", true),
        );
        let mut t = crate::game_logic::ThingTemplate::new(name);
        t.add_kind_of(KindOf::Infantry)
            .add_kind_of(KindOf::Selectable)
            .set_health(100.0)
            .set_primary_weapon_name(weapon);
        logic.templates.insert(name.into(), t);
        let mut e = crate::game_logic::ThingTemplate::new("TestChinaGrunt");
        e.add_kind_of(KindOf::Infantry)
            .add_kind_of(KindOf::Selectable)
            .set_health(100.0);
        logic.templates.insert("TestChinaGrunt".into(), e);
        let v = logic
            .create_object(name, Team::USA, Vec3::ZERO)
            .expect("victim");
        let eid = logic
            .create_object("TestChinaGrunt", Team::China, Vec3::new(20.0, 0.0, 0.0))
            .expect("enemy");
        logic.host_object_mut(eid).unwrap().vision_range = 300.0;
        logic.order_idle_enemies_to_attack_on_reveal(v);
        logic.host_object(eid).unwrap().target == Some(v)
    };
    assert!(
        run(
            "TestSniperInfantry",
            crate::game_logic::host_pathfinder::PATHFINDER_SNIPER_WEAPON
        ),
        "authored Pathfinder sniper WeaponSet orders idle enemies on reveal"
    );
    assert!(
        !run("AmericaInfantryPathfinder", "TestNonRetailRifle"),
        "Pathfinder-named infantry without the authored sniper does not"
    );
}

/// hq-2qedt.194: C++ BlackNapalm / Tactical Nuke MiG are WeaponSetUpgrade
/// (WEAPONSET_PLAYER_UPGRADE → updateWeaponSet); the refreshed PRIMARY is the
/// upgrade row's authored weapon, not name-built mig_weapon() stats.
#[test]
fn mig_upgrade_refresh_binds_authored_weapon_not_name_built_stats() {
    use crate::game_logic::host_mig::{
        BLACK_NAPALM_MISSILE_WEAPON, MigLoadout, NAPALM_MISSILE_WEAPON, NUKE_MIG_MISSILE_WEAPON,
        NUKE_NUKE_MISSILE_WEAPON, mig_weapon,
    };
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    // Retail base PRIMARY authored (hq-2qedt.248 gate keys on authored MiG missile).
    for (tpl_name, nuke, weapon_name, loadout, base) in [
        (
            "ChinaJetMIG",
            false,
            BLACK_NAPALM_MISSILE_WEAPON,
            MigLoadout::BlackNapalm,
            NAPALM_MISSILE_WEAPON,
        ),
        (
            "Nuke_ChinaJetMIG",
            true,
            NUKE_NUKE_MISSILE_WEAPON,
            MigLoadout::NukeTactical,
            NUKE_MIG_MISSILE_WEAPON,
        ),
    ] {
        let mut game_logic = GameLogic::new();
        let mut tpl = crate::game_logic::ThingTemplate::new(tpl_name);
        tpl.add_kind_of(KindOf::Aircraft)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_health(300.0)
            .set_primary_weapon_name(base);
        game_logic.templates.insert(tpl_name.to_string(), tpl);
        let id = game_logic
            .create_object(tpl_name, Team::China, Vec3::new(0.0, 0.0, 0.0))
            .expect("mig");
        let applied = if nuke {
            game_logic.apply_mig_tactical_nuke_upgrade(id)
        } else {
            game_logic.apply_mig_black_napalm_upgrade(id)
        };
        assert!(applied, "{tpl_name} upgrade applies");
        let prim = game_logic
            .host_object(id)
            .and_then(|o| o.weapon.clone())
            .expect("upgraded primary");
        let authored = game_logic
            .with_own_services(|| {
                crate::game_logic::ThingTemplate::weapon_from_host_store(weapon_name)
            })
            .expect("authored upgrade weapon");
        let built = mig_weapon(loadout);
        assert!(
            (authored.splash_radius - built.splash_radius).abs() > 0.01
                || (authored.damage - built.damage).abs() > 0.01,
            "fixture must distinguish authored vs name-built"
        );
        assert!(
            (prim.damage - authored.damage).abs() < 0.01
                && (prim.splash_radius - authored.splash_radius).abs() < 0.01
                && (prim.min_range - authored.min_range).abs() < 0.01,
            "{tpl_name}: refresh used name-built mig_weapon(): {prim:?} vs authored {authored:?}"
        );
    }
}

/// hq-2qedt.156: C++ binds PRIMARY from the template WeaponSet at create
/// (WeaponSet::updateWeaponSet), not a template-name host table. A
/// MiG-named template authoring a different PRIMARY must keep it.
#[test]
fn mig_create_primary_comes_from_authored_weapon_set() {
    use crate::game_logic::host_mig::{MigLoadout, mig_primary_damage};
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let mut game_logic = GameLogic::new();
    let mut tpl = crate::game_logic::ThingTemplate::new("ChinaJetMIG");
    tpl.add_kind_of(KindOf::Aircraft)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(300.0)
        .set_primary_weapon_name("HumveeGun");
    game_logic.templates.insert("ChinaJetMIG".to_string(), tpl);
    let id = game_logic
        .create_object("ChinaJetMIG", Team::China, Vec3::new(0.0, 0.0, 0.0))
        .expect("mig");
    let obj = game_logic.host_object(id).expect("mig");
    let prim = obj.weapon.clone().expect("authored primary");
    assert!(
        (prim.damage - mig_primary_damage(MigLoadout::Standard)).abs() > 0.01,
        "authored HumveeGun PRIMARY overwritten by mig_weapon() name peel: {prim:?}"
    );
}

/// hq-2qedt.154: C++ binds PRIMARY from the template WeaponSet at create
/// (WeaponSet::updateWeaponSet), not a template-name host table. A
/// Scorpion-named template authoring a different PRIMARY must keep it.
#[test]
fn scorpion_create_primary_comes_from_authored_weapon_set() {
    use crate::game_logic::host_scorpion::{SCORPION_GUN_DAMAGE, SCORPION_GUN_DAMAGE_PLUS};
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    ensure_host_weapon_store();
    let mut game_logic = GameLogic::new();
    let mut tpl = crate::game_logic::ThingTemplate::new("GLATankScorpion");
    tpl.add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(300.0)
        .set_primary_weapon_name("HumveeGun");
    game_logic
        .templates
        .insert("GLATankScorpion".to_string(), tpl);
    let id = game_logic
        .create_object("GLATankScorpion", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("scorpion");
    let obj = game_logic.host_object(id).expect("scorpion");
    let prim = obj.weapon.clone().expect("authored primary");
    assert!(
        (prim.damage - SCORPION_GUN_DAMAGE).abs() > 0.01
            && (prim.damage - SCORPION_GUN_DAMAGE_PLUS).abs() > 0.01,
        "authored HumveeGun PRIMARY overwritten by scorpion_gun_weapon() name peel: {prim:?}"
    );
}

#[test]
fn stealth_fighter_air_and_mig_residual_has_no_template_name_sniff() {
    // C++ StealthFighter residual keys off the authored weapon, never the template name.
    let src = include_str!("../../world_combat/air_and_mig.rs");
    assert!(!src.contains("is_stealth_fighter_template"));
}

/// hq-2qedt.135: the innate/ATTACKING-forbidden stealth residual follows the
/// template's authored StealthUpdate (InnateStealth / StealthDelay /
/// StealthForbiddenConditions), as C++ StealthUpdate module data does, not a
/// template-name sniff.
#[test]
fn stealth_fighter_innate_stealth_residual_keys_on_authored_stealth_update() {
    let mut game_logic = GameLogic::new();
    let mut named = crate::game_logic::ThingTemplate::new("AmericaJetStealthFighter");
    named
        .add_kind_of(KindOf::Aircraft)
        .add_kind_of(KindOf::Selectable)
        .set_health(150.0);
    game_logic
        .templates
        .insert("AmericaJetStealthFighter".to_string(), named);
    let mut authored = crate::game_logic::ThingTemplate::new("TestCloakJet");
    authored
        .add_kind_of(KindOf::Aircraft)
        .add_kind_of(KindOf::Selectable)
        .set_health(150.0);
    authored.stealth_update_innate = true;
    authored.stealth_update_forbids_attacking = true;
    authored.stealth_update_delay_ms = Some(2000);
    game_logic
        .templates
        .insert("TestCloakJet".to_string(), authored);

    let named_id = game_logic
        .create_object(
            "AmericaJetStealthFighter",
            Team::USA,
            Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("named");
    let authored_id = game_logic
        .create_object("TestCloakJet", Team::USA, Vec3::new(50.0, 0.0, 0.0))
        .expect("authored");
    game_logic.update_stealth_and_detection();

    let n = game_logic.host_object(named_id).expect("named");
    assert!(
        !n.innate_stealth,
        "name-only Stealth Fighter template without StealthUpdate must not get innate stealth"
    );
    let a = game_logic.host_object(authored_id).expect("authored");
    assert!(a.innate_stealth, "authored InnateStealth must apply");
    assert_eq!(
        a.stealth_delay_frames, 60,
        "authored StealthDelay 2000ms = 60f"
    );
}

/// hq-2qedt.153: object_hit Comanche residuals key on the fired slot's authored
/// weapon (Comanche20mmCannonWeapon), not an is_comanche_template sniff. C++
/// Weapon::fireWeapon has no firing-template name branch.
#[test]
fn comanche_cannon_object_hit_residual_keys_on_authored_weapon_not_template_name() {
    use crate::game_logic::host_comanche_rocket_pods::COMANCHE_PRIMARY_WEAPON;
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    fn fire(name: &str, weapon: &str) -> u32 {
        ensure_host_weapon_store();
        let mut gl = GameLogic::new();
        ensure_test_tank_template(&mut gl);
        let mut tpl = crate::game_logic::ThingTemplate::new(name);
        tpl.add_kind_of(KindOf::Aircraft)
            .add_kind_of(KindOf::Vehicle)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_health(160.0)
            .set_primary_weapon_name(weapon);
        gl.templates.insert(name.to_string(), tpl);
        let shooter = gl
            .create_object(name, Team::USA, Vec3::new(0.0, 0.0, 0.0))
            .expect("shooter");
        let enemy = gl
            .create_object("TestTank", Team::GLA, Vec3::new(50.0, 0.0, 0.0))
            .expect("enemy");
        {
            let (f, ev) = gl.host_object_and_attack_events_mut(shooter).unwrap();
            f.attack_target(
                enemy,
                ev,
                &mut crate::game_logic::ObjectAudio::record(&mut Vec::new()),
            );
            if let Some(w) = f.weapon.as_mut() {
                w.last_fire_time = -20.0;
                w.reload_time = 0.05;
            }
        }
        gl.set_current_frame(50);
        gl.update_combat(&[shooter, enemy], LOGIC_FRAME_TIMESTEP);
        gl.comanche_cannon_residual_fires
    }

    assert!(
        fire("TestAuthoredGunshipCarrier", COMANCHE_PRIMARY_WEAPON) > 0,
        "authored Comanche20mmCannonWeapon must take the cannon residual"
    );
    assert_eq!(
        fire("AmericaVehicleComanche", "RaptorJetMissileWeapon"),
        0,
        "comanche-named template with a different authored weapon must not"
    );
}

/// hq-2qedt.180: vehicle_shells Scorpion splash keys on the fired slot's
/// authored weapon (ScorpionTankGun), not an is_scorpion_template sniff.
/// C++ splash comes from the Weapon template (Weapon::fireWeapon / dealDamageInternal).
#[test]
fn scorpion_splash_residual_keys_on_authored_weapon_not_template_name() {
    use crate::game_logic::host_scorpion::SCORPION_TANK_GUN;
    use crate::game_logic::weapon_bootstrap::ensure_host_weapon_store;

    fn splash(name: &str, weapon: &str) -> u32 {
        ensure_host_weapon_store();
        let mut gl = GameLogic::new();
        ensure_test_tank_template(&mut gl);
        let mut tpl = crate::game_logic::ThingTemplate::new(name);
        tpl.add_kind_of(KindOf::Vehicle)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_health(200.0)
            .set_primary_weapon_name(weapon);
        gl.templates.insert(name.to_string(), tpl);
        let shooter = gl
            .create_object(name, Team::GLA, Vec3::new(0.0, 0.0, 0.0))
            .expect("shooter");
        let enemy = gl
            .create_object("TestTank", Team::USA, Vec3::new(45.0, 0.0, 0.0))
            .expect("enemy");
        let pos = gl.objects.get(&enemy).unwrap().get_position();
        gl.apply_scorpion_residual_at(pos, Some(shooter), Some(enemy), 0)
            .0
    }

    assert!(
        splash("TestAuthoredGunCarrier", SCORPION_TANK_GUN) > 0,
        "authored ScorpionTankGun must take the splash residual"
    );
    assert_eq!(
        splash("GLATankScorpion", "RaptorJetMissileWeapon"),
        0,
        "scorpion-named template with a different authored weapon must not"
    );
}

/// hq-2qedt.180: vehicle_shells has no Scorpion template-name sniff.
#[test]
fn scorpion_vehicle_shells_has_no_template_name_sniff() {
    let src = include_str!("../../world_combat/vehicle_shells.rs");
    assert!(!src.contains("is_scorpion_template"));
}

/// hq-2qedt.208: factory_init Scorpion rocket SECONDARY gate keys on the
/// authored PRIMARY (ScorpionTankGun family), not is_scorpion_template.
/// C++ WeaponSetUpgrade flips PLAYER_UPGRADE on any object whose authored
/// modules carry it; WeaponSet::updateWeaponSet then picks the row.
#[test]
fn scorpion_factory_init_rocket_gate_keys_on_authored_gun_not_template_name() {
    let src = include_str!("../../world_objects/create_destroy_die/factory_init.rs");
    assert!(!src.contains("is_scorpion_template"));
    assert!(src.contains("authored_scorpion_gun"));
}

/// hq-2qedt.209: object_hit has no Scorpion template-name sniff.
#[test]
fn scorpion_object_hit_residual_has_no_template_name_sniff() {
    let src = include_str!("../../world_tick/combat/object_hit.rs");
    assert!(!src.contains("is_scorpion_template"));
    assert!(!src.contains("should_apply_scorpion_residual"));
}

/// hq-2qedt.172: Comanche ground_target / air_and_mig residuals carry no
/// is_comanche_template sniff. C++ CommandXlat keys rocket-pod ground fire on
/// the weapon name "ComancheRocketPodWeapon", not the firing template name.
#[test]
fn comanche_rocket_pod_ground_fire_keys_on_authored_tertiary_not_template_name() {
    use crate::game_logic::host_comanche_rocket_pods::COMANCHE_ROCKET_POD_WEAPON;
    let gt = include_str!("../../world_tick/combat/ground_target.rs");
    assert!(!gt.contains("is_comanche_template"));
    assert!(gt.contains("COMANCHE_ROCKET_POD_WEAPON"));
    let am = include_str!("../../world_combat/air_and_mig.rs");
    assert!(!am.contains("is_comanche_template"));

    let mut gl = GameLogic::new();
    let mut tpl = crate::game_logic::ThingTemplate::new("TestAuthoredPodCarrier");
    tpl.add_kind_of(KindOf::Aircraft)
        .add_kind_of(KindOf::Selectable)
        .set_health(100.0)
        .set_tertiary_weapon_name(COMANCHE_ROCKET_POD_WEAPON);
    gl.templates
        .insert("TestAuthoredPodCarrier".to_string(), tpl);
    let id = gl
        .create_object(
            "TestAuthoredPodCarrier",
            Team::USA,
            Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("carrier");
    assert_eq!(
        gl.host_object(id).unwrap().authored_weapon_name_for_slot(2),
        Some(COMANCHE_ROCKET_POD_WEAPON)
    );
}
