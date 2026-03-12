//! Upgrade Mux System
//!
//! Base functionality for modules that respond to upgrade triggers.
//! Matches C++ UpgradeMux and UpgradeMuxData from UpgradeModule.h/.cpp
//!
//! Original C++ Authors: Johnson, Day, Smallwood, September 2002

use super::super::{upgrade_mask_for_name, UpgradeMask};
use crate::common::*;
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::system::{Snapshotable, Xfer};
use std::sync::Arc;

/// Interface for upgrade modules
/// Matches C++ UpgradeModuleInterface from UpgradeModule.h
pub trait UpgradeModuleInterface {
    /// Check if upgrade has already been applied
    fn is_already_upgraded(&self) -> bool;

    /// Attempt to apply upgrade with given mask
    /// Returns true if upgrade was triggered
    fn attempt_upgrade(&mut self, key_mask: UpgradeMask, object: &mut Object) -> bool;

    /// Check if this upgrade would trigger with given mask (without applying)
    fn would_upgrade(&self, key_mask: UpgradeMask) -> bool;

    /// Reset upgrade state (for conflicting upgrades)
    fn reset_upgrade(&mut self, key_mask: UpgradeMask) -> bool;

    /// Test if upgrade conditions are met
    fn test_upgrade_conditions(&self, key_mask: UpgradeMask) -> bool;

    /// Force refresh of already-applied upgrade
    fn force_refresh_upgrade(&mut self, object: &mut Object);
}

/// Configuration data for upgrade triggers
/// Matches C++ UpgradeMuxData from UpgradeModule.h
#[derive(Debug, Clone, Default)]
pub struct UpgradeMuxData {
    /// Upgrades that trigger this module (activation conditions)
    pub trigger_upgrade_names: Vec<AsciiString>,
    /// Same as trigger names (legacy field)
    pub activation_upgrade_names: Vec<AsciiString>,
    /// Upgrades that conflict with this (prevent activation)
    pub conflicting_upgrade_names: Vec<AsciiString>,
    /// Upgrades to remove when this activates
    pub removal_upgrade_names: Vec<AsciiString>,
    /// FX list to play when upgrade triggers
    pub fx_list_upgrade: Option<Arc<FXList>>,
    /// Cached activation mask
    activation_mask: Option<UpgradeMask>,
    /// Cached conflicting mask
    conflicting_mask: Option<UpgradeMask>,
    /// Require all triggers (AND) vs any trigger (OR)
    pub requires_all_triggers: bool,
}

impl UpgradeMuxData {
    /// Get activation and conflicting masks (lazy compute)
    /// Matches C++ UpgradeMuxData::getUpgradeActivationMasks
    pub fn get_upgrade_activation_masks(&mut self) -> (UpgradeMask, UpgradeMask) {
        // Use cached values if available
        if let (Some(activation), Some(conflicting)) = (self.activation_mask, self.conflicting_mask)
        {
            return (activation, conflicting);
        }

        // Compute activation mask
        let mut activation = UpgradeMask::none();
        for name in &self.activation_upgrade_names {
            let mask = upgrade_mask_for_name(name.as_str());
            activation |= mask;
        }
        for name in &self.trigger_upgrade_names {
            let mask = upgrade_mask_for_name(name.as_str());
            activation |= mask;
        }

        // Compute conflicting mask
        let mut conflicting = UpgradeMask::none();
        for name in &self.conflicting_upgrade_names {
            let mask = upgrade_mask_for_name(name.as_str());
            conflicting |= mask;
        }

        // Cache for next time
        self.activation_mask = Some(activation);
        self.conflicting_mask = Some(conflicting);

        (activation, conflicting)
    }

    /// Perform upgrade FX
    /// Matches C++ UpgradeMuxData::performUpgradeFX
    pub fn perform_upgrade_fx(&self, object: &mut Object) {
        if let Some(fx_list) = &self.fx_list_upgrade {
            if let Some(fx_mgr) = crate::helpers::get_fx_list_manager() {
                fx_mgr.do_fx_obj(fx_list.id(), object.get_id());
            } else {
                log::debug!(
                    "Upgrade FX list requested for object {}, but FX manager is not registered",
                    object.get_id()
                );
            }
        }
    }

    /// Process upgrade removals
    /// Matches C++ UpgradeMuxData::muxDataProcessUpgradeRemoval
    pub fn process_upgrade_removal(&self, object: &mut Object) {
        for name in &self.removal_upgrade_names {
            let mask = upgrade_mask_for_name(name.as_str());

            // Convert UpgradeMask to UpgradeMaskType for object methods
            let mask_bits = crate::common::UpgradeMaskType::from_bits_retain(mask.to_bits());

            // Remove conflicting upgrades from object
            // Matches C++ UpgradeMuxData::muxDataProcessUpgradeRemoval
            // which calls obj->loseUpgrade(mask) for each removal
            object.remove_upgrade_mask(mask_bits);

            log::debug!(
                "Removed conflicting upgrade {} (mask {:?}) from object {}",
                name,
                mask,
                object.get_id()
            );
        }
    }

