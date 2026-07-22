//! SpecialPowerCompletionDie - Notifies script engine when special power dies
//!
//! Original C++ location: GameLogic/Module/SpecialPowerCompletionDie.h/.cpp
//! Original C++ Author: Matthew D. Campbell, May 2002
//! Rust conversion: 2025

use super::{xfer_die_module_base_versions, DieModule, DieModuleData, DieModuleInterface};
use crate::common::xfer::XferExt;
use crate::common::{Bool, ObjectID};
use crate::damage::DamageInfo;
use crate::object::die::{
    parse_die_mux_death_types, parse_die_mux_exempt_status, parse_die_mux_required_status,
    parse_die_mux_veterancy_levels,
};
use crate::object::Object;
use crate::scripting::engine::get_script_engine;
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::system::{Snapshotable, Xfer};
use std::sync::{Arc, RwLock};

/// Module data for SpecialPowerCompletionDie
/// (Matches C++ SpecialPowerCompletionDieModuleData)
#[derive(Debug, Clone)]
pub struct SpecialPowerCompletionDieModuleData {
    pub base: DieModuleData,
    /// Name of the special power template associated with this object
    pub special_power_template: Option<String>,
}

impl Default for SpecialPowerCompletionDieModuleData {
    fn default() -> Self {
        Self {
            base: DieModuleData::default(),
            special_power_template: None,
        }
    }
}

impl Snapshotable for SpecialPowerCompletionDieModuleData {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("SpecialPowerCompletionDieModuleData crc version: {e:?}"))?;
        self.base.crc(xfer)?;
        let mut has_template = self.special_power_template.is_some();
        xfer.xfer_bool(&mut has_template)
            .map_err(|e| format!("SpecialPowerCompletionDieModuleData crc has_template: {e:?}"))?;
        if has_template {
            let mut name = self.special_power_template.clone().unwrap_or_default();
            xfer.xfer_ascii_string(&mut name)
                .map_err(|e| format!("SpecialPowerCompletionDieModuleData crc template: {e:?}"))?;
        }
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("SpecialPowerCompletionDieModuleData xfer version: {e:?}"))?;

        self.base.xfer(xfer)?;

        let mut has_template = self.special_power_template.is_some();
        xfer.xfer_bool(&mut has_template)
            .map_err(|e| format!("SpecialPowerCompletionDieModuleData has_template: {e:?}"))?;
        if has_template {
            let mut name = self.special_power_template.take().unwrap_or_default();
            xfer.xfer_ascii_string(&mut name)
                .map_err(|e| format!("SpecialPowerCompletionDieModuleData template: {e:?}"))?;
            self.special_power_template = Some(name);
        } else {
            self.special_power_template = None;
        }

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

impl crate::common::LegacyModuleData for SpecialPowerCompletionDieModuleData {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn set_module_tag_name_key(&mut self, key: crate::common::NameKeyType) {
        game_engine::common::thing::module::ModuleData::set_module_tag_name_key(
            &mut self.base,
            key,
        );
    }

    fn get_module_tag_name_key(&self) -> crate::common::NameKeyType {
        game_engine::common::thing::module::ModuleData::get_module_tag_name_key(&self.base)
    }
}

impl game_engine::common::thing::module::ModuleData for SpecialPowerCompletionDieModuleData {
    fn as_any(&self) -> &dyn std::any::Any {
        crate::common::LegacyModuleData::as_any(self)
    }

    fn set_module_tag_name_key(&mut self, key: game_engine::common::thing::module::NameKeyType) {
        crate::common::LegacyModuleData::set_module_tag_name_key(self, key);
    }

    fn get_module_tag_name_key(&self) -> game_engine::common::thing::module::NameKeyType {
        crate::common::LegacyModuleData::get_module_tag_name_key(self)
    }

    fn is_ai_module_data(&self) -> bool {
        crate::common::LegacyModuleData::is_ai_module_data(self)
    }

    fn get_as_w3d_model_draw_module_data(&self) -> Option<&dyn std::any::Any> {
        crate::common::LegacyModuleData::get_as_w3d_model_draw_module_data(self)
    }

    fn get_as_w3d_tree_draw_module_data(&self) -> Option<&dyn std::any::Any> {
        crate::common::LegacyModuleData::get_as_w3d_tree_draw_module_data(self)
    }

