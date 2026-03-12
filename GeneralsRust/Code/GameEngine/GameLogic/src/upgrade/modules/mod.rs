//! Upgrade Modules
//!
//! Object modules that respond to player upgrades by modifying object stats.
//! Matches C++ UpgradeModule.h and various specific upgrade modules.
//!
//! Original C++ Authors: Graham Smallwood, Chris Brue, Chris Huybregts, March-July 2002

pub mod armor;
pub mod cost_modifier;
pub mod experience_scalar;
pub mod locomotor_set;
pub mod max_health;
pub mod model_condition;
pub mod object_creation;
pub mod radar;
pub mod status_bits;
pub mod stealth;
pub mod subobjects;
pub mod upgrade_mux;
pub mod weapon_bonus;
pub mod weapon_set;

// Re-export main types
pub use armor::ArmorUpgrade;
pub use cost_modifier::CostModifierUpgrade;
pub use experience_scalar::ExperienceScalarUpgrade;
pub use locomotor_set::LocomotorSetUpgrade;
pub use max_health::MaxHealthUpgrade;
pub use model_condition::ModelConditionUpgrade;
pub use object_creation::ObjectCreationUpgrade;
pub use radar::RadarUpgrade;
pub use status_bits::StatusBitsUpgrade;
pub use stealth::StealthUpgrade;
pub use subobjects::SubObjectsUpgrade;
pub use upgrade_mux::{UpgradeModuleInterface, UpgradeMux, UpgradeMuxData};
pub use weapon_bonus::WeaponBonusUpgrade;
pub use weapon_set::WeaponSetUpgrade;
