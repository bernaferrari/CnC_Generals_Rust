//! Host LocomotorStore bootstrap for template → movement speed binding.
//!
//! # Why host Movement defaults to 10 u/s
//!
//! `Object::new` uses `Movement::default()` (max_speed 10, accel 5). Full
//! Locomotor.ini population happens when the engine loads BIG archives into
//! Common's `ini_locomotor` store. Headless unit tests and many host probes
//! never open archives, so rangers stayed at the host default and golden
//! skirmish had to lift them toward retail BasicHumanLocomotor (20).
//!
//! This module is the reliable host-side fill path (mirrors `weapon_bootstrap`):
//! - Prefer loading extracted / shipped `Data/INI/Locomotor.ini` when present
//! - Always seed a small set of golden-unit locomotors if still missing
//! - Bind SET_NORMAL locomotor names on known host unit templates
//!
//! Fail-closed residual:
//! - Not a full multi-locomotor set / surface-type matrix / SET_PANIC upgrades
//! - Host binds only primary SET_NORMAL speed/accel/turn for create_object
//! - Common store stores per-frame values (C++ parity); host Movement is
//!   dist/sec and rads/sec, so we convert when resolving for Object.movement

use game_engine::common::ini::ini_locomotor::{
    get_locomotor_store, get_locomotor_store_mut, load_locomotors_from_str,
    parse_locomotor_template_definition, LocomotorTemplate,
};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

/// Retail SET_NORMAL locomotor names used by host golden / skirmish unit templates.
pub const BASIC_HUMAN_LOCOMOTOR: &str = "BasicHumanLocomotor";
pub const REDGUARD_LOCOMOTOR: &str = "RedguardLocomotor";
pub const HUMVEE_LOCOMOTOR: &str = "HumveeLocomotor";
pub const CRUSADER_LOCOMOTOR: &str = "CrusaderLocomotor";
pub const SCORPION_LOCOMOTOR: &str = "ScorpionLocomotor";
pub const BATTLE_MASTER_LOCOMOTOR: &str = "BattleMasterLocomotor";
pub const TECHNICAL_LOCOMOTOR: &str = "TechnicalLocomotor";

// --- Wave 81 common-unit locomotor residual deepen ---
/// Retail Pathfinder / Colonel Burton ground residual.
pub const COLONEL_BURTON_GROUND_LOCOMOTOR: &str = "ColonelBurtonGroundLocomotor";
/// Retail AmericaVehicleTomahawk residual.
pub const TOMAHAWK_LOCOMOTOR: &str = "TomahawkLocomotor";
/// Retail GLAVehicleSCUDLauncher residual.
pub const SCUD_LAUNCHER_LOCOMOTOR: &str = "ScudLauncherLocomotor";
/// Retail GLAVehicleQuadCannon residual.
pub const QUAD_CANNON_LOCOMOTOR: &str = "QuadCannonLocomotor";
/// Retail AmericaJetRaptor residual.
pub const RAPTOR_JET_LOCOMOTOR: &str = "RaptorJetLocomotor";

// --- Wave 92 common-unit locomotor residual expand ---
/// Retail ChinaTankOverlord residual.
pub const OVERLORD_LOCOMOTOR: &str = "OverlordLocomotor";
/// Retail GLATankMarauder residual.
pub const MARAUDER_LOCOMOTOR: &str = "MarauderLocomotor";
/// Retail ChinaTankDragon residual.
pub const DRAGON_LOCOMOTOR: &str = "DragonLocomotor";
/// Retail AmericaVehicleComanche residual.
pub const COMANCHE_LOCOMOTOR: &str = "ComancheLocomotor";
/// Retail ChinaJetMIG residual.
pub const MIG_LOCOMOTOR: &str = "MIGLocomotor";
/// Retail GLAVehicleRocketBuggy residual.
pub const ROCKET_BUGGY_LOCOMOTOR: &str = "RocketBuggyLocomotor";
/// Retail GLAVehicleBattleBus residual.
pub const BATTLE_BUS_LOCOMOTOR: &str = "BattleBusLocomotor";
/// Retail SupplyTruck residual.
pub const SUPPLY_TRUCK_LOCOMOTOR: &str = "SupplyTruckLocomotor";
/// Retail AmericaVehicleAvenger residual.
pub const AVENGER_LOCOMOTOR: &str = "AvengerLocomotor";
/// Retail ChinaTankGattling residual.
pub const GATTLING_TANK_LOCOMOTOR: &str = "GattlingTankLocomotor";
/// Retail ChinaVehicleInfernoCannon residual.
pub const INFERNO_LOCOMOTOR: &str = "InfernoLocomotor";
/// Retail AmericaVehicleDozer residual.
pub const AMERICA_DOZER_LOCOMOTOR: &str = "AmericaVehicleDozerLocomotor";
/// Retail ChinaVehicleHelix residual.
pub const HELIX_LOCOMOTOR: &str = "HelixLocomotor";

