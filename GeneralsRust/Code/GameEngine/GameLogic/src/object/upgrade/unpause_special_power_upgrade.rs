use std::sync::Arc;

use crate::common::{AsciiString, LegacyModuleData, ObjectID, UpgradeMaskType};
use crate::modules::UpgradeModuleInterface;
use crate::object::special_power_interface_cast::module_special_power_interface;
use crate::object::special_power_template::find_or_create_special_power_template;
use crate::object::{SpecialPowerTemplate, OBJECT_REGISTRY};
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{Module, ModuleData, ModuleInterfaceType, NameKeyType};

/// Module data describing the unpause special power upgrade.
#[derive(Debug, Clone)]
pub struct UnpauseSpecialPowerUpgradeModuleData {
    module_tag_name_key: NameKeyType,
    special_power_template: Option<Arc<SpecialPowerTemplate>>,
}

impl Default for UnpauseSpecialPowerUpgradeModuleData {
    fn default() -> Self {
        Self {
            module_tag_name_key: 0,
            special_power_template: None,
        }
    }
}

impl UnpauseSpecialPowerUpgradeModuleData {
    pub fn special_power_template(&self) -> Option<&Arc<SpecialPowerTemplate>> {
        self.special_power_template.as_ref()
    }

    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, UNPAUSE_SPECIAL_POWER_UPGRADE_FIELDS)
    }
}

crate::impl_legacy_module_data_with_key_field!(
    UnpauseSpecialPowerUpgradeModuleData,
    module_tag_name_key
);

impl Snapshotable for UnpauseSpecialPowerUpgradeModuleData {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 0;
        xfer.xfer_version(&mut version, 1).map_err(|e| e.to_string())?;
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 0;
        xfer.xfer_version(&mut version, 1).map_err(|e| e.to_string())?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

/// Upgrade module that enables/unpauses a special power on the owning object.
pub struct UnpauseSpecialPowerUpgrade {
    module_name_key: NameKeyType,
    data: Arc<UnpauseSpecialPowerUpgradeModuleData>,
    object_id: ObjectID,
    applied: bool,
}

impl UnpauseSpecialPowerUpgrade {
    pub fn new(
        module_name_key: NameKeyType,
        data: Arc<UnpauseSpecialPowerUpgradeModuleData>,
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

impl Module for UnpauseSpecialPowerUpgrade {
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

impl Snapshotable for UnpauseSpecialPowerUpgrade {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 0;
        xfer.xfer_version(&mut version, 1).map_err(|e| e.to_string())?;
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

impl UpgradeModuleInterface for UnpauseSpecialPowerUpgrade {
    fn can_upgrade(&self, _upgrade_mask: UpgradeMaskType) -> bool {
        !self.applied
    }

    fn apply_upgrade(&mut self, _upgrade_mask: UpgradeMaskType) -> bool {
        if self.applied {
            return false;
        }
        let Some(template) = self.data.special_power_template() else {
            log::warn!(
                "UnpauseSpecialPowerUpgrade: Missing SpecialPowerTemplate on object {}",
                self.object_id
            );
            self.applied = true;
            return true;
        };

        let Some(object) = OBJECT_REGISTRY.get_object(self.object_id) else {
            log::warn!(
                "UnpauseSpecialPowerUpgrade: Object {} not found",
                self.object_id
            );
            return false;
        };

        let object_guard = match object.write() {
            Ok(guard) => guard,
            Err(_) => {
                log::error!(
                    "UnpauseSpecialPowerUpgrade: Failed to lock object {}",
                    self.object_id
                );
                return false;
            }
        };

        let mut paused = false;
        let template_name = template.get_name().to_string();
        for module_handle in object_guard.modules_with_interface(ModuleInterfaceType::SPECIAL_POWER)
        {
            module_handle.with_module(|module| {
                let Some(sp_module) = module_special_power_interface(module) else {
                    return;
                };
                if sp_module.get_power_name() == template_name {
                    sp_module.pause_countdown(false);
                    paused = true;
                }
            });
        }

        if !paused {
            for behavior in object_guard.get_behavior_modules() {
                let mut behavior_guard = match behavior.lock() {
                    Ok(guard) => guard,
                    Err(_) => {
                        log::warn!(
                            "UnpauseSpecialPowerUpgrade: Failed to lock behavior on object {}",
                            self.object_id
                        );
                        continue;
                    }
                };

                if let Some(sp_module) = behavior_guard.get_special_power_module_interface() {
                    if sp_module.get_power_name() == template_name {
                        sp_module.pause_countdown(false);
                    }
                }
            }
        }

        self.applied = true;
        true
    }

    fn remove_upgrade(&mut self, _upgrade_mask: UpgradeMaskType) {
        // C++ does not revert this upgrade; keep parity by doing nothing.
    }
}

fn parse_special_power_template_field(
    _ini: &mut INI,
    data: &mut UnpauseSpecialPowerUpgradeModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    if tokens.is_empty() {
        return Err(INIError::InvalidData);
    }
    let name = AsciiString::from(tokens[0]);
    data.special_power_template = Some(find_or_create_special_power_template(&name));
    Ok(())
}

const UNPAUSE_SPECIAL_POWER_UPGRADE_FIELDS: &[FieldParse<UnpauseSpecialPowerUpgradeModuleData>] =
    &[FieldParse {
        token: "SpecialPowerTemplate",
        parse: parse_special_power_template_field,
    }];
