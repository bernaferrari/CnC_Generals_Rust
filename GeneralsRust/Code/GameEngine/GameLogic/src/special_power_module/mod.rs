//! Special Power Module System
//!
//! This module implements the complete special power/superweapon system for C&C Generals Zero Hour.
//! Includes support for various power types including airstrikes, nukes, cash bounties, and more.

// Core system modules
pub mod area_damage; // Area damage application system
pub mod baikonur_launch_power;
pub mod base_power;
pub mod cash_hack_power;
pub mod cash_power;
pub mod cleanup_area_power;
pub mod cooldown;
pub mod defector_power;
pub mod demoralize_power;
pub mod fire_weapon_power;
pub mod integration; // Integration layer with game engine systems
pub mod ocl_power;
pub mod owner_resolve;
pub mod player_money; // Player money/resource system
pub mod player_science; // Player science/rank system
pub mod registry;
pub mod spy_vision_power;
pub mod targeting;
pub mod types;

// USA special powers
pub mod a10_strike_power;
pub mod aurora_strike_power;
pub mod fuel_air_bomb_power;
pub mod paradrop_power;
pub mod spectre_gunship_power;

// China special powers
pub mod artillery_barrage_power;
pub mod carpet_bomb_power;
pub mod emp_pulse_power;
pub mod nuclear_missile_power;

// GLA special powers
pub mod anthrax_bomb_power;
pub mod gps_scrambler_power;
pub mod rebel_ambush_power;
pub mod sneak_attack_power;

// General powers
pub mod emergency_repair_power;

// Test module for improvements
#[cfg(test)]
mod test_improvements;

// Re-export key types and traits
pub use area_damage::{
    AreaDamageApplicator, AreaDamageConfig, DamageFalloff, DamageResult, NuclearDamageHelper,
};
pub use base_power::{SpecialPowerModule, SpecialPowerModuleData, SpecialPowerModuleInterface};
pub use cooldown::{CooldownManager, CooldownState};
pub use owner_resolve::{resolve_special_power_owner, resolve_special_power_owner_id};
pub use player_money::{
    get_player_money_manager, initialize_player_money, PlayerMoney, PlayerMoneyManager,
};
pub use player_science::{
    get_player_science_manager, initialize_player_science, PlayerRank, PlayerScience,
    PlayerScienceManager,
};
pub use registry::{
    get_player_powers, get_power, get_power_registry, register_power, SpecialPowerRegistry,
};
pub use targeting::{TargetValidator, TargetingInfo};
pub use types::*;

// Re-export specific power implementations (existing)
pub use baikonur_launch_power::{BaikonurLaunchPower, BaikonurLaunchPowerData};
pub use cash_hack_power::{CashHackSpecialPower, CashHackSpecialPowerData};
pub use cash_power::{CashBountyPower, CashBountyPowerData};
pub use cleanup_area_power::{CleanupAreaPower, CleanupAreaPowerData};
pub use defector_power::{DefectorSpecialPower, DefectorSpecialPowerData};
pub use demoralize_power::{DemoralizeSpecialPower, DemoralizeSpecialPowerData};
pub use fire_weapon_power::{FireWeaponPower, FireWeaponPowerData};
pub use ocl_power::{OCLSpecialPower, OCLSpecialPowerData};
pub use spy_vision_power::{SpyVisionSpecialPower, SpyVisionSpecialPowerData};

// Re-export USA special powers
pub use a10_strike_power::{A10StrikePower, A10StrikePowerData};
pub use aurora_strike_power::{AuroraStrikePower, AuroraStrikePowerData};
pub use fuel_air_bomb_power::{FuelAirBombPower, FuelAirBombPowerData};
pub use paradrop_power::{ParadropPower, ParadropPowerData};
pub use spectre_gunship_power::{SpectreGunshipPower, SpectreGunshipPowerData};

// Re-export China special powers
pub use artillery_barrage_power::{ArtilleryBarragePower, ArtilleryBarragePowerData};
pub use carpet_bomb_power::{CarpetBombPower, CarpetBombPowerData};
pub use emp_pulse_power::{EmpPulsePower, EmpPulsePowerData};
pub use nuclear_missile_power::{NuclearMissilePower, NuclearMissilePowerData};

// Re-export GLA special powers
pub use anthrax_bomb_power::{AnthraxBombPower, AnthraxBombPowerData};
pub use gps_scrambler_power::{GpsScramblerPower, GpsScramblerPowerData};
pub use rebel_ambush_power::{RebelAmbushPower, RebelAmbushPowerData};
pub use sneak_attack_power::{SneakAttackPower, SneakAttackPowerData};

// Re-export General powers
pub use emergency_repair_power::{EmergencyRepairPower, EmergencyRepairPowerData};

/// Initialize the special power system
pub fn initialize() {
    registry::initialize_power_registry();
    player_money::initialize_player_money();
    player_science::initialize_player_science();
    integration::initialize_integration_context();
    // Ensure the integration context can resolve simulation frame timing immediately.
    integration::set_game_logic(std::sync::Arc::new(std::sync::RwLock::new(
        integration::TheGameLogicBridge,
    )));
    integration::set_partition_manager(std::sync::Arc::new(std::sync::RwLock::new(
        crate::helpers::ThePartitionManagerBridge::default(),
    )));
}

/// Update all active special powers
pub fn update() {
    let current_frame = crate::helpers::TheGameLogic::get_frame();
    area_damage::update_damage_over_time(current_frame);
    if let Some(registry) = get_power_registry() {
        let mut reg = registry.write().unwrap();
        reg.update();
    }
}