    /// Check if triggered by specific upgrade name
    /// Matches C++ UpgradeMuxData::isTriggeredBy
    pub fn is_triggered_by(&self, upgrade: &str) -> bool {
        self.activation_upgrade_names
            .iter()
            .any(|n| n.as_str() == upgrade)
            || self
                .trigger_upgrade_names
                .iter()
                .any(|n| n.as_str() == upgrade)
    }

    /// Parse from INI
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, UPGRADE_MUX_DATA_FIELDS)
    }
}

/// Upgrade mux state and behavior
/// Matches C++ UpgradeMux from UpgradeModule.h/.cpp
#[derive(Debug)]
pub struct UpgradeMux {
    /// Configuration data
    pub data: UpgradeMuxData,
    /// Whether upgrade has been executed
    upgrade_executed: bool,
}

impl UpgradeMux {
    /// Create new upgrade mux
    /// Matches C++ UpgradeMux::UpgradeMux
    pub fn new(data: UpgradeMuxData) -> Self {
        Self {
            data,
            upgrade_executed: false,
        }
    }

    /// Check if already upgraded
    /// Matches C++ UpgradeMux::isAlreadyUpgraded
    pub fn is_already_upgraded(&self) -> bool {
        self.upgrade_executed
    }

    /// Set upgraded state
    pub fn set_upgrade_executed(&mut self, executed: bool) {
        self.upgrade_executed = executed;
    }

    /// Attempt upgrade
    /// Matches C++ UpgradeMux::attemptUpgrade
    pub fn attempt_upgrade(&mut self, key_mask: UpgradeMask, object: &mut Object) -> bool {
        if self.would_upgrade(key_mask) {
            self.give_self_upgrade(object);
            true
        } else {
            false
        }
    }

    /// Check if would upgrade
    /// Matches C++ UpgradeMux::wouldUpgrade
    pub fn would_upgrade(&self, key_mask: UpgradeMask) -> bool {
        let (activation, conflicting) = self.data.clone().get_upgrade_activation_masks();

        // Must have activation conditions and not be executed yet
        if !activation.any() || !key_mask.any() || self.upgrade_executed {
            return false;
        }

        // Check for conflicts
        if key_mask.test_for_any(conflicting) {
            return false;
        }

        // Check activation conditions
        if self.data.requires_all_triggers {
            // ALL triggers must be met
            key_mask.test_for_all(activation)
        } else {
            // ANY trigger is sufficient
            key_mask.test_for_any(activation)
        }
    }

    /// Give self upgrade
    /// Matches C++ UpgradeMux::giveSelfUpgrade
    fn give_self_upgrade(&mut self, object: &mut Object) {
        self.data.perform_upgrade_fx(object);
        self.data.process_upgrade_removal(object);
        // Actual upgrade implementation is handled by subclass
        self.set_upgrade_executed(true);
    }

    /// Test upgrade conditions
    /// Matches C++ UpgradeMux::testUpgradeConditions
    pub fn test_upgrade_conditions(&self, key_mask: UpgradeMask) -> bool {
        let (activation, conflicting) = self.data.clone().get_upgrade_activation_masks();

        // Check for conflicts
        if key_mask.any() && key_mask.test_for_any(conflicting) {
            return false;
        }

        // If no activation requirements, only check conflicts
        if !activation.any() {
            return true;
        }

        // Check activation conditions
        if self.data.requires_all_triggers {
            key_mask.test_for_all(activation)
        } else {
            key_mask.test_for_any(activation)
        }
    }

    /// Reset upgrade
    /// Matches C++ UpgradeMux::resetUpgrade
    pub fn reset_upgrade(&mut self, key_mask: UpgradeMask) -> bool {
        let (activation, _) = self.data.clone().get_upgrade_activation_masks();

        if key_mask.test_for_any(activation) && self.upgrade_executed {
            self.upgrade_executed = false;
            true
        } else {
            false
        }
    }

    /// Force refresh upgrade
    /// Matches C++ UpgradeMux::forceRefreshUpgrade
    pub fn force_refresh_upgrade(&mut self, object: &mut Object) {
        if self.upgrade_executed {
            // Only refresh if already upgraded
            // Actual implementation is in subclass
            log::debug!("Force refreshing upgrade for object {}", object.get_id());
        }
    }
}

