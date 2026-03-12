//! Special Power System
//!
//! Complete implementation of special powers and superweapons.

// Re-export key types for backward compatibility.
pub use crate::special_power_module::{
    get_player_powers, get_power, get_power_registry, register_power, ActivationResult,
    ActivationState, BaikonurLaunchPower, CashBountyPower, CashHackSpecialPower, CleanupAreaPower,
    CooldownManager, CooldownState, DefectorSpecialPower, DemoralizeSpecialPower, FireWeaponPower,
    OCLSpecialPower, SpecialPowerFlags, SpecialPowerID, SpecialPowerKind, SpecialPowerModule,
    SpecialPowerModuleData, SpecialPowerModuleInterface, SpecialPowerRegistry, SpecialPowerStats,
    SpyVisionSpecialPower, TargetValidator, TargetingInfo,
};

use crate::common::*;

/// Special power type (legacy compatibility)
pub type SpecialPowerType = u32;

/// Special power structure (simplified for compatibility)
#[derive(Debug, Clone)]
pub struct SpecialPower {
    pub id: SpecialPowerType,
    pub name: String,
    pub cooldown: Real,
    pub cost: Int,
}

impl SpecialPower {
    pub fn new(id: SpecialPowerType, name: String, cooldown: Real, cost: Int) -> Self {
        Self {
            id,
            name,
            cooldown,
            cost,
        }
    }
}

/// Special power template (legacy compatibility)
#[derive(Debug, Clone)]
pub struct SpecialPowerTemplate {
    pub name: AsciiString,
    pub cooldown: Real,
    pub cost: Int,
}

impl SpecialPowerTemplate {
    pub fn new(name: AsciiString, cooldown: Real, cost: Int) -> Self {
        Self {
            name,
            cooldown,
            cost,
        }
    }

    pub fn get_name(&self) -> &AsciiString {
        &self.name
    }
}

/// Initialize the special power system
pub fn initialize_special_power_system() {
    crate::special_power_module::initialize();
}

/// Update all special powers (call every frame)
pub fn update_special_powers() {
    crate::special_power_module::update();
}
