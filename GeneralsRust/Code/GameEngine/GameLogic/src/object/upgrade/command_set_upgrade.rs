use std::sync::Arc;

use crate::common::{AsciiString, LegacyModuleData, ObjectID, UpgradeMaskType};
use crate::modules::UpgradeModuleInterface;
use crate::object::upgrade::upgrade_module::{
    UpgradeMuxData, mux_can_upgrade, mux_give_self_upgrade_for_object,
};
use game_engine::common::ini::{FieldParse, INI, INIError};
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{Module, ModuleData, NameKeyType};

/// Module data describing the command set upgrade.
#[derive(Debug, Clone)]
pub struct CommandSetUpgradeModuleData {
    module_tag_name_key: NameKeyType,
    pub upgrade_mux_data: UpgradeMuxData,
    command_set_name: AsciiString,
    command_set_alt: AsciiString,
    trigger_alt: AsciiString,
}

impl Default for CommandSetUpgradeModuleData {
    fn default() -> Self {
        Self {
            module_tag_name_key: 0,
            upgrade_mux_data: UpgradeMuxData::default(),
            command_set_name: AsciiString::new(),
            command_set_alt: AsciiString::new(),
            trigger_alt: AsciiString::new(),
        }
    }
}

impl CommandSetUpgradeModuleData {
    pub fn command_set_name(&self) -> &AsciiString {
        &self.command_set_name
    }

    pub fn command_set_alt(&self) -> &AsciiString {
        &self.command_set_alt
    }

    pub fn trigger_alt(&self) -> &AsciiString {
        &self.trigger_alt
    }

    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, COMMAND_SET_UPGRADE_FIELDS)
    }
}

crate::impl_legacy_module_data_with_key_field!(CommandSetUpgradeModuleData, module_tag_name_key);

impl Snapshotable for CommandSetUpgradeModuleData {
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

/// Upgrade module that changes available commands on the owning object.
pub struct CommandSetUpgrade {
    module_name_key: NameKeyType,
    data: Arc<CommandSetUpgradeModuleData>,
    object_id: ObjectID,
    applied: bool,
}

impl CommandSetUpgrade {
    pub fn new(
        module_name_key: NameKeyType,
        data: Arc<CommandSetUpgradeModuleData>,
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

impl Module for CommandSetUpgrade {
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

impl Snapshotable for CommandSetUpgrade {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 0;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| e.to_string())?;
        Ok(())
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

impl UpgradeModuleInterface for CommandSetUpgrade {
    fn can_upgrade(&self, upgrade_mask: UpgradeMaskType) -> bool {
        mux_can_upgrade(&self.data.upgrade_mux_data, self.applied, upgrade_mask)
    }

    fn apply_upgrade(&mut self, _upgrade_mask: UpgradeMaskType) -> bool {
        if self.applied {
            return false;
        }
        mux_give_self_upgrade_for_object(&self.data.upgrade_mux_data, self.object_id);
        use crate::helpers::TheGameLogic;
        use crate::object::registry::OBJECT_REGISTRY;
        use crate::upgrade::center::with_upgrade_center;

        let trigger_alt = self.data.trigger_alt().clone();
        let command_set_alt = self.data.command_set_alt().clone();
        let command_set_name = self.data.command_set_name().clone();

        let apply_to = |object_guard: &mut crate::object::Object| {
            let mut use_alt = false;
            if !trigger_alt.is_empty() {
                let upgrade =
                    with_upgrade_center(|center| center.find_upgrade(trigger_alt.as_str()));
                if let Some(template) = upgrade {
                    let mask_bits = UpgradeMaskType::from_bits_retain(template.mask().bits());

                    if let Some(player) = object_guard.get_controlling_player() {
                        if let Ok(player_guard) = player.read() {
                            if player_guard
                                .get_completed_upgrade_mask()
                                .intersects(mask_bits)
                            {
                                use_alt = true;
                            }
                        }
                    }

                    if !use_alt && object_guard.completed_upgrades().intersects(mask_bits) {
                        use_alt = true;
                    }
                }
            }

            if use_alt {
                object_guard.set_command_set_string_override(&command_set_alt);
            } else {
                object_guard.set_command_set_string_override(&command_set_name);
            }
            use_alt
        };

        let found = OBJECT_REGISTRY
            .with_object_mut(self.object_id, |object_guard| apply_to(object_guard))
            .or_else(|| {
                TheGameLogic::find_object_by_id(self.object_id)
                    .and_then(|obj| obj.write().ok().map(|mut guard| apply_to(&mut guard)))
            });
        if found.is_none() {
            log::warn!("CommandSetUpgrade: Object {} not found", self.object_id);
            return false;
        }
        crate::control_bar::mark_ui_dirty();

        self.applied = true;
        true
    }

    fn remove_upgrade(&mut self, upgrade_mask: UpgradeMaskType) {
        // C++ leaves the override; resetUpgrade only clears executed.
        let _ = crate::object::upgrade::upgrade_module::mux_reset_upgrade(
            &self.data.upgrade_mux_data,
            &mut self.applied,
            upgrade_mask,
        );
    }
}

fn parse_command_set_field(
    _ini: &mut INI,
    data: &mut CommandSetUpgradeModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    if tokens.is_empty() {
        return Err(INIError::InvalidData);
    }
    data.command_set_name = AsciiString::from(tokens[0]);
    Ok(())
}

fn parse_command_set_alt_field(
    _ini: &mut INI,
    data: &mut CommandSetUpgradeModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    if tokens.is_empty() {
        return Err(INIError::InvalidData);
    }
    data.command_set_alt = AsciiString::from(tokens[0]);
    Ok(())
}

fn parse_trigger_alt_field(
    _ini: &mut INI,
    data: &mut CommandSetUpgradeModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    if tokens.is_empty() {
        return Err(INIError::InvalidData);
    }
    data.trigger_alt = AsciiString::from(tokens[0]);
    Ok(())
}

crate::impl_upgrade_mux_field_parsers!(CommandSetUpgradeModuleData);

const COMMAND_SET_UPGRADE_FIELDS: &[FieldParse<CommandSetUpgradeModuleData>] = crate::upgrade_mux_field_table!(
    FieldParse {
        token: "CommandSet",
        parse: parse_command_set_field,
    },
    FieldParse {
        token: "CommandSetAlt",
        parse: parse_command_set_alt_field,
    },
    FieldParse {
        token: "TriggerAlt",
        parse: parse_trigger_alt_field,
    },
);
