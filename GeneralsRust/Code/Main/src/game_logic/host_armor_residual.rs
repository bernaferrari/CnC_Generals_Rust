//! Host armor residual table honesty (ProjectileArmor / HazardousMaterialArmor
//! + Wave 92 common unit armors + Wave 103 specialized unit armors).
//!
//! Wave 81 residual peel: Armor.ini coefficient residual for projectile shells
//! and cleanup-hazard fields. Host-testable without full Armor.ini archive load.
//!
//! Wave 92 residual expand: HumanArmor / TankArmor / StructureArmor /
//! AirplaneArmor / TruckArmor key coefficient residual (common unit classes).
//!
//! Wave 103 residual expand: HazMatHumanArmor / ChemSuitHumanArmor / DozerArmor /
//! UpgradedTankArmor / HumveeArmor / DragonTankArmor / ToxinTruckArmor /
//! ComancheArmor / StructureArmorTough key coefficient residual.
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
/// Retail HumanArmor residual name (infantry).
pub const HUMAN_ARMOR: &str = "HumanArmor";
/// Retail TankArmor residual name (MBTs).
pub const TANK_ARMOR: &str = "TankArmor";
/// Retail StructureArmor residual name (buildings).
pub const STRUCTURE_ARMOR: &str = "StructureArmor";
/// Retail AirplaneArmor residual name (jets).
pub const AIRPLANE_ARMOR: &str = "AirplaneArmor";
/// Retail TruckArmor residual name (soft vehicles).
pub const TRUCK_ARMOR: &str = "TruckArmor";

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

// --- Wave 92 common unit armor residual coefficients (Armor.ini key scalars) ---
// HumanArmor: CRUSH 200%, ARMOR_PIERCING 10%, FLAME 150%, SNIPER 200%, LASER 50%,
//             HAZARD_CLEANUP 0%, KILL_PILOT 0%, SURRENDER 100%, SUBDUAL_* 0%.
// TankArmor: CRUSH 50%, SMALL_ARMS 25%, GATTLING 10%, FLAME 25%, RADIATION 50%,
//            POISON 25%, SNIPER 0%, MELEE 0%, LASER 0%, KILL_PILOT 100%,
//            SURRENDER 0%, SUBDUAL_VEHICLE 100%.
// StructureArmor: SMALL_ARMS 50%, GATTLING 10%, RADIATION 0%, SNIPER 0%, POISON 1%,
//                 LASER 0%, PARTICLE_BEAM 200%, AURORA_BOMB 250%, FLAME 50%,
//                 SUBDUAL_BUILDING 100%, LAND_MINE 0%.
// AirplaneArmor: SMALL_ARMS 120%, GATTLING 120%, JET_MISSILES 25%, POISON 25%,
//                RADIATION 25%, SNIPER 0%, LASER 0%, KILL_PILOT 0%.
// TruckArmor: CRUSH 50%, SMALL_ARMS 50%, GATTLING 50%, SNIPER 0%, MELEE 0%,
//             LASER 0%, KILL_PILOT 100%, SUBDUAL_VEHICLE 100%.

/// HumanArmor CRUSH residual.
pub const HUMAN_ARMOR_CRUSH: f32 = 2.0;
/// HumanArmor ARMOR_PIERCING residual.
pub const HUMAN_ARMOR_ARMOR_PIERCING: f32 = 0.10;
/// HumanArmor FLAME residual.
pub const HUMAN_ARMOR_FLAME: f32 = 1.50;
/// HumanArmor SNIPER residual.
pub const HUMAN_ARMOR_SNIPER: f32 = 2.0;
/// HumanArmor LASER residual.
pub const HUMAN_ARMOR_LASER: f32 = 0.50;

/// TankArmor SMALL_ARMS residual.
pub const TANK_ARMOR_SMALL_ARMS: f32 = 0.25;
/// TankArmor GATTLING residual.
pub const TANK_ARMOR_GATTLING: f32 = 0.10;
/// TankArmor FLAME residual.
pub const TANK_ARMOR_FLAME: f32 = 0.25;
/// TankArmor SNIPER residual (immune).
pub const TANK_ARMOR_SNIPER: f32 = 0.0;
/// TankArmor LASER residual (immune).
pub const TANK_ARMOR_LASER: f32 = 0.0;
/// TankArmor KILL_PILOT residual.
pub const TANK_ARMOR_KILL_PILOT: f32 = 1.0;
/// TankArmor SUBDUAL_VEHICLE residual.
pub const TANK_ARMOR_SUBDUAL_VEHICLE: f32 = 1.0;

/// StructureArmor SMALL_ARMS residual.
pub const STRUCTURE_ARMOR_SMALL_ARMS: f32 = 0.50;
/// StructureArmor GATTLING residual.
pub const STRUCTURE_ARMOR_GATTLING: f32 = 0.10;
/// StructureArmor RADIATION residual (immune).
pub const STRUCTURE_ARMOR_RADIATION: f32 = 0.0;
/// StructureArmor SNIPER residual (immune).
pub const STRUCTURE_ARMOR_SNIPER: f32 = 0.0;
/// StructureArmor PARTICLE_BEAM residual.
pub const STRUCTURE_ARMOR_PARTICLE_BEAM: f32 = 2.0;
/// StructureArmor AURORA_BOMB residual.
pub const STRUCTURE_ARMOR_AURORA_BOMB: f32 = 2.50;
/// StructureArmor FLAME residual.
pub const STRUCTURE_ARMOR_FLAME: f32 = 0.50;
/// StructureArmor SUBDUAL_BUILDING residual.
pub const STRUCTURE_ARMOR_SUBDUAL_BUILDING: f32 = 1.0;

/// AirplaneArmor SMALL_ARMS residual.
pub const AIRPLANE_ARMOR_SMALL_ARMS: f32 = 1.20;
/// AirplaneArmor GATTLING residual.
pub const AIRPLANE_ARMOR_GATTLING: f32 = 1.20;
/// AirplaneArmor JET_MISSILES residual.
pub const AIRPLANE_ARMOR_JET_MISSILES: f32 = 0.25;
/// AirplaneArmor SNIPER residual (immune).
pub const AIRPLANE_ARMOR_SNIPER: f32 = 0.0;
/// AirplaneArmor LASER residual (immune).
pub const AIRPLANE_ARMOR_LASER: f32 = 0.0;

/// TruckArmor SMALL_ARMS residual.
pub const TRUCK_ARMOR_SMALL_ARMS: f32 = 0.50;
/// TruckArmor GATTLING residual.
pub const TRUCK_ARMOR_GATTLING: f32 = 0.50;
/// TruckArmor SNIPER residual (immune).
pub const TRUCK_ARMOR_SNIPER: f32 = 0.0;
/// TruckArmor KILL_PILOT residual.
pub const TRUCK_ARMOR_KILL_PILOT: f32 = 1.0;
/// TruckArmor SUBDUAL_VEHICLE residual.
pub const TRUCK_ARMOR_SUBDUAL_VEHICLE: f32 = 1.0;

/// Build retail HumanArmor residual template from key coefficient residual.
pub fn build_human_armor_residual() -> ArmorTemplate {
    let mut t = ArmorTemplate::new();
    t.set_default(1.0);
    t.set_coefficient(DamageType::Crush, HUMAN_ARMOR_CRUSH);
    t.set_coefficient(DamageType::ArmorPiercing, HUMAN_ARMOR_ARMOR_PIERCING);
    t.set_coefficient(DamageType::InfantryMissile, 0.10);
    t.set_coefficient(DamageType::Flame, HUMAN_ARMOR_FLAME);
    t.set_coefficient(DamageType::ParticleBeam, 1.50);
    t.set_coefficient(DamageType::Sniper, HUMAN_ARMOR_SNIPER);
    t.set_coefficient(DamageType::Laser, HUMAN_ARMOR_LASER);
    t.set_coefficient(DamageType::HazardCleanup, 0.0);
    t.set_coefficient(DamageType::KillPilot, 0.0);
    t.set_coefficient(DamageType::Surrender, 1.0);
    t.set_coefficient(DamageType::SubdualMissile, 0.0);
    t.set_coefficient(DamageType::SubdualVehicle, 0.0);
    t.set_coefficient(DamageType::SubdualBuilding, 0.0);
    t
}

