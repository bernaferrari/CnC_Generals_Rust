use std::sync::Arc;

use crate::common::{AsciiString, LegacyModuleData, ObjectID, UpgradeMaskType};
use crate::modules::UpgradeModuleInterface;
use crate::object::registry::OBJECT_REGISTRY;
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::rts::{get_science_store, ScienceType, SCIENCE_INVALID};
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{Module, ModuleData, NameKeyType};

/// Module data describing the science to grant when the upgrade activates.
#[derive(Debug, Clone)]
pub struct GrantScienceUpgradeModuleData {
    module_tag_name_key: NameKeyType,
    grant_science_name: AsciiString,
}

impl Default for GrantScienceUpgradeModuleData {
    fn default() -> Self {
        Self {
            module_tag_name_key: 0,
            grant_science_name: AsciiString::default(),
        }
    }
}

impl GrantScienceUpgradeModuleData {
    pub fn grant_science_name(&self) -> &AsciiString {
        &self.grant_science_name
    }

    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, GRANT_SCIENCE_UPGRADE_FIELDS)
    }
}

crate::impl_legacy_module_data_with_key_field!(GrantScienceUpgradeModuleData, module_tag_name_key);

impl Snapshotable for GrantScienceUpgradeModuleData {
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

/// Upgrade module that grants a science to the owning player.
pub struct GrantScienceUpgrade {
    module_name_key: NameKeyType,
    data: Arc<GrantScienceUpgradeModuleData>,
    object_id: ObjectID,
    science_type: ScienceType,
    applied: bool,
}

impl GrantScienceUpgrade {
    pub fn new(
        module_name_key: NameKeyType,
        data: Arc<GrantScienceUpgradeModuleData>,
        object_id: ObjectID,
    ) -> Self {
        Self {
            module_name_key,
            data,
            object_id,
            science_type: SCIENCE_INVALID,
            applied: false,
        }
    }

    fn resolve_science_type(&mut self) {
        if self.science_type != SCIENCE_INVALID {
            return;
        }
        let Some(store) = get_science_store() else {
            log::warn!("GrantScienceUpgrade: Science store not initialized");
            return;
        };
        self.science_type =
            store.get_science_from_internal_name(self.data.grant_science_name().as_str());
        if self.science_type == SCIENCE_INVALID {
            log::warn!(
                "GrantScienceUpgrade: Unknown science '{}'",
                self.data.grant_science_name()
            );
        }
    }
}

impl Module for GrantScienceUpgrade {
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

impl Snapshotable for GrantScienceUpgrade {
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

impl UpgradeModuleInterface for GrantScienceUpgrade {
    fn can_upgrade(&self, _upgrade_mask: UpgradeMaskType) -> bool {
        !self.applied
    }

    fn apply_upgrade(&mut self, _upgrade_mask: UpgradeMaskType) -> bool {
        if self.applied {
            return false;
        }
        self.resolve_science_type();
        if self.science_type == SCIENCE_INVALID {
            // C++ still considers the upgrade executed even if the science name is invalid.
            self.applied = true;
            return true;
        }

        let Some(object) = OBJECT_REGISTRY.get_object(self.object_id) else {
            log::warn!("GrantScienceUpgrade: Object {} not found", self.object_id);
            self.applied = true;
            return true;
        };

        let object_guard = match object.read() {
            Ok(guard) => guard,
            Err(_) => {
                log::error!(
                    "GrantScienceUpgrade: Failed to lock object {}",
                    self.object_id
                );
                self.applied = true;
                return true;
            }
        };

        if let Some(player) = object_guard.get_controlling_player() {
            if let Ok(mut player_guard) = player.write() {
                player_guard.grant_science(self.science_type);
            }
        }

        // C++ marks upgrade executed even if player is missing; keep parity.
        self.applied = true;
        true
    }

    fn remove_upgrade(&mut self, _upgrade_mask: UpgradeMaskType) {
        // C++ does not remove granted science.
    }
}

fn parse_grant_science_field(
    _ini: &mut INI,
    data: &mut GrantScienceUpgradeModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    if tokens.is_empty() {
        return Err(INIError::InvalidData);
    }
    data.grant_science_name = AsciiString::from(tokens[0]);
    Ok(())
}

const GRANT_SCIENCE_UPGRADE_FIELDS: &[FieldParse<GrantScienceUpgradeModuleData>] = &[FieldParse {
    token: "GrantScience",
    parse: parse_grant_science_field,
}];
