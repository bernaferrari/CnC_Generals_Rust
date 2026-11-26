//! Authored StatusBitsUpgrade module data and its INI fields.

use super::{
    FieldParse, INI, INIError, ModuleData, NameKeyType, Snapshotable, UpgradeMuxData, Xfer,
};
use crate::common::system::object_status_types::ObjectStatusMaskType;
use std::any::Any;

/// Module data describing the status bits to set/clear when an upgrade is applied.
#[derive(Debug, Clone)]
pub struct StatusBitsUpgradeModuleData {
    module_tag_name_key: NameKeyType,
    pub upgrade_mux_data: UpgradeMuxData,
    status_to_set: ObjectStatusMaskType,
    status_to_clear: ObjectStatusMaskType,
}

impl Default for StatusBitsUpgradeModuleData {
    fn default() -> Self {
        Self {
            module_tag_name_key: 0,
            upgrade_mux_data: UpgradeMuxData::default(),
            status_to_set: ObjectStatusMaskType::empty(),
            status_to_clear: ObjectStatusMaskType::empty(),
        }
    }
}

impl StatusBitsUpgradeModuleData {
    pub fn status_to_set(&self) -> ObjectStatusMaskType {
        self.status_to_set
    }

    pub fn status_to_clear(&self) -> ObjectStatusMaskType {
        self.status_to_clear
    }

    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, STATUS_BITS_UPGRADE_FIELDS)
    }

    pub fn set_status_to_set_from_tokens(&mut self, tokens: &[&str]) -> Result<(), String> {
        self.status_to_set = parse_status_tokens(tokens)?;
        Ok(())
    }

    pub fn set_status_to_clear_from_tokens(&mut self, tokens: &[&str]) -> Result<(), String> {
        self.status_to_clear = parse_status_tokens(tokens)?;
        Ok(())
    }
}

impl_definition!(StatusBitsUpgradeModuleData);

fn parse_status_tokens(tokens: &[&str]) -> Result<ObjectStatusMaskType, String> {
    let normalized: Vec<&str> = tokens
        .iter()
        .copied()
        .filter(|token| *token != "=")
        .collect();
    if normalized.len() > 1
        && normalized
            .iter()
            .any(|token| token.eq_ignore_ascii_case("NONE"))
    {
        return Err("mixing NONE with other tokens is invalid".to_string());
    }
    ObjectStatusMaskType::parse_tokens(normalized)
}

fn parse_status_to_set_field(
    _ini: &mut INI,
    data: &mut StatusBitsUpgradeModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.set_status_to_set_from_tokens(tokens)
        .map_err(|_| INIError::InvalidData)
}

fn parse_status_to_clear_field(
    _ini: &mut INI,
    data: &mut StatusBitsUpgradeModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.set_status_to_clear_from_tokens(tokens)
        .map_err(|_| INIError::InvalidData)
}

const STATUS_BITS_UPGRADE_FIELDS: &[FieldParse<StatusBitsUpgradeModuleData>] = upgrade_fields!(
    StatusBitsUpgradeModuleData,
    FieldParse {
        token: "StatusToSet",
        parse: parse_status_to_set_field,
    },
    FieldParse {
        token: "StatusToClear",
        parse: parse_status_to_clear_field,
    },
);
