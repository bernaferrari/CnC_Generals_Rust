use std::sync::Arc;

use crate::common::{Bool, LegacyModuleData, ObjectID, UpgradeMaskType};
use crate::modules::UpgradeModuleInterface;
use crate::object::registry::OBJECT_REGISTRY;
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{Module, ModuleData, NameKeyType};

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
}

crate::impl_legacy_module_data_with_key_field!(RadarUpgradeModuleData, module_tag_name_key);

impl Snapshotable for RadarUpgradeModuleData {
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

    fn apply_radar_upgrade(&mut self) -> Result<(), String> {
        let Some(object) = OBJECT_REGISTRY.get_object(self.object_id) else {
            return Err(format!(
                "RadarUpgrade could not find object {} in registry",
                self.object_id
            ));
        };

        let object_guard = object
            .write()
            .map_err(|_| "RadarUpgrade failed to lock object for writing".to_string())?;

        let disable_proof = self.data.is_disable_proof();

        if let Some(player) = object_guard.get_controlling_player() {
            if let Ok(mut player_guard) = player.write() {
                player_guard.add_radar(disable_proof);
            }
        }

        if let Some(radar_module) = object_guard.find_update_module("RadarUpdate") {
            let _ = radar_module.with_module_downcast::<crate::object::behavior::radar_update::RadarUpdateModule, _, _>(|module| {
                module.behavior_mut().extend_radar();
            });
        }

        Ok(())
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
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 1;
        let _ = _xfer.xfer_version(&mut version, 1);
        crate::object::upgrade::upgrade_module::xfer_upgrade_module_state(
            _xfer,
            &mut self.applied,
        )?;
        Ok(())
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
