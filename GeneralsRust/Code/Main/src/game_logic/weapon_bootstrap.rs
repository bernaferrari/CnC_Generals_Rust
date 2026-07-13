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
pub const HUMVEE_PRIMARY_WEAPON: &str = "HumveeGun";

/// Retail secondary weapon names used by host golden / skirmish unit templates.
/// Fail-closed residual: only units that need SECONDARY in host combat probes.
pub const RANGER_SECONDARY_WEAPON: &str = "RangerFlashBangGrenadeWeapon";
pub const HUMVEE_SECONDARY_WEAPON: &str = "HumveeMissileWeapon";

/// Retail base-defense primary weapons (Patriot / Gattling structure residual).
pub const PATRIOT_PRIMARY_WEAPON: &str = "PatriotMissileWeapon";
pub const GATTLING_BUILDING_PRIMARY_WEAPON: &str = "GattlingBuildingGun";

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

/// Retail StealthJetMissileWeapon residual (Bunker Buster carrier primary).
pub const STEALTH_JET_MISSILE_WEAPON: &str = "StealthJetMissileWeapon";

/// Retail MicrowaveTankBuildingClearer residual (KILL_GARRISONED damage type).
pub const MICROWAVE_BUILDING_CLEARER_WEAPON: &str = "MicrowaveTankBuildingClearer";

/// Retail Comanche primary / rocket-pod residual weapons.
pub const COMANCHE_PRIMARY_WEAPON: &str = "Comanche20mmCannonWeapon";
pub const COMANCHE_ROCKET_POD_WEAPON: &str = "ComancheRocketPodWeapon";

/// Retail Sentry Drone gun residual weapon (PLAYER_UPGRADE primary).
pub const SENTRY_DRONE_GUN_WEAPON: &str = "SentryDroneGun";

static BOOTSTRAP_ATTEMPTED: AtomicBool = AtomicBool::new(false);

/// Initialize the GameLogic WeaponStore (if needed) and ensure host combat
/// weapons are registered. Safe to call repeatedly.
///
/// Returns how many templates were added by this call (seed + filesystem load).
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
        "China_RedGuard" | "China_Soldier" | "ChinaInfantryRedguard" => {
            Some(REDGUARD_PRIMARY_WEAPON)
        }
        "USA_Humvee" | "AmericaVehicleHumvee" => Some(HUMVEE_PRIMARY_WEAPON),
        // Base-defense structures (Patriot / Gattling residual auto-fire).
        "USA_Patriot" | "USA_PatriotMissile" | "AmericaPatriotBattery" | "PatriotMissile" => {
            Some(PATRIOT_PRIMARY_WEAPON)
        }
        "China_GattlingCannon" | "ChinaGattlingCannon" => Some(GATTLING_BUILDING_PRIMARY_WEAPON),
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
        // Sentry gun is PLAYER_UPGRADE only — no primary until research residual.
        "AmericaVehicleSentryDrone"
        | "USA_SentryDrone"
        | "TestSentryDrone"
        | "AirF_AmericaVehicleSentryDrone"
        | "SupW_AmericaVehicleSentryDrone"
        | "Lazr_AmericaVehicleSentryDrone" => None,
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
            // Sentry without gun upgrade has no residual primary (fail-closed).
            if crate::game_logic::host_sentry_drone::is_sentry_drone_template(template_name) {
                return None;
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
        "USA_Humvee" | "AmericaVehicleHumvee" => Some(HUMVEE_SECONDARY_WEAPON),
        "China_NukeCannon"
        | "ChinaVehicleNukeCannon"
        | "Nuke_ChinaVehicleNukeCannon"
        | "TestNukeCannon" => Some(NUKE_CANNON_NEUTRON_WEAPON),
        // Rocket pods are PLAYER_UPGRADE residual — not bound at create; research equips.
        "AmericaVehicleComanche"
        | "USA_Comanche"
        | "TestComanche"
        | "AirF_AmericaVehicleComanche"
        | "SupW_AmericaVehicleComanche"
        | "Lazr_AmericaVehicleComanche" => None,
        _ => {
            if crate::game_logic::host_neutron_shell::is_nuke_cannon_template(template_name) {
                Some(NUKE_CANNON_NEUTRON_WEAPON)
            } else {
                // Comanche rocket pods are upgrade-gated (fail-closed at spawn).
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
                let n = crate::assets::ini_template_loader::register_weapons_from_ini_text(
                    &content,
                );
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
        // DelayBetweenShots 250ms → 8 frames @ 30 FPS.
        SeedWeapon {
            name: PATRIOT_PRIMARY_WEAPON,
            primary_damage: 30.0,
            attack_range: 225.0,
            delay_frames: 8,
            clip_size: 4,
            weapon_speed: 1.0,
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
        // ChinaVehicleNukeCannon PRIMARY — NukeCannonGun residual seed.
        // PrimaryDamage ~200 retail shell; host residual uses 200 / range 350.
        SeedWeapon {
            name: NUKE_CANNON_PRIMARY_WEAPON,
            primary_damage: 200.0,
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
        // ComancheRocketPodWeapon residual SECONDARY (retail TERTIARY).
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
        t.anti_mask.insert(WeaponAntiMask::GROUND);
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
        let pri_last = atk
            .weapon
            .as_ref()
            .map(|w| w.last_fire_time)
            .unwrap_or(0.0);
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
}
