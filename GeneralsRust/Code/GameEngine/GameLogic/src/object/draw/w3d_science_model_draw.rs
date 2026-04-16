use super::draw_module::*;
use super::w3d_model_draw::*;
use crate::common::*;
use crate::player::ThePlayerList;
use game_engine::common::ini::{INIError, INI};
use game_engine::common::rts::{get_science_store, ScienceType, SCIENCE_INVALID};
use game_engine::common::system::{Snapshotable, Xfer, XferVersion};
use game_engine::common::thing::module::{Module, ModuleData, NameKeyType, TimeOfDay};
use std::any::Any;

#[derive(Debug, Clone)]
pub struct W3DScienceModelDrawModuleData {
    pub base: W3DModelDrawModuleData,
    pub required_science: ScienceType,
}

impl W3DScienceModelDrawModuleData {
    pub fn new() -> Self {
        Self {
            base: W3DModelDrawModuleData::new(),
            required_science: SCIENCE_INVALID,
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
            let value_tokens = tokens
                .iter()
                .map(String::as_str)
                .skip(1)
                .filter(|t| *t != "=")
                .collect::<Vec<_>>();
            match key.to_ascii_uppercase().as_str() {
                "REQUIREDSCIENCE" => {
                    let store = get_science_store().ok_or(INIError::InvalidData)?;
                    self.required_science =
                        store.get_science_from_internal_name(required_value(&value_tokens)?);
                }
                _ => {
                    if !self
                        .base
                        .parse_ini_field(ini, key.as_str(), &value_tokens)?
                    {
                        return Err(INIError::UnknownToken);
                    }
                }
            }
        }
        Ok(())
    }
}

fn required_value<'a>(tokens: &'a [&str]) -> Result<&'a str, INIError> {
    tokens
        .iter()
        .copied()
        .find(|t| !t.is_empty())
        .ok_or(INIError::InvalidData)
}

impl Default for W3DScienceModelDrawModuleData {
    fn default() -> Self {
        Self::new()
    }
}
impl ModuleData for W3DScienceModelDrawModuleData {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn set_module_tag_name_key(&mut self, key: NameKeyType) {
        self.base.set_module_tag_name_key(key);
    }
    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.base.get_module_tag_name_key()
    }
}
impl DrawModuleData for W3DScienceModelDrawModuleData {
    fn as_any(&self) -> &dyn Any {
        self
    }
}
impl Snapshotable for W3DScienceModelDrawModuleData {
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

pub struct W3DScienceModelDraw {
    data: W3DScienceModelDrawModuleData,
    base: W3DModelDraw,
}
impl W3DScienceModelDraw {
    pub fn new(data: W3DScienceModelDrawModuleData) -> Self {
        Self {
            base: W3DModelDraw::new(data.base.clone()),
            data,
        }
    }
    pub fn bind_owner_id(&mut self, owner_id: ObjectID) {
        self.base.bind_owner_id(owner_id);
    }
}
impl Module for W3DScienceModelDraw {
    fn on_object_created(&mut self) {
        self.base.on_object_created();
    }
    fn on_drawable_bound_to_object(&mut self) {
        self.base.on_drawable_bound_to_object();
    }
    fn preload_assets(&mut self, time_of_day: TimeOfDay) {
        self.base.preload_assets(time_of_day);
    }
    fn on_delete(&mut self) {
        self.base.on_delete();
    }
    fn get_module_name_key(&self) -> NameKeyType {
        self.base.get_module_name_key()
    }
    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.base.get_module_tag_name_key()
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
    fn get_module_data(&self) -> &dyn ModuleData {
        &self.data
    }
}
impl DrawModule for W3DScienceModelDraw {
    fn do_draw_module(&mut self, transform_mtx: &Matrix3D) {
        let science = self.data.required_science;
        if science == SCIENCE_INVALID {
            DrawModule::set_hidden(&mut self.base, true);
            return;
        }
        let has_science = ThePlayerList()
            .read()
            .ok()
            .and_then(|list| list.get_local_player().cloned())
            .and_then(|p| {
                p.read()
                    .ok()
                    .map(|g| (!g.is_player_active()) || g.has_science(science))
            })
            .unwrap_or(true);
        DrawModule::set_hidden(&mut self.base, !has_science);
        if has_science {
            self.base.do_draw_module(transform_mtx);
        }
    }
    fn set_shadows_enabled(&mut self, enable: bool) {
        self.base.set_shadows_enabled(enable);
    }
    fn release_shadows(&mut self) {
        self.base.release_shadows();
    }
    fn allocate_shadows(&mut self) {
        self.base.allocate_shadows();
    }
    fn set_fully_obscured_by_shroud(&mut self, fully_obscured: bool) {
        self.base.set_fully_obscured_by_shroud(fully_obscured);
    }
    fn set_hidden(&mut self, hidden: bool) {
        DrawModule::set_hidden(&mut self.base, hidden);
    }
    fn is_visible(&self) -> bool {
        self.base.is_visible()
    }
    fn react_to_transform_change(
        &mut self,
        old_mtx: &Matrix3D,
        old_pos: &Coord3D,
        old_angle: Real,
    ) {
        self.base
            .react_to_transform_change(old_mtx, old_pos, old_angle);
    }
    fn react_to_geometry_change(&mut self) {
        self.base.react_to_geometry_change();
    }
    fn get_object_draw_interface(&self) -> Option<&dyn ObjectDrawInterface> {
        Some(&self.base)
    }
    fn get_object_draw_interface_mut(&mut self) -> Option<&mut dyn ObjectDrawInterface> {
        Some(&mut self.base)
    }
}
impl Snapshotable for W3DScienceModelDraw {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.base.crc(xfer)
    }
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let current_version: XferVersion = 1;
        let mut version = current_version;
        xfer.xfer_version(&mut version, current_version)
            .map_err(|e| e.to_string())?;
        self.base.xfer(xfer)
    }
    fn load_post_process(&mut self) -> Result<(), String> {
        self.base.load_post_process()
    }
}
