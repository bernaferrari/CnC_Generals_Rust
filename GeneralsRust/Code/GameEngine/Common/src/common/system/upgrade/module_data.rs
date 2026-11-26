//! Typed definitions for authored object upgrades.

use super::UpgradeMuxData;
use crate::common::ascii_string::AsciiString;
use crate::common::ini::{FieldParse, INI, INIError};
use crate::common::system::{Snapshotable, Xfer};
use crate::common::thing::module::{ModuleData, NameKeyType, RadarUpgradeConfig};
use std::any::Any;

macro_rules! impl_definition {
    ($data:ty $(, $method:item)* $(,)?) => {
        impl ModuleData for $data {
            fn as_any(&self) -> &dyn Any {
                self
            }

            fn set_module_tag_name_key(&mut self, key: NameKeyType) {
                self.module_tag_name_key = key;
            }

            fn get_module_tag_name_key(&self) -> NameKeyType {
                self.module_tag_name_key
            }

            $($method)*
        }

        impl Snapshotable for $data {
            fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
                let mut version = 0;
                xfer.xfer_version(&mut version, 1)
                    .map_err(|error| error.to_string())
            }

            fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
                let mut version = 0;
                xfer.xfer_version(&mut version, 1)
                    .map_err(|error| error.to_string())
            }

            fn load_post_process(&mut self) -> Result<(), String> {
                Ok(())
            }
        }
    };
}

macro_rules! upgrade_fields {
    ($data:ty, $($extra:expr),* $(,)?) => {
        &[
            FieldParse { token: "TriggeredBy", parse: |_, data: &mut $data, tokens| {
                data.upgrade_mux_data.definition.parse_triggered_by_tokens(tokens)
            }},
            FieldParse { token: "ConflictsWith", parse: |_, data: &mut $data, tokens| {
                data.upgrade_mux_data.definition.parse_conflicts_with_tokens(tokens)
            }},
            FieldParse { token: "RemovesUpgrades", parse: |_, data: &mut $data, tokens| {
                data.upgrade_mux_data.definition.parse_removes_upgrades_tokens(tokens)
            }},
            FieldParse { token: "RequiresAllTriggers", parse: |_, data: &mut $data, tokens| {
                data.upgrade_mux_data.definition.parse_requires_all_triggers_tokens(tokens)
            }},
            FieldParse { token: "FXListUpgrade", parse: |_, data: &mut $data, tokens| {
                data.upgrade_mux_data.definition.parse_fx_list_upgrade_tokens(tokens)
            }},
            $($extra,)*
        ]
    };
}

mod max_health;
pub use max_health::{MaxHealthChangeType, MaxHealthUpgradeModuleData};

/// Module data describing the armor upgrade to apply.
#[derive(Debug, Clone)]
pub struct ArmorUpgradeModuleData {
    module_tag_name_key: NameKeyType,
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
        self.upgrade_mux_data.parse_from_ini(ini)
    }
}

impl_definition!(ArmorUpgradeModuleData);

/// Module data describing the command set upgrade.
#[derive(Debug, Clone)]
pub struct CommandSetUpgradeModuleData {
    module_tag_name_key: NameKeyType,
    pub upgrade_mux_data: UpgradeMuxData,
    command_set_name: AsciiString,
    command_set_alt: AsciiString,
    trigger_alt: AsciiString,
}

impl Default for CommandSetUpgradeModuleData {
    fn default() -> Self {
        Self {
            module_tag_name_key: 0,
            upgrade_mux_data: UpgradeMuxData::default(),
            command_set_name: AsciiString::new(),
            command_set_alt: AsciiString::new(),
            trigger_alt: AsciiString::new(),
        }
    }
}

impl CommandSetUpgradeModuleData {
    pub fn command_set_name(&self) -> &AsciiString {
        &self.command_set_name
    }

    pub fn command_set_alt(&self) -> &AsciiString {
        &self.command_set_alt
    }