/// Wave 81/92 residual seed table: (name, Speed, Acceleration, TurnRate deg).
/// Values match retail Locomotor.ini for common host units.
pub const HOST_LOCOMOTOR_SEED_RESIDUAL_TABLE: &[(&str, f32, f32, f32)] = &[
    (BASIC_HUMAN_LOCOMOTOR, 20.0, 100.0, 500.0),
    (REDGUARD_LOCOMOTOR, 25.0, 100.0, 500.0),
    (HUMVEE_LOCOMOTOR, 60.0, 1000.0, 180.0),
    (CRUSADER_LOCOMOTOR, 30.0, 1000.0, 180.0),
    (SCORPION_LOCOMOTOR, 40.0, 1000.0, 180.0),
    (BATTLE_MASTER_LOCOMOTOR, 25.0, 1000.0, 180.0),
    (TECHNICAL_LOCOMOTOR, 90.0, 100.0, 180.0),
    // Wave 81 deepen:
    (COLONEL_BURTON_GROUND_LOCOMOTOR, 30.0, 100.0, 500.0),
    (TOMAHAWK_LOCOMOTOR, 30.0, 1000.0, 180.0),
    (SCUD_LAUNCHER_LOCOMOTOR, 20.0, 160.0, 50.0),
    (QUAD_CANNON_LOCOMOTOR, 40.0, 1000.0, 180.0),
    (RAPTOR_JET_LOCOMOTOR, 175.0, 120.0, 120.0),
    // Wave 92 expand:
    (OVERLORD_LOCOMOTOR, 20.0, 15.0, 60.0),
    (MARAUDER_LOCOMOTOR, 40.0, 1000.0, 180.0),
    (DRAGON_LOCOMOTOR, 30.0, 1000.0, 180.0),
    (COMANCHE_LOCOMOTOR, 120.0, 60.0, 180.0),
    (MIG_LOCOMOTOR, 160.0, 110.0, 120.0),
    (ROCKET_BUGGY_LOCOMOTOR, 90.0, 90.0, 180.0),
    (BATTLE_BUS_LOCOMOTOR, 70.0, 1000.0, 90.0),
    (SUPPLY_TRUCK_LOCOMOTOR, 40.0, 240.0, 90.0),
    (AVENGER_LOCOMOTOR, 30.0, 1000.0, 180.0),
    (GATTLING_TANK_LOCOMOTOR, 40.0, 1000.0, 180.0),
    (INFERNO_LOCOMOTOR, 30.0, 1000.0, 120.0),
    (AMERICA_DOZER_LOCOMOTOR, 30.0, 30.0, 90.0),
    (HELIX_LOCOMOTOR, 75.0, 60.0, 180.0),
];

/// Logic FPS used by C++ Locomotor.ini unit conversion (Speed / 30 → dist/frame).
const LOGIC_FPS: f32 = 30.0;

static BOOTSTRAP_ATTEMPTED: AtomicBool = AtomicBool::new(false);

/// Host-facing movement stats resolved from a LocomotorTemplate (dist/sec, rads/sec).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HostMovementStats {
    pub max_speed: f32,
    pub acceleration: f32,
    pub turn_rate: f32,
}

/// Initialize / seed the Common LocomotorStore for host create_object binding.
/// Safe to call repeatedly.
///
/// Returns how many templates were added by this call (seed + filesystem load).
pub fn ensure_host_locomotor_store() -> usize {
    let mut added = 0usize;

    // Prefer real INI data when extracted game data is on disk (once).
    if !BOOTSTRAP_ATTEMPTED.swap(true, Ordering::Relaxed) {
        added += try_load_locomotor_ini_from_disk();
    }

    // Always fill missing golden / Wave 81 common-unit locomotors.
    // (INI load may have BasicHuman but omit some residual names.)
    added += seed_known_host_locomotors();
    added
}

