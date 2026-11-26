//! Authored conditions and effects of C++ UpgradeMuxData (UpgradeModule.h).

use super::UpgradeMask;
use crate::common::ascii_string::AsciiString;
use crate::common::ini::{FieldParse, INI, INIError};

/// Names and options carried by an authored upgrade module.
#[derive(Debug, Clone, Default)]
pub struct UpgradeMuxDefinition {
    /// Names retained when the legacy parser materializes the activation mask.
    pub trigger_upgrade_names: Vec<AsciiString>,
    pub activation_upgrade_names: Vec<AsciiString>,
    pub conflicting_upgrade_names: Vec<AsciiString>,
    pub removal_upgrade_names: Vec<AsciiString>,
    pub fx_list_name: Option<AsciiString>,
    pub requires_all_triggers: bool,
}

impl UpgradeMuxDefinition {
    pub fn activation_names(&self) -> impl Iterator<Item = &AsciiString> {
        self.activation_upgrade_names
            .iter()
            .chain(&self.trigger_upgrade_names)
    }

    /// Check if triggered by specific upgrade name
    /// Matches C++ UpgradeMuxData::isTriggeredBy
    pub fn is_triggered_by(&self, upgrade: &str) -> bool {
        self.activation_names()
            .any(|name| name.as_str().eq_ignore_ascii_case(upgrade))
    }

    /// Parse `TriggeredBy` tokens into activation names.
    pub fn parse_triggered_by_tokens(&mut self, tokens: &[&str]) -> Result<(), INIError> {
        // UpgradeModule.h:73-76 uses INI.cpp:649-657's replacing vector
        // parser, including an empty field clearing the previously read list.
        replace_names(&mut self.activation_upgrade_names, tokens);
        Ok(())
    }

    /// Parse `ConflictsWith` tokens into conflicting names.
    pub fn parse_conflicts_with_tokens(&mut self, tokens: &[&str]) -> Result<(), INIError> {
        replace_names(&mut self.conflicting_upgrade_names, tokens);
        Ok(())
    }

    /// Parse `RemovesUpgrades` tokens into removal names.
    pub fn parse_removes_upgrades_tokens(&mut self, tokens: &[&str]) -> Result<(), INIError> {
        replace_names(&mut self.removal_upgrade_names, tokens);
        Ok(())
    }

    /// Parse `RequiresAllTriggers`.
    pub fn parse_requires_all_triggers_tokens(&mut self, tokens: &[&str]) -> Result<(), INIError> {
        let value = tokens
            .iter()
            .skip_while(|t| **t == "=")
            .next()
            .ok_or(INIError::InvalidData)?;
        self.requires_all_triggers = INI::parse_bool(value)?;
        Ok(())
    }

    /// Parse `FXListUpgrade` into a named FX list handle.
    /// Matches C++ `INI::parseFXList` stored on `UpgradeMuxData::m_fxListUpgrade`.
    pub fn parse_fx_list_upgrade_tokens(&mut self, tokens: &[&str]) -> Result<(), INIError> {
        let value = tokens
            .iter()
            .skip_while(|t| **t == "=")
            .next()
            .ok_or(INIError::InvalidData)?;
        if value.eq_ignore_ascii_case("None") {
            self.fx_list_name = None;
            return Ok(());
        }
        self.fx_list_name = Some(AsciiString::from(value));
        Ok(())
    }

    /// Parse from INI
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, UPGRADE_MUX_DATA_FIELDS)
    }
}

fn replace_names(names: &mut Vec<AsciiString>, tokens: &[&str]) {
    names.clear();
    names.extend(
        tokens
            .iter()
            .skip_while(|token| **token == "=")
            .filter(|token| !token.is_empty())
            .map(|token| AsciiString::from(*token)),
    );
}

fn parse_triggered_by(
    _ini: &mut INI,
    data: &mut UpgradeMuxDefinition,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.parse_triggered_by_tokens(tokens)
}

fn parse_conflicts_with(
    _ini: &mut INI,
    data: &mut UpgradeMuxDefinition,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.parse_conflicts_with_tokens(tokens)
}

fn parse_removes_upgrades(
    _ini: &mut INI,
    data: &mut UpgradeMuxDefinition,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.parse_removes_upgrades_tokens(tokens)
}

fn parse_requires_all_triggers(
    _ini: &mut INI,
    data: &mut UpgradeMuxDefinition,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.parse_requires_all_triggers_tokens(tokens)
}

fn parse_fx_list_upgrade(
    _ini: &mut INI,
    data: &mut UpgradeMuxDefinition,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.parse_fx_list_upgrade_tokens(tokens)
}

const UPGRADE_MUX_DATA_FIELDS: &[FieldParse<UpgradeMuxDefinition>] = &[
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
        token: "FXListUpgrade",
        parse: parse_fx_list_upgrade,
    },
];

/// Parsed UpgradeMux data and the optional masks materialized by legacy users.
#[derive(Debug, Clone, Default)]
pub struct UpgradeMuxData {
    pub definition: UpgradeMuxDefinition,
    activation_mask: Option<UpgradeMask>,
    conflicting_mask: Option<UpgradeMask>,
}

impl UpgradeMuxData {
    pub fn from_definition(definition: UpgradeMuxDefinition) -> Self {
        Self {
            definition,
            ..Self::default()
        }
    }

    pub fn activation_masks_with(
        &self,
        resolve: impl Fn(&str) -> UpgradeMask,
    ) -> (UpgradeMask, UpgradeMask) {
        if self.definition.activation_upgrade_names.is_empty()
            && self.definition.conflicting_upgrade_names.is_empty()
        {
            return (
                self.activation_mask.unwrap_or_else(UpgradeMask::none),
                self.conflicting_mask.unwrap_or_else(UpgradeMask::none),
            );
        }

        let mut activation = UpgradeMask::none();
        for name in &self.definition.activation_upgrade_names {
            activation |= resolve(name.as_str());
        }
        let mut conflicting = UpgradeMask::none();
        for name in &self.definition.conflicting_upgrade_names {
            conflicting |= resolve(name.as_str());
        }
        (activation, conflicting)
    }

    /// Get activation and conflicting masks (lazy compute)
    /// Matches C++ UpgradeMuxData::getUpgradeActivationMasks (Module.cpp:264-310).
    pub fn cache_activation_masks_with(
        &mut self,
        resolve: impl Fn(&str) -> UpgradeMask,
    ) -> (UpgradeMask, UpgradeMask) {
        let (activation, conflicting) = self.activation_masks_with(resolve);
        if self.definition.activation_upgrade_names.is_empty()
            && self.definition.conflicting_upgrade_names.is_empty()
        {
            return (activation, conflicting);
        }
        // C++ copies activation names into trigger list, then clears both vectors.
        self.definition.trigger_upgrade_names = self.definition.activation_upgrade_names.clone();
        self.definition.activation_upgrade_names.clear();
        self.definition.conflicting_upgrade_names.clear();
        self.activation_mask = Some(activation);
        self.conflicting_mask = Some(conflicting);
        (activation, conflicting)
    }

    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        self.definition.parse_from_ini(ini)
    }
}
