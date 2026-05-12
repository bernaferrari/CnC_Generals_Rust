use std::sync::Arc;

use crate::common::{LegacyModuleData, ObjectID, Real, UpgradeMaskType};
use crate::modules::UpgradeModuleInterface;
use crate::object::registry::OBJECT_REGISTRY;
use crate::object::INVALID_ID;
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{
    ActiveShroudUpgradeConfig, Module, ModuleData, NameKeyType,
};

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

    fn to_config(&self) -> ActiveShroudUpgradeConfig {
        ActiveShroudUpgradeConfig {
            new_shroud_range: self.new_shroud_range,
        }
    }

    fn from_config(config: ActiveShroudUpgradeConfig, module_tag_name_key: NameKeyType) -> Self {
        Self {
            module_tag_name_key,
            new_shroud_range: config.new_shroud_range,
        }
    }
}

impl LegacyModuleData for ActiveShroudUpgradeModuleData {
    fn set_module_tag_name_key(&mut self, key: NameKeyType) {
        self.module_tag_name_key = key;
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.module_tag_name_key
    }

    fn get_active_shroud_upgrade_config(&self) -> Option<ActiveShroudUpgradeConfig> {
        Some(self.to_config())
    }
}

impl ModuleData for ActiveShroudUpgradeModuleData {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn set_module_tag_name_key(&mut self, key: NameKeyType) {
        LegacyModuleData::set_module_tag_name_key(self, key);
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        LegacyModuleData::get_module_tag_name_key(self)
    }

    fn get_active_shroud_upgrade_config(&self) -> Option<ActiveShroudUpgradeConfig> {
        Some(self.to_config())
    }
}

impl crate::common::types::ModuleData for ActiveShroudUpgradeModuleData {
    fn get_active_shroud_upgrade_config(&self) -> Option<ActiveShroudUpgradeConfig> {
        Some(self.to_config())
    }
}

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

    pub fn from_module_data(
        module_name_key: NameKeyType,
        module_data: Arc<dyn ModuleData>,
        object_id: ObjectID,
    ) -> Option<Self> {
        let config = module_data.get_active_shroud_upgrade_config()?;
        Some(Self::new(
            module_name_key,
            Arc::new(ActiveShroudUpgradeModuleData::from_config(
                config,
                module_data.get_module_tag_name_key(),
            )),
            object_id,
        ))
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
    data.new_shroud_range = INI::parse_real(tokens[0])?;
    Ok(())
}

const ACTIVE_SHROUD_UPGRADE_FIELDS: &[FieldParse<ActiveShroudUpgradeModuleData>] = &[FieldParse {
    token: "NewShroudRange",
    parse: parse_shroud_clearing_range_field,
}];

#[cfg(test)]
mod tests {
    use super::*;
    use game_engine::common::ini::INI;

    #[test]
    fn new_shroud_range_uses_ini_real_parser() {
        let mut data = ActiveShroudUpgradeModuleData::default();
        let mut ini = INI::new();

        parse_shroud_clearing_range_field(&mut ini, &mut data, &["275.5"])
            .expect("real shroud range");

        assert_eq!(data.new_shroud_range(), 275.5);
    }

    #[test]
    fn active_shroud_upgrade_builds_from_typed_config() {
        let mut data = ActiveShroudUpgradeModuleData::default();
        LegacyModuleData::set_module_tag_name_key(&mut data, 0xCAFE);
        data.new_shroud_range = 300.0;

        let module = ActiveShroudUpgrade::from_module_data(0xBEEF, Arc::new(data), 42)
            .expect("active shroud config");

        assert_eq!(module.module_name_key, 0xBEEF);
        assert_eq!(module.object_id, 42);
        assert_eq!(module.data.new_shroud_range(), 300.0);
        assert_eq!(module.get_module_tag_name_key(), 0xCAFE);
    }
}
