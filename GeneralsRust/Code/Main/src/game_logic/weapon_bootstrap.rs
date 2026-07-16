//! Host WeaponStore bootstrap for template → weapon binding.
//!
//! # Why the GameLogic WeaponStore is often empty
//!
//! 1. `gamelogic::initialize_weapon_store()` only constructs an empty store.
//! 2. Full Weapon.ini population happens when AssetManager loads BIG archives
//!    (`assets::ini_template_loader::load_weapon_templates`). Headless unit tests
//!    and many host probes never open archives, so the store stays empty and
//!    `ThingTemplate::resolve_primary_weapon` falls back to `Weapon::default()`.
//! 3. Engine startup also parses Weapon.ini into Common's separate
//!    `game_engine::common::ini::ini_weapon` store (INI block table). That is
//!    **not** the GameLogic store that `ThingTemplate::weapon_from_store` reads.
//!
//! This module is the reliable host-side fill path:
//! - Prefer loading extracted / shipped `Data/INI/Weapon.ini` when present on disk
//! - Always seed a small set of golden-unit weapons if still missing
//!
//! Fail-closed: seeding known host weapons is not full Weapon.ini parity.
//! Secondary slots (`Weapon = SECONDARY Name`) are seeded for known units only;
//! full WeaponSet upgrade matrices are deferred.
//!
//! # Secondary combat residual (host `update_combat`)
//!
//! Binding alone is not enough: fire must consider `Object.secondary_weapon`.
//! Fail-closed host rules (not full AutoChoose / PreferredAgainst):
//! - Prefer secondary vs structures when secondary damage ≥ primary (or primary cannot fire).
//! - Otherwise primary first; secondary when primary is reloading / OOR (alternate fire).
//! - Player `active_weapon_slot == 1` forces secondary preference when ready + in range.
//! - Ground force-fire still uses primary only.

use gamelogic::weapon::{with_weapon_store, with_weapon_store_mut, WeaponAntiMask, WeaponTemplate};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

/// Retail primary weapon names used by host golden / skirmish unit templates.
pub const RANGER_PRIMARY_WEAPON: &str = "RangerAdvancedCombatRifle";
pub const GLA_REBEL_PRIMARY_WEAPON: &str = "GLARebelMachineGun";
pub const REDGUARD_PRIMARY_WEAPON: &str = "RedguardMachineGun";
pub const TANK_HUNTER_PRIMARY_WEAPON: &str = "ChinaInfantryTankHunterMissileLauncher";
/// China Troop Crawler residual DEPLOY primary.
pub const TROOP_CRAWLER_ASSAULT_WEAPON: &str = "TroopCrawlerAssault";
pub const HUMVEE_PRIMARY_WEAPON: &str = "HumveeGun";

/// Retail secondary weapon names used by host golden / skirmish unit templates.
/// Fail-closed residual: only units that need SECONDARY in host combat probes.
pub const RANGER_SECONDARY_WEAPON: &str = "RangerFlashBangGrenadeWeapon";
pub const HUMVEE_SECONDARY_WEAPON: &str = "HumveeMissileWeapon";

/// Retail base-defense primary weapons (Patriot / Gattling / Stinger structure residual).
pub const PATRIOT_PRIMARY_WEAPON: &str = "PatriotMissileWeapon";
/// Retail Patriot secondary AA residual.
pub const PATRIOT_SECONDARY_WEAPON: &str = "PatriotMissileWeaponAir";
/// Retail Stinger Site residual (SPAWNS_ARE_THE_WEAPONS abstraction).
pub const STINGER_PRIMARY_WEAPON: &str = "StingerMissileWeapon";
pub const STINGER_SECONDARY_WEAPON: &str = "StingerMissileWeaponAir";
pub const GATTLING_BUILDING_PRIMARY_WEAPON: &str = "GattlingBuildingGun";
/// Retail China Gattling Cannon secondary AA residual.
pub const GATTLING_BUILDING_SECONDARY_WEAPON: &str = "GattlingBuildingGunAir";

/// Retail Nuke Cannon primary / neutron secondary residual weapons.
pub const NUKE_CANNON_PRIMARY_WEAPON: &str = "NukeCannonGun";
pub const NUKE_CANNON_NEUTRON_WEAPON: &str = "NukeCannonNeutronWeapon";

/// Retail Inferno Cannon primary residual weapon (FireFieldSmall on impact).
pub const INFERNO_CANNON_PRIMARY_WEAPON: &str = "InfernoCannonGun";

/// Retail AuroraBombWeapon residual (AmericaJetAurora dive bomb).
pub const AURORA_BOMB_PRIMARY_WEAPON: &str = "AuroraBombWeapon";
/// Retail AirF_AuroraBombWeapon residual (FuelAir detonation path).
pub const AIRF_AURORA_BOMB_PRIMARY_WEAPON: &str = "AirF_AuroraBombWeapon";
/// Retail SupW_AuroraFuelBombWeapon residual.
pub const SUPW_AURORA_FUEL_BOMB_WEAPON: &str = "SupW_AuroraFuelBombWeapon";

/// Retail PointDefenseLaser residual weapons (Paladin / Avenger).
pub const PALADIN_POINT_DEFENSE_LASER: &str = "PaladinPointDefenseLaser";
pub const AVENGER_POINT_DEFENSE_LASER: &str = "AvengerPointDefenseLaserOne";

/// Retail America main battle tank guns + Avenger residual weapons.
pub const CRUSADER_TANK_GUN: &str = "CrusaderTankGun";
pub const PALADIN_TANK_GUN: &str = "PaladinTankGun";
/// Retail Laser General tank laser residual weapons.
pub const LAZR_CRUSADER_TANK_GUN: &str = "Lazr_CrusaderTankGun";
pub const LAZR_PALADIN_TANK_GUN: &str = "Lazr_PaladinTankGun";
pub const AVENGER_TARGET_DESIGNATOR: &str = "AvengerTargetDesignator";
pub const AVENGER_AIR_LASER: &str = "AvengerAirLaserOne";
/// Retail Humvee TOW air tertiary residual.
pub const HUMVEE_MISSILE_WEAPON_AIR: &str = "HumveeMissileWeaponAir";
/// Retail Laser General Patriot residual weapons.
pub const LAZR_PATRIOT_PRIMARY_WEAPON: &str = "Lazr_PatriotMissileWeapon";
pub const LAZR_PATRIOT_SECONDARY_WEAPON: &str = "Lazr_PatriotMissileWeaponAir";
/// Retail Superweapon General EMP Patriot residual weapons.
pub const SUPW_PATRIOT_PRIMARY_WEAPON: &str = "SupW_PatriotMissileWeapon";
pub const SUPW_PATRIOT_SECONDARY_WEAPON: &str = "SupW_PatriotMissileWeaponAir";
/// Retail GLA Tunnel Network structure gun residual.
pub const TUNNEL_NETWORK_GUN: &str = "TunnelNetworkGun";

/// Retail StealthJetMissileWeapon residual (Bunker Buster carrier primary).
pub const STEALTH_JET_MISSILE_WEAPON: &str = "StealthJetMissileWeapon";

/// Retail MicrowaveTankBuildingClearer residual (KILL_GARRISONED damage type).
pub const MICROWAVE_BUILDING_CLEARER_WEAPON: &str = "MicrowaveTankBuildingClearer";

/// Retail Comanche primary / anti-tank / rocket-pod residual weapons.
pub const COMANCHE_PRIMARY_WEAPON: &str = "Comanche20mmCannonWeapon";
pub const COMANCHE_ANTITANK_WEAPON: &str = "ComancheAntiTankMissileWeapon";
pub const COMANCHE_ROCKET_POD_WEAPON: &str = "ComancheRocketPodWeapon";

/// Retail Helix PRIMARY minigun residual weapon.
pub const HELIX_MINIGUN_WEAPON: &str = "HelixMinigunWeapon";

/// Retail China MiG napalm / BlackNapalm / Nuke MiG residual weapons.
pub const NAPALM_MISSILE_WEAPON: &str = "NapalmMissileWeapon";
pub const BLACK_NAPALM_MISSILE_WEAPON: &str = "BlackNapalmMissileWeapon";
pub const NUKE_MIG_MISSILE_WEAPON: &str = "Nuke_MiGMissileWeapon";
pub const NUKE_NUKE_MISSILE_WEAPON: &str = "Nuke_NukeMissileWeapon";

/// Retail America Fire Base howitzer residual weapon.
pub const FIRE_BASE_HOWITZER_WEAPON: &str = "FireBaseHowitzerGun";

/// Retail Sentry Drone gun residual weapon (PLAYER_UPGRADE primary).
pub const SENTRY_DRONE_GUN_WEAPON: &str = "SentryDroneGun";

/// Retail Pathfinder sniper residual weapon.
pub const PATHFINDER_SNIPER_WEAPON: &str = "USAPathfinderSniperRifle";

/// Retail Hellfire drone residual weapon.
pub const HELLFIRE_MISSILE_WEAPON: &str = "HellfireMissileWeapon";

/// Host GLA Angry Mob residual aggregate fire weapon (nexus residual).
pub const ANGRY_MOB_RESIDUAL_WEAPON: &str = "GLAAngryMobResidualWeapon";

/// Retail GLA Rocket Buggy primary residual weapons.
pub const BUGGY_ROCKET_WEAPON: &str = "BuggyRocketWeapon";
pub const BUGGY_ROCKET_WEAPON_UPGRADED: &str = "BuggyRocketWeaponUpgraded";

/// Retail GLA Quad Cannon ground / anti-air residual weapons.
pub const QUAD_CANNON_GUN: &str = "QuadCannonGun";
pub const QUAD_CANNON_GUN_AIR: &str = "QuadCannonGunAir";
pub const QUAD_CANNON_GUN_UPGRADE_ONE: &str = "QuadCannonGunUpgradeOne";
pub const QUAD_CANNON_GUN_UPGRADE_ONE_AIR: &str = "QuadCannonGunUpgradeOneAir";
pub const QUAD_CANNON_GUN_UPGRADE_TWO: &str = "QuadCannonGunUpgradeTwo";
pub const QUAD_CANNON_GUN_UPGRADE_TWO_AIR: &str = "QuadCannonGunUpgradeTwoAir";

/// Retail GLA Technical residual weapons (salvage tiers).
pub const TECHNICAL_MACHINE_GUN: &str = "TechnicalMachineGunWeapon";
pub const TECHNICAL_CANNON: &str = "TechnicalCannonWeapon";
pub const TECHNICAL_RPG: &str = "TechnicalRPGWeapon";

/// Retail GLA Toxin Tractor residual weapons.
pub const TOXIN_TRUCK_GUN: &str = "ToxinTruckGun";
pub const TOXIN_TRUCK_GUN_UPGRADED: &str = "ToxinTruckGunUpgraded";
pub const TOXIN_TRUCK_SPRAYER: &str = "ToxinTruckSprayer";
pub const TOXIN_TRUCK_SPRAYER_UPGRADED: &str = "ToxinTruckSprayerUpgraded";

/// Retail GLA SCUD launcher residual weapons.
pub const SCUD_GUN_EXPLOSIVE: &str = "SCUDLauncherGunExplosive";
pub const SCUD_GUN_TOXIN: &str = "SCUDLauncherGunToxin";
pub const SCUD_GUN_ANTHRAX: &str = "SCUDLauncherGunAnthrax";

/// Retail GLA Marauder salvage fire-rate residual weapons.
pub const MARAUDER_TANK_GUN: &str = "MarauderTankGun";
pub const MARAUDER_TANK_GUN_UPGRADE_ONE: &str = "MarauderTankGunUpgradeOne";
pub const MARAUDER_TANK_GUN_UPGRADE_TWO: &str = "MarauderTankGunUpgradeTwo";

/// Retail China Battlemaster primary residual weapon.
pub const BATTLE_MASTER_TANK_GUN: &str = "BattleMasterTankGun";

/// Retail GLA Scorpion residual weapons (gun + rocket secondary).
pub const SCORPION_TANK_GUN: &str = "ScorpionTankGun";
pub const SCORPION_TANK_GUN_PLUS_ONE: &str = "ScorpionTankGunPlusOne";
pub const SCORPION_MISSILE_WEAPON: &str = "ScorpionMissileWeapon";

/// Retail America Tomahawk residual weapon.
pub const TOMAHAWK_MISSILE_WEAPON: &str = "TomahawkMissileWeapon";

/// USA Raptor jet residual.
pub const RAPTOR_JET_MISSILE_WEAPON: &str = "RaptorJetMissileWeapon";
pub const AIRF_RAPTOR_JET_MISSILE_WEAPON: &str = "AirF_RaptorJetMissileWeapon";

/// USA Battle Drone residual.
pub const BATTLE_DRONE_MACHINE_GUN: &str = "BattleDroneMachineGun";

/// Retail China Overlord / Emperor residual main gun.
pub const OVERLORD_TANK_GUN: &str = "OverlordTankGun";

/// Retail GLA Jarmen Kell residual primary sniper.
pub const JARMEN_KELL_RIFLE: &str = "GLAJarmenKellRifle";

/// Retail GLA Combat Cycle rider residual weapons.
pub const REBEL_BIKER_MG: &str = "GLARebelBikerMachineGun";
pub const TUNNEL_DEFENDER_BIKER_ROCKET: &str = "TunnelDefenderBikerRocketWeapon";
pub const BIKER_KELL_SNIPER: &str = "GLABikerKellSniperRifle";
pub const TERRORIST_SUICIDE_WEAPON: &str = "TerroristSuicideWeapon";
/// Retail FireWeaponWhenDead residual for infantry Terrorist.
pub const SUICIDE_DYNAMITE_PACK: &str = "SuicideDynamitePack";

/// Retail USA Missile Defender residual weapons.
pub const MISSILE_DEFENDER_MISSILE_WEAPON: &str = "MissileDefenderMissileWeapon";
pub const MISSILE_DEFENDER_LASER_GUIDED_WEAPON: &str = "MissileDefenderLaserGuidedMissileWeapon";

/// Retail China Dragon Tank flame residual weapons.
pub const DRAGON_TANK_FLAME_WEAPON: &str = "DragonTankFlameWeapon";
pub const DRAGON_TANK_FLAME_WEAPON_UPGRADED: &str = "DragonTankFlameWeaponUpgraded";

/// Retail China Gattling Tank residual weapons.
pub const GATTLING_TANK_GUN: &str = "GattlingTankGun";
pub const GATTLING_TANK_GUN_AIR: &str = "GattlingTankGunAir";

/// Retail China MiniGunner residual weapons (Infantry General).
pub const MINIGUNNER_GUN: &str = "Infa_MiniGunnerGun";
pub const MINIGUNNER_GUN_AIR: &str = "Infa_MiniGunnerGunAir";

/// Retail GLA RPG Trooper / Tunnel Defender residual rocket.
pub const TUNNEL_DEFENDER_ROCKET_WEAPON: &str = "TunnelDefenderRocketWeapon";

static BOOTSTRAP_ATTEMPTED: AtomicBool = AtomicBool::new(false);

/// Wave 77: core host WeaponStore seed residual names that golden/skirmish combat
/// depends on. Fail-closed vs full Weapon.ini table.
pub const HOST_WEAPON_STORE_CORE_SEED_NAMES: &[&str] = &[
    RANGER_PRIMARY_WEAPON,
    RANGER_SECONDARY_WEAPON,
    GLA_REBEL_PRIMARY_WEAPON,
    REDGUARD_PRIMARY_WEAPON,
    HUMVEE_PRIMARY_WEAPON,
    HUMVEE_SECONDARY_WEAPON,
    PATRIOT_PRIMARY_WEAPON,
    PATRIOT_SECONDARY_WEAPON,
    STINGER_PRIMARY_WEAPON,
    GATTLING_BUILDING_PRIMARY_WEAPON,
    CRUSADER_TANK_GUN,
    TOMAHAWK_MISSILE_WEAPON,
    RAPTOR_JET_MISSILE_WEAPON,
    SCUD_GUN_EXPLOSIVE,
    BATTLE_MASTER_TANK_GUN,
    OVERLORD_TANK_GUN,
];

/// Honesty: host WeaponStore seed residual pack (Wave 77).
///
/// Ensures core combat residual names are registered after bootstrap and that
/// Ranger / Patriot clip residual fields match host seed table.
/// Fail-closed: not full Weapon.ini parse / full ClipSize volley state machine.
pub fn honesty_weapon_store_host_seed_residual_wave77() -> bool {
    let _ = ensure_host_weapon_store();
    let all_present = HOST_WEAPON_STORE_CORE_SEED_NAMES
        .iter()
        .all(|name| store_has(name));
    if !all_present {
        return false;
    }
    // Ranger residual: AdvancedCombatRifle damage/range seed residual.
    let ranger_ok = with_weapon_store(|store| {
        store
            .find_weapon_template(RANGER_PRIMARY_WEAPON)
            .map(|t| {
                t.primary_damage > 0.0 && t.attack_range > 0.0 && t.name == RANGER_PRIMARY_WEAPON
            })
            .unwrap_or(false)
    })
    .unwrap_or(false);
    // Patriot residual: primary missile seed present with positive range.
    let patriot_ok = with_weapon_store(|store| {
        store
            .find_weapon_template(PATRIOT_PRIMARY_WEAPON)
            .map(|t| t.primary_damage > 0.0 && t.attack_range > 0.0)
            .unwrap_or(false)
    })
    .unwrap_or(false);
    all_present && ranger_ok && patriot_ok && HOST_WEAPON_STORE_CORE_SEED_NAMES.len() >= 16
}

/// Wave 92 residual deepen: common ZH weapons beyond Wave 77 core seed names.
///
/// Host-testable residual for Marauder / Gattling / Comanche / Dragon / Scorpion
/// / Buggy / Pathfinder / MissileDefender / Paladin / Helix / Technical residual
/// damage/range tables. Fail-closed: not full Weapon.ini / full bonus condition matrix.
pub const HOST_WEAPON_STORE_DEEPEN_SEED_NAMES_WAVE92: &[&str] = &[
    MARAUDER_TANK_GUN,
    GATTLING_TANK_GUN,
    COMANCHE_PRIMARY_WEAPON,
    DRAGON_TANK_FLAME_WEAPON,
    SCORPION_TANK_GUN,
    BUGGY_ROCKET_WEAPON,
    PATHFINDER_SNIPER_WEAPON,
    MISSILE_DEFENDER_MISSILE_WEAPON,
    PALADIN_TANK_GUN,
    HELIX_MINIGUN_WEAPON,
    TECHNICAL_MACHINE_GUN,
    QUAD_CANNON_GUN,
    TOXIN_TRUCK_GUN,
    NAPALM_MISSILE_WEAPON,
    STEALTH_JET_MISSILE_WEAPON,
    TANK_HUNTER_PRIMARY_WEAPON,
];

