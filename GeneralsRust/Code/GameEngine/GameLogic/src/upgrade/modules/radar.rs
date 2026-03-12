//! Radar Upgrade Module
//!
//! Adds radar capability to the owning player when upgrade is researched.
//! Matches C++ RadarUpgrade from RadarUpgrade.h/.cpp
//!
//! Original C++ Author: Colin Day, March 2002

use super::super::UpgradeMask;
use super::upgrade_mux::{UpgradeModuleInterface, UpgradeMux, UpgradeMuxData};
use crate::common::*;
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{Module, ModuleData, NameKeyType};
use std::sync::Arc;

/// Module data for radar upgrade
/// Matches C++ RadarUpgradeModuleData from RadarUpgrade.h
#[derive(Debug, Clone)]
pub struct RadarUpgradeModuleData {
    module_tag_name_key: NameKeyType,
    /// Upgrade mux configuration
    pub upgrade_mux_data: UpgradeMuxData,
    /// Super radar that ignores radarDisabled checks (matches C++ m_isDisableProof)
    pub is_disable_proof: bool,
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
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, RADAR_UPGRADE_FIELDS)
    }
}

impl ModuleData for RadarUpgradeModuleData {
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

impl Snapshotable for RadarUpgradeModuleData {
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

/// Radar upgrade module
/// Matches C++ RadarUpgrade from RadarUpgrade.cpp
pub struct RadarUpgrade {
    module_name_key: NameKeyType,
    data: Arc<RadarUpgradeModuleData>,
    object_id: ObjectID,
    mux: UpgradeMux,
}

impl RadarUpgrade {
    pub fn new(
        module_name_key: NameKeyType,
        data: Arc<RadarUpgradeModuleData>,
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

    /// Apply the radar upgrade
    /// Matches C++ RadarUpgrade::upgradeImplementation (lines 104-119)
    fn upgrade_implementation(&mut self, object: &mut Object) {
        let is_disable_proof = self.data.is_disable_proof;

        log::info!(
            "RadarUpgrade: Adding radar for object {} (disable_proof: {})",
            self.object_id,
            is_disable_proof
        );

        // Update the player with another radar facility
        // Matches C++ RadarUpgrade.cpp line 111: player->addRadar( md->m_isDisableProof );
        if let Some(player_arc) = object.get_controlling_player() {
            // Lock the player to get mutable access
            player_arc.write().unwrap().add_radar(is_disable_proof);
            log::debug!(
                "Added radar to player (disable_proof: {})",
                is_disable_proof
            );
        }

        // Find the radar update module of this object and extend radar
        // Matches C++ RadarUpgrade.cpp lines 114-117
        // C++: RadarUpdate *radarUpdate = (RadarUpdate *)getObject()->findUpdateModule( radarUpdateKey );
        // if( radarUpdate ) radarUpdate->extendRadar();
        if let Some(radar_module) = object.find_update_module("RadarUpdate") {
            let _ = radar_module.with_module_downcast::<crate::object::behavior::radar_update::RadarUpdateModule, _, _>(|module| {
                module.behavior_mut().extend_radar();
            });
        }
    }

    /// Handle module deletion
    /// Matches C++ RadarUpgrade::onDelete (lines 48-68)
    pub fn on_delete(&mut self, object: &mut Object) {
        // If we haven't been upgraded there is nothing to clean up
        if !self.is_already_upgraded() {
            return;
        }

        // If we're currently disabled, we shouldn't do anything, because we've already done it
        if object.is_disabled() {
            return;
        }

        // Remove the radar from the player
        if let Some(player_arc) = object.get_controlling_player() {
            // Lock the player to get mutable access
            player_arc
                .write()
                .unwrap()
                .remove_radar(self.data.is_disable_proof);
            log::debug!("Removed radar from player");
        }

        // This upgrade module is now "not upgraded"
        self.mux.set_upgrade_executed(false);
    }

    /// Handle object capture (team change)
    /// Matches C++ RadarUpgrade::onCapture (lines 72-100)
    pub fn on_capture(
        &mut self,
        object: &mut Object,
        old_owner: Option<&Player>,
        new_owner: Option<&Player>,
    ) {
        // Do nothing if we haven't upgraded yet
        if !self.is_already_upgraded() {
            return;
        }

        // If we're currently disabled, we shouldn't do anything
        if object.is_disabled() {
            return;
        }

        let is_disable_proof = self.data.is_disable_proof;

        // Remove radar from old player
        if let Some(old_player) = old_owner {
            // Note: We need mutable access to modify player state
            // In a real implementation, this would be done via a mutable reference
            // For now, we log the intent - full implementation requires refactoring
            // the Player system to support mutable access during capture
            log::warn!("RadarUpgrade::on_capture needs mutable Player access to remove radar from old owner");
            self.mux.set_upgrade_executed(false);
        }

        // Add radar to new player
        if let Some(new_player) = new_owner {
            // Note: Same issue - needs mutable access
            log::warn!(
                "RadarUpgrade::on_capture needs mutable Player access to add radar to new owner"
            );
            self.mux.set_upgrade_executed(true);
        }
    }

    /// Get whether this radar is disable-proof
    pub fn get_is_disable_proof(&self) -> bool {
        self.data.is_disable_proof
    }
}

impl UpgradeModuleInterface for RadarUpgrade {
    fn is_already_upgraded(&self) -> bool {
        self.mux.is_already_upgraded()
    }