impl Snapshotable for UpgradeMux {
    /// CRC for save game validation
    /// Matches C++ UpgradeMux::upgradeMuxCRC
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        // CRC calculation for save game validation
        // Matches C++ UpgradeMux::upgradeMuxCRC
        // CRC only reads state for validation, doesn't modify it
        let mut version = 1u8;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| e.to_string())?;

        // CRC the upgrade_executed flag
        // Note: In CRC mode, xfer is read-only but needs mutable reference
        // This is a limitation of the Xfer trait design
        let mut upgrade_executed = self.upgrade_executed;
        xfer.xfer_bool(&mut upgrade_executed)
            .map_err(|e| e.to_string())?;

        Ok(())
    }

    /// Serialize/deserialize
    /// Matches C++ UpgradeMux::upgradeMuxXfer
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        // Version 1
        let mut version = 1u8;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| e.to_string())?;

        // Serialize upgrade_executed flag
        xfer.xfer_bool(&mut self.upgrade_executed)
            .map_err(|e| e.to_string())?;

        Ok(())
    }

    /// Post-load processing
    /// Matches C++ UpgradeMux::upgradeMuxLoadPostProcess
    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

// INI parsing functions
fn parse_triggered_by(
    _ini: &mut INI,
    data: &mut UpgradeMuxData,
    tokens: &[&str],
) -> Result<(), INIError> {
    // Parse list of upgrade names
    for token in tokens.iter().skip_while(|t| **t == "=") {
        if !token.is_empty() {
            data.activation_upgrade_names
                .push(AsciiString::from(*token));
        }
    }
    Ok(())
}

fn parse_conflicts_with(
    _ini: &mut INI,
    data: &mut UpgradeMuxData,
    tokens: &[&str],
) -> Result<(), INIError> {
    for token in tokens.iter().skip_while(|t| **t == "=") {
        if !token.is_empty() {
            data.conflicting_upgrade_names
                .push(AsciiString::from(*token));
        }
    }
    Ok(())
}

fn parse_removes_upgrades(
    _ini: &mut INI,
    data: &mut UpgradeMuxData,
    tokens: &[&str],
) -> Result<(), INIError> {
    for token in tokens.iter().skip_while(|t| **t == "=") {
        if !token.is_empty() {
            data.removal_upgrade_names.push(AsciiString::from(*token));
        }
    }
    Ok(())
}

fn parse_requires_all_triggers(
    _ini: &mut INI,
    data: &mut UpgradeMuxData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let value = tokens
        .iter()
        .skip_while(|t| **t == "=")
        .next()
        .ok_or(INIError::InvalidData)?;
    data.requires_all_triggers = INI::parse_bool(value)?;
    Ok(())
}

const UPGRADE_MUX_DATA_FIELDS: &[FieldParse<UpgradeMuxData>] = &[
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

    fn make_test_mask_with_bit(bit: usize) -> UpgradeMask {
        let mut mask = UpgradeMask::none();
        mask.set_bit(bit);
        mask
    }

    #[test]
    fn test_upgrade_mux_basic() {
        let data = UpgradeMuxData {
            activation_upgrade_names: vec![AsciiString::from("TestUpgrade")],
            ..Default::default()
        };

        let mux = UpgradeMux::new(data);
        assert!(!mux.is_already_upgraded());
    }

    #[test]
    fn test_would_upgrade() {
        let mut data = UpgradeMuxData {
            activation_upgrade_names: vec![AsciiString::from("TestUpgrade")],
            requires_all_triggers: false,
            ..Default::default()
        };

        // Get masks
        let (activation, _) = data.get_upgrade_activation_masks();

        let mux = UpgradeMux::new(data);
        assert!(mux.would_upgrade(activation));
    }

    #[test]
    fn test_conflicting_upgrades() {
        let mut data = UpgradeMuxData {
            activation_upgrade_names: vec![AsciiString::from("UpgradeA")],
            conflicting_upgrade_names: vec![AsciiString::from("UpgradeB")],
            requires_all_triggers: false,
            ..Default::default()
        };

        let (activation, conflicting) = data.get_upgrade_activation_masks();

        let mux = UpgradeMux::new(data);

        // With only activation, should work
        assert!(mux.would_upgrade(activation));

        // With activation + conflict, should not work
        let combined = activation | conflicting;
        assert!(!mux.would_upgrade(combined));
    }

    #[test]
    fn test_requires_all_triggers() {
        let mut data = UpgradeMuxData {
            activation_upgrade_names: vec![
                AsciiString::from("UpgradeA"),
                AsciiString::from("UpgradeB"),
            ],
            requires_all_triggers: true,
            ..Default::default()
        };

        let (activation, _) = data.get_upgrade_activation_masks();

        let mux = UpgradeMux::new(data);

        // Must have ALL bits set
        assert!(mux.would_upgrade(activation));

        // With only partial, should fail
        let mask_a = upgrade_mask_for_name("UpgradeA");
        assert!(!mux.would_upgrade(mask_a));
    }
}