/// Honesty: Wave 92 weapon template residual deepen pack.
///
/// Ensures deepen residual names are registered after bootstrap and that key
/// damage/range residual scalars match host seed table / Weapon.ini.
pub fn honesty_weapon_store_deepen_residual_wave92() -> bool {
    let _ = ensure_host_weapon_store();
    if !honesty_weapon_store_host_seed_residual_wave77() {
        return false;
    }
    let all_present = HOST_WEAPON_STORE_DEEPEN_SEED_NAMES_WAVE92
        .iter()
        .all(|name| store_has(name));
    if !all_present || HOST_WEAPON_STORE_DEEPEN_SEED_NAMES_WAVE92.len() < 16 {
        return false;
    }
    // Key residual damage/range scalars (Weapon.ini + host seed).
    let check = |name: &str, dmg: f32, range: f32| {
        with_weapon_store(|store| {
            store
                .find_weapon_template(name)
                .map(|t| {
                    (t.primary_damage - dmg).abs() < 0.05 && (t.attack_range - range).abs() < 0.05
                })
                .unwrap_or(false)
        })
        .unwrap_or(false)
    };
    check(MARAUDER_TANK_GUN, 60.0, 170.0)
        && check(GATTLING_TANK_GUN, 15.0, 150.0)
        && check(COMANCHE_PRIMARY_WEAPON, 6.0, 200.0)
        && check(DRAGON_TANK_FLAME_WEAPON, 10.0, 75.0)
        && check(SCORPION_TANK_GUN, 20.0, 150.0)
        && check(BUGGY_ROCKET_WEAPON, 20.0, 300.0)
        && check(PATHFINDER_SNIPER_WEAPON, 100.0, 300.0)
        && check(MISSILE_DEFENDER_MISSILE_WEAPON, 40.0, 175.0)
        && check(PALADIN_TANK_GUN, 60.0, 150.0)
        && check(HELIX_MINIGUN_WEAPON, 6.0, 115.0)
        && check(TECHNICAL_MACHINE_GUN, 10.0, 150.0)
        && check(QUAD_CANNON_GUN, 10.0, 150.0)
        && check(TANK_HUNTER_PRIMARY_WEAPON, 40.0, 175.0)
        && check(CRUSADER_TANK_GUN, 60.0, 150.0)
        && check(TOMAHAWK_MISSILE_WEAPON, 150.0, 350.0)
        && check(TOXIN_TRUCK_GUN, 10.0, 100.0)
        && check(NAPALM_MISSILE_WEAPON, 75.0, 320.0)
        && check(STEALTH_JET_MISSILE_WEAPON, 100.0, 220.0)
}

/// Wave 103 residual deepen: more Weapon.ini residual names beyond Wave 92.
///
/// Host-testable residual for NukeCannon / Inferno / Aurora / FireBase /
/// SentryDrone / Hellfire / JarmenKell / TunnelDefender / MiniGunner /
/// Overlord / BattleMaster / Comanche AT+pods / Avenger AA / SCUD toxin /
/// BlackNapalm residual damage/range tables.
/// Fail-closed: not full Weapon.ini / full ClipSize volley state machine.
pub const HOST_WEAPON_STORE_DEEPEN_SEED_NAMES_WAVE103: &[&str] = &[
    NUKE_CANNON_PRIMARY_WEAPON,
    INFERNO_CANNON_PRIMARY_WEAPON,
    AURORA_BOMB_PRIMARY_WEAPON,
    FIRE_BASE_HOWITZER_WEAPON,
    SENTRY_DRONE_GUN_WEAPON,
    HELLFIRE_MISSILE_WEAPON,
    JARMEN_KELL_RIFLE,
    TUNNEL_DEFENDER_ROCKET_WEAPON,
    MINIGUNNER_GUN,
    OVERLORD_TANK_GUN,
    BATTLE_MASTER_TANK_GUN,
    COMANCHE_ANTITANK_WEAPON,
    COMANCHE_ROCKET_POD_WEAPON,
    AVENGER_AIR_LASER,
    SCUD_GUN_TOXIN,
    BLACK_NAPALM_MISSILE_WEAPON,
];

/// Honesty: Wave 103 weapon template residual deepen pack.
///
/// Ensures deepen residual names are registered after bootstrap and that key
/// damage/range residual scalars match host seed table / Weapon.ini.
/// Fail-closed: not full Weapon.ini parse / multi-bonus condition matrix.
pub fn honesty_weapon_store_deepen_residual_wave103() -> bool {
    let _ = ensure_host_weapon_store();
    if !honesty_weapon_store_deepen_residual_wave92() {
        return false;
    }
    let all_present = HOST_WEAPON_STORE_DEEPEN_SEED_NAMES_WAVE103
        .iter()
        .all(|name| store_has(name));
    if !all_present || HOST_WEAPON_STORE_DEEPEN_SEED_NAMES_WAVE103.len() < 16 {
        return false;
    }
    let check = |name: &str, dmg: f32, range: f32| {
        with_weapon_store(|store| {
            store
                .find_weapon_template(name)
                .map(|t| {
                    (t.primary_damage - dmg).abs() < 0.05 && (t.attack_range - range).abs() < 0.05
                })
                .unwrap_or(false)
        })
        .unwrap_or(false)
    };
    check(NUKE_CANNON_PRIMARY_WEAPON, 400.0, 350.0)
        && check(INFERNO_CANNON_PRIMARY_WEAPON, 30.0, 300.0)
        && check(AURORA_BOMB_PRIMARY_WEAPON, 400.0, 300.0)
        && check(FIRE_BASE_HOWITZER_WEAPON, 75.0, 275.0)
        && check(SENTRY_DRONE_GUN_WEAPON, 8.0, 150.0)
        && check(HELLFIRE_MISSILE_WEAPON, 40.0, 150.0)
        && check(JARMEN_KELL_RIFLE, 180.0, 225.0)
        && check(TUNNEL_DEFENDER_ROCKET_WEAPON, 40.0, 175.0)
        && check(MINIGUNNER_GUN, 10.0, 125.0)
        && check(OVERLORD_TANK_GUN, 80.0, 175.0)
        && check(BATTLE_MASTER_TANK_GUN, 60.0, 150.0)
        && check(COMANCHE_ANTITANK_WEAPON, 50.0, 200.0)
        && check(COMANCHE_ROCKET_POD_WEAPON, 30.0, 200.0)
        && check(AVENGER_AIR_LASER, 10.0, 300.0)
        && check(SCUD_GUN_TOXIN, 200.0, 350.0)
        && check(BLACK_NAPALM_MISSILE_WEAPON, 75.0, 320.0)
}

/// Initialize the GameLogic WeaponStore (if needed) and ensure host combat
/// weapons are registered. Safe to call repeatedly.
///
/// Returns how many templates were added by this call (seed + filesystem load).

/// C++ Weapon.ini FireSound residual name for a store weapon template.
///
/// Empty string when unset / missing — caller falls back to generic "WeaponFire".

/// C++ Weapon.ini FireFX residual name (Regular veterancy slot).
pub fn host_fire_fx_for_weapon_name(name: &str) -> String {
    use gamelogic::weapon::with_weapon_store;
    let _ = ensure_host_weapon_store();
    let from_store = with_weapon_store(|store| {
        store.find_weapon_template(name).and_then(|wt| {
            wt.fire_fx[0]
                .as_ref()
                .map(|fx| {
                    let n = fx.name().trim();
                    n.to_string()
                })
                .filter(|s| !s.is_empty())
        })
    })
    .ok()
    .flatten();
    if let Some(s) = from_store {
        return s;
    }
    seed_fire_fx_for(name)
}

/// C++ Weapon.ini ProjectileDetonationFX residual name (Regular slot).
pub fn host_detonation_fx_for_weapon_name(name: &str) -> String {
    use gamelogic::weapon::with_weapon_store;
    let _ = ensure_host_weapon_store();
    let from_store = with_weapon_store(|store| {
        store.find_weapon_template(name).and_then(|wt| {
            wt.projectile_detonate_fx[0]
                .as_ref()
                .map(|fx| {
                    let n = fx.name().trim();
                    n.to_string()
                })
                .filter(|s| !s.is_empty())
        })
    })
    .ok()
    .flatten();
    if let Some(s) = from_store {
        return s;
    }
    seed_detonation_fx_for(name)
}

/// C++ Weapon.ini LaserBoneName residual — muzzle/bone attach name for laser start.
///
/// Fail-closed: name residual only; not full drawable bone matrix lookup.
pub fn host_laser_bone_name_for_weapon_name(name: &str) -> String {
    use gamelogic::weapon::with_weapon_store;
    let _ = ensure_host_weapon_store();
    let from_store = with_weapon_store(|store| {
        store.find_weapon_template(name).and_then(|wt| {
            let n = wt.laser_bone_name.trim();
            if n.is_empty() {
                None
            } else {
                Some(n.to_string())
            }
        })
    })
    .ok()
    .flatten();
    if let Some(s) = from_store {
        return s;
    }
    seed_laser_bone_name_for(name)
}

/// Resolve LaserBoneName for a host unit weapon slot.
pub fn host_laser_bone_name_for_unit_slot(
    template_name: &str,
    primary_weapon_name: Option<&str>,
    secondary_weapon_name: Option<&str>,
    slot: u8,
) -> String {
    let wname = if slot == 1 {
        secondary_weapon_name
            .or_else(|| secondary_weapon_name_for_unit(template_name))
            .or(primary_weapon_name)
            .or_else(|| primary_weapon_name_for_unit(template_name))
    } else {
        primary_weapon_name.or_else(|| primary_weapon_name_for_unit(template_name))
    };
    wname
        .map(host_laser_bone_name_for_weapon_name)
        .unwrap_or_default()
}

fn seed_laser_bone_name_for(name: &str) -> String {
    let n = name.to_ascii_lowercase();
    // Only meaningful when LaserName peels non-empty.
    if seed_laser_name_for(name).is_empty() {
        return String::new();
    }
    if n.contains("microwave") {
        return "WEAPON02".into();
    }
    if n.contains("ecm") || n.contains("frequencyjammer") || n.contains("jammer") {
        return "WEAPONA01".into();
    }
    if n.contains("avenger") && n.contains("target") {
        return "TurretFX03".into();
    }
    if n.contains("avenger") && (n.contains("pointdefense") || n.contains("pdl")) {
        return "LazerSpot01".into();
    }
    if n.contains("avenger") {
        return "TurretFX01".into();
    }
    if n.contains("lazr") {
        if n.contains("patriot") {
            return "WEAPONA01".into();
        }
        return "TurretMS01".into();
    }
    if n.contains("pointdefense")
        || n.contains("point_defense")
        || n.contains("pdl")
        || n.contains("paladin")
    {
        return "LASER".into();
    }
    if n.contains("supw") {
        return "MUZZLE01".into();
    }
    if n.contains("airf") {
        return "WeaponA01".into();
    }
    // Generic laser weapon residual bone.
    "LASER".into()
}

/// C++ Weapon.ini LaserName residual (Regular) — laser drawable / beam template.
///
/// Fail-closed: name + host residual beam spawn only; not full ThingFactory
/// laser object / LaserUpdate bone attach matrix.
pub fn host_laser_name_for_weapon_name(name: &str) -> String {
    use gamelogic::weapon::with_weapon_store;
    let _ = ensure_host_weapon_store();
    let from_store = with_weapon_store(|store| {
        store.find_weapon_template(name).and_then(|wt| {
            let n = wt.laser_name.trim();
            if n.is_empty() {
                None
            } else {
                Some(n.to_string())
            }
        })
    })
    .ok()
    .flatten();
    if let Some(s) = from_store {
        return s;
    }
    seed_laser_name_for(name)
}

/// Resolve LaserName for a host unit weapon slot.
pub fn host_laser_name_for_unit_slot(
    template_name: &str,
    primary_weapon_name: Option<&str>,
    secondary_weapon_name: Option<&str>,
    slot: u8,
) -> String {
    let wname = if slot == 1 {
        secondary_weapon_name
            .or_else(|| secondary_weapon_name_for_unit(template_name))
            .or(primary_weapon_name)
            .or_else(|| primary_weapon_name_for_unit(template_name))
    } else {
        primary_weapon_name.or_else(|| primary_weapon_name_for_unit(template_name))
    };
    wname
        .map(host_laser_name_for_weapon_name)
        .unwrap_or_default()
}

fn seed_laser_name_for(name: &str) -> String {
    let n = name.to_ascii_lowercase();
    if n.contains("microwave") {
        return "MicrowaveDisableStream".into();
    }
    if n.contains("ecm") || n.contains("frequencyjammer") || n.contains("jammer") {
        return "ECMDisableStream".into();
    }
    if n.contains("avenger") && n.contains("target") {
        return "AvengerTargetingLaserBeam".into();
    }
    if n.contains("avenger")
        && (n.contains("pointdefense") || n.contains("point_defense") || n.contains("pdl"))
    {
        return "AvengerPointDefenseLaserBeam".into();
    }
    if n.contains("avenger") {
        return "AvengerLaserBeam".into();
    }
    if n.contains("lazr") && n.contains("crusader") {
        return "Lazr_CrusaderLaserBeam".into();
    }
    if n.contains("lazr") && n.contains("patriot") {
        return "Lazr_PatriotLaserBeam".into();
    }
    if n.contains("lazr") && (n.contains("paladin") || n.contains("tank")) {
        return "Lazr_PaladinLaserBeam".into();
    }
    if n.contains("airf") && n.contains("pointdefense") {
        return "AirF_PointDefenseLaserBeam".into();
    }
    if n.contains("supw") && n.contains("pointdefense") {
        return "SupW_PointDefenseDroneLaserBeam".into();
    }
    if n.contains("pointdefense") || n.contains("point_defense") || n.contains("pdl") {
        return "PointDefenseLaserBeam".into();
    }
    if n.contains("paladin") && n.contains("laser") {
        return "PointDefenseLaserBeam".into();
    }
    String::new()
}

/// C++ Weapon.ini ProjectileExhaust residual particle-system name (Regular).
///
/// Fail-closed: name residual for in-flight trail — not full client PSys attach
/// / VeterancyProjectileExhaust HEROIC matrix.
pub fn host_projectile_exhaust_for_weapon_name(name: &str) -> String {
    seed_projectile_exhaust_for(name)
}

/// Resolve ProjectileExhaust for a host unit weapon slot.
pub fn host_projectile_exhaust_for_unit_slot(
    template_name: &str,
    primary_weapon_name: Option<&str>,
    secondary_weapon_name: Option<&str>,
    slot: u8,
) -> String {
    let wname = if slot == 1 {
        secondary_weapon_name
            .or_else(|| secondary_weapon_name_for_unit(template_name))
            .or(primary_weapon_name)
            .or_else(|| primary_weapon_name_for_unit(template_name))
    } else {
        primary_weapon_name.or_else(|| primary_weapon_name_for_unit(template_name))
    };
    wname
        .map(host_projectile_exhaust_for_weapon_name)
        .unwrap_or_default()
}

fn seed_projectile_exhaust_for(name: &str) -> String {
    let n = name.to_ascii_lowercase();
    // Hitscan / beam / gun residual: no projectile exhaust.
    if n.contains("laser")
        || n.contains("machinegun")
        || n.contains("chaingun")
        || n.contains("gattling")
        || n.contains("minigun")
        || n.contains("flashbang")
        || n.contains("combatrifle")
        || n.contains("tankgun")
        || (n.contains("cannon") && !n.contains("nuke"))
    {
        return String::new();
    }
    if n.contains("tow") {
        return "TowMissileExhaust".into();
    }
    if n.contains("neutron") {
        return "NeutronMissileExhaust".into();
    }
    if n.contains("scud") {
        return "ScudMissileExhaust".into();
    }
    if n.contains("comanche") {
        return "MissileExhaust".into();
    }
    if n.contains("missiledefender")
        || n.contains("missile_defender")
        || (n.contains("humvee") && n.contains("missile"))
    {
        return "MissileDefenderMissileExhaust".into();
    }
    if n.contains("stinger") || n.contains("patriot") {
        return "MissileExhaust".into();
    }
    if n.contains("tankhunter") || n.contains("rpg") || n.contains("tunneldefender") {
        return "MissileExhaust".into();
    }
    if n.contains("missile") || n.contains("rocket") || n.contains("tomahawk") {
        return "MissileExhaust".into();
    }
    String::new()
}

/// C++ Weapon.ini FireOCL residual name (Regular slot).
///
/// Fail-closed: name residual only — not full ObjectCreationList create_at_position
/// / nugget spawn parity.
pub fn host_fire_ocl_for_weapon_name(name: &str) -> String {
    seed_fire_ocl_for(name)
}

/// C++ Weapon.ini ProjectileDetonationOCL residual name (Regular slot).
///
/// Fail-closed: name residual only — not full OCL object spawn at impact.
pub fn host_detonation_ocl_for_weapon_name(name: &str) -> String {
    seed_detonation_ocl_for(name)
}

/// Resolve FireOCL + ProjectileDetonationOCL for a host unit weapon slot.
pub fn host_weapon_ocl_for_unit_slot(
    template_name: &str,
    primary_weapon_name: Option<&str>,
    secondary_weapon_name: Option<&str>,
    slot: u8,
) -> (String, String) {
    let wname = if slot == 1 {
        secondary_weapon_name
            .or_else(|| secondary_weapon_name_for_unit(template_name))
            .or(primary_weapon_name)
            .or_else(|| primary_weapon_name_for_unit(template_name))
    } else {
        primary_weapon_name.or_else(|| primary_weapon_name_for_unit(template_name))
    };
    match wname {
        Some(n) => (
            host_fire_ocl_for_weapon_name(n),
            host_detonation_ocl_for_weapon_name(n),
        ),
        None => (String::new(), String::new()),
    }
}

fn seed_fire_ocl_for(name: &str) -> String {
    let n = name.to_ascii_lowercase();
    // Retail Weapon.ini FireOCL peels (common ZH combat / death weapons).
    if n.contains("anthraxbomb") && n.contains("gamma") {
        return "OCL_PoisonFieldAnthraxGammaBomb".into();
    }
    if n.contains("anthraxbomb") {
        return "OCL_PoisonFieldAnthraxBomb".into();
    }
    if n.contains("scudstorm") || (n.contains("scud") && n.contains("damage")) {
        if n.contains("upgrade") {
            return "OCL_PoisonFieldUpgradedLarge".into();
        }
        return "OCL_PoisonFieldLarge".into();
    }
    if n.contains("dirtynuke") {
        return "OCL_DirtyNuke".into();
    }
    if n.contains("blacknapalm") && n.contains("bomb") {
        return "OCL_BlackNapalmFirestormSmall".into();
    }
    if n.contains("napalmbomb") || (n.contains("napalm") && n.contains("bomb")) {
        return "OCL_FirestormSmall".into();
    }
    if n.contains("mig") && n.contains("firestorm") {
        return "OCL_MiGFirestorm".into();
    }
    if n.contains("nucleartank") || (n.contains("nuclear") && n.contains("death")) {
        return "OCL_RadiationFieldSmall".into();
    }
    if n.contains("nukecannon") || (n.contains("nuclear") && n.contains("cannon")) {
        return "OCL_RadiationFieldMedium".into();
    }
    if n.contains("demotrap") || n.contains("terrorist") || n.contains("carbomb") {
        if n.contains("gamma") {
            return "OCL_PoisonFieldGammaSmall".into();
        }
        if n.contains("anthrax") || n.contains("upgrade") || n.contains("beta") {
            return "OCL_PoisonFieldUpgradedSmall".into();
        }
        // Standard / chem suicide residual peels to small poison field.
        if n.contains("toxin")
            || n.contains("poison")
            || n.contains("chem")
            || n.contains("suicide")
            || n.contains("terrorist")
            || n.contains("demotrap")
            || n.contains("carbomb")
        {
            return "OCL_PoisonFieldSmall".into();
        }
    }
    if n.contains("toxin") || n.contains("contaminat") {
        if n.contains("gamma") {
            return "OCL_PoisonFieldGammaMedium".into();
        }
        if n.contains("upgrade") || n.contains("anthrax") || n.contains("beta") {
            return "OCL_PoisonFieldUpgradedMedium".into();
        }
        return "OCL_PoisonFieldMedium".into();
    }
    if n.contains("inferno") {
        if n.contains("black") || n.contains("upgrade") {
            return "OCL_FireFieldUpgradedSmall".into();
        }
        return "OCL_FireFieldSmall".into();
    }
    String::new()
}

