//! Object Creation Upgrade Module
//!
//! Spawns objects from an ObjectCreationList when upgrade is researched.
//! Matches C++ ObjectCreationUpgrade from ObjectCreationUpgrade.h/.cpp
//!
//! Original C++ Author: Matthew D. Campbell, April 2002

use crate::upgrade::modules::upgrade_mux::UpgradeMuxRuntime;
use super::super::UpgradeMask;
use super::upgrade_mux::{UpgradeModuleInterface, UpgradeMux};
use crate::common::UpgradeMaskType;
use crate::common::*;
use crate::modules::UpgradeModuleInterface as RuntimeUpgradeModuleInterface;
use crate::object::Object;
use crate::object::registry::OBJECT_REGISTRY;
use crate::object_creation_list::live_creation_context;
use crate::object_creation_list::store::{ObjectCreationList, get_object_creation_list_store};
use game_engine::common::ini::{INI, INIError};
use game_engine::common::system::upgrade::ObjectCreationUpgradeDefinition;
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{Module, ModuleData, ModuleDataHandle, ModuleDataView, NameKeyType};
use std::sync::Arc;

/// Wave 452: host-only path has no dual-world factory objects.
#[inline]
fn dual_world_registry_unavailable() -> bool {
    crate::object::registry::OBJECT_REGISTRY.is_empty()
}

/// Legacy module data binds the authored name to the legacy OCL at parse time.
#[derive(Debug, Clone, Default)]
pub struct ObjectCreationUpgradeModuleData {
    authored: ObjectCreationUpgradeDefinition,
    ocl: Option<Arc<ObjectCreationList>>,
}

impl ObjectCreationUpgradeModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        let ocl = &mut self.ocl;
        self.authored.parse_from_ini_with_ocl(ini, |name| {
            *ocl = TheObjectCreationListStore::find_object_creation_list(name);
            if ocl.is_none() {
                log::warn!("ObjectCreationUpgrade: unresolved OCL '{}'", name);
            }
        })
    }
}

impl ModuleData for ObjectCreationUpgradeModuleData {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn get_object_creation_upgrade_definition(&self) -> Option<&ObjectCreationUpgradeDefinition> {
        Some(&self.authored)
    }

    fn set_module_tag_name_key(&mut self, key: NameKeyType) {
        self.authored.set_module_tag_name_key(key);
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.authored.get_module_tag_name_key()
    }
}