/// Build retail TankArmor residual template from key coefficient residual.
pub fn build_tank_armor_residual() -> ArmorTemplate {
    let mut t = ArmorTemplate::new();
    t.set_default(1.0);
    t.set_coefficient(DamageType::Crush, 0.50);
    t.set_coefficient(DamageType::SmallArms, TANK_ARMOR_SMALL_ARMS);
    t.set_coefficient(DamageType::Gattling, TANK_ARMOR_GATTLING);
    t.set_coefficient(DamageType::ComancheVulcan, 0.25);
    t.set_coefficient(DamageType::Flame, TANK_ARMOR_FLAME);
    t.set_coefficient(DamageType::Radiation, 0.50);
    t.set_coefficient(DamageType::Microwave, 0.0);
    t.set_coefficient(DamageType::Poison, 0.25);
    t.set_coefficient(DamageType::Sniper, TANK_ARMOR_SNIPER);
    t.set_coefficient(DamageType::Melee, 0.0);
    t.set_coefficient(DamageType::Laser, TANK_ARMOR_LASER);
    t.set_coefficient(DamageType::HazardCleanup, 0.0);
    t.set_coefficient(DamageType::ParticleBeam, 1.0);
    t.set_coefficient(DamageType::KillPilot, TANK_ARMOR_KILL_PILOT);
    t.set_coefficient(DamageType::Surrender, 0.0);
    t.set_coefficient(DamageType::SubdualMissile, 0.0);
    t.set_coefficient(DamageType::SubdualVehicle, TANK_ARMOR_SUBDUAL_VEHICLE);
    t.set_coefficient(DamageType::SubdualBuilding, 0.0);
    t
}

/// Build retail StructureArmor residual template from key coefficient residual.
pub fn build_structure_armor_residual() -> ArmorTemplate {
    let mut t = ArmorTemplate::new();
    t.set_default(1.0);
    t.set_coefficient(DamageType::Surrender, 0.0);
    t.set_coefficient(DamageType::SmallArms, STRUCTURE_ARMOR_SMALL_ARMS);
    t.set_coefficient(DamageType::Gattling, STRUCTURE_ARMOR_GATTLING);
    t.set_coefficient(DamageType::ComancheVulcan, 0.50);
    t.set_coefficient(DamageType::Radiation, STRUCTURE_ARMOR_RADIATION);
    t.set_coefficient(DamageType::Microwave, 0.0);
    t.set_coefficient(DamageType::Sniper, STRUCTURE_ARMOR_SNIPER);
    t.set_coefficient(DamageType::Poison, 0.01);
    t.set_coefficient(DamageType::Melee, 0.0);
    t.set_coefficient(DamageType::Laser, 0.0);
    t.set_coefficient(DamageType::HazardCleanup, 0.0);
    t.set_coefficient(DamageType::InfantryMissile, 0.50);
    t.set_coefficient(DamageType::ParticleBeam, STRUCTURE_ARMOR_PARTICLE_BEAM);
    t.set_coefficient(DamageType::KillPilot, 0.0);
    t.set_coefficient(DamageType::AuroraBomb, STRUCTURE_ARMOR_AURORA_BOMB);
    t.set_coefficient(DamageType::LandMine, 0.0);
    t.set_coefficient(DamageType::Flame, STRUCTURE_ARMOR_FLAME);
    t.set_coefficient(DamageType::SubdualMissile, 0.0);
    t.set_coefficient(DamageType::SubdualVehicle, 0.0);
    t.set_coefficient(
        DamageType::SubdualBuilding,
        STRUCTURE_ARMOR_SUBDUAL_BUILDING,
    );
    t
}

/// Build retail AirplaneArmor residual template from key coefficient residual.
pub fn build_airplane_armor_residual() -> ArmorTemplate {
    let mut t = ArmorTemplate::new();
    t.set_default(1.0);
    t.set_coefficient(DamageType::SmallArms, AIRPLANE_ARMOR_SMALL_ARMS);
    t.set_coefficient(DamageType::Gattling, AIRPLANE_ARMOR_GATTLING);
    t.set_coefficient(DamageType::Explosion, 1.0);
    t.set_coefficient(DamageType::InfantryMissile, 1.20);
    t.set_coefficient(DamageType::Laser, AIRPLANE_ARMOR_LASER);
    t.set_coefficient(DamageType::HazardCleanup, 0.0);
    t.set_coefficient(DamageType::KillPilot, 0.0);
    t.set_coefficient(DamageType::Surrender, 0.0);
    t.set_coefficient(DamageType::JetMissiles, AIRPLANE_ARMOR_JET_MISSILES);
    t.set_coefficient(DamageType::Poison, 0.25);
    t.set_coefficient(DamageType::Radiation, 0.25);
    t.set_coefficient(DamageType::Microwave, 0.0);
    t.set_coefficient(DamageType::Sniper, AIRPLANE_ARMOR_SNIPER);
    t.set_coefficient(DamageType::SubdualMissile, 0.0);
    t.set_coefficient(DamageType::SubdualVehicle, 0.0);
    t.set_coefficient(DamageType::SubdualBuilding, 0.0);
    t.set_coefficient(DamageType::Melee, 0.0);
    t
}

/// Build retail TruckArmor residual template from key coefficient residual.
pub fn build_truck_armor_residual() -> ArmorTemplate {
    let mut t = ArmorTemplate::new();
    t.set_default(1.0);
    t.set_coefficient(DamageType::Crush, 0.50);
    t.set_coefficient(DamageType::SmallArms, TRUCK_ARMOR_SMALL_ARMS);
    t.set_coefficient(DamageType::Gattling, TRUCK_ARMOR_GATTLING);
    t.set_coefficient(DamageType::ComancheVulcan, 0.50);
    t.set_coefficient(DamageType::InfantryMissile, 0.50);
    t.set_coefficient(DamageType::Poison, 0.50);
    t.set_coefficient(DamageType::Microwave, 0.0);
    t.set_coefficient(DamageType::Sniper, TRUCK_ARMOR_SNIPER);
    t.set_coefficient(DamageType::Melee, 0.0);
    t.set_coefficient(DamageType::Laser, 0.0);
    t.set_coefficient(DamageType::HazardCleanup, 0.0);
    t.set_coefficient(DamageType::KillPilot, TRUCK_ARMOR_KILL_PILOT);
    t.set_coefficient(DamageType::Surrender, 0.0);
    t.set_coefficient(DamageType::SubdualMissile, 0.0);
    t.set_coefficient(DamageType::SubdualVehicle, TRUCK_ARMOR_SUBDUAL_VEHICLE);
    t.set_coefficient(DamageType::SubdualBuilding, 0.0);
    t
}

