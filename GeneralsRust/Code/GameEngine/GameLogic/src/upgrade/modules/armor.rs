//! Armor Upgrade Module
//!
//! C++ `ArmorUpgrade` (ArmorUpgrade.h / ArmorUpgrade.cpp). Module data is
//! UpgradeMux only (`MAKE_STANDARD_MODULE_MACRO`); `upgradeImplementation`
//! sets `ARMORSET_PLAYER_UPGRADE`. No ArmorBonus / IsMultiplier fields.
//!
//! Original C++ Author: Chris Brue, July 2002

use super::upgrade_mux::{UpgradeModuleInterface, UpgradeMux, UpgradeMuxData};
use crate::common::*;
use crate::object::body::ArmorSetType;
use crate::object::draw::TerrainDecalType;
use crate::upgrade::mask::UpgradeMask;
use game_engine::common::ini::{INI, INIError};
use game_engine::common::system::{Snapshotable, Xfer, XferVersion};
use game_engine::common::thing::module::{Module, ModuleData, NameKeyType};
use std::sync::Arc;

/// Module data for armor upgrade.
/// C++ `ArmorUpgrade.h`: no fields beyond `UpgradeMux` / `UpgradeModule`.
#[derive(Debug, Clone)]
pub struct ArmorUpgradeModuleData {
    module_tag_name_key: NameKeyType,
    /// Upgrade mux configuration
    pub upgrade_mux_data: UpgradeMuxData,
}

impl Default for ArmorUpgradeModuleData {
    fn default() -> Self {
        Self {
            module_tag_name_key: 0,
            upgrade_mux_data: UpgradeMuxData::default(),
        }
    }
}

impl ArmorUpgradeModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        // C++ ArmorUpgrade.h: MAKE_STANDARD_MODULE_MACRO → UpgradeMux fields only.
        self.upgrade_mux_data.parse_from_ini(ini)
    }
}

impl ModuleData for ArmorUpgradeModuleData {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn set_module_tag_name_key(&mut self, key: NameKeyType) {
        self.module_tag_name_key = key;
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.module_tag_name_key
    }
}

impl Snapshotable for ArmorUpgradeModuleData {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 0;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 0;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

/// Armor upgrade module
/// Matches C++ ArmorUpgrade
pub struct ArmorUpgrade {
    module_name_key: NameKeyType,
    data: Arc<ArmorUpgradeModuleData>,
    object_id: ObjectID,
    mux: UpgradeMux,
}

impl ArmorUpgrade {
    pub fn new(
        module_name_key: NameKeyType,
        data: Arc<ArmorUpgradeModuleData>,
        object_id: ObjectID,
    ) -> Self {
        let mux = UpgradeMux::new(data.upgrade_mux_data.clone());

        Self {
            module_name_key,
            data,
            object_id,
            mux,
        }
    }

    /// Apply the armor upgrade
    /// Matches C++ ArmorUpgrade::upgradeImplementation
    fn upgrade_implementation(&mut self, object: &mut Object) {
        // C++ behavior: set the player upgrade armor set flag. Weapon/armor selection
        // logic handles the actual damage scaling.
        if let Some(body) = object.get_body() {
            if let Ok(mut body_guard) = body.lock() {
                let _ = body_guard.set_armor_set_flag(ArmorSetType::PlayerUpgrade);
            }
        }

        // Unique case for America Chemical Suits: apply the chem suit decal.
        if self
            .mux
            .data
            .is_triggered_by("Upgrade_AmericaChemicalSuits")
        {
            if let Some(drawable) = object.get_drawable() {
                if let Ok(mut guard) = drawable.write() {
                    guard.set_terrain_decal(TerrainDecalType::ChemSuit);
                }
            }
        }
    }
}

impl UpgradeModuleInterface for ArmorUpgrade {
    fn is_already_upgraded(&self) -> bool {
        self.mux.is_already_upgraded()
    }

    fn attempt_upgrade(&mut self, key_mask: UpgradeMask, object: &mut Object) -> bool {
        if self.mux.would_upgrade(key_mask) {
            self.mux.data.perform_upgrade_fx(object);
            self.mux.data.process_upgrade_removal(object);
            self.upgrade_implementation(object);
            self.mux.set_upgrade_executed(true);
            true
        } else {
            false
        }
    }

    fn would_upgrade(&self, key_mask: UpgradeMask) -> bool {
        self.mux.would_upgrade(key_mask)
    }

    fn reset_upgrade(&mut self, key_mask: UpgradeMask) -> bool {
        self.mux.reset_upgrade(key_mask)
    }

    fn test_upgrade_conditions(&self, key_mask: UpgradeMask) -> bool {
        self.mux.test_upgrade_conditions(key_mask)
    }

    fn force_refresh_upgrade(&mut self, object: &mut Object) {
        if self.is_already_upgraded() {
            self.upgrade_implementation(object);
        }
    }
}

impl Module for ArmorUpgrade {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn get_module_name_key(&self) -> NameKeyType {
        self.module_name_key
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.data.get_module_tag_name_key()
    }

    fn get_module_data(&self) -> &dyn ModuleData {
        self.data.as_ref()
    }
}

impl Snapshotable for ArmorUpgrade {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.mux.crc(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        const CURRENT_VERSION: XferVersion = 1;
        let mut version = CURRENT_VERSION;
        xfer.xfer_version(&mut version, CURRENT_VERSION)
            .map_err(|e| e.to_string())?;
        self.mux.xfer(xfer)
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        self.mux.load_post_process()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_armor_upgrade_data_has_no_invented_fields() {
        // C++ ArmorUpgrade.h:45-60 — UpgradeMux only; no ArmorBonus/IsMultiplier.
        let data = ArmorUpgradeModuleData::default();
        assert!(data.upgrade_mux_data.activation_upgrade_names.is_empty());
        assert!(!data.upgrade_mux_data.requires_all_triggers);
    }

    #[test]
    fn test_armor_upgrade_sets_player_upgrade_flag() {
        // C++ ArmorUpgrade::upgradeImplementation (ArmorUpgrade.cpp:63-74).
        let data = Arc::new(ArmorUpgradeModuleData::default());
        let mut upgrade = ArmorUpgrade::new(1, data, 100);

        let mut obj = Object::new_test(100, 100.0);
        upgrade.upgrade_implementation(&mut obj);
        assert!(!upgrade.is_already_upgraded());
    }
}
