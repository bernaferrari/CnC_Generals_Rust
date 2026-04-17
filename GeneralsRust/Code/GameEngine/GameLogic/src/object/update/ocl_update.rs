// OCLUpdate - Update spits out an OCL on a timer
// Author: Graham Smallwood, August 2002
// Ported to Rust

use crate::common::{Bool, Color, ObjectID, ObjectStatusTypes, UnsignedInt};
use crate::effects::ObjectCreationList;
use crate::helpers::{
    game_logic_random_value, TheGameLogic, TheObjectCreationListStore, TheTerrainLogic,
};
use crate::modules::{
    OCLUpdateInterface, UpdateModuleInterface, UpdateSleepTime, UPDATE_SLEEP_NONE,
};
use crate::object::Object;
use crate::object_creation_list::live_creation_context;
use crate::player::PlayerArcExt;
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{Module, ModuleData, NameKeyType};
use std::any::Any;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct FactionOCLInfo {
    pub faction_name: String,
    pub ocl: Option<Arc<ObjectCreationList>>,
}

#[derive(Debug, Clone)]
pub struct OCLUpdateModuleData {
    pub module_tag_name_key: NameKeyType,
    pub min_delay: UnsignedInt,
    pub max_delay: UnsignedInt,
    pub ocl: Option<Arc<ObjectCreationList>>,
    pub faction_ocl: Vec<FactionOCLInfo>,
    pub is_create_at_edge: Bool,
    pub is_faction_triggered: Bool,
}

impl Default for OCLUpdateModuleData {
    fn default() -> Self {
        Self {
            module_tag_name_key: 0,
            min_delay: 0,
            max_delay: 0,
            ocl: None,
            faction_ocl: Vec::new(),
            is_create_at_edge: false,
            is_faction_triggered: false,
        }
    }
}

impl OCLUpdateModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, OCL_UPDATE_FIELDS)
    }
}

impl Snapshotable for OCLUpdateModuleData {
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

crate::impl_legacy_module_data_with_key_field!(OCLUpdateModuleData, module_tag_name_key);

fn parse_ocl(
    _ini: &mut INI,
    data: &mut OCLUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let value = tokens
        .iter()
        .copied()
        .find(|t| *t != "=")
        .ok_or(INIError::InvalidData)?;
    data.ocl = TheObjectCreationListStore::find_object_creation_list(value);
    if data.ocl.is_none() {
        log::warn!("OCLUpdate: unresolved OCL '{}'", value);
    }
    Ok(())
}

fn parse_faction_ocl(
    _ini: &mut INI,
    data: &mut OCLUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let mut parts: Vec<&str> = Vec::new();
    for token in tokens.iter().copied() {
        if token.is_empty() || token == "=" {
            continue;
        }
        for part in token.split(':') {
            if !part.is_empty() {
                parts.push(part);
            }
        }
    }

    let mut faction_name: Option<String> = None;
    let mut ocl_name: Option<String> = None;
    let mut i = 0;
    while i < parts.len() {
        let key = parts[i];
        if key.eq_ignore_ascii_case("Faction") {
            i += 1;
            if i >= parts.len() {
                return Err(INIError::InvalidData);
            }
            faction_name = Some(parts[i].to_string());
        } else if key.eq_ignore_ascii_case("OCL") {
            i += 1;
            if i >= parts.len() {
                return Err(INIError::InvalidData);
            }
            ocl_name = Some(parts[i].to_string());
        }
        i += 1;
    }

    let faction_name = faction_name.ok_or(INIError::InvalidData)?;
    let ocl = ocl_name.and_then(|name| {
        let resolved = TheObjectCreationListStore::find_object_creation_list(name.as_str());
        if resolved.is_none() {
            log::warn!("OCLUpdate: unresolved faction OCL '{}'", name);
        }
        resolved
    });
    data.faction_ocl.push(FactionOCLInfo { faction_name, ocl });
    Ok(())
}

fn parse_min_delay(
    _ini: &mut INI,
    data: &mut OCLUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens
        .iter()
        .copied()
        .find(|t| *t != "=")
        .ok_or(INIError::InvalidData)?;
    data.min_delay = INI::parse_duration_unsigned_int(token)?;
    Ok(())
}

fn parse_max_delay(
    _ini: &mut INI,
    data: &mut OCLUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens
        .iter()
        .copied()
        .find(|t| *t != "=")
        .ok_or(INIError::InvalidData)?;
    data.max_delay = INI::parse_duration_unsigned_int(token)?;
    Ok(())
}

fn parse_create_at_edge(
    _ini: &mut INI,
    data: &mut OCLUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens
        .iter()
        .copied()
        .find(|t| *t != "=")
        .ok_or(INIError::InvalidData)?;
    data.is_create_at_edge = INI::parse_bool(token)?;
    Ok(())
}

fn parse_faction_triggered(
    _ini: &mut INI,
    data: &mut OCLUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens
        .iter()
        .copied()
        .find(|t| *t != "=")
        .ok_or(INIError::InvalidData)?;
    data.is_faction_triggered = INI::parse_bool(token)?;
    Ok(())
}

const OCL_UPDATE_FIELDS: &[FieldParse<OCLUpdateModuleData>] = &[
    FieldParse {
        token: "OCL",
        parse: parse_ocl,
    },
    FieldParse {
        token: "FactionOCL",
        parse: parse_faction_ocl,
    },
    FieldParse {
        token: "MinDelay",
        parse: parse_min_delay,
    },
    FieldParse {
        token: "MaxDelay",
        parse: parse_max_delay,
    },
    FieldParse {
        token: "CreateAtEdge",
        parse: parse_create_at_edge,
    },
    FieldParse {
        token: "FactionTriggered",
        parse: parse_faction_triggered,
    },
];

#[derive(Debug, Clone)]
pub struct OCLUpdate {
    object_id: ObjectID,
    module_data: Arc<OCLUpdateModuleData>,
    next_creation_frame: UnsignedInt,
    timer_started_frame: UnsignedInt,
    is_faction_neutral: Bool,
    current_player_color: Color,
}

impl OCLUpdate {
    pub fn new(object_id: ObjectID, module_data: Arc<OCLUpdateModuleData>) -> Self {
        Self {
            object_id,
            module_data,
            next_creation_frame: 0,
            timer_started_frame: 0,
            is_faction_neutral: true,
            current_player_color: Color::default(),
        }
    }

