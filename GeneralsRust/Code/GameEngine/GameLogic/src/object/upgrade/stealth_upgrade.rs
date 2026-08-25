//! StealthUpgrade Module - Complete Port from C++
//!
//! Matches C++ StealthUpgrade.cpp and StealthUpgrade.h exactly
//! Location: GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Upgrade/StealthUpgrade.cpp
//!
//! Simple upgrade that grants OBJECT_STATUS_CAN_STEALTH status to enable stealth capability.
//! Used for upgrades like "Black Market" that grant stealth to units.

use std::sync::Arc;

use crate::common::{
    AsciiString, KindOf, LegacyModuleData, ObjectID, ObjectStatusMaskType, UpgradeMaskType,
};
use crate::modules::UpgradeModuleInterface;
use crate::object::registry::OBJECT_REGISTRY;
use crate::upgrade::UpgradeMask;
use crate::upgrade::modules::upgrade_mux::{UpgradeMux, UpgradeMuxData};
use game_engine::common::ini::{FieldParse, INI, INIError};
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{Module, ModuleData, NameKeyType};
use log::debug;

/// Wave 448: host-only path has no dual-world factory objects.
#[inline]
fn dual_world_registry_unavailable() -> bool {
    crate::object::registry::OBJECT_REGISTRY.is_empty()
}

/// Stealth upgrade module data
/// Matches C++ StealthUpgrade (UpgradeModuleData parses TriggeredBy/ConflictsWith).
#[derive(Debug, Clone)]
pub struct StealthUpgradeModuleData {
    module_tag_name_key: NameKeyType,
    pub upgrade_mux_data: UpgradeMuxData,
}

impl Default for StealthUpgradeModuleData {
    fn default() -> Self {
        Self {
            module_tag_name_key: 0,
            upgrade_mux_data: UpgradeMuxData::default(),
        }
    }
}

impl StealthUpgradeModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, STEALTH_UPGRADE_FIELDS)
    }
}