/// Look up the retail SET_NORMAL locomotor template name for a host unit template.
/// Fail-closed: only known infantry/vehicle units; not full Object.ini Locomotor sets.
pub fn locomotor_name_for_unit(template_name: &str) -> Option<&'static str> {
    match template_name {
        // USA infantry (AmericaInfantryRanger → BasicHumanLocomotor)
        "USA_Ranger" | "GoldenRanger" | "AmericaInfantryRanger" => Some(BASIC_HUMAN_LOCOMOTOR),
        // Pathfinder / Colonel Burton share ground locomotor residual (Wave 81).
        "USA_Pathfinder"
        | "AmericaInfantryPathfinder"
        | "AirF_AmericaInfantryPathfinder"
        | "SupW_AmericaInfantryPathfinder"
        | "Lazr_AmericaInfantryPathfinder"
        | "USA_ColonelBurton"
        | "AmericaInfantryColonelBurton" => Some(COLONEL_BURTON_GROUND_LOCOMOTOR),
        // GLA infantry (GLAInfantryRebel → BasicHumanLocomotor)
        "GLA_Soldier" | "GLA_Rebel" | "GLAInfantryRebel" => Some(BASIC_HUMAN_LOCOMOTOR),
        // China infantry (ChinaInfantryRedguard → RedguardLocomotor @ 25)
        "China_RedGuard" | "China_Soldier" | "ChinaInfantryRedguard" => Some(REDGUARD_LOCOMOTOR),
        // USA vehicles
        "USA_Humvee" | "AmericaVehicleHumvee" => Some(HUMVEE_LOCOMOTOR),
        "USA_Crusader" | "USA_CrusaderTank" | "AmericaTankCrusader" => Some(CRUSADER_LOCOMOTOR),
        "USA_Tomahawk" | "AmericaVehicleTomahawk" => Some(TOMAHAWK_LOCOMOTOR),
        "USA_Avenger" | "AmericaVehicleAvenger" | "AmericaTankAvenger" => Some(AVENGER_LOCOMOTOR),
        "USA_Dozer" | "AmericaVehicleDozer" | "AmericaDozer" => Some(AMERICA_DOZER_LOCOMOTOR),
        // GLA vehicles
        "GLA_Technical" | "GLAVehicleTechnical" => Some(TECHNICAL_LOCOMOTOR),
        "GLA_Scorpion" | "GLA_ScorpionTank" | "GLATankScorpion" => Some(SCORPION_LOCOMOTOR),
        "GLA_ScudLauncher" | "GLAVehicleSCUDLauncher" | "GLAVehicleScudLauncher" => {
            Some(SCUD_LAUNCHER_LOCOMOTOR)
        }
        "GLA_QuadCannon" | "GLAVehicleQuadCannon" => Some(QUAD_CANNON_LOCOMOTOR),
        "GLA_Marauder" | "GLATankMarauder" => Some(MARAUDER_LOCOMOTOR),
        "GLA_RocketBuggy" | "GLAVehicleRocketBuggy" => Some(ROCKET_BUGGY_LOCOMOTOR),
        "GLA_BattleBus" | "GLAVehicleBattleBus" => Some(BATTLE_BUS_LOCOMOTOR),
        // China vehicles
        "China_BattleTank" | "ChinaTankBattleMaster" => Some(BATTLE_MASTER_LOCOMOTOR),
        "China_Overlord" | "ChinaTankOverlord" => Some(OVERLORD_LOCOMOTOR),
        "China_Dragon" | "ChinaTankDragon" => Some(DRAGON_LOCOMOTOR),
        "China_Gattling" | "ChinaTankGattling" => Some(GATTLING_TANK_LOCOMOTOR),
        "China_Inferno" | "ChinaVehicleInfernoCannon" => Some(INFERNO_LOCOMOTOR),
        // USA aircraft residual
        "USA_Raptor" | "AmericaJetRaptor" => Some(RAPTOR_JET_LOCOMOTOR),
        "USA_Comanche" | "AmericaVehicleComanche" => Some(COMANCHE_LOCOMOTOR),
        // China aircraft residual
        "China_MIG" | "ChinaJetMIG" | "ChinaJetMiG" => Some(MIG_LOCOMOTOR),
        "China_Helix" | "ChinaVehicleHelix" => Some(HELIX_LOCOMOTOR),
        // Supply trucks (all factions share residual)
        "AmericaVehicleSupplyTruck" | "ChinaVehicleSupplyTruck" | "GLAVehicleSupplyTruck" => {
            Some(SUPPLY_TRUCK_LOCOMOTOR)
        }
        _ => None,
    }
}