    pub fn tick(&mut self) -> UpdateSleepTime {
        let Some(object_arc) = TheGameLogic::find_object_by_id(self.object_id) else {
            return UPDATE_SLEEP_NONE;
        };
        let Ok(object) = object_arc.read() else {
            return UPDATE_SLEEP_NONE;
        };

        if object.is_disabled() {
            self.next_creation_frame = self.next_creation_frame.saturating_add(1);
            return UPDATE_SLEEP_NONE;
        }

        let is_faction_triggered = self.module_data.is_faction_triggered;
        let is_create_at_edge = self.module_data.is_create_at_edge;
        let faction_ocl = self.module_data.faction_ocl.clone();
        let default_ocl = self.module_data.ocl.clone();

        if is_faction_triggered {
            let player = object.get_controlling_player();
            if self.is_faction_neutral {
                if let Some(player_arc) = player.as_ref() {
                    if let Ok(player_guard) = player_arc.read() {
                        if player_guard.is_playable_side() {
                            self.current_player_color = player_guard.get_player_color();
                            self.is_faction_neutral = false;
                            self.set_next_creation_frame();
                        }
                    }
                }
            } else {
                let mut should_reset = false;
                match player.as_ref() {
                    Some(player_arc) => {
                        if let Ok(player_guard) = player_arc.read() {
                            if !player_guard.is_playable_side() {
                                self.is_faction_neutral = true;
                            } else if player_guard.get_player_color() != self.current_player_color {
                                self.current_player_color = player_guard.get_player_color();
                                should_reset = true;
                            }
                        }
                    }
                    None => {
                        self.is_faction_neutral = true;
                    }
                }
                if should_reset {
                    self.set_next_creation_frame();
                }
            }

            if self.is_faction_neutral {
                return UPDATE_SLEEP_NONE;
            }
        }

        if self.should_create(&object) {
            if self.next_creation_frame == 0 {
                self.set_next_creation_frame();
                return UPDATE_SLEEP_NONE;
            }

            self.set_next_creation_frame();

            let creation_coord = if is_create_at_edge {
                TheTerrainLogic::get()
                    .map(|terrain| terrain.find_closest_edge_point(object.get_position()))
                    .unwrap_or(*object.get_position())
            } else {
                *object.get_position()
            };

            if is_faction_triggered {
                let Some(player_arc) = object.get_controlling_player() else {
                    return UPDATE_SLEEP_NONE;
                };
                let Some(player_template) = player_arc.get_player_template() else {
                    return UPDATE_SLEEP_NONE;
                };
                let faction_name = player_template.get_side();
                for info in &faction_ocl {
                    if faction_name == info.faction_name {
                        if let Some(ocl) = info.ocl.as_ref() {
                            let ctx = live_creation_context();
                            let _ = ocl.create_with_angle(
                                &ctx,
                                Some(&object),
                                &creation_coord,
                                object.get_position(),
                                object.get_orientation(),
                                0,
                            );
                        }
                        break;
                    }
                }
            } else if let Some(ocl) = default_ocl.as_ref() {
                let ctx = live_creation_context();
                let _ = ocl.create_with_angle(
                    &ctx,
                    Some(&object),
                    &creation_coord,
                    object.get_position(),
                    object.get_orientation(),
                    0,
                );
            }
        }

        UPDATE_SLEEP_NONE
    }

