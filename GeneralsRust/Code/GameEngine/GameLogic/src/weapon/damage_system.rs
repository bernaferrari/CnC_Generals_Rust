//! Re-exports from the canonical damage module.
//!
//! This file previously contained duplicate DamageType, DeathType, ObjectStatusTypes,
//! DamageInfoInput, DamageInfoOutput, DamageInfo, ArmorTemplate, and DamageCalculator
//! definitions. Those have been removed — the correct implementations live in
//! `crate::damage` and `crate::common`. This module now only re-exports for
//! backward compatibility with code importing `crate::weapon::damage_system::{DamageType, DeathType}`.

// Re-export canonical DamageType and DeathType from crate::damage for
// backward compatibility (common/types.rs imports from here).
pub use crate::damage::{DamageType, DeathType};