/// Wave 81 residual honesty: common-unit locomotor seed residual table.
///
/// Ensures golden + Wave 81 deepen names are present with retail Speed residual.
/// Fail-closed: not full multi-surface / SET_PANIC / pitch-roll matrix.
pub fn honesty_locomotor_residual_table_wave81() -> bool {
    let _ = ensure_host_locomotor_store();
    if HOST_LOCOMOTOR_SEED_RESIDUAL_TABLE.len() < 12 {
        return false;
    }
    // Core + Wave 81 names present in table.
    let names_ok = [
        BASIC_HUMAN_LOCOMOTOR,
        COLONEL_BURTON_GROUND_LOCOMOTOR,
        TOMAHAWK_LOCOMOTOR,
        SCUD_LAUNCHER_LOCOMOTOR,
        QUAD_CANNON_LOCOMOTOR,
        RAPTOR_JET_LOCOMOTOR,
        TECHNICAL_LOCOMOTOR,
        HUMVEE_LOCOMOTOR,
    ]
    .iter()
    .all(|n| HOST_LOCOMOTOR_SEED_RESIDUAL_TABLE.iter().any(|(name, ..)| name == n));
    if !names_ok {
        return false;
    }
    // Seeded / loaded templates resolve retail speeds.
    let speed_ok = |name: &str, expected: f32| {
        movement_from_store(name)
            .map(|m| (m.max_speed - expected).abs() < 0.5)
            .unwrap_or(false)
    };
    names_ok
        && speed_ok(BASIC_HUMAN_LOCOMOTOR, 20.0)
        && speed_ok(COLONEL_BURTON_GROUND_LOCOMOTOR, 30.0)
        && speed_ok(TOMAHAWK_LOCOMOTOR, 30.0)
        && speed_ok(SCUD_LAUNCHER_LOCOMOTOR, 20.0)
        && speed_ok(QUAD_CANNON_LOCOMOTOR, 40.0)
        && speed_ok(RAPTOR_JET_LOCOMOTOR, 175.0)
        && locomotor_name_for_unit("AmericaInfantryPathfinder")
            == Some(COLONEL_BURTON_GROUND_LOCOMOTOR)
        && locomotor_name_for_unit("AmericaVehicleTomahawk") == Some(TOMAHAWK_LOCOMOTOR)
        && locomotor_name_for_unit("GLAVehicleQuadCannon") == Some(QUAD_CANNON_LOCOMOTOR)
        && locomotor_name_for_unit("AmericaJetRaptor") == Some(RAPTOR_JET_LOCOMOTOR)
}

