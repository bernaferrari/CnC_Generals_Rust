use std::sync::Arc;

use crate::common::{LegacyModuleData, ObjectID, UpgradeMaskType};
use crate::modules::UpgradeModuleInterface;
use crate::object::registry::OBJECT_REGISTRY;
use crate::object::upgrade::upgrade_module::{
    UpgradeMuxData, mux_can_upgrade, mux_give_self_upgrade_for_object,
};
use game_engine::common::ini::{INI, INIError};
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{Module, ModuleData, NameKeyType};

/// Wave 437: host-only path has no dual-world factory objects.
#[inline]
fn dual_world_registry_unavailable() -> bool {
    crate::object::registry::OBJECT_REGISTRY.is_empty()
}

/// Module data describing the power plant upgrade.
#[derive(Debug, Clone)]
pub struct PowerPlantUpgradeModuleData {
    module_tag_name_key: NameKeyType,
    pub upgrade_mux_data: UpgradeMuxData,
}

impl Default for PowerPlantUpgradeModuleData {
    fn default() -> Self {
        Self {
            module_tag_name_key: 0,
            upgrade_mux_data: UpgradeMuxData::default(),
        }
    }
}

impl PowerPlantUpgradeModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        self.upgrade_mux_data.parse_from_ini(ini)
    }
}

crate::impl_legacy_module_data_with_key_field!(PowerPlantUpgradeModuleData, module_tag_name_key);

impl Snapshotable for PowerPlantUpgradeModuleData {
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

/// Upgrade module that increases power output on power plant buildings.
pub struct PowerPlantUpgrade {
    module_name_key: NameKeyType,
    data: Arc<PowerPlantUpgradeModuleData>,
    object_id: ObjectID,
    applied: bool,
}

impl PowerPlantUpgrade {
    pub fn new(
        module_name_key: NameKeyType,
        data: Arc<PowerPlantUpgradeModuleData>,
        object_id: ObjectID,
    ) -> Self {
        Self {
            module_name_key,
            data,
            object_id,
            applied: false,
        }
    }

    pub fn is_applied(&self) -> bool {
        self.applied
    }
}

impl Module for PowerPlantUpgrade {
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

impl Snapshotable for PowerPlantUpgrade {
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
        // Wave 437: empty dual-world → Ok(()).
        if dual_world_registry_unavailable() {
            return Ok(());
        }

        if !self.applied {
            return Ok(());
        }

        use crate::object::registry::OBJECT_REGISTRY;

        let object_id = self.object_id;
        let _ = OBJECT_REGISTRY.with_object(self.object_id, |object_guard| {
            if let Some(player) = object_guard.get_controlling_player() {
                if let Ok(mut player_guard) = player.write() {
                    player_guard.add_power_bonus(object_id);
                }
            }
        });

        Ok(())
    }
}

impl UpgradeModuleInterface for PowerPlantUpgrade {
    fn can_upgrade(&self, upgrade_mask: UpgradeMaskType) -> bool {
        mux_can_upgrade(&self.data.upgrade_mux_data, self.applied, upgrade_mask)
    }

    fn apply_upgrade(&mut self, _upgrade_mask: UpgradeMaskType) -> bool {
        // Wave 437: empty dual-world → false.
        if dual_world_registry_unavailable() {
            return false;
        }

        if self.applied {
            return false;
        }
        mux_give_self_upgrade_for_object(&self.data.upgrade_mux_data, self.object_id);
        let object_id = self.object_id;
        let Some(()) = OBJECT_REGISTRY.with_object_mut(self.object_id, |object_guard| {
            if let Some(player) = object_guard.get_controlling_player() {
                if let Ok(mut player_guard) = player.write() {
                    player_guard.add_power_bonus(object_id);
                }
            }

            let _ = object_guard.with_power_plant_update_interface(|ppui| {
                ppui.extend_rods(true);
            });
        }) else {
            log::warn!("PowerPlantUpgrade: Object {} not found", self.object_id);
            return false;
        };

        self.applied = true;
        true
    }

    fn remove_upgrade(&mut self, upgrade_mask: UpgradeMaskType) {
        // C++ does not remove the power bonus; resetUpgrade only clears executed.
        let _ = crate::object::upgrade::upgrade_module::mux_reset_upgrade(
            &self.data.upgrade_mux_data,
            &mut self.applied,
            upgrade_mask,
        );
    }

    fn on_delete(&mut self, object: &mut crate::object::Object) {
        if !self.applied {
            return;
        }

        if let Some(player) = object.get_controlling_player() {
            if let Ok(mut player_guard) = player.write() {
                player_guard.remove_power_bonus(self.object_id);
            }
        }

        self.applied = false;
    }

    fn on_capture(
        &mut self,
        object: &mut crate::object::Object,
        old_owner: Option<&Arc<std::sync::RwLock<crate::player::Player>>>,
        new_owner: Option<&Arc<std::sync::RwLock<crate::player::Player>>>,
    ) {
        if !self.applied || object.is_disabled() {
            return;
        }

        if let Some(old_owner) = old_owner {
            if let Ok(mut player_guard) = old_owner.write() {
                player_guard.remove_power_bonus(self.object_id);
                self.applied = false;
            }
        }

        if let Some(new_owner) = new_owner {
            if let Ok(mut player_guard) = new_owner.write() {
                player_guard.add_power_bonus(self.object_id);
                self.applied = true;
            }
        }
    }
}
