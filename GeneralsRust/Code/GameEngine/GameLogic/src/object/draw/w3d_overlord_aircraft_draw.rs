use super::draw_module::*;
use super::w3d_model_draw::*;
use crate::common::*;
use crate::drawable::Drawable as DrawableTrait;
use crate::object::drawable::DrawableArcExt;
use crate::object::drawable::TintStatus;
use game_engine::common::ini::{INIError, INI};
use game_engine::common::system::{Snapshotable, Xfer, XferVersion};
use game_engine::common::thing::module::{Module, ModuleData, NameKeyType, TimeOfDay};
use std::any::Any;

#[derive(Debug, Clone, Default)]
pub struct W3DOverlordAircraftDrawModuleData {
    pub base: W3DModelDrawModuleData,
}
impl W3DOverlordAircraftDrawModuleData {
    pub fn new() -> Self {
        Self {
            base: W3DModelDrawModuleData::new(),
        }
    }
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        self.base.parse_from_ini(ini)
    }
}
impl ModuleData for W3DOverlordAircraftDrawModuleData {
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
impl DrawModuleData for W3DOverlordAircraftDrawModuleData {
    fn as_any(&self) -> &dyn Any {
        self
    }
}
impl Snapshotable for W3DOverlordAircraftDrawModuleData {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 0;
        xfer.xfer_version(&mut version, 1).map_err(|e| e.to_string())?;
        Ok(())
    }
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 0;
        xfer.xfer_version(&mut version, 1).map_err(|e| e.to_string())?;
        Ok(())
    }
    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

pub struct W3DOverlordAircraftDraw {
    data: W3DOverlordAircraftDrawModuleData,
    base: W3DModelDraw,
    owner_id: Option<ObjectID>,
}
impl W3DOverlordAircraftDraw {
    pub fn new(data: W3DOverlordAircraftDrawModuleData) -> Self {
        Self {
            base: W3DModelDraw::new(data.base.clone()),
            data,
            owner_id: None,
        }
    }
    pub fn bind_owner_id(&mut self, owner_id: ObjectID) {
        self.owner_id = Some(owner_id);
        self.base.bind_owner_id(owner_id);
    }
}
impl Module for W3DOverlordAircraftDraw {
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
impl DrawModule for W3DOverlordAircraftDraw {
    fn do_draw_module(&mut self, transform_mtx: &Matrix3D) {
        self.base.do_draw_module(transform_mtx);
        let Some(owner_id) = self.owner_id else {
            return;
        };
        let Some(owner) = crate::object::registry::OBJECT_REGISTRY.get_object(owner_id) else {
            return;
        };
        let Ok(owner_guard) = owner.read() else {
            return;
        };
        let (tint, tint_status) = owner_guard
            .get_drawable()
            .as_ref()
            .and_then(|d| d.read().ok())
            .map(|g| (g.get_tint_color(), g.get_tint_status()))
            .unwrap_or((Color::white(), TintStatus::NONE));
        let Some(contain) = owner_guard.get_contain() else {
            return;
        };
        let Ok(contain_guard) = contain.lock() else {
            return;
        };
        let Some(rider_id) = contain_guard.friend_get_rider() else {
            return;
        };
        drop(contain_guard);
        let Some(rider) = crate::object::registry::OBJECT_REGISTRY.get_object(rider_id) else {
            return;
        };
        let Ok(rider_guard) = rider.read() else {
            return;
        };
        let Some(drawable) = rider_guard.get_drawable() else {
            return;
        };
        let drawable = drawable.clone();
        {
            let Ok(mut drawable_guard) = drawable.write() else {
                return;
            };
            drawable_guard.set_color_tint(tint);
            drawable_guard.set_tint_status_exact(tint_status);
            drawable_guard.notify_drawable_dependency_cleared();
            drawable_guard.draw(None);
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
        let Some(owner) = self
            .owner_id
            .and_then(|id| crate::object::registry::OBJECT_REGISTRY.get_object(id))
        else {
            return;
        };
        let Ok(owner_guard) = owner.read() else {
            return;
        };
        let Some(contain) = owner_guard.get_contain() else {
            return;
        };
        let Ok(contain_guard) = contain.lock() else {
            return;
        };
        let Some(rider_id) = contain_guard.friend_get_rider() else {
            return;
        };
        drop(contain_guard);
        let Some(rider) = crate::object::registry::OBJECT_REGISTRY.get_object(rider_id) else {
            return;
        };
        let Ok(rider_guard) = rider.read() else {
            return;
        };
        let Some(drawable) = rider_guard.get_drawable() else {
            return;
        };
        let _ = drawable.set_drawable_hidden(hidden);
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
impl Snapshotable for W3DOverlordAircraftDraw {
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