/// Wave 92 residual honesty: expand common-unit locomotor residual names.
///
/// Adds Overlord / Marauder / Dragon / Comanche / MIG / RocketBuggy / BattleBus /
/// SupplyTruck / Avenger / GattlingTank / Inferno / Dozer / Helix residual.
/// Fail-closed: not full multi-surface / SET_PANIC / pitch-roll matrix.
pub fn honesty_locomotor_residual_expand_wave92() -> bool {
    let _ = ensure_host_locomotor_store();
    // Wave 81 base + Wave 92 expand (≥ 25 rows).
    if HOST_LOCOMOTOR_SEED_RESIDUAL_TABLE.len() < 25 {
        return false;
    }
    let wave92_names = [
        OVERLORD_LOCOMOTOR,
        MARAUDER_LOCOMOTOR,
        DRAGON_LOCOMOTOR,
        COMANCHE_LOCOMOTOR,
        MIG_LOCOMOTOR,
        ROCKET_BUGGY_LOCOMOTOR,
        BATTLE_BUS_LOCOMOTOR,
        SUPPLY_TRUCK_LOCOMOTOR,
        AVENGER_LOCOMOTOR,
        GATTLING_TANK_LOCOMOTOR,
        INFERNO_LOCOMOTOR,
        AMERICA_DOZER_LOCOMOTOR,
        HELIX_LOCOMOTOR,
    ];
    let names_ok = wave92_names
        .iter()
        .all(|n| HOST_LOCOMOTOR_SEED_RESIDUAL_TABLE.iter().any(|(name, ..)| name == n));
    if !names_ok {
        return false;
    }
    let speed_ok = |name: &str, expected: f32| {
        movement_from_store(name)
            .map(|m| (m.max_speed - expected).abs() < 0.5)
            .unwrap_or(false)
    };
    names_ok
        && speed_ok(OVERLORD_LOCOMOTOR, 20.0)
        && speed_ok(MARAUDER_LOCOMOTOR, 40.0)
        && speed_ok(DRAGON_LOCOMOTOR, 30.0)
        && speed_ok(COMANCHE_LOCOMOTOR, 120.0)
        && speed_ok(MIG_LOCOMOTOR, 160.0)
        && speed_ok(ROCKET_BUGGY_LOCOMOTOR, 90.0)
        && speed_ok(BATTLE_BUS_LOCOMOTOR, 70.0)
        && speed_ok(SUPPLY_TRUCK_LOCOMOTOR, 40.0)
        && speed_ok(AVENGER_LOCOMOTOR, 30.0)
        && speed_ok(GATTLING_TANK_LOCOMOTOR, 40.0)
        && speed_ok(INFERNO_LOCOMOTOR, 30.0)
        && speed_ok(AMERICA_DOZER_LOCOMOTOR, 30.0)
        && speed_ok(HELIX_LOCOMOTOR, 75.0)
        && locomotor_name_for_unit("ChinaTankOverlord") == Some(OVERLORD_LOCOMOTOR)
        && locomotor_name_for_unit("GLATankMarauder") == Some(MARAUDER_LOCOMOTOR)
        && locomotor_name_for_unit("ChinaTankDragon") == Some(DRAGON_LOCOMOTOR)
        && locomotor_name_for_unit("AmericaVehicleComanche") == Some(COMANCHE_LOCOMOTOR)
        && locomotor_name_for_unit("ChinaJetMIG") == Some(MIG_LOCOMOTOR)
        && locomotor_name_for_unit("GLAVehicleRocketBuggy") == Some(ROCKET_BUGGY_LOCOMOTOR)
        && locomotor_name_for_unit("AmericaTankAvenger") == Some(AVENGER_LOCOMOTOR)
        && locomotor_name_for_unit("ChinaVehicleHelix") == Some(HELIX_LOCOMOTOR)
        && honesty_locomotor_residual_table_wave81()
}

/// Resolve host Movement stats from the Locomotor catalog by template name.
/// Ensures the store is bootstrapped first. Returns None if missing or unusable.
pub fn resolve_host_movement(locomotor_name: &str) -> Option<HostMovementStats> {
    let _ = ensure_host_locomotor_store();
    movement_from_store(locomotor_name)
}

/// Convert a Common LocomotorTemplate (per-frame) into host Movement units (per-sec).
///
/// C++ Locomotor.ini: Speed is dist/sec, stored as Speed/30 dist/frame.
/// Host `Movement.max_speed` is dist/sec (used with dt seconds).
pub fn movement_from_store(name: &str) -> Option<HostMovementStats> {
    let store = get_locomotor_store();
    let t = store.find_template(name)?;
    if t.max_speed <= 0.0 {
        return None;
    }
    Some(host_stats_from_template(t))
}

fn host_stats_from_template(t: &LocomotorTemplate) -> HostMovementStats {
    // Store: dist/frame, dist/frame², rads/frame → host: dist/sec, dist/sec², rads/sec.
    let max_speed = t.max_speed * LOGIC_FPS;
    let acceleration = if t.acceleration > 0.0 {
        t.acceleration * LOGIC_FPS * LOGIC_FPS
    } else {
        // Fallback: snappy enough not to crawl; retail infantry uses 100.
        100.0
    };
    let turn_rate = if t.max_turn_rate > 0.0 {
        t.max_turn_rate * LOGIC_FPS
    } else {
        std::f32::consts::PI
    };
    HostMovementStats {
        max_speed,
        acceleration,
        turn_rate,
    }
}

fn store_has(name: &str) -> bool {
    get_locomotor_store().find_template(name).is_some()
}

fn try_load_locomotor_ini_from_disk() -> usize {
    let mut total = 0usize;
    for path in locomotor_ini_candidate_paths() {
        if !path.is_file() {
            continue;
        }
        match std::fs::read_to_string(&path) {
            Ok(content) => match load_locomotors_from_str(&content) {
                Ok(n) if n > 0 => {
                    log::info!(
                        "Host LocomotorStore: loaded {} locomotor templates from {}",
                        n,
                        path.display()
                    );
                    total += n;
                    // One full Locomotor.ini is enough; further copies are overrides.
                    break;
                }
                Ok(_) => {}
                Err(e) => {
                    log::debug!(
                        "Host LocomotorStore: parse failed for {}: {e}",
                        path.display()
                    );
                }
            },
            Err(e) => {
                log::debug!("Host LocomotorStore: cannot read {}: {e}", path.display());
            }
        }
    }
    total
}