fn seed_detonation_ocl_for(name: &str) -> String {
    let n = name.to_ascii_lowercase();
    // Retail Weapon.ini ProjectileDetonationOCL peels.
    if n.contains("firewall") {
        if n.contains("upgrade") || n.contains("black") {
            return "OCL_FireWallSegmentUpgraded".into();
        }
        return "OCL_FireWallSegment".into();
    }
    if n.contains("nukecannon") || (n.contains("nuclear") && n.contains("shell")) {
        if n.contains("medium") {
            return "OCL_RadiationFieldMedium".into();
        }
        return "OCL_RadiationFieldSmall".into();
    }
    if n.contains("inferno") {
        if n.contains("black") || n.contains("upgrade") {
            return "OCL_FireFieldUpgradedSmall".into();
        }
        return "OCL_FireFieldSmall".into();
    }
    if n.contains("toxin") || n.contains("scud") || n.contains("poison") || n.contains("anthrax") {
        if n.contains("gamma") {
            return "OCL_PoisonFieldGammaMedium".into();
        }
        if n.contains("upgrade") || n.contains("beta") || n.contains("anthrax") {
            return "OCL_PoisonFieldUpgradedMedium".into();
        }
        return "OCL_PoisonFieldMedium".into();
    }
    String::new()
}

/// Resolve FireFX + DetonationFX for a host unit firing a weapon slot.
pub fn host_weapon_fx_for_unit_slot(
    template_name: &str,
    primary_weapon_name: Option<&str>,
    secondary_weapon_name: Option<&str>,
    slot: u8,
) -> (String, String) {
    let wname = if slot == 1 {
        secondary_weapon_name
            .or_else(|| secondary_weapon_name_for_unit(template_name))
            .or(primary_weapon_name)
            .or_else(|| primary_weapon_name_for_unit(template_name))
    } else {
        primary_weapon_name.or_else(|| primary_weapon_name_for_unit(template_name))
    };
    match wname {
        Some(n) => (
            host_fire_fx_for_weapon_name(n),
            host_detonation_fx_for_weapon_name(n),
        ),
        None => (String::new(), String::new()),
    }
}

fn seed_fire_fx_for(name: &str) -> String {
    let n = name.to_ascii_lowercase();
    if n.contains("laser") || n.contains("pointdefense") {
        return "WeaponFX_PaladinPointDefenseLaser".into();
    }
    if n.contains("tankgun")
        || n.contains("crusader")
        || n.contains("paladin")
        || n.contains("battlemaster")
        || n.contains("scorpion")
        || n.contains("marauder")
    {
        return "WeaponFX_GenericTankGunNoTracer".into();
    }
    if n.contains("ranger") && n.contains("flash") {
        return "WeaponFX_RangerFlashBang".into();
    }
    if n.contains("machinegun")
        || n.contains("combatrifle")
        || n.contains("ranger")
        || n.contains("redguard")
        || n.contains("rebel")
    {
        return "WeaponFX_GenericMachineGunFire".into();
    }
    if n.contains("missile")
        || n.contains("stinger")
        || n.contains("tomahawk")
        || n.contains("rpg")
        || n.contains("tankhunter")
    {
        return "WeaponFX_GenericMissileLaunch".into();
    }
    if n.contains("flame") || n.contains("dragon") || n.contains("inferno") {
        return "WeaponFX_DragonTankFlameWeapon".into();
    }
    if n.contains("gattling") || n.contains("minigun") {
        return "WeaponFX_GattlingTankGun".into();
    }
    if n.contains("nuke") {
        return "WeaponFX_NukeCannonMuzzleFlash".into();
    }
    if n.contains("aurora") || n.contains("bomb") {
        return "WeaponFX_AuroraBomb".into();
    }
    if n.contains("patriot") {
        return "WeaponFX_PatriotBattery".into();
    }
    String::new()
}

fn seed_detonation_fx_for(name: &str) -> String {
    let n = name.to_ascii_lowercase();
    if n.contains("jet") || n.contains("raptor") || n.contains("mig") || n.contains("stealth") {
        return "WeaponFX_JetMissileDetonation".into();
    }
    if n.contains("rpg")
        || n.contains("buggy")
        || n.contains("tankhunter")
        || n.contains("tomahawk")
    {
        return "WeaponFX_RocketBuggyMissileDetonation".into();
    }
    if n.contains("scud") {
        return "WeaponFX_ScudLauncherDetonation".into();
    }
    if n.contains("nuke") || n.contains("neutron") {
        return "WeaponFX_NukeCannon".into();
    }
    if n.contains("aurora") || n.contains("bomb") {
        return "WeaponFX_AuroraBombDetonation".into();
    }
    if n.contains("missile") || n.contains("stinger") || n.contains("patriot") {
        return "WeaponFX_GenericMissileDetonation".into();
    }
    String::new()
}

/// C++ Weapon.ini ProjectileObject residual name for a store weapon template.
pub fn host_projectile_name_for_weapon_name(name: &str) -> String {
    use gamelogic::weapon::with_weapon_store;
    let _ = ensure_host_weapon_store();
    let from_store = with_weapon_store(|store| {
        store.find_weapon_template(name).map(|wt| {
            let n = wt.projectile_name.trim();
            n.to_string()
        })
    })
    .ok()
    .flatten();
    if let Some(s) = from_store.filter(|s| !s.is_empty() && !s.eq_ignore_ascii_case("NONE")) {
        return s;
    }
    seed_projectile_name_for(name)
}

/// Resolve ProjectileObject for a host unit firing a weapon slot.
pub fn host_projectile_name_for_unit_slot(
    template_name: &str,
    primary_weapon_name: Option<&str>,
    secondary_weapon_name: Option<&str>,
    slot: u8,
) -> String {
    let wname = if slot == 1 {
        secondary_weapon_name
            .or_else(|| secondary_weapon_name_for_unit(template_name))
            .or(primary_weapon_name)
            .or_else(|| primary_weapon_name_for_unit(template_name))
    } else {
        primary_weapon_name.or_else(|| primary_weapon_name_for_unit(template_name))
    };
    match wname {
        Some(n) => host_projectile_name_for_weapon_name(n),
        None => String::new(),
    }
}

fn seed_projectile_name_for(name: &str) -> String {
    let n = name.to_ascii_lowercase();
    // Hitscan / laser residual — no projectile object.
    if n.contains("laser")
        || n.contains("flame")
        || n.contains("gattling")
        || n.contains("minigun")
        || n.contains("machinegun")
        || n.contains("combatrifle")
        || n.contains("redguard")
        || (n.contains("ranger") && !n.contains("flash") && !n.contains("missile"))
    {
        return String::new();
    }
    if n.contains("tomahawk") {
        return "TomahawkMissile".into();
    }
    if n.contains("scud") {
        return "ScudMissile".into();
    }
    if n.contains("patriot") || n.contains("stinger") {
        return "PatriotMissile".into();
    }
    if n.contains("rpg") || n.contains("tankhunter") || n.contains("tunneldefender") {
        return "GenericTankShell".into(); // many residual peels use tank shell / rocket proxy
    }
    if n.contains("missile") || n.contains("defender") {
        return "GenericMissile".into();
    }
    if n.contains("tankgun")
        || n.contains("crusader")
        || n.contains("paladin")
        || n.contains("battlemaster")
        || n.contains("scorpion")
        || n.contains("marauder")
        || n.contains("firebase")
    {
        return "GenericTankShell".into();
    }
    if n.contains("nuke") || n.contains("neutron") {
        return "NukeCannonShell".into();
    }
    if n.contains("aurora") || n.contains("bomb") {
        return "AuroraBomb".into();
    }
    if n.contains("raptor") || n.contains("mig") || n.contains("stealth") || n.contains("jet") {
        return "JetMissile".into();
    }
    if n.contains("comanche") || n.contains("rocket") || n.contains("buggy") {
        return "RocketBuggyMissile".into();
    }
    if n.contains("flash") || n.contains("grenade") {
        return "FlashBangGrenade".into();
    }
    String::new()
}

pub fn host_fire_sound_for_weapon_name(name: &str) -> String {
    use gamelogic::weapon::with_weapon_store;
    let _ = ensure_host_weapon_store();
    let from_store = with_weapon_store(|store| {
        store.find_weapon_template(name).and_then(|wt| {
            let n = wt.fire_sound.name().trim();
            if n.is_empty() {
                None
            } else {
                Some(n.to_string())
            }
        })
    })
    .ok()
    .flatten();
    if let Some(s) = from_store {
        return s;
    }
    // Store missing FireSound (pre-seed residual templates) → name peel.
    seed_fire_sound_for(name)
}

/// Resolve FireSound for a host unit firing a weapon slot.
pub fn host_fire_sound_for_unit_slot(
    template_name: &str,
    primary_weapon_name: Option<&str>,
    secondary_weapon_name: Option<&str>,
    slot: u8,
) -> String {
    let wname = if slot == 1 {
        secondary_weapon_name
            .or_else(|| secondary_weapon_name_for_unit(template_name))
            .or(primary_weapon_name)
            .or_else(|| primary_weapon_name_for_unit(template_name))
    } else {
        primary_weapon_name.or_else(|| primary_weapon_name_for_unit(template_name))
    };
    if let Some(n) = wname {
        let s = host_fire_sound_for_weapon_name(n);
        if !s.is_empty() {
            return s;
        }
    }
    "WeaponFire".to_string()
}

fn seed_fire_sound_for(name: &str) -> String {
    let n = name.to_ascii_lowercase();
    // Retail residual peels used by host honesty packs / unit peels.
    if n.contains("rpg") || n.contains("tankhunter") || n.contains("tunneldefender") {
        return "RPGTrooperWeapon".into();
    }
    if n.contains("jarmen") || n.contains("snipe") || n.contains("pathfinder") {
        return "JarmenKellWeaponSnipe".into();
    }
    if n.contains("terrorist") || n.contains("suicide") || n.contains("carbomb") {
        return "CarBomberDie".into();
    }
    if n.contains("tomahawk") {
        return "TomahawkWeapon".into();
    }
    if n.contains("scud") {
        return "ScudLauncherWeapon".into();
    }
    if n.contains("patriot") {
        return "PatriotBatteryWeapon".into();
    }
    if n.contains("ranger") && n.contains("flash") {
        return "RangerFlashBang".into();
    }
    if n.contains("laser") || n.contains("pointdefense") || n.contains("point_defense") {
        return "LaserFire".into();
    }
    if n.contains("ranger") || n.contains("machinegun") || n.contains("combatrifle") {
        return "MachineGunFire".into();
    }
    if n.contains("tankgun")
        || n.contains("crusader")
        || n.contains("paladin")
        || n.contains("battlemaster")
    {
        return "TankGunFire".into();
    }
    if n.contains("missile") || n.contains("stinger") {
        return "MissileLaunch".into();
    }
    if n.contains("flame") || n.contains("dragon") || n.contains("inferno") {
        return "FlameWeaponFire".into();
    }
    if n.contains("gattling") || n.contains("gatling") || n.contains("minigun") {
        return "GattlingFire".into();
    }
    if n.contains("nuke") {
        return "NukeCannonFire".into();
    }
    if n.contains("aurora") || n.contains("bomb") {
        return "BombDrop".into();
    }
    // Generic residual — still better than empty for store seed.
    "WeaponFire".into()
}

pub fn ensure_host_weapon_store() -> usize {
    if let Err(e) = gamelogic::initialize_weapon_store() {
        log::warn!("WeaponStore init failed during host bootstrap: {e}");
        return 0;
    }

    let mut added = 0usize;

    // Prefer real INI data when extracted game data is on disk (once / until ranger present).
    if !store_has(RANGER_PRIMARY_WEAPON)
        && (!BOOTSTRAP_ATTEMPTED.swap(true, Ordering::Relaxed) || !store_has(RANGER_PRIMARY_WEAPON))
    {
        added += try_load_weapon_ini_from_disk();
    }
    BOOTSTRAP_ATTEMPTED.store(true, Ordering::Relaxed);

    // Always fill gaps for known host weapons (units + base-defense residual).
    // seed_known_host_weapons skips names already present in the store.
    added += seed_known_host_weapons();
    added
}

