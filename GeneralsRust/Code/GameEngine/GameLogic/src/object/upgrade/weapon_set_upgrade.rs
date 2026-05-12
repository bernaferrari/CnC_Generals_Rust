use std::sync::Arc;

use crate::common::{LegacyModuleData, ObjectID, UpgradeMaskType};
use crate::modules::UpgradeModuleInterface;
use game_engine::common::ini::{INIError, INI};
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{Module, ModuleData, NameKeyType};

/// Module data describing the weapon set upgrade.
#[derive(Debug, Clone)]
pub struct WeaponSetUpgradeModuleData {
    module_tag_name_key: NameKeyType,
}

impl Default for WeaponSetUpgradeModuleData {
    fn default() -> Self {
        Self {
            module_tag_name_key: 0,
        }
    }
}

impl WeaponSetUpgradeModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        let _ = ini;
        Ok(())
    }
}

crate::impl_legacy_module_data_with_key_field!(WeaponSetUpgradeModuleData, module_tag_name_key);

impl Snapshotable for WeaponSetUpgradeModuleData {
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

/// Upgrade module that changes the weapon set on the owning object.
pub struct WeaponSetUpgrade {
    module_name_key: NameKeyType,
    data: Arc<WeaponSetUpgradeModuleData>,
    object_id: ObjectID,
    applied: bool,
}

impl WeaponSetUpgrade {
    pub fn new(
        module_name_key: NameKeyType,
        data: Arc<WeaponSetUpgradeModuleData>,
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

impl Module for WeaponSetUpgrade {
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

impl Snapshotable for WeaponSetUpgrade {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        crate::object::upgrade::upgrade_module::xfer_upgrade_module_with_version(
            _xfer,
            &mut self.applied,
            std::any::type_name::<Self>(),
        )
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

impl UpgradeModuleInterface for WeaponSetUpgrade {
    fn can_upgrade(&self, _upgrade_mask: UpgradeMaskType) -> bool {
        !self.applied
    }

    fn apply_upgrade(&mut self, _upgrade_mask: UpgradeMaskType) -> bool {
        if self.applied {
            return false;
        }
        // Matches C++ WeaponSetUpgrade::upgradeImplementation.
        use crate::object::registry::OBJECT_REGISTRY;
        use crate::weapon::WeaponSetType;

        let Some(object) = OBJECT_REGISTRY.get_object(self.object_id) else {
            log::warn!("WeaponSetUpgrade: Object {} not found", self.object_id);
            return false;
        };

        let mut object_guard = match object.write() {
            Ok(guard) => guard,
            Err(_) => {
                log::error!("WeaponSetUpgrade: Failed to lock object {}", self.object_id);
                return false;
            }
        };

        object_guard.set_weapon_set_flag(WeaponSetType::PlayerUpgrade);
        log::debug!(
            "Applied player-upgrade weapon set flag to object {}",
            self.object_id
        );
        self.applied = true;
        true
    }

    fn remove_upgrade(&mut self, _upgrade_mask: UpgradeMaskType) {
        // C++ does not clear the flag; keep parity by doing nothing.
    }
}