    pub fn trigger_alt(&self) -> &AsciiString {
        &self.trigger_alt
    }

    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, COMMAND_SET_UPGRADE_FIELDS)
    }
}

impl_definition!(CommandSetUpgradeModuleData);

fn parse_command_set_field(
    _ini: &mut INI,
    data: &mut CommandSetUpgradeModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    if tokens.is_empty() {
        return Err(INIError::InvalidData);
    }
    data.command_set_name = AsciiString::from(tokens[0]);
    Ok(())
}

fn parse_command_set_alt_field(
    _ini: &mut INI,
    data: &mut CommandSetUpgradeModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    if tokens.is_empty() {
        return Err(INIError::InvalidData);
    }
    data.command_set_alt = AsciiString::from(tokens[0]);
    Ok(())
}

fn parse_trigger_alt_field(
    _ini: &mut INI,
    data: &mut CommandSetUpgradeModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    if tokens.is_empty() {
        return Err(INIError::InvalidData);
    }
    data.trigger_alt = AsciiString::from(tokens[0]);
    Ok(())
}

const COMMAND_SET_UPGRADE_FIELDS: &[FieldParse<CommandSetUpgradeModuleData>] = upgrade_fields!(
    CommandSetUpgradeModuleData,
    FieldParse {
        token: "CommandSet",
        parse: parse_command_set_field,
    },
    FieldParse {
        token: "CommandSetAlt",
        parse: parse_command_set_alt_field,
    },
    FieldParse {
        token: "TriggerAlt",
        parse: parse_trigger_alt_field,
    },
);

/// Module data for SubObjectsUpgrade.
#[derive(Debug, Clone)]
pub struct SubObjectsUpgradeModuleData {
    module_tag_name_key: NameKeyType,
    pub upgrade_mux_data: UpgradeMuxData,
    show_sub_object_names: Vec<AsciiString>,
    hide_sub_object_names: Vec<AsciiString>,
}

impl Default for SubObjectsUpgradeModuleData {
    fn default() -> Self {
        Self {
            module_tag_name_key: 0,
            upgrade_mux_data: UpgradeMuxData::default(),
            show_sub_object_names: Vec::new(),
            hide_sub_object_names: Vec::new(),
        }
    }
}

impl SubObjectsUpgradeModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, SUBOBJECTS_UPGRADE_FIELDS)
    }

    pub fn show_sub_object_names(&self) -> &[AsciiString] {
        &self.show_sub_object_names
    }

    pub fn hide_sub_object_names(&self) -> &[AsciiString] {
        &self.hide_sub_object_names
    }
}

impl_definition!(SubObjectsUpgradeModuleData);

fn parse_show_sub_objects(
    _ini: &mut INI,
    data: &mut SubObjectsUpgradeModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    for token in tokens.iter().skip_while(|t| **t == "=") {
        if !token.is_empty() {
            data.show_sub_object_names.push(AsciiString::from(*token));
        }
    }
    Ok(())
}

fn parse_hide_sub_objects(
    _ini: &mut INI,
    data: &mut SubObjectsUpgradeModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    for token in tokens.iter().skip_while(|t| **t == "=") {
        if !token.is_empty() {
            data.hide_sub_object_names.push(AsciiString::from(*token));
        }
    }
    Ok(())
}

const SUBOBJECTS_UPGRADE_FIELDS: &[FieldParse<SubObjectsUpgradeModuleData>] = upgrade_fields!(
    SubObjectsUpgradeModuleData,
    FieldParse {
        token: "ShowSubObjects",
        parse: parse_show_sub_objects,
    },
    FieldParse {
        token: "HideSubObjects",
        parse: parse_hide_sub_objects,
    },
);

/// Module data describing the power plant upgrade.
#[derive(Debug, Clone)]
pub struct PowerPlantUpgradeModuleData {
    module_tag_name_key: NameKeyType,
    pub upgrade_mux_data: UpgradeMuxData,
}

