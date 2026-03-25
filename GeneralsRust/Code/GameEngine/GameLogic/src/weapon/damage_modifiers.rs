//! Damage Modifiers — C++ Parity Stub
//!
//! PARITY_NOTE: This entire module was fabricated and has been gutted.
//! The following mechanics have NO equivalent in C++ GeneralsMD:
//!   - Critical hits (5% base chance, 2x multiplier)
//!   - Headshot system (3x multiplier, enable_headshots flag)
//!   - Sniper auto-crit on infantry
//!   - Veterancy crit bonus (+2%/+5%/+10%)
//!   - Damage variance (±10% random, normal distribution)
//!   - Range falloff curves (Linear, Quadratic, InverseQuadratic, Exponential)
//!
//! In C++, weapon damage is a fixed value from WeaponTemplate. Veterancy
//! bonuses are applied to weapon OUTPUT only (via WeaponBonusSet conditions
//! VETERAN/ELITE/HERO), never as random variance or crit rolls.
//!
//! All types are kept as stubs to avoid breaking callers.

// ---------------------------------------------------------------------------
// Stub types (all fabricated mechanics removed)
// ---------------------------------------------------------------------------

/// PARITY_NOTE: Fabricated — C++ has no critical hit system.
/// Kept as empty struct for API compatibility.
#[derive(Debug, Clone)]
pub struct CriticalHitConfig {}

impl CriticalHitConfig {
    pub fn new() -> Self {
        Self {}
    }
}

impl Default for CriticalHitConfig {
    fn default() -> Self {
        Self::new()
    }
}

/// PARITY_NOTE: Fabricated — C++ has no damage randomization.
/// Kept as empty struct for API compatibility.
#[derive(Debug, Clone)]
pub struct DamageVarianceConfig {}

impl DamageVarianceConfig {
    pub fn new() -> Self {
        Self {}
    }

    pub fn no_variance() -> Self {
        Self {}
    }
}

impl Default for DamageVarianceConfig {
    fn default() -> Self {
        Self::new()
    }
}

/// PARITY_NOTE: Fabricated — C++ has no range-based damage falloff curves.
/// Kept as empty struct for API compatibility.
#[derive(Debug, Clone)]
pub struct RangeFalloffConfig {}

impl RangeFalloffConfig {
    pub fn new() -> Self {
        Self {}
    }

    pub fn no_falloff() -> Self {
        Self {}
    }
}

impl Default for RangeFalloffConfig {
    fn default() -> Self {
        Self::new()
    }
}

/// PARITY_NOTE: Fabricated — kept as stub.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FalloffCurve {
    Linear,
}

/// PARITY_NOTE: Fabricated — entire system removed.
/// Kept as empty struct for API compatibility.
pub struct DamageModifierSystem {}

impl DamageModifierSystem {
    pub fn new() -> Self {
        Self {}
    }

    pub fn with_defaults() -> Self {
        Self {}
    }
}

impl Default for DamageModifierSystem {
    fn default() -> Self {
        Self::with_defaults()
    }
}

/// PARITY_NOTE: Fabricated — kept as stub.
#[derive(Debug, Clone)]
pub struct DamageModifierResult {
    pub base_damage: f32,
    pub final_damage: f32,
}

impl DamageModifierResult {
    pub fn passthrough(base_damage: f32) -> Self {
        Self {
            base_damage,
            final_damage: base_damage,
        }
    }
}
