use std::sync::Arc;

use crate::common::{Bool, LegacyModuleData, ObjectID, UpgradeMaskType};
use crate::object::upgrade::upgrade_module::{UpgradeMuxData, mux_can_upgrade};
use game_engine::common::ini::{FieldParse, INI, INIError};
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{Module, ModuleData, NameKeyType, RadarUpgradeConfig};

/// Module data describing the radar upgrade.
#[derive(Debug, Clone)]
pub struct RadarUpgradeModuleData {
    module_tag_name_key: NameKeyType,
    pub upgrade_mux_data: UpgradeMuxData,
    is_disable_proof: Bool,
}

impl Default for RadarUpgradeModuleData {
    fn default() -> Self {
        Self {
            module_tag_name_key: 0,
            upgrade_mux_data: UpgradeMuxData::default(),
            is_disable_proof: false,
        }
    }
}

impl RadarUpgradeModuleData {
    pub fn is_disable_proof(&self) -> Bool {
        self.is_disable_proof
    }

    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, RADAR_UPGRADE_FIELDS)
    }

    fn to_config(&self) -> RadarUpgradeConfig {
        RadarUpgradeConfig {
            is_disable_proof: self.is_disable_proof,
        }
    }

    fn from_config(config: RadarUpgradeConfig, module_tag_name_key: NameKeyType) -> Self {
        Self {
            module_tag_name_key,
            upgrade_mux_data: UpgradeMuxData::default(),
            is_disable_proof: config.is_disable_proof,
        }
    }
}

impl LegacyModuleData for RadarUpgradeModuleData {
    fn set_module_tag_name_key(&mut self, key: NameKeyType) {
        self.module_tag_name_key = key;
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.module_tag_name_key
    }

    fn get_radar_upgrade_config(&self) -> Option<RadarUpgradeConfig> {
        Some(self.to_config())
    }
}

impl ModuleData for RadarUpgradeModuleData {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn set_module_tag_name_key(&mut self, key: NameKeyType) {
        LegacyModuleData::set_module_tag_name_key(self, key);
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        LegacyModuleData::get_module_tag_name_key(self)
    }

    fn get_radar_upgrade_config(&self) -> Option<RadarUpgradeConfig> {
        Some(self.to_config())
    }
}

impl crate::common::types::ModuleData for RadarUpgradeModuleData {
    fn get_radar_upgrade_config(&self) -> Option<RadarUpgradeConfig> {
        Some(self.to_config())
    }
}

impl Snapshotable for RadarUpgradeModuleData {
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

/// Upgrade module that grants radar to the owning player's UI.
pub struct RadarUpgrade {
    module_name_key: NameKeyType,
    data: Arc<RadarUpgradeModuleData>,
    applied: bool,
}

impl RadarUpgrade {
    pub fn new(
        module_name_key: NameKeyType,
        data: Arc<RadarUpgradeModuleData>,
        _object_id: ObjectID,
    ) -> Self {
        Self {
            module_name_key,
            data,
            applied: false,
        }
    }

    pub fn is_applied(&self) -> bool {
        self.applied
    }

    pub fn is_disable_proof(&self) -> Bool {
        self.data.is_disable_proof()
    }

    pub fn from_module_data(
        module_name_key: NameKeyType,
        module_data: Arc<dyn ModuleData>,
        object_id: ObjectID,
    ) -> Option<Self> {
        let config = module_data.get_radar_upgrade_config()?;
        Some(Self::new(
            module_name_key,
            Arc::new(RadarUpgradeModuleData::from_config(
                config,
                module_data.get_module_tag_name_key(),
            )),
            object_id,
        ))
    }

    pub(crate) fn prepare_upgrade(
        &self,
        mask: UpgradeMaskType,
    ) -> Option<Arc<RadarUpgradeModuleData>> {
        mux_can_upgrade(&self.data.upgrade_mux_data, self.applied, mask)
            .then(|| Arc::clone(&self.data))
    }

    pub(crate) fn finish_upgrade(&mut self) {
        self.applied = true;
    }

    pub(crate) fn enabled_applied_radar_proof(&self, disabled: bool) -> Option<bool> {
        (self.applied && !disabled).then(|| self.data.is_disable_proof())
    }

    pub(crate) fn set_applied_owned(&mut self, applied: bool) {
        self.applied = applied;
    }

    // Removing the upgrade does nothing; cleanup happens on delete/capture.
}

impl Module for RadarUpgrade {
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

impl Snapshotable for RadarUpgrade {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
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

impl RadarUpgrade {
    pub(crate) fn can_upgrade(&mut self, upgrade_mask: UpgradeMaskType) -> bool {
        mux_can_upgrade(&self.data.upgrade_mux_data, self.applied, upgrade_mask)
    }

    pub(crate) fn remove_upgrade(&mut self, upgrade_mask: UpgradeMaskType) {
        // C++ doesn't remove radar; resetUpgrade only clears executed.
        let _ = crate::object::upgrade::upgrade_module::mux_reset_upgrade(
            &self.data.upgrade_mux_data,
            &mut self.applied,
            upgrade_mask,
        );
    }

    pub(crate) fn on_delete(&mut self, object: &mut crate::object::Object) {
        if !self.applied || object.is_disabled() {
            return;
        }

        object.with_controlling_player_mut(|player_guard| {
            player_guard.remove_radar(self.data.is_disable_proof());
        });

        self.applied = false;
    }

    pub(crate) fn on_capture(
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
                player_guard.remove_radar(self.data.is_disable_proof());
                self.applied = false;
            }
        }

        if let Some(new_owner) = new_owner {
            if let Ok(mut player_guard) = new_owner.write() {
                player_guard.add_radar(self.data.is_disable_proof());
                self.applied = true;
            }
        }
    }
}

fn parse_disable_proof_field(
    _ini: &mut INI,
    data: &mut RadarUpgradeModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    if tokens.is_empty() {
        return Err(INIError::InvalidData);
    }
    data.is_disable_proof = INI::parse_bool(tokens[0])?;
    Ok(())
}

crate::impl_upgrade_mux_field_parsers!(RadarUpgradeModuleData);

const RADAR_UPGRADE_FIELDS: &[FieldParse<RadarUpgradeModuleData>] =
    crate::upgrade_mux_field_table!(FieldParse {
        token: "DisableProof",
        parse: parse_disable_proof_field,
    },);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::LegacyModuleData;
    use game_engine::common::ini::INI;

    #[test]
    fn disable_proof_uses_ini_bool_parser() {
        let mut data = RadarUpgradeModuleData::default();
        let mut ini = INI::new();

        parse_disable_proof_field(&mut ini, &mut data, &["yes"]).expect("bool disable proof");

        assert!(data.is_disable_proof());
    }

    #[test]
    fn radar_upgrade_builds_from_typed_config() {
        let mut data = RadarUpgradeModuleData::default();
        LegacyModuleData::set_module_tag_name_key(&mut data, 0xCAFE);
        data.is_disable_proof = true;

        let module =
            RadarUpgrade::from_module_data(0xBEEF, Arc::new(data), 42).expect("radar config");

        assert_eq!(module.module_name_key, 0xBEEF);
        assert!(module.is_disable_proof());
        assert_eq!(module.get_module_tag_name_key(), 0xCAFE);
    }
}
