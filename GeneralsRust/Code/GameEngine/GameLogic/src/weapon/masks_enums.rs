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

/// Weapon slot types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeaponSlotType {
    Primary,
    Secondary,
    Tertiary,
}

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

impl From<WeaponSlotType> for crate::common::WeaponSlotType {
    fn from(value: WeaponSlotType) -> Self {
        match value {
            WeaponSlotType::Primary => crate::common::WeaponSlotType::Primary,
            WeaponSlotType::Secondary => crate::common::WeaponSlotType::Secondary,
            WeaponSlotType::Tertiary => crate::common::WeaponSlotType::Tertiary,
        }
    }
}

impl From<crate::common::WeaponSlotType> for WeaponSlotType {
    fn from(value: crate::common::WeaponSlotType) -> Self {
        match value {
            crate::common::WeaponSlotType::Primary => WeaponSlotType::Primary,
            crate::common::WeaponSlotType::Secondary => WeaponSlotType::Secondary,
            crate::common::WeaponSlotType::Tertiary => WeaponSlotType::Tertiary,
        }
    }
}

/// Weapon status — discriminants must match C++ WeaponStatus enum (WeaponStatus.h)
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeaponStatus {
    ReadyToFire,        // C++ = 0
    OutOfAmmo,          // C++ = 1
    BetweenFiringShots, // C++ = 2
    ReloadingClip,      // C++ = 3
    PreAttack,          // C++ = 4
}

/// Weapon firing errors
#[derive(Debug, Clone, PartialEq)]
pub enum WeaponError {
    NoAmmo,
    NotReady { time_remaining: f32 },
    OutOfRange { distance: f32, max_range: f32 },
    TargetObstructed,
    TargetNotVisible,
    InvalidTarget,
    NoTemplate,
    SystemError(String),
}

impl std::fmt::Display for WeaponError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WeaponError::NoAmmo => write!(f, "Weapon has no ammunition"),
            WeaponError::NotReady { time_remaining } => {
                write!(
                    f,
                    "Weapon not ready to fire ({:.2}s remaining)",
                    time_remaining
                )
            }
            WeaponError::OutOfRange {
                distance,
                max_range,
            } => {
                write!(
                    f,
                    "Target out of range ({:.1} > {:.1})",
                    distance, max_range
                )
            }
            WeaponError::TargetObstructed => write!(f, "Line of sight to target obstructed"),
            WeaponError::TargetNotVisible => write!(f, "Target is outside vision range"),
            WeaponError::InvalidTarget => write!(f, "Invalid or dead target"),
            WeaponError::NoTemplate => write!(f, "Weapon template not available"),
            WeaponError::SystemError(msg) => write!(f, "System error: {}", msg),
        }
    }
}

impl std::error::Error for WeaponError {}

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
