use std::sync::Arc;

use crate::common::{LegacyModuleData, ObjectID, Real, UpgradeMaskType};
use crate::object::upgrade::upgrade_module::{UpgradeMuxData, mux_can_upgrade, mux_reset_upgrade};
use game_engine::common::ini::{FieldParse, INI, INIError};
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{Module, ModuleData, NameKeyType};

/// Module data describing the experience scalar upgrade.
#[derive(Debug, Clone)]
pub struct ExperienceScalarUpgradeModuleData {
    module_tag_name_key: NameKeyType,
    pub upgrade_mux_data: UpgradeMuxData,
    add_xp_scalar: Real,
}

impl Default for ExperienceScalarUpgradeModuleData {
    fn default() -> Self {
        Self {
            module_tag_name_key: 0,
            upgrade_mux_data: UpgradeMuxData::default(),
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

/// Upgrade module that increases XP gain rate on the owning object.
pub struct ExperienceScalarUpgrade {
    module_name_key: NameKeyType,
    data: Arc<ExperienceScalarUpgradeModuleData>,
    applied: bool,
}

impl ExperienceScalarUpgrade {
    pub fn new(
        module_name_key: NameKeyType,
        data: Arc<ExperienceScalarUpgradeModuleData>,
        _object_id: ObjectID,
    ) -> Self {
        Self {
            module_name_key,
            data,
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
        // C++ derived CRC delegates to the base; only UpgradeMux contributes bytes.
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

impl ExperienceScalarUpgrade {
    pub(crate) fn can_upgrade(&self, mask: UpgradeMaskType) -> bool {
        mux_can_upgrade(&self.data.upgrade_mux_data, self.applied, mask)
    }

    /// Release the installed module before synchronous FX, removals, and owner effects.
    pub(crate) fn prepare_upgrade(
        &self,
        mask: UpgradeMaskType,
    ) -> Option<Arc<ExperienceScalarUpgradeModuleData>> {
        self.can_upgrade(mask).then(|| Arc::clone(&self.data))
    }

    pub(crate) fn remove_upgrade(&mut self, mask: UpgradeMaskType) {
        // C++ resetUpgrade only clears execution; it does not undo the added scalar.
        mux_reset_upgrade(&self.data.upgrade_mux_data, &mut self.applied, mask);
    }

    pub(crate) fn finish_upgrade(&mut self) {
        // C++ commits after upgradeImplementation even when no tracker exists.
        self.applied = true;
    }
}

impl ExperienceScalarUpgradeModuleData {
    pub(crate) fn apply_to_object(&self, owner: &mut crate::object::Object) {
        // ExperienceScalarUpgrade.cpp:59–66 adds to this owner's existing scalar.
        let _ = owner.with_experience_tracker_mut(|tracker| {
            tracker.set_experience_scalar(tracker.get_experience_scalar() + self.add_xp_scalar);
        });
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

crate::impl_upgrade_mux_field_parsers!(ExperienceScalarUpgradeModuleData);

const EXPERIENCE_SCALAR_UPGRADE_FIELDS: &[FieldParse<ExperienceScalarUpgradeModuleData>] =
    crate::upgrade_mux_field_table!(FieldParse {
        token: "AddXPScalar",
        parse: parse_add_xp_scalar_field,
    },);

#[cfg(test)]
#[path = "experience_scalar_upgrade/tests.rs"]
mod ownership_tests;