// --- Wave 103 specialized unit armor residual coefficients (Armor.ini key scalars) ---
// HazMatHumanArmor: FLAME 25%, POISON 0%, RADIATION 0%, SNIPER 200%, LASER 25%.
// ChemSuitHumanArmor: FLAME 150%, POISON 20%, RADIATION 20%, SNIPER 200%.
// DozerArmor: SMALL_ARMS 25%, GATTLING 10%, FLAME 25%, SNIPER 0%.
// UpgradedTankArmor: SMALL_ARMS 20%, GATTLING 10%, FLAME 10%, POISON 10%, SNIPER 0%.
// HumveeArmor: SMALL_ARMS 50%, GATTLING 50%, JET_MISSILES 30%, FLAME 50%, SNIPER 0%.
// DragonTankArmor: SMALL_ARMS 25%, GATTLING 25%, FLAME 0%, POISON 25%, SNIPER 0%.
// ToxinTruckArmor: SMALL_ARMS 50%, GATTLING 50%, POISON 0%, SNIPER 0%.
// ComancheArmor: SMALL_ARMS 120%, GATTLING 120%, EXPLOSION 130%, POISON 25%, SNIPER 0%.
// StructureArmorTough: SMALL_ARMS 50%, GATTLING 10%, EXPLOSION 80%, SNIPER 0%, FLAME 50%.

/// Retail HazMatHumanArmor residual name.
pub const HAZMAT_HUMAN_ARMOR: &str = "HazMatHumanArmor";
/// Retail ChemSuitHumanArmor residual name.
pub const CHEM_SUIT_HUMAN_ARMOR: &str = "ChemSuitHumanArmor";
/// Retail DozerArmor residual name.
pub const DOZER_ARMOR: &str = "DozerArmor";
/// Retail UpgradedTankArmor residual name.
pub const UPGRADED_TANK_ARMOR: &str = "UpgradedTankArmor";
/// Retail HumveeArmor residual name.
pub const HUMVEE_ARMOR: &str = "HumveeArmor";
/// Retail DragonTankArmor residual name.
pub const DRAGON_TANK_ARMOR: &str = "DragonTankArmor";
/// Retail ToxinTruckArmor residual name.
pub const TOXIN_TRUCK_ARMOR: &str = "ToxinTruckArmor";
/// Retail ComancheArmor residual name.
pub const COMANCHE_ARMOR: &str = "ComancheArmor";
/// Retail StructureArmorTough residual name.
pub const STRUCTURE_ARMOR_TOUGH: &str = "StructureArmorTough";

/// HazMatHumanArmor FLAME residual.
pub const HAZMAT_HUMAN_ARMOR_FLAME: f32 = 0.25;
/// HazMatHumanArmor POISON residual (immune).
pub const HAZMAT_HUMAN_ARMOR_POISON: f32 = 0.0;
/// HazMatHumanArmor SNIPER residual.
pub const HAZMAT_HUMAN_ARMOR_SNIPER: f32 = 2.0;
/// HazMatHumanArmor LASER residual.
pub const HAZMAT_HUMAN_ARMOR_LASER: f32 = 0.25;

/// ChemSuitHumanArmor POISON residual.
pub const CHEM_SUIT_HUMAN_ARMOR_POISON: f32 = 0.20;
/// ChemSuitHumanArmor FLAME residual.
pub const CHEM_SUIT_HUMAN_ARMOR_FLAME: f32 = 1.50;
/// ChemSuitHumanArmor SNIPER residual.
pub const CHEM_SUIT_HUMAN_ARMOR_SNIPER: f32 = 2.0;

/// DozerArmor SMALL_ARMS residual.
pub const DOZER_ARMOR_SMALL_ARMS: f32 = 0.25;
/// DozerArmor GATTLING residual.
pub const DOZER_ARMOR_GATTLING: f32 = 0.10;
/// DozerArmor SNIPER residual (immune).
pub const DOZER_ARMOR_SNIPER: f32 = 0.0;

/// UpgradedTankArmor SMALL_ARMS residual.
pub const UPGRADED_TANK_ARMOR_SMALL_ARMS: f32 = 0.20;
/// UpgradedTankArmor FLAME residual.
pub const UPGRADED_TANK_ARMOR_FLAME: f32 = 0.10;
/// UpgradedTankArmor POISON residual.
pub const UPGRADED_TANK_ARMOR_POISON: f32 = 0.10;

/// HumveeArmor SMALL_ARMS residual.
pub const HUMVEE_ARMOR_SMALL_ARMS: f32 = 0.50;
/// HumveeArmor JET_MISSILES residual.
pub const HUMVEE_ARMOR_JET_MISSILES: f32 = 0.30;
/// HumveeArmor FLAME residual.
pub const HUMVEE_ARMOR_FLAME: f32 = 0.50;

/// DragonTankArmor SMALL_ARMS residual.
pub const DRAGON_TANK_ARMOR_SMALL_ARMS: f32 = 0.25;
/// DragonTankArmor FLAME residual (immune).
pub const DRAGON_TANK_ARMOR_FLAME: f32 = 0.0;
/// DragonTankArmor GATTLING residual.
pub const DRAGON_TANK_ARMOR_GATTLING: f32 = 0.25;

/// ToxinTruckArmor POISON residual (immune).
pub const TOXIN_TRUCK_ARMOR_POISON: f32 = 0.0;
/// ToxinTruckArmor SMALL_ARMS residual.
pub const TOXIN_TRUCK_ARMOR_SMALL_ARMS: f32 = 0.50;

/// ComancheArmor SMALL_ARMS residual.
pub const COMANCHE_ARMOR_SMALL_ARMS: f32 = 1.20;
/// ComancheArmor EXPLOSION residual.
pub const COMANCHE_ARMOR_EXPLOSION: f32 = 1.30;
/// ComancheArmor SNIPER residual (immune).
pub const COMANCHE_ARMOR_SNIPER: f32 = 0.0;

/// StructureArmorTough EXPLOSION residual.
pub const STRUCTURE_ARMOR_TOUGH_EXPLOSION: f32 = 0.80;
/// StructureArmorTough SMALL_ARMS residual.
pub const STRUCTURE_ARMOR_TOUGH_SMALL_ARMS: f32 = 0.50;
/// StructureArmorTough GATTLING residual.
pub const STRUCTURE_ARMOR_TOUGH_GATTLING: f32 = 0.10;

/// Build retail HazMatHumanArmor residual template.
pub fn build_hazmat_human_armor_residual() -> ArmorTemplate {
    let mut t = ArmorTemplate::new();
    t.set_default(1.0);
    t.set_coefficient(DamageType::Crush, 2.0);
    t.set_coefficient(DamageType::ArmorPiercing, 0.10);
    t.set_coefficient(DamageType::Sniper, HAZMAT_HUMAN_ARMOR_SNIPER);
    t.set_coefficient(DamageType::Flame, HAZMAT_HUMAN_ARMOR_FLAME);
    t.set_coefficient(DamageType::Laser, HAZMAT_HUMAN_ARMOR_LASER);
    t.set_coefficient(DamageType::Poison, HAZMAT_HUMAN_ARMOR_POISON);
    t.set_coefficient(DamageType::Radiation, 0.0);
    t.set_coefficient(DamageType::Microwave, 0.0);
    t.set_coefficient(DamageType::ParticleBeam, 1.50);
    t.set_coefficient(DamageType::HazardCleanup, 0.0);
    t.set_coefficient(DamageType::KillPilot, 0.0);
    t.set_coefficient(DamageType::Surrender, 1.0);
    t.set_coefficient(DamageType::SubdualMissile, 0.0);
    t.set_coefficient(DamageType::SubdualVehicle, 0.0);
    t.set_coefficient(DamageType::SubdualBuilding, 0.0);
    t
}

