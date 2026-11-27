//! Legacy weapon types tied to GameLogic coordinates and status conversions.

use crate::common::Coord3D;

pub use generals_sim::weapon::WeaponDamageStatus as ObjectStatusTypes;

pub use generals_sim::weapon::{
    Coord2D,
    WEAPON_AFFECTS_MASK_NAMES,
    WEAPON_BONUS_CONDITION_COUNT,
    WEAPON_BONUS_FIELD_COUNT,
    WEAPON_BONUS_FIELD_NAMES,
    WEAPON_BONUS_NAMES,
    WEAPON_COLLIDE_MASK_NAMES,
    WEAPON_PREFIRE_COUNT,
    WEAPON_PREFIRE_NAMES,
    WEAPON_RELOAD_NAMES,
    WeaponAffectsMask,
    WeaponAntiMask,
    WeaponBonus,
    WeaponBonusConditionFlags,
    WeaponBonusConditionType,
    WeaponBonusField,
    WeaponBonusSet,
    WeaponCollideMask,
    WeaponPrefireType,
    WeaponReloadType,
};

/// Historic weapon damage tracking
#[derive(Debug, Clone)]
pub struct HistoricWeaponDamageInfo {
    pub frame: u32,
    pub location: Coord3D,
}

impl HistoricWeaponDamageInfo {
    pub fn new(frame: u32, location: Coord3D) -> Self {
        Self { frame, location }
    }
}

pub use generals_sim::weapon::WeaponSlotType;

/// Veterancy levels
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VeterancyLevel {
    Regular = 0,
    Veteran = 1,
    Elite = 2,
    Heroic = 3,
}

// damage_system now re-exports canonical types from crate::damage,
// so no From<> conversion impls needed between the two.

pub use generals_sim::error::WeaponError;
pub use generals_sim::weapon::WeaponFireStatus as WeaponStatus;

/// Fire mode for different weapon types
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FireMode {
    /// Projectile weapon (creates physical projectile object)
    Projectile { speed: f32, lifetime: f32 },
    /// Instant impact weapon (no travel time)
    InstantImpact { splash_radius: f32 },
    /// Continuous beam weapon (sustained damage over time)
    ContinuousBeam {
        duration: f32,
        damage_per_frame: f32,
    },
}

/// Object type for scatter calculations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectType {
    Infantry,
    Vehicle,
    Structure,
    Projectile,
    Unknown,
}
