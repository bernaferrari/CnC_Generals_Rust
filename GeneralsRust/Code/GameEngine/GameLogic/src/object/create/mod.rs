//! Object creation modules (`CreateModuleClass` in the C++ source).
//!
//! The C++ tree exposes `Object/Create` as a dedicated namespace that owns the
//! base create-module interfaces.  The Rust port now mirrors that layout so the
//! module tree matches the original include paths.

pub mod create_module;
pub mod eva_announce_client_create;
pub mod grant_upgrade_create;
pub mod lock_weapon_create;
pub mod preorder_create;
pub mod special_power_create;
pub mod supply_center_create;
pub mod supply_warehouse_create;
pub mod veterancy_gain_create;

pub use create_module::{create_safe_module, CreateModule, CreateModuleData, SafeCreateModule};
pub use game_engine::common::thing::module::CreateInterface as CreateModuleInterface;

pub use preorder_create::PreorderCreate;
pub type PreorderCreateData = CreateModuleData;

pub use special_power_create::SpecialPowerCreate;
pub type SpecialPowerCreateData = CreateModuleData;

pub use veterancy_gain_create::{VeterancyGainCreate, VeterancyGainCreateModuleData};
pub type VeterancyGainCreateData = VeterancyGainCreateModuleData;

pub use grant_upgrade_create::{GrantUpgradeCreate, GrantUpgradeCreateModuleData};
pub type GrantUpgradeCreateData = GrantUpgradeCreateModuleData;

pub use supply_warehouse_create::SupplyWarehouseCreate;
pub type SupplyWarehouseCreateData = CreateModuleData;

pub use supply_center_create::SupplyCenterCreate;
pub type SupplyCenterCreateData = CreateModuleData;

pub use lock_weapon_create::{LockWeaponCreate, LockWeaponCreateModuleData};
pub type LockWeaponCreateData = LockWeaponCreateModuleData;

pub use eva_announce_client_create::{EvaAnnounceClientCreate, EvaAnnounceClientCreateData};