fn locomotor_ini_candidate_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    let relative = [
        "windows_game/extracted_big_files/INIZH/Data/INI/Locomotor.ini",
        "windows_game/extracted_big_files_v2/INIZH/Data/INI/Locomotor.ini",
        "Data/INI/Locomotor.ini",
        "Data/INI/Default/Locomotor.ini",
        "INIZH/Data/INI/Locomotor.ini",
    ];
    for r in relative {
        paths.push(PathBuf::from(r));
    }

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

    let mut seen = std::collections::HashSet::new();
    paths.retain(|p| seen.insert(p.clone()));
    paths
}

/// Seed minimal retail-ish stats for host golden units when INI is unavailable.
///
/// Values match retail Locomotor.ini Speed / Acceleration / TurnRate (dist/sec).
/// Wave 81: uses [`HOST_LOCOMOTOR_SEED_RESIDUAL_TABLE`] (golden + common deepen).
fn seed_known_host_locomotors() -> usize {
    let mut added = 0usize;
    for &(name, speed, accel, turn_deg) in HOST_LOCOMOTOR_SEED_RESIDUAL_TABLE {
        if store_has(name) {
            continue;
        }
        let mut props = HashMap::new();
        props.insert("Speed".to_string(), format!("{}", speed));
        props.insert("Acceleration".to_string(), format!("{}", accel));
        props.insert("TurnRate".to_string(), format!("{}", turn_deg));
        // Air residual locomotors; others GROUND (host seed surfaces residual only).
        let surfaces = if name == RAPTOR_JET_LOCOMOTOR
            || name == COMANCHE_LOCOMOTOR
            || name == MIG_LOCOMOTOR
            || name == HELIX_LOCOMOTOR
        {
            "AIR"
        } else {
            "GROUND"
        };
        props.insert("Surfaces".to_string(), surfaces.to_string());
        match parse_locomotor_template_definition(name, &props) {
            Ok(template) => match get_locomotor_store_mut().add_template(template) {
                Ok(()) => {
                    log::debug!("Host LocomotorStore: seeded locomotor {}", name);
                    added += 1;
                }
                Err(e) => {
                    log::warn!("Host LocomotorStore: failed to add {}: {e}", name);
                }
            },
            Err(e) => {
                log::warn!("Host LocomotorStore: failed to seed {}: {e}", name);
            }
        }
    }
    if added > 0 {
        log::info!(
            "Host LocomotorStore: seeded {} known golden-unit locomotors (INI data unavailable or incomplete)",
            added
        );
    }
    added
}

