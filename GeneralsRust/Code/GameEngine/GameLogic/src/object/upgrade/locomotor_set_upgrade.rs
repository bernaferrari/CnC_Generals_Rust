use std::sync::Arc;

use crate::common::{LegacyModuleData, ObjectID, UpgradeMaskType};
use crate::modules::UpgradeModuleInterface;
use crate::object::upgrade::upgrade_module::{
    UpgradeMuxData, mux_can_upgrade, mux_give_self_upgrade_for_object,
};
use game_engine::common::ini::{INI, INIError};
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{Module, ModuleData, NameKeyType};

use crate::helpers::TheGameLogic;
/// Module data describing the locomotor set upgrade.
#[derive(Debug, Clone)]
pub struct LocomotorSetUpgradeModuleData {
    module_tag_name_key: NameKeyType,
    pub upgrade_mux_data: UpgradeMuxData,
}

impl Default for LocomotorSetUpgradeModuleData {
    fn default() -> Self {
        Self {
            module_tag_name_key: 0,
            upgrade_mux_data: UpgradeMuxData::default(),
        }
    }
}

impl LocomotorSetUpgradeModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        self.upgrade_mux_data.parse_from_ini(ini)
    }
}

crate::impl_legacy_module_data_with_key_field!(LocomotorSetUpgradeModuleData, module_tag_name_key);

impl Snapshotable for LocomotorSetUpgradeModuleData {
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
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 0;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        crate::object::upgrade::upgrade_module::xfer_upgrade_module_with_version(
            xfer,
            &mut self.applied,
            std::any::type_name::<Self>(),
        )
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

impl UpgradeModuleInterface for LocomotorSetUpgrade {
    fn can_upgrade(&self, upgrade_mask: UpgradeMaskType) -> bool {
        mux_can_upgrade(&self.data.upgrade_mux_data, self.applied, upgrade_mask)
    }

    fn apply_upgrade(&mut self, _upgrade_mask: UpgradeMaskType) -> bool {
        if self.applied {
            return false;
        }
        mux_give_self_upgrade_for_object(&self.data.upgrade_mux_data, self.object_id);

        // C++ LocomotorSetUpgrade::upgradeImplementation: getObject()->getAIUpdateInterface()
        // Live host objects live on TheGameLogic (registry may be empty).
        let object = TheGameLogic::find_object_by_id(self.object_id)
            .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(self.object_id));

        let Some(object) = object else {
            log::warn!("LocomotorSetUpgrade: Object {} not found", self.object_id);
            return false;
        };

        if let Ok(object_guard) = object.read() {
            if let Some(ai) = object_guard.get_ai_update_interface() {
                if let Ok(mut ai_guard) = ai.lock() {
                    let _ = ai_guard.set_locomotor_upgrade(true);
                }
            }
        }

        self.applied = true;
        true
    }

    fn remove_upgrade(&mut self, upgrade_mask: UpgradeMaskType) {
        // C++ does not clear the locomotor flag; resetUpgrade only clears executed.
        let _ = crate::object::upgrade::upgrade_module::mux_reset_upgrade(
            &self.data.upgrade_mux_data,
            &mut self.applied,
            upgrade_mask,
        );
    }
}