/// Build retail ChemSuitHumanArmor residual template.
pub fn build_chem_suit_human_armor_residual() -> ArmorTemplate {
    let mut t = ArmorTemplate::new();
    t.set_default(1.0);
    t.set_coefficient(DamageType::Crush, 2.0);
    t.set_coefficient(DamageType::ArmorPiercing, 0.10);
    t.set_coefficient(DamageType::InfantryMissile, 0.10);
    t.set_coefficient(DamageType::Flame, CHEM_SUIT_HUMAN_ARMOR_FLAME);
    t.set_coefficient(DamageType::Poison, CHEM_SUIT_HUMAN_ARMOR_POISON);
    t.set_coefficient(DamageType::Radiation, 0.20);
    t.set_coefficient(DamageType::Microwave, 0.20);
    t.set_coefficient(DamageType::ParticleBeam, 1.50);
    t.set_coefficient(DamageType::Sniper, CHEM_SUIT_HUMAN_ARMOR_SNIPER);
    t.set_coefficient(DamageType::Laser, 0.50);
    t.set_coefficient(DamageType::HazardCleanup, 0.0);
    t.set_coefficient(DamageType::KillPilot, 0.0);
    t.set_coefficient(DamageType::Surrender, 1.0);
    t.set_coefficient(DamageType::SubdualMissile, 0.0);
    t.set_coefficient(DamageType::SubdualVehicle, 0.0);
    t.set_coefficient(DamageType::SubdualBuilding, 0.0);
    t
}

/// Build retail DozerArmor residual template.
pub fn build_dozer_armor_residual() -> ArmorTemplate {
    let mut t = ArmorTemplate::new();
    t.set_default(1.0);
    t.set_coefficient(DamageType::SmallArms, DOZER_ARMOR_SMALL_ARMS);
    t.set_coefficient(DamageType::Gattling, DOZER_ARMOR_GATTLING);
    t.set_coefficient(DamageType::Flame, 0.25);
    t.set_coefficient(DamageType::Poison, 0.25);
    t.set_coefficient(DamageType::Sniper, DOZER_ARMOR_SNIPER);
    t.set_coefficient(DamageType::KillPilot, 1.0);
    t.set_coefficient(DamageType::SubdualVehicle, 1.0);
    t
}

/// Build retail UpgradedTankArmor residual template.
pub fn build_upgraded_tank_armor_residual() -> ArmorTemplate {
    let mut t = ArmorTemplate::new();
    t.set_default(1.0);
    t.set_coefficient(DamageType::SmallArms, UPGRADED_TANK_ARMOR_SMALL_ARMS);
    t.set_coefficient(DamageType::Gattling, 0.10);
    t.set_coefficient(DamageType::Flame, UPGRADED_TANK_ARMOR_FLAME);
    t.set_coefficient(DamageType::Poison, UPGRADED_TANK_ARMOR_POISON);
    t.set_coefficient(DamageType::Sniper, 0.0);
    t.set_coefficient(DamageType::Laser, 0.0);
    t.set_coefficient(DamageType::KillPilot, 1.0);
    t.set_coefficient(DamageType::SubdualVehicle, 1.0);
    t
}

/// Build retail HumveeArmor residual template.
pub fn build_humvee_armor_residual() -> ArmorTemplate {
    let mut t = ArmorTemplate::new();
    t.set_default(1.0);
    t.set_coefficient(DamageType::JetMissiles, HUMVEE_ARMOR_JET_MISSILES);
    t.set_coefficient(DamageType::SmallArms, HUMVEE_ARMOR_SMALL_ARMS);
    t.set_coefficient(DamageType::Gattling, 0.50);
    t.set_coefficient(DamageType::Poison, 0.50);
    t.set_coefficient(DamageType::Sniper, 0.0);
    t.set_coefficient(DamageType::Flame, HUMVEE_ARMOR_FLAME);
    t.set_coefficient(DamageType::KillPilot, 1.0);
    t.set_coefficient(DamageType::SubdualVehicle, 1.0);
    t
}

/// Build retail DragonTankArmor residual template.
pub fn build_dragon_tank_armor_residual() -> ArmorTemplate {
    let mut t = ArmorTemplate::new();
    t.set_default(1.0);
    t.set_coefficient(DamageType::SmallArms, DRAGON_TANK_ARMOR_SMALL_ARMS);
    t.set_coefficient(DamageType::Gattling, DRAGON_TANK_ARMOR_GATTLING);
    t.set_coefficient(DamageType::Flame, DRAGON_TANK_ARMOR_FLAME);
    t.set_coefficient(DamageType::Poison, 0.25);
    t.set_coefficient(DamageType::Sniper, 0.0);
    t.set_coefficient(DamageType::KillPilot, 1.0);
    t.set_coefficient(DamageType::SubdualVehicle, 1.0);
    t
}

/// Build retail ToxinTruckArmor residual template.
pub fn build_toxin_truck_armor_residual() -> ArmorTemplate {
    let mut t = ArmorTemplate::new();
    t.set_default(1.0);
    t.set_coefficient(DamageType::SmallArms, TOXIN_TRUCK_ARMOR_SMALL_ARMS);
    t.set_coefficient(DamageType::Gattling, 0.50);
    t.set_coefficient(DamageType::Poison, TOXIN_TRUCK_ARMOR_POISON);
    t.set_coefficient(DamageType::Sniper, 0.0);
    t.set_coefficient(DamageType::KillPilot, 1.0);
    t.set_coefficient(DamageType::SubdualVehicle, 1.0);
    t
}

/// Build retail ComancheArmor residual template.
pub fn build_comanche_armor_residual() -> ArmorTemplate {
    let mut t = ArmorTemplate::new();
    t.set_default(1.0);
    t.set_coefficient(DamageType::SmallArms, COMANCHE_ARMOR_SMALL_ARMS);
    t.set_coefficient(DamageType::Gattling, 1.20);
    t.set_coefficient(DamageType::Explosion, COMANCHE_ARMOR_EXPLOSION);
    t.set_coefficient(DamageType::Poison, 0.25);
    t.set_coefficient(DamageType::Sniper, COMANCHE_ARMOR_SNIPER);
    t
}

/// Build retail StructureArmorTough residual template.
pub fn build_structure_armor_tough_residual() -> ArmorTemplate {
    let mut t = ArmorTemplate::new();
    t.set_default(1.0);
    t.set_coefficient(DamageType::SmallArms, STRUCTURE_ARMOR_TOUGH_SMALL_ARMS);
    t.set_coefficient(DamageType::Gattling, STRUCTURE_ARMOR_TOUGH_GATTLING);
    t.set_coefficient(DamageType::Sniper, 0.0);
    t.set_coefficient(DamageType::Poison, 0.01);
    t.set_coefficient(DamageType::Flame, 0.50);
    t.set_coefficient(DamageType::Explosion, STRUCTURE_ARMOR_TOUGH_EXPLOSION);
    t.set_coefficient(DamageType::SubdualBuilding, 1.0);
    t
}