impl ModuleData for StealthUpgradeModuleData {
    fn set_module_tag_name_key(&mut self, key: NameKeyType) {
        self.module_tag_name_key = key;
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.module_tag_name_key
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl LegacyModuleData for StealthUpgradeModuleData {
    fn set_module_tag_name_key(&mut self, key: NameKeyType) {
        self.module_tag_name_key = key;
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.module_tag_name_key
    }
}

impl Snapshotable for StealthUpgradeModuleData {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 0;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 0;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

/// Stealth upgrade module
/// Matches C++ StealthUpgrade class lines 20-35
pub struct StealthUpgrade {
    module_name_key: NameKeyType,
    data: Arc<StealthUpgradeModuleData>,
    mux: UpgradeMux,
    object_id: ObjectID,
    applied: bool,
}

impl StealthUpgrade {
    pub fn new(
        module_name_key: NameKeyType,
        data: Arc<StealthUpgradeModuleData>,
        object_id: ObjectID,
    ) -> Self {
        let mux = UpgradeMux::new(data.upgrade_mux_data.clone());
        Self {
            module_name_key,
            data,
            mux,
            object_id,
            applied: false,
        }
    }

    /// Apply the upgrade
    /// Matches C++ upgradeImplementation lines 27-42
    pub fn upgrade_implementation(&mut self) -> Result<(), String> {
        // C++ UpgradeMux::wouldUpgrade refuses if m_upgradeExecuted.
        if self.applied || self.mux.is_already_upgraded() {
            return Ok(());
        }

        // Wave 448: empty dual-world → Ok(()).
        if dual_world_registry_unavailable() {
            return Ok(());
        }

        let Some(()) = OBJECT_REGISTRY.with_object_mut(self.object_id, |guard| {
            // The logic that does the stealthupdate will notice this and start stealthing
            // C++ line 30: me->setStatus( MAKE_OBJECT_STATUS_MASK( OBJECT_STATUS_CAN_STEALTH ) );
            guard.set_status(ObjectStatusMaskType::CAN_STEALTH, true);

            // Grant stealth to spawns if applicable (C++ lines 33-41)
            if guard.is_kind_of(KindOf::SpawnsAreTheWeapons) {
                let _ = guard.with_spawn_behavior_full_interface(|spawn_behavior| {
                    let _ = spawn_behavior.give_slaves_stealth_upgrade(true);
                });
            }
        }) else {
            return Err("Object not found".to_string());
        };

        self.applied = true;
        self.mux.set_upgrade_executed(true);
        debug!("Stealth upgrade applied to object {}", self.object_id);

        Ok(())
    }

    pub fn is_applied(&self) -> bool {
        self.applied
    }
}

impl Module for StealthUpgrade {
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
        ModuleData::get_module_tag_name_key(&*self.data)
    }

    fn get_module_data(&self) -> &dyn ModuleData {
        self.data.as_ref()
    }

    fn on_object_created(&mut self) {
        // Upgrade is typically triggered by player upgrade, not on creation
    }
}

impl UpgradeModuleInterface for StealthUpgrade {
    fn can_upgrade(&self, upgrade_mask: UpgradeMaskType) -> bool {
        if self.applied || self.mux.is_already_upgraded() {
            return false;
        }
        let mask = UpgradeMask::from_bits_retain(upgrade_mask.bits());
        self.mux.would_upgrade(mask)
    }

    fn apply_upgrade(&mut self, upgrade_mask: UpgradeMaskType) -> bool {
        if !self.can_upgrade(upgrade_mask) {
            return false;
        }
        self.upgrade_implementation().is_ok()
    }

    fn remove_upgrade(&mut self, upgrade_mask: UpgradeMaskType) {
        // C++ resetUpgrade: clear executed so RemovesUpgrades can re-arm.
        // Does not remove stealth status once applied.
        let mask = crate::upgrade::UpgradeMask::from_bits_retain(upgrade_mask.bits());
        if self.mux.reset_upgrade(mask) {
            self.applied = false;
        }
    }
}

impl Snapshotable for StealthUpgrade {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let current_version: u8 = 1;
        let mut version = current_version;
        xfer.xfer_version(&mut version, current_version)
            .map_err(|e| e.to_string())?;
        crate::object::upgrade::upgrade_module::crc_upgrade_module_state(xfer, self.applied)?;
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let current_version: u8 = 1;
        let mut version = current_version;
        xfer.xfer_version(&mut version, current_version)
            .map_err(|e| e.to_string())?;
        crate::object::upgrade::upgrade_module::xfer_upgrade_module_state(xfer, &mut self.applied)?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        self.mux.set_upgrade_executed(self.applied);
        Ok(())
    }
}

fn parse_triggered_by(
    _ini: &mut INI,
    data: &mut StealthUpgradeModuleData,
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
    data: &mut StealthUpgradeModuleData,
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
    data: &mut StealthUpgradeModuleData,
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
    data: &mut StealthUpgradeModuleData,
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

const STEALTH_UPGRADE_FIELDS: &[FieldParse<StealthUpgradeModuleData>] = &[
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
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stealth_upgrade_creation() {
        let data = Arc::new(StealthUpgradeModuleData::default());
        let upgrade = StealthUpgrade::new(1, data, 100);
        assert!(!upgrade.is_applied());
    }

    #[test]
    fn test_stealth_upgrade_module_data() {
        let data = StealthUpgradeModuleData::default();
        assert_eq!(data.module_tag_name_key, 0);
    }

    #[test]
    fn remove_upgrade_resets_executed_so_module_can_rearm() {
        let mut data = StealthUpgradeModuleData::default();
        data.upgrade_mux_data
            .activation_upgrade_names
            .push(AsciiString::from("Upgrade_GLACamouflage"));
        let mut upgrade = StealthUpgrade::new(1, Arc::new(data), 100);
        upgrade.applied = true;
        upgrade.mux.set_upgrade_executed(true);
        let mask = crate::common::UpgradeMaskType::from_bits_retain(
            crate::upgrade::upgrade_mask_for_name("Upgrade_GLACamouflage").to_bits(),
        );
        upgrade.remove_upgrade(mask);
        assert!(!upgrade.is_applied());
        assert!(!upgrade.mux.is_already_upgraded());
        assert!(upgrade.can_upgrade(mask));
    }
}
