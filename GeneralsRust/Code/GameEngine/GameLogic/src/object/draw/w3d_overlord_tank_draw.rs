//! W3DOverlordTankDraw - Overlord tank special rendering
//!
//! Port of C++ W3DOverlordTankDraw.h
//! Reference: /GeneralsMD/Code/GameEngineDevice/Include/W3DDevice/GameClient/Module/W3DOverlordTankDraw.h
//!
//! The Overlord has special rendering needs - it needs its rider (top upgrade)
//! to draw explicitly after it, with direct access despite OverlordContain hiding it.

use super::draw_module::*;
use super::w3d_tank_draw::*;
use crate::common::*;
use crate::drawable::Drawable;
use crate::object::drawable::{DrawableArcExt, TintStatus};
use game_engine::common::ini::{INIError, INI};
use game_engine::common::system::{Snapshotable, Xfer, XferVersion};
use game_engine::common::thing::module::{Module, ModuleData, NameKeyType};
use std::any::Any;

#[derive(Debug, Clone)]
pub struct W3DOverlordTankDrawModuleData {
    pub base: W3DTankDrawModuleData,
}

impl W3DOverlordTankDrawModuleData {
    pub fn new() -> Self {
        Self {
            base: W3DTankDrawModuleData::new(),
        }
    }

    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        self.base.parse_from_ini(ini)
    }
}

impl Default for W3DOverlordTankDrawModuleData {
    fn default() -> Self {
        Self::new()
    }
}

impl ModuleData for W3DOverlordTankDrawModuleData {
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

impl DrawModuleData for W3DOverlordTankDrawModuleData {
    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl Snapshotable for W3DOverlordTankDrawModuleData {
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

pub struct W3DOverlordTankDraw {
    data: W3DOverlordTankDrawModuleData,
    base: W3DTankDraw,
    owner_id: Option<ObjectID>,
}

impl W3DOverlordTankDraw {
    pub fn new(data: W3DOverlordTankDrawModuleData) -> Self {
        let base_data = data.base.clone();
        let base = W3DTankDraw::new(base_data);

        Self {
            data,
            base,
            owner_id: None,
        }
    }

    pub fn bind_owner_id(&mut self, owner_id: ObjectID) {
        self.owner_id = Some(owner_id);
    }
}

impl Module for W3DOverlordTankDraw {
    fn on_drawable_bound_to_object(&mut self) {
        self.base.on_drawable_bound_to_object();
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

impl DrawModule for W3DOverlordTankDraw {
    fn do_draw_module(&mut self, transform_mtx: &Matrix3D) {
        // Draw base tank
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

        let owner_drawable = owner_guard.get_drawable();
        let (owner_tint, owner_tint_status) = owner_drawable
            .as_ref()
            .and_then(|drawable| drawable.read().ok())
            .map(|guard| (guard.get_tint_color(), guard.get_tint_status()))
            .unwrap_or((Color::white(), TintStatus::NONE));

        let Some(contain) = owner_guard.get_contain() else {
            return;
        };

        let Ok(contain_guard) = contain.lock() else {
            return;
        };

        let rider_id = contain_guard.friend_get_rider();
        drop(contain_guard);

        let Some(rider_id) = rider_id else {
            return;
        };

        let Some(rider) = crate::object::registry::OBJECT_REGISTRY.get_object(rider_id) else {
            return;
        };
        let Ok(rider_guard) = rider.read() else {
            return;
        };

        let Some(drawable) = rider_guard.get_drawable() else {
            return;
        };

        {
            let Ok(mut drawable_guard) = drawable.write() else {
                return;
            };
            drawable_guard.set_color_tint(owner_tint);
            drawable_guard.set_tint_status_exact(owner_tint_status);
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

    fn set_hidden(&mut self, hidden: bool) {
        self.base.set_hidden(hidden);

        let Some(owner_id) = self.owner_id else {
            return;
        };

        let Some(owner) = crate::object::registry::OBJECT_REGISTRY.get_object(owner_id) else {
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
        let rider_id = contain_guard.friend_get_rider();
        drop(contain_guard);
        let Some(rider_id) = rider_id else {
            return;
        };
        let Some(rider) = crate::object::registry::OBJECT_REGISTRY.get_object(rider_id) else {
            return;
        };
        let Ok(rider_guard) = rider.read() else {
            return;
        };
        let Some(drawable) = rider_guard.get_drawable() else {
            return;
        };
        drawable.set_drawable_hidden(hidden);
    }

    fn set_fully_obscured_by_shroud(&mut self, fully_obscured: bool) {
        self.base.set_fully_obscured_by_shroud(fully_obscured);
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

impl Snapshotable for W3DOverlordTankDraw {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.base.crc(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        // C++ parity: W3DOverlordTankDraw::xfer writes version before delegating to W3DTankDraw.
        const CURRENT_VERSION: XferVersion = 1;
        let mut version = CURRENT_VERSION;
        xfer.xfer_version(&mut version, CURRENT_VERSION)
            .map_err(|e| e.to_string())?;
        self.base.xfer(xfer)
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        self.base.load_post_process()
    }
}
