use std::sync::Arc;

use crate::common::{LegacyModuleData, ObjectID, Real, UpgradeMaskType};
use crate::modules::UpgradeModuleInterface;
use crate::object::registry::OBJECT_REGISTRY;
use crate::object::INVALID_ID;
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{Module, ModuleData, NameKeyType};

/// Module data describing the active shroud upgrade.
#[derive(Debug, Clone)]
pub struct ActiveShroudUpgradeModuleData {
    module_tag_name_key: NameKeyType,
    new_shroud_range: Real,
}

impl Default for ActiveShroudUpgradeModuleData {
    fn default() -> Self {
        Self {
            module_tag_name_key: 0,
            new_shroud_range: 0.0,
        }
    }
}

impl ActiveShroudUpgradeModuleData {
    pub fn new_shroud_range(&self) -> Real {
        self.new_shroud_range
    }

    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, ACTIVE_SHROUD_UPGRADE_FIELDS)
    }
}

crate::impl_legacy_module_data_with_key_field!(ActiveShroudUpgradeModuleData, module_tag_name_key);

impl Snapshotable for ActiveShroudUpgradeModuleData {
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

/// Upgrade module that enables or increases shroud clearing capability.
pub struct ActiveShroudUpgrade {
    module_name_key: NameKeyType,
    data: Arc<ActiveShroudUpgradeModuleData>,
    object_id: ObjectID,
    applied: bool,
}

impl ActiveShroudUpgrade {
    pub fn new(
        module_name_key: NameKeyType,
        data: Arc<ActiveShroudUpgradeModuleData>,
        object_id: ObjectID,
    ) -> Self {
        Self {
            module_name_key,
            data,
            object_id,
            applied: false,
        }
    }

    fn apply_shroud_upgrade(&mut self) -> Result<(), String> {
        let Some(object) = OBJECT_REGISTRY.get_object(self.object_id) else {
            return Err(format!(
                "ActiveShroudUpgrade could not find object {} in registry",
                self.object_id
            ));
        };

        let mut object = object
            .write()
            .map_err(|_| "ActiveShroudUpgrade failed to lock object for writing".to_string())?;

        object.set_shroud_range(self.data.new_shroud_range());
        object.handle_partition_cell_maintenance();

        Ok(())
    }
}

impl Module for ActiveShroudUpgrade {
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

impl Snapshotable for ActiveShroudUpgrade {
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

impl UpgradeModuleInterface for ActiveShroudUpgrade {
    fn can_upgrade(&self, _upgrade_mask: UpgradeMaskType) -> bool {
        !self.applied
    }

    fn apply_upgrade(&mut self, _upgrade_mask: UpgradeMaskType) -> bool {
        if self.applied {
            return false;
        }
        if self.apply_shroud_upgrade().is_ok() {
            self.applied = true;
            true
        } else {
            false
        }
    }

    fn remove_upgrade(&mut self, _upgrade_mask: UpgradeMaskType) {
        // C++ does not revert shroud range on removal; keep parity.
    }
}

fn parse_shroud_clearing_range_field(
    _ini: &mut INI,
    data: &mut ActiveShroudUpgradeModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    if tokens.is_empty() {
        return Err(INIError::InvalidData);
    }
    data.new_shroud_range = tokens[0]
        .parse::<Real>()
        .map_err(|_| INIError::InvalidData)?;
    Ok(())
}

const ACTIVE_SHROUD_UPGRADE_FIELDS: &[FieldParse<ActiveShroudUpgradeModuleData>] = &[FieldParse {
    token: "NewShroudRange",
    parse: parse_shroud_clearing_range_field,
}];
