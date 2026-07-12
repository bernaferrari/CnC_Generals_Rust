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
    // Fast path: already have BasicHumanLocomotor from archive load or prior seed.
    if store_has(BASIC_HUMAN_LOCOMOTOR) {
        BOOTSTRAP_ATTEMPTED.store(true, Ordering::Relaxed);
        return 0;
    }

    let mut added = 0usize;

    // Prefer real INI data when extracted game data is on disk.
    if !BOOTSTRAP_ATTEMPTED.swap(true, Ordering::Relaxed) || !store_has(BASIC_HUMAN_LOCOMOTOR) {
        added += try_load_locomotor_ini_from_disk();
    }

    // Always guarantee golden-unit locomotors even without game data.
    added += seed_known_host_locomotors();
    added
}

/// Look up the retail SET_NORMAL locomotor template name for a host unit template.
/// Fail-closed: only known infantry/vehicle units; not full Object.ini Locomotor sets.
pub fn locomotor_name_for_unit(template_name: &str) -> Option<&'static str> {
    match template_name {
        // USA infantry (AmericaInfantryRanger → BasicHumanLocomotor)
        "USA_Ranger" | "GoldenRanger" | "AmericaInfantryRanger" => Some(BASIC_HUMAN_LOCOMOTOR),
        // GLA infantry (GLAInfantryRebel → BasicHumanLocomotor)
        "GLA_Soldier" | "GLA_Rebel" | "GLAInfantryRebel" => Some(BASIC_HUMAN_LOCOMOTOR),
        // China infantry (ChinaInfantryRedguard → RedguardLocomotor @ 25)
        "China_RedGuard" | "China_Soldier" | "ChinaInfantryRedguard" => Some(REDGUARD_LOCOMOTOR),
        // USA vehicles
        "USA_Humvee" | "AmericaVehicleHumvee" => Some(HUMVEE_LOCOMOTOR),
        "USA_Crusader" | "USA_CrusaderTank" | "AmericaTankCrusader" => Some(CRUSADER_LOCOMOTOR),
        // GLA vehicles
        "GLA_Technical" | "GLAVehicleTechnical" => Some(TECHNICAL_LOCOMOTOR),
        "GLA_Scorpion" | "GLA_ScorpionTank" | "GLATankScorpion" => Some(SCORPION_LOCOMOTOR),
        // China vehicles
        "China_BattleTank" | "ChinaTankBattleMaster" => Some(BATTLE_MASTER_LOCOMOTOR),
        _ => None,
    }
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
fn seed_known_host_locomotors() -> usize {
    // Retail Locomotor.ini values (dist/sec, dist/sec², degrees/sec).
    let seeds: &[(&str, f32, f32, f32)] = &[
        // BasicHumanLocomotor — AmericaInfantryRanger / GLA Rebel
        (BASIC_HUMAN_LOCOMOTOR, 20.0, 100.0, 500.0),
        // RedguardLocomotor — China Red Guard (~25% faster than basic human)
        (REDGUARD_LOCOMOTOR, 25.0, 100.0, 500.0),
        // HumveeLocomotor
        (HUMVEE_LOCOMOTOR, 60.0, 1000.0, 180.0),
        // CrusaderLocomotor
        (CRUSADER_LOCOMOTOR, 30.0, 1000.0, 180.0),
        // ScorpionLocomotor
        (SCORPION_LOCOMOTOR, 40.0, 1000.0, 180.0),
        // BattleMasterLocomotor
        (BATTLE_MASTER_LOCOMOTOR, 25.0, 1000.0, 180.0),
        // TechnicalLocomotor
        (TECHNICAL_LOCOMOTOR, 90.0, 100.0, 180.0),
    ];

    let mut added = 0usize;
    for &(name, speed, accel, turn_deg) in seeds {
        if store_has(name) {
            continue;
        }
        let mut props = HashMap::new();
        props.insert("Speed".to_string(), format!("{}", speed));
        props.insert("Acceleration".to_string(), format!("{}", accel));
        props.insert("TurnRate".to_string(), format!("{}", turn_deg));
        props.insert("Surfaces".to_string(), "GROUND".to_string());
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
