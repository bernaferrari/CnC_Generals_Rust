//! W3DRopeDraw - Rope rendering module
//!
//! Port of C++ W3DRopeDraw.h/cpp
//! Reference: /GeneralsMD/Code/GameEngineDevice/Include/W3DDevice/GameClient/Module/W3DRopeDraw.h
//!
//! ## Rendering Gap
//!
//! Active implementation in the draw pipeline (instantiated by `module_overrides.rs`,
//! dispatched by `GameLogic Drawable::draw()`). However, `do_draw_module()` only
//! computes segment positions in memory — it never creates `SegmentedLine` objects
//! in `W3DDisplay::global_scene()` because GameLogic cannot depend on GameEngineDevice.
//!
//! Reference rendering: `GameEngineDevice/.../Drawable/Draw/wthree_d_rope_draw.rs`

use super::draw_module::*;
use crate::common::*;
use crate::helpers::get_game_logic_random_value_real;
use crate::helpers::{remove_scene_line, submit_scene_line, update_scene_line};
use game_engine::common::system::{SceneLineDesc, SceneLineId, Snapshotable, Xfer, XferVersion};
use game_engine::common::thing::module::{Module, ModuleData};
use std::any::Any;

#[derive(Debug, Clone)]
pub struct W3DRopeDrawModuleData {
    module_tag_name_key: NameKeyType,
}

impl W3DRopeDrawModuleData {
    pub fn new() -> Self {
        Self {
            module_tag_name_key: 0,
        }
    }
}

impl Default for W3DRopeDrawModuleData {
    fn default() -> Self {
        Self::new()
    }
}

