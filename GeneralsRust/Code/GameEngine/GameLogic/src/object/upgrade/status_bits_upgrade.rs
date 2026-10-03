use std::sync::Arc;

use crate::common::{
    AsciiString, LegacyModuleData, ObjectID, ObjectStatusMaskType, UpgradeMaskType,
};
use crate::object::Object;
use crate::object::upgrade::upgrade_module::{UpgradeMuxData, mux_can_upgrade, mux_reset_upgrade};
use game_engine::common::ini::{FieldParse, INI, INIError};
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{Module, ModuleData, NameKeyType};

/// Module data describing the status bits to set/clear when an upgrade is applied.
#[derive(Debug, Clone)]
pub struct StatusBitsUpgradeModuleData {
    module_tag_name_key: NameKeyType,
    pub upgrade_mux_data: UpgradeMuxData,
    status_to_set: ObjectStatusMaskType,
    status_to_clear: ObjectStatusMaskType,
}

impl Default for StatusBitsUpgradeModuleData {
    fn default() -> Self {
        Self {
            module_tag_name_key: 0,
            upgrade_mux_data: UpgradeMuxData::default(),
            status_to_set: ObjectStatusMaskType::none(),
            status_to_clear: ObjectStatusMaskType::none(),
        }
    }
}

impl StatusBitsUpgradeModuleData {
    pub fn status_to_set(&self) -> ObjectStatusMaskType {
        self.status_to_set
    }

    pub fn status_to_clear(&self) -> ObjectStatusMaskType {
        self.status_to_clear
    }

    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, STATUS_BITS_UPGRADE_FIELDS)
    }

    pub fn set_status_to_set_from_tokens(&mut self, tokens: &[&str]) -> Result<(), String> {
        self.status_to_set = parse_status_tokens(tokens)?;
        Ok(())
    }

    pub fn set_status_to_clear_from_tokens(&mut self, tokens: &[&str]) -> Result<(), String> {
        self.status_to_clear = parse_status_tokens(tokens)?;
        Ok(())
    }
}

crate::impl_legacy_module_data_with_key_field!(StatusBitsUpgradeModuleData, module_tag_name_key);

impl Snapshotable for StatusBitsUpgradeModuleData {
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

/// Installed upgrade state. Definitions are immutable; execution belongs to this module.
pub struct StatusBitsUpgrade {
    module_name_key: NameKeyType,
    data: Arc<StatusBitsUpgradeModuleData>,
    applied: bool,
}

impl StatusBitsUpgrade {
    pub fn new(
        module_name_key: NameKeyType,
        data: Arc<StatusBitsUpgradeModuleData>,
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

    /// Release the installed module before synchronous FX, removals, and owner callbacks.
    pub(crate) fn prepare_upgrade(
        &self,
        mask: UpgradeMaskType,
    ) -> Option<Arc<StatusBitsUpgradeModuleData>> {
        self.can_upgrade(mask).then(|| Arc::clone(&self.data))
    }

    pub(crate) fn remove_upgrade(&mut self, mask: UpgradeMaskType) {
        // C++ resetUpgrade only clears execution; it does not undo the effect.
        mux_reset_upgrade(&self.data.upgrade_mux_data, &mut self.applied, mask);
    }

    pub(crate) fn finish_upgrade(&mut self) {
        self.applied = true;
    }
}

impl StatusBitsUpgradeModuleData {
    pub(crate) fn apply_to_object(&self, owner: &mut Object) {
        // C++ writes each module's set then clear masks in authored order.
        // Status writes can invoke object callbacks: hold no upgrade module guard.
        owner.set_status(self.status_to_set, true);
        owner.clear_status(self.status_to_clear);
    }
}

impl Module for StatusBitsUpgrade {
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

impl Snapshotable for StatusBitsUpgrade {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        // C++ base CRCs are empty; UpgradeMux writes its version and executed flag.
        let mut version = 1u8;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| e.to_string())?;
        let mut executed = self.applied;
        xfer.xfer_bool(&mut executed).map_err(|e| e.to_string())
    }
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        crate::object::upgrade::upgrade_module::xfer_upgrade_module_with_version(
            xfer,
            &mut self.applied,
            "StatusBitsUpgrade",
        )
    }
    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

fn parse_status_tokens(tokens: &[&str]) -> Result<ObjectStatusMaskType, String> {
    if tokens.is_empty() {
        return Ok(ObjectStatusMaskType::none());
    }

    let normalized: Vec<&str> = tokens
        .iter()
        .copied()
        .filter(|token| *token != "=")
        .collect();
    ObjectStatusMaskType::parse_tokens(normalized)
}

fn parse_status_to_set_field(
    _ini: &mut INI,
    data: &mut StatusBitsUpgradeModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.set_status_to_set_from_tokens(tokens)
        .map_err(|_| INIError::InvalidData)
}

fn parse_status_to_clear_field(
    _ini: &mut INI,
    data: &mut StatusBitsUpgradeModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.set_status_to_clear_from_tokens(tokens)
        .map_err(|_| INIError::InvalidData)
}

crate::impl_upgrade_mux_field_parsers!(StatusBitsUpgradeModuleData);

const STATUS_BITS_UPGRADE_FIELDS: &[FieldParse<StatusBitsUpgradeModuleData>] = crate::upgrade_mux_field_table!(
    FieldParse {
        token: "StatusToSet",
        parse: parse_status_to_set_field,
    },
    FieldParse {
        token: "StatusToClear",
        parse: parse_status_to_clear_field,
    },
);

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn status_bits_upgrade_data_parses_tokens() {
        let mut data = StatusBitsUpgradeModuleData::default();
        data.set_status_to_set_from_tokens(&["STEALTHED", "DETECTED"])
            .unwrap();
        data.set_status_to_clear_from_tokens(&["+MASKED", "-MASKED"])
            .unwrap();
        assert!(
            data.status_to_set()
                .contains(ObjectStatusMaskType::STEALTHED)
        );
        assert!(
            data.status_to_set()
                .contains(ObjectStatusMaskType::DETECTED)
        );
        assert!(data.status_to_clear().is_empty());
    }
}

#[cfg(test)]
#[path = "status_bits_upgrade/tests.rs"]
mod ownership_tests;