impl Snapshotable for ObjectCreationUpgradeModuleData {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.authored.crc(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.authored.xfer(xfer)
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        if self.ocl.is_none() {
            if let Some(name) = self.authored.ocl_name() {
                let cell = get_object_creation_list_store();
                if let Ok(guard) = cell.read() {
                    if let Some(store) = guard.as_ref() {
                        self.ocl = store.find_object_creation_list(name);
                    }
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
    data: ModuleDataView<ObjectCreationUpgradeModuleData>,
    object_id: ObjectID,
    mux: UpgradeMux,
}

impl ObjectCreationUpgrade {
    pub fn new(
        module_name_key: NameKeyType,
        data: ModuleDataHandle,
        object_id: ObjectID,
    ) -> Self {
        let mux = UpgradeMux::new_shared(ModuleDataView::new(data.clone(), |data| {
            &data.as_any().downcast_ref::<ObjectCreationUpgradeModuleData>()
                .expect("ObjectCreationUpgrade module data").authored.upgrade_mux_data
        }));
        let data = ModuleDataView::new(data, |data| {
            data.as_any().downcast_ref::<ObjectCreationUpgradeModuleData>()
                .expect("ObjectCreationUpgrade module data")
        });

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
        let ocl = self.data.read().ocl.clone();
        if let Some(ocl) = ocl.as_ref() {
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
            let mux_data = self.data.read().authored.upgrade_mux_data.clone();
            mux_data.perform_upgrade_fx(object);
            mux_data.process_upgrade_removal(object);
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
    fn can_upgrade(&mut self, upgrade_mask: UpgradeMaskType) -> bool {
        if upgrade_mask.is_empty() {
            return false;
        }
        let key_mask = UpgradeMask::from_bits_retain(upgrade_mask.bits());
        self.mux.would_upgrade(key_mask)
    }

    fn apply_upgrade(&mut self, upgrade_mask: UpgradeMaskType) -> bool {
        // Wave 452: empty dual-world → false.
        if dual_world_registry_unavailable() {
            return false;
        }

        let key_mask = UpgradeMask::from_bits_retain(upgrade_mask.bits());
        if !self.mux.would_upgrade(key_mask) {
            return false;
        }

        let data = self.data.read();
        let mux_data = data.authored.upgrade_mux_data.clone();
        let ocl = data.ocl.clone();
        drop(data);
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

    fn remove_upgrade(&mut self, upgrade_mask: UpgradeMaskType) {
        // C++ resetUpgrade: clear executed so RemovesUpgrades can re-arm.
        // Does not revert spawned OCL objects.
        let mask = crate::upgrade::UpgradeMask::from_bits_retain(upgrade_mask.bits());
        let _ = self.reset_upgrade(mask);
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
        self.data.read().get_module_tag_name_key()
    }

    fn get_module_data(&self) -> game_engine::common::thing::module::ModuleDataRead<'_> {
        game_engine::common::thing::module::ModuleDataRead::shared(self.data.handle())
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

#[cfg(test)]
mod tests {
    use super::*;

    fn data_with_ocl(name: &str) -> ObjectCreationUpgradeModuleData {
        let mut data = ObjectCreationUpgradeModuleData::default();
        INI::new()
            .with_inline_source(&format!("UpgradeObject = {name}\nEnd\n"), |ini| {
                data.authored.parse_from_ini(ini)
            })
            .unwrap();
        data
    }

    #[test]
    fn test_object_creation_upgrade_data_default() {
        let data = ObjectCreationUpgradeModuleData::default();
        assert!(data.authored.ocl_name().is_none());
    }

    #[test]
    fn test_object_creation_upgrade_with_ocl() {
        let data = data_with_ocl("OCL_SpawnTank");

        let data = ModuleDataHandle::new(data);
        let upgrade = ObjectCreationUpgrade::new(1, data.clone(), 100);

        assert_eq!(upgrade.data.read().authored.ocl_name(), Some("OCL_SpawnTank"));
        assert!(!upgrade.is_already_upgraded());
    }

    #[test]
    fn object_creation_upgrade_preserves_the_resolved_ocl_and_late_retry() {
        use crate::object_creation_list::store::ObjectCreationListStore;
        use crate::system::engine_stores::{new_world_services, with_world_services};

        let world = new_world_services();
        world.ocl_defaults_initialized().set(()).unwrap();
        with_world_services(&world, || {
            let cell = get_object_creation_list_store();
            let register = |name: &str| {
                cell.write()
                    .unwrap()
                    .get_or_insert_with(ObjectCreationListStore::new)
                    .register_ocl(name.to_owned(), ObjectCreationList::new())
            };
            let initial = register("OCL_Initial");
            let final_list = register("OCL_Final");
            let mut data = ObjectCreationUpgradeModuleData::default();
            let parsed = INI::new().with_inline_source(
                "UpgradeObject = OCL_Initial\nUpgradeObject = OCL_Final\nInvalid = field\nEnd\n",
                |ini| data.parse_from_ini(ini),
            );
            assert_eq!(parsed, Err(INIError::UnknownToken));
            let bound = data.ocl.as_ref().unwrap();
            assert!(Arc::ptr_eq(bound, &final_list));
            assert!(!Arc::ptr_eq(bound, &initial));

            let replacement = register("OCL_Final");
            data.load_post_process().unwrap();
            assert!(Arc::ptr_eq(data.ocl.as_ref().unwrap(), &final_list));
            assert!(!Arc::ptr_eq(data.ocl.as_ref().unwrap(), &replacement));

            INI::new()
                .with_inline_source("UpgradeObject = OCL_Late\nEnd\n", |ini| {
                    data.parse_from_ini(ini)
                })
                .unwrap();
            assert!(data.ocl.is_none());
            let late = register("OCL_Late");
            data.load_post_process().unwrap();
            assert!(Arc::ptr_eq(data.ocl.as_ref().unwrap(), &late));
        });
    }

    #[test]
    fn upgrade_uses_live_admitted_mux_data() {
        let handle = ModuleDataHandle::new(ObjectCreationUpgradeModuleData::default());
        let upgrade = ObjectCreationUpgrade::new(1, handle.clone(), 100);
        let mask = crate::upgrade::test_upgrade_mask("Upgrade_Tank");
        assert!(!upgrade.would_upgrade(mask));
        {
            let mut data = handle.write();
            let any: &mut dyn std::any::Any = &mut *data;
            let data = any.downcast_mut::<ObjectCreationUpgradeModuleData>().unwrap();
            data.authored.upgrade_mux_data.definition.activation_upgrade_names.push(AsciiString::from("Upgrade_Tank"));
            data.set_module_tag_name_key(42);
        }
        assert!(upgrade.would_upgrade(mask));
        assert_eq!(upgrade.get_module_tag_name_key(), 42);
    }

    #[test]
    fn test_object_creation_upgrade_execution() {
        let mut data = data_with_ocl("OCL_Test");

        data.authored.upgrade_mux_data
            .definition
            .activation_upgrade_names
            .push(AsciiString::from("Upgrade_Tank"));

        let data = ModuleDataHandle::new(data);
        let mut upgrade = ObjectCreationUpgrade::new(1, data, 100);

        let upgrade_mask = crate::upgrade::test_upgrade_mask("Upgrade_Tank");

        let mut obj = Object::new_test(100, 100.0);

        // Should trigger upgrade
        assert!(upgrade.would_upgrade(upgrade_mask));
        assert!(upgrade.attempt_upgrade(upgrade_mask, &mut obj));
        assert!(upgrade.is_already_upgraded());
    }
}