    fn attempt_upgrade(&mut self, key_mask: UpgradeMask, object: &mut Object) -> bool {
        if self.mux.would_upgrade(key_mask) {
            self.mux.data.perform_upgrade_fx(object);
            self.mux.data.process_upgrade_removal(object);
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

impl Module for RadarUpgrade {
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

impl Snapshotable for RadarUpgrade {
    /// CRC for save game validation
    /// Matches C++ RadarUpgrade::crc
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.mux.crc(xfer)
    }

    /// Serialize/deserialize
    /// Matches C++ RadarUpgrade::xfer (version 1)
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version = 1u8;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| e.to_string())?;
        self.mux.xfer(xfer)
    }

    /// Post-load processing
    /// Matches C++ RadarUpgrade::loadPostProcess
    fn load_post_process(&mut self) -> Result<(), String> {
        self.mux.load_post_process()
    }
}

// INI parsing
fn parse_disable_proof(
    _ini: &mut INI,
    data: &mut RadarUpgradeModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let value = tokens
        .iter()
        .skip_while(|t| **t == "=")
        .next()
        .ok_or(INIError::InvalidData)?;
    data.is_disable_proof = INI::parse_bool(value)?;
    Ok(())
}

fn parse_triggered_by(
    _ini: &mut INI,
    data: &mut RadarUpgradeModuleData,
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
    data: &mut RadarUpgradeModuleData,
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
    data: &mut RadarUpgradeModuleData,
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
    data: &mut RadarUpgradeModuleData,
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

const RADAR_UPGRADE_FIELDS: &[FieldParse<RadarUpgradeModuleData>] = &[
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
        token: "DisableProof",
        parse: parse_disable_proof,
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_radar_upgrade_data_default() {
        let data = RadarUpgradeModuleData::default();
        assert!(!data.is_disable_proof);
    }

    #[test]
    fn test_radar_upgrade_normal() {
        let mut data = RadarUpgradeModuleData::default();
        data.is_disable_proof = false;

        let data = Arc::new(data);
        let upgrade = RadarUpgrade::new(1, data.clone(), 100);

        assert!(!upgrade.get_is_disable_proof());
        assert!(!upgrade.is_already_upgraded());
    }

    #[test]
    fn test_radar_upgrade_disable_proof() {
        let mut data = RadarUpgradeModuleData::default();
        data.is_disable_proof = true;

        let data = Arc::new(data);
        let upgrade = RadarUpgrade::new(1, data.clone(), 100);

        assert!(upgrade.get_is_disable_proof());
    }

    #[test]
    fn test_radar_upgrade_execution() {
        let mut data = RadarUpgradeModuleData::default();
        let mut mask_data = UpgradeMuxData::default();
        mask_data
            .activation_upgrade_names
            .push(AsciiString::from("Upgrade_Radar"));
        data.upgrade_mux_data = mask_data;

        let data = Arc::new(data);
        let mut upgrade = RadarUpgrade::new(1, data, 100);

        let upgrade_mask = crate::upgrade::upgrade_mask_for_name("Upgrade_Radar");

        let mut obj = Object::new_test(100, 100.0);

        // Should trigger upgrade
        assert!(upgrade.would_upgrade(upgrade_mask));
        assert!(upgrade.attempt_upgrade(upgrade_mask, &mut obj));
        assert!(upgrade.is_already_upgraded());
    }

    #[test]
    fn test_radar_upgrade_on_delete_clears_state() {
        let mut data = RadarUpgradeModuleData::default();
        let mut mask_data = UpgradeMuxData::default();
        mask_data
            .activation_upgrade_names
            .push(AsciiString::from("Upgrade_Radar"));
        data.upgrade_mux_data = mask_data;

        let data = Arc::new(data);
        let mut upgrade = RadarUpgrade::new(1, data, 100);

        let upgrade_mask = crate::upgrade::upgrade_mask_for_name("Upgrade_Radar");
        let mut obj = Object::new_test(100, 100.0);

        // Apply upgrade
        upgrade.attempt_upgrade(upgrade_mask, &mut obj);
        assert!(upgrade.is_already_upgraded());

        // Delete should clear state
        upgrade.on_delete(&mut obj);
        assert!(!upgrade.is_already_upgraded());
    }
}
