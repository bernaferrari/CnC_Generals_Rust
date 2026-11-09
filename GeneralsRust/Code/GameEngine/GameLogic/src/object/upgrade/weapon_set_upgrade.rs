
use crate::common::{ObjectID, UpgradeMaskType};
use crate::modules::UpgradeModuleInterface;
use crate::object::upgrade::upgrade_module::{
    UpgradeMuxData, mux_can_upgrade, mux_give_self_upgrade_for_object,
};
use game_engine::common::ini::{INI, INIError};
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{Module, ModuleData, ModuleDataHandle, NameKeyType};

/// Wave 448: host-only path has no dual-world factory objects.
#[inline]
fn dual_world_registry_unavailable() -> bool {
    crate::object::registry::OBJECT_REGISTRY.is_empty()
}

/// Module data describing the weapon set upgrade.
#[derive(Debug, Clone)]
pub struct WeaponSetUpgradeModuleData {
    module_tag_name_key: NameKeyType,
    pub upgrade_mux_data: UpgradeMuxData,
}

impl Default for WeaponSetUpgradeModuleData {
    fn default() -> Self {
        Self {
            module_tag_name_key: 0,
            upgrade_mux_data: UpgradeMuxData::default(),
        }
    }
}

impl WeaponSetUpgradeModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        self.upgrade_mux_data.parse_from_ini(ini)
    }
}

crate::impl_legacy_module_data_with_key_field!(WeaponSetUpgradeModuleData, module_tag_name_key);

impl Snapshotable for WeaponSetUpgradeModuleData {
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

/// Upgrade module that changes the weapon set on the owning object.
pub struct WeaponSetUpgrade {
    module_name_key: NameKeyType,
    data: ModuleDataHandle,
    object_id: ObjectID,
    applied: bool,
}

impl WeaponSetUpgrade {
    pub fn new(
        module_name_key: NameKeyType,
        data: ModuleDataHandle,
        object_id: ObjectID,
    ) -> Self {
        assert!(data.is::<WeaponSetUpgradeModuleData>());
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
        self.data.read().get_module_tag_name_key()
    }

    fn get_module_data(&self) -> game_engine::common::thing::module::ModuleDataRead<'_> {
        game_engine::common::thing::module::ModuleDataRead::shared(&self.data)
    }
}

impl Snapshotable for WeaponSetUpgrade {
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

impl UpgradeModuleInterface for WeaponSetUpgrade {
    fn can_upgrade(&mut self, upgrade_mask: UpgradeMaskType) -> bool {
        mux_can_upgrade(&self.data.read().as_any().downcast_ref::<WeaponSetUpgradeModuleData>().expect("WeaponSetUpgrade module data").upgrade_mux_data, self.applied, upgrade_mask)
    }

    fn apply_upgrade(&mut self, _upgrade_mask: UpgradeMaskType) -> bool {
        // Wave 448: empty dual-world → false.
        if dual_world_registry_unavailable() {
            return false;
        }

        if self.applied {
            return false;
        }
        let data = self.data.read().as_any().downcast_ref::<WeaponSetUpgradeModuleData>().expect("WeaponSetUpgrade module data").upgrade_mux_data.clone();
        mux_give_self_upgrade_for_object(&data, self.object_id);
        // Matches C++ WeaponSetUpgrade::upgradeImplementation.
        use crate::object::registry::OBJECT_REGISTRY;
        use crate::weapon::WeaponSetType;

        let Some(()) = OBJECT_REGISTRY.with_object_mut(self.object_id, |object_guard| {
            object_guard.set_weapon_set_flag(WeaponSetType::PlayerUpgrade);
        }) else {
            log::warn!("WeaponSetUpgrade: Object {} not found", self.object_id);
            return false;
        };
        log::debug!(
            "Applied player-upgrade weapon set flag to object {}",
            self.object_id
        );
        self.applied = true;
        true
    }

    fn remove_upgrade(&mut self, upgrade_mask: UpgradeMaskType) {
        // C++ does not clear the flag; resetUpgrade only clears executed.
        let _ = crate::object::upgrade::upgrade_module::mux_reset_upgrade(
            &self.data.read().as_any().downcast_ref::<WeaponSetUpgradeModuleData>().expect("WeaponSetUpgrade module data").upgrade_mux_data,
            &mut self.applied,
            upgrade_mask,
        );
    }
}