    pub fn reset_timer(&mut self) {
        self.set_next_creation_frame();
    }

    fn should_create(&self, object: &Object) -> Bool {
        if TheGameLogic::get_frame() < self.next_creation_frame {
            return false;
        }
        if object.test_status(ObjectStatusTypes::UnderConstruction) {
            return false;
        }
        true
    }

    fn set_next_creation_frame(&mut self) {
        let delay = game_logic_random_value(self.module_data.min_delay, self.module_data.max_delay);
        let now = TheGameLogic::get_frame();
        self.timer_started_frame = now;
        self.next_creation_frame = now.saturating_add(delay);
    }

    pub fn get_countdown_percent(&self) -> f32 {
        if self.next_creation_frame <= self.timer_started_frame {
            return 0.0;
        }
        let now = TheGameLogic::get_frame();
        if now >= self.next_creation_frame {
            return 1.0;
        }
        let remaining = self.next_creation_frame - now;
        let total = self.next_creation_frame - self.timer_started_frame;
        1.0 - (remaining as f32 / total as f32)
    }

    pub fn get_remaining_frames(&self) -> UnsignedInt {
        let now = TheGameLogic::get_frame();
        self.next_creation_frame.saturating_sub(now)
    }

    pub fn save(&self, xfer: &mut dyn Xfer) {
        let xfer_io = |result: std::io::Result<()>, field: &str| {
            if let Err(err) = result {
                panic!("OCLUpdate::save failed to xfer {field}: {err}");
            }
        };

        let mut version: u8 = 1;
        xfer_io(xfer.xfer_version(&mut version, 1), "version");
        let mut next_creation_frame = self.next_creation_frame;
        xfer_io(
            xfer.xfer_u32(&mut next_creation_frame),
            "next_creation_frame",
        );
        let mut timer_started_frame = self.timer_started_frame;
        xfer_io(
            xfer.xfer_u32(&mut timer_started_frame),
            "timer_started_frame",
        );
        let mut is_faction_neutral = self.is_faction_neutral;
        xfer_io(
            xfer.xfer_bool(&mut is_faction_neutral),
            "is_faction_neutral",
        );
        let mut packed_color = ((self.current_player_color.a as u32) << 24)
            | ((self.current_player_color.b as u32) << 16)
            | ((self.current_player_color.g as u32) << 8)
            | (self.current_player_color.r as u32);
        xfer_io(xfer.xfer_u32(&mut packed_color), "current_player_color");
    }

