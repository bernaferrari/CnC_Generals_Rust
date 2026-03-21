// ExperienceScalarUpgrade - Adds a scalar multiplier to object's experience gain
//
// This upgrade module increases the rate at which a unit gains experience.
// When triggered, it adds the configured scalar value to the object's
// experience tracker scalar.
//
// Matches C++ implementation from:
// - GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Upgrade/ExperienceScalarUpgrade.cpp
// - Author: Kris Morness, September 2002

use super::super::UpgradeMask;
use super::upgrade_mux::{UpgradeModuleInterface, UpgradeMux, UpgradeMuxData};
use crate::common::*;
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::system::{Snapshotable, Xfer, XferVersion};
use game_engine::common::thing::module::{Module, ModuleData, NameKeyType};
use std::sync::Arc;

/// Module data for ExperienceScalarUpgrade
/// Matches C++ ExperienceScalarUpgradeModuleData
#[derive(Debug, Clone, Default)]
pub struct ExperienceScalarUpgradeModuleData {
    module_tag_name_key: NameKeyType,
    pub upgrade_mux_data: UpgradeMuxData,
    /// Experience scalar to add when upgrade triggers
    /// Matches C++ m_addXPScalar
    pub add_xp_scalar: Real,
}

impl ExperienceScalarUpgradeModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, EXPERIENCE_SCALAR_UPGRADE_FIELDS)
    }
}

impl ModuleData for ExperienceScalarUpgradeModuleData {
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

impl Snapshotable for ExperienceScalarUpgradeModuleData {
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

fn parse_add_xp_scalar(
    _ini: &mut INI,
    data: &mut ExperienceScalarUpgradeModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let value = tokens
        .iter()
        .skip_while(|t| **t == "=")
        .next()
        .ok_or(INIError::InvalidData)?;
    data.add_xp_scalar = INI::parse_real(value)?;
    Ok(())
}

fn parse_triggered_by(
    _ini: &mut INI,
    data: &mut ExperienceScalarUpgradeModuleData,
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
    data: &mut ExperienceScalarUpgradeModuleData,
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
    data: &mut ExperienceScalarUpgradeModuleData,
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
    data: &mut ExperienceScalarUpgradeModuleData,
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

const EXPERIENCE_SCALAR_UPGRADE_FIELDS: &[FieldParse<ExperienceScalarUpgradeModuleData>] = &[
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
        token: "AddXPScalar",
        parse: parse_add_xp_scalar,
    },
];

/// ExperienceScalarUpgrade module
/// Matches C++ ExperienceScalarUpgrade class
pub struct ExperienceScalarUpgrade {
    module_name_key: NameKeyType,
    data: Arc<ExperienceScalarUpgradeModuleData>,
    object_id: ObjectID,
    mux: UpgradeMux,
}

impl ExperienceScalarUpgrade {
    /// Create a new ExperienceScalarUpgrade
    /// Matches C++ ExperienceScalarUpgrade::ExperienceScalarUpgrade
    pub fn new(
        module_name_key: NameKeyType,
        data: Arc<ExperienceScalarUpgradeModuleData>,
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

    /// Get the module data
    fn get_experience_scalar_upgrade_module_data(&self) -> &ExperienceScalarUpgradeModuleData {
        &self.data
    }

    /// Apply the experience scalar upgrade to the object
    /// Matches C++ ExperienceScalarUpgrade::upgradeImplementation
    fn upgrade_implementation(&mut self, object: &mut Object) {
        let data = self.get_experience_scalar_upgrade_module_data();

        // Get the experience tracker from the object and add the scalar
        if let Some(xp_tracker) = object.get_experience_tracker() {
            if let Ok(mut guard) = xp_tracker.lock() {
                let current_scalar = guard.get_experience_scalar();
                let new_scalar = current_scalar + data.add_xp_scalar;
                guard.set_experience_scalar(new_scalar);
                log::info!(
                    "ExperienceScalarUpgrade: Added XP scalar {} (now {}) for object {}",
                    data.add_xp_scalar,
                    new_scalar,
                    self.object_id
                );
            }
        } else {
            log::warn!(
                "ExperienceScalarUpgrade: Object {} has no experience tracker",
                self.object_id
            );
        }
    }
}

impl UpgradeModuleInterface for ExperienceScalarUpgrade {
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

impl Module for ExperienceScalarUpgrade {
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

impl Snapshotable for ExperienceScalarUpgrade {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.mux.crc(xfer)
    }
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        const CURRENT_VERSION: XferVersion = 1;
        let mut version = CURRENT_VERSION;
        xfer.xfer_version(&mut version, CURRENT_VERSION).map_err(|e| e.to_string())?;
        self.mux.xfer(xfer)
    }
    fn load_post_process(&mut self) -> Result<(), String> {
        self.mux.load_post_process()
    }
}
