use super::draw_module::*;
use super::w3d_truck_draw::*;
use crate::common::*;
use crate::helpers::{game_client_random_value_real, TheGameClient};
use game_engine::common::system::{Snapshotable, Xfer, XferVersion};
use game_engine::common::thing::module::{Module, ModuleData, NameKeyType, TimeOfDay};
use std::any::Any;

#[derive(Debug, Clone, Default)]
pub struct W3DPoliceCarDrawModuleData {
    pub base: W3DTruckDrawModuleData,
}
impl W3DPoliceCarDrawModuleData {
    pub fn new() -> Self {
        Self {
            base: W3DTruckDrawModuleData::new(),
        }
    }
    pub fn parse_from_ini(
        &mut self,
        ini: &mut game_engine::common::ini::INI,
    ) -> Result<(), game_engine::common::ini::INIError> {
        self.base.parse_from_ini(ini)
    }
}
impl ModuleData for W3DPoliceCarDrawModuleData {
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
impl DrawModuleData for W3DPoliceCarDrawModuleData {
    fn as_any(&self) -> &dyn Any {
        self
    }
}
impl Snapshotable for W3DPoliceCarDrawModuleData {
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

pub struct W3DPoliceCarDraw {
    data: W3DPoliceCarDrawModuleData,
    base: W3DTruckDraw,
    cur_frame: Real,
}
impl W3DPoliceCarDraw {
    pub fn new(data: W3DPoliceCarDrawModuleData) -> Self {
        Self {
            base: W3DTruckDraw::new(data.base.clone()),
            data,
            cur_frame: game_client_random_value_real(0.0, 10.0),
        }
    }
    pub fn bind_owner_id(&mut self, owner_id: ObjectID) {
        self.base.bind_owner_id(owner_id);
    }
}
impl Module for W3DPoliceCarDraw {
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
    fn get_module_data(&self) -> &dyn ModuleData {
        &self.data
    }
}
impl DrawModule for W3DPoliceCarDraw {
    fn do_draw_module(&mut self, transform_mtx: &Matrix3D) {
        self.cur_frame += 0.25;
        if self.cur_frame > 14.0 {
            self.cur_frame = 0.0;
        }
        self.base.do_draw_module(transform_mtx);
        if let Some(owner_id) = self.base.owner_id() {
            if let Some(client) = TheGameClient::get() {
                if let Some(mut state) = client.get_drawable_model_draw(owner_id) {
                    state.animation_time = (self.cur_frame / 14.0).clamp(0.0, 1.0);
                    client.set_drawable_model_draw(owner_id, state);
                }
            }
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
        self.base.get_object_draw_interface()
    }
    fn get_object_draw_interface_mut(&mut self) -> Option<&mut dyn ObjectDrawInterface> {
        self.base.get_object_draw_interface_mut()
    }
}
impl Snapshotable for W3DPoliceCarDraw {
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
