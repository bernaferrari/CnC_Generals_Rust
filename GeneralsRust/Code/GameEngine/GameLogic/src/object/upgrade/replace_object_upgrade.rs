use std::sync::Arc;

use crate::ai::THE_AI;
use crate::common::{AsciiString, LegacyModuleData, ObjectID, UpgradeMaskType};
use crate::modules::UpgradeModuleInterface;
use crate::object::OBJECT_REGISTRY;
use crate::{helpers::TheThingFactory, object_manager::get_object_manager};
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{Module, ModuleData, NameKeyType};

/// Module data describing the replace object upgrade.
#[derive(Debug, Clone)]
pub struct ReplaceObjectUpgradeModuleData {
    module_tag_name_key: NameKeyType,
    replace_object_name: AsciiString,
}

impl Default for ReplaceObjectUpgradeModuleData {
    fn default() -> Self {
        Self {
            module_tag_name_key: 0,
            replace_object_name: AsciiString::new(),
        }
    }
}

impl ReplaceObjectUpgradeModuleData {
    pub fn replace_object_name(&self) -> &AsciiString {
        &self.replace_object_name
    }

    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, REPLACE_OBJECT_UPGRADE_FIELDS)
    }
}

crate::impl_legacy_module_data_with_key_field!(ReplaceObjectUpgradeModuleData, module_tag_name_key);

impl Snapshotable for ReplaceObjectUpgradeModuleData {
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

/// Upgrade module that replaces the object with a different type (transformation).
pub struct ReplaceObjectUpgrade {
    module_name_key: NameKeyType,
    data: Arc<ReplaceObjectUpgradeModuleData>,
    object_id: ObjectID,
    applied: bool,
}

impl ReplaceObjectUpgrade {
    pub fn new(
        module_name_key: NameKeyType,
        data: Arc<ReplaceObjectUpgradeModuleData>,
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

impl Module for ReplaceObjectUpgrade {
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

impl Snapshotable for ReplaceObjectUpgrade {
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

impl UpgradeModuleInterface for ReplaceObjectUpgrade {
    fn can_upgrade(&self, _upgrade_mask: UpgradeMaskType) -> bool {
        !self.applied
    }

    fn apply_upgrade(&mut self, _upgrade_mask: UpgradeMaskType) -> bool {
        if self.applied {
            return false;
        }
        let replace_name = self.data.replace_object_name();
        if replace_name.is_empty() {
            log::warn!(
                "ReplaceObjectUpgrade: Missing ReplaceObject name for object {}",
                self.object_id
            );
            return false;
        }

        let Some(object) = OBJECT_REGISTRY.get_object(self.object_id) else {
            log::warn!("ReplaceObjectUpgrade: Object {} not found", self.object_id);
            return false;
        };

        let (transform, position, team_opt, was_structure) = {
            let Ok(object_guard) = object.read() else {
                log::error!(
                    "ReplaceObjectUpgrade: Failed to lock object {}",
                    self.object_id
                );
                return false;
            };
            (
                object_guard.get_transform_matrix(),
                *object_guard.get_position(),
                object_guard.get_team(),
                object_guard.is_structure(),
            )
        };

        let Some(replacement_template) = TheThingFactory::find_template(replace_name.as_str())
        else {
            log::error!(
                "ReplaceObjectUpgrade: No such object '{}' for object {}",
                replace_name.as_str(),
                self.object_id
            );
            return false;
        };

        if let Ok(ai_guard) = THE_AI.read() {
            if let Some(pathfinder) = ai_guard.pathfinder() {
                if let Ok(mut pf) = pathfinder.write() {
                    if was_structure {
                        if let Ok(object_guard) = object.read() {
                            pf.remove_wall_from_object(&object_guard);
                        }
                    } else {
                        pf.remove_object_from_map(self.object_id, &[position]);
                    }
                }
            }
        }

        if let Ok(mut manager) = get_object_manager().write() {
            manager.destroy_object(self.object_id);
        }

        let factory = match TheThingFactory::get() {
            Ok(factory) => factory,
            Err(err) => {
                log::error!(
                    "ReplaceObjectUpgrade: ThingFactory unavailable for object {}: {}",
                    self.object_id,
                    err
                );
                return false;
            }
        };

        let replacement_object = match team_opt.as_ref().and_then(|team| team.read().ok()) {
            Some(team_guard) => factory.new_object(replacement_template, &*team_guard),
            None => factory.new_object_optional_team(replacement_template, None),
        };

        let replacement_object = match replacement_object {
            Ok(obj) => obj,
            Err(err) => {
                log::error!(
                    "ReplaceObjectUpgrade: Failed to create replacement for object {}: {}",
                    self.object_id,
                    err
                );
                return false;
            }
        };

        if let Ok(mut replacement_guard) = replacement_object.write() {
            replacement_guard.set_transform_matrix(&transform);
        }

        let replacement_id = match replacement_object.read() {
            Ok(guard) => guard.get_id(),
            Err(_) => {
                log::error!(
                    "ReplaceObjectUpgrade: Failed to read replacement object for {}",
                    self.object_id
                );
                return false;
            }
        };

        if let Ok(manager) = get_object_manager().write() {
            if let Some(instance) = manager.get_object(replacement_id) {
                if let Ok(mut instance_guard) = instance.write() {
                    instance_guard.set_position(position);
                }
            }
        }

        if let Ok(ai_guard) = THE_AI.read() {
            if let Some(pathfinder) = ai_guard.pathfinder() {
                if let Ok(mut pf) = pathfinder.write() {
                    if let Ok(replacement_guard) = replacement_object.read() {
                        if replacement_guard.is_structure() {
                            pf.create_wall_from_object(&replacement_guard);
                        } else {
                            pf.add_object_to_map(replacement_id, &[position], false);
                        }
                    }
                }
            }
        }

        if let Ok(mut replacement_guard) = replacement_object.write() {
            replacement_guard.on_build_complete();
        }

        if let Ok(replacement_guard) = replacement_object.read() {
            if let Some(player) = replacement_guard.get_controlling_player() {
                if let Ok(mut player_guard) = player.write() {
                    player_guard.on_structure_construction_complete(
                        Some(&object),
                        &replacement_object,
                        false,
                    );
                }
            }
        }

        self.applied = true;
        true
    }

    fn remove_upgrade(&mut self, _upgrade_mask: UpgradeMaskType) {
        // C++ does not revert this upgrade; keep parity by doing nothing.
    }
}

fn parse_replace_object_field(
    _ini: &mut INI,
    data: &mut ReplaceObjectUpgradeModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    if tokens.is_empty() {
        return Err(INIError::InvalidData);
    }
    data.replace_object_name = AsciiString::from(tokens[0]);
    Ok(())
}

const REPLACE_OBJECT_UPGRADE_FIELDS: &[FieldParse<ReplaceObjectUpgradeModuleData>] =
    &[FieldParse {
        token: "ReplaceObject",
        parse: parse_replace_object_field,
    }];
