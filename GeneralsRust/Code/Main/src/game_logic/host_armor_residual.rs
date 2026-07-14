//! Host armor residual table honesty (ProjectileArmor / HazardousMaterialArmor).
//!
//! Wave 81 residual peel: Armor.ini coefficient residual for projectile shells
//! and cleanup-hazard fields. Host-testable without full Armor.ini archive load.
//!
//! Fail-closed:
//! - Not full Armor.ini multi-template matrix / ArmorSet upgrade graph
//! - Not live ActiveBody armor-set swap / DamageFX interleave
//! - Not network armor residual replication (network deferred)

use gamelogic::common::AsciiString;
use gamelogic::damage::DamageType;
use gamelogic::object::armor::{ArmorTemplate, TheArmorStore};

/// Retail ProjectileArmor residual name (missiles / shells / Spectre howitzer).
pub const PROJECTILE_ARMOR: &str = "ProjectileArmor";
/// Retail HazardousMaterialArmor residual name (poison / radiation fields).
pub const HAZARDOUS_MATERIAL_ARMOR: &str = "HazardousMaterialArmor";

// --- ProjectileArmor residual coefficients (Armor.ini) ---
// DEFAULT 25%; FALLING 0%; LASER 100%; SMALL_ARMS 25%; MICROWAVE 0%;
// GATTLING 25%; HAZARD_CLEANUP 0%; KILL_PILOT 0%; SURRENDER 0%;
// SUBDUAL_MISSILE 100%; SUBDUAL_VEHICLE 0%; SUBDUAL_BUILDING 0%;
// POISON 0%; RADIATION 0%; FLAME 0%.

/// ProjectileArmor DEFAULT residual coefficient.
pub const PROJECTILE_ARMOR_DEFAULT: f32 = 0.25;
/// ProjectileArmor LASER residual (point-defense effective).
pub const PROJECTILE_ARMOR_LASER: f32 = 1.0;
/// ProjectileArmor SMALL_ARMS residual.
pub const PROJECTILE_ARMOR_SMALL_ARMS: f32 = 0.25;
/// ProjectileArmor GATTLING residual.
pub const PROJECTILE_ARMOR_GATTLING: f32 = 0.25;
/// ProjectileArmor FALLING residual (immune).
pub const PROJECTILE_ARMOR_FALLING: f32 = 0.0;
/// ProjectileArmor MICROWAVE residual (immune).
pub const PROJECTILE_ARMOR_MICROWAVE: f32 = 0.0;
/// ProjectileArmor HAZARD_CLEANUP residual (immune).
pub const PROJECTILE_ARMOR_HAZARD_CLEANUP: f32 = 0.0;
/// ProjectileArmor POISON residual (immune).
pub const PROJECTILE_ARMOR_POISON: f32 = 0.0;
/// ProjectileArmor RADIATION residual (immune).
pub const PROJECTILE_ARMOR_RADIATION: f32 = 0.0;
/// ProjectileArmor FLAME residual (immune).
pub const PROJECTILE_ARMOR_FLAME: f32 = 0.0;
/// ProjectileArmor SUBDUAL_MISSILE residual.
pub const PROJECTILE_ARMOR_SUBDUAL_MISSILE: f32 = 1.0;

// --- HazardousMaterialArmor residual coefficients (Armor.ini) ---
// DEFAULT 0%; HAZARD_CLEANUP 100%; FLAME 0%.

/// HazardousMaterialArmor DEFAULT residual (only cleanup harms).
pub const HAZARDOUS_MATERIAL_ARMOR_DEFAULT: f32 = 0.0;
/// HazardousMaterialArmor HAZARD_CLEANUP residual (full cleanup damage).
pub const HAZARDOUS_MATERIAL_ARMOR_CLEANUP: f32 = 1.0;
/// HazardousMaterialArmor FLAME residual (flame cannot clean).
pub const HAZARDOUS_MATERIAL_ARMOR_FLAME: f32 = 0.0;

