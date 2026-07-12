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

    // Fast path: already have the ranger weapon from archive load or prior seed.
    if store_has(RANGER_PRIMARY_WEAPON) {
        BOOTSTRAP_ATTEMPTED.store(true, Ordering::Relaxed);
        return 0;
    }

    let mut added = 0usize;

    // Prefer real INI data when extracted game data is on disk.
    if !BOOTSTRAP_ATTEMPTED.swap(true, Ordering::Relaxed) || !store_has(RANGER_PRIMARY_WEAPON) {
        added += try_load_weapon_ini_from_disk();
    }

    // Always guarantee golden-unit weapons even without game data.
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
        _ => None,
    }
}

/// Look up the retail secondary weapon template name for a host unit template.
/// Fail-closed: only known multi-slot units; not full WeaponSet upgrade matrices.
pub fn secondary_weapon_name_for_unit(template_name: &str) -> Option<&'static str> {
    match template_name {
        "USA_Ranger" | "GoldenRanger" | "AmericaInfantryRanger" => Some(RANGER_SECONDARY_WEAPON),
        "USA_Humvee" | "AmericaVehicleHumvee" => Some(HUMVEE_SECONDARY_WEAPON),
        _ => None,
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

    /// Residual: infantry (non-structure) still uses primary when both ready.
    #[test]
    fn update_combat_uses_primary_vs_infantry_when_both_ready() {
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

        let primary_dmg = logic
            .objects
            .get(&attacker_id)
            .and_then(|a| a.weapon.as_ref())
            .map(|w| w.damage)
            .unwrap_or(0.0);

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
            (dealt - primary_dmg).abs() < 1.0 || (dealt > 0.0 && dealt <= primary_dmg + 0.5),
            "infantry shot must use primary path: dealt={dealt} primary={primary_dmg}"
        );

        let atk = logic.objects.get(&attacker_id).expect("attacker");
        let pri_last = atk.weapon.as_ref().map(|w| w.last_fire_time).unwrap_or(0.0);
        let sec_last = atk
            .secondary_weapon
            .as_ref()
            .map(|w| w.last_fire_time)
            .unwrap_or(0.0);
        assert!(pri_last > 0.0, "primary must fire vs infantry");
        assert!(
            (sec_last - 0.0).abs() < f32::EPSILON,
            "secondary must stay idle vs infantry when primary ready"
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
