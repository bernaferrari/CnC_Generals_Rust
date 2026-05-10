//! Special Powers Module
//!
//! Concrete special power behavior modules that extend the base SpecialPowerModule.
//! Each special power overrides specific doSpecialPower methods to implement its
//! unique behavior, following the C++ class hierarchy.
//!
//! C++ base class: SpecialPowerModule
//! C++ concrete classes:
//!   - OCLSpecialPower: Creates objects from ObjectCreationList at target location
//!   - FireWeaponPower: Reloads ammo and fires weapons at target
//!   - DemoralizeSpecialPower: Slows nearby enemy infantry (GLA ability)
//!   - CashHackSpecialPower: Steals money from enemy buildings (Black Lotus)
//!   - CleanupAreaPower: Cleans mines/debris in area (Ambulance)
//!   - CashBountyPower: Sets cash bounty percentage
//!   - DefectorSpecialPower: Converts enemy units to caster's team
//!   - SpyVisionSpecialPower: Reveals shrouded areas
//!   - BaikonurLaunchPower: Baikonur launch special power
//!   - SpecialAbility: Generic special ability

pub mod baikonur_launch_power;
pub mod cash_bounty_power;
pub mod cash_hack_special_power;
pub mod cleanup_area_power;
pub mod defector_special_power;
pub mod demoralize_special_power;
pub mod fire_weapon_power;
pub mod ocl_special_power;
pub mod special_ability;
pub mod spy_vision_special_power;

pub use baikonur_launch_power::{BaikonurLaunchPower, BaikonurLaunchPowerModuleData};
pub use cash_bounty_power::{CashBountyPower, CashBountyPowerModuleData};
pub use cash_hack_special_power::{CashHackSpecialPower, CashHackSpecialPowerModuleData};
pub use cleanup_area_power::{CleanupAreaPower, CleanupAreaPowerModuleData};
pub use defector_special_power::{DefectorSpecialPower, DefectorSpecialPowerModuleData};
pub use demoralize_special_power::{DemoralizeSpecialPower, DemoralizeSpecialPowerModuleData};
pub use fire_weapon_power::{FireWeaponPower, FireWeaponPowerModuleData};
pub use ocl_special_power::{OclSpecialPower, OclSpecialPowerModuleData};
pub use special_ability::{SpecialAbility, SpecialAbilityModuleData};
pub use spy_vision_special_power::{SpyVisionSpecialPower, SpyVisionSpecialPowerModuleData};