/// Build retail ProjectileArmor residual template from coefficient table.
pub fn build_projectile_armor_residual() -> ArmorTemplate {
    let mut t = ArmorTemplate::new();
    t.set_default(PROJECTILE_ARMOR_DEFAULT);
    t.set_coefficient(DamageType::Falling, PROJECTILE_ARMOR_FALLING);
    t.set_coefficient(DamageType::Laser, PROJECTILE_ARMOR_LASER);
    t.set_coefficient(DamageType::SmallArms, PROJECTILE_ARMOR_SMALL_ARMS);
    t.set_coefficient(DamageType::Microwave, PROJECTILE_ARMOR_MICROWAVE);
    t.set_coefficient(DamageType::Gattling, PROJECTILE_ARMOR_GATTLING);
    t.set_coefficient(DamageType::HazardCleanup, PROJECTILE_ARMOR_HAZARD_CLEANUP);
    t.set_coefficient(DamageType::KillPilot, 0.0);
    t.set_coefficient(DamageType::Surrender, 0.0);
    t.set_coefficient(DamageType::SubdualMissile, PROJECTILE_ARMOR_SUBDUAL_MISSILE);
    t.set_coefficient(DamageType::SubdualVehicle, 0.0);
    t.set_coefficient(DamageType::SubdualBuilding, 0.0);
    t.set_coefficient(DamageType::Poison, PROJECTILE_ARMOR_POISON);
    t.set_coefficient(DamageType::Radiation, PROJECTILE_ARMOR_RADIATION);
    t.set_coefficient(DamageType::Flame, PROJECTILE_ARMOR_FLAME);
    t
}

/// Build retail HazardousMaterialArmor residual template from coefficient table.
pub fn build_hazardous_material_armor_residual() -> ArmorTemplate {
    let mut t = ArmorTemplate::new();
    t.set_default(HAZARDOUS_MATERIAL_ARMOR_DEFAULT);
    t.set_coefficient(DamageType::HazardCleanup, HAZARDOUS_MATERIAL_ARMOR_CLEANUP);
    t.set_coefficient(DamageType::Flame, HAZARDOUS_MATERIAL_ARMOR_FLAME);
    t
}

/// Ensure residual armor templates are registered when store lacks them.
///
/// Prefer full Armor.ini load when available; only seed missing names.
/// Returns how many templates were registered by this call.
pub fn ensure_host_armor_residual_seed() -> usize {
    gamelogic::object::armor::ensure_default_templates_loaded();
    let mut added = 0usize;
    let projectile_name = AsciiString::from(PROJECTILE_ARMOR);
    let hazard_name = AsciiString::from(HAZARDOUS_MATERIAL_ARMOR);
    if TheArmorStore::find_template(&projectile_name).is_none() {
        TheArmorStore::register_template(&projectile_name, build_projectile_armor_residual());
        added += 1;
    }
    if TheArmorStore::find_template(&hazard_name).is_none() {
        TheArmorStore::register_template(&hazard_name, build_hazardous_material_armor_residual());
        added += 1;
    }
    added
}

