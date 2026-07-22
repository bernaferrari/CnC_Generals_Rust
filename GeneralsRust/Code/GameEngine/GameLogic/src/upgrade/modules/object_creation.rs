//! Object Creation Upgrade Module
//!
//! Spawns objects from an ObjectCreationList when upgrade is researched.
//! Matches C++ ObjectCreationUpgrade from ObjectCreationUpgrade.h/.cpp
//!
//! Original C++ Author: Matthew D. Campbell, April 2002

use super::super::UpgradeMask;
use super::upgrade_mux::{UpgradeModuleInterface, UpgradeMux, UpgradeMuxData};
use crate::common::UpgradeMaskType;
use crate::common::*;
use crate::modules::UpgradeModuleInterface as RuntimeUpgradeModuleInterface;
use crate::object::registry::OBJECT_REGISTRY;
use crate::object::Object;
use crate::object_creation_list::live_creation_context;
use crate::object_creation_list::store::{get_object_creation_list_store, ObjectCreationList};
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{Module, ModuleData, NameKeyType};
use std::sync::Arc;

/// Module data for object creation upgrade
/// Matches C++ ObjectCreationUpgradeModuleData from ObjectCreationUpgrade.h
#[derive(Debug, Clone)]
pub struct ObjectCreationUpgradeModuleData {
    module_tag_name_key: NameKeyType,
    /// Upgrade mux configuration
    pub upgrade_mux_data: UpgradeMuxData,
    /// Object creation list to spawn (matches C++ m_ocl)
    pub ocl_name: Option<AsciiString>,
    /// Resolved OCL pointer (matches C++ parseObjectCreationList behavior)
    pub ocl: Option<Arc<ObjectCreationList>>,
}

impl Default for ObjectCreationUpgradeModuleData {
    fn default() -> Self {
        Self {
            module_tag_name_key: 0,
            upgrade_mux_data: UpgradeMuxData::default(),
            ocl_name: None,
            ocl: None,
        }
    }
}

impl ObjectCreationUpgradeModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, OBJECT_CREATION_UPGRADE_FIELDS)
    }
}

impl ModuleData for ObjectCreationUpgradeModuleData {
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

impl Snapshotable for ObjectCreationUpgradeModuleData {
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
        if self.ocl.is_none() {
            if let Some(name) = self.ocl_name.as_ref() {
                if let Some(store) = get_object_creation_list_store().as_ref() {
                    self.ocl = store.find_object_creation_list(name.as_str());
                }
            }
        }
        Ok(())
    }
}

/// Object creation upgrade module
/// Matches C++ ObjectCreationUpgrade from ObjectCreationUpgrade.cpp
pub struct ObjectCreationUpgrade {
    module_name_key: NameKeyType,
    data: Arc<ObjectCreationUpgradeModuleData>,
    object_id: ObjectID,
    mux: UpgradeMux,
}

impl ObjectCreationUpgrade {
    pub fn new(
        module_name_key: NameKeyType,
        data: Arc<ObjectCreationUpgradeModuleData>,
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

    /// Apply the object creation upgrade
    /// Matches C++ ObjectCreationUpgrade::upgradeImplementation (ObjectCreationUpgrade.cpp:68-75)
    ///
    /// When upgrade is granted, this spawns all objects defined in the OCL.
    /// The C++ implementation calls:
    /// ```cpp
    /// ObjectCreationList::create((getObjectCreationUpgradeModuleData()->m_ocl), getObject(), NULL);
    /// ```
    fn upgrade_implementation(&mut self, object: &mut Object) {
        // Spawn everything in the OCL
        // Matches C++ ObjectCreationUpgrade.cpp lines 68-75
        if let Some(ocl) = self.data.ocl.as_ref() {
            let ctx = live_creation_context();
            let _ = ocl.create_with_objects(&ctx, object, None, 0);
        }
    }
}

impl UpgradeModuleInterface for ObjectCreationUpgrade {
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

impl RuntimeUpgradeModuleInterface for ObjectCreationUpgrade {
    fn can_upgrade(&self, upgrade_mask: UpgradeMaskType) -> bool {
        if upgrade_mask.is_empty() {
            return false;
        }
        let key_mask = UpgradeMask::from_bits_retain(upgrade_mask.bits());
        self.mux.would_upgrade(key_mask)
    }

