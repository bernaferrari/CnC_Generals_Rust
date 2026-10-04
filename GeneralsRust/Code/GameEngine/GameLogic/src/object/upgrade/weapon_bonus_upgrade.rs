use std::sync::Arc;

use crate::common::{LegacyModuleData, ObjectID, UpgradeMaskType, WeaponBonusConditionType};
use crate::object::upgrade::upgrade_module::{UpgradeMuxData, mux_can_upgrade, mux_reset_upgrade};
use game_engine::common::ini::{INI, INIError};
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{Module, ModuleData, NameKeyType};

/// Module data describing the weapon bonus upgrade.
#[derive(Debug, Clone)]
pub struct WeaponBonusUpgradeModuleData {
    module_tag_name_key: NameKeyType,
    pub upgrade_mux_data: UpgradeMuxData,
}

impl Default for WeaponBonusUpgradeModuleData {
    fn default() -> Self {
        Self {
            module_tag_name_key: 0,
            upgrade_mux_data: UpgradeMuxData::default(),
        }
    }
}

impl WeaponBonusUpgradeModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        self.upgrade_mux_data.parse_from_ini(ini)
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
    applied: bool,
}

impl WeaponBonusUpgrade {
    pub fn new(
        module_name_key: NameKeyType,
        data: Arc<WeaponBonusUpgradeModuleData>,
        _object_id: ObjectID,
    ) -> Self {
        Self {
            module_name_key,
            data,
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

impl WeaponBonusUpgrade {
    pub(crate) fn can_upgrade(&self, mask: UpgradeMaskType) -> bool {
        mux_can_upgrade(&self.data.upgrade_mux_data, self.applied, mask)
    }

    /// Retain immutable rules while releasing the installed module before callbacks.
    pub(crate) fn prepare_upgrade(
        &self,
        mask: UpgradeMaskType,
    ) -> Option<Arc<WeaponBonusUpgradeModuleData>> {
        self.can_upgrade(mask).then(|| Arc::clone(&self.data))
    }

    pub(crate) fn remove_upgrade(&mut self, mask: UpgradeMaskType) {
        // C++ resetUpgrade clears execution without undoing the owner bonus flag.
        mux_reset_upgrade(&self.data.upgrade_mux_data, &mut self.applied, mask);
    }

    pub(crate) fn finish_upgrade(&mut self) {
        // UpgradeMux commits only after FX, removals, and upgradeImplementation.
        self.applied = true;
    }
}

impl WeaponBonusUpgradeModuleData {
    pub(crate) fn apply_to_object(&self, owner: &mut crate::object::Object) {
        // WeaponBonusUpgrade.cpp:62–69 acts on its exact Object. The setter
        // preserves Object.cpp:4650–4659's change-only weapon notifications.
        owner.set_weapon_bonus_condition(WeaponBonusConditionType::PlayerUpgrade);
    }
}

#[cfg(test)]
#[path = "weapon_bonus_upgrade/tests.rs"]
mod ownership_tests;
