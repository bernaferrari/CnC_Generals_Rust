//! Stealth Upgrade Module
//!
//! Grants stealth capability to units when upgrade is researched.
//! Sets OBJECT_STATUS_CAN_STEALTH status on the object.
//! Matches C++ StealthUpgrade from StealthUpgrade.h/.cpp
//!
//! Original C++ Author: Kris Morness, May 2002

use super::super::UpgradeMask;
use super::upgrade_mux::{UpgradeModuleInterface, UpgradeMux, UpgradeMuxData};
use crate::common::*;
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{Module, ModuleData, NameKeyType};
use std::sync::Arc;

/// Module data for stealth upgrade
/// Matches C++ StealthUpgrade (no specific module data, uses base UpgradeModuleData)
#[derive(Debug, Clone, Default)]
pub struct StealthUpgradeModuleData {
    module_tag_name_key: NameKeyType,
    pub upgrade_mux_data: UpgradeMuxData,
}

impl StealthUpgradeModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, STEALTH_UPGRADE_FIELDS)
    }
}

impl ModuleData for StealthUpgradeModuleData {
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

impl Snapshotable for StealthUpgradeModuleData {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

fn parse_triggered_by(
    _ini: &mut INI,
    data: &mut StealthUpgradeModuleData,
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
    data: &mut StealthUpgradeModuleData,
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
    data: &mut StealthUpgradeModuleData,
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
    data: &mut StealthUpgradeModuleData,
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

const STEALTH_UPGRADE_FIELDS: &[FieldParse<StealthUpgradeModuleData>] = &[
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
];

/// Stealth upgrade module
/// Matches C++ StealthUpgrade from StealthUpgrade.cpp
pub struct StealthUpgrade {
    module_name_key: NameKeyType,
    data: Arc<StealthUpgradeModuleData>,
    object_id: ObjectID,
    mux: UpgradeMux,
}

impl StealthUpgrade {
    pub fn new(
        module_name_key: NameKeyType,
        data: Arc<StealthUpgradeModuleData>,
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

    /// Apply the stealth upgrade
    /// Matches C++ StealthUpgrade::upgradeImplementation (lines 27-42)
    fn upgrade_implementation(&mut self, object: &mut Object) {
        // The logic that does the stealthupdate will notice this and start stealthing
        // Matches C++ StealthUpgrade.cpp line 31: me->setStatus( MAKE_OBJECT_STATUS_MASK( OBJECT_STATUS_CAN_STEALTH ) );
        object.set_status(
            crate::common::types::ObjectStatusMaskType::CAN_STEALTH,
            true,
        );

        // Grant stealth to spawns if applicable
        // Matches C++ StealthUpgrade.cpp lines 34-41
        // C++: if( me->isKindOf( KINDOF_SPAWNS_ARE_THE_WEAPONS ) )

        if object.is_kind_of(crate::common::KindOf::SpawnsAreTheWeapons) {
            let _ = object.with_spawn_behavior_full_interface(|spawn_behavior| {
                let _ = spawn_behavior.give_slaves_stealth_upgrade(true);
            });
        }
    }
}

impl UpgradeModuleInterface for StealthUpgrade {
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

impl Module for StealthUpgrade {
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

impl Snapshotable for StealthUpgrade {
    /// CRC for save game validation
    /// Matches C++ StealthUpgrade::crc
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.mux.crc(xfer)
    }

    /// Serialize/deserialize
    /// Matches C++ StealthUpgrade::xfer (version 1)
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version = 1u8;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| e.to_string())?;
        self.mux.xfer(xfer)
    }

    /// Post-load processing
    /// Matches C++ StealthUpgrade::loadPostProcess
    fn load_post_process(&mut self) -> Result<(), String> {
        self.mux.load_post_process()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stealth_upgrade_data_default() {
        let data = StealthUpgradeModuleData::default();
        assert_eq!(data.module_tag_name_key, 0);
    }

    #[test]
    fn test_stealth_upgrade_basic() {
        let data = Arc::new(StealthUpgradeModuleData::default());
        let upgrade = StealthUpgrade::new(1, data, 100);

        assert!(!upgrade.is_already_upgraded());
    }

    #[test]
    fn test_stealth_upgrade_execution() {
        let mut data = StealthUpgradeModuleData::default();
        let mut mask_data = UpgradeMuxData::default();
        mask_data
            .activation_upgrade_names
            .push(AsciiString::from("Upgrade_Stealth"));
        data.upgrade_mux_data = mask_data;

        let data = Arc::new(data);
        let mut upgrade = StealthUpgrade::new(1, data, 100);

        let upgrade_mask = crate::upgrade::upgrade_mask_for_name("Upgrade_Stealth");

        let mut obj = Object::new_test(100, 100.0);

        // Should trigger upgrade
        assert!(upgrade.would_upgrade(upgrade_mask));
        assert!(upgrade.attempt_upgrade(upgrade_mask, &mut obj));
        assert!(upgrade.is_already_upgraded());
    }

    #[test]
    fn test_stealth_upgrade_no_double_apply() {
        let mut data = StealthUpgradeModuleData::default();
        let mut mask_data = UpgradeMuxData::default();
        mask_data
            .activation_upgrade_names
            .push(AsciiString::from("Upgrade_Stealth"));
        data.upgrade_mux_data = mask_data;

        let data = Arc::new(data);
        let mut upgrade = StealthUpgrade::new(1, data, 100);

        let upgrade_mask = crate::upgrade::upgrade_mask_for_name("Upgrade_Stealth");

        let mut obj = Object::new_test(100, 100.0);

        // First attempt should succeed
        assert!(upgrade.attempt_upgrade(upgrade_mask, &mut obj));
        assert!(upgrade.is_already_upgraded());

        // Second attempt should fail (already upgraded)
        assert!(!upgrade.would_upgrade(upgrade_mask));
        assert!(!upgrade.attempt_upgrade(upgrade_mask, &mut obj));
    }

    #[test]
    fn test_stealth_upgrade_reset() {
        let mut data = StealthUpgradeModuleData::default();
        let mut mask_data = UpgradeMuxData::default();
        mask_data
            .activation_upgrade_names
            .push(AsciiString::from("Upgrade_Stealth"));
        data.upgrade_mux_data = mask_data;

        let data = Arc::new(data);
        let mut upgrade = StealthUpgrade::new(1, data, 100);

        let upgrade_mask = crate::upgrade::upgrade_mask_for_name("Upgrade_Stealth");

        let mut obj = Object::new_test(100, 100.0);

        // Apply upgrade
        upgrade.attempt_upgrade(upgrade_mask, &mut obj);
        assert!(upgrade.is_already_upgraded());

        // Reset upgrade
        assert!(upgrade.reset_upgrade(upgrade_mask));
        assert!(!upgrade.is_already_upgraded());

        // Can apply again
        assert!(upgrade.would_upgrade(upgrade_mask));
    }
}