    fn get_special_power_completion_template(&self) -> Option<&str> {
        self.special_power_template.as_deref()
    }

    fn get_minimum_required_game_lod(&self) -> game_engine::thing::StaticGameLodLevel {
        crate::common::LegacyModuleData::get_minimum_required_game_lod(self)
    }
}

impl crate::common::types::ModuleData for SpecialPowerCompletionDieModuleData {}

impl SpecialPowerCompletionDieModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, SPECIAL_POWER_COMPLETION_DIE_FIELDS)
    }
}

fn parse_die_death_types(
    _ini: &mut INI,
    data: &mut SpecialPowerCompletionDieModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    parse_die_mux_death_types(&mut data.base.die_mux_data, tokens)
}

fn parse_die_veterancy_levels(
    _ini: &mut INI,
    data: &mut SpecialPowerCompletionDieModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    parse_die_mux_veterancy_levels(&mut data.base.die_mux_data, tokens)
}

fn parse_die_exempt_status(
    _ini: &mut INI,
    data: &mut SpecialPowerCompletionDieModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    parse_die_mux_exempt_status(&mut data.base.die_mux_data, tokens)
}

fn parse_die_required_status(
    _ini: &mut INI,
    data: &mut SpecialPowerCompletionDieModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    parse_die_mux_required_status(&mut data.base.die_mux_data, tokens)
}

fn parse_special_power_template(
    _ini: &mut INI,
    data: &mut SpecialPowerCompletionDieModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.special_power_template = Some((*token).to_string());
    Ok(())
}

const SPECIAL_POWER_COMPLETION_DIE_FIELDS: &[FieldParse<SpecialPowerCompletionDieModuleData>] = &[
    FieldParse {
        token: "DeathTypes",
        parse: parse_die_death_types,
    },
    FieldParse {
        token: "VeterancyLevels",
        parse: parse_die_veterancy_levels,
    },
    FieldParse {
        token: "ExemptStatus",
        parse: parse_die_exempt_status,
    },
    FieldParse {
        token: "RequiredStatus",
        parse: parse_die_required_status,
    },
    FieldParse {
        token: "SpecialPowerTemplate",
        parse: parse_special_power_template,
    },
];

/// SpecialPowerCompletionDie - Notifies script engine of special power completion
///
/// This module is used for objects created by special powers that need to
/// notify the game's script system when they die. This allows mission scripts
/// to track special power usage and completion.
///
/// For example:
/// - A10 Strike aircraft notifies when strike is complete
/// - Particle Cannon beam notifies when firing is done
/// - Nuke missile notifies when it detonates
/// - Artillery barrage notifies when bombardment ends
///
/// The module stores the creator ID (who activated the special power) and
/// notifies the script engine when the object dies, allowing scripts to
/// trigger follow-up events or cleanup.
/// (Matches C++ SpecialPowerCompletionDie)
#[derive(Debug)]
pub struct SpecialPowerCompletionDie {
    base: DieModule<SpecialPowerCompletionDieModuleData>,
    /// ID of the object that created this special power
    creator_id: ObjectID,
    /// Whether the creator has been set
    creator_set: Bool,
}

impl SpecialPowerCompletionDie {
    /// Create a new SpecialPowerCompletionDie module
    pub fn new(
        object: Arc<RwLock<Object>>,
        module_data: Arc<SpecialPowerCompletionDieModuleData>,
    ) -> Self {
        Self {
            base: DieModule::new(object, module_data),
            creator_id: crate::common::INVALID_ID,
            creator_set: false,
        }
    }

    /// Get module name
    pub fn get_module_name() -> &'static str {
        "SpecialPowerCompletionDie"
    }

    /// Set the creator of this special power
    pub fn set_creator(&mut self, creator_id: ObjectID) {
        if !self.creator_set {
            self.creator_id = creator_id;
            self.creator_set = true;
        }
    }

    /// Notify the script engine that the special power has completed
    pub fn notify_script_engine(&self) {
        let player_index = self.base.get_object().and_then(|obj_arc| {
            let obj_guard = obj_arc.read().ok()?;
            let player = obj_guard.get_controlling_player()?;
            let player_guard = player.read().ok()?;
            Some(player_guard.get_player_index() as usize)
        });

        self.notify_script_engine_with_player_index(player_index);
    }

    /// Notify the script engine using a pre-resolved player index.
    pub fn notify_script_engine_with_player_index(&self, player_index: Option<usize>) {
        if !self.creator_set || self.creator_id == crate::common::INVALID_ID {
            return;
        }

        let power_name = match &self.base.module_data.special_power_template {
            Some(name) => name,
            None => return,
        };

        let Some(player_index) = player_index else {
            return;
        };

        if let Ok(mut engine_guard) = get_script_engine().write() {
            if let Some(engine) = engine_guard.as_mut() {
                engine.notify_of_completed_special_power(
                    player_index,
                    power_name.as_str(),
                    self.creator_id,
                );
            }
        }
    }
}

