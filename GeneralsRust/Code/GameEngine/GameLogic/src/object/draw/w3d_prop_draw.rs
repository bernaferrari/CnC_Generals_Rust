use super::draw_module::*;
use crate::common::*;
use crate::helpers::{ModelDrawState, TheGameClient};
use game_engine::common::ini::{INIError, INI};
use game_engine::common::system::{Snapshotable, Xfer, XferVersion};
use game_engine::common::thing::module::{Module, ModuleData, NameKeyType, TimeOfDay};
use std::any::Any;

#[derive(Debug, Clone, Default)]
pub struct W3DPropDrawModuleData {
    module_tag_name_key: NameKeyType,
    pub model_name: AsciiString,
}
impl W3DPropDrawModuleData {
    pub fn new() -> Self {
        Self {
            module_tag_name_key: 0,
            model_name: AsciiString::new(),
        }
    }
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        loop {
            ini.read_line()?;
            if ini.is_eof() {
                return Err(INIError::EndOfFile);
            }
            let tokens = ini
                .get_line_tokens()
                .into_iter()
                .map(|t| t.to_string())
                .collect::<Vec<_>>();
            let Some(key) = tokens.first().cloned() else {
                continue;
            };
            if key.eq_ignore_ascii_case("End") {
                break;
            }
            let values = tokens
                .iter()
                .map(String::as_str)
                .skip(1)
                .filter(|t| *t != "=")
                .collect::<Vec<_>>();
            match key.to_ascii_uppercase().as_str() {
                "MODELNAME" => {
                    let parsed = INI::parse_ascii_string(
                        values.first().copied().ok_or(INIError::InvalidData)?,
                    )?;
                    self.model_name = AsciiString::from(parsed.as_str())
                }
                _ => return Err(INIError::UnknownToken),
            }
        }
        Ok(())
    }
}
impl ModuleData for W3DPropDrawModuleData {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn set_module_tag_name_key(&mut self, key: NameKeyType) {
        self.module_tag_name_key = key;
    }
    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.module_tag_name_key
    }
}
impl DrawModuleData for W3DPropDrawModuleData {
    fn as_any(&self) -> &dyn Any {
        self
    }
}
impl Snapshotable for W3DPropDrawModuleData {
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

pub struct W3DPropDraw {
    data: W3DPropDrawModuleData,
    owner_id: Option<ObjectID>,
    prop_added: bool,
    hidden: bool,
}
impl W3DPropDraw {
    pub fn new(data: W3DPropDrawModuleData) -> Self {
        Self {
            data,
            owner_id: None,
            prop_added: false,
            hidden: false,
        }
    }
    pub fn bind_owner_id(&mut self, owner_id: ObjectID) {
        self.owner_id = Some(owner_id);
    }
}
impl Module for W3DPropDraw {
    fn on_object_created(&mut self) {}
    fn on_drawable_bound_to_object(&mut self) {}
    fn preload_assets(&mut self, _time_of_day: TimeOfDay) {}
    fn on_delete(&mut self) {}
    fn get_module_name_key(&self) -> NameKeyType {
        game_engine::common::name_key_generator::NameKeyGenerator::name_to_key("W3DPropDraw")
    }
    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.data.get_module_tag_name_key()
    }
    fn get_module_data(&self) -> &dyn ModuleData {
        &self.data
    }
}
impl DrawModule for W3DPropDraw {
    fn do_draw_module(&mut self, transform_mtx: &Matrix3D) {
        if self.hidden || self.data.model_name.is_empty() {
            return;
        }
        if let (Some(owner_id), Some(client)) = (self.owner_id, TheGameClient::get()) {
            client.set_drawable_model_draw(
                owner_id,
                ModelDrawState {
                    model_name: self.data.model_name.to_string(),
                    world_transform: *transform_mtx,
                    condition_flags_bits: 0,
                    bone_overrides: Vec::new(),
                    animation_name: None,
                    animation_time: 0.0,
                    animation_mode: 0,
                    mesh_uv_overrides: Vec::new(),
                    sub_object_visibility: Vec::new(),
                },
            );
        }
    }
    fn set_shadows_enabled(&mut self, _enable: bool) {}
    fn release_shadows(&mut self) {}
    fn allocate_shadows(&mut self) {}
    fn set_fully_obscured_by_shroud(&mut self, fully_obscured: bool) {
        self.hidden = fully_obscured;
    }
    fn set_hidden(&mut self, hidden: bool) {
        self.hidden = hidden;
    }
    fn is_visible(&self) -> bool {
        !self.hidden
    }
    fn react_to_transform_change(
        &mut self,
        _old_mtx: &Matrix3D,
        old_pos: &Coord3D,
        _old_angle: Real,
    ) {
        if !self.prop_added && (old_pos.x != 0.0 || old_pos.y != 0.0) {
            self.prop_added = true;
        }
    }
    fn react_to_geometry_change(&mut self) {}
}
impl Snapshotable for W3DPropDraw {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let current_version: XferVersion = 1;
        let mut version = current_version;
        xfer.xfer_version(&mut version, current_version)
            .map_err(|e| e.to_string())?;
        Ok(())
    }
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let current_version: XferVersion = 1;
        let mut version = current_version;
        xfer.xfer_version(&mut version, current_version)
            .map_err(|e| e.to_string())
    }
    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}
