//! UpgradeDie - Removes upgrade from producer on death
//!
//! Original C++ location: GameLogic/Module/UpgradeDie.h/.cpp
//! Original C++ Author: Kris Morness, August 2002
//! Rust conversion: 2025

use super::{DieModule, DieModuleData, DieModuleInterface};
use crate::common::{AsciiString, ModuleData};
use crate::damage::DamageInfo;
use crate::helpers::TheGameLogic;
use crate::object::INVALID_ID;
use crate::object::Object;
use crate::object::die::{
    parse_die_mux_death_types, parse_die_mux_exempt_status, parse_die_mux_required_status,
    parse_die_mux_veterancy_levels,
};
use crate::upgrade::center::with_upgrade_center;
use game_engine::common::ini::{FieldParse, INI, INIError};
use game_engine::common::system::{Snapshotable, Xfer};
use std::sync::{Arc, RwLock};

/// Module data for UpgradeDie
/// (Matches C++ UpgradeDieModuleData)
#[derive(Debug, Clone)]
pub struct UpgradeDieModuleData {
    pub base: DieModuleData,
    /// Name of the upgrade to remove when this object dies
    pub upgrade_name: AsciiString,
}

impl Default for UpgradeDieModuleData {
    fn default() -> Self {
        Self {
            base: DieModuleData::default(),
            upgrade_name: AsciiString::default(),
        }
    }
}

impl Snapshotable for UpgradeDieModuleData {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("UpgradeDieModuleData crc version: {e:?}"))?;
        self.base.crc(xfer)?;
        let mut name = self.upgrade_name.as_str().to_string();
        xfer.xfer_ascii_string(&mut name)
            .map_err(|e| format!("UpgradeDieModuleData crc upgrade_name: {e:?}"))?;
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("UpgradeDieModuleData xfer version: {e:?}"))?;

        self.base.xfer(xfer)?;

        let mut name = self.upgrade_name.as_str().to_string();
        xfer.xfer_ascii_string(&mut name)
            .map_err(|e| format!("UpgradeDieModuleData upgrade_name: {e:?}"))?;
        self.upgrade_name = AsciiString::from(name.as_str());

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

crate::impl_legacy_module_data_via_base!(UpgradeDieModuleData, base);

impl UpgradeDieModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, UPGRADE_DIE_FIELDS)
    }
}

/// UpgradeDie - Frees producer's upgrade on death
///
/// This module removes an upgrade from the producer (typically a building)
/// when the object dies. This is used for units that were created via an
/// upgrade, so that when the unit is destroyed, the upgrade slot becomes
/// available again.
///
/// For example:
/// - A general's power created by an upgrade
/// - A special unit that occupies an upgrade slot
/// - Hero units tied to building upgrades
///
/// (Matches C++ UpgradeDie)
#[derive(Debug)]
pub struct UpgradeDie {
    base: DieModule<UpgradeDieModuleData>,
}

impl UpgradeDie {
    /// Create a new UpgradeDie module
    pub fn new(object: Arc<RwLock<Object>>, module_data: Arc<UpgradeDieModuleData>) -> Self {
        Self {
            base: DieModule::new(object, module_data),
        }
    }

    /// Get module name
    pub fn get_module_name() -> &'static str {
        "UpgradeDie"
    }

    /// Remove the upgrade from the producer
    fn remove_upgrade(&self, object: &Object) {
        let upgrade_name = &self.base.module_data.upgrade_name;

        if upgrade_name.is_empty() {
            return;
        }

        let producer_id = object.get_producer_id();
        if producer_id == INVALID_ID {
            return;
        }

        let Some(producer) = TheGameLogic::find_object_by_id(producer_id) else {
            return;
        };

        let upgrade = with_upgrade_center(|center| center.find_upgrade(upgrade_name.as_str()));
        let Some(upgrade) = upgrade else {
            return;
        };

        let Ok(mut producer_guard) = producer.write() else {
            return;
        };
        if producer_guard.has_upgrade(&upgrade) {
            producer_guard.remove_upgrade(&upgrade);
        } else {
            debug_assert!(
                false,
                "Object {} died and tried to free upgrade '{}' in producer {}, but producer lacks it.",
                object.get_template().get_name().as_str(),
                upgrade_name,
                producer_guard.get_template().get_name().as_str()
            );
        }
    }
}