/// Test helper: force re-bootstrap attempt (does not clear existing templates).
#[cfg(test)]
pub fn reset_bootstrap_attempt_flag_for_tests() {
    BOOTSTRAP_ATTEMPTED.store(false, Ordering::Relaxed);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    use glam::Vec3;

    #[test]
    fn bootstrap_seeds_basic_human_at_retail_speed() {
        ensure_host_locomotor_store();
        assert!(store_has(BASIC_HUMAN_LOCOMOTOR));
        let m = movement_from_store(BASIC_HUMAN_LOCOMOTOR).expect("movement");
        assert!(
            (m.max_speed - 20.0).abs() < 0.05,
            "retail BasicHumanLocomotor Speed is 20 u/s, got {}",
            m.max_speed
        );
        assert!(
            (m.acceleration - 100.0).abs() < 0.5,
            "retail Acceleration is 100, got {}",
            m.acceleration
        );
    }

    #[test]
    fn create_object_usa_ranger_binds_retail_infantry_speed() {
        ensure_host_locomotor_store();

        let mut logic = crate::game_logic::GameLogic::new();
        let mut ranger = ThingTemplate::new("USA_Ranger");
        ranger
            .add_kind_of(KindOf::Infantry)
            .add_kind_of(KindOf::Attackable)
            .add_kind_of(KindOf::Selectable)
            .set_health(120.0)
            .set_locomotor_name(BASIC_HUMAN_LOCOMOTOR);
        logic.templates.insert("USA_Ranger".to_string(), ranger);

        let id = logic
            .create_object("USA_Ranger", Team::USA, Vec3::new(0.0, 0.0, 0.0))
            .expect("create USA_Ranger");
        let obj = logic.objects.get(&id).expect("object");
        // Catalog path must replace Movement::default() (10) with retail ~20.
        assert!(
            (obj.movement.max_speed - 10.0).abs() > 0.01,
            "expected catalog speed, still host default 10: {}",
            obj.movement.max_speed
        );
        assert!(
            (obj.movement.max_speed - 20.0).abs() < 0.05,
            "expected BasicHumanLocomotor 20 u/s, got {}",
            obj.movement.max_speed
        );
        assert!(
            (obj.movement.acceleration - 100.0).abs() < 0.5,
            "expected retail accel 100, got {}",
            obj.movement.acceleration
        );
    }

    #[test]
    fn infantry_template_resolves_movement_from_locomotor_path() {
        ensure_host_locomotor_store();
        let mut t = ThingTemplate::new("USA_Ranger");
        t.add_kind_of(KindOf::Infantry)
            .set_locomotor_name(BASIC_HUMAN_LOCOMOTOR);
        let m = t.resolve_movement().expect("locomotor path");
        assert!((m.max_speed - 20.0).abs() < 0.05);
    }

    #[test]
    fn locomotor_name_for_known_units() {
        assert_eq!(
            locomotor_name_for_unit("USA_Ranger"),
            Some(BASIC_HUMAN_LOCOMOTOR)
        );
        assert_eq!(
            locomotor_name_for_unit("USA_Humvee"),
            Some(HUMVEE_LOCOMOTOR)
        );
        assert_eq!(locomotor_name_for_unit("USA_Dozer"), None);
        assert_eq!(
            locomotor_name_for_unit("China_Soldier"),
            Some(REDGUARD_LOCOMOTOR)
        );
        assert_eq!(
            locomotor_name_for_unit("AmericaInfantryPathfinder"),
            Some(COLONEL_BURTON_GROUND_LOCOMOTOR)
        );
        assert_eq!(
            locomotor_name_for_unit("AmericaVehicleTomahawk"),
            Some(TOMAHAWK_LOCOMOTOR)
        );
    }

    #[test]
    fn locomotor_residual_table_wave81_honesty() {
        assert!(honesty_locomotor_residual_table_wave81());
        assert!(HOST_LOCOMOTOR_SEED_RESIDUAL_TABLE.len() >= 12);
        assert!(store_has(COLONEL_BURTON_GROUND_LOCOMOTOR));
        assert!(store_has(RAPTOR_JET_LOCOMOTOR));
    }

    #[test]
    fn locomotor_residual_expand_wave92_honesty() {
        assert!(honesty_locomotor_residual_expand_wave92());
        assert!(HOST_LOCOMOTOR_SEED_RESIDUAL_TABLE.len() >= 25);
        assert!(store_has(OVERLORD_LOCOMOTOR));
        assert!(store_has(COMANCHE_LOCOMOTOR));
        assert!(store_has(MIG_LOCOMOTOR));
        assert!(store_has(HELIX_LOCOMOTOR));
        assert_eq!(
            locomotor_name_for_unit("ChinaTankOverlord"),
            Some(OVERLORD_LOCOMOTOR)
        );
        assert_eq!(
            locomotor_name_for_unit("AmericaVehicleComanche"),
            Some(COMANCHE_LOCOMOTOR)
        );
    }

    #[test]
    fn create_object_humvee_binds_vehicle_speed_when_catalog_present() {
        ensure_host_locomotor_store();
        let mut logic = crate::game_logic::GameLogic::new();
        let mut humvee = ThingTemplate::new("USA_Humvee");
        humvee
            .add_kind_of(KindOf::Vehicle)
            .add_kind_of(KindOf::Attackable)
            .set_locomotor_name(HUMVEE_LOCOMOTOR);
        logic.templates.insert("USA_Humvee".to_string(), humvee);

        let id = logic
            .create_object("USA_Humvee", Team::USA, Vec3::ZERO)
            .expect("create");
        let obj = logic.objects.get(&id).expect("object");
        assert!(
            (obj.movement.max_speed - 60.0).abs() < 0.1,
            "HumveeLocomotor Speed is 60, got {}",
            obj.movement.max_speed
        );
    }
}
