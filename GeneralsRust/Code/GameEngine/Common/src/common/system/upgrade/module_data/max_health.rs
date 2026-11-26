//! Authored maximum-health change and how it affects current health.

use super::{
    FieldParse, INI, INIError, ModuleData, NameKeyType, Snapshotable, UpgradeMuxData, Xfer,
};
use std::any::Any;

/// How to handle max health changes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaxHealthChangeType {
    /// Keep current health the same
    SameCurrentHealth,
    /// Preserve the health ratio
    PreserveRatio,
    /// Add the health difference to current health too
    AddCurrentHealthToo,
    /// Fully heal to new max
    FullyHeal,
}

impl Default for MaxHealthChangeType {
    fn default() -> Self {
        MaxHealthChangeType::SameCurrentHealth
    }
}

/// Module data describing the max health increase to apply.
#[derive(Debug, Clone)]
pub struct MaxHealthUpgradeModuleData {
    module_tag_name_key: NameKeyType,
    pub upgrade_mux_data: UpgradeMuxData,
    add_max_health: f32,
    change_type: MaxHealthChangeType,
}

impl Default for MaxHealthUpgradeModuleData {
    fn default() -> Self {
        Self {
            module_tag_name_key: 0,
            upgrade_mux_data: UpgradeMuxData::default(),
            add_max_health: 0.0,
            change_type: MaxHealthChangeType::SameCurrentHealth,
        }
    }
}

impl MaxHealthUpgradeModuleData {
    pub fn add_max_health(&self) -> f32 {
        self.add_max_health
    }

    pub fn change_type(&self) -> MaxHealthChangeType {
        self.change_type
    }

    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, MAX_HEALTH_UPGRADE_FIELDS)
    }

    pub fn set_add_max_health(&mut self, value: f32) {
        self.add_max_health = value;
    }

    pub fn set_change_type(&mut self, type_str: &str) -> Result<(), String> {
        self.change_type = match type_str.to_uppercase().as_str() {
            "SAME_CURRENTHEALTH" => MaxHealthChangeType::SameCurrentHealth,
            "PRESERVE_RATIO" => MaxHealthChangeType::PreserveRatio,
            "ADD_CURRENT_HEALTH_TOO" => MaxHealthChangeType::AddCurrentHealthToo,
            "FULLY_HEAL" => MaxHealthChangeType::FullyHeal,
            _ => return Err(format!("Unknown change type: {}", type_str)),
        };
        Ok(())
    }
}

impl_definition!(MaxHealthUpgradeModuleData);

fn parse_add_max_health_field(
    _ini: &mut INI,
    data: &mut MaxHealthUpgradeModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    if tokens.is_empty() {
        return Err(INIError::InvalidData);
    }
    data.add_max_health = tokens[0]
        .parse::<f32>()
        .map_err(|_| INIError::InvalidData)?;
    Ok(())
}

fn parse_change_type_field(
    _ini: &mut INI,
    data: &mut MaxHealthUpgradeModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    if tokens.is_empty() {
        return Err(INIError::InvalidData);
    }
    data.set_change_type(tokens[0])
        .map_err(|_| INIError::InvalidData)
}

const MAX_HEALTH_UPGRADE_FIELDS: &[FieldParse<MaxHealthUpgradeModuleData>] = upgrade_fields!(
    MaxHealthUpgradeModuleData,
    FieldParse {
        token: "AddMaxHealth",
        parse: parse_add_max_health_field,
    },
    FieldParse {
        token: "ChangeType",
        parse: parse_change_type_field,
    },
);