    pub fn load(&mut self, xfer: &mut dyn Xfer) {
        let xfer_io = |result: std::io::Result<()>, field: &str| {
            if let Err(err) = result {
                panic!("OCLUpdate::load failed to xfer {field}: {err}");
            }
        };

        let mut version: u8 = 1;
        xfer_io(xfer.xfer_version(&mut version, 1), "version");
        if version >= 1 {
            xfer_io(
                xfer.xfer_u32(&mut self.next_creation_frame),
                "next_creation_frame",
            );
            xfer_io(
                xfer.xfer_u32(&mut self.timer_started_frame),
                "timer_started_frame",
            );
            xfer_io(
                xfer.xfer_bool(&mut self.is_faction_neutral),
                "is_faction_neutral",
            );
            let mut packed_color = 0u32;
            xfer_io(xfer.xfer_u32(&mut packed_color), "current_player_color");
            self.current_player_color.r = (packed_color & 0xFF) as u8;
            self.current_player_color.g = ((packed_color >> 8) & 0xFF) as u8;
            self.current_player_color.b = ((packed_color >> 16) & 0xFF) as u8;
            self.current_player_color.a = ((packed_color >> 24) & 0xFF) as u8;
        }
    }
}

impl UpdateModuleInterface for OCLUpdate {
    fn update(&mut self) -> Result<UpdateSleepTime, Box<dyn std::error::Error + Send + Sync>> {
        Ok(self.tick())
    }
}

impl OCLUpdateInterface for OCLUpdate {
    fn reset_timer(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.reset_timer();
        Ok(())
    }

    fn get_remaining_frames(&self) -> Option<UnsignedInt> {
        Some(self.get_remaining_frames())
    }

    fn get_countdown_percent(&self) -> Option<f32> {
        Some(self.get_countdown_percent())
    }
}

/// Module wrapper for OCLUpdate update behavior.
pub struct OCLUpdateModule {
    module_name_key: NameKeyType,
    module_data: Arc<OCLUpdateModuleData>,
    update: OCLUpdate,
}

impl OCLUpdateModule {
    pub fn new(
        module_name_key: NameKeyType,
        module_data: Arc<OCLUpdateModuleData>,
        owner_id: ObjectID,
    ) -> Self {
        let update = OCLUpdate::new(owner_id, Arc::clone(&module_data));
        Self {
            module_name_key,
            module_data,
            update,
        }
    }

    pub fn update(&mut self) -> UpdateSleepTime {
        self.update.tick()
    }

    pub fn reset_timer(&mut self) {
        self.update.reset_timer();
    }

    pub fn remaining_frames(&mut self) -> UnsignedInt {
        self.ensure_timer_initialized();
        self.update.get_remaining_frames()
    }

    pub fn countdown_percent(&mut self) -> f32 {
        self.ensure_timer_initialized();
        self.update.get_countdown_percent()
    }

    fn ensure_timer_initialized(&mut self) {
        if self.update.next_creation_frame != 0 {
            return;
        }
        let Some(object_arc) = TheGameLogic::find_object_by_id(self.update.object_id) else {
            return;
        };
        let Ok(object) = object_arc.read() else {
            return;
        };
        if object.test_status(ObjectStatusTypes::UnderConstruction) {
            return;
        }
        self.update.set_next_creation_frame();
    }
}

impl Snapshotable for OCLUpdateModule {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.module_data.crc(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Arc::make_mut(&mut self.module_data).xfer(xfer)?;
        self.update.save(xfer);
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Arc::make_mut(&mut self.module_data).load_post_process()
    }
}

impl Module for OCLUpdateModule {

    fn get_module_name_key(&self) -> NameKeyType {
        self.module_name_key
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.module_data.get_module_tag_name_key()
    }

    fn get_module_data(&self) -> &dyn ModuleData {
        self.module_data.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_ocl_keeps_missing_reference_none() {
        let mut data = OCLUpdateModuleData::default();
        let mut ini = INI::new();
        let missing = "OCL___UNIT_TEST_MISSING_REFERENCE";
        let tokens = [missing];

        parse_ocl(&mut ini, &mut data, &tokens).expect("parse_ocl failed");
        assert!(data.ocl.is_none());
    }

    #[test]
    fn parse_faction_ocl_keeps_missing_reference_none() {
        let mut data = OCLUpdateModuleData::default();
        let mut ini = INI::new();
        let tokens = ["Faction:America", "OCL:OCL___UNIT_TEST_MISSING_FACTION"];

        parse_faction_ocl(&mut ini, &mut data, &tokens).expect("parse_faction_ocl failed");
        assert_eq!(data.faction_ocl.len(), 1);
        assert!(data.faction_ocl[0].ocl.is_none());
    }
}