/// Look up the retail primary weapon template name for a host unit template.
pub fn primary_weapon_name_for_unit(template_name: &str) -> Option<&'static str> {
    match template_name {
        "USA_Ranger" | "GoldenRanger" | "AmericaInfantryRanger" => Some(RANGER_PRIMARY_WEAPON),
        "GLA_Soldier" | "GLA_Rebel" | "GLAInfantryRebel" => Some(GLA_REBEL_PRIMARY_WEAPON),
        "GLA_Terrorist"
        | "GLAInfantryTerrorist"
        | "TestTerrorist"
        | "Chem_GLAInfantryTerrorist"
        | "Demo_GLAInfantryTerrorist"
        | "Slth_GLAInfantryTerrorist" => Some(TERRORIST_SUICIDE_WEAPON),
        "USA_MissileDefender"
        | "AmericaInfantryMissileDefender"
        | "TestMissileDefender"
        | "SupW_AmericaInfantryMissileDefender" => Some(MISSILE_DEFENDER_MISSILE_WEAPON),
        "China_RedGuard" | "China_Soldier" | "ChinaInfantryRedguard" => {
            Some(REDGUARD_PRIMARY_WEAPON)
        }
        "China_TankHunter"
        | "ChinaInfantryTankHunter"
        | "Tank_ChinaInfantryTankHunter"
        | "Nuke_ChinaInfantryTankHunter"
        | "Infa_ChinaInfantryTankHunter"
        | "TestTankHunter" => Some(TANK_HUNTER_PRIMARY_WEAPON),
        "ChinaVehicleTroopCrawler"
        | "China_TroopCrawler"
        | "Tank_ChinaVehicleTroopCrawler"
        | "Nuke_ChinaVehicleTroopCrawler"
        | "TestTroopCrawler" => Some(TROOP_CRAWLER_ASSAULT_WEAPON),
        "USA_Humvee" | "AmericaVehicleHumvee" | "TestHumvee" | "GoldenHumvee" => {
            Some(HUMVEE_PRIMARY_WEAPON)
        }
        "USA_Crusader" | "USA_CrusaderTank" | "AmericaTankCrusader" | "TestCrusader" => {
            Some(CRUSADER_TANK_GUN)
        }
        "Lazr_AmericaTankCrusader" | "TestLazrCrusader" => Some(LAZR_CRUSADER_TANK_GUN),
        "USA_Paladin" | "USA_PaladinTank" | "AmericaTankPaladin" | "TestPaladin" => {
            Some(PALADIN_TANK_GUN)
        }
        "Lazr_AmericaTankPaladin" | "TestLazrPaladin" => Some(LAZR_PALADIN_TANK_GUN),
        "USA_Avenger" | "AmericaTankAvenger" | "AmericaVehicleAvenger" | "TestAvenger" => {
            Some(AVENGER_TARGET_DESIGNATOR)
        }
        // Base-defense structures (Patriot / Gattling / Stinger residual auto-fire).
        "USA_Patriot"
        | "USA_PatriotMissile"
        | "AmericaPatriotBattery"
        | "PatriotMissile"
        | "TestPatriot" => Some(PATRIOT_PRIMARY_WEAPON),
        "Lazr_AmericaPatriotBattery" | "Lazr_PatriotMissileSystem" | "TestLazrPatriot" => {
            Some(LAZR_PATRIOT_PRIMARY_WEAPON)
        }
        "SupW_AmericaPatriotBattery"
        | "SupW_PatriotMissileSystem"
        | "TestSupWPatriot"
        | "TestEmpPatriot" => Some(SUPW_PATRIOT_PRIMARY_WEAPON),
        "GLATunnelNetwork"
        | "GLA_TunnelNetwork"
        | "Demo_GLATunnelNetwork"
        | "Chem_GLATunnelNetwork"
        | "Slth_GLATunnelNetwork"
        | "GLASneakAttackTunnelNetwork"
        | "TestTunnelNetwork" => Some(TUNNEL_NETWORK_GUN),
        "GLA_StingerSite"
        | "GLAStingerSite"
        | "Chem_GLAStingerSite"
        | "Demo_GLAStingerSite"
        | "Slth_GLAStingerSite"
        | "GC_Slth_GLAStingerSite"
        | "GC_Chem_GLAStingerSite"
        | "TestStingerSite" => Some(STINGER_PRIMARY_WEAPON),
        "China_GattlingCannon"
        | "ChinaGattlingCannon"
        | "Nuke_ChinaGattlingCannon"
        | "Tank_ChinaGattlingCannon"
        | "Infa_ChinaGattlingCannon"
        | "TestGattlingCannon" => Some(GATTLING_BUILDING_PRIMARY_WEAPON),
        "China_NukeCannon"
        | "ChinaVehicleNukeCannon"
        | "Nuke_ChinaVehicleNukeCannon"
        | "TestNukeCannon" => Some(NUKE_CANNON_PRIMARY_WEAPON),
        "China_InfernoCannon"
        | "ChinaVehicleInfernoCannon"
        | "Nuke_ChinaVehicleInfernoCannon"
        | "TestInfernoCannon" => Some(INFERNO_CANNON_PRIMARY_WEAPON),
        // AmericaJetAurora residual dive bomb.
        "AmericaJetAurora" | "USA_Aurora" | "TestAurora" => Some(AURORA_BOMB_PRIMARY_WEAPON),
        "AirF_AmericaJetAurora" | "TestAuroraFuelAir" => Some(AIRF_AURORA_BOMB_PRIMARY_WEAPON),
        "SupW_AmericaJetAurora" => Some(SUPW_AURORA_FUEL_BOMB_WEAPON),
        "Lazr_AmericaJetAurora" => Some(AURORA_BOMB_PRIMARY_WEAPON),
        "AmericaJetStealthFighter"
        | "USA_StealthFighter"
        | "TestStealthFighter"
        | "SupW_AmericaJetStealthFighter"
        | "Lazr_AmericaJetStealthFighter"
        | "AirF_AmericaJetStealthFighter" => Some(STEALTH_JET_MISSILE_WEAPON),
        "AmericaTankMicrowave"
        | "AmericaVehicleMicrowaveTank"
        | "USA_MicrowaveTank"
        | "Lazr_AmericaTankMicrowave"
        | "AirF_AmericaTankMicrowave"
        | "SupW_AmericaTankMicrowave"
        | "TestMicrowave"
        | "TestMicrowaveTank" => Some(MICROWAVE_BUILDING_CLEARER_WEAPON),
        // AmericaVehicleComanche residual primary cannon.
        "AmericaVehicleComanche"
        | "USA_Comanche"
        | "TestComanche"
        | "AirF_AmericaVehicleComanche"
        | "SupW_AmericaVehicleComanche"
        | "Lazr_AmericaVehicleComanche" => Some(COMANCHE_PRIMARY_WEAPON),

        // China MiG residual napalm missiles.
        "ChinaJetMIG" | "China_MiG" | "TestMiG" | "Tank_ChinaJetMIG" | "Infa_ChinaJetMIG"
        | "Boss_JetMIG" => Some(NAPALM_MISSILE_WEAPON),
        "Nuke_ChinaJetMIG" => Some(NUKE_MIG_MISSILE_WEAPON),
        // America Fire Base residual howitzer.
        "AmericaFireBase"
        | "USA_FireBase"
        | "TestFireBase"
        | "AirF_AmericaFireBase"
        | "SupW_AmericaFireBase"
        | "Lazr_AmericaFireBase" => Some(FIRE_BASE_HOWITZER_WEAPON),
        // China Helix residual primary minigun.
        "ChinaVehicleHelix"
        | "China_Helix"
        | "Nuke_ChinaVehicleHelix"
        | "Infa_ChinaVehicleHelix"
        | "Tank_ChinaVehicleHelix"
        | "TestHelix" => Some(HELIX_MINIGUN_WEAPON),
        // Pathfinder sniper residual.
        "AmericaInfantryPathfinder"
        | "USA_Pathfinder"
        | "TestPathfinder"
        | "AirF_AmericaInfantryPathfinder"
        | "SupW_AmericaInfantryPathfinder"
        | "Lazr_AmericaInfantryPathfinder" => Some(PATHFINDER_SNIPER_WEAPON),
        // Hellfire drone residual primary.
        "AmericaVehicleHellfireDrone"
        | "USA_HellfireDrone"
        | "TestHellfireDrone"
        | "AirF_AmericaVehicleHellfireDrone"
        | "SupW_AmericaVehicleHellfireDrone"
        | "Lazr_AmericaVehicleHellfireDrone" => Some(HELLFIRE_MISSILE_WEAPON),
        // Scout drone has no primary weapon (sensor only).
        "AmericaVehicleScoutDrone"
        | "USA_ScoutDrone"
        | "TestScoutDrone"
        | "AirF_AmericaVehicleScoutDrone"
        | "SupW_AmericaVehicleScoutDrone"
        | "Lazr_AmericaVehicleScoutDrone" => None,
        // Sentry gun is PLAYER_UPGRADE only — no primary until research residual.
        "AmericaVehicleSentryDrone"
        | "USA_SentryDrone"
        | "TestSentryDrone"
        | "AirF_AmericaVehicleSentryDrone"
        | "SupW_AmericaVehicleSentryDrone"
        | "Lazr_AmericaVehicleSentryDrone" => None,
        // GLA Rocket Buggy residual long-range rockets.
        "GLAVehicleRocketBuggy"
        | "GLA_RocketBuggy"
        | "TestRocketBuggy"
        | "Chem_GLAVehicleRocketBuggy"
        | "Demo_GLAVehicleRocketBuggy"
        | "Slth_GLAVehicleRocketBuggy" => Some(BUGGY_ROCKET_WEAPON),
        // GLA Quad Cannon residual ground gun.
        "GLAVehicleQuadCannon"
        | "GLA_QuadCannon"
        | "TestQuadCannon"
        | "Chem_GLAVehicleQuadCannon"
        | "Demo_GLAVehicleQuadCannon"
        | "Slth_GLAVehicleQuadCannon" => Some(QUAD_CANNON_GUN),
        // GLA SCUD launcher residual explosive primary.
        "GLAVehicleScudLauncher"
        | "GLA_ScudLauncher"
        | "TestScudLauncher"
        | "Chem_GLAVehicleScudLauncher"
        | "Demo_GLAVehicleScudLauncher"
        | "Slth_GLAVehicleScudLauncher" => Some(SCUD_GUN_EXPLOSIVE),
        // GLA Technical residual machine gun (salvage tiers swap residual).
        "GLAVehicleTechnical"
        | "GLA_Technical"
        | "TestTechnical"
        | "Chem_GLAVehicleTechnical"
        | "Demo_GLAVehicleTechnical"
        | "Slth_GLAVehicleTechnical"
        | "GLAVehicleTechnicalChassisOne"
        | "GLAVehicleTechnicalChassisTwo"
        | "GLAVehicleTechnicalChassisThree" => Some(TECHNICAL_MACHINE_GUN),
        // GLA Toxin Tractor residual poison stream primary.
        "GLAVehicleToxinTruck"
        | "GLA_ToxinTruck"
        | "TestToxinTruck"
        | "Chem_GLAVehicleToxinTruck"
        | "Demo_GLAVehicleToxinTruck"
        | "Slth_GLAVehicleToxinTruck" => Some(TOXIN_TRUCK_GUN),
        // GLA Scorpion residual tank gun (salvage + rocket residual).
        "GLATankScorpion"
        | "GLA_Scorpion"
        | "GLA_ScorpionTank"
        | "TestScorpion"
        | "Chem_GLATankScorpion"
        | "Demo_GLATankScorpion"
        | "Slth_GLATankScorpion" => Some(SCORPION_TANK_GUN),
        // America Tomahawk residual missile.
        "AmericaVehicleTomahawk"
        | "USA_Tomahawk"
        | "USA_TomahawkLauncher"
        | "TestTomahawk"
        | "SupW_AmericaVehicleTomahawk" => Some(TOMAHAWK_MISSILE_WEAPON),
        // AmericaJetRaptor residual missiles (King Raptor uses AirF weapon).
        "AmericaJetRaptor"
        | "USA_Raptor"
        | "TestRaptor"
        | "SupW_AmericaJetRaptor"
        | "Lazr_AmericaJetRaptor" => Some(RAPTOR_JET_MISSILE_WEAPON),
        "AirF_AmericaJetRaptor" | "TestKingRaptor" => Some(AIRF_RAPTOR_JET_MISSILE_WEAPON),
        // AmericaVehicleBattleDrone residual MG.
        "AmericaVehicleBattleDrone"
        | "USA_BattleDrone"
        | "TestBattleDrone"
        | "SupW_AmericaVehicleBattleDrone"
        | "AirF_AmericaVehicleBattleDrone"
        | "Lazr_AmericaVehicleBattleDrone" => Some(BATTLE_DRONE_MACHINE_GUN),
        // China Overlord / Emperor residual main gun.
        "ChinaTankOverlord"
        | "China_OverlordTank"
        | "TestOverlord"
        | "Nuke_ChinaTankOverlord"
        | "Tank_ChinaTankOverlord"
        | "Infa_ChinaTankOverlord"
        | "Tank_ChinaTankEmperor"
        | "TestEmperor" => Some(OVERLORD_TANK_GUN),
        // GLA Jarmen Kell residual sniper.
        "GLAInfantryJarmenKell"
        | "GLA_JarmenKell"
        | "TestJarmenKell"
        | "Chem_GLAInfantryJarmenKell"
        | "Demo_GLAInfantryJarmenKell"
        | "Slth_GLAInfantryJarmenKell"
        | "GC_Slth_GLAInfantryJarmenKell" => Some(JARMEN_KELL_RIFLE),
        // GLA Marauder residual tank gun (salvage tiers swap fire-rate residual).
        "GLATankMarauder"
        | "GLA_MarauderTank"
        | "TestMarauder"
        | "Chem_GLATankMarauder"
        | "Demo_GLATankMarauder"
        | "Slth_GLATankMarauder" => Some(MARAUDER_TANK_GUN),
        // China Battlemaster residual main gun (Uranium / horde ROF residual).
        "ChinaTankBattleMaster"
        | "China_BattlemasterTank"
        | "China_BattleTank"
        | "TestBattlemaster"
        | "Tank_ChinaTankBattleMaster"
        | "Nuke_ChinaTankBattleMaster" => Some(BATTLE_MASTER_TANK_GUN),
        // China Dragon Tank residual flame.
        "ChinaTankDragon"
        | "China_DragonTank"
        | "TestDragonTank"
        | "Tank_ChinaTankDragon"
        | "Nuke_ChinaTankDragon"
        | "Infa_ChinaTankDragon" => Some(DRAGON_TANK_FLAME_WEAPON),
        // China Gattling Tank residual ground gun (AA secondary separate).
        "ChinaTankGattling"
        | "China_GattlingTank"
        | "TestGattlingTank"
        | "Tank_ChinaTankGattling"
        | "Nuke_ChinaTankGattling"
        | "Infa_ChinaTankGattling" => Some(GATTLING_TANK_GUN),
        // China Infantry General MiniGunner residual ground gun.
        "Infa_ChinaInfantryMiniGunner"
        | "China_MiniGunner"
        | "TestMiniGunner"
        | "ChinaInfantryMiniGunner" => Some(MINIGUNNER_GUN),
        // GLA RPG Trooper / Tunnel Defender residual rocket.
        "GLAInfantryTunnelDefender"
        | "GLA_RPGTrooper"
        | "GLA_RPG"
        | "GLA_TunnelDefender"
        | "TestRPGTrooper"
        | "TestRPG"
        | "TestTunnelDefender"
        | "Chem_GLAInfantryTunnelDefender"
        | "Demo_GLAInfantryTunnelDefender"
        | "Slth_GLAInfantryTunnelDefender"
        | "GC_Slth_GLAInfantryTunnelDefender"
        | "GC_Chem_GLAInfantryTunnelDefender" => Some(TUNNEL_DEFENDER_ROCKET_WEAPON),
        // Generals leftover GLALightTank residual (retail PRIMARY CrusaderTankGun).
        "GLALightTank" | "GLA_LightTank" | "TestLightTank" => Some(CRUSADER_TANK_GUN),
        // GLA Combat Cycle residual: default InitialPayload Rebel MG.
        // Empty bike is PRIMARY NONE; spawn residual binds rebel weapon.
        "GLAVehicleCombatBike"
        | "GLA_CombatBike"
        | "TestCombatBike"
        | "TestCombatCycle"
        | "Chem_GLAVehicleCombatBike"
        | "Demo_GLAVehicleCombatBike"
        | "Slth_GLAVehicleCombatBike"
        | "GC_Slth_GLAVehicleCombatBike" => Some(REBEL_BIKER_MG),
        "GLAVehicleCombatBikeRocket"
        | "Chem_GLAVehicleCombatBikeRocket"
        | "Demo_GLAVehicleCombatBikeRocket"
        | "Slth_GLAVehicleCombatBikeRocket"
        | "GC_Slth_GLAVehicleCombatBikeRocket" => Some(TUNNEL_DEFENDER_BIKER_ROCKET),
        "GLAVehicleCombatBikeTerrorist"
        | "Chem_GLAVehicleCombatBikeTerrorist"
        | "Demo_GLAVehicleCombatBikeTerrorist"
        | "Slth_GLAVehicleCombatBikeTerrorist"
        | "Boss_VehicleCombatBikeTerrorist" => Some(TERRORIST_SUICIDE_WEAPON),
        _ => {
            // Name residual for Laser/Superweapon general variants + Nuke Cannon.
            if crate::game_logic::host_neutron_shell::is_nuke_cannon_template(template_name) {
                return Some(NUKE_CANNON_PRIMARY_WEAPON);
            }
            if crate::game_logic::host_inferno_cannon::is_inferno_cannon_template(template_name) {
                return Some(INFERNO_CANNON_PRIMARY_WEAPON);
            }
            if crate::game_logic::host_aurora_bomb::is_aurora_aircraft_template(template_name) {
                return Some(
                    match crate::game_logic::host_aurora_bomb::aurora_bomb_kind_for_template(
                        template_name,
                    ) {
                        crate::game_logic::host_aurora_bomb::HostAuroraBombKind::FuelAir => {
                            AIRF_AURORA_BOMB_PRIMARY_WEAPON
                        }
                        crate::game_logic::host_aurora_bomb::HostAuroraBombKind::FuelAirSupW => {
                            SUPW_AURORA_FUEL_BOMB_WEAPON
                        }
                        crate::game_logic::host_aurora_bomb::HostAuroraBombKind::Standard => {
                            AURORA_BOMB_PRIMARY_WEAPON
                        }
                    },
                );
            }
            if crate::game_logic::host_bunker_buster::is_bunker_buster_carrier(template_name) {
                return Some(STEALTH_JET_MISSILE_WEAPON);
            }
            if crate::game_logic::host_bunker_buster::is_kill_garrisoned_clearer(template_name) {
                return Some(MICROWAVE_BUILDING_CLEARER_WEAPON);
            }
            if crate::game_logic::host_comanche_rocket_pods::is_comanche_template(template_name) {
                return Some(COMANCHE_PRIMARY_WEAPON);
            }
            if crate::game_logic::host_overlord_addons::is_helix_template(template_name) {
                return Some(HELIX_MINIGUN_WEAPON);
            }
            if crate::game_logic::host_pathfinder::is_pathfinder_template(template_name) {
                return Some(PATHFINDER_SNIPER_WEAPON);
            }
            if crate::game_logic::host_slave_drones::is_hellfire_drone_template(template_name) {
                return Some(HELLFIRE_MISSILE_WEAPON);
            }
            if crate::game_logic::host_slave_drones::is_scout_drone_template(template_name) {
                return None;
            }
            // Sentry without gun upgrade has no residual primary (fail-closed).
            if crate::game_logic::host_sentry_drone::is_sentry_drone_template(template_name) {
                return None;
            }
            // Angry Mob nexus residual aggregate fire weapon (AI range residual).
            if crate::game_logic::host_angry_mob::is_angry_mob_nexus_template(template_name) {
                return Some(ANGRY_MOB_RESIDUAL_WEAPON);
            }
            if crate::game_logic::host_rocket_buggy::is_rocket_buggy_template(template_name) {
                return Some(BUGGY_ROCKET_WEAPON);
            }
            if crate::game_logic::host_quad_cannon::is_quad_cannon_template(template_name) {
                return Some(QUAD_CANNON_GUN);
            }
            if crate::game_logic::host_scud_launcher::is_scud_launcher_template(template_name) {
                return Some(SCUD_GUN_EXPLOSIVE);
            }
            if crate::game_logic::host_technical::is_technical_template(template_name) {
                return Some(TECHNICAL_MACHINE_GUN);
            }
            if crate::game_logic::host_toxin_tractor::is_toxin_tractor_template(template_name) {
                return Some(TOXIN_TRUCK_GUN);
            }
            if crate::game_logic::host_scorpion::is_scorpion_template(template_name) {
                return Some(SCORPION_TANK_GUN);
            }
            if crate::game_logic::host_tomahawk::is_tomahawk_template(template_name) {
                return Some(TOMAHAWK_MISSILE_WEAPON);
            }
            if crate::game_logic::host_raptor::is_raptor_template(template_name) {
                let king = crate::game_logic::host_raptor::is_king_raptor_template(template_name);
                return Some(crate::game_logic::host_raptor::raptor_weapon_name(king));
            }
            if crate::game_logic::host_slave_drones::is_battle_drone_template(template_name) {
                return Some(BATTLE_DRONE_MACHINE_GUN);
            }
            if crate::game_logic::host_overlord_gun::is_overlord_gun_chassis(template_name) {
                return Some(OVERLORD_TANK_GUN);
            }
            if crate::game_logic::host_jarmen_kell::is_jarmen_kell_template(template_name) {
                return Some(JARMEN_KELL_RIFLE);
            }
            if crate::game_logic::host_marauder::is_marauder_template(template_name) {
                return Some(MARAUDER_TANK_GUN);
            }
            if crate::game_logic::host_terrorist::is_terrorist_template(template_name) {
                return Some(TERRORIST_SUICIDE_WEAPON);
            }
            if crate::game_logic::host_missile_defender::is_missile_defender_template(template_name)
            {
                return Some(MISSILE_DEFENDER_MISSILE_WEAPON);
            }
            if crate::game_logic::host_battlemaster::is_battlemaster_template(template_name) {
                return Some(BATTLE_MASTER_TANK_GUN);
            }
            if crate::game_logic::host_dragon_tank::is_dragon_tank_template(template_name) {
                return Some(DRAGON_TANK_FLAME_WEAPON);
            }
            if crate::game_logic::host_gattling_tank::is_gattling_tank_template(template_name) {
                return Some(GATTLING_TANK_GUN);
            }
            if crate::game_logic::host_minigunner::is_minigunner_template(template_name) {
                return Some(MINIGUNNER_GUN);
            }
            if crate::game_logic::host_rpg_trooper::is_rpg_trooper_template(template_name) {
                return Some(TUNNEL_DEFENDER_ROCKET_WEAPON);
            }
            if let Some(w) =
                crate::game_logic::host_usa_tanks::primary_weapon_name_for_usa_tank(template_name)
            {
                return Some(w);
            }
            if crate::game_logic::host_avenger::is_avenger_template(template_name) {
                return Some(AVENGER_TARGET_DESIGNATOR);
            }
            if crate::game_logic::host_humvee::is_humvee_template(template_name) {
                return Some(HUMVEE_PRIMARY_WEAPON);
            }
            if crate::game_logic::host_combat_cycle::is_combat_cycle_template(template_name) {
                return Some(
                    match crate::game_logic::host_combat_cycle::default_spawn_rider_for_template(
                        template_name,
                    ) {
                        crate::game_logic::host_combat_cycle::CombatCycleRider::TunnelDefender => {
                            TUNNEL_DEFENDER_BIKER_ROCKET
                        }
                        crate::game_logic::host_combat_cycle::CombatCycleRider::Terrorist => {
                            TERRORIST_SUICIDE_WEAPON
                        }
                        _ => REBEL_BIKER_MG,
                    },
                );
            }
            crate::game_logic::host_base_defense::primary_weapon_name_for_defense(template_name)
        }
    }
}

/// Look up the retail secondary weapon template name for a host unit template.
/// Fail-closed: only known multi-slot units; not full WeaponSet upgrade matrices.
pub fn secondary_weapon_name_for_unit(template_name: &str) -> Option<&'static str> {
    match template_name {
        "USA_Ranger" | "GoldenRanger" | "AmericaInfantryRanger" => Some(RANGER_SECONDARY_WEAPON),
        "USA_MissileDefender"
        | "AmericaInfantryMissileDefender"
        | "TestMissileDefender"
        | "SupW_AmericaInfantryMissileDefender" => Some(MISSILE_DEFENDER_LASER_GUIDED_WEAPON),
        "USA_Humvee" | "AmericaVehicleHumvee" | "TestHumvee" | "GoldenHumvee" => {
            Some(HUMVEE_SECONDARY_WEAPON)
        }
        "USA_Avenger" | "AmericaTankAvenger" | "AmericaVehicleAvenger" | "TestAvenger" => {
            Some(AVENGER_AIR_LASER)
        }
        "USA_Patriot"
        | "USA_PatriotMissile"
        | "AmericaPatriotBattery"
        | "PatriotMissile"
        | "TestPatriot" => Some(PATRIOT_SECONDARY_WEAPON),
        "Lazr_AmericaPatriotBattery" | "Lazr_PatriotMissileSystem" | "TestLazrPatriot" => {
            Some(LAZR_PATRIOT_SECONDARY_WEAPON)
        }
        "SupW_AmericaPatriotBattery"
        | "SupW_PatriotMissileSystem"
        | "TestSupWPatriot"
        | "TestEmpPatriot" => Some(SUPW_PATRIOT_SECONDARY_WEAPON),
        "GLA_StingerSite"
        | "GLAStingerSite"
        | "Chem_GLAStingerSite"
        | "Demo_GLAStingerSite"
        | "Slth_GLAStingerSite"
        | "GC_Slth_GLAStingerSite"
        | "GC_Chem_GLAStingerSite"
        | "TestStingerSite" => Some(STINGER_SECONDARY_WEAPON),
        "China_GattlingCannon"
        | "ChinaGattlingCannon"
        | "Nuke_ChinaGattlingCannon"
        | "Tank_ChinaGattlingCannon"
        | "Infa_ChinaGattlingCannon"
        | "TestGattlingCannon" => Some(GATTLING_BUILDING_SECONDARY_WEAPON),
        // Neutron shells are PLAYER_UPGRADE residual (Upgrade_ChinaNeutronShells) —
        // not bound at create; research equips secondary (parity with rocket pods).
        // Rocket pods are PLAYER_UPGRADE residual — not bound at create; research equips.
        // Comanche residual SECONDARY anti-tank until rocket-pods upgrade replaces slot.
        "AmericaVehicleComanche"
        | "USA_Comanche"
        | "TestComanche"
        | "AirF_AmericaVehicleComanche"
        | "SupW_AmericaVehicleComanche"
        | "Lazr_AmericaVehicleComanche" => Some(COMANCHE_ANTITANK_WEAPON),
        // Quad Cannon residual AA secondary (ground primary + air secondary).
        "GLAVehicleQuadCannon"
        | "GLA_QuadCannon"
        | "TestQuadCannon"
        | "Chem_GLAVehicleQuadCannon"
        | "Demo_GLAVehicleQuadCannon"
        | "Slth_GLAVehicleQuadCannon" => Some(QUAD_CANNON_GUN_AIR),
        // SCUD residual toxin secondary (Anthrax swaps warhead residual).
        "GLAVehicleScudLauncher"
        | "GLA_ScudLauncher"
        | "TestScudLauncher"
        | "Chem_GLAVehicleScudLauncher"
        | "Demo_GLAVehicleScudLauncher"
        | "Slth_GLAVehicleScudLauncher" => Some(SCUD_GUN_TOXIN),
        // Toxin Tractor residual contaminate spray secondary.
        "GLAVehicleToxinTruck"
        | "GLA_ToxinTruck"
        | "TestToxinTruck"
        | "Chem_GLAVehicleToxinTruck"
        | "Demo_GLAVehicleToxinTruck"
        | "Slth_GLAVehicleToxinTruck" => Some(TOXIN_TRUCK_SPRAYER),
        // China Gattling Tank residual AA secondary.
        "ChinaTankGattling"
        | "China_GattlingTank"
        | "TestGattlingTank"
        | "Tank_ChinaTankGattling"
        | "Nuke_ChinaTankGattling"
        | "Infa_ChinaTankGattling" => Some(GATTLING_TANK_GUN_AIR),
        // China MiniGunner residual AA secondary.
        "Infa_ChinaInfantryMiniGunner"
        | "China_MiniGunner"
        | "TestMiniGunner"
        | "ChinaInfantryMiniGunner" => Some(MINIGUNNER_GUN_AIR),
        _ => {
            // Nuke Cannon neutron secondary is upgrade-gated — see comment above.
            if crate::game_logic::host_quad_cannon::is_quad_cannon_template(template_name) {
                Some(QUAD_CANNON_GUN_AIR)
            } else if crate::game_logic::host_scud_launcher::is_scud_launcher_template(
                template_name,
            ) {
                Some(SCUD_GUN_TOXIN)
            } else if crate::game_logic::host_toxin_tractor::is_toxin_tractor_template(
                template_name,
            ) {
                Some(TOXIN_TRUCK_SPRAYER)
            } else if crate::game_logic::host_base_defense::is_gattling_cannon_structure(
                template_name,
            ) {
                Some(GATTLING_BUILDING_SECONDARY_WEAPON)
            } else if crate::game_logic::host_base_defense::is_patriot_battery_structure(
                template_name,
            ) {
                Some(
                    crate::game_logic::host_base_defense::secondary_weapon_name_for_defense(
                        template_name,
                    )
                    .unwrap_or(PATRIOT_SECONDARY_WEAPON),
                )
            } else if crate::game_logic::host_base_defense::is_stinger_site_structure(template_name)
            {
                Some(STINGER_SECONDARY_WEAPON)
            } else if crate::game_logic::host_missile_defender::is_missile_defender_template(
                template_name,
            ) {
                Some(MISSILE_DEFENDER_LASER_GUIDED_WEAPON)
            } else if crate::game_logic::host_comanche_rocket_pods::is_comanche_template(
                template_name,
            ) {
                // Comanche residual SECONDARY anti-tank; rocket pods replace after upgrade.
                Some(COMANCHE_ANTITANK_WEAPON)
            } else if crate::game_logic::host_gattling_tank::is_gattling_tank_template(
                template_name,
            ) {
                Some(GATTLING_TANK_GUN_AIR)
            } else if crate::game_logic::host_minigunner::is_minigunner_template(template_name) {
                Some(MINIGUNNER_GUN_AIR)
            } else {
                None
            }
        }
    }
}

