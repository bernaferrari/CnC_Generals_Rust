// MaxHealthUpgrade - Increases unit max health
use super::super::UpgradeMask;
use super::upgrade_mux::{UpgradeModuleInterface, UpgradeMux, UpgradeMuxData};
use crate::common::*;
use crate::object::body::body_module::MaxHealthChangeType;
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::system::{Snapshotable, Xfer, XferVersion};
use game_engine::common::thing::module::{Module, ModuleData, NameKeyType};
use std::sync::Arc;

#[derive(Debug, Clone, Default)]
pub struct MaxHealthUpgradeModuleData {
    module_tag_name_key: NameKeyType,
    pub upgrade_mux_data: UpgradeMuxData,
    pub health_bonus: Real,
    pub change_type: MaxHealthChangeType,
}

impl ModuleData for MaxHealthUpgradeModuleData {
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

impl MaxHealthUpgradeModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, MAX_HEALTH_UPGRADE_FIELDS)
    }
}

impl Snapshotable for MaxHealthUpgradeModuleData {
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

pub struct MaxHealthUpgrade {
    module_name_key: NameKeyType,
    data: Arc<MaxHealthUpgradeModuleData>,
    object_id: ObjectID,
    mux: UpgradeMux,
}

impl MaxHealthUpgrade {
    pub fn new(
        module_name_key: NameKeyType,
        data: Arc<MaxHealthUpgradeModuleData>,
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

    fn upgrade_implementation(&mut self, object: &mut Object) {
        if let Some(body) = object.get_body() {
            if let Ok(mut body_guard) = body.lock() {
                let current_max = body_guard.get_max_health();
                let new_max = current_max + self.data.health_bonus;
                let _ = body_guard.set_max_health(new_max, self.data.change_type);
                log::info!(
                    "MaxHealthUpgrade: Increased max health from {} to {} for object {}",
                    current_max,
                    new_max,
                    self.object_id
                );
            }
        }
    }
}

impl UpgradeModuleInterface for MaxHealthUpgrade {
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
    fn force_refresh_upgrade(&mut self, object: &mut Object) {
        if self.is_already_upgraded() {
            self.upgrade_implementation(object);
        }
    }
}

impl Module for MaxHealthUpgrade {
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

impl Snapshotable for MaxHealthUpgrade {
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

fn parse_add_max_health(
    _ini: &mut INI,
    data: &mut MaxHealthUpgradeModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let value = tokens
        .iter()
        .skip_while(|t| **t == "=")
        .next()
        .ok_or(INIError::InvalidData)?;
    data.health_bonus = INI::parse_real(value)?;
    Ok(())
}

fn parse_change_type(
    _ini: &mut INI,
    data: &mut MaxHealthUpgradeModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let value = tokens
        .iter()
        .skip_while(|t| **t == "=")
        .next()
        .ok_or(INIError::InvalidData)?;
    data.change_type = match value.to_uppercase().as_str() {
        "SAME_CURRENTHEALTH" => MaxHealthChangeType::SameCurrentHealth,
        "PRESERVE_RATIO" => MaxHealthChangeType::PreserveRatio,
        "ADD_CURRENT_HEALTH_TOO" => MaxHealthChangeType::AddCurrentHealthToo,
        "FULLY_HEAL" => MaxHealthChangeType::FullyHeal,
        _ => return Err(INIError::InvalidData),
    };
    Ok(())
}

fn parse_triggered_by(
    _ini: &mut INI,
    data: &mut MaxHealthUpgradeModuleData,
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
    data: &mut MaxHealthUpgradeModuleData,
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
    data: &mut MaxHealthUpgradeModuleData,
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
    data: &mut MaxHealthUpgradeModuleData,
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

const MAX_HEALTH_UPGRADE_FIELDS: &[FieldParse<MaxHealthUpgradeModuleData>] = &[
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
        token: "AddMaxHealth",
        parse: parse_add_max_health,
    },
    FieldParse {
        token: "ChangeType",
        parse: parse_change_type,
    },
];
