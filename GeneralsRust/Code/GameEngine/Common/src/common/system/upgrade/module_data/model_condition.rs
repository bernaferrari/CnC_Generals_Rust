//! Authored ModelConditionUpgrade module data and its INI fields.

use super::{
    FieldParse, INI, INIError, ModuleData, NameKeyType, Snapshotable, UpgradeMuxData, Xfer,
};
use crate::common::model_condition::ModelConditionFlags;
use std::any::Any;

/// Module data describing the model condition upgrade.
#[derive(Debug, Clone)]
pub struct ModelConditionUpgradeModuleData {
    module_tag_name_key: NameKeyType,
    pub upgrade_mux_data: UpgradeMuxData,
    condition_flag: ModelConditionFlags,
}

impl Default for ModelConditionUpgradeModuleData {
    fn default() -> Self {
        Self {
            module_tag_name_key: 0,
            upgrade_mux_data: UpgradeMuxData::default(),
            condition_flag: ModelConditionFlags::empty(),
        }
    }
}

impl ModelConditionUpgradeModuleData {
    pub fn condition_flag(&self) -> ModelConditionFlags {
        self.condition_flag
    }

    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, MODEL_CONDITION_UPGRADE_FIELDS)
    }
}

impl_definition!(ModelConditionUpgradeModuleData);

fn parse_model_condition_flag(token: &str) -> Option<ModelConditionFlags> {
    let upper = token.trim().to_ascii_uppercase();
    // Compact / alias spellings used in some INI blocks. C++ parseSingleBitFromINI
    // uses BitFlags.cpp s_bitNameList; aliases are extra.
    let canonical = match upper.as_str() {
        "FIRING_PRIMARY" => "FIRING_A",
        "FIRING_SECONDARY" => "FIRING_B",
        "FIRING_TERTIARY" => "FIRING_C",
        "REALLY_DAMAGED" => "REALLYDAMAGED",
        "POWERPLANTUPGRADING" => "POWER_PLANT_UPGRADING",
        "POWERPLANTUPGRADED" => "POWER_PLANT_UPGRADED",
        "DOOR1OPENING" => "DOOR_1_OPENING",
        "DOOR1WAITINGOPEN" => "DOOR_1_WAITING_OPEN",
        "DOOR1CLOSING" => "DOOR_1_CLOSING",
        "DOOR1WAITINGTOCLOSE" => "DOOR_1_WAITING_TO_CLOSE",
        "DOOR2OPENING" => "DOOR_2_OPENING",
        "DOOR2WAITINGOPEN" => "DOOR_2_WAITING_OPEN",
        "DOOR2CLOSING" => "DOOR_2_CLOSING",
        "DOOR2WAITINGTOCLOSE" => "DOOR_2_WAITING_TO_CLOSE",
        "DOOR3OPENING" => "DOOR_3_OPENING",
        "DOOR3WAITINGOPEN" => "DOOR_3_WAITING_OPEN",
        "DOOR3CLOSING" => "DOOR_3_CLOSING",
        "DOOR3WAITINGTOCLOSE" => "DOOR_3_WAITING_TO_CLOSE",
        "DOOR4OPENING" => "DOOR_4_OPENING",
        "DOOR4WAITINGOPEN" => "DOOR_4_WAITING_OPEN",
        "DOOR4CLOSING" => "DOOR_4_CLOSING",
        "DOOR4WAITINGTOCLOSE" => "DOOR_4_WAITING_TO_CLOSE",
        "CENTERTORIGHT" => "CENTER_TO_RIGHT",
        "CENTERTOLEFT" => "CENTER_TO_LEFT",
        "RIGHTTOCENTER" => "RIGHT_TO_CENTER",
        "LEFTTOCENTER" => "LEFT_TO_CENTER",
        other => other,
    };

    if let Some(flag) = ModelConditionFlags::from_ini_name(canonical) {
        return Some(flag);
    }

    // Additional Rust model state names.
    match canonical {
        "PRISTINE" => Some(ModelConditionFlags::PRISTINE),
        "SELECTED" => Some(ModelConditionFlags::SELECTED),
        "WEAPON_UPGRADED" => Some(ModelConditionFlags::WEAPON_UPGRADED),
        "ARMOR_UPGRADED" => Some(ModelConditionFlags::ARMOR_UPGRADED),
        _ => None,
    }
}

fn parse_condition_flag_field(
    _ini: &mut INI,
    data: &mut ModelConditionUpgradeModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    if tokens.is_empty() {
        return Err(INIError::InvalidData);
    }
    let Some(flag) = parse_model_condition_flag(tokens[0]) else {
        return Err(INIError::InvalidData);
    };
    data.condition_flag = flag;
    Ok(())
}

const MODEL_CONDITION_UPGRADE_FIELDS: &[FieldParse<ModelConditionUpgradeModuleData>] = upgrade_fields!(
    ModelConditionUpgradeModuleData,
    FieldParse {
        token: "ConditionFlag",
        parse: parse_condition_flag_field,
    },
);