/// Wave 81 residual honesty: ProjectileArmor / HazardousMaterialArmor coefficient tables.
///
/// Verifies retail Armor.ini residual scalars via adjust_damage on built templates
/// and store registration. Fail-closed: not full Armor.ini / ArmorSet upgrade matrix.
pub fn honesty_armor_residual_table_wave81() -> bool {
    let _ = ensure_host_armor_residual_seed();

    let names_ok = PROJECTILE_ARMOR == "ProjectileArmor"
        && HAZARDOUS_MATERIAL_ARMOR == "HazardousMaterialArmor"
        && (PROJECTILE_ARMOR_DEFAULT - 0.25).abs() < 0.001
        && (PROJECTILE_ARMOR_LASER - 1.0).abs() < 0.001
        && (PROJECTILE_ARMOR_SMALL_ARMS - 0.25).abs() < 0.001
        && (PROJECTILE_ARMOR_GATTLING - 0.25).abs() < 0.001
        && PROJECTILE_ARMOR_FALLING == 0.0
        && PROJECTILE_ARMOR_MICROWAVE == 0.0
        && PROJECTILE_ARMOR_HAZARD_CLEANUP == 0.0
        && PROJECTILE_ARMOR_POISON == 0.0
        && PROJECTILE_ARMOR_RADIATION == 0.0
        && PROJECTILE_ARMOR_FLAME == 0.0
        && (PROJECTILE_ARMOR_SUBDUAL_MISSILE - 1.0).abs() < 0.001
        && HAZARDOUS_MATERIAL_ARMOR_DEFAULT == 0.0
        && (HAZARDOUS_MATERIAL_ARMOR_CLEANUP - 1.0).abs() < 0.001
        && HAZARDOUS_MATERIAL_ARMOR_FLAME == 0.0;

    if !names_ok {
        return false;
    }

    // Built residual templates: adjust_damage residual matrix.
    let proj = build_projectile_armor_residual();
    let proj_ok = approx_eq(proj.adjust_damage(DamageType::Explosion, 100.0), 25.0)
        && approx_eq(proj.adjust_damage(DamageType::Laser, 100.0), 100.0)
        && approx_eq(proj.adjust_damage(DamageType::SmallArms, 100.0), 25.0)
        && approx_eq(proj.adjust_damage(DamageType::Gattling, 100.0), 25.0)
        && approx_eq(proj.adjust_damage(DamageType::Falling, 100.0), 0.0)
        && approx_eq(proj.adjust_damage(DamageType::Microwave, 100.0), 0.0)
        && approx_eq(proj.adjust_damage(DamageType::HazardCleanup, 100.0), 0.0)
        && approx_eq(proj.adjust_damage(DamageType::Poison, 100.0), 0.0)
        && approx_eq(proj.adjust_damage(DamageType::Radiation, 100.0), 0.0)
        && approx_eq(proj.adjust_damage(DamageType::Flame, 100.0), 0.0)
        && approx_eq(proj.adjust_damage(DamageType::SubdualMissile, 100.0), 100.0)
        // Unresistable bypasses armor residual.
        && approx_eq(proj.adjust_damage(DamageType::Unresistable, 100.0), 100.0);

    let haz = build_hazardous_material_armor_residual();
    let haz_ok = approx_eq(haz.adjust_damage(DamageType::Explosion, 100.0), 0.0)
        && approx_eq(haz.adjust_damage(DamageType::SmallArms, 100.0), 0.0)
        && approx_eq(haz.adjust_damage(DamageType::HazardCleanup, 100.0), 100.0)
        && approx_eq(haz.adjust_damage(DamageType::Flame, 100.0), 0.0)
        && approx_eq(haz.adjust_damage(DamageType::Unresistable, 50.0), 50.0);

    // Store residual: both templates registered (INI or seed).
    let projectile_name = AsciiString::from(PROJECTILE_ARMOR);
    let hazard_name = AsciiString::from(HAZARDOUS_MATERIAL_ARMOR);
    let store_ok = TheArmorStore::find_template(&projectile_name).is_some()
        && TheArmorStore::find_template(&hazard_name).is_some();

    // If store loaded full Armor.ini, verify store templates match residual key scalars.
    let store_coeff_ok = match (
        TheArmorStore::find_template(&projectile_name),
        TheArmorStore::find_template(&hazard_name),
    ) {
        (Some(p), Some(h)) => {
            approx_eq(p.adjust_damage(DamageType::Explosion, 100.0), 25.0)
                && approx_eq(p.adjust_damage(DamageType::Laser, 100.0), 100.0)
                && approx_eq(p.adjust_damage(DamageType::Falling, 100.0), 0.0)
                && approx_eq(h.adjust_damage(DamageType::HazardCleanup, 100.0), 100.0)
                && approx_eq(h.adjust_damage(DamageType::Explosion, 100.0), 0.0)
                && approx_eq(h.adjust_damage(DamageType::Flame, 100.0), 0.0)
        }
        _ => false,
    };

    proj_ok && haz_ok && store_ok && store_coeff_ok
}

#[inline]
fn approx_eq(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.05
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn armor_residual_table_wave81_honesty() {
        assert!(honesty_armor_residual_table_wave81());
        let p = build_projectile_armor_residual();
        assert!((p.adjust_damage(DamageType::Laser, 40.0) - 40.0).abs() < 0.01);
        let h = build_hazardous_material_armor_residual();
        assert!((h.adjust_damage(DamageType::HazardCleanup, 40.0) - 40.0).abs() < 0.01);
        assert!((h.adjust_damage(DamageType::Explosion, 40.0)).abs() < 0.01);
    }

    #[test]
    fn armor_residual_names_match_retail() {
        assert_eq!(PROJECTILE_ARMOR, "ProjectileArmor");
        assert_eq!(HAZARDOUS_MATERIAL_ARMOR, "HazardousMaterialArmor");
    }
}
