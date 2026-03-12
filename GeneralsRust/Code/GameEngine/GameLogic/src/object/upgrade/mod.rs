pub mod active_shroud_upgrade;
pub mod armor_upgrade;
pub mod command_set_upgrade;
pub mod cost_modifier_upgrade;
pub mod experience_scalar_upgrade;
pub mod grant_science_upgrade;
pub mod locomotor_set_upgrade;
pub mod max_health_upgrade;
pub mod model_condition_upgrade;
pub mod object_creation_upgrade;
pub mod passengers_fire_upgrade;
pub mod power_plant_upgrade;
pub mod radar_upgrade;
pub mod replace_object_upgrade;
pub mod status_bits_upgrade;
pub mod stealth_upgrade;
pub mod sub_objects_upgrade;
pub mod subobjects_upgrade;
pub mod unpause_special_power_upgrade;
pub mod upgrade_module;
pub mod weapon_bonus_upgrade;
pub mod weapon_set_upgrade;

// Re-export commonly used types
pub use active_shroud_upgrade::{ActiveShroudUpgrade, ActiveShroudUpgradeModuleData};
pub use armor_upgrade::{ArmorUpgrade, ArmorUpgradeModuleData};
pub use command_set_upgrade::{CommandSetUpgrade, CommandSetUpgradeModuleData};
pub use cost_modifier_upgrade::{CostModifierUpgrade, CostModifierUpgradeModuleData};
pub use experience_scalar_upgrade::{ExperienceScalarUpgrade, ExperienceScalarUpgradeModuleData};
pub use grant_science_upgrade::{GrantScienceUpgrade, GrantScienceUpgradeModuleData};
pub use locomotor_set_upgrade::{LocomotorSetUpgrade, LocomotorSetUpgradeModuleData};
pub use max_health_upgrade::{MaxHealthUpgrade, MaxHealthUpgradeModuleData};
pub use model_condition_upgrade::{ModelConditionUpgrade, ModelConditionUpgradeModuleData};
pub use object_creation_upgrade::{ObjectCreationUpgrade, ObjectCreationUpgradeModuleData};
pub use passengers_fire_upgrade::{PassengersFireUpgrade, PassengersFireUpgradeModuleData};
pub use power_plant_upgrade::{PowerPlantUpgrade, PowerPlantUpgradeModuleData};
pub use radar_upgrade::{RadarUpgrade, RadarUpgradeModuleData};
pub use replace_object_upgrade::{ReplaceObjectUpgrade, ReplaceObjectUpgradeModuleData};
pub use status_bits_upgrade::{StatusBitsUpgrade, StatusBitsUpgradeModuleData};
pub use stealth_upgrade::{StealthUpgrade, StealthUpgradeModuleData};
pub use sub_objects_upgrade::{
    SubObjectsUpgrade as SubObjectsUpgradeAlias,
    SubObjectsUpgradeModuleData as SubObjectsUpgradeModuleDataAlias,
};
pub use subobjects_upgrade::{SubObjectsUpgrade, SubObjectsUpgradeModuleData};
pub use unpause_special_power_upgrade::{
    UnpauseSpecialPowerUpgrade, UnpauseSpecialPowerUpgradeModuleData,
};
pub use upgrade_module::{UpgradeModuleInterface, UpgradeMux, UpgradeMuxData};
pub use weapon_bonus_upgrade::{WeaponBonusUpgrade, WeaponBonusUpgradeModuleData};
pub use weapon_set_upgrade::{WeaponSetUpgrade, WeaponSetUpgradeModuleData};
