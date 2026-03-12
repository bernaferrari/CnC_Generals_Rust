//! Object Helper Modules - Utility modules for various game mechanics
//!
//! Helper modules provide utility functionality for objects that doesn't fit
//! into standard module categories. They handle things like:
//! - Defection timers and visual effects
//! - Repulsor physics (unit pushing)
//! - Status condition management
//! - Subdual damage healing
//! - Special model condition states
//! - Weapon status tracking
//! - Temporary weapon bonuses
//!
//! Original C++ Authors: Steven Johnson, Colin Day, Graham Smallwood (2002-2003)
//! Rust conversion: 2025

use crate::common::*;
use crate::modules::UpdateModuleInterface;
use std::sync::{Arc, RwLock};

pub mod object_defection_helper;
pub mod object_helper;
pub mod object_repulsor_helper;
pub mod object_smc_helper;
pub mod object_weapon_status_helper;
pub mod status_damage_helper;
pub mod subdual_damage_helper;
pub mod temp_weapon_bonus_helper;

pub use object_defection_helper::{ObjectDefectionHelper, ObjectDefectionHelperModuleData};
pub use object_helper::*;
pub use object_repulsor_helper::{ObjectRepulsorHelper, ObjectRepulsorHelperModuleData};
pub use object_smc_helper::{ObjectSMCHelper, ObjectSMCHelperModuleData};
pub use object_weapon_status_helper::{
    ObjectWeaponStatusHelper, ObjectWeaponStatusHelperModuleData,
};
pub use status_damage_helper::{StatusDamageHelper, StatusDamageHelperModuleData};
pub use subdual_damage_helper::{SubdualDamageHelper, SubdualDamageHelperModuleData};
pub use temp_weapon_bonus_helper::{TempWeaponBonusHelper, TempWeaponBonusHelperModuleData};

/// Update sleep time returned by helper modules
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum UpdateSleepTime {
    /// Update every frame
    None,
    /// Update after N frames
    Frames(u32),
    /// Never update again
    Forever,
}

impl UpdateSleepTime {
    pub const FOREVER: UpdateSleepTime = UpdateSleepTime::Forever;
    pub const NONE: UpdateSleepTime = UpdateSleepTime::None;

    pub fn frames(n: u32) -> Self {
        UpdateSleepTime::Frames(n)
    }

    /// Convert from u32 representation (for compatibility)
    /// 0 = None, u32::MAX = Forever, other = Frames(n)
    pub fn from_u32(value: u32) -> Self {
        match value {
            0 => UpdateSleepTime::None,
            u32::MAX => UpdateSleepTime::Forever,
            n => UpdateSleepTime::Frames(n),
        }
    }

    /// Convert to u32 representation (for compatibility)
    /// None = 0, Forever = u32::MAX, Frames(n) = n
    pub fn to_u32(self) -> u32 {
        match self {
            UpdateSleepTime::None => 0,
            UpdateSleepTime::Forever => u32::MAX,
            UpdateSleepTime::Frames(n) => n,
        }
    }

    /// Get the maximum of two sleep times
    pub fn max(self, other: Self) -> Self {
        if self > other {
            self
        } else {
            other
        }
    }
}

/// Base trait for all object helper modules
///
/// Helpers are lightweight update modules that handle utility functions.
/// They typically sleep most of the time and wake up only when needed.
pub trait ObjectHelperInterface: Send + Sync + std::fmt::Debug {
    /// Update the helper module
    /// Returns when the module wants to be updated next
    fn update(&mut self, current_frame: u32) -> UpdateSleepTime;

    /// Get the helper module name
    fn get_module_name(&self) -> &str;

    /// Sleep until a specific frame
    fn sleep_until(&mut self, wake_frame: u32);

    /// Check if module should process disabled objects
    fn get_disabled_types_to_process(&self) -> DisabledMaskType {
        DisabledMaskType::None
    }
}

/// Disabled mask types - which disabled states to process
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisabledMaskType {
    None,
    All,
    Specific(u32),
}

impl DisabledMaskType {
    pub const DISABLEDMASK_ALL: DisabledMaskType = DisabledMaskType::All;
}

/// Update phase for helpers that need to run in specific phases
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SleepyUpdatePhase {
    /// Normal update phase
    Normal,
    /// Final update phase (after all normal updates)
    Final,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_update_sleep_time() {
        assert_eq!(UpdateSleepTime::Forever, UpdateSleepTime::FOREVER);
        assert_eq!(UpdateSleepTime::None, UpdateSleepTime::NONE);
        assert_eq!(UpdateSleepTime::frames(10), UpdateSleepTime::Frames(10));
    }

    #[test]
    fn test_disabled_mask() {
        assert_eq!(DisabledMaskType::All, DisabledMaskType::DISABLEDMASK_ALL);
    }
}