    fn apply_upgrade(&mut self, upgrade_mask: UpgradeMaskType) -> bool {
        let key_mask = UpgradeMask::from_bits_retain(upgrade_mask.bits());
        if !self.mux.would_upgrade(key_mask) {
            return false;
        }

        let mux_data = self.mux.data.clone();
        let ocl = self.data.ocl.clone();
        let applied = OBJECT_REGISTRY.with_object_mut(self.object_id, |object_guard| {
            mux_data.perform_upgrade_fx(object_guard);
            mux_data.process_upgrade_removal(object_guard);
            // Spawn everything in the OCL
            // Matches C++ ObjectCreationUpgrade.cpp lines 68-75
            if let Some(ocl) = ocl.as_ref() {
                let ctx = live_creation_context();
                let _ = ocl.create_with_objects(&ctx, object_guard, None, 0);
            }
        });
        // C++ still marks upgrade as executed even if object is missing.
        self.mux.set_upgrade_executed(true);
        true
    }

    fn remove_upgrade(&mut self, _upgrade_mask: UpgradeMaskType) {
        // C++ does not revert object creation upgrades.
    }
}

impl Module for ObjectCreationUpgrade {
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

impl Snapshotable for ObjectCreationUpgrade {
    /// CRC for save game validation
    /// Matches C++ ObjectCreationUpgrade::crc
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.mux.crc(xfer)
    }

    /// Serialize/deserialize
    /// Matches C++ ObjectCreationUpgrade::xfer (version 1)
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version = 1u8;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| e.to_string())?;
        self.mux.xfer(xfer)
    }

    /// Post-load processing
    /// Matches C++ ObjectCreationUpgrade::loadPostProcess
    fn load_post_process(&mut self) -> Result<(), String> {
        self.mux.load_post_process()
    }
}

// INI parsing
fn parse_upgrade_object(
    _ini: &mut INI,
    data: &mut ObjectCreationUpgradeModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let value = tokens
        .iter()
        .skip_while(|t| **t == "=")
        .next()
        .ok_or(INIError::InvalidData)?;
    data.ocl_name = Some(AsciiString::from(*value));
    data.ocl = TheObjectCreationListStore::find_object_creation_list(value);
    if data.ocl.is_none() {
        log::warn!("ObjectCreationUpgrade: unresolved OCL '{}'", value);
    }
    Ok(())
}

fn parse_triggered_by(
    _ini: &mut INI,
    data: &mut ObjectCreationUpgradeModuleData,
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
    data: &mut ObjectCreationUpgradeModuleData,
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
    data: &mut ObjectCreationUpgradeModuleData,
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
    data: &mut ObjectCreationUpgradeModuleData,
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

const OBJECT_CREATION_UPGRADE_FIELDS: &[FieldParse<ObjectCreationUpgradeModuleData>] = &[
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
        token: "UpgradeObject",
        parse: parse_upgrade_object,
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_object_creation_upgrade_data_default() {
        let data = ObjectCreationUpgradeModuleData::default();
        assert!(data.ocl_name.is_none());
    }

    #[test]
    fn test_object_creation_upgrade_with_ocl() {
        let mut data = ObjectCreationUpgradeModuleData::default();
        data.ocl_name = Some(AsciiString::from("OCL_SpawnTank"));

        let data = Arc::new(data);
        let upgrade = ObjectCreationUpgrade::new(1, data.clone(), 100);

        assert_eq!(data.ocl_name.as_ref().unwrap().as_str(), "OCL_SpawnTank");
        assert!(!upgrade.is_already_upgraded());
    }

    #[test]
    fn test_object_creation_upgrade_execution() {
        let mut data = ObjectCreationUpgradeModuleData::default();
        data.ocl_name = Some(AsciiString::from("OCL_Test"));

        let mut mask_data = UpgradeMuxData::default();
        mask_data
            .activation_upgrade_names
            .push(AsciiString::from("Upgrade_Tank"));
        data.upgrade_mux_data = mask_data;

        let data = Arc::new(data);
        let mut upgrade = ObjectCreationUpgrade::new(1, data, 100);

        let upgrade_mask = crate::upgrade::upgrade_mask_for_name("Upgrade_Tank");

        let mut obj = Object::new_test(100, 100.0);

        // Should trigger upgrade
        assert!(upgrade.would_upgrade(upgrade_mask));
        assert!(upgrade.attempt_upgrade(upgrade_mask, &mut obj));
        assert!(upgrade.is_already_upgraded());
    }
}
