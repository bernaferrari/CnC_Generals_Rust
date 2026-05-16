use std::sync::Arc;

use crate::common::{LegacyModuleData, ObjectID, UpgradeMaskType, WeaponBonusConditionType};
use crate::modules::UpgradeModuleInterface;
use game_engine::common::ini::{INIError, INI};
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{Module, ModuleData, NameKeyType};

/// Module data describing the weapon bonus upgrade.
#[derive(Debug, Clone)]
pub struct WeaponBonusUpgradeModuleData {
    module_tag_name_key: NameKeyType,
}

impl Default for WeaponBonusUpgradeModuleData {
    fn default() -> Self {
        Self {
            module_tag_name_key: 0,
        }
    }
}

impl WeaponBonusUpgradeModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        let _ = ini;
        Ok(())
    }
}

crate::impl_legacy_module_data_with_key_field!(WeaponBonusUpgradeModuleData, module_tag_name_key);

impl Snapshotable for WeaponBonusUpgradeModuleData {
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

/// Upgrade module that increases weapon damage on the owning object.
pub struct WeaponBonusUpgrade {
    module_name_key: NameKeyType,
    data: Arc<WeaponBonusUpgradeModuleData>,
    object_id: ObjectID,
    applied: bool,
}

impl WeaponBonusUpgrade {
    pub fn new(
        module_name_key: NameKeyType,
        data: Arc<WeaponBonusUpgradeModuleData>,
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

impl Module for WeaponBonusUpgrade {
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

impl Snapshotable for WeaponBonusUpgrade {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|err| format!("{:?} crc version: {err:?}", std::any::type_name::<Self>()))?;
        crate::object::upgrade::upgrade_module::crc_upgrade_module_state(xfer, self.applied)
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

impl UpgradeModuleInterface for WeaponBonusUpgrade {
    fn can_upgrade(&self, _upgrade_mask: UpgradeMaskType) -> bool {
        !self.applied
    }

    fn apply_upgrade(&mut self, _upgrade_mask: UpgradeMaskType) -> bool {
        if self.applied {
            return false;
        }
        // Apply weapon damage bonus to object
        // Matches C++ WeaponBonusUpgrade::upgradeImplementation from WeaponBonusUpgrade.cpp lines 62-69
        use crate::object::registry::OBJECT_REGISTRY;

        let Some(object) = OBJECT_REGISTRY.get_object(self.object_id) else {
            log::warn!("WeaponBonusUpgrade: Object {} not found", self.object_id);
            return false;
        };

        let mut object_guard = match object.write() {
            Ok(guard) => guard,
            Err(_) => {
                log::error!(
                    "WeaponBonusUpgrade: Failed to lock object {}",
                    self.object_id
                );
                return false;
            }
        };

        // C++ code: obj->setWeaponBonusCondition(WEAPONBONUSCONDITION_PLAYER_UPGRADE);
        object_guard.set_weapon_bonus_condition(WeaponBonusConditionType::PlayerUpgrade);

        self.applied = true;
        true
    }

    fn remove_upgrade(&mut self, _upgrade_mask: UpgradeMaskType) {
        // C++ does not clear the weapon bonus condition here; keep parity.
    }
}
