use std::sync::Arc;

use crate::common::{LegacyModuleData, ObjectID, Real, UpgradeMaskType};
use crate::modules::UpgradeModuleInterface;
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{Module, ModuleData, NameKeyType};

/// Module data describing the experience scalar upgrade.
#[derive(Debug, Clone)]
pub struct ExperienceScalarUpgradeModuleData {
    module_tag_name_key: NameKeyType,
    add_xp_scalar: Real,
}

impl Default for ExperienceScalarUpgradeModuleData {
    fn default() -> Self {
        Self {
            module_tag_name_key: 0,
            add_xp_scalar: 0.0,
        }
    }
}

impl ExperienceScalarUpgradeModuleData {
    pub fn add_xp_scalar(&self) -> Real {
        self.add_xp_scalar
    }

    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, EXPERIENCE_SCALAR_UPGRADE_FIELDS)
    }
}

crate::impl_legacy_module_data_with_key_field!(
    ExperienceScalarUpgradeModuleData,
    module_tag_name_key
);

impl Snapshotable for ExperienceScalarUpgradeModuleData {
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

/// Upgrade module that increases XP gain rate on the owning object.
pub struct ExperienceScalarUpgrade {
    module_name_key: NameKeyType,
    data: Arc<ExperienceScalarUpgradeModuleData>,
    object_id: ObjectID,
    applied: bool,
}

impl ExperienceScalarUpgrade {
    pub fn new(
        module_name_key: NameKeyType,
        data: Arc<ExperienceScalarUpgradeModuleData>,
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

impl Module for ExperienceScalarUpgrade {
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

impl Snapshotable for ExperienceScalarUpgrade {
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

impl UpgradeModuleInterface for ExperienceScalarUpgrade {
    fn can_upgrade(&self, _upgrade_mask: UpgradeMaskType) -> bool {
        !self.applied
    }

    fn apply_upgrade(&mut self, _upgrade_mask: UpgradeMaskType) -> bool {
        if self.applied {
            return false;
        }
        use crate::object::registry::OBJECT_REGISTRY;

        let Some(object) = OBJECT_REGISTRY.get_object(self.object_id) else {
            log::warn!(
                "ExperienceScalarUpgrade: Object {} not found",
                self.object_id
            );
            return false;
        };

        let object_guard = match object.read() {
            Ok(guard) => guard,
            Err(_) => {
                log::error!(
                    "ExperienceScalarUpgrade: Failed to lock object {}",
                    self.object_id
                );
                return false;
            }
        };

        let Some(tracker) = object_guard.get_experience_tracker() else {
            self.applied = true;
            return true;
        };

        if let Ok(mut tracker_guard) = tracker.lock() {
            let current_scalar = tracker_guard.get_experience_scalar();
            tracker_guard.set_experience_scalar(current_scalar + self.data.add_xp_scalar());
        }

        self.applied = true;
        true
    }

    fn remove_upgrade(&mut self, _upgrade_mask: UpgradeMaskType) {
        // C++ does not remove the added XP scalar; keep parity.
    }
}

fn parse_add_xp_scalar_field(
    _ini: &mut INI,
    data: &mut ExperienceScalarUpgradeModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    if tokens.is_empty() {
        return Err(INIError::InvalidData);
    }
    data.add_xp_scalar = tokens[0]
        .parse::<Real>()
        .map_err(|_| INIError::InvalidData)?;
    Ok(())
}

const EXPERIENCE_SCALAR_UPGRADE_FIELDS: &[FieldParse<ExperienceScalarUpgradeModuleData>] =
    &[FieldParse {
        token: "AddXPScalar",
        parse: parse_add_xp_scalar_field,
    }];
