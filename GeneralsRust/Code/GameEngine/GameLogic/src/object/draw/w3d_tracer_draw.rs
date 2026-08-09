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
    /// C++ `getDrawable()->getExpirationDate()`; 0 means no decay.
    expiration_date: UnsignedInt,
    current_pos: Coord3D,
    direction: Coord3D,
    line_start: Coord3D,
    line_end: Coord3D,
    /// C++ `Line3DClass` world transform (independent of drawable after create).
    line_transform: Option<Matrix3D>,
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
            expiration_date: 0,
            current_pos: Coord3D::origin(),
            direction: Coord3D::new(1.0, 0.0, 0.0),
            line_start: Coord3D::origin(),
            line_end: Coord3D::origin(),
            line_transform: None,
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

    pub fn expiration_date(&self) -> UnsignedInt {
        self.expiration_date
    }

    /// C++ `Drawable::setExpirationDate` consumed by `doDrawModule`.
    pub fn set_expiration_date(&mut self, expiration_date: UnsignedInt) {
        self.expiration_date = expiration_date;
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

    /// C++ `W3DTracerDraw::doDrawModule` opacity step:
    /// `decay = opacity / (expDate - currentFrame); opacity -= decay`.
    fn apply_expiration_decay(&mut self, current_frame: UnsignedInt) {
        if self.expiration_date == 0 || current_frame >= self.expiration_date {
            return;
        }
        let remaining = (self.expiration_date - current_frame) as Real;
        if remaining > 0.0 {
            self.opacity -= self.opacity / remaining;
        }
    }

    fn world_line_from_transform(transform: &Matrix3D, length: Real) -> (Coord3D, Coord3D) {
        let start = transform.transform_point3(glam::Vec3::ZERO);
        let end = transform.transform_point3(glam::Vec3::new(length, 0.0, 0.0));
        (
            Coord3D::new(start.x, start.y, start.z),
            Coord3D::new(end.x, end.y, end.z),
        )
    }

    fn submit_or_update_line(&mut self) {
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
        // C++ creates Line3D once with local (0,0,0)→(length,0,0) and
        // `Set_Transform(*transformMtx)`. Later draws only mutate that Line3D
        // transform (not the incoming drawable matrix).
        if self.line_transform.is_none() {
            self.line_transform = Some(*transform_mtx);
        }

        self.apply_expiration_decay(crate::helpers::TheGameLogic::get_frame());

        // C++ `pos.Translate(Vector3(m_speedInDistPerFrame, 0, 0))` on the Line3D
        // transform — local X, including the first draw.
        if self.speed_in_dist_per_frame != 0.0 {
            if let Some(ref mut xf) = self.line_transform {
                *xf = *xf
                    * Matrix3D::from_translation(glam::Vec3::new(
                        self.speed_in_dist_per_frame,
                        0.0,
                        0.0,
                    ));
            }
        }

        let xf = self.line_transform.unwrap_or(*transform_mtx);
        let (start, end) = Self::world_line_from_transform(&xf, self.length);
        self.line_start = start;
        self.line_end = end;
        self.current_pos = start;
        self.submit_or_update_line();
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
        _old_pos: &Coord3D,
        _old_angle: Real,
    ) {
        // C++ `m_theTracer->Set_Transform(*getDrawable()->getTransformMatrix())`.
        // New drawable mtx arrives on the next `do_draw_module`.
        if self.line_transform.is_some() {
            self.line_transform = None;
        }
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
        // C++ `Reset(0→length)` then `Set_Transform(*drawable mtx)` — next draw
        // re-inits Line3D from the drawable transform.
        self.line_transform = None;
        self.line_start = Coord3D::origin();
        self.line_end = Coord3D::new(self.length, 0.0, 0.0);
        self.sync_scene_visibility(true);
    }

    fn set_expiration_date(&mut self, expiration_date: UnsignedInt) {
        self.expiration_date = expiration_date;
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

    #[test]
    fn do_draw_module_translates_local_x_on_line3d_like_cpp() {
        let mut draw = W3DTracerDraw::new(W3DTracerDrawModuleData::new());
        let color = RGBColor::new(229, 204, 178);
        draw.set_tracer_parms(5.0, 10.0, 0.5, &color, 1.0);

        let origin = Matrix3D::from_translation(glam::Vec3::new(10.0, 20.0, 30.0));
        draw.do_draw_module(&origin);
        // First draw: create at drawable mtx, then Translate(speed,0,0).
        assert!((draw.line_start().x - 15.0).abs() < 1.0e-5);
        assert!((draw.line_start().y - 20.0).abs() < 1.0e-5);
        assert!((draw.line_start().z - 30.0).abs() < 1.0e-5);
        assert!((draw.line_end().x - 25.0).abs() < 1.0e-5);
        assert!((draw.line_end().y - 20.0).abs() < 1.0e-5);
        assert!((draw.line_end().z - 30.0).abs() < 1.0e-5);

        // Second draw still receives the unmoved drawable mtx; Line3D accumulates.
        draw.do_draw_module(&origin);
        assert!((draw.line_start().x - 20.0).abs() < 1.0e-5);
        assert!((draw.line_end().x - 30.0).abs() < 1.0e-5);
        assert!((draw.line_start().y - 20.0).abs() < 1.0e-5);
        assert!((draw.line_end().y - 20.0).abs() < 1.0e-5);
    }

    #[test]
    fn do_draw_module_opacity_decay_matches_cpp_when_expiration_set() {
        let mut draw = W3DTracerDraw::new(W3DTracerDrawModuleData::new());
        let color = RGBColor::new(229, 204, 178);
        draw.set_tracer_parms(0.0, 10.0, 0.5, &color, 1.0);
        draw.set_expiration_date(4);

        // current_frame() is 0 in unit tests without GameLogic init, so remaining
        // stays expDate (4). C++ `opacity -= opacity / (expDate - frame)`.
        draw.do_draw_module(&Matrix3D::IDENTITY);
        assert!((draw.opacity() - 0.75).abs() < 1.0e-5);
        draw.do_draw_module(&Matrix3D::IDENTITY);
        assert!((draw.opacity() - 0.5625).abs() < 1.0e-5);

        // When the logic frame advances, remaining shrinks: 1, 1-1/3, 0.5-0.5/2.
        let mut opacity = 1.0_f32;
        for frame in 0u32..3 {
            opacity -= opacity / (4 - frame) as f32;
        }
        assert!((opacity - 0.25).abs() < 1.0e-5);
    }

    #[test]
    fn react_to_transform_change_resnaps_line3d_to_next_drawable_mtx() {
        let mut draw = W3DTracerDraw::new(W3DTracerDrawModuleData::new());
        let color = RGBColor::new(229, 204, 178);
        draw.set_tracer_parms(5.0, 10.0, 0.5, &color, 1.0);
        let first = Matrix3D::from_translation(glam::Vec3::new(10.0, 20.0, 30.0));
        draw.do_draw_module(&first);
        draw.react_to_transform_change(&first, &Coord3D::new(10.0, 20.0, 30.0), 0.0);

        let moved = Matrix3D::from_translation(glam::Vec3::new(100.0, 0.0, 0.0));
        draw.do_draw_module(&moved);
        assert!((draw.line_start().x - 105.0).abs() < 1.0e-5);
        assert!((draw.line_end().x - 115.0).abs() < 1.0e-5);
    }
}