impl DieModuleInterface for UpgradeDie {
    /// Called when the object dies - removes the upgrade
    /// (Matches C++ UpgradeDie::onDie)
    fn on_die(&mut self, object: &mut Object, damage_info: &DamageInfo) {
        // Check if this die module should activate
        if !self.is_die_applicable(
            object,
            damage_info,
            &self.base.module_data.base.die_mux_data,
        ) {
            return;
        }

        // Remove the upgrade from the producer
        self.remove_upgrade(object);
    }
}

fn parse_die_death_types(
    _ini: &mut INI,
    data: &mut UpgradeDieModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    parse_die_mux_death_types(&mut data.base.die_mux_data, tokens)
}

fn parse_die_veterancy_levels(
    _ini: &mut INI,
    data: &mut UpgradeDieModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    parse_die_mux_veterancy_levels(&mut data.base.die_mux_data, tokens)
}

fn parse_die_exempt_status(
    _ini: &mut INI,
    data: &mut UpgradeDieModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    parse_die_mux_exempt_status(&mut data.base.die_mux_data, tokens)
}

fn parse_die_required_status(
    _ini: &mut INI,
    data: &mut UpgradeDieModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    parse_die_mux_required_status(&mut data.base.die_mux_data, tokens)
}

fn parse_upgrade_to_remove(
    _ini: &mut INI,
    data: &mut UpgradeDieModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens
        .iter()
        .skip_while(|t| **t == "=")
        .next()
        .ok_or(INIError::InvalidData)?;
    data.upgrade_name = AsciiString::from(token);
    Ok(())
}

// C++ UpgradeDie.h:30 DieModuleData::buildFieldParse then UpgradeToRemove.
const UPGRADE_DIE_FIELDS: &[FieldParse<UpgradeDieModuleData>] = &[
    FieldParse {
        token: "DeathTypes",
        parse: parse_die_death_types,
    },
    FieldParse {
        token: "VeterancyLevels",
        parse: parse_die_veterancy_levels,
    },
    FieldParse {
        token: "ExemptStatus",
        parse: parse_die_exempt_status,
    },
    FieldParse {
        token: "RequiredStatus",
        parse: parse_die_required_status,
    },
    FieldParse {
        token: "UpgradeToRemove",
        parse: parse_upgrade_to_remove,
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_upgrade_die_module_data_default() {
        let data = UpgradeDieModuleData::default();
        assert!(data.upgrade_name.is_empty());
    }

    #[test]
    fn test_upgrade_die_module_name() {
        assert_eq!(UpgradeDie::get_module_name(), "UpgradeDie");
    }

    #[test]
    fn test_upgrade_die_with_upgrade_name() {
        let mut data = UpgradeDieModuleData::default();
        data.upgrade_name = AsciiString::from("Upgrade_HeroUnit");
        assert_eq!(data.upgrade_name.as_str(), "Upgrade_HeroUnit");
    }

    #[test]
    fn upgrade_die_parses_die_mux_keys() {
        // C++ UpgradeDie.h:30 DieModuleData::buildFieldParse(p)
        assert!(UPGRADE_DIE_FIELDS.iter().any(|f| f.token == "DeathTypes"));
        assert!(
            UPGRADE_DIE_FIELDS
                .iter()
                .any(|f| f.token == "VeterancyLevels")
        );
        assert!(UPGRADE_DIE_FIELDS.iter().any(|f| f.token == "ExemptStatus"));
        assert!(
            UPGRADE_DIE_FIELDS
                .iter()
                .any(|f| f.token == "RequiredStatus")
        );
        assert!(
            UPGRADE_DIE_FIELDS
                .iter()
                .any(|f| f.token == "UpgradeToRemove")
        );
    }
}
