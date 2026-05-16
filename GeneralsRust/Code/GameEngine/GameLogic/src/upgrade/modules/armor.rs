//! Armor Upgrade Module
//!
//! Increases unit armor/defense when upgrade is researched.
//! Matches C++ ArmorUpgrade from ArmorUpgrade.h/.cpp
//!
//! Original C++ Author: Chris Brue, July 2002

use super::super::UpgradeMask;
use super::upgrade_mux::{UpgradeModuleInterface, UpgradeMux, UpgradeMuxData};
use crate::common::*;
use crate::object::body::ArmorSetType;
use crate::object::draw::TerrainDecalType;
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::system::{Snapshotable, Xfer, XferVersion};
use game_engine::common::thing::module::{Module, ModuleData, NameKeyType};
use std::sync::Arc;

/// Module data for armor upgrade
#[derive(Debug, Clone)]
pub struct ArmorUpgradeModuleData {
    module_tag_name_key: NameKeyType,
    /// Upgrade mux configuration
    pub upgrade_mux_data: UpgradeMuxData,
    /// Amount to add to armor
    pub armor_bonus: Real,
    /// Whether to multiply armor instead of add
    pub is_multiplier: bool,
}

impl Default for ArmorUpgradeModuleData {
    fn default() -> Self {
        Self {
            module_tag_name_key: 0,
            upgrade_mux_data: UpgradeMuxData::default(),
            armor_bonus: 0.0,
            is_multiplier: false,
        }
    }
}

impl ArmorUpgradeModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, ARMOR_UPGRADE_FIELDS)
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

// INI parsing
fn parse_armor_bonus(
    _ini: &mut INI,
    data: &mut ArmorUpgradeModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let value = tokens
        .iter()
        .skip_while(|t| **t == "=")
        .next()
        .ok_or(INIError::InvalidData)?;
    data.armor_bonus = INI::parse_real(value)?;
    Ok(())
}

fn parse_is_multiplier(
    _ini: &mut INI,
    data: &mut ArmorUpgradeModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let value = tokens
        .iter()
        .skip_while(|t| **t == "=")
        .next()
        .ok_or(INIError::InvalidData)?;
    data.is_multiplier = INI::parse_bool(value)?;
    Ok(())
}

fn parse_triggered_by(
    _ini: &mut INI,
    data: &mut ArmorUpgradeModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    for token in tokens.iter().skip_while(|t| **t == "=") {
        if !token.is_empty() {
            data.upgrade_mux_data
                .activation_upgrade_names
                .push(AsciiString::from(*token));
        }
    }
    Ok(())
}

fn parse_conflicts_with(
    _ini: &mut INI,
    data: &mut ArmorUpgradeModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    for token in tokens.iter().skip_while(|t| **t == "=") {
        if !token.is_empty() {
            data.upgrade_mux_data
                .conflicting_upgrade_names
                .push(AsciiString::from(*token));
        }
    }
    Ok(())
}

fn parse_removes_upgrades(
    _ini: &mut INI,
    data: &mut ArmorUpgradeModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    for token in tokens.iter().skip_while(|t| **t == "=") {
        if !token.is_empty() {
            data.upgrade_mux_data
                .removal_upgrade_names
                .push(AsciiString::from(*token));
        }
    }
    Ok(())
}

fn parse_requires_all_triggers(
    _ini: &mut INI,
    data: &mut ArmorUpgradeModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let value = tokens
        .iter()
        .skip_while(|t| **t == "=")
        .next()
        .ok_or(INIError::InvalidData)?;
    data.upgrade_mux_data.requires_all_triggers = INI::parse_bool(value)?;
    Ok(())
}

const ARMOR_UPGRADE_FIELDS: &[FieldParse<ArmorUpgradeModuleData>] = &[
    FieldParse {
        token: "TriggeredBy",
        parse: parse_triggered_by,
    },
    FieldParse {
        token: "ConflictsWith",
        parse: parse_conflicts_with,
    },
    FieldParse {
        token: "RemovesUpgrades",
        parse: parse_removes_upgrades,
    },
    FieldParse {
        token: "RequiresAllTriggers",
        parse: parse_requires_all_triggers,
    },
    FieldParse {
        token: "ArmorBonus",
        parse: parse_armor_bonus,
    },
    FieldParse {
        token: "IsMultiplier",
        parse: parse_is_multiplier,
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_armor_upgrade_data_default() {
        let data = ArmorUpgradeModuleData::default();
        assert_eq!(data.armor_bonus, 0.0);
        assert!(!data.is_multiplier);
    }

    #[test]
    fn test_armor_upgrade_additive() {
        let mut data = ArmorUpgradeModuleData::default();
        data.armor_bonus = 5.0;
        data.is_multiplier = false;

        let data = Arc::new(data);
        let mut upgrade = ArmorUpgrade::new(1, data, 100);

        let mut obj = Object::new_test(100, 100.0);
        upgrade.upgrade_implementation(&mut obj);
        assert!(!upgrade.is_already_upgraded());
    }

    #[test]
    fn test_armor_upgrade_multiplier() {
        let mut data = ArmorUpgradeModuleData::default();
        data.armor_bonus = 0.5; // 50% increase
        data.is_multiplier = true;

        let data = Arc::new(data);
        let mut upgrade = ArmorUpgrade::new(1, data, 100);

        let mut obj = Object::new_test(100, 100.0);
        upgrade.upgrade_implementation(&mut obj);
        assert!(!upgrade.is_already_upgraded());
    }
}
