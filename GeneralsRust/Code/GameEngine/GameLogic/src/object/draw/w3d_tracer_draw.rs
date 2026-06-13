//! W3DTracerDraw - Tracer bullet rendering
//!
//! Port of C++ W3DTracerDraw.h
//! Reference: /GeneralsMD/Code/GameEngineDevice/Include/W3DDevice/GameClient/Module/W3DTracerDraw.h
//!
use super::draw_module::*;
use crate::common::*;
use crate::helpers::{remove_scene_line, submit_scene_line, update_scene_line};
use game_engine::common::system::{SceneLineDesc, SceneLineId, Snapshotable, Xfer, XferVersion};
use game_engine::common::thing::module::{Module, ModuleData};
use std::any::Any;

#[derive(Debug, Clone)]
pub struct W3DTracerDrawModuleData {
    module_tag_name_key: NameKeyType,
    // No additional data, tracer parameters set at runtime
}

impl W3DTracerDrawModuleData {
    pub fn new() -> Self {
        Self {
            module_tag_name_key: 0,
        }
    }
}

impl Default for W3DTracerDrawModuleData {
    fn default() -> Self {
        Self::new()
    }
}

impl ModuleData for W3DTracerDrawModuleData {
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

impl DrawModuleData for W3DTracerDrawModuleData {
    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl Snapshotable for W3DTracerDrawModuleData {
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

pub struct W3DTracerDraw {
    _data: W3DTracerDrawModuleData,
    length: Real,
    width: Real,
    color: RGBColor,
    speed_in_dist_per_frame: Real,
    opacity: Real,
    current_pos: Coord3D,
    direction: Coord3D,
    line_start: Coord3D,
    line_end: Coord3D,
    scene_line_id: Option<SceneLineId>,
    hidden: bool,
    fully_obscured_by_shroud: bool,
    shadows_enabled: bool,
}

impl W3DTracerDraw {
    pub fn new(data: W3DTracerDrawModuleData) -> Self {
        Self {
            _data: data,
            length: 20.0,
            width: 0.5,
            color: RGBColor::new(229, 204, 178),
            speed_in_dist_per_frame: 1.0,
            opacity: 1.0,
            current_pos: Coord3D::origin(),
            direction: Coord3D::new(1.0, 0.0, 0.0),
            line_start: Coord3D::origin(),
            line_end: Coord3D::origin(),
            scene_line_id: None,
            hidden: false,
            fully_obscured_by_shroud: false,
            shadows_enabled: true,
        }
    }

    pub fn length(&self) -> Real {
        self.length
    }

    pub fn width(&self) -> Real {
        self.width
    }

    pub fn color(&self) -> RGBColor {
        self.color
    }

    pub fn speed_in_dist_per_frame(&self) -> Real {
        self.speed_in_dist_per_frame
    }

    pub fn opacity(&self) -> Real {
        self.opacity
    }

    pub fn line_start(&self) -> Coord3D {
        self.line_start
    }

    pub fn line_end(&self) -> Coord3D {
        self.line_end
    }

    fn sync_scene_visibility(&mut self, visible: bool) {
        let Some(id) = self.scene_line_id else {
            return;
        };

        let desc = SceneLineDesc {
            start: game_engine::common::system::geometry::Coord3D::new(
                self.line_start.x,
                self.line_start.y,
                self.line_start.z,
            ),
            end: game_engine::common::system::geometry::Coord3D::new(
                self.line_end.x,
                self.line_end.y,
                self.line_end.z,
            ),
            width: self.width,
            color_r: self.color.r as f32 / 255.0,
            color_g: self.color.g as f32 / 255.0,
            color_b: self.color.b as f32 / 255.0,
            opacity: self.opacity,
            texture_name: None,
            tile_factor: 0.0,
            visible,
        };
        update_scene_line(id, &desc);
    }
}

impl Module for W3DTracerDraw {
    fn on_drawable_bound_to_object(&mut self) {}
    fn on_delete(&mut self) {
        if let Some(id) = self.scene_line_id.take() {
            remove_scene_line(id);
        }
    }
    fn get_module_name_key(&self) -> NameKeyType {
        game_engine::common::name_key_generator::NameKeyGenerator::name_to_key("W3DTracerDraw")
    }
    fn get_module_tag_name_key(&self) -> NameKeyType {
        self._data.module_tag_name_key
    }
    fn get_module_data(&self) -> &dyn ModuleData {
        &self._data
    }
}

impl DrawModule for W3DTracerDraw {
    fn do_draw_module(&mut self, transform_mtx: &Matrix3D) {
        let translation = transform_mtx.w_axis;
        self.current_pos = Coord3D::new(translation.x, translation.y, translation.z);

        let dir = if self.direction.length() > 0.001 {
            self.direction.normalize()
        } else {
            Coord3D::new(1.0, 0.0, 0.0)
        };

        self.line_start = self.current_pos;
        self.line_end = self.current_pos + dir * self.length;
        self.current_pos += dir * self.speed_in_dist_per_frame;

        let desc = SceneLineDesc {
            start: game_engine::common::system::geometry::Coord3D::new(
                self.line_start.x,
                self.line_start.y,
                self.line_start.z,
            ),
            end: game_engine::common::system::geometry::Coord3D::new(
                self.line_end.x,
                self.line_end.y,
                self.line_end.z,
            ),
            width: self.width,
            color_r: self.color.r as f32 / 255.0,
            color_g: self.color.g as f32 / 255.0,
            color_b: self.color.b as f32 / 255.0,
            opacity: self.opacity,
            texture_name: None,
            tile_factor: 0.0,
            visible: !self.hidden && !self.fully_obscured_by_shroud,
        };

        match self.scene_line_id {
            None => {
                self.scene_line_id = submit_scene_line(0, &desc);
            }
            Some(id) => {
                update_scene_line(id, &desc);
            }
        }
    }

    fn set_shadows_enabled(&mut self, enable: bool) {
        let _ = enable;
    }
    fn release_shadows(&mut self) {}
    fn allocate_shadows(&mut self) {}
    fn set_hidden(&mut self, hidden: bool) {
        let _ = hidden;
    }

    fn set_fully_obscured_by_shroud(&mut self, fully_obscured: bool) {
        let _ = fully_obscured;
    }

    fn react_to_transform_change(
        &mut self,
        _old_mtx: &Matrix3D,
        old_pos: &Coord3D,
        _old_angle: Real,
    ) {
        self.current_pos = *old_pos;
    }

    fn react_to_geometry_change(&mut self) {
        self.sync_scene_visibility(!self.hidden && !self.fully_obscured_by_shroud);
    }

    fn get_tracer_draw_interface(&self) -> Option<&dyn TracerDrawInterface> {
        Some(self)
    }

    fn get_tracer_draw_interface_mut(&mut self) -> Option<&mut dyn TracerDrawInterface> {
        Some(self)
    }
}

impl TracerDrawInterface for W3DTracerDraw {
    fn set_tracer_parms(
        &mut self,
        speed: Real,
        length: Real,
        width: Real,
        color: &RGBColor,
        initial_opacity: Real,
    ) {
        self.speed_in_dist_per_frame = speed;
        self.length = length;
        self.width = width;
        self.color = *color;
        self.opacity = initial_opacity;
        self.line_start = Coord3D::origin();
        self.line_end = Coord3D::new(self.length, 0.0, 0.0);
        self.sync_scene_visibility(true);
    }
}

impl Snapshotable for W3DTracerDraw {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        const CURRENT_VERSION: XferVersion = 1;
        let mut version = CURRENT_VERSION;
        xfer.xfer_version(&mut version, CURRENT_VERSION)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        // C++ parity: W3DTracerDraw::xfer version stamp with no persistent payload.
        const CURRENT_VERSION: XferVersion = 1;
        let mut version = CURRENT_VERSION;
        xfer.xfer_version(&mut version, CURRENT_VERSION)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constructor_matches_cpp_defaults() {
        let draw = W3DTracerDraw::new(W3DTracerDrawModuleData::new());

        assert_eq!(draw.length(), 20.0);
        assert_eq!(draw.width(), 0.5);
        assert_eq!(draw.speed_in_dist_per_frame(), 1.0);
        assert_eq!(draw.opacity(), 1.0);
        assert_eq!(draw.color().r, 229);
        assert_eq!(draw.color().g, 204);
        assert_eq!(draw.color().b, 178);
    }

    #[test]
    fn set_tracer_parms_preserves_speed_as_dist_per_frame() {
        let mut draw = W3DTracerDraw::new(W3DTracerDrawModuleData::new());
        let color = RGBColor::new(10, 20, 30);

        draw.set_tracer_parms(9.0, 40.0, 2.5, &color, 0.75);

        assert_eq!(draw.speed_in_dist_per_frame(), 9.0);
        assert_eq!(draw.length(), 40.0);
        assert_eq!(draw.width(), 2.5);
        assert_eq!(draw.opacity(), 0.75);
        assert_eq!(draw.color().r, 10);
        assert_eq!(draw.color().g, 20);
        assert_eq!(draw.color().b, 30);
        assert_eq!(draw.line_start(), Coord3D::origin());
        assert_eq!(draw.line_end(), Coord3D::new(40.0, 0.0, 0.0));
    }

    #[test]
    fn hidden_shadow_and_shroud_hooks_match_cpp_noops() {
        let mut draw = W3DTracerDraw::new(W3DTracerDrawModuleData::new());

        draw.set_hidden(true);
        draw.set_fully_obscured_by_shroud(true);
        draw.set_shadows_enabled(false);

        assert_eq!(draw.length(), 20.0);
        assert_eq!(draw.opacity(), 1.0);
    }
}
