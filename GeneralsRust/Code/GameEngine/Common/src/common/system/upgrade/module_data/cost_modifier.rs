//! Authored CostModifierUpgrade module data and its INI fields.

use super::{
    FieldParse, INI, INIError, ModuleData, NameKeyType, Snapshotable, UpgradeMuxData, Xfer,
};
use crate::common::system::kind_of::KindOfMask;
use std::any::Any;

/// Module data describing the cost modifier upgrade.
#[derive(Debug, Clone)]
pub struct CostModifierUpgradeModuleData {
    module_tag_name_key: NameKeyType,
    pub upgrade_mux_data: UpgradeMuxData,
    kind_of: u128,
    percentage: f32,
}

impl Default for CostModifierUpgradeModuleData {
    fn default() -> Self {
        Self {
            module_tag_name_key: 0,
            upgrade_mux_data: UpgradeMuxData::default(),
            kind_of: 0,
            percentage: 0.0,
        }
    }
}

impl CostModifierUpgradeModuleData {
    pub fn kind_of(&self) -> u128 {
        self.kind_of
    }

    pub fn percentage(&self) -> f32 {
        self.percentage
    }

    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, COST_MODIFIER_UPGRADE_FIELDS)
    }
}

impl_definition!(CostModifierUpgradeModuleData);

fn parse_effect_kind_of_field(
    _ini: &mut INI,
    data: &mut CostModifierUpgradeModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let previous = KindOfMask::from_bits_retain(data.kind_of);
    data.kind_of = KindOfMask::parse_ini(previous, &tokens.join(" "))
        .map_err(|_| INIError::InvalidNameList)?
        .bits();
    Ok(())
}

fn parse_percentage_field(
    _ini: &mut INI,
    data: &mut CostModifierUpgradeModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    if tokens.is_empty() {
        return Err(INIError::InvalidData);
    }
    data.percentage = INI::parse_percent_to_real(tokens[0])?;
    Ok(())
}

const COST_MODIFIER_UPGRADE_FIELDS: &[FieldParse<CostModifierUpgradeModuleData>] = upgrade_fields!(
    CostModifierUpgradeModuleData,
    FieldParse {
        token: "EffectKindOf",
        parse: parse_effect_kind_of_field,
    },
    FieldParse {
        token: "Percentage",
        parse: parse_percentage_field,
    },
);