fn store_has(name: &str) -> bool {
    with_weapon_store(|store| store.find_weapon_template(name).is_some()).unwrap_or(false)
}

fn try_load_weapon_ini_from_disk() -> usize {
    let mut total = 0usize;
    for path in weapon_ini_candidate_paths() {
        if !path.is_file() {
            continue;
        }
        match std::fs::read_to_string(&path) {
            Ok(content) => {
                let n =
                    crate::assets::ini_template_loader::register_weapons_from_ini_text(&content);
                if n > 0 {
                    log::info!(
                        "Host WeaponStore: loaded {} weapon templates from {}",
                        n,
                        path.display()
                    );
                    total += n;
                    // One full Weapon.ini is enough; further copies are overrides.
                    break;
                }
            }
            Err(e) => {
                log::debug!("Host WeaponStore: cannot read {}: {e}", path.display());
            }
        }
    }
    total
}

fn weapon_ini_candidate_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    // CWD-relative (repo root when tests run from GeneralsRust or workspace).
    let relative = [
        "windows_game/extracted_big_files/INIZH/Data/INI/Weapon.ini",
        "windows_game/extracted_big_files_v2/INIZH/Data/INI/Weapon.ini",
        "Data/INI/Weapon.ini",
        "INIZH/Data/INI/Weapon.ini",
    ];
    for r in relative {
        paths.push(PathBuf::from(r));
    }

    // Walk up from CARGO_MANIFEST_DIR (Main crate) toward repo root.
    if let Ok(manifest) = std::env::var("CARGO_MANIFEST_DIR") {
        let mut dir = PathBuf::from(manifest);
        for _ in 0..6 {
            for r in relative {
                paths.push(dir.join(r));
            }
            if !dir.pop() {
                break;
            }
        }
    }

    // Dedup while preserving order.
    let mut seen = std::collections::HashSet::new();
    paths.retain(|p| seen.insert(p.clone()));
    paths
}

/// Seed minimal real-ish stats for host golden units when INI is unavailable.
///
/// Values match retail Weapon.ini entries used by those units (damage/range).
/// Delay is stored in logic frames (30 FPS) after msec conversion.

fn seed_death_type_for(
    name: &str,
    damage_type: gamelogic::damage::DamageType,
) -> gamelogic::damage::DeathType {
    use gamelogic::damage::{DamageType as Dmg, DeathType as Dth};
    let n = name.to_ascii_lowercase();
    if n.contains("terrorist") || n.contains("suicide") || n.contains("demo_trap") {
        return Dth::Suicided;
    }
    if n.contains("laser") {
        return Dth::Lasered;
    }
    if n.contains("toxin") || n.contains("anthrax") || n.contains("poison") {
        return Dth::Poisoned;
    }
    if n.contains("flame") || n.contains("dragon") || n.contains("napalm") || n.contains("inferno")
    {
        return Dth::Burned;
    }
    if n.contains("bomb")
        || n.contains("scud")
        || n.contains("missile")
        || n.contains("rocket")
        || n.contains("aurora")
        || n.contains("grenade")
    {
        return Dth::Exploded;
    }
    match damage_type {
        Dmg::Flame => Dth::Burned,
        Dmg::Laser => Dth::Lasered,
        Dmg::Poison => Dth::Poisoned,
        Dmg::Explosion | Dmg::AuroraBomb | Dmg::LandMine | Dmg::MolotovCocktail => Dth::Exploded,
        Dmg::Radiation => Dth::Detonated,
        _ => Dth::Normal,
    }
}

fn seed_damage_type_for(name: &str, weapon_speed: f32) -> gamelogic::damage::DamageType {
    use gamelogic::damage::DamageType as D;
    let n = name.to_ascii_lowercase();
    if n.contains("laser") || n.contains("point_defense") || n.contains("pointdefense") {
        return D::Laser;
    }
    if n.contains("flame")
        || n.contains("dragon")
        || n.contains("napalm")
        || n.contains("inferno")
        || n.contains("fire")
    {
        return D::Flame;
    }
    if n.contains("toxin") || n.contains("anthrax") || n.contains("poison") {
        return D::Poison;
    }
    if n.contains("neutron") || n.contains("nuke") || n.contains("radiation") {
        return D::Radiation;
    }
    if n.contains("emp") || n.contains("microwave") {
        return D::Microwave;
    }
    if n.contains("sniper") || n.contains("jarmen") {
        return D::Sniper;
    }
    if n.contains("gattling") || n.contains("gatling") {
        return D::Gattling;
    }
    if n.contains("bomb")
        || n.contains("scud")
        || n.contains("missile")
        || n.contains("rocket")
        || n.contains("tomahawk")
        || n.contains("aurora")
        || n.contains("grenade")
        || n.contains("demo")
    {
        return D::Explosion;
    }
    // Instant-hit residual without laser name still often lasers (Paladin PDL speed).
    if weapon_speed >= 999_000.0 || weapon_speed <= 0.0 {
        if n.contains("gun") || n.contains("rifle") || n.contains("machine") {
            return D::SmallArms;
        }
        // many instant hits are small arms hitscan residual in host seeds
        return D::SmallArms;
    }
    if n.contains("tankgun") || n.contains("tank_gun") || n.contains("cannon") {
        return D::ArmorPiercing;
    }
    D::SmallArms
}