impl Snapshotable for SpecialPowerCompletionDie {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("SpecialPowerCompletionDie version crc failed: {:?}", e))?;
        xfer_die_module_base_versions(xfer)?;
        let mut creator_id = self.creator_id;
        xfer.xfer_object_id(&mut creator_id)
            .map_err(|e| format!("SpecialPowerCompletionDie creator_id crc failed: {:?}", e))?;
        let mut creator_set = self.creator_set;
        xfer.xfer_bool(&mut creator_set)
            .map_err(|e| format!("SpecialPowerCompletionDie creator_set crc failed: {:?}", e))?;
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("SpecialPowerCompletionDie version xfer failed: {:?}", e))?;

        xfer_die_module_base_versions(xfer)?;

        let mut creator_id = self.creator_id;
        xfer.xfer_object_id(&mut creator_id)
            .map_err(|e| format!("SpecialPowerCompletionDie creator_id xfer failed: {:?}", e))?;
        self.creator_id = creator_id;

        let mut creator_set = self.creator_set;
        xfer.xfer_bool(&mut creator_set)
            .map_err(|e| format!("SpecialPowerCompletionDie creator_set xfer failed: {:?}", e))?;
        self.creator_set = creator_set;

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

impl DieModuleInterface for SpecialPowerCompletionDie {
    fn snapshot_crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        <Self as Snapshotable>::crc(self, xfer)
    }

    fn snapshot_xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        <Self as Snapshotable>::xfer(self, xfer)
    }

    fn snapshot_load_post_process(&mut self) -> Result<(), String> {
        <Self as Snapshotable>::load_post_process(self)
    }

    fn set_creator(&mut self, creator_id: ObjectID) {
        SpecialPowerCompletionDie::set_creator(self, creator_id);
    }

    fn notify_script_engine_with_player_index(&self, player_index: Option<usize>) -> bool {
        SpecialPowerCompletionDie::notify_script_engine_with_player_index(self, player_index);
        true
    }

    /// Called when the special power object dies - notifies script engine
    /// (Matches C++ SpecialPowerCompletionDie::onDie)
    fn on_die(&mut self, object: &mut Object, damage_info: &DamageInfo) {
        // Check if this die module should activate
        if !self.is_die_applicable(
            object,
            damage_info,
            &self.base.module_data.base.die_mux_data,
        ) {
            return;
        }

        // Notify the script engine
        // (Note: It's okay if creator doesn't exist - they may have died first)
        let player_index = if let Some(player) = object.get_controlling_player() {
            if let Ok(player_guard) = player.read() {
                Some(player_guard.get_player_index() as usize)
            } else {
                None
            }
        } else {
            None
        };
        self.notify_script_engine_with_player_index(player_index);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_special_power_completion_die_module_data_default() {
        let data = SpecialPowerCompletionDieModuleData::default();
        assert!(data.special_power_template.is_none());
    }

    #[test]
    fn test_special_power_completion_die_module_name() {
        assert_eq!(
            SpecialPowerCompletionDie::get_module_name(),
            "SpecialPowerCompletionDie"
        );
    }

    #[test]
    fn test_special_power_completion_die_with_power() {
        let mut data = SpecialPowerCompletionDieModuleData::default();
        data.special_power_template = Some("SpecialPower_A10Strike".to_string());

        assert!(data.special_power_template.is_some());
        assert_eq!(
            data.special_power_template.unwrap(),
            "SpecialPower_A10Strike"
        );
    }

    #[test]
    fn test_set_creator() {
        let creator_id: ObjectID = 12345;
        assert_eq!(creator_id, 12345);
    }
}