/// Ensure residual armor templates are registered when store lacks them.
///
/// Prefer full Armor.ini load when available; only seed missing names.
/// Returns how many templates were registered by this call.
pub fn ensure_host_armor_residual_seed() -> usize {
    gamelogic::object::armor::ensure_default_templates_loaded();
    let mut added = 0usize;
    let seeds: &[(&str, fn() -> ArmorTemplate)] = &[
        (PROJECTILE_ARMOR, build_projectile_armor_residual),
        (
            HAZARDOUS_MATERIAL_ARMOR,
            build_hazardous_material_armor_residual,
        ),
        // Wave 92 expand:
        (HUMAN_ARMOR, build_human_armor_residual),
        (TANK_ARMOR, build_tank_armor_residual),
        (STRUCTURE_ARMOR, build_structure_armor_residual),
        (AIRPLANE_ARMOR, build_airplane_armor_residual),
        (TRUCK_ARMOR, build_truck_armor_residual),
        // Wave 103 expand:
        (HAZMAT_HUMAN_ARMOR, build_hazmat_human_armor_residual),
        (CHEM_SUIT_HUMAN_ARMOR, build_chem_suit_human_armor_residual),
        (DOZER_ARMOR, build_dozer_armor_residual),
        (UPGRADED_TANK_ARMOR, build_upgraded_tank_armor_residual),
        (HUMVEE_ARMOR, build_humvee_armor_residual),
        (DRAGON_TANK_ARMOR, build_dragon_tank_armor_residual),
        (TOXIN_TRUCK_ARMOR, build_toxin_truck_armor_residual),
        (COMANCHE_ARMOR, build_comanche_armor_residual),
        (STRUCTURE_ARMOR_TOUGH, build_structure_armor_tough_residual),
    ];
    for &(name, builder) in seeds {
        let key = AsciiString::from(name);
        if TheArmorStore::find_template(&key).is_none() {
            TheArmorStore::register_template(&key, builder());
            added += 1;
        }
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

/// Wave 92 residual honesty: HumanArmor / TankArmor / StructureArmor /
/// AirplaneArmor / TruckArmor key coefficient residual expand.
///
/// Verifies retail Armor.ini residual scalars via adjust_damage on built
/// templates and store registration. Fail-closed: not full ArmorSet upgrade
/// matrix / exclusive general armors.
pub fn honesty_armor_residual_expand_wave92() -> bool {
    let _ = ensure_host_armor_residual_seed();

    let names_ok = HUMAN_ARMOR == "HumanArmor"
        && TANK_ARMOR == "TankArmor"
        && STRUCTURE_ARMOR == "StructureArmor"
        && AIRPLANE_ARMOR == "AirplaneArmor"
        && TRUCK_ARMOR == "TruckArmor"
        && (HUMAN_ARMOR_CRUSH - 2.0).abs() < 0.001
        && (HUMAN_ARMOR_ARMOR_PIERCING - 0.10).abs() < 0.001
        && (HUMAN_ARMOR_FLAME - 1.50).abs() < 0.001
        && (HUMAN_ARMOR_SNIPER - 2.0).abs() < 0.001
        && (HUMAN_ARMOR_LASER - 0.50).abs() < 0.001
        && (TANK_ARMOR_SMALL_ARMS - 0.25).abs() < 0.001
        && (TANK_ARMOR_GATTLING - 0.10).abs() < 0.001
        && (TANK_ARMOR_FLAME - 0.25).abs() < 0.001
        && TANK_ARMOR_SNIPER == 0.0
        && TANK_ARMOR_LASER == 0.0
        && (TANK_ARMOR_KILL_PILOT - 1.0).abs() < 0.001
        && (STRUCTURE_ARMOR_SMALL_ARMS - 0.50).abs() < 0.001
        && (STRUCTURE_ARMOR_GATTLING - 0.10).abs() < 0.001
        && STRUCTURE_ARMOR_RADIATION == 0.0
        && STRUCTURE_ARMOR_SNIPER == 0.0
        && (STRUCTURE_ARMOR_PARTICLE_BEAM - 2.0).abs() < 0.001
        && (STRUCTURE_ARMOR_AURORA_BOMB - 2.50).abs() < 0.001
        && (STRUCTURE_ARMOR_FLAME - 0.50).abs() < 0.001
        && (AIRPLANE_ARMOR_SMALL_ARMS - 1.20).abs() < 0.001
        && (AIRPLANE_ARMOR_GATTLING - 1.20).abs() < 0.001
        && (AIRPLANE_ARMOR_JET_MISSILES - 0.25).abs() < 0.001
        && AIRPLANE_ARMOR_SNIPER == 0.0
        && (TRUCK_ARMOR_SMALL_ARMS - 0.50).abs() < 0.001
        && (TRUCK_ARMOR_GATTLING - 0.50).abs() < 0.001
        && TRUCK_ARMOR_SNIPER == 0.0
        && (TRUCK_ARMOR_KILL_PILOT - 1.0).abs() < 0.001;

    if !names_ok {
        return false;
    }

    let human = build_human_armor_residual();
    let human_ok = approx_eq(human.adjust_damage(DamageType::Crush, 100.0), 200.0)
        && approx_eq(human.adjust_damage(DamageType::ArmorPiercing, 100.0), 10.0)
        && approx_eq(human.adjust_damage(DamageType::Flame, 100.0), 150.0)
        && approx_eq(human.adjust_damage(DamageType::Sniper, 100.0), 200.0)
        && approx_eq(human.adjust_damage(DamageType::Laser, 100.0), 50.0)
        && approx_eq(human.adjust_damage(DamageType::KillPilot, 100.0), 0.0)
        && approx_eq(human.adjust_damage(DamageType::Surrender, 100.0), 100.0);

    let tank = build_tank_armor_residual();
    let tank_ok = approx_eq(tank.adjust_damage(DamageType::SmallArms, 100.0), 25.0)
        && approx_eq(tank.adjust_damage(DamageType::Gattling, 100.0), 10.0)
        && approx_eq(tank.adjust_damage(DamageType::Flame, 100.0), 25.0)
        && approx_eq(tank.adjust_damage(DamageType::Sniper, 100.0), 0.0)
        && approx_eq(tank.adjust_damage(DamageType::Laser, 100.0), 0.0)
        && approx_eq(tank.adjust_damage(DamageType::KillPilot, 100.0), 100.0)
        && approx_eq(tank.adjust_damage(DamageType::SubdualVehicle, 100.0), 100.0)
        && approx_eq(tank.adjust_damage(DamageType::Melee, 100.0), 0.0);

    let structure = build_structure_armor_residual();
    let structure_ok = approx_eq(structure.adjust_damage(DamageType::SmallArms, 100.0), 50.0)
        && approx_eq(structure.adjust_damage(DamageType::Gattling, 100.0), 10.0)
        && approx_eq(structure.adjust_damage(DamageType::Radiation, 100.0), 0.0)
        && approx_eq(structure.adjust_damage(DamageType::Sniper, 100.0), 0.0)
        && approx_eq(
            structure.adjust_damage(DamageType::ParticleBeam, 100.0),
            200.0,
        )
        && approx_eq(
            structure.adjust_damage(DamageType::AuroraBomb, 100.0),
            250.0,
        )
        && approx_eq(structure.adjust_damage(DamageType::Flame, 100.0), 50.0)
        && approx_eq(
            structure.adjust_damage(DamageType::SubdualBuilding, 100.0),
            100.0,
        )
        && approx_eq(structure.adjust_damage(DamageType::LandMine, 100.0), 0.0);

    let airplane = build_airplane_armor_residual();
    let airplane_ok = approx_eq(airplane.adjust_damage(DamageType::SmallArms, 100.0), 120.0)
        && approx_eq(airplane.adjust_damage(DamageType::Gattling, 100.0), 120.0)
        && approx_eq(airplane.adjust_damage(DamageType::JetMissiles, 100.0), 25.0)
        && approx_eq(airplane.adjust_damage(DamageType::Sniper, 100.0), 0.0)
        && approx_eq(airplane.adjust_damage(DamageType::Laser, 100.0), 0.0)
        && approx_eq(airplane.adjust_damage(DamageType::Poison, 100.0), 25.0);

    let truck = build_truck_armor_residual();
    let truck_ok = approx_eq(truck.adjust_damage(DamageType::SmallArms, 100.0), 50.0)
        && approx_eq(truck.adjust_damage(DamageType::Gattling, 100.0), 50.0)
        && approx_eq(truck.adjust_damage(DamageType::Sniper, 100.0), 0.0)
        && approx_eq(truck.adjust_damage(DamageType::KillPilot, 100.0), 100.0)
        && approx_eq(
            truck.adjust_damage(DamageType::SubdualVehicle, 100.0),
            100.0,
        )
        && approx_eq(truck.adjust_damage(DamageType::Melee, 100.0), 0.0);

    // Store residual: Wave 92 templates registered (INI or seed).
    let store_ok = [
        HUMAN_ARMOR,
        TANK_ARMOR,
        STRUCTURE_ARMOR,
        AIRPLANE_ARMOR,
        TRUCK_ARMOR,
    ]
    .iter()
    .all(|n| TheArmorStore::find_template(&AsciiString::from(*n)).is_some());

    // If store loaded full Armor.ini, verify key scalars still match residual.
    let store_coeff_ok = match (
        TheArmorStore::find_template(&AsciiString::from(HUMAN_ARMOR)),
        TheArmorStore::find_template(&AsciiString::from(TANK_ARMOR)),
        TheArmorStore::find_template(&AsciiString::from(STRUCTURE_ARMOR)),
        TheArmorStore::find_template(&AsciiString::from(AIRPLANE_ARMOR)),
        TheArmorStore::find_template(&AsciiString::from(TRUCK_ARMOR)),
    ) {
        (Some(h), Some(t), Some(s), Some(a), Some(tr)) => {
            approx_eq(h.adjust_damage(DamageType::Sniper, 100.0), 200.0)
                && approx_eq(h.adjust_damage(DamageType::ArmorPiercing, 100.0), 10.0)
                && approx_eq(t.adjust_damage(DamageType::SmallArms, 100.0), 25.0)
                && approx_eq(t.adjust_damage(DamageType::Sniper, 100.0), 0.0)
                && approx_eq(s.adjust_damage(DamageType::Gattling, 100.0), 10.0)
                && approx_eq(s.adjust_damage(DamageType::AuroraBomb, 100.0), 250.0)
                && approx_eq(a.adjust_damage(DamageType::JetMissiles, 100.0), 25.0)
                && approx_eq(a.adjust_damage(DamageType::SmallArms, 100.0), 120.0)
                && approx_eq(tr.adjust_damage(DamageType::SmallArms, 100.0), 50.0)
                && approx_eq(tr.adjust_damage(DamageType::Sniper, 100.0), 0.0)
        }
        _ => false,
    };

    human_ok && tank_ok && structure_ok && airplane_ok && truck_ok && store_ok && store_coeff_ok
}

/// Wave 103 residual honesty: specialized unit armor residual expand.
///
/// Verifies HazMat / ChemSuit / Dozer / UpgradedTank / Humvee / Dragon /
/// ToxinTruck / Comanche / StructureTough key Armor.ini residual scalars.
/// Fail-closed: not full ArmorSet PLAYER_UPGRADE matrix / exclusive general armors.
pub fn honesty_armor_residual_expand_wave103() -> bool {
    let _ = ensure_host_armor_residual_seed();
    if !honesty_armor_residual_expand_wave92() {
        return false;
    }

    let names_ok = HAZMAT_HUMAN_ARMOR == "HazMatHumanArmor"
        && CHEM_SUIT_HUMAN_ARMOR == "ChemSuitHumanArmor"
        && DOZER_ARMOR == "DozerArmor"
        && UPGRADED_TANK_ARMOR == "UpgradedTankArmor"
        && HUMVEE_ARMOR == "HumveeArmor"
        && DRAGON_TANK_ARMOR == "DragonTankArmor"
        && TOXIN_TRUCK_ARMOR == "ToxinTruckArmor"
        && COMANCHE_ARMOR == "ComancheArmor"
        && STRUCTURE_ARMOR_TOUGH == "StructureArmorTough"
        && (HAZMAT_HUMAN_ARMOR_FLAME - 0.25).abs() < 0.001
        && HAZMAT_HUMAN_ARMOR_POISON == 0.0
        && (HAZMAT_HUMAN_ARMOR_SNIPER - 2.0).abs() < 0.001
        && (CHEM_SUIT_HUMAN_ARMOR_POISON - 0.20).abs() < 0.001
        && (CHEM_SUIT_HUMAN_ARMOR_FLAME - 1.50).abs() < 0.001
        && (DOZER_ARMOR_SMALL_ARMS - 0.25).abs() < 0.001
        && DOZER_ARMOR_SNIPER == 0.0
        && (UPGRADED_TANK_ARMOR_SMALL_ARMS - 0.20).abs() < 0.001
        && (UPGRADED_TANK_ARMOR_FLAME - 0.10).abs() < 0.001
        && (HUMVEE_ARMOR_SMALL_ARMS - 0.50).abs() < 0.001
        && (HUMVEE_ARMOR_JET_MISSILES - 0.30).abs() < 0.001
        && (HUMVEE_ARMOR_FLAME - 0.50).abs() < 0.001
        && DRAGON_TANK_ARMOR_FLAME == 0.0
        && TOXIN_TRUCK_ARMOR_POISON == 0.0
        && (COMANCHE_ARMOR_SMALL_ARMS - 1.20).abs() < 0.001
        && (COMANCHE_ARMOR_EXPLOSION - 1.30).abs() < 0.001
        && (STRUCTURE_ARMOR_TOUGH_EXPLOSION - 0.80).abs() < 0.001;

    if !names_ok {
        return false;
    }

    let hazmat = build_hazmat_human_armor_residual();
    let hazmat_ok = approx_eq(hazmat.adjust_damage(DamageType::Flame, 100.0), 25.0)
        && approx_eq(hazmat.adjust_damage(DamageType::Poison, 100.0), 0.0)
        && approx_eq(hazmat.adjust_damage(DamageType::Sniper, 100.0), 200.0)
        && approx_eq(hazmat.adjust_damage(DamageType::Radiation, 100.0), 0.0);

    let chem = build_chem_suit_human_armor_residual();
    let chem_ok = approx_eq(chem.adjust_damage(DamageType::Poison, 100.0), 20.0)
        && approx_eq(chem.adjust_damage(DamageType::Flame, 100.0), 150.0)
        && approx_eq(chem.adjust_damage(DamageType::Sniper, 100.0), 200.0);

    let dozer = build_dozer_armor_residual();
    let dozer_ok = approx_eq(dozer.adjust_damage(DamageType::SmallArms, 100.0), 25.0)
        && approx_eq(dozer.adjust_damage(DamageType::Gattling, 100.0), 10.0)
        && approx_eq(dozer.adjust_damage(DamageType::Sniper, 100.0), 0.0);

    let upgraded = build_upgraded_tank_armor_residual();
    let upgraded_ok = approx_eq(upgraded.adjust_damage(DamageType::SmallArms, 100.0), 20.0)
        && approx_eq(upgraded.adjust_damage(DamageType::Flame, 100.0), 10.0)
        && approx_eq(upgraded.adjust_damage(DamageType::Poison, 100.0), 10.0);

    let humvee = build_humvee_armor_residual();
    let humvee_ok = approx_eq(humvee.adjust_damage(DamageType::SmallArms, 100.0), 50.0)
        && approx_eq(humvee.adjust_damage(DamageType::JetMissiles, 100.0), 30.0)
        && approx_eq(humvee.adjust_damage(DamageType::Flame, 100.0), 50.0)
        && approx_eq(humvee.adjust_damage(DamageType::Sniper, 100.0), 0.0);

    let dragon = build_dragon_tank_armor_residual();
    let dragon_ok = approx_eq(dragon.adjust_damage(DamageType::SmallArms, 100.0), 25.0)
        && approx_eq(dragon.adjust_damage(DamageType::Flame, 100.0), 0.0)
        && approx_eq(dragon.adjust_damage(DamageType::Gattling, 100.0), 25.0);

    let toxin = build_toxin_truck_armor_residual();
    let toxin_ok = approx_eq(toxin.adjust_damage(DamageType::Poison, 100.0), 0.0)
        && approx_eq(toxin.adjust_damage(DamageType::SmallArms, 100.0), 50.0);

    let comanche = build_comanche_armor_residual();
    let comanche_ok = approx_eq(comanche.adjust_damage(DamageType::SmallArms, 100.0), 120.0)
        && approx_eq(comanche.adjust_damage(DamageType::Explosion, 100.0), 130.0)
        && approx_eq(comanche.adjust_damage(DamageType::Sniper, 100.0), 0.0);

    let tough = build_structure_armor_tough_residual();
    let tough_ok = approx_eq(tough.adjust_damage(DamageType::Explosion, 100.0), 80.0)
        && approx_eq(tough.adjust_damage(DamageType::SmallArms, 100.0), 50.0)
        && approx_eq(tough.adjust_damage(DamageType::Gattling, 100.0), 10.0)
        && approx_eq(tough.adjust_damage(DamageType::Sniper, 100.0), 0.0);

    let store_names = [
        HAZMAT_HUMAN_ARMOR,
        CHEM_SUIT_HUMAN_ARMOR,
        DOZER_ARMOR,
        UPGRADED_TANK_ARMOR,
        HUMVEE_ARMOR,
        DRAGON_TANK_ARMOR,
        TOXIN_TRUCK_ARMOR,
        COMANCHE_ARMOR,
        STRUCTURE_ARMOR_TOUGH,
    ];
    let store_ok = store_names
        .iter()
        .all(|n| TheArmorStore::find_template(&AsciiString::from(*n)).is_some());

    // If store loaded full Armor.ini, verify key scalars still match residual.
    let store_coeff_ok = match (
        TheArmorStore::find_template(&AsciiString::from(HAZMAT_HUMAN_ARMOR)),
        TheArmorStore::find_template(&AsciiString::from(DRAGON_TANK_ARMOR)),
        TheArmorStore::find_template(&AsciiString::from(TOXIN_TRUCK_ARMOR)),
        TheArmorStore::find_template(&AsciiString::from(STRUCTURE_ARMOR_TOUGH)),
        TheArmorStore::find_template(&AsciiString::from(HUMVEE_ARMOR)),
    ) {
        (Some(h), Some(d), Some(t), Some(s), Some(u)) => {
            approx_eq(h.adjust_damage(DamageType::Poison, 100.0), 0.0)
                && approx_eq(h.adjust_damage(DamageType::Flame, 100.0), 25.0)
                && approx_eq(d.adjust_damage(DamageType::Flame, 100.0), 0.0)
                && approx_eq(t.adjust_damage(DamageType::Poison, 100.0), 0.0)
                && approx_eq(s.adjust_damage(DamageType::Explosion, 100.0), 80.0)
                && approx_eq(u.adjust_damage(DamageType::JetMissiles, 100.0), 30.0)
        }
        _ => false,
    };

    hazmat_ok
        && chem_ok
        && dozer_ok
        && upgraded_ok
        && humvee_ok
        && dragon_ok
        && toxin_ok
        && comanche_ok
        && tough_ok
        && store_ok
        && store_coeff_ok
}

#[inline]
fn approx_eq(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.05
}

/// Map host combat damage class → Armor.ini DamageType residual.
pub fn map_host_damage_type(dt: crate::game_logic::combat::DamageType) -> DamageType {
    use crate::game_logic::combat::DamageType as H;
    match dt {
        H::Bullet => DamageType::SmallArms,
        H::Explosive => DamageType::Explosion,
        H::Fire | H::Flame => DamageType::Flame,
        H::Laser => DamageType::Laser,
        H::Toxin | H::Anthrax => DamageType::Poison,
        H::Radiation => DamageType::Radiation,
        H::EMP => DamageType::Microwave,
        H::Unresistable => DamageType::Unresistable,
        H::Falling => DamageType::Falling,
        H::Status => DamageType::Status,
        H::KillPilot => DamageType::KillPilot,
        H::Disarm => DamageType::Disarm,
        H::Deploy => DamageType::Deploy,
        H::Hack => DamageType::Hack,
        H::Surrender => DamageType::Surrender,
        H::Penalty => DamageType::Penalty,
        H::KillGarrisoned => DamageType::KillGarrisoned,
        H::Healing => DamageType::Healing,
        H::Water => DamageType::Water,
    }
}

/// Pick residual Armor.ini template by host object kind (fail-closed coarse matrix).
pub fn residual_armor_for_object(obj: &crate::game_logic::Object) -> ArmorTemplate {
    use crate::game_logic::KindOf;
    if obj.is_kind_of(KindOf::Aircraft) {
        return build_airplane_armor_residual();
    }
    if obj.is_kind_of(KindOf::Structure) || obj.is_kind_of(KindOf::Immobile) {
        return build_structure_armor_residual();
    }
    if obj.is_kind_of(KindOf::Infantry) {
        // C++ ChemSuitHumanArmor when Upgrade_AmericaChemicalSuits residual is active.
        if obj.has_upgrade_tag("Upgrade_AmericaChemicalSuits")
            || obj.has_upgrade_tag("UpgradeChemicalSuits")
            || obj
                .applied_upgrades
                .iter()
                .any(|u| u.to_ascii_lowercase().contains("chemicalsuit"))
        {
            return build_chem_suit_human_armor_residual();
        }
        return build_human_armor_residual();
    }
    if obj.is_kind_of(KindOf::Vehicle) {
        return build_tank_armor_residual();
    }
    let mut t = ArmorTemplate::new();
    t.set_default(1.0);
    t
}

/// Apply residual armor coefficient to raw damage.
pub fn apply_residual_armor(
    obj: &crate::game_logic::Object,
    host_damage_type: crate::game_logic::combat::DamageType,
    amount: f32,
) -> f32 {
    let armor = residual_armor_for_object(obj);
    let dt = map_host_damage_type(host_damage_type);
    armor.adjust_damage(dt, amount).max(0.0)
}

/// Map gamelogic / Weapon.ini DamageType → host combat damage class residual.
pub fn map_store_damage_type(
    dt: gamelogic::damage::DamageType,
) -> crate::game_logic::combat::DamageType {
    use crate::game_logic::combat::DamageType as H;
    use gamelogic::damage::DamageType as G;
    match dt {
        G::Explosion | G::LandMine | G::AuroraBomb | G::MolotovCocktail => H::Explosive,
        G::Flame => H::Flame,
        G::Laser | G::ParticleBeam => H::Laser,
        G::Poison => H::Toxin,
        G::Radiation => H::Radiation,
        G::Microwave => H::EMP,
        G::Falling => H::Falling,
        G::Status => H::Status,
        G::KillPilot => H::KillPilot,
        G::Disarm => H::Disarm,
        G::Deploy => H::Deploy,
        G::Hack => H::Hack,
        G::Surrender => H::Surrender,
        G::Penalty => H::Penalty,
        G::KillGarrisoned => H::KillGarrisoned,
        G::Healing => H::Healing,
        G::Water => H::Water,
        G::Unresistable
        | G::Toppling
        | G::SubdualMissile
        | G::SubdualVehicle
        | G::SubdualBuilding
        | G::SubdualUnresistable
        | G::HazardCleanup => H::Unresistable,
        G::SmallArms
        | G::ComancheVulcan
        | G::Melee
        | G::Crush
        | G::ArmorPiercing
        | G::InfantryMissile
        | G::JetMissiles
        | G::StealthJetMissiles
        | G::Gattling
        | G::Sniper => H::Bullet,
        _ => H::Bullet,
    }
}

/// Look up Weapon.ini DamageType residual by weapon template name.
pub fn host_damage_type_for_weapon_name(name: &str) -> crate::game_logic::combat::DamageType {
    use gamelogic::weapon::with_weapon_store;
    let dt = with_weapon_store(|store| {
        store
            .find_weapon_template(name)
            .map(|wt| map_store_damage_type(wt.damage_type))
    })
    .ok()
    .flatten();
    if let Some(d) = dt {
        return d;
    }
    if crate::game_logic::weapon_bootstrap::host_weapon_is_status_damage(name) {
        return crate::game_logic::combat::DamageType::Status;
    }
    if crate::game_logic::weapon_bootstrap::host_weapon_is_kill_pilot_damage(name) {
        return crate::game_logic::combat::DamageType::KillPilot;
    }
    if crate::game_logic::weapon_bootstrap::host_weapon_is_disarm_damage(name) {
        return crate::game_logic::combat::DamageType::Disarm;
    }
    if crate::game_logic::weapon_bootstrap::host_weapon_is_deploy_damage(name) {
        return crate::game_logic::combat::DamageType::Deploy;
    }
    if crate::game_logic::weapon_bootstrap::host_weapon_is_hack_damage(name) {
        return crate::game_logic::combat::DamageType::Hack;
    }
    if crate::game_logic::weapon_bootstrap::host_weapon_is_surrender_damage(name) {
        return crate::game_logic::combat::DamageType::Surrender;
    }
    if crate::game_logic::weapon_bootstrap::host_weapon_is_kill_garrisoned_damage(name) {
        return crate::game_logic::combat::DamageType::KillGarrisoned;
    }
    if crate::game_logic::weapon_bootstrap::host_weapon_is_healing_damage(name) {
        return crate::game_logic::combat::DamageType::Healing;
    }
    if crate::game_logic::weapon_bootstrap::host_weapon_is_water_damage(name) {
        return crate::game_logic::combat::DamageType::Water;
    }
    crate::game_logic::combat::DamageType::Bullet
}

/// Look up Weapon.ini DeathType residual by weapon template name.
pub fn host_death_type_for_weapon_name(
    name: &str,
) -> crate::game_logic::host_usa_pilot::HostDeathType {
    use crate::game_logic::host_usa_pilot::HostDeathType;
    use gamelogic::weapon::with_weapon_store;
    let dt = with_weapon_store(|store| {
        store.find_weapon_template(name).map(|wt| {
            let d = HostDeathType::from_store(wt.death_type);
            // When store leaves Normal, keep Normal (caller may refine by DamageType).
            d
        })
    })
    .ok()
    .flatten();
    dt.unwrap_or(HostDeathType::Normal)
}

/// Resolve kill DeathType: store DeathType, else DamageType residual mapping.
pub fn resolve_host_death_type(
    weapon_name: Option<&str>,
    damage_type: crate::game_logic::combat::DamageType,
) -> crate::game_logic::host_usa_pilot::HostDeathType {
    use crate::game_logic::host_usa_pilot::HostDeathType;
    if let Some(n) = weapon_name {
        let _ = crate::game_logic::weapon_bootstrap::ensure_host_weapon_store();
        if crate::game_logic::thing::ThingTemplate::weapon_from_store(n).is_some() {
            let d = host_death_type_for_weapon_name(n);
            if d != HostDeathType::Normal {
                return d;
            }
        }
    }
    HostDeathType::from_host_damage_type(damage_type)
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
    fn armor_residual_expand_wave92_honesty() {
        assert!(honesty_armor_residual_expand_wave92());
        let tank = build_tank_armor_residual();
        assert!((tank.adjust_damage(DamageType::SmallArms, 40.0) - 10.0).abs() < 0.01);
        let human = build_human_armor_residual();
        assert!((human.adjust_damage(DamageType::Sniper, 50.0) - 100.0).abs() < 0.01);
        let structure = build_structure_armor_residual();
        assert!((structure.adjust_damage(DamageType::AuroraBomb, 100.0) - 250.0).abs() < 0.01);
    }

    #[test]
    fn armor_residual_names_match_retail() {
        assert_eq!(PROJECTILE_ARMOR, "ProjectileArmor");
        assert_eq!(HAZARDOUS_MATERIAL_ARMOR, "HazardousMaterialArmor");
        assert_eq!(HUMAN_ARMOR, "HumanArmor");
        assert_eq!(TANK_ARMOR, "TankArmor");
        assert_eq!(STRUCTURE_ARMOR, "StructureArmor");
        assert_eq!(AIRPLANE_ARMOR, "AirplaneArmor");
        assert_eq!(TRUCK_ARMOR, "TruckArmor");
        assert_eq!(HAZMAT_HUMAN_ARMOR, "HazMatHumanArmor");
        assert_eq!(DRAGON_TANK_ARMOR, "DragonTankArmor");
        assert_eq!(STRUCTURE_ARMOR_TOUGH, "StructureArmorTough");
    }

    #[test]
    fn armor_residual_expand_pack_honesty_wave103() {
        assert!(honesty_armor_residual_expand_wave103());
        let dragon = build_dragon_tank_armor_residual();
        assert!((dragon.adjust_damage(DamageType::Flame, 100.0)).abs() < 0.01);
        let hazmat = build_hazmat_human_armor_residual();
        assert!((hazmat.adjust_damage(DamageType::Poison, 100.0)).abs() < 0.01);
        let tough = build_structure_armor_tough_residual();
        assert!((tough.adjust_damage(DamageType::Explosion, 100.0) - 80.0).abs() < 0.01);
    }

    #[test]
    fn store_damage_type_maps_explosion_and_laser() {
        use gamelogic::damage::DamageType as G;
        assert_eq!(
            map_store_damage_type(G::Explosion),
            crate::game_logic::combat::DamageType::Explosive
        );
        assert_eq!(
            map_store_damage_type(G::Laser),
            crate::game_logic::combat::DamageType::Laser
        );
        assert_eq!(
            map_store_damage_type(G::SmallArms),
            crate::game_logic::combat::DamageType::Bullet
        );
    }

    #[test]
    fn host_weapon_name_damage_type_uses_seeded_store() {
        let _ = crate::game_logic::weapon_bootstrap::ensure_host_weapon_store();
        let laser = host_damage_type_for_weapon_name("PaladinPointDefenseLaser");
        assert_eq!(laser, crate::game_logic::combat::DamageType::Laser);
        let rifle = host_damage_type_for_weapon_name("RangerAdvancedCombatRifle");
        assert_eq!(rifle, crate::game_logic::combat::DamageType::Bullet);
        let bomb = host_damage_type_for_weapon_name("AuroraBombWeapon");
        assert_eq!(bomb, crate::game_logic::combat::DamageType::Explosive);
    }

    #[test]
    fn seeded_weapon_death_type_residual() {
        use crate::game_logic::host_usa_pilot::HostDeathType;
        let _ = crate::game_logic::weapon_bootstrap::ensure_host_weapon_store();
        assert_eq!(
            host_death_type_for_weapon_name("PaladinPointDefenseLaser"),
            HostDeathType::Lasered
        );
        assert_eq!(
            host_death_type_for_weapon_name("AuroraBombWeapon"),
            HostDeathType::Exploded
        );
        assert_eq!(
            resolve_host_death_type(
                Some("PaladinPointDefenseLaser"),
                crate::game_logic::combat::DamageType::Bullet
            ),
            HostDeathType::Lasered
        );
    }
}