impl ModuleData for W3DRopeDrawModuleData {
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

impl DrawModuleData for W3DRopeDrawModuleData {
    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl Snapshotable for W3DRopeDrawModuleData {
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

#[derive(Debug, Clone)]
struct RopeSegment {
    start: Coord3D,
    end: Coord3D,
    wobble_axis_x: Real,
    wobble_axis_y: Real,
}

pub struct W3DRopeDraw {
    _data: W3DRopeDrawModuleData,
    segments: Vec<RopeSegment>,
    cur_len: Real,
    max_len: Real,
    width: Real,
    color: RGBColor,
    cur_speed: Real,
    max_speed: Real,
    accel: Real,
    wobble_len: Real,
    wobble_amp: Real,
    wobble_rate: Real,
    cur_wobble_phase: Real,
    cur_z_offset: Real,
    segment_line_ids: Vec<Option<SceneLineId>>,
    hidden: bool,
    fully_obscured_by_shroud: bool,
    shadows_enabled: bool,
}

impl W3DRopeDraw {
    pub fn new(data: W3DRopeDrawModuleData) -> Self {
        Self {
            _data: data,
            segments: Vec::new(),
            cur_len: 0.0,
            max_len: 1.0,
            width: 0.5,
            color: RGBColor::black(),
            cur_speed: 0.0,
            max_speed: 0.0,
            accel: 0.0,
            wobble_len: 1.0,
            wobble_amp: 0.0,
            wobble_rate: 0.0,
            cur_wobble_phase: 0.0,
            cur_z_offset: 0.0,
            segment_line_ids: Vec::new(),
            hidden: false,
            fully_obscured_by_shroud: false,
            shadows_enabled: false,
        }
    }

    fn toss_segments(&mut self) {
        self.segments.clear();
        for id in self.segment_line_ids.drain(..) {
            if let Some(line_id) = id {
                remove_scene_line(line_id);
            }
        }
    }

    fn build_segments(&mut self) {
        self.segments.clear();
        let wobble_len = if self.wobble_len.abs() < 0.001 {
            0.001
        } else {
            self.wobble_len.abs()
        };
        let num_segs = (self.max_len / wobble_len).ceil().max(1.0) as usize;
        let each_len = self.max_len / num_segs as Real;
        let mut z = 0.0;
        for _ in 0..num_segs {
            let axis = get_game_logic_random_value_real(0.0, 2.0 * std::f32::consts::PI);
            let seg = RopeSegment {
                start: Coord3D::new(0.0, 0.0, z),
                end: Coord3D::new(0.0, 0.0, z + each_len),
                wobble_axis_x: axis.cos(),
                wobble_axis_y: axis.sin(),
            };
            self.segments.push(seg);
            z += each_len;
        }
    }

    fn sync_segment_visibility(&mut self, visible: bool) {
        for (i, seg) in self.segments.iter().enumerate() {
            let Some(Some(id)) = self.segment_line_ids.get(i) else {
                continue;
            };
            let desc = SceneLineDesc {
                start: game_engine::common::system::geometry::Coord3D::new(
                    seg.start.x,
                    seg.start.y,
                    seg.start.z,
                ),
                end: game_engine::common::system::geometry::Coord3D::new(
                    seg.end.x, seg.end.y, seg.end.z,
                ),
                width: self.width,
                color_r: self.color.r as f32 / 255.0,
                color_g: self.color.g as f32 / 255.0,
                color_b: self.color.b as f32 / 255.0,
                opacity: 1.0,
                texture_name: None,
                tile_factor: 0.0,
                visible,
            };
            update_scene_line(*id, &desc);
        }
    }
}

impl Module for W3DRopeDraw {
    fn on_drawable_bound_to_object(&mut self) {}
    fn on_delete(&mut self) {
        self.toss_segments();
    }
    fn get_module_name_key(&self) -> NameKeyType {
        self._data.module_tag_name_key
    }
    fn get_module_tag_name_key(&self) -> NameKeyType {
        self._data.module_tag_name_key
    }
    fn get_module_data(&self) -> &dyn ModuleData {
        &self._data
    }
}

impl DrawModule for W3DRopeDraw {
    fn do_draw_module(&mut self, transform_mtx: &Matrix3D) {
        if self.hidden || self.fully_obscured_by_shroud {
            self.sync_segment_visibility(false);
            return;
        }

        if self.segments.is_empty() {
            self.build_segments();
        }

        if !self.segments.is_empty() {
            let (_, _, translation) = transform_mtx.to_scale_rotation_translation();
            let deflection = self.cur_wobble_phase.sin() * self.wobble_amp;
            let mut start = Coord3D::new(
                translation.x,
                translation.y,
                translation.z + self.cur_z_offset,
            );
            let each_len = self.cur_len / self.segments.len() as Real;
            for seg in &mut self.segments {
                let end = Coord3D::new(
                    translation.x + deflection * seg.wobble_axis_x,
                    translation.y + deflection * seg.wobble_axis_y,
                    start.z - each_len,
                );
                seg.start = start;
                seg.end = end;
                start = end;
            }

            while self.segment_line_ids.len() < self.segments.len() {
                self.segment_line_ids.push(None);
            }

            for (i, seg) in self.segments.iter().enumerate() {
                let desc = SceneLineDesc {
                    start: game_engine::common::system::geometry::Coord3D::new(
                        seg.start.x,
                        seg.start.y,
                        seg.start.z,
                    ),
                    end: game_engine::common::system::geometry::Coord3D::new(
                        seg.end.x, seg.end.y, seg.end.z,
                    ),
                    width: self.width,
                    color_r: self.color.r as f32 / 255.0,
                    color_g: self.color.g as f32 / 255.0,
                    color_b: self.color.b as f32 / 255.0,
                    opacity: 1.0,
                    texture_name: None,
                    tile_factor: 0.0,
                    visible: true,
                };

                match self.segment_line_ids[i] {
                    None => {
                        self.segment_line_ids[i] = submit_scene_line(0, &desc);
                    }
                    Some(id) => {
                        update_scene_line(id, &desc);
                    }
                }
            }
        }

        self.cur_wobble_phase += self.wobble_rate;
        if self.cur_wobble_phase > 2.0 * std::f32::consts::PI {
            self.cur_wobble_phase -= 2.0 * std::f32::consts::PI;
        }

        self.cur_z_offset += self.cur_speed;
        self.cur_speed += self.accel;
        if self.cur_speed > self.max_speed {
            self.cur_speed = self.max_speed;
        } else if self.cur_speed < -self.max_speed {
            self.cur_speed = -self.max_speed;
        }
    }

    fn set_shadows_enabled(&mut self, enable: bool) {
        self.shadows_enabled = enable;
    }
    fn release_shadows(&mut self) {}
    fn allocate_shadows(&mut self) {}
    fn set_hidden(&mut self, hidden: bool) {
        self.hidden = hidden;
        self.sync_segment_visibility(!hidden && !self.fully_obscured_by_shroud);
    }
    fn set_fully_obscured_by_shroud(&mut self, fully_obscured: bool) {
        self.fully_obscured_by_shroud = fully_obscured;
        self.sync_segment_visibility(!fully_obscured && !self.hidden);
    }
    fn react_to_transform_change(
        &mut self,
        _old_mtx: &Matrix3D,
        _old_pos: &Coord3D,
        _old_angle: Real,
    ) {
    }
    fn react_to_geometry_change(&mut self) {
        self.toss_segments();
    }

    fn get_rope_draw_interface(&self) -> Option<&dyn RopeDrawInterface> {
        Some(self)
    }

    fn get_rope_draw_interface_mut(&mut self) -> Option<&mut dyn RopeDrawInterface> {
        Some(self)
    }
}

impl RopeDrawInterface for W3DRopeDraw {
    fn init_rope_parms(
        &mut self,
        length: Real,
        width: Real,
        color: &RGBColor,
        wobble_len: Real,
        wobble_amp: Real,
        wobble_rate: Real,
    ) {
        self.max_len = length.max(1.0);
        self.cur_len = 0.0;
        self.width = width;
        self.color = *color;
        self.wobble_len = self.max_len.min(wobble_len);
        self.wobble_amp = wobble_amp;
        self.wobble_rate = wobble_rate;
        self.cur_z_offset = 0.0;
        self.toss_segments();
        self.build_segments();
    }

    fn set_rope_cur_len(&mut self, length: Real) {
        self.cur_len = length;
    }

    fn set_rope_speed(&mut self, cur_speed: Real, max_speed: Real, accel: Real) {
        self.cur_speed = cur_speed;
        self.max_speed = max_speed;
        self.accel = accel;
    }
}

impl Snapshotable for W3DRopeDraw {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        const CURRENT_VERSION: XferVersion = 1;
        let mut version = CURRENT_VERSION;
        xfer.xfer_version(&mut version, CURRENT_VERSION)
            .map_err(|e| e.to_string())?;

        // C++ parity: DrawModule::xfer -> DrawableModule::xfer -> Module::xfer
        // Each writes a version(1) byte. Match the 3-byte base class chain.
        let mut draw_module_version: XferVersion = 1;
        xfer.xfer_version(&mut draw_module_version, 1).map_err(|e| e.to_string())?;
        let mut drawable_module_version: XferVersion = 1;
        xfer.xfer_version(&mut drawable_module_version, 1).map_err(|e| e.to_string())?;
        let mut module_version: XferVersion = 1;
        xfer.xfer_version(&mut module_version, 1).map_err(|e| e.to_string())?;

        xfer.xfer_real(&mut self.cur_len)
            .map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut self.max_len)
            .map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut self.width).map_err(|e| e.to_string())?;
        let mut color_r = self.color.r as Real;
        let mut color_g = self.color.g as Real;
        let mut color_b = self.color.b as Real;
        xfer.xfer_real(&mut color_r).map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut color_g).map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut color_b).map_err(|e| e.to_string())?;
        if xfer.is_reading() {
            self.color = RGBColor::new(
                color_r.clamp(0.0, 255.0) as u8,
                color_g.clamp(0.0, 255.0) as u8,
                color_b.clamp(0.0, 255.0) as u8,
            );
        }
        xfer.xfer_real(&mut self.cur_speed)
            .map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut self.max_speed)
            .map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut self.accel).map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut self.wobble_len)
            .map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut self.wobble_amp)
            .map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut self.wobble_rate)
            .map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut self.cur_wobble_phase)
            .map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut self.cur_z_offset)
            .map_err(|e| e.to_string())?;

        if xfer.is_reading() {
            self.toss_segments();
        }

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}
