use std::sync::Arc;

use crate::common::{LegacyModuleData, ObjectID, UpgradeMaskType};
use crate::modules::UpgradeModuleInterface;
use game_engine::common::ini::{INIError, INI};
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{Module, ModuleData, NameKeyType};

/// Module data describing the locomotor set upgrade.
#[derive(Debug, Clone)]
pub struct LocomotorSetUpgradeModuleData {
    module_tag_name_key: NameKeyType,
}

impl Default for LocomotorSetUpgradeModuleData {
    fn default() -> Self {
        Self {
            module_tag_name_key: 0,
        }
    }
}

impl LocomotorSetUpgradeModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        let _ = ini;
        Ok(())
    }
}

crate::impl_legacy_module_data_with_key_field!(LocomotorSetUpgradeModuleData, module_tag_name_key);

impl Snapshotable for LocomotorSetUpgradeModuleData {
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

/// Upgrade module that changes locomotor (movement type) on the owning object.
pub struct LocomotorSetUpgrade {
    module_name_key: NameKeyType,
    data: Arc<LocomotorSetUpgradeModuleData>,
    object_id: ObjectID,
    applied: bool,
}

impl LocomotorSetUpgrade {
    pub fn new(
        module_name_key: NameKeyType,
        data: Arc<LocomotorSetUpgradeModuleData>,
        object_id: ObjectID,
    ) -> Self {
        Self {
            module_name_key,
            data,
            object_id,
            applied: false,
        }
    }
}

impl Module for LocomotorSetUpgrade {
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
        LegacyModuleData::get_module_tag_name_key(self.data.as_ref())
    }

    fn get_module_data(&self) -> &dyn ModuleData {
        self.data.as_ref()
    }
}

impl Snapshotable for LocomotorSetUpgrade {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 1;
        let _ = _xfer.xfer_version(&mut version, 1);
        let mut applied = self.applied;
        let _ = _xfer.xfer_bool(&mut applied);
        self.applied = applied;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

impl UpgradeModuleInterface for LocomotorSetUpgrade {
    fn can_upgrade(&self, _upgrade_mask: UpgradeMaskType) -> bool {
        !self.applied
    }

    fn apply_upgrade(&mut self, _upgrade_mask: UpgradeMaskType) -> bool {
        if self.applied {
            return false;
        }
        use crate::object::registry::OBJECT_REGISTRY;

        let Some(object) = OBJECT_REGISTRY.get_object(self.object_id) else {
            log::warn!("LocomotorSetUpgrade: Object {} not found", self.object_id);
            return false;
        };

        let object_guard = match object.read() {
            Ok(guard) => guard,
            Err(_) => {
                log::error!(
                    "LocomotorSetUpgrade: Failed to lock object {}",
                    self.object_id
                );
                return false;
            }
        };

        if let Some(ai) = object_guard.get_ai_update_interface() {
            if let Ok(mut ai_guard) = ai.lock() {
                let _ = ai_guard.set_locomotor_upgrade(true);
            }
        }

        self.applied = true;
        true
    }

    fn remove_upgrade(&mut self, _upgrade_mask: UpgradeMaskType) {
        // C++ does not clear the locomotor upgrade flag; keep parity.
    }
}
