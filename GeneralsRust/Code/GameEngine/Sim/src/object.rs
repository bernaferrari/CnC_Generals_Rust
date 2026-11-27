//! Object status and experience identities used by simulation rules.

use std::fmt;

pub mod ai;
pub mod body;
mod ids;
mod movement;
mod roster;
pub use generals_game_domain::body::BodyDamageType;
pub use generals_game_domain::object_status::ObjectShroudStatus;
pub use ids::ObjectIds;
pub use movement::Movement;
pub use roster::{
    ConstructorLookupError, ConstructorLookupReceipt, ObjectIdentity, ObjectRoster, RosterEntry,
};

mod status_mask;
pub use status_mask::ObjectStatusMaskType;

/// Object status types (matching C++ ObjectStatusTypes)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum ObjectStatusTypes {
    None = 0,
    Destroyed,
    CanAttack,
    UnderConstruction,
    Unselectable,
    NoCollisions,
    NoAttack,
    AirborneTarget,
    Parachuting,
    Repulsor,
    Hijacked,
    Aflame,
    Burned,
    Wet,
    IsFiringWeapon,
    Braking,
    Stealthed,
    Detected,
    CanStealth,
    Sold,
    UndergoingRepair,
    Reconstructing,
    Masked,
    IsAttacking,
    IsUsingAbility,
    IsAimingWeapon,
    NoAttackFromAi,
    IgnoringStealth,
    IsCarBomb,
    DeckHeightOffset,
    Rider1,
    Rider2,
    Rider3,
    Rider4,
    Rider5,
    Rider6,
    Rider7,
    Rider8,
    FaerieFire,
    MissileKillingSelf,
    ReassignParking,
    BoobyTrapped,
    Immobile,
    Disguised,
    Deployed,
}

impl ObjectStatusTypes {
    /// Convert from a raw integer value, defaulting to `ObjectStatusTypes::None`.
    pub fn from_u32(value: u32) -> Self {
        match value {
            0 => ObjectStatusTypes::None,
            1 => ObjectStatusTypes::Destroyed,
            2 => ObjectStatusTypes::CanAttack,
            3 => ObjectStatusTypes::UnderConstruction,
            4 => ObjectStatusTypes::Unselectable,
            5 => ObjectStatusTypes::NoCollisions,
            6 => ObjectStatusTypes::NoAttack,
            7 => ObjectStatusTypes::AirborneTarget,
            8 => ObjectStatusTypes::Parachuting,
            9 => ObjectStatusTypes::Repulsor,
            10 => ObjectStatusTypes::Hijacked,
            11 => ObjectStatusTypes::Aflame,
            12 => ObjectStatusTypes::Burned,
            13 => ObjectStatusTypes::Wet,
            14 => ObjectStatusTypes::IsFiringWeapon,
            15 => ObjectStatusTypes::Braking,
            16 => ObjectStatusTypes::Stealthed,
            17 => ObjectStatusTypes::Detected,
            18 => ObjectStatusTypes::CanStealth,
            19 => ObjectStatusTypes::Sold,
            20 => ObjectStatusTypes::UndergoingRepair,
            21 => ObjectStatusTypes::Reconstructing,
            22 => ObjectStatusTypes::Masked,
            23 => ObjectStatusTypes::IsAttacking,
            24 => ObjectStatusTypes::IsUsingAbility,
            25 => ObjectStatusTypes::IsAimingWeapon,
            26 => ObjectStatusTypes::NoAttackFromAi,
            27 => ObjectStatusTypes::IgnoringStealth,
            28 => ObjectStatusTypes::IsCarBomb,
            29 => ObjectStatusTypes::DeckHeightOffset,
            30 => ObjectStatusTypes::Rider1,
            31 => ObjectStatusTypes::Rider2,
            32 => ObjectStatusTypes::Rider3,
            33 => ObjectStatusTypes::Rider4,
            34 => ObjectStatusTypes::Rider5,
            35 => ObjectStatusTypes::Rider6,
            36 => ObjectStatusTypes::Rider7,
            37 => ObjectStatusTypes::Rider8,
            38 => ObjectStatusTypes::FaerieFire,
            39 => ObjectStatusTypes::MissileKillingSelf,
            40 => ObjectStatusTypes::ReassignParking,
            41 => ObjectStatusTypes::BoobyTrapped,
            42 => ObjectStatusTypes::Immobile,
            43 => ObjectStatusTypes::Disguised,
            44 => ObjectStatusTypes::Deployed,
            _ => ObjectStatusTypes::None,
        }
    }

    // Legacy C++-style aliases used by in-progress ported call sites.
    pub const OBJECT_STATUS_IS_ATTACKING: ObjectStatusTypes = ObjectStatusTypes::IsAttacking;
}

/// Veterancy level (matching C++ VeterancyLevel)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
#[repr(i32)]
pub enum VeterancyLevel {
    Rookie = 0,
    Veteran = 1,
    Elite = 2,
    Heroic = 3,
}

impl VeterancyLevel {
    /// C++ INI name for the unpromoted rank.
    #[allow(non_upper_case_globals)]
    pub const Regular: Self = Self::Rookie;

    pub fn saturating_add_levels(self, delta: i32) -> Self {
        let raw = self as i32;
        let min = VeterancyLevel::Rookie as i32;
        let max = VeterancyLevel::Heroic as i32;
        let clamped = (raw + delta).clamp(min, max);
        match clamped {
            0 => VeterancyLevel::Rookie,
            1 => VeterancyLevel::Veteran,
            2 => VeterancyLevel::Elite,
            _ => VeterancyLevel::Heroic,
        }
    }
}

impl fmt::Display for VeterancyLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            VeterancyLevel::Rookie => "Regular",
            VeterancyLevel::Veteran => "Veteran",
            VeterancyLevel::Elite => "Elite",
            VeterancyLevel::Heroic => "Heroic",
        };
        write!(f, "{}", label)
    }
}

/// Status applied by a weapon, stored as the original single status ordinal.
#[derive(Debug, Clone, Copy)]
pub struct WeaponDamageStatus(u32);

impl WeaponDamageStatus {
    pub const fn bits(&self) -> u32 {
        self.0
    }

    pub const NONE: u32 = 0;

    pub fn new(status: u32) -> Self {
        Self(status)
    }
}

impl From<WeaponDamageStatus> for ObjectStatusTypes {
    fn from(value: WeaponDamageStatus) -> ObjectStatusTypes {
        ObjectStatusTypes::from_u32(value.0)
    }
}

impl From<ObjectStatusTypes> for WeaponDamageStatus {
    fn from(value: ObjectStatusTypes) -> Self {
        WeaponDamageStatus::new(value as u32)
    }
}