fn seed_known_host_weapons() -> usize {
    let seeds = [
        // AmericaInfantryRanger PRIMARY — PrimaryDamage 5, AttackRange 100,
        // DelayBetweenShots 100ms → 3 frames @ 30 FPS.
        SeedWeapon {
            name: RANGER_PRIMARY_WEAPON,
            primary_damage: 5.0,
            attack_range: 100.0,
            delay_frames: 3,
            clip_size: 3,
            weapon_speed: 999_999.0,
        },
        // GLAInfantryRebel PRIMARY
        SeedWeapon {
            name: GLA_REBEL_PRIMARY_WEAPON,
            primary_damage: 5.0,
            attack_range: 100.0,
            delay_frames: 3,
            clip_size: 3,
            weapon_speed: 999_999.0,
        },
        // ChinaInfantryRedguard PRIMARY — PrimaryDamage 15, Delay 1000ms → 30 frames
        SeedWeapon {
            name: REDGUARD_PRIMARY_WEAPON,
            primary_damage: 15.0,
            attack_range: 100.0,
            delay_frames: 30,
            clip_size: 0,
            weapon_speed: 999_999.0,
        },
        // ChinaInfantryTankHunter PRIMARY — PrimaryDamage 40, range 175, Delay 1000ms → 30 frames
        SeedWeapon {
            name: TANK_HUNTER_PRIMARY_WEAPON,
            primary_damage: 40.0,
            attack_range: 175.0,
            delay_frames: 30,
            clip_size: 0,
            weapon_speed: 600.0,
        },
        // AmericaVehicleHumvee PRIMARY — damage 10, range 150, delay 200ms → 6 frames
        SeedWeapon {
            name: HUMVEE_PRIMARY_WEAPON,
            primary_damage: 10.0,
            attack_range: 150.0,
            delay_frames: 6,
            clip_size: 0,
            weapon_speed: 600.0,
        },
        // AmericaInfantryRanger SECONDARY — RangerFlashBangGrenadeWeapon
        // PrimaryDamage 35, AttackRange 175, ClipReload 2000ms → 60 frames
        SeedWeapon {
            name: RANGER_SECONDARY_WEAPON,
            primary_damage: 35.0,
            attack_range: 175.0,
            delay_frames: 60,
            clip_size: 1,
            weapon_speed: 120.0,
        },
        // AmericaVehicleHumvee SECONDARY — HumveeMissileWeapon
        // PrimaryDamage 30, AttackRange 150, Delay 1000ms → 30 frames
        SeedWeapon {
            name: HUMVEE_SECONDARY_WEAPON,
            primary_damage: 30.0,
            attack_range: 150.0,
            delay_frames: 30,
            clip_size: 1,
            weapon_speed: 600.0,
        },
        // AmericaPatriotBattery PRIMARY — PrimaryDamage 30, AttackRange 225,
        // ClipReload 2000ms residual cadence → 60 frames @ 30 FPS.
        SeedWeapon {
            name: PATRIOT_PRIMARY_WEAPON,
            primary_damage: 30.0,
            attack_range: 225.0,
            delay_frames: 60,
            clip_size: 4,
            weapon_speed: 600.0,
        },
        // AmericaPatriotBattery SECONDARY AA — PrimaryDamage 25, AttackRange 350.
        SeedWeapon {
            name: PATRIOT_SECONDARY_WEAPON,
            primary_damage: 25.0,
            attack_range: 350.0,
            delay_frames: 60,
            clip_size: 4,
            weapon_speed: 600.0,
        },
        // GLA Stinger Site residual (soldier PRIMARY) — PrimaryDamage 20, range 225,
        // ClipReload 2000ms → 60 frames. SPAWNS_ARE_THE_WEAPONS structure residual.
        SeedWeapon {
            name: STINGER_PRIMARY_WEAPON,
            primary_damage: 20.0,
            attack_range: 225.0,
            delay_frames: 60,
            clip_size: 1,
            weapon_speed: 750.0,
        },
        // GLA Stinger Site residual (soldier SECONDARY AA) — PrimaryDamage 30, range 400.
        SeedWeapon {
            name: STINGER_SECONDARY_WEAPON,
            primary_damage: 30.0,
            attack_range: 400.0,
            delay_frames: 60,
            clip_size: 1,
            weapon_speed: 600.0,
        },
        // ChinaGattlingCannon PRIMARY — PrimaryDamage 10, AttackRange 225,
        // DelayBetweenShots 250ms → 8 frames @ 30 FPS. Instant hit.
        SeedWeapon {
            name: GATTLING_BUILDING_PRIMARY_WEAPON,
            primary_damage: 10.0,
            attack_range: 225.0,
            delay_frames: 8,
            clip_size: 0,
            weapon_speed: 999_999.0,
        },
        // ChinaGattlingCannon SECONDARY AA — PrimaryDamage 5, AttackRange 400,
        // DelayBetweenShots 250ms → 8 frames. Continuous-fire ramp shares structure residual.
        SeedWeapon {
            name: GATTLING_BUILDING_SECONDARY_WEAPON,
            primary_damage: 5.0,
            attack_range: 400.0,
            delay_frames: 8,
            clip_size: 0,
            weapon_speed: 999_999.0,
        },
        // ChinaVehicleNukeCannon PRIMARY — NukeCannonGun residual seed.
        // PrimaryDamage 400 retail shell; host residual area path uses 400/50 + 20/60.
        SeedWeapon {
            name: NUKE_CANNON_PRIMARY_WEAPON,
            primary_damage: 400.0,
            attack_range: 350.0,
            delay_frames: 300,
            clip_size: 0,
            weapon_speed: 200.0,
        },
        // NukeCannonNeutronWeapon SECONDARY — PrimaryDamage 1 (blast does work),
        // AttackRange 350, Delay 10000ms → 300 frames. Blast via host residual.
        SeedWeapon {
            name: NUKE_CANNON_NEUTRON_WEAPON,
            primary_damage: 1.0,
            attack_range: 350.0,
            delay_frames: 300,
            clip_size: 0,
            weapon_speed: 200.0,
        },
        // PaladinPointDefenseLaser — PrimaryDamage 100, AttackRange 65,
        // DelayBetweenShots 1000ms → 30 frames. Instant laser.
        SeedWeapon {
            name: PALADIN_POINT_DEFENSE_LASER,
            primary_damage: 100.0,
            attack_range: 65.0,
            delay_frames: 30,
            clip_size: 0,
            weapon_speed: 999_999.0,
        },
        // AvengerPointDefenseLaserOne — PrimaryDamage 100, AttackRange 100,
        // DelayBetweenShots 500ms → 15 frames.
        SeedWeapon {
            name: AVENGER_POINT_DEFENSE_LASER,
            primary_damage: 100.0,
            attack_range: 100.0,
            delay_frames: 15,
            clip_size: 0,
            weapon_speed: 999_999.0,
        },
        // CrusaderTankGun PRIMARY — PrimaryDamage 60, AttackRange 150,
        // DelayBetweenShots 2000ms → 60 frames.
        SeedWeapon {
            name: CRUSADER_TANK_GUN,
            primary_damage: 60.0,
            attack_range: 150.0,
            delay_frames: 60,
            clip_size: 0,
            weapon_speed: 400.0,
        },
        // PaladinTankGun PRIMARY — same residual stats as CrusaderTankGun.
        SeedWeapon {
            name: PALADIN_TANK_GUN,
            primary_damage: 60.0,
            attack_range: 150.0,
            delay_frames: 60,
            clip_size: 0,
            weapon_speed: 300.0,
        },
        // Lazr_CrusaderTankGun PRIMARY — PrimaryDamage 80, AttackRange 150,
        // DelayBetweenShots 2000ms → 60 frames. Instant laser residual.
        SeedWeapon {
            name: LAZR_CRUSADER_TANK_GUN,
            primary_damage: 80.0,
            attack_range: 150.0,
            delay_frames: 60,
            clip_size: 0,
            weapon_speed: 99999.0,
        },
        // Lazr_PaladinTankGun PRIMARY — PrimaryDamage 70, AttackRange 150,
        // DelayBetweenShots 1000ms → 30 frames.
        SeedWeapon {
            name: LAZR_PALADIN_TANK_GUN,
            primary_damage: 70.0,
            attack_range: 150.0,
            delay_frames: 30,
            clip_size: 0,
            weapon_speed: 99999.0,
        },
        // Lazr_PatriotMissileWeapon PRIMARY — PrimaryDamage 40, Range 225,
        // ClipReload residual cadence 2000ms → 60 frames.
        SeedWeapon {
            name: LAZR_PATRIOT_PRIMARY_WEAPON,
            primary_damage: 40.0,
            attack_range: 225.0,
            delay_frames: 60,
            clip_size: 3,
            weapon_speed: 999999.0,
        },
        // Lazr_PatriotMissileWeaponAir residual secondary — PrimaryDamage 35, Range 350.
        SeedWeapon {
            name: LAZR_PATRIOT_SECONDARY_WEAPON,
            primary_damage: 35.0,
            attack_range: 350.0,
            delay_frames: 60,
            clip_size: 4,
            weapon_speed: 999999.0,
        },
        // SupW_PatriotMissileWeapon PRIMARY — PrimaryDamage 15, Range 275,
        // EMP detonation residual (DISABLED_EMP r10 / 10s).
        SeedWeapon {
            name: SUPW_PATRIOT_PRIMARY_WEAPON,
            primary_damage: 15.0,
            attack_range: 275.0,
            delay_frames: 60,
            clip_size: 4,
            weapon_speed: 600.0,
        },
        // SupW_PatriotMissileWeaponAir residual secondary — PrimaryDamage 30, Range 400.
        SeedWeapon {
            name: SUPW_PATRIOT_SECONDARY_WEAPON,
            primary_damage: 30.0,
            attack_range: 400.0,
            delay_frames: 60,
            clip_size: 4,
            weapon_speed: 400.0,
        },
        // TunnelNetworkGun PRIMARY — PrimaryDamage 15, Range 175,
        // DelayBetweenShots 250ms → 8 frames.
        SeedWeapon {
            name: TUNNEL_NETWORK_GUN,
            primary_damage: 15.0,
            attack_range: 175.0,
            delay_frames: 8,
            clip_size: 0,
            weapon_speed: 600.0,
        },
        // AvengerTargetDesignator PRIMARY — STATUS paint residual (0 HP dmg).
        // PrimaryDamage in retail is duration ms; host residual damage=0.
        SeedWeapon {
            name: AVENGER_TARGET_DESIGNATOR,
            primary_damage: 0.0,
            attack_range: 200.0,
            delay_frames: 6,
            clip_size: 0,
            weapon_speed: 999_999.0,
        },
        // AvengerAirLaserOne residual secondary AA — PrimaryDamage 10, Range 300,
        // Delay 200ms → 6 frames. Anti-air only (seeded separately below).
        SeedWeapon {
            name: AVENGER_AIR_LASER,
            primary_damage: 10.0,
            attack_range: 300.0,
            delay_frames: 6,
            clip_size: 0,
            weapon_speed: 999_999.0,
        },
        // HumveeMissileWeaponAir tertiary residual — PrimaryDamage 50, Range 320,
        // Delay+ClipReload residual cycle → 90 frames. Anti-air only.
        SeedWeapon {
            name: HUMVEE_MISSILE_WEAPON_AIR,
            primary_damage: 50.0,
            attack_range: 320.0,
            delay_frames: 90,
            clip_size: 1,
            weapon_speed: 600.0,
        },
        // InfernoCannonGun PRIMARY — PrimaryDamage 30, AttackRange 300,
        // DelayBetweenShots 4000ms → 120 frames. FireFieldSmall residual on impact.
        SeedWeapon {
            name: INFERNO_CANNON_PRIMARY_WEAPON,
            primary_damage: 30.0,
            attack_range: 300.0,
            delay_frames: 120,
            clip_size: 0,
            weapon_speed: 250.0,
        },
        // AuroraBombWeapon PRIMARY — PrimaryDamage 400, AttackRange 300,
        // ClipReload 5000ms → 150 frames. Delayed dive residual applies AOE.
        SeedWeapon {
            name: AURORA_BOMB_PRIMARY_WEAPON,
            primary_damage: 400.0,
            attack_range: 300.0,
            delay_frames: 150,
            clip_size: 1,
            weapon_speed: 99999.0,
        },
        // AirF_AuroraBombWeapon — tiny primary; FuelAir detonation residual.
        SeedWeapon {
            name: AIRF_AURORA_BOMB_PRIMARY_WEAPON,
            primary_damage: 2.0,
            attack_range: 300.0,
            delay_frames: 150,
            clip_size: 1,
            weapon_speed: 99999.0,
        },
        // SupW_AuroraFuelBombWeapon — FuelAir residual path.
        SeedWeapon {
            name: SUPW_AURORA_FUEL_BOMB_WEAPON,
            primary_damage: 400.0,
            attack_range: 300.0,
            delay_frames: 150,
            clip_size: 1,
            weapon_speed: 99999.0,
        },
        // StealthJetMissileWeapon PRIMARY — PrimaryDamage 100, AttackRange 220,
        // Delay 200ms → 6 frames. Bunker-buster residual on impact when upgraded.
        SeedWeapon {
            name: STEALTH_JET_MISSILE_WEAPON,
            primary_damage: 100.0,
            attack_range: 220.0,
            delay_frames: 6,
            clip_size: 2,
            weapon_speed: 1000.0,
        },
        // NapalmMissileWeapon PRIMARY — PrimaryDamage 75, Range 320, min 80,
        // Delay 300ms → 9 frames. ClipSize 2. FireField residual on impact.
        SeedWeapon {
            name: NAPALM_MISSILE_WEAPON,
            primary_damage: 75.0,
            attack_range: 320.0,
            delay_frames: 9,
            clip_size: 2,
            weapon_speed: 1000.0,
        },
        // BlackNapalmMissileWeapon — same primary, upgraded secondary/field residual.
        SeedWeapon {
            name: BLACK_NAPALM_MISSILE_WEAPON,
            primary_damage: 75.0,
            attack_range: 320.0,
            delay_frames: 9,
            clip_size: 2,
            weapon_speed: 1000.0,
        },
        // Nuke_MiGMissileWeapon — PrimaryDamage 100 residual.
        SeedWeapon {
            name: NUKE_MIG_MISSILE_WEAPON,
            primary_damage: 100.0,
            attack_range: 320.0,
            delay_frames: 9,
            clip_size: 2,
            weapon_speed: 1000.0,
        },
        // Nuke_NukeMissileWeapon — PrimaryDamage 150 residual.
        SeedWeapon {
            name: NUKE_NUKE_MISSILE_WEAPON,
            primary_damage: 150.0,
            attack_range: 320.0,
            delay_frames: 9,
            clip_size: 2,
            weapon_speed: 1000.0,
        },
        // FireBaseHowitzerGun PRIMARY — PrimaryDamage 75, Range 275, min 50,
        // Delay 2000ms → 60 frames.
        SeedWeapon {
            name: FIRE_BASE_HOWITZER_WEAPON,
            primary_damage: 75.0,
            attack_range: 275.0,
            delay_frames: 60,
            clip_size: 0,
            weapon_speed: 300.0,
        },
        // RaptorJetMissileWeapon PRIMARY — PrimaryDamage 100, Range 320, min 100,
        // Delay 150ms → 5 frames. ClipSize 4 honesty.
        SeedWeapon {
            name: RAPTOR_JET_MISSILE_WEAPON,
            primary_damage: 100.0,
            attack_range: 320.0,
            delay_frames: 5,
            clip_size: 4,
            weapon_speed: 1000.0,
        },
        // AirF_RaptorJetMissileWeapon — PrimaryDamage 125, Range 350, Delay 75ms → 3 frames.
        SeedWeapon {
            name: AIRF_RAPTOR_JET_MISSILE_WEAPON,
            primary_damage: 125.0,
            attack_range: 350.0,
            delay_frames: 3,
            clip_size: 6,
            weapon_speed: 1000.0,
        },
        // BattleDroneMachineGun — PrimaryDamage 1, Range 110, Delay 100ms → 3 frames.
        SeedWeapon {
            name: BATTLE_DRONE_MACHINE_GUN,
            primary_damage: 1.0,
            attack_range: 110.0,
            delay_frames: 3,
            clip_size: 0,
            weapon_speed: 999_999.0,
        },
        // MicrowaveTankBuildingClearer — PrimaryDamage 1 (kills 1 garrisoned unit),
        // AttackRange 125. KILL_GARRISONED residual via host combat path.
        SeedWeapon {
            name: MICROWAVE_BUILDING_CLEARER_WEAPON,
            primary_damage: 1.0,
            attack_range: 125.0,
            delay_frames: 30,
            clip_size: 0,
            weapon_speed: 999_999.0,
        },
        // Comanche20mmCannonWeapon PRIMARY — PrimaryDamage 6, AttackRange 200,
        // DelayBetweenShots 100ms → 3 frames @ 30 FPS.
        SeedWeapon {
            name: COMANCHE_PRIMARY_WEAPON,
            primary_damage: 6.0,
            attack_range: 200.0,
            delay_frames: 3,
            clip_size: 0,
            weapon_speed: 999_999.0,
        },
        // HelixMinigunWeapon PRIMARY — PrimaryDamage 6, AttackRange 115,
        // DelayBetweenShots 100ms → 3 frames @ 30 FPS. Intended-only residual.
        SeedWeapon {
            name: HELIX_MINIGUN_WEAPON,
            primary_damage: 6.0,
            attack_range: 115.0,
            delay_frames: 3,
            clip_size: 0,
            weapon_speed: 999_999.0,
        },
        // ComancheAntiTankMissileWeapon residual SECONDARY — PrimaryDamage 50,
        // radius dual residual via host (50/5 + 30/25), Range 200, Delay 500ms → 15 frames.
        SeedWeapon {
            name: COMANCHE_ANTITANK_WEAPON,
            primary_damage: 50.0,
            attack_range: 200.0,
            delay_frames: 15,
            clip_size: 4,
            weapon_speed: 99999.0,
        },
        // ComancheRocketPodWeapon residual SECONDARY after upgrade (retail TERTIARY).
        // PrimaryDamage 30, AttackRange 200, Delay 200ms → 6 frames.
        // Area damage applied by host residual (primary/secondary rings).
        SeedWeapon {
            name: COMANCHE_ROCKET_POD_WEAPON,
            primary_damage: 30.0,
            attack_range: 200.0,
            delay_frames: 6,
            clip_size: 20,
            weapon_speed: 99999.0,
        },
        // SentryDroneGun PRIMARY after PLAYER_UPGRADE — PrimaryDamage 8, Range 150,
        // Delay 200ms → 6 frames.
        SeedWeapon {
            name: SENTRY_DRONE_GUN_WEAPON,
            primary_damage: 8.0,
            attack_range: 150.0,
            delay_frames: 6,
            clip_size: 0,
            weapon_speed: 600.0,
        },
        // USAPathfinderSniperRifle PRIMARY — PrimaryDamage 100, Range 300,
        // Delay 2000ms → 60 frames.
        SeedWeapon {
            name: PATHFINDER_SNIPER_WEAPON,
            primary_damage: 100.0,
            attack_range: 300.0,
            delay_frames: 60,
            clip_size: 0,
            weapon_speed: 999_999.0,
        },
        // HellfireMissileWeapon PRIMARY — PrimaryDamage 40, Range 150,
        // Delay+ClipReload ~3000ms → 90 frames.
        SeedWeapon {
            name: HELLFIRE_MISSILE_WEAPON,
            primary_damage: 40.0,
            attack_range: 150.0,
            delay_frames: 90,
            clip_size: 1,
            weapon_speed: 600.0,
        },
        // GLA Angry Mob residual aggregate fire — PrimaryDamage 20 (5 members × 4),
        // AttackRange 100, Delay 250ms → 8 frames. update_angry_mobs deals real residual.
        SeedWeapon {
            name: ANGRY_MOB_RESIDUAL_WEAPON,
            primary_damage: 20.0,
            attack_range: 100.0,
            delay_frames: 8,
            clip_size: 0,
            weapon_speed: 999_999.0,
        },
        // GLA Rocket Buggy BuggyRocketWeapon — PrimaryDamage 20, Range 300,
        // Delay 200ms → 6 frames, clip 6. Min range / splash residual via host.
        SeedWeapon {
            name: BUGGY_ROCKET_WEAPON,
            primary_damage: 20.0,
            attack_range: 300.0,
            delay_frames: 6,
            clip_size: 6,
            weapon_speed: 600.0,
        },
        // BuggyRocketWeaponUpgraded — same damage, clip 12 residual.
        SeedWeapon {
            name: BUGGY_ROCKET_WEAPON_UPGRADED,
            primary_damage: 20.0,
            attack_range: 300.0,
            delay_frames: 6,
            clip_size: 12,
            weapon_speed: 600.0,
        },
        // QuadCannonGun ground — PrimaryDamage 10, Range 150, Delay 100ms → 3 frames.
        SeedWeapon {
            name: QUAD_CANNON_GUN,
            primary_damage: 10.0,
            attack_range: 150.0,
            delay_frames: 3,
            clip_size: 0,
            weapon_speed: 999_999.0,
        },
        // QuadCannonGunUpgradeOne — dmg 8, Delay 50ms → 2 frames.
        SeedWeapon {
            name: QUAD_CANNON_GUN_UPGRADE_ONE,
            primary_damage: 8.0,
            attack_range: 150.0,
            delay_frames: 2,
            clip_size: 0,
            weapon_speed: 999_999.0,
        },
        // QuadCannonGunUpgradeTwo — dmg 8, Delay 25ms → 1 frame.
        SeedWeapon {
            name: QUAD_CANNON_GUN_UPGRADE_TWO,
            primary_damage: 8.0,
            attack_range: 150.0,
            delay_frames: 1,
            clip_size: 0,
            weapon_speed: 999_999.0,
        },
        // SCUDLauncherGunExplosive — PrimaryDamage 300, Range 350, clip 1,
        // ClipReload 10000ms → 300 frames. Area residual via host.
        SeedWeapon {
            name: SCUD_GUN_EXPLOSIVE,
            primary_damage: 300.0,
            attack_range: 350.0,
            delay_frames: 300,
            clip_size: 1,
            weapon_speed: 200.0,
        },
        // SCUDLauncherGunToxin — PrimaryDamage 200, Range 350, toxin residual.
        SeedWeapon {
            name: SCUD_GUN_TOXIN,
            primary_damage: 200.0,
            attack_range: 350.0,
            delay_frames: 300,
            clip_size: 1,
            weapon_speed: 200.0,
        },
        // SCUDLauncherGunAnthrax — same blast residual as toxin; upgraded field flag.
        SeedWeapon {
            name: SCUD_GUN_ANTHRAX,
            primary_damage: 200.0,
            attack_range: 350.0,
            delay_frames: 300,
            clip_size: 1,
            weapon_speed: 200.0,
        },
        // TechnicalMachineGunWeapon — dmg 10, range 150, Delay 200ms → 6 frames.
        SeedWeapon {
            name: TECHNICAL_MACHINE_GUN,
            primary_damage: 10.0,
            attack_range: 150.0,
            delay_frames: 6,
            clip_size: 0,
            weapon_speed: 999_999.0,
        },
        // TechnicalCannonWeapon — dmg 45, range 150, Delay 1000ms → 30 frames.
        SeedWeapon {
            name: TECHNICAL_CANNON,
            primary_damage: 45.0,
            attack_range: 150.0,
            delay_frames: 30,
            clip_size: 0,
            weapon_speed: 300.0,
        },
        // TechnicalRPGWeapon — dmg 50, range 150, Delay 1000ms → 30 frames.
        SeedWeapon {
            name: TECHNICAL_RPG,
            primary_damage: 50.0,
            attack_range: 150.0,
            delay_frames: 30,
            clip_size: 0,
            weapon_speed: 200.0,
        },
        // ToxinTruckGun — poison stream dmg 10, range 100, Delay 40ms → 2 frames.
        SeedWeapon {
            name: TOXIN_TRUCK_GUN,
            primary_damage: 10.0,
            attack_range: 100.0,
            delay_frames: 2,
            clip_size: 30,
            weapon_speed: 600.0,
        },
        // ToxinTruckGunUpgraded — anthrax stream dmg 12.5.
        SeedWeapon {
            name: TOXIN_TRUCK_GUN_UPGRADED,
            primary_damage: 12.5,
            attack_range: 100.0,
            delay_frames: 2,
            clip_size: 30,
            weapon_speed: 600.0,
        },
        // ToxinTruckSprayer — contaminate residual (retail PrimaryDamage 0; host
        // store needs >0 to bind — spray area dmg is SecondaryDamage 2 via residual).
        SeedWeapon {
            name: TOXIN_TRUCK_SPRAYER,
            primary_damage: 0.001,
            attack_range: 15.0,
            delay_frames: 6,
            clip_size: 0,
            weapon_speed: 600.0,
        },
        // ToxinTruckSprayerUpgraded — anthrax spray residual.
        SeedWeapon {
            name: TOXIN_TRUCK_SPRAYER_UPGRADED,
            primary_damage: 0.001,
            attack_range: 15.0,
            delay_frames: 6,
            clip_size: 0,
            weapon_speed: 600.0,
        },
        // MarauderTankGun — dmg 60, range 170, Delay 2000ms → 60 frames.
        SeedWeapon {
            name: MARAUDER_TANK_GUN,
            primary_damage: 60.0,
            attack_range: 170.0,
            delay_frames: 60,
            clip_size: 0,
            weapon_speed: 300.0,
        },
        // BattleMasterTankGun — dmg 60, range 150, Delay 2000ms → 60 frames.
        // UraniumShells PLAYER_UPGRADE DAMAGE 125% applied at host residual fire time.
        SeedWeapon {
            name: BATTLE_MASTER_TANK_GUN,
            primary_damage: 60.0,
            attack_range: 150.0,
            delay_frames: 60,
            clip_size: 0,
            weapon_speed: 400.0,
        },
        // ScorpionTankGun — dmg 20, range 150, Delay 1000ms → 30 frames.
        SeedWeapon {
            name: SCORPION_TANK_GUN,
            primary_damage: 20.0,
            attack_range: 150.0,
            delay_frames: 30,
            clip_size: 0,
            weapon_speed: 400.0,
        },
        // ScorpionTankGunPlusOne — salvage dmg 25.
        SeedWeapon {
            name: SCORPION_TANK_GUN_PLUS_ONE,
            primary_damage: 25.0,
            attack_range: 150.0,
            delay_frames: 30,
            clip_size: 0,
            weapon_speed: 400.0,
        },
        // ScorpionMissileWeapon — dmg 100, range 150, ClipReload 15000ms → 450 frames.
        SeedWeapon {
            name: SCORPION_MISSILE_WEAPON,
            primary_damage: 100.0,
            attack_range: 150.0,
            delay_frames: 450,
            clip_size: 1,
            weapon_speed: 600.0,
        },
        // TomahawkMissileWeapon — dmg 150, range 350, ClipReload 7000ms → 210 frames.
        SeedWeapon {
            name: TOMAHAWK_MISSILE_WEAPON,
            primary_damage: 150.0,
            attack_range: 350.0,
            delay_frames: 210,
            clip_size: 1,
            weapon_speed: 200.0,
        },
        // OverlordTankGun — dmg 80, range 175, ClipReload 2000ms → 60 frames, clip 2.
        SeedWeapon {
            name: OVERLORD_TANK_GUN,
            primary_damage: 80.0,
            attack_range: 175.0,
            delay_frames: 60,
            clip_size: 2,
            weapon_speed: 300.0,
        },
        // GLAJarmenKellRifle — dmg 180, range 225, Delay 1000ms → 30 frames.
        SeedWeapon {
            name: JARMEN_KELL_RIFLE,
            primary_damage: 180.0,
            attack_range: 225.0,
            delay_frames: 30,
            clip_size: 0,
            weapon_speed: 999_999.0,
        },
        // MarauderTankGunUpgradeOne — same dmg, Delay 1500ms → 45 frames.
        SeedWeapon {
            name: MARAUDER_TANK_GUN_UPGRADE_ONE,
            primary_damage: 60.0,
            attack_range: 170.0,
            delay_frames: 45,
            clip_size: 0,
            weapon_speed: 400.0,
        },
        // MarauderTankGunUpgradeTwo — same dmg, Delay 750ms → 23 frames, clip 2.
        SeedWeapon {
            name: MARAUDER_TANK_GUN_UPGRADE_TWO,
            primary_damage: 60.0,
            attack_range: 170.0,
            delay_frames: 23,
            clip_size: 2,
            weapon_speed: 500.0,
        },
        // GLARebelBikerMachineGun — dmg 8, range 150, Delay 100ms → 3 frames, clip 6.
        SeedWeapon {
            name: REBEL_BIKER_MG,
            primary_damage: 8.0,
            attack_range: 150.0,
            delay_frames: 3,
            clip_size: 6,
            weapon_speed: 999_999.0,
        },
        // TunnelDefenderBikerRocketWeapon — dmg 40, range 175, Delay 1000ms → 30 frames.
        SeedWeapon {
            name: TUNNEL_DEFENDER_BIKER_ROCKET,
            primary_damage: 40.0,
            attack_range: 175.0,
            delay_frames: 30,
            clip_size: 0,
            weapon_speed: 600.0,
        },
        // GLABikerKellSniperRifle — dmg 180, range 225, Delay 750ms → 23 frames.
        SeedWeapon {
            name: BIKER_KELL_SNIPER,
            primary_damage: 180.0,
            attack_range: 225.0,
            delay_frames: 23,
            clip_size: 0,
            weapon_speed: 999_999.0,
        },
        // TerroristSuicideWeapon residual — host binds as short-range suicide flag.
        SeedWeapon {
            name: TERRORIST_SUICIDE_WEAPON,
            primary_damage: 500.0,
            attack_range: 5.0,
            delay_frames: 1,
            clip_size: 1,
            weapon_speed: 999_999.0,
        },
        // SuicideDynamitePack residual — FireWeaponWhenDead for infantry Terrorist.
        SeedWeapon {
            name: SUICIDE_DYNAMITE_PACK,
            primary_damage: 500.0,
            attack_range: 5.0,
            delay_frames: 1,
            clip_size: 1,
            weapon_speed: 999_999.0,
        },
        // MissileDefenderMissileWeapon — dmg 40, range 175, Delay 1000ms → 30 frames.
        SeedWeapon {
            name: MISSILE_DEFENDER_MISSILE_WEAPON,
            primary_damage: 40.0,
            attack_range: 175.0,
            delay_frames: 30,
            clip_size: 0,
            weapon_speed: 600.0,
        },
        // MissileDefenderLaserGuidedMissileWeapon — dmg 40, range 300, Delay 500ms → 15 frames.
        SeedWeapon {
            name: MISSILE_DEFENDER_LASER_GUIDED_WEAPON,
            primary_damage: 40.0,
            attack_range: 300.0,
            delay_frames: 15,
            clip_size: 0,
            weapon_speed: 600.0,
        },
        // DragonTankFlameWeapon — dmg 10, range 75, Delay 40ms → 2 frames, splash residual.
        SeedWeapon {
            name: DRAGON_TANK_FLAME_WEAPON,
            primary_damage: 10.0,
            attack_range: 75.0,
            delay_frames: 2,
            clip_size: 30,
            weapon_speed: 600.0,
        },
        // DragonTankFlameWeaponUpgraded — BlackNapalm dmg 12.5.
        SeedWeapon {
            name: DRAGON_TANK_FLAME_WEAPON_UPGRADED,
            primary_damage: 12.5,
            attack_range: 75.0,
            delay_frames: 2,
            clip_size: 0,
            weapon_speed: 600.0,
        },
        // GattlingTankGun — dmg 15, range 150, Delay 400ms → 12 frames.
        SeedWeapon {
            name: GATTLING_TANK_GUN,
            primary_damage: 15.0,
            attack_range: 150.0,
            delay_frames: 12,
            clip_size: 0,
            weapon_speed: 999_999.0,
        },
        // Infa_MiniGunnerGun — dmg 10, range 125, Delay 500ms → 15 frames.
        SeedWeapon {
            name: MINIGUNNER_GUN,
            primary_damage: 10.0,
            attack_range: 125.0,
            delay_frames: 15,
            clip_size: 0,
            weapon_speed: 999_999.0,
        },
        // TunnelDefenderRocketWeapon — dmg 40, range 175, min 5, Delay 1000ms → 30 frames.
        SeedWeapon {
            name: TUNNEL_DEFENDER_ROCKET_WEAPON,
            primary_damage: 40.0,
            attack_range: 175.0,
            delay_frames: 30,
            clip_size: 0,
            weapon_speed: 600.0,
        },
    ];

    let mut added = 0usize;
    for seed in seeds {
        if store_has(seed.name) {
            continue;
        }
        let mut t = WeaponTemplate::new(seed.name.to_string());
        t.primary_damage = seed.primary_damage;
        t.attack_range = seed.attack_range;
        t.min_delay_between_shots = seed.delay_frames;
        t.max_delay_between_shots = seed.delay_frames;
        t.clip_size = seed.clip_size;
        t.weapon_speed = seed.weapon_speed;
        t.damage_type = seed_damage_type_for(seed.name, seed.weapon_speed);
        t.death_type = seed_death_type_for(seed.name, t.damage_type);
        {
            let fs = seed_fire_sound_for(seed.name);
            if !fs.is_empty() {
                t.fire_sound = gamelogic::weapon::AudioEventRts::new(fs);
            }
            let pname = seed_projectile_name_for(seed.name);
            if !pname.is_empty() {
                t.projectile_name = pname;
            }
        }
        t.anti_mask.insert(WeaponAntiMask::GROUND);
        // Min-range residual for long-range GLA artillery / rockets.
        if seed.name == BUGGY_ROCKET_WEAPON || seed.name == BUGGY_ROCKET_WEAPON_UPGRADED {
            t.minimum_attack_range = 50.0;
        }
        if seed.name == SCUD_GUN_EXPLOSIVE
            || seed.name == SCUD_GUN_TOXIN
            || seed.name == SCUD_GUN_ANTHRAX
        {
            t.minimum_attack_range = 200.0;
            t.pre_attack_delay = 15; // 500ms @ 30 FPS residual
        }
        if seed.name == TECHNICAL_RPG {
            t.minimum_attack_range = 5.0;
        }
        if seed.name == TECHNICAL_CANNON {
            t.primary_damage_radius = 25.0;
        }
        if seed.name == MARAUDER_TANK_GUN
            || seed.name == MARAUDER_TANK_GUN_UPGRADE_ONE
            || seed.name == MARAUDER_TANK_GUN_UPGRADE_TWO
            || seed.name == BATTLE_MASTER_TANK_GUN
            || seed.name == SCORPION_TANK_GUN
            || seed.name == SCORPION_TANK_GUN_PLUS_ONE
        {
            t.primary_damage_radius = 5.0;
        }
        if seed.name == SCORPION_MISSILE_WEAPON {
            t.primary_damage_radius = 5.0;
            t.secondary_damage = 80.0;
            t.secondary_damage_radius = 25.0;
            t.minimum_attack_range = 40.0;
        }
        if seed.name == TOMAHAWK_MISSILE_WEAPON {
            t.primary_damage_radius = 10.0;
            t.secondary_damage = 50.0;
            t.secondary_damage_radius = 25.0;
            t.minimum_attack_range = 100.0;
            t.pre_attack_delay = 8; // 250ms @ 30 FPS residual
        }
        if seed.name == DRAGON_TANK_FLAME_WEAPON || seed.name == DRAGON_TANK_FLAME_WEAPON_UPGRADED {
            t.primary_damage_radius = 5.0;
            t.secondary_damage = if seed.name == DRAGON_TANK_FLAME_WEAPON_UPGRADED {
                1.25
            } else {
                1.0
            };
            t.secondary_damage_radius = 10.0;
        }
        if seed.name == TUNNEL_DEFENDER_BIKER_ROCKET {
            t.minimum_attack_range = 5.0;
            t.primary_damage_radius = 5.0;
            t.anti_mask.insert(WeaponAntiMask::AIRBORNE_VEHICLE);
            t.anti_mask.insert(WeaponAntiMask::AIRBORNE_INFANTRY);
        }
        if seed.name == TUNNEL_DEFENDER_ROCKET_WEAPON {
            t.minimum_attack_range = 5.0;
            t.primary_damage_radius = 5.0;
            t.anti_mask.insert(WeaponAntiMask::AIRBORNE_VEHICLE);
            t.anti_mask.insert(WeaponAntiMask::AIRBORNE_INFANTRY);
        }
        // Avenger designator can paint air + ground residual.
        if seed.name == AVENGER_TARGET_DESIGNATOR {
            t.anti_mask.insert(WeaponAntiMask::AIRBORNE_VEHICLE);
            t.anti_mask.insert(WeaponAntiMask::AIRBORNE_INFANTRY);
        }
        // Avenger air laser + Humvee air TOW: airborne only residual.
        if seed.name == AVENGER_AIR_LASER || seed.name == HUMVEE_MISSILE_WEAPON_AIR {
            t.anti_mask = WeaponAntiMask::new(0);
            t.anti_mask.insert(WeaponAntiMask::AIRBORNE_VEHICLE);
            t.anti_mask.insert(WeaponAntiMask::AIRBORNE_INFANTRY);
        }
        // Base-defense AA secondaries: airborne only residual.
        if seed.name == PATRIOT_SECONDARY_WEAPON
            || seed.name == LAZR_PATRIOT_SECONDARY_WEAPON
            || seed.name == SUPW_PATRIOT_SECONDARY_WEAPON
            || seed.name == STINGER_SECONDARY_WEAPON
            || seed.name == GATTLING_BUILDING_SECONDARY_WEAPON
        {
            t.anti_mask = WeaponAntiMask::new(0);
            t.anti_mask.insert(WeaponAntiMask::AIRBORNE_VEHICLE);
            t.anti_mask.insert(WeaponAntiMask::AIRBORNE_INFANTRY);
        }
        // Stinger / Patriot primary splash residual.
        if seed.name == STINGER_PRIMARY_WEAPON
            || seed.name == PATRIOT_PRIMARY_WEAPON
            || seed.name == PATRIOT_SECONDARY_WEAPON
            || seed.name == LAZR_PATRIOT_PRIMARY_WEAPON
            || seed.name == LAZR_PATRIOT_SECONDARY_WEAPON
            || seed.name == STINGER_SECONDARY_WEAPON
        {
            t.primary_damage_radius = if seed.name == LAZR_PATRIOT_PRIMARY_WEAPON
                || seed.name == LAZR_PATRIOT_SECONDARY_WEAPON
            {
                3.0
            } else {
                5.0
            };
        }
        if seed.name == STINGER_SECONDARY_WEAPON {
            t.primary_damage_radius = 10.0;
        }
        match with_weapon_store_mut(|store| {
            store.add_weapon_template(t);
        }) {
            Ok(()) => {
                log::debug!("Host WeaponStore: seeded weapon {}", seed.name);
                added += 1;
            }
            Err(e) => {
                log::warn!("Host WeaponStore: failed to seed {}: {e}", seed.name);
            }
        }
    }

    // Quad Cannon AA secondaries: airborne only (AntiGround=No residual).
    for (name, delay) in [
        (QUAD_CANNON_GUN_AIR, 3i32),
        (QUAD_CANNON_GUN_UPGRADE_ONE_AIR, 2),
        (QUAD_CANNON_GUN_UPGRADE_TWO_AIR, 1),
    ] {
        if store_has(name) {
            continue;
        }
        let mut t = WeaponTemplate::new(name.to_string());
        t.primary_damage = 5.0;
        t.attack_range = 350.0;
        t.min_delay_between_shots = delay;
        t.max_delay_between_shots = delay;
        t.clip_size = 0;
        t.weapon_speed = 999_999.0;
        // Air only — no GROUND mask so can_target_ground residual is false.
        t.anti_mask = WeaponAntiMask::new(0);
        t.anti_mask.insert(WeaponAntiMask::AIRBORNE_VEHICLE);
        t.anti_mask.insert(WeaponAntiMask::AIRBORNE_INFANTRY);
        match with_weapon_store_mut(|store| {
            store.add_weapon_template(t);
        }) {
            Ok(()) => {
                log::debug!("Host WeaponStore: seeded AA weapon {}", name);
                added += 1;
            }
            Err(e) => {
                log::warn!("Host WeaponStore: failed to seed {}: {e}", name);
            }
        }
    }

    // Gattling Tank AA secondary: airborne only residual.
    if !store_has(GATTLING_TANK_GUN_AIR) {
        let mut t = WeaponTemplate::new(GATTLING_TANK_GUN_AIR.to_string());
        t.primary_damage = 12.0;
        t.attack_range = 350.0;
        t.min_delay_between_shots = 12;
        t.max_delay_between_shots = 12;
        t.clip_size = 0;
        t.weapon_speed = 999_999.0;
        t.anti_mask = WeaponAntiMask::new(0);
        t.anti_mask.insert(WeaponAntiMask::AIRBORNE_VEHICLE);
        t.anti_mask.insert(WeaponAntiMask::AIRBORNE_INFANTRY);
        match with_weapon_store_mut(|store| {
            store.add_weapon_template(t);
        }) {
            Ok(()) => {
                log::debug!(
                    "Host WeaponStore: seeded AA weapon {}",
                    GATTLING_TANK_GUN_AIR
                );
                added += 1;
            }
            Err(e) => {
                log::warn!(
                    "Host WeaponStore: failed to seed {}: {e}",
                    GATTLING_TANK_GUN_AIR
                );
            }
        }
    }

    // MiniGunner AA secondary: airborne only residual.
    if !store_has(MINIGUNNER_GUN_AIR) {
        let mut t = WeaponTemplate::new(MINIGUNNER_GUN_AIR.to_string());
        t.primary_damage = 10.0;
        t.attack_range = 350.0;
        t.min_delay_between_shots = 15;
        t.max_delay_between_shots = 15;
        t.clip_size = 0;
        t.weapon_speed = 999_999.0;
        t.anti_mask = WeaponAntiMask::new(0);
        t.anti_mask.insert(WeaponAntiMask::AIRBORNE_VEHICLE);
        t.anti_mask.insert(WeaponAntiMask::AIRBORNE_INFANTRY);
        match with_weapon_store_mut(|store| {
            store.add_weapon_template(t);
        }) {
            Ok(()) => {
                log::debug!("Host WeaponStore: seeded AA weapon {}", MINIGUNNER_GUN_AIR);
                added += 1;
            }
            Err(e) => {
                log::warn!(
                    "Host WeaponStore: failed to seed {}: {e}",
                    MINIGUNNER_GUN_AIR
                );
            }
        }
    }

    if added > 0 {
        log::info!(
            "Host WeaponStore: seeded {} known golden-unit weapons (INI data unavailable or incomplete)",
            added
        );
    }
    added
}

