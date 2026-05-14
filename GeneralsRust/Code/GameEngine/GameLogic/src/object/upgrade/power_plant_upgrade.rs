use std::sync::Arc;

use crate::common::{LegacyModuleData, ObjectID, UpgradeMaskType};
use crate::modules::UpgradeModuleInterface;
use crate::object::registry::OBJECT_REGISTRY;
use game_engine::common::ini::{INIError, INI};
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{Module, ModuleData, NameKeyType};

/// Module data describing the power plant upgrade.
#[derive(Debug, Clone)]
pub struct PowerPlantUpgradeModuleData {
    module_tag_name_key: NameKeyType,
}

impl Default for PowerPlantUpgradeModuleData {
    fn default() -> Self {
        Self {
            module_tag_name_key: 0,
        }
    }
}

impl PowerPlantUpgradeModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        let _ = ini;
        Ok(())
    }
}

crate::impl_legacy_module_data_with_key_field!(PowerPlantUpgradeModuleData, module_tag_name_key);

impl Snapshotable for PowerPlantUpgradeModuleData {
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
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
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
        if !self.applied {
            return Ok(());
        }

        use crate::object::registry::OBJECT_REGISTRY;

        let Some(object) = OBJECT_REGISTRY.get_object(self.object_id) else {
            return Ok(());
        };

        if let Ok(object_guard) = object.read() {
            if let Some(player) = object_guard.get_controlling_player() {
                if let Ok(mut player_guard) = player.write() {
                    player_guard.add_power_bonus(self.object_id);
                }
            }
        }

        Ok(())
    }
}

impl UpgradeModuleInterface for PowerPlantUpgrade {
    fn can_upgrade(&self, _upgrade_mask: UpgradeMaskType) -> bool {
        !self.applied
    }

    fn apply_upgrade(&mut self, _upgrade_mask: UpgradeMaskType) -> bool {
        if self.applied {
            return false;
        }
        let Some(object) = OBJECT_REGISTRY.get_object(self.object_id) else {
            log::warn!("PowerPlantUpgrade: Object {} not found", self.object_id);
            return false;
        };

        let object_guard = match object.write() {
            Ok(guard) => guard,
            Err(_) => {
                log::error!(
                    "PowerPlantUpgrade: Failed to lock object {}",
                    self.object_id
                );
                return false;
            }
        };

        if let Some(player) = object_guard.get_controlling_player() {
            if let Ok(mut player_guard) = player.write() {
                player_guard.add_power_bonus(self.object_id);
            }
        }

        let _ = object_guard.with_power_plant_update_interface(|ppui| {
            ppui.extend_rods(true);
        });

        self.applied = true;
        true
    }

    fn remove_upgrade(&mut self, _upgrade_mask: UpgradeMaskType) {
        // C++ does not remove the power bonus on upgrade removal; only on delete/capture.
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
