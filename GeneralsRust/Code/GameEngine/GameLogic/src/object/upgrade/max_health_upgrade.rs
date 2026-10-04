use std::sync::Arc;

use crate::common::{AsciiString, LegacyModuleData, ObjectID, Real, UpgradeMaskType};
use crate::object::Object;
use crate::object::body::body_module::MaxHealthChangeType;
use crate::object::upgrade::upgrade_module::{UpgradeMuxData, mux_can_upgrade, mux_reset_upgrade};
use game_engine::common::ini::{FieldParse, INI, INIError};
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{Module, ModuleData, NameKeyType};

/// Module data describing the max health increase to apply.
#[derive(Debug, Clone)]
pub struct MaxHealthUpgradeModuleData {
    module_tag_name_key: NameKeyType,
    pub upgrade_mux_data: UpgradeMuxData,
    add_max_health: Real,
    change_type: MaxHealthChangeType,
}

impl Default for MaxHealthUpgradeModuleData {
    fn default() -> Self {
        Self {
            module_tag_name_key: 0,
            upgrade_mux_data: UpgradeMuxData::default(),
            add_max_health: 0.0,
            change_type: MaxHealthChangeType::SameCurrentHealth,
        }
    }
}

impl MaxHealthUpgradeModuleData {
    pub fn add_max_health(&self) -> Real {
        self.add_max_health
    }

    pub fn change_type(&self) -> MaxHealthChangeType {
        self.change_type
    }

    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, MAX_HEALTH_UPGRADE_FIELDS)
    }

    pub fn set_add_max_health(&mut self, value: Real) {
        self.add_max_health = value;
    }

    pub fn set_change_type(&mut self, type_str: &str) -> Result<(), String> {
        self.change_type = match type_str.to_uppercase().as_str() {
            "SAME_CURRENTHEALTH" => MaxHealthChangeType::SameCurrentHealth,
            "PRESERVE_RATIO" => MaxHealthChangeType::PreserveRatio,
            "ADD_CURRENT_HEALTH_TOO" => MaxHealthChangeType::AddCurrentHealthToo,
            "FULLY_HEAL" => MaxHealthChangeType::FullyHeal,
            _ => return Err(format!("Unknown change type: {}", type_str)),
        };
        Ok(())
    }
}

crate::impl_legacy_module_data_with_key_field!(MaxHealthUpgradeModuleData, module_tag_name_key);

impl Snapshotable for MaxHealthUpgradeModuleData {
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

/// Installed mutable state belongs only to this module; definitions remain immutable.
pub struct MaxHealthUpgrade {
    module_name_key: NameKeyType,
    data: Arc<MaxHealthUpgradeModuleData>,
    applied: bool,
}

impl MaxHealthUpgradeModuleData {
    /// C++ MaxHealthUpgrade.cpp:56-68. The caller already borrows the actual owner.
    pub(crate) fn apply_to_object(&self, owner: &mut Object) {
        owner
            .add_body_max_health_with_owner(self.add_max_health, self.change_type)
            .expect("max health upgrade could not update body");
    }
}

impl MaxHealthUpgrade {
    pub fn new(
        module_name_key: NameKeyType,
        data: Arc<MaxHealthUpgradeModuleData>,
        _object_id: ObjectID,
    ) -> Self {
        Self {
            module_name_key,
            data,
            applied: false,
        }
    }
    pub(crate) fn can_upgrade(&self, mask: UpgradeMaskType) -> bool {
        mux_can_upgrade(&self.data.upgrade_mux_data, self.applied, mask)
    }
    pub(crate) fn prepare_upgrade(
        &self,
        mask: UpgradeMaskType,
    ) -> Option<Arc<MaxHealthUpgradeModuleData>> {
        self.can_upgrade(mask).then(|| Arc::clone(&self.data))
    }
    pub(crate) fn remove_upgrade(&mut self, mask: UpgradeMaskType) {
        // C++ resetUpgrade only resets execution; the health addition is not undone.
        mux_reset_upgrade(&self.data.upgrade_mux_data, &mut self.applied, mask);
    }
    pub(crate) fn finish_upgrade(&mut self) {
        // C++ commits execution even when the object has no BodyModule.
        self.applied = true;
    }
}

impl Module for MaxHealthUpgrade {
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

impl Snapshotable for MaxHealthUpgrade {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        crate::object::upgrade::upgrade_module::crc_upgrade_module_state(xfer, self.applied)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        crate::object::upgrade::upgrade_module::xfer_upgrade_module_with_version(
            xfer,
            &mut self.applied,
            "MaxHealthUpgrade",
        )
    }
    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

fn parse_add_max_health_field(
    _ini: &mut INI,
    data: &mut MaxHealthUpgradeModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    if tokens.is_empty() {
        return Err(INIError::InvalidData);
    }
    data.add_max_health = tokens[0]
        .parse::<Real>()
        .map_err(|_| INIError::InvalidData)?;
    Ok(())
}

fn parse_change_type_field(
    _ini: &mut INI,
    data: &mut MaxHealthUpgradeModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    if tokens.is_empty() {
        return Err(INIError::InvalidData);
    }
    data.set_change_type(tokens[0])
        .map_err(|_| INIError::InvalidData)
}

crate::impl_upgrade_mux_field_parsers!(MaxHealthUpgradeModuleData);

const MAX_HEALTH_UPGRADE_FIELDS: &[FieldParse<MaxHealthUpgradeModuleData>] = crate::upgrade_mux_field_table!(
    FieldParse {
        token: "AddMaxHealth",
        parse: parse_add_max_health_field,
    },
    FieldParse {
        token: "ChangeType",
        parse: parse_change_type_field,
    },
);

#[cfg(test)]
mod installed_tests;