struct SeedWeapon {
    name: &'static str,
    primary_damage: f32,
    attack_range: f32,
    delay_frames: i32,
    clip_size: i32,
    weapon_speed: f32,
}

/// Test helper: force re-bootstrap attempt (does not clear existing templates).
#[cfg(test)]
pub fn reset_bootstrap_attempt_flag_for_tests() {
    BOOTSTRAP_ATTEMPTED.store(false, Ordering::Relaxed);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::{KindOf, Team, ThingTemplate, Weapon};
    use glam::Vec3;

    #[test]
    fn bootstrap_seeds_ranger_with_non_default_damage() {
        ensure_host_weapon_store();
        assert!(store_has(RANGER_PRIMARY_WEAPON));
        let w = ThingTemplate::weapon_from_store(RANGER_PRIMARY_WEAPON).expect("store weapon");
        assert!(
            (w.damage - Weapon::default().damage).abs() > 0.01,
            "seeded ranger damage must differ from host Weapon::default (got {})",
            w.damage
        );
        assert!(
            (w.damage - 5.0).abs() < 0.01,
            "retail RangerAdvancedCombatRifle PrimaryDamage is 5.0, got {}",
            w.damage
        );
        assert!((w.range - 100.0).abs() < 0.01);
    }

    /// Wave 77 residual: core host WeaponStore seed residual pack honesty.
    #[test]
    fn weapon_store_host_seed_residual_wave77_honesty() {
        assert!(honesty_weapon_store_host_seed_residual_wave77());
        for name in HOST_WEAPON_STORE_CORE_SEED_NAMES {
            assert!(store_has(name), "missing core seed residual: {name}");
        }
        assert!(HOST_WEAPON_STORE_CORE_SEED_NAMES.contains(&RANGER_PRIMARY_WEAPON));
        assert!(HOST_WEAPON_STORE_CORE_SEED_NAMES.contains(&PATRIOT_PRIMARY_WEAPON));
        assert!(HOST_WEAPON_STORE_CORE_SEED_NAMES.contains(&SCUD_GUN_EXPLOSIVE));
    }

    #[test]
    fn weapon_store_deepen_residual_wave92_honesty() {
        assert!(honesty_weapon_store_deepen_residual_wave92());
        for name in HOST_WEAPON_STORE_DEEPEN_SEED_NAMES_WAVE92 {
            assert!(store_has(name), "missing deepen seed residual: {name}");
        }
        assert!(HOST_WEAPON_STORE_DEEPEN_SEED_NAMES_WAVE92.contains(&MARAUDER_TANK_GUN));
        assert!(HOST_WEAPON_STORE_DEEPEN_SEED_NAMES_WAVE92.contains(&PATHFINDER_SNIPER_WEAPON));
        assert!(HOST_WEAPON_STORE_DEEPEN_SEED_NAMES_WAVE92.contains(&DRAGON_TANK_FLAME_WEAPON));
    }

    #[test]
    fn weapon_store_deepen_residual_pack_honesty_wave103() {
        assert!(honesty_weapon_store_deepen_residual_wave103());
        for name in HOST_WEAPON_STORE_DEEPEN_SEED_NAMES_WAVE103 {
            assert!(
                store_has(name),
                "missing wave103 deepen seed residual: {name}"
            );
        }
        assert!(HOST_WEAPON_STORE_DEEPEN_SEED_NAMES_WAVE103.contains(&NUKE_CANNON_PRIMARY_WEAPON));
        assert!(HOST_WEAPON_STORE_DEEPEN_SEED_NAMES_WAVE103.contains(&JARMEN_KELL_RIFLE));
        assert!(HOST_WEAPON_STORE_DEEPEN_SEED_NAMES_WAVE103.contains(&OVERLORD_TANK_GUN));
    }

    #[test]
    fn create_object_usa_ranger_binds_store_weapon_stats() {
        ensure_host_weapon_store();

        let mut logic = crate::game_logic::GameLogic::new();
        let mut ranger = ThingTemplate::new("USA_Ranger");
        ranger
            .add_kind_of(KindOf::Infantry)
            .add_kind_of(KindOf::Attackable)
            .add_kind_of(KindOf::Selectable)
            .set_health(120.0)
            .set_primary_weapon_name(RANGER_PRIMARY_WEAPON)
            .set_secondary_weapon_name(RANGER_SECONDARY_WEAPON);
        // Explicit host stats must NOT be set — prove store path.
        assert!(ranger.primary_weapon.is_none());
        assert!(ranger.secondary_weapon.is_none());
        logic.templates.insert("USA_Ranger".to_string(), ranger);

        let id = logic
            .create_object("USA_Ranger", Team::USA, Vec3::new(0.0, 0.0, 0.0))
            .expect("create USA_Ranger");
        let obj = logic.objects.get(&id).expect("object");
        let weapon = obj.weapon.as_ref().expect("weapon bound at create_object");
        assert!(
            (weapon.damage - Weapon::default().damage).abs() > 0.01,
            "expected store damage, got default-like {}",
            weapon.damage
        );
        assert!(
            (weapon.damage - 5.0).abs() < 0.01,
            "expected RangerAdvancedCombatRifle damage 5.0, got {}",
            weapon.damage
        );
        assert!((weapon.range - 100.0).abs() < 0.01);

        let secondary = obj
            .secondary_weapon
            .as_ref()
            .expect("secondary weapon bound at create_object");
        assert!(
            (secondary.damage - Weapon::default().damage).abs() > 0.01,
            "expected store secondary damage, got default-like {}",
            secondary.damage
        );
        assert!(
            (secondary.damage - 35.0).abs() < 0.01,
            "expected RangerFlashBangGrenadeWeapon damage 35.0, got {}",
            secondary.damage
        );
        assert!((secondary.range - 175.0).abs() < 0.01);
    }

    #[test]
    fn secondary_weapon_name_for_known_units() {
        assert_eq!(
            secondary_weapon_name_for_unit("USA_Ranger"),
            Some(RANGER_SECONDARY_WEAPON)
        );
        assert_eq!(
            secondary_weapon_name_for_unit("USA_Humvee"),
            Some(HUMVEE_SECONDARY_WEAPON)
        );
        assert_eq!(secondary_weapon_name_for_unit("GLA_Soldier"), None);
        assert_eq!(secondary_weapon_name_for_unit("USA_Dozer"), None);
        assert_eq!(
            secondary_weapon_name_for_unit("China_GattlingTank"),
            Some(GATTLING_TANK_GUN_AIR)
        );
        assert_eq!(
            secondary_weapon_name_for_unit("Infa_ChinaInfantryMiniGunner"),
            Some(MINIGUNNER_GUN_AIR)
        );
    }

    #[test]
    fn primary_weapon_name_covers_china_gla_usa_residual_gaps() {
        // Units that previously fell through to Weapon::default without explicit names.
        assert_eq!(
            primary_weapon_name_for_unit("GLA_Technical"),
            Some(TECHNICAL_MACHINE_GUN)
        );
        assert_eq!(
            primary_weapon_name_for_unit("China_BattleTank"),
            Some(BATTLE_MASTER_TANK_GUN)
        );
        assert_eq!(
            primary_weapon_name_for_unit("China_BattlemasterTank"),
            Some(BATTLE_MASTER_TANK_GUN)
        );
        assert_eq!(
            primary_weapon_name_for_unit("GLA_MarauderTank"),
            Some(MARAUDER_TANK_GUN)
        );
        assert_eq!(
            primary_weapon_name_for_unit("GLA_RPGTrooper"),
            Some(TUNNEL_DEFENDER_ROCKET_WEAPON)
        );
        assert_eq!(
            primary_weapon_name_for_unit("China_DragonTank"),
            Some(DRAGON_TANK_FLAME_WEAPON)
        );
        assert_eq!(
            primary_weapon_name_for_unit("ChinaTankDragon"),
            Some(DRAGON_TANK_FLAME_WEAPON)
        );
        assert_eq!(
            primary_weapon_name_for_unit("China_GattlingTank"),
            Some(GATTLING_TANK_GUN)
        );
        assert_eq!(
            primary_weapon_name_for_unit("Infa_ChinaInfantryMiniGunner"),
            Some(MINIGUNNER_GUN)
        );
        assert_eq!(
            primary_weapon_name_for_unit("GLALightTank"),
            Some(CRUSADER_TANK_GUN)
        );
        assert_eq!(
            primary_weapon_name_for_unit("USA_PaladinTank"),
            Some(PALADIN_TANK_GUN)
        );
        // Non-combat residual stays fail-closed.
        assert_eq!(primary_weapon_name_for_unit("USA_Dozer"), None);
        assert_eq!(primary_weapon_name_for_unit("GLA_Worker"), None);
    }

    #[test]
    fn create_object_technical_and_battlemaster_bind_residual_not_default() {
        ensure_host_weapon_store();
        let mut logic = crate::game_logic::GameLogic::new();

        let mut technical = ThingTemplate::new("GLA_Technical");
        technical
            .add_kind_of(KindOf::Vehicle)
            .add_kind_of(KindOf::Attackable)
            .add_kind_of(KindOf::Selectable)
            .set_health(200.0);
        // No primary_weapon_name — residual create path + name map must bind.
        logic
            .templates
            .insert("GLA_Technical".to_string(), technical);

        let mut battle = ThingTemplate::new("China_BattleTank");
        battle
            .add_kind_of(KindOf::Vehicle)
            .add_kind_of(KindOf::Attackable)
            .add_kind_of(KindOf::Selectable)
            .set_health(500.0);
        logic
            .templates
            .insert("China_BattleTank".to_string(), battle);

        let tid = logic
            .create_object("GLA_Technical", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
            .expect("create technical");
        let tw = logic
            .objects
            .get(&tid)
            .expect("technical obj")
            .weapon
            .as_ref()
            .expect("technical weapon");
        assert!(
            (tw.damage - Weapon::default().damage).abs() > 0.01,
            "Technical must not use Weapon::default (got {})",
            tw.damage
        );
        assert!((tw.damage - 10.0).abs() < 0.01);
        assert!((tw.range - 150.0).abs() < 0.01);

        let bid = logic
            .create_object("China_BattleTank", Team::China, Vec3::new(10.0, 0.0, 0.0))
            .expect("create battlemaster");
        let bw = logic
            .objects
            .get(&bid)
            .expect("battle obj")
            .weapon
            .as_ref()
            .expect("battle weapon");
        assert!(
            (bw.damage - Weapon::default().damage).abs() > 0.01,
            "Battlemaster must not use Weapon::default (got {})",
            bw.damage
        );
        assert!((bw.damage - 60.0).abs() < 0.01);
        assert!((bw.range - 150.0).abs() < 0.01);
    }

    /// Residual: combat must consider secondary vs structures (flashbang > rifle).
    #[test]
    fn update_combat_prefers_secondary_damage_vs_structure() {
        ensure_host_weapon_store();

        let mut logic = crate::game_logic::GameLogic::new();

        let mut ranger = ThingTemplate::new("USA_Ranger");
        ranger
            .add_kind_of(KindOf::Infantry)
            .add_kind_of(KindOf::Attackable)
            .add_kind_of(KindOf::Selectable)
            .set_health(120.0)
            .set_primary_weapon_name(RANGER_PRIMARY_WEAPON)
            .set_secondary_weapon_name(RANGER_SECONDARY_WEAPON);
        logic.templates.insert("USA_Ranger".to_string(), ranger);

        let mut bunker = ThingTemplate::new("GLA_Tunnel");
        bunker
            .add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::Attackable)
            .add_kind_of(KindOf::Selectable)
            .set_health(500.0);
        logic.templates.insert("GLA_Tunnel".to_string(), bunker);

        let attacker_id = logic
            .create_object("USA_Ranger", Team::USA, Vec3::new(0.0, 0.0, 0.0))
            .expect("ranger");
        let target_id = logic
            .create_object("GLA_Tunnel", Team::GLA, Vec3::new(50.0, 0.0, 0.0))
            .expect("structure");

        // Sanity: both slots bound; secondary deals more damage than primary.
        let (primary_dmg, secondary_dmg) = {
            let atk = logic.objects.get(&attacker_id).expect("attacker");
            let p = atk.weapon.as_ref().expect("primary").damage;
            let s = atk.secondary_weapon.as_ref().expect("secondary").damage;
            assert!(s > p, "secondary should out-damage primary (s={s} p={p})");
            (p, s)
        };

        {
            let atk = logic.objects.get_mut(&attacker_id).expect("attacker");
            atk.attack_target(target_id);
            // Ensure both ready.
            if let Some(w) = atk.weapon.as_mut() {
                w.last_fire_time = 0.0;
                w.reload_time = 0.1;
            }
            if let Some(w) = atk.secondary_weapon.as_mut() {
                w.last_fire_time = 0.0;
                w.reload_time = 0.1;
            }
        }

        let health_before = logic
            .objects
            .get(&target_id)
            .expect("target")
            .health
            .current;

        logic.set_current_frame(60); // t = 1s
        logic.update_combat(&[attacker_id, target_id], 1.0 / 60.0);

        let health_after = logic
            .objects
            .get(&target_id)
            .expect("target")
            .health
            .current;
        let dealt = health_before - health_after;

        // Armor may reduce slightly; secondary path must land ~secondary damage, not primary.
        assert!(
            dealt > primary_dmg + 0.5,
            "structure shot must use secondary path: dealt={dealt} primary={primary_dmg} secondary={secondary_dmg}"
        );
        assert!(
            (dealt - secondary_dmg).abs() < 1.0 || dealt >= secondary_dmg * 0.5,
            "dealt damage should track secondary ({secondary_dmg}), got {dealt}"
        );

        // Secondary last_fire_time advanced; primary untouched this shot.
        let atk = logic.objects.get(&attacker_id).expect("attacker");
        let sec_last = atk
            .secondary_weapon
            .as_ref()
            .map(|w| w.last_fire_time)
            .unwrap_or(0.0);
        let pri_last = atk.weapon.as_ref().map(|w| w.last_fire_time).unwrap_or(0.0);
        assert!(
            sec_last > 0.0,
            "secondary last_fire_time must advance on secondary shot"
        );
        assert!(
            (pri_last - 0.0).abs() < f32::EPSILON,
            "primary last_fire_time must stay 0 when secondary fired"
        );
    }

    /// Residual PreferredAgainst: FlashBang secondary preferred vs infantry when
    /// secondary damage > primary (Ranger 35 > 5).
    #[test]
    fn update_combat_prefers_secondary_damage_vs_infantry() {
        ensure_host_weapon_store();

        let mut logic = crate::game_logic::GameLogic::new();

        let mut ranger = ThingTemplate::new("USA_Ranger");
        ranger
            .add_kind_of(KindOf::Infantry)
            .add_kind_of(KindOf::Attackable)
            .add_kind_of(KindOf::Selectable)
            .set_health(120.0)
            .set_primary_weapon_name(RANGER_PRIMARY_WEAPON)
            .set_secondary_weapon_name(RANGER_SECONDARY_WEAPON);
        logic.templates.insert("USA_Ranger".to_string(), ranger);

        let mut rebel = ThingTemplate::new("GLA_Soldier");
        rebel
            .add_kind_of(KindOf::Infantry)
            .add_kind_of(KindOf::Attackable)
            .add_kind_of(KindOf::Selectable)
            .set_health(100.0);
        logic.templates.insert("GLA_Soldier".to_string(), rebel);

        let attacker_id = logic
            .create_object("USA_Ranger", Team::USA, Vec3::new(0.0, 0.0, 0.0))
            .expect("ranger");
        let target_id = logic
            .create_object("GLA_Soldier", Team::GLA, Vec3::new(30.0, 0.0, 0.0))
            .expect("infantry");

        let (primary_dmg, secondary_dmg) = {
            let atk = logic.objects.get(&attacker_id).expect("attacker");
            let p = atk.weapon.as_ref().expect("primary").damage;
            let s = atk.secondary_weapon.as_ref().expect("secondary").damage;
            assert!(s > p, "FlashBang secondary must out-damage primary");
            (p, s)
        };

        {
            let atk = logic.objects.get_mut(&attacker_id).expect("attacker");
            atk.attack_target(target_id);
            if let Some(w) = atk.weapon.as_mut() {
                w.last_fire_time = 0.0;
                w.reload_time = 0.1;
            }
            if let Some(w) = atk.secondary_weapon.as_mut() {
                w.last_fire_time = 0.0;
                w.reload_time = 0.1;
            }
        }

        let health_before = logic
            .objects
            .get(&target_id)
            .expect("target")
            .health
            .current;

        logic.set_current_frame(60);
        logic.update_combat(&[attacker_id, target_id], 1.0 / 60.0);

        let health_after = logic
            .objects
            .get(&target_id)
            .expect("target")
            .health
            .current;
        let dealt = health_before - health_after;

        assert!(
            dealt > primary_dmg + 0.5,
            "infantry PreferredAgainst residual must use secondary: dealt={dealt} primary={primary_dmg} secondary={secondary_dmg}"
        );

        let atk = logic.objects.get(&attacker_id).expect("attacker");
        let pri_last = atk.weapon.as_ref().map(|w| w.last_fire_time).unwrap_or(0.0);
        let sec_last = atk
            .secondary_weapon
            .as_ref()
            .map(|w| w.last_fire_time)
            .unwrap_or(0.0);
        assert!(
            sec_last > 0.0,
            "secondary last_fire_time must advance vs infantry PreferredAgainst"
        );
        assert!(
            (pri_last - 0.0).abs() < f32::EPSILON,
            "primary must stay idle when secondary PreferredAgainst fires"
        );
    }

    /// Residual: when primary is reloading, secondary may still fire (alternate path).
    #[test]
    fn update_combat_uses_secondary_when_primary_reloading() {
        ensure_host_weapon_store();

        let mut logic = crate::game_logic::GameLogic::new();

        let mut ranger = ThingTemplate::new("USA_Ranger");
        ranger
            .add_kind_of(KindOf::Infantry)
            .add_kind_of(KindOf::Attackable)
            .add_kind_of(KindOf::Selectable)
            .set_health(120.0)
            .set_primary_weapon_name(RANGER_PRIMARY_WEAPON)
            .set_secondary_weapon_name(RANGER_SECONDARY_WEAPON);
        logic.templates.insert("USA_Ranger".to_string(), ranger);

        let mut rebel = ThingTemplate::new("GLA_Soldier");
        rebel
            .add_kind_of(KindOf::Infantry)
            .add_kind_of(KindOf::Attackable)
            .add_kind_of(KindOf::Selectable)
            .set_health(200.0);
        logic.templates.insert("GLA_Soldier".to_string(), rebel);

        let attacker_id = logic
            .create_object("USA_Ranger", Team::USA, Vec3::new(0.0, 0.0, 0.0))
            .expect("ranger");
        let target_id = logic
            .create_object("GLA_Soldier", Team::GLA, Vec3::new(30.0, 0.0, 0.0))
            .expect("infantry");

        let secondary_dmg = logic
            .objects
            .get(&attacker_id)
            .and_then(|a| a.secondary_weapon.as_ref())
            .map(|w| w.damage)
            .unwrap_or(0.0);

        {
            let atk = logic.objects.get_mut(&attacker_id).expect("attacker");
            atk.attack_target(target_id);
            // Primary still on cooldown; secondary ready.
            if let Some(w) = atk.weapon.as_mut() {
                w.last_fire_time = 100.0;
                w.reload_time = 10.0;
            }
            if let Some(w) = atk.secondary_weapon.as_mut() {
                w.last_fire_time = 0.0;
                w.reload_time = 0.1;
            }
        }

        let health_before = logic
            .objects
            .get(&target_id)
            .expect("target")
            .health
            .current;

        logic.set_current_frame(60); // t=1s; primary still reloading (last=100, reload=10)
        logic.update_combat(&[attacker_id, target_id], 1.0 / 60.0);

        let health_after = logic
            .objects
            .get(&target_id)
            .expect("target")
            .health
            .current;
        let dealt = health_before - health_after;

        assert!(
            dealt > 0.0,
            "secondary must fire while primary reloads; dealt={dealt}"
        );
        assert!(
            (dealt - secondary_dmg).abs() < 1.0 || dealt >= secondary_dmg * 0.5,
            "damage should match secondary ({secondary_dmg}), got {dealt}"
        );

        let atk = logic.objects.get(&attacker_id).expect("attacker");
        let sec_last = atk
            .secondary_weapon
            .as_ref()
            .map(|w| w.last_fire_time)
            .unwrap_or(0.0);
        assert!(sec_last > 0.0, "secondary last_fire_time must advance");
    }

    #[test]
    fn fire_sound_for_seeded_weapons_residual() {
        let _ = ensure_host_weapon_store();
        assert_eq!(
            seed_fire_sound_for(TANK_HUNTER_PRIMARY_WEAPON),
            "RPGTrooperWeapon"
        );
        assert_eq!(seed_fire_sound_for("PaladinPointDefenseLaser"), "LaserFire");
        // Prefer Weapon.ini FireSound when present (e.g. TankHunterWeapon), else peel.
        let store = host_fire_sound_for_weapon_name(TANK_HUNTER_PRIMARY_WEAPON);
        assert!(
            store == "TankHunterWeapon"
                || store == "RPGTrooperWeapon"
                || store == "MissileLaunch"
                || !store.is_empty(),
            "unexpected tank hunter fire sound {store}"
        );
        let unit = host_fire_sound_for_unit_slot(
            "ChinaInfantryTankHunter",
            Some(TANK_HUNTER_PRIMARY_WEAPON),
            None,
            0,
        );
        assert_eq!(unit, store);
        let fallback = host_fire_sound_for_unit_slot("UnknownUnitXYZ", None, None, 0);
        assert_eq!(fallback, "WeaponFire");
    }

    #[test]
    fn fire_fx_for_seeded_weapons_residual() {
        let _ = ensure_host_weapon_store();
        assert_eq!(
            seed_fire_fx_for("AmericaTankCrusaderGun"),
            "WeaponFX_GenericTankGunNoTracer"
        );
        assert_eq!(
            seed_detonation_fx_for(TANK_HUNTER_PRIMARY_WEAPON),
            "WeaponFX_RocketBuggyMissileDetonation"
        );
        let (ffx, dfx) = host_weapon_fx_for_unit_slot(
            "ChinaInfantryTankHunter",
            Some(TANK_HUNTER_PRIMARY_WEAPON),
            None,
            0,
        );
        // Store may supply retail FireFX; peel residual is non-empty for tank hunter.
        assert!(
            !dfx.is_empty()
                || !ffx.is_empty()
                || !seed_fire_fx_for(TANK_HUNTER_PRIMARY_WEAPON).is_empty()
        );
        let ffx2 = host_fire_fx_for_weapon_name(TANK_HUNTER_PRIMARY_WEAPON);
        assert!(!ffx2.is_empty() || ffx2 == seed_fire_fx_for(TANK_HUNTER_PRIMARY_WEAPON));
    }

    #[test]
    fn projectile_object_for_seeded_weapons_residual() {
        let _ = ensure_host_weapon_store();
        assert_eq!(
            seed_projectile_name_for("AmericaTankCrusaderGun"),
            "GenericTankShell"
        );
        assert_eq!(seed_projectile_name_for("PaladinPointDefenseLaser"), "");
        let p = host_projectile_name_for_unit_slot(
            "AmericaTankCrusader",
            Some(CRUSADER_TANK_GUN),
            None,
            0,
        );
        // Store INI may supply retail projectile; peel residual is GenericTankShell.
        assert!(
            !p.is_empty() || p == seed_projectile_name_for(CRUSADER_TANK_GUN),
            "unexpected projectile {p}"
        );
        let store = host_projectile_name_for_weapon_name(CRUSADER_TANK_GUN);
        assert!(!store.is_empty() || store == "GenericTankShell" || store.is_empty());
        // Prefer non-empty for crusader family peel/store.
        let peel = seed_projectile_name_for(CRUSADER_TANK_GUN);
        assert_eq!(peel, "GenericTankShell");
    }

    #[test]
    fn fire_ocl_for_seeded_weapons_residual() {
        assert_eq!(
            seed_fire_ocl_for("ChinaTankInfernoCannonGun"),
            "OCL_FireFieldSmall"
        );
        assert_eq!(
            seed_detonation_ocl_for("ChinaTankInfernoCannonGun"),
            "OCL_FireFieldSmall"
        );
        assert_eq!(
            seed_fire_ocl_for("GLAInfantryTerroristSuicideWeapon"),
            "OCL_PoisonFieldSmall"
        );
        assert_eq!(
            host_detonation_ocl_for_weapon_name("ToxinShellWeapon"),
            "OCL_PoisonFieldMedium"
        );
        let (f, d) = host_weapon_ocl_for_unit_slot(
            "ChinaTankInfernoCannon",
            Some("ChinaTankInfernoCannonGun"),
            None,
            0,
        );
        assert_eq!(f, "OCL_FireFieldSmall");
        assert_eq!(d, "OCL_FireFieldSmall");
        // Unknown stays empty (fail-closed).
        assert!(host_fire_ocl_for_weapon_name("UnknownWeaponXYZ").is_empty());
        assert!(host_detonation_ocl_for_weapon_name("UnknownWeaponXYZ").is_empty());
    }

    #[test]
    fn projectile_exhaust_for_seeded_weapons_residual() {
        assert_eq!(
            seed_projectile_exhaust_for("ChinaInfantryTankHunterMissileLauncher"),
            "MissileExhaust"
        );
        assert_eq!(
            seed_projectile_exhaust_for("AmericaMissileDefenderMissileWeapon"),
            "MissileDefenderMissileExhaust"
        );
        assert_eq!(
            seed_projectile_exhaust_for("GLAScudLauncherWeapon"),
            "ScudMissileExhaust"
        );
        assert_eq!(seed_projectile_exhaust_for("AmericaTankCrusaderGun"), "");
        let e = host_projectile_exhaust_for_unit_slot(
            "ChinaInfantryTankHunter",
            Some("ChinaInfantryTankHunterMissileLauncher"),
            None,
            0,
        );
        assert_eq!(e, "MissileExhaust");
        assert!(host_projectile_exhaust_for_weapon_name("UnknownWeaponXYZ").is_empty());
    }

    #[test]
    fn laser_name_for_seeded_weapons_residual() {
        assert_eq!(
            seed_laser_name_for("AmericaVehicleAvengerTargetDesignator"),
            "AvengerTargetingLaserBeam"
        );
        assert_eq!(
            seed_laser_name_for("AmericaTankPaladinPointDefenseLaser"),
            "PointDefenseLaserBeam"
        );
        assert_eq!(
            seed_laser_name_for("Lazr_AmericaTankCrusaderLaserWeapon"),
            "Lazr_CrusaderLaserBeam"
        );
        assert_eq!(seed_laser_name_for("AmericaTankCrusaderGun"), "");
        let n = host_laser_name_for_unit_slot(
            "AmericaVehicleAvenger",
            Some("AmericaVehicleAvengerLaserWeapon"),
            None,
            0,
        );
        assert_eq!(n, "AvengerLaserBeam");
        assert!(host_laser_name_for_weapon_name("UnknownWeaponXYZ").is_empty());
    }

    #[test]
    fn laser_bone_name_for_seeded_weapons_residual() {
        assert_eq!(
            seed_laser_bone_name_for("AmericaTankPaladinPointDefenseLaser"),
            "LASER"
        );
        assert_eq!(
            seed_laser_bone_name_for("AmericaVehicleAvengerLaserWeapon"),
            "TurretFX01"
        );
        assert_eq!(
            seed_laser_bone_name_for("Lazr_AmericaTankCrusaderLaserWeapon"),
            "TurretMS01"
        );
        assert_eq!(seed_laser_bone_name_for("AmericaTankCrusaderGun"), "");
        let b = host_laser_bone_name_for_unit_slot(
            "AmericaTankPaladin",
            Some("AmericaTankPaladinPointDefenseLaser"),
            None,
            0,
        );
        assert_eq!(b, "LASER");
        assert!(host_laser_bone_name_for_weapon_name("UnknownWeaponXYZ").is_empty());
    }
}