impl Default for PowerPlantUpgradeModuleData {
    fn default() -> Self {
        Self {
            module_tag_name_key: 0,
            upgrade_mux_data: UpgradeMuxData::default(),
        }
    }
}

impl PowerPlantUpgradeModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        self.upgrade_mux_data.parse_from_ini(ini)
    }
}

impl_definition!(PowerPlantUpgradeModuleData);

/// Module data for PassengersFireUpgrade (no custom fields in C++).
#[derive(Debug, Clone)]
pub struct PassengersFireUpgradeModuleData {
    module_tag_name_key: NameKeyType,
    pub upgrade_mux_data: UpgradeMuxData,
}

impl Default for PassengersFireUpgradeModuleData {
    fn default() -> Self {
        Self {
            module_tag_name_key: 0,
            upgrade_mux_data: UpgradeMuxData::default(),
        }
    }
}

impl PassengersFireUpgradeModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        self.upgrade_mux_data.parse_from_ini(ini)
    }
}

impl_definition!(PassengersFireUpgradeModuleData);

/// Module data describing the radar upgrade.
#[derive(Debug, Clone)]
pub struct RadarUpgradeModuleData {
    module_tag_name_key: NameKeyType,
    pub upgrade_mux_data: UpgradeMuxData,
    is_disable_proof: bool,
}

impl Default for RadarUpgradeModuleData {
    fn default() -> Self {
        Self {
            module_tag_name_key: 0,
            upgrade_mux_data: UpgradeMuxData::default(),
            is_disable_proof: false,
        }
    }
}

impl RadarUpgradeModuleData {
    pub fn is_disable_proof(&self) -> bool {
        self.is_disable_proof
    }

    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, RADAR_UPGRADE_FIELDS)
    }
}

impl_definition!(
    RadarUpgradeModuleData,
    fn get_radar_upgrade_config(&self) -> Option<RadarUpgradeConfig> {
        Some(RadarUpgradeConfig {
            is_disable_proof: self.is_disable_proof,
        })
    },
);

fn parse_disable_proof_field(
    _ini: &mut INI,
    data: &mut RadarUpgradeModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    if tokens.is_empty() {
        return Err(INIError::InvalidData);
    }
    data.is_disable_proof = INI::parse_bool(tokens[0])?;
    Ok(())
}

const RADAR_UPGRADE_FIELDS: &[FieldParse<RadarUpgradeModuleData>] = upgrade_fields!(
    RadarUpgradeModuleData,
    FieldParse {
        token: "DisableProof",
        parse: parse_disable_proof_field,
    },
);

/// Module data describing the replace object upgrade.
#[derive(Debug, Clone)]
pub struct ReplaceObjectUpgradeModuleData {
    module_tag_name_key: NameKeyType,
    pub upgrade_mux_data: UpgradeMuxData,
    replace_object_name: AsciiString,
}

impl Default for ReplaceObjectUpgradeModuleData {
    fn default() -> Self {
        Self {
            module_tag_name_key: 0,
            upgrade_mux_data: UpgradeMuxData::default(),
            replace_object_name: AsciiString::new(),
        }
    }
}

impl ReplaceObjectUpgradeModuleData {
    pub fn replace_object_name(&self) -> &AsciiString {
        &self.replace_object_name
    }

    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, REPLACE_OBJECT_UPGRADE_FIELDS)
    }
}

impl_definition!(ReplaceObjectUpgradeModuleData);

fn parse_replace_object_field(
    _ini: &mut INI,
    data: &mut ReplaceObjectUpgradeModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    if tokens.is_empty() {
        return Err(INIError::InvalidData);
    }
    data.replace_object_name = AsciiString::from(tokens[0]);
    Ok(())
}

const REPLACE_OBJECT_UPGRADE_FIELDS: &[FieldParse<ReplaceObjectUpgradeModuleData>] = upgrade_fields!(
    ReplaceObjectUpgradeModuleData,
    FieldParse {
        token: "ReplaceObject",
        parse: parse_replace_object_field,
    },
);
