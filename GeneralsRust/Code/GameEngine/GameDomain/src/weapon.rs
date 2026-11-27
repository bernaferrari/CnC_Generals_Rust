//! Weapon slot and runtime state identities shared by simulation and presentation.

use serde::{Deserialize, Serialize};

/// Authored weapon slots in C++ GameType.h.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum WeaponSlotType {
    Primary = 0,
    Secondary = 1,
    Tertiary = 2,
}

impl WeaponSlotType {
    /// C++ WeaponSet visits slots in descending ordinal order.
    pub const DESCENDING: [Self; 3] = [Self::Tertiary, Self::Secondary, Self::Primary];

    pub const fn from_u32(value: u32) -> Option<Self> {
        match value {
            0 => Some(Self::Primary),
            1 => Some(Self::Secondary),
            2 => Some(Self::Tertiary),
            _ => None,
        }
    }
}

/// C++ `WeaponStatus` (WeaponStatus.h) for the active weapon slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum WeaponFireStatus {
    ReadyToFire = 0,
    OutOfAmmo = 1,
    BetweenFiringShots = 2,
    ReloadingClip = 3,
    PreAttack = 4,
}

impl WeaponFireStatus {
    pub const fn from_ordinal(value: u8) -> Option<Self> {
        Some(match value {
            0 => Self::ReadyToFire,
            1 => Self::OutOfAmmo,
            2 => Self::BetweenFiringShots,
            3 => Self::ReloadingClip,
            4 => Self::PreAttack,
            _ => return None,
        })
    }
}

impl Default for WeaponFireStatus {
    fn default() -> Self {
        Self::ReadyToFire
    }
}

/// C++ WeaponLockType (WeaponSet.h).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[repr(u8)]
pub enum WeaponLockType {
    #[default]
    NotLocked = 0,
    /// Locked until clip empty / attack state exits.
    LockedTemporarily = 1,
    /// Locked until explicitly unlocked or lock changes.
    LockedPermanently = 2,
}
