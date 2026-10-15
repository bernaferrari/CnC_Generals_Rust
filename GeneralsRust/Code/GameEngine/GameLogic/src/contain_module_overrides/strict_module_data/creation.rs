//! Strict helpers module-data registrations.
use super::*;

pub(super) fn register(factory: &mut impl DataParserRegistrar) {
    register_parsed!(
        factory,
        Behavior,
        "GrantUpgradeCreate",
        GrantUpgradeCreateModuleData,
        default
    );
    register_parsed!(
        factory,
        Behavior,
        "LockWeaponCreate",
        LockWeaponCreateModuleData,
        default
    );
    register_empty!(
        factory,
        Behavior,
        "PreorderCreate",
        CreateModuleData,
        default
    );
    register_empty!(
        factory,
        Behavior,
        "SpecialPowerCreate",
        CreateModuleData,
        default
    );
    register_empty!(
        factory,
        Behavior,
        "SupplyCenterCreate",
        CreateModuleData,
        default
    );
    register_empty!(
        factory,
        Behavior,
        "SupplyWarehouseCreate",
        CreateModuleData,
        default
    );
    register_parsed!(
        factory,
        Behavior,
        "VeterancyGainCreate",
        VeterancyGainCreateModuleData,
        default
    );
}

#[cfg(test)]
pub(super) const NAMES: &[&str] = &[
    "GrantUpgradeCreate",
    "LockWeaponCreate",
    "PreorderCreate",
    "SpecialPowerCreate",
    "SupplyCenterCreate",
    "SupplyWarehouseCreate",
    "VeterancyGainCreate",
];
