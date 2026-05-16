// StatusBitsUpgrade - Sets object status bits
use super::super::UpgradeMask;
use super::upgrade_mux::{UpgradeModuleInterface, UpgradeMux, UpgradeMuxData};
use crate::common::*;
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::system::{Snapshotable, Xfer, XferVersion};
use game_engine::common::thing::module::{Module, ModuleData, NameKeyType};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct StatusBitsUpgradeModuleData {
    module_tag_name_key: NameKeyType,
    pub upgrade_mux_data: UpgradeMuxData,
    pub status_to_set: ObjectStatusMaskType,
    pub status_to_clear: ObjectStatusMaskType,
}

impl Default for StatusBitsUpgradeModuleData {
    fn default() -> Self {
        Self {
            module_tag_name_key: 0,
            upgrade_mux_data: UpgradeMuxData::default(),
            status_to_set: ObjectStatusMaskType::none(),
            status_to_clear: ObjectStatusMaskType::none(),
        }
    }
}

impl StatusBitsUpgradeModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, STATUS_BITS_UPGRADE_FIELDS)
    }
}

impl ModuleData for StatusBitsUpgradeModuleData {
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

impl Snapshotable for StatusBitsUpgradeModuleData {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 0;
        xfer.xfer_version(&mut version, 1).map_err(|e| e.to_string())?;
        Ok(())
    }
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 0;
        xfer.xfer_version(&mut version, 1).map_err(|e| e.to_string())?;
        Ok(())
    }
    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

fn parse_triggered_by(
    _ini: &mut INI,
    data: &mut StatusBitsUpgradeModuleData,
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
    data: &mut StatusBitsUpgradeModuleData,
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
    data: &mut StatusBitsUpgradeModuleData,
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
    data: &mut StatusBitsUpgradeModuleData,
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

fn parse_status_to_set(
    _ini: &mut INI,
    data: &mut StatusBitsUpgradeModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.status_to_set = ObjectStatusMaskType::parse_tokens(tokens.iter().copied())
        .map_err(|_| INIError::InvalidData)?;
    Ok(())
}

fn parse_status_to_clear(
    _ini: &mut INI,
    data: &mut StatusBitsUpgradeModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.status_to_clear = ObjectStatusMaskType::parse_tokens(tokens.iter().copied())
        .map_err(|_| INIError::InvalidData)?;
    Ok(())
}

const STATUS_BITS_UPGRADE_FIELDS: &[FieldParse<StatusBitsUpgradeModuleData>] = &[
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
        token: "StatusToSet",
        parse: parse_status_to_set,
    },
    FieldParse {
        token: "StatusToClear",
        parse: parse_status_to_clear,
    },
];

pub struct StatusBitsUpgrade {
    module_name_key: NameKeyType,
    data: Arc<StatusBitsUpgradeModuleData>,
    mux: UpgradeMux,
}

impl StatusBitsUpgrade {
    pub fn new(
        module_name_key: NameKeyType,
        data: Arc<StatusBitsUpgradeModuleData>,
        _object_id: ObjectID,
    ) -> Self {
        let mux = UpgradeMux::new(data.upgrade_mux_data.clone());
        Self {
            module_name_key,
            data,
            mux,
        }
    }

    fn upgrade_implementation(&mut self, object: &mut Object) {
        if self.data.status_to_set.any() {
            object.set_status(self.data.status_to_set, true);
        }
        if self.data.status_to_clear.any() {
            object.set_status(self.data.status_to_clear, false);
        }
    }
}

impl UpgradeModuleInterface for StatusBitsUpgrade {
    fn is_already_upgraded(&self) -> bool {
        self.mux.is_already_upgraded()
    }
    fn attempt_upgrade(&mut self, key_mask: UpgradeMask, object: &mut Object) -> bool {
        if self.mux.would_upgrade(key_mask) {
            self.mux.data.perform_upgrade_fx(object);
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
    fn force_refresh_upgrade(&mut self, _object: &mut Object) {}
}

impl Module for StatusBitsUpgrade {
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

impl Snapshotable for StatusBitsUpgrade {
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
