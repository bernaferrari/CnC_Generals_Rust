use std::sync::Arc;

use crate::common::{Bool, LegacyModuleData, ObjectID, UpgradeMaskType};
use crate::modules::UpgradeModuleInterface;
use crate::object::registry::OBJECT_REGISTRY;
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{Module, ModuleData, NameKeyType, RadarUpgradeConfig};

/// Module data describing the radar upgrade.
#[derive(Debug, Clone)]
pub struct RadarUpgradeModuleData {
    module_tag_name_key: NameKeyType,
    is_disable_proof: Bool,
}

impl Default for RadarUpgradeModuleData {
    fn default() -> Self {
        Self {
            module_tag_name_key: 0,
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
    object_id: ObjectID,
    applied: bool,
}

impl RadarUpgrade {
    pub fn new(
        module_name_key: NameKeyType,
        data: Arc<RadarUpgradeModuleData>,
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

    fn apply_radar_upgrade(&mut self) -> Result<(), String> {
        let disable_proof = self.data.is_disable_proof();
        let object_id = self.object_id;
        match OBJECT_REGISTRY.with_object_mut(self.object_id, |object_guard| {
            if let Some(player) = object_guard.get_controlling_player() {
                if let Ok(mut player_guard) = player.write() {
                    player_guard.add_radar(disable_proof);
                }
            }

            if let Some(radar_module) = object_guard.find_update_module("RadarUpdate") {
                radar_module.with_module(|module| {
                    if let Some(radar_update) = module.get_radar_update_interface() {
                        radar_update.extend_radar();
                    }
                });
            }
        }) {
            Some(()) => Ok(()),
            None => Err(format!(
                "RadarUpgrade could not find object {} in registry",
                object_id
            )),
        }
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

impl UpgradeModuleInterface for RadarUpgrade {
    fn can_upgrade(&self, _upgrade_mask: UpgradeMaskType) -> bool {
        !self.applied
    }

    fn apply_upgrade(&mut self, _upgrade_mask: UpgradeMaskType) -> bool {
        if self.applied {
            return false;
        }
        if self.apply_radar_upgrade().is_ok() {
            self.applied = true;
            true
        } else {
            false
        }
    }

    fn remove_upgrade(&mut self, _upgrade_mask: UpgradeMaskType) {
        // C++ doesn't remove radar on upgrade removal; only on delete/capture.
    }

    fn on_delete(&mut self, object: &mut crate::object::Object) {
        if !self.applied || object.is_disabled() {
            return;
        }

        if let Some(player) = object.get_controlling_player() {
            if let Ok(mut player_guard) = player.write() {
                player_guard.remove_radar(self.data.is_disable_proof());
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

const RADAR_UPGRADE_FIELDS: &[FieldParse<RadarUpgradeModuleData>] = &[FieldParse {
    token: "DisableProof",
    parse: parse_disable_proof_field,
}];

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
        assert_eq!(module.object_id, 42);
        assert!(module.is_disable_proof());
        assert_eq!(module.get_module_tag_name_key(), 0xCAFE);
    }
}
