//! W3DLaserDraw - Laser beam rendering
//!
//! Port of C++ W3DLaserDraw.h
//! Reference: /GeneralsMD/Code/GameEngineDevice/Include/W3DDevice/GameClient/Module/W3DLaserDraw.h
//!
//! C++ parity note: `LaserUpdate` owns laser position and width lifetime. This
//! draw module only mirrors that state into beam line geometry.

use super::draw_module::*;
use crate::common::*;
use crate::helpers::TheGameLogic;
use crate::helpers::{remove_scene_line, submit_scene_line, update_scene_line};
use crate::object::drawable::DrawableArcExt;
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::system::{SceneLineDesc, SceneLineId, Snapshotable, Xfer, XferVersion};
use game_engine::common::thing::module::{Module, ModuleData};
use std::any::Any;

#[derive(Debug, Clone)]
pub struct W3DLaserDrawModuleData {
    pub module_tag_name_key: NameKeyType,
    pub inner_color: Color,
    pub outer_color: Color,
    pub inner_beam_width: Real,
    pub outer_beam_width: Real,
    pub scroll_rate: Real,
    pub tile: bool,
    pub num_beams: u32,
    pub max_intensity_frames: u32,
    pub fade_frames: u32,
    pub texture_name: AsciiString,
    pub segments: u32,
    pub arc_height: Real,
    pub segment_overlap_ratio: Real,
    pub tiling_scalar: Real,
}

impl W3DLaserDrawModuleData {
    pub fn new() -> Self {
        Self {
            module_tag_name_key: 0,
            inner_color: Color::new(0, 0, 0, 0),
            outer_color: Color::new(0, 0, 0, 0),
            inner_beam_width: 0.0,
            outer_beam_width: 1.0,
            scroll_rate: 0.0,
            tile: false,
            num_beams: 1,
            max_intensity_frames: 0,
            fade_frames: 0,
            texture_name: AsciiString::new(),
            segments: 1,
            arc_height: 0.0,
            segment_overlap_ratio: 0.0,
            tiling_scalar: 1.0,
        }
    }

    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, W3D_LASER_DRAW_FIELDS)
    }
}

impl Default for W3DLaserDrawModuleData {
    fn default() -> Self {
        Self::new()
    }
}

fn parse_unsigned_field(
    _ini: &mut INI,
    data: &mut W3DLaserDrawModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let value = tokens
        .iter()
        .copied()
        .find(|t| *t != "=")
        .ok_or(INIError::InvalidData)?;
    let parsed = INI::parse_unsigned_int(value)?;
    data.num_beams = parsed;
    Ok(())
}

fn parse_inner_width_field(
    _ini: &mut INI,
    data: &mut W3DLaserDrawModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let value = tokens
        .iter()
        .copied()
        .find(|t| *t != "=")
        .ok_or(INIError::InvalidData)?;
    data.inner_beam_width = INI::parse_real(value)?;
    Ok(())
}

fn parse_outer_width_field(
    _ini: &mut INI,
    data: &mut W3DLaserDrawModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let value = tokens
        .iter()
        .copied()
        .find(|t| *t != "=")
        .ok_or(INIError::InvalidData)?;
    data.outer_beam_width = INI::parse_real(value)?;
    Ok(())
}

fn parse_color_tokens(tokens: &[&str]) -> Result<Color, INIError> {
    let mut r = None;
    let mut g = None;
    let mut b = None;
    let mut values = Vec::new();
    let mut i = 0;

    while i < tokens.len() {
        let token = tokens[i];
        if token == "=" {
            i += 1;
            continue;
        }
        if let Some((left, right)) = token.split_once(':') {
            let value = if right.is_empty() {
                i += 1;
                if i >= tokens.len() {
                    return Err(INIError::InvalidData);
                }
                tokens[i]
            } else {
                right
            };
            let parsed: i32 = value.parse().map_err(|_| INIError::InvalidData)?;
            if !(0..=255).contains(&parsed) {
                return Err(INIError::InvalidData);
            }
            match left.to_ascii_uppercase().as_str() {
                "R" => r = Some(parsed),
                "G" => g = Some(parsed),
                "B" => b = Some(parsed),
                _ => {}
            }
        } else {
            let parsed: i32 = token.parse().map_err(|_| INIError::InvalidData)?;
            values.push(parsed);
        }
        i += 1;
    }

    if r.is_none() || g.is_none() || b.is_none() {
        if values.len() >= 3 {
            r = Some(values[0]);
            g = Some(values[1]);
            b = Some(values[2]);
        }
    }

    let r = r.ok_or(INIError::InvalidData)?;
    let g = g.ok_or(INIError::InvalidData)?;
    let b = b.ok_or(INIError::InvalidData)?;

    if !(0..=255).contains(&r) || !(0..=255).contains(&g) || !(0..=255).contains(&b) {
        return Err(INIError::InvalidData);
    }

    Ok(Color::new(r as u8, g as u8, b as u8, 255))
}

fn parse_inner_color_field(
    _ini: &mut INI,
    data: &mut W3DLaserDrawModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.inner_color = parse_color_tokens(tokens)?;
    Ok(())
}

fn parse_outer_color_field(
    _ini: &mut INI,
    data: &mut W3DLaserDrawModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.outer_color = parse_color_tokens(tokens)?;
    Ok(())
}

fn parse_duration_frames(value: &str) -> Result<UnsignedInt, INIError> {
    INI::parse_duration_unsigned_int(value)
}

fn parse_max_intensity_field(
    _ini: &mut INI,
    data: &mut W3DLaserDrawModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let value = tokens
        .iter()
        .copied()
        .find(|t| *t != "=")
        .ok_or(INIError::InvalidData)?;
    data.max_intensity_frames = parse_duration_frames(value)?;
    Ok(())
}

fn parse_fade_frames_field(
    _ini: &mut INI,
    data: &mut W3DLaserDrawModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let value = tokens
        .iter()
        .copied()
        .find(|t| *t != "=")
        .ok_or(INIError::InvalidData)?;
    data.fade_frames = parse_duration_frames(value)?;
    Ok(())
}

fn parse_texture_field(
    _ini: &mut INI,
    data: &mut W3DLaserDrawModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let value = tokens
        .iter()
        .copied()
        .find(|t| *t != "=")
        .ok_or(INIError::InvalidData)?;
    data.texture_name = AsciiString::from(&INI::parse_ascii_string(value)?);
    Ok(())
}

fn parse_scroll_rate_field(
    _ini: &mut INI,
    data: &mut W3DLaserDrawModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let value = tokens
        .iter()
        .copied()
        .find(|t| *t != "=")
        .ok_or(INIError::InvalidData)?;
    data.scroll_rate = INI::parse_real(value)?;
    Ok(())
}

fn parse_tile_field(
    _ini: &mut INI,
    data: &mut W3DLaserDrawModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let value = tokens
        .iter()
        .copied()
        .find(|t| *t != "=")
        .ok_or(INIError::InvalidData)?;
    data.tile = INI::parse_bool(value)?;
    Ok(())
}

fn parse_segments_field(
    _ini: &mut INI,
    data: &mut W3DLaserDrawModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let value = tokens
        .iter()
        .copied()
        .find(|t| *t != "=")
        .ok_or(INIError::InvalidData)?;
    data.segments = INI::parse_unsigned_int(value)?;
    Ok(())
}

fn parse_arc_height_field(
    _ini: &mut INI,
    data: &mut W3DLaserDrawModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let value = tokens
        .iter()
        .copied()
        .find(|t| *t != "=")
        .ok_or(INIError::InvalidData)?;
    data.arc_height = INI::parse_real(value)?;
    Ok(())
}

fn parse_segment_overlap_field(
    _ini: &mut INI,
    data: &mut W3DLaserDrawModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let value = tokens
        .iter()
        .copied()
        .find(|t| *t != "=")
        .ok_or(INIError::InvalidData)?;
    data.segment_overlap_ratio = INI::parse_real(value)?;
    Ok(())
}

fn parse_tiling_scalar_field(
    _ini: &mut INI,
    data: &mut W3DLaserDrawModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let value = tokens
        .iter()
        .copied()
        .find(|t| *t != "=")
        .ok_or(INIError::InvalidData)?;
    data.tiling_scalar = INI::parse_real(value)?;
    Ok(())
}

const W3D_LASER_DRAW_FIELDS: &[FieldParse<W3DLaserDrawModuleData>] = &[
    FieldParse {
        token: "NumBeams",
        parse: parse_unsigned_field,
    },
    FieldParse {
        token: "InnerBeamWidth",
        parse: parse_inner_width_field,
    },
    FieldParse {
        token: "OuterBeamWidth",
        parse: parse_outer_width_field,
    },
    FieldParse {
        token: "InnerColor",
        parse: parse_inner_color_field,
    },
    FieldParse {
        token: "OuterColor",
        parse: parse_outer_color_field,
    },
    FieldParse {
        token: "MaxIntensityLifetime",
        parse: parse_max_intensity_field,
    },
    FieldParse {
        token: "FadeLifetime",
        parse: parse_fade_frames_field,
    },
    FieldParse {
        token: "Texture",
        parse: parse_texture_field,
    },
    FieldParse {
        token: "ScrollRate",
        parse: parse_scroll_rate_field,
    },
    FieldParse {
        token: "Tile",
        parse: parse_tile_field,
    },
    FieldParse {
        token: "Segments",
        parse: parse_segments_field,
    },
    FieldParse {
        token: "ArcHeight",
        parse: parse_arc_height_field,
    },
    FieldParse {
        token: "SegmentOverlapRatio",
        parse: parse_segment_overlap_field,
    },
    FieldParse {
        token: "TilingScalar",
        parse: parse_tiling_scalar_field,
    },
];

impl ModuleData for W3DLaserDrawModuleData {
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

impl DrawModuleData for W3DLaserDrawModuleData {
    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl Snapshotable for W3DLaserDrawModuleData {
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

pub struct W3DLaserDraw {
    data: W3DLaserDrawModuleData,
    owner_id: Option<ObjectID>,
    self_dirty: bool,
    start_pos: Coord3D,
    end_pos: Coord3D,
    width_scale: Real,
    lines: Vec<LaserLine>,
    has_texture: bool,
    texture_aspect_ratio: Real,
    scene_line_ids: Vec<Option<SceneLineId>>,
}

impl W3DLaserDraw {
    pub fn new(data: W3DLaserDrawModuleData) -> Self {
        Self {
            data,
            owner_id: None,
            self_dirty: true,
            start_pos: Coord3D::origin(),
            end_pos: Coord3D::origin(),
            width_scale: 1.0,
            lines: Vec::new(),
            has_texture: false,
            texture_aspect_ratio: 1.0,
            scene_line_ids: Vec::new(),
        }
    }

    pub fn bind_owner_id(&mut self, owner_id: ObjectID) {
        self.owner_id = Some(owner_id);
        self.self_dirty = true;
    }

    /// Get the laser template width (outer beam width)
    pub fn get_laser_template_width(&self) -> Real {
        self.data.outer_beam_width * 0.5
    }

    pub fn is_self_dirty(&self) -> bool {
        self.self_dirty
    }

    fn refresh_from_laser_update(&mut self) -> bool {
        let Some(owner_id) = self.owner_id else {
            return false;
        };
        let Some(object) = TheGameLogic::find_object_by_id(owner_id) else {
            return false;
        };
        let Ok(obj_guard) = object.read() else {
            return false;
        };
        let Some(drawable) = obj_guard.get_drawable() else {
            return false;
        };

        let mut update_positions = None;
        let mut width_scale = None;
        for module in drawable.get_draw_modules() {
            let mut matched = false;
            module.with_module(|module| {
                if let Some(laser_update) = module.get_laser_update_interface() {
                    matched = true;
                    if laser_update.is_dirty() || self.self_dirty {
                        update_positions =
                            Some((laser_update.get_start_pos(), laser_update.get_end_pos()));
                        laser_update.set_dirty(false);
                    }
                    width_scale = Some(laser_update.get_width_scale());
                }
            });
            if matched {
                break;
            }
        }

        if let Some((start, end)) = update_positions {
            self.start_pos = Coord3D::new(start[0], start[1], start[2]);
            self.end_pos = Coord3D::new(end[0], end[1], end[2]);
            self.self_dirty = false;
        }
        if let Some(width) = width_scale {
            self.width_scale = width;
        }

        update_positions.is_some()
    }

    fn ensure_lines(&mut self) {
        let beams = self.data.num_beams.max(1);
        let segments = self.data.segments.max(1);
        let total = (beams * segments) as usize;
        if self.lines.len() != total {
            self.lines = vec![LaserLine::default(); total];
            self.init_lines();
        }
    }

    fn init_lines(&mut self) {
        self.has_texture = !self.data.texture_name.is_empty();
        self.texture_aspect_ratio = 1.0;

        let (inner_r, inner_g, inner_b, inner_a) = color_components_real(self.data.inner_color);
        let (outer_r, outer_g, outer_b, outer_a) = color_components_real(self.data.outer_color);

        let beams = self.data.num_beams.max(1);
        let segments = self.data.segments.max(1);
        let total = (beams * segments) as usize;
        if self.lines.len() != total {
            self.lines.resize(total, LaserLine::default());
        }

        for segment in 0..segments {
            for beam in 0..beams {
                let index = (segment * beams + beam) as usize;
                let (width, color) = if beams == 1 {
                    let width = self.data.inner_beam_width;
                    let color = color_from_real(
                        inner_r * inner_a,
                        inner_g * inner_a,
                        inner_b * inner_a,
                        inner_a,
                    );
                    (width, color)
                } else {
                    let scale = beam as Real / (beams - 1) as Real;
                    let width = self.data.inner_beam_width
                        + scale * (self.data.outer_beam_width - self.data.inner_beam_width);
                    let alpha = inner_a + scale * (outer_a - inner_a);
                    let red = inner_r + scale * (outer_r - inner_r) * inner_a;
                    let green = inner_g + scale * (outer_g - inner_g) * inner_a;
                    let blue = inner_b + scale * (outer_b - inner_b) * inner_a;
                    (width, color_from_real(red, green, blue, alpha))
                };

                let line = &mut self.lines[index];
                line.width = width;
                line.color = color;
                line.visible = false;
            }
        }
    }

    fn release_scene_lines(&mut self) {
        for id in self.scene_line_ids.drain(..) {
            if let Some(line_id) = id {
                remove_scene_line(line_id);
            }
        }
    }

    fn sync_scene_line_count(&mut self, required: usize) {
        if self.scene_line_ids.len() == required {
            return;
        }
        self.release_scene_lines();
        self.scene_line_ids.resize(required, None);
    }
}

impl Module for W3DLaserDraw {
    fn on_drawable_bound_to_object(&mut self) {}
    fn on_delete(&mut self) {
        self.release_scene_lines();
    }
    fn get_module_name_key(&self) -> NameKeyType {
        game_engine::common::name_key_generator::NameKeyGenerator::name_to_key("W3DLaserDraw")
    }
    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.data.module_tag_name_key
    }
    fn get_module_data(&self) -> &dyn ModuleData {
        &self.data
    }
}

impl DrawModule for W3DLaserDraw {
    fn do_draw_module(&mut self, transform_mtx: &Matrix3D) {
        let _ = transform_mtx;

        let needs_update = self.refresh_from_laser_update();

        if !needs_update {
            return;
        }

        self.ensure_lines();

        let beams = self.data.num_beams.max(1);
        let segments = self.data.segments.max(1);
        let total = (beams * segments) as usize;
        self.sync_scene_line_count(total);
        let use_arc = self.data.arc_height > 0.0 && segments > 1;

        for segment in 0..segments {
            let (seg_start, seg_end) = if use_arc {
                let line_start = self.start_pos;
                let line_end = self.end_pos;
                let line_vector = line_end - line_start;
                let line_length = line_vector.length();
                let half_length = line_length * 0.5;
                if half_length <= 0.0001 {
                    (line_start, line_end)
                } else {
                    let line_middle = (line_start + line_end) * 0.5;
                    let mut start_ratio = segment as Real / segments as Real;
                    let mut end_ratio = (segment as Real + 1.0) / segments as Real;
                    if segment > 0 {
                        start_ratio -= self.data.segment_overlap_ratio;
                    }
                    if segment < segments - 1 {
                        end_ratio += self.data.segment_overlap_ratio;
                    }

                    let segment_start = line_start + line_vector * start_ratio;
                    let segment_end = line_start + line_vector * end_ratio;

                    let dist_start = (line_middle - segment_start).length();
                    let dist_end = (line_middle - segment_end).length();

                    let scaled_start = dist_start / half_length * std::f32::consts::PI * 0.5;
                    let scaled_end = dist_end / half_length * std::f32::consts::PI * 0.5;

                    let height_start = scaled_start.cos() * self.data.arc_height;
                    let height_end = scaled_end.cos() * self.data.arc_height;

                    let mut curved_start = segment_start;
                    let mut curved_end = segment_end;
                    curved_start.z += height_start;
                    curved_end.z += height_end;

                    if let Some(terrain) = crate::helpers::TheTerrainLogic::get() {
                        let ground_start =
                            terrain.get_ground_height(curved_start.x, curved_start.y, None);
                        let ground_end =
                            terrain.get_ground_height(curved_end.x, curved_end.y, None);
                        curved_start.z = curved_start.z.max(2.0 + ground_start);
                        curved_end.z = curved_end.z.max(2.0 + ground_end);
                    }

                    (curved_start, curved_end)
                }
            } else {
                (self.start_pos, self.end_pos)
            };

            let length = (seg_end - seg_start).length();

            for beam in (0..beams).rev() {
                let index = (segment * beams + beam) as usize;
                let line = &mut self.lines[index];

                let width = if beams == 1 {
                    self.data.inner_beam_width * self.width_scale
                } else {
                    let scale = beam as Real / (beams - 1) as Real;
                    (self.data.inner_beam_width
                        + scale * (self.data.outer_beam_width - self.data.inner_beam_width))
                        * self.width_scale
                };

                line.start = seg_start;
                line.end = seg_end;
                line.width = width;
                line.visible = true;

                if self.has_texture && self.data.tile && width > 0.0 {
                    let tile_factor =
                        length / width * self.texture_aspect_ratio * self.data.tiling_scalar;
                    line.tile_factor = tile_factor;
                }

                let (cr, cg, cb, _ca) = color_components_real(line.color);
                let desc = SceneLineDesc {
                    start: game_engine::common::system::geometry::Coord3D::new(
                        line.start.x,
                        line.start.y,
                        line.start.z,
                    ),
                    end: game_engine::common::system::geometry::Coord3D::new(
                        line.end.x, line.end.y, line.end.z,
                    ),
                    width: line.width,
                    color_r: cr,
                    color_g: cg,
                    color_b: cb,
                    opacity: _ca,
                    texture_name: if self.has_texture {
                        Some(self.data.texture_name.to_string())
                    } else {
                        None
                    },
                    tile_factor: line.tile_factor,
                    visible: line.visible,
                };

                match self.scene_line_ids[index] {
                    None => {
                        self.scene_line_ids[index] = submit_scene_line(0, &desc);
                    }
                    Some(id) => {
                        update_scene_line(id, &desc);
                    }
                }
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
        _old_pos: &Coord3D,
        _old_angle: Real,
    ) {
    }
    fn react_to_geometry_change(&mut self) {}
    fn is_laser(&self) -> bool {
        true
    }

    fn get_laser_draw_interface(&self) -> Option<&dyn LaserDrawInterface> {
        Some(self)
    }

    fn get_laser_draw_interface_mut(&mut self) -> Option<&mut dyn LaserDrawInterface> {
        Some(self)
    }
}

impl LaserDrawInterface for W3DLaserDraw {
    fn get_laser_template_width(&self) -> Real {
        self.data.outer_beam_width * 0.5
    }
}

impl Snapshotable for W3DLaserDraw {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        const CURRENT_VERSION: XferVersion = 1;
        let mut version = CURRENT_VERSION;
        xfer.xfer_version(&mut version, CURRENT_VERSION)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        // C++ parity: W3DLaserDraw::xfer version stamp with no persistent payload.
        const CURRENT_VERSION: XferVersion = 1;
        let mut version = CURRENT_VERSION;
        xfer.xfer_version(&mut version, CURRENT_VERSION)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        self.self_dirty = true;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
struct LaserLine {
    start: Coord3D,
    end: Coord3D,
    width: Real,
    color: Color,
    tile_factor: Real,
    visible: bool,
}

impl Default for LaserLine {
    fn default() -> Self {
        Self {
            start: Coord3D::origin(),
            end: Coord3D::origin(),
            width: 0.0,
            color: Color::transparent(),
            tile_factor: 1.0,
            visible: false,
        }
    }
}

fn color_components_real(color: Color) -> (Real, Real, Real, Real) {
    (
        color.r as Real / 255.0,
        color.g as Real / 255.0,
        color.b as Real / 255.0,
        color.a as Real / 255.0,
    )
}

fn real_to_color_channel(value: Real) -> u8 {
    (value * 255.0).clamp(0.0, 255.0) as u8
}

fn color_from_real(r: Real, g: Real, b: Real, a: Real) -> Color {
    Color::new(
        real_to_color_channel(r),
        real_to_color_channel(g),
        real_to_color_channel(b),
        real_to_color_channel(a),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constructor_matches_cpp_module_defaults() {
        let data = W3DLaserDrawModuleData::new();

        assert_eq!(data.inner_beam_width, 0.0);
        assert_eq!(data.outer_beam_width, 1.0);
        assert_eq!(data.num_beams, 1);
        assert_eq!(data.max_intensity_frames, 0);
        assert_eq!(data.fade_frames, 0);
        assert_eq!(data.scroll_rate, 0.0);
        assert!(!data.tile);
        assert_eq!(data.segments, 1);
        assert_eq!(data.arc_height, 0.0);
        assert_eq!(data.segment_overlap_ratio, 0.0);
        assert_eq!(data.tiling_scalar, 1.0);
    }

    #[test]
    fn hidden_shadow_shroud_and_geometry_hooks_are_noops() {
        let mut draw = W3DLaserDraw::new(W3DLaserDrawModuleData::new());
        draw.self_dirty = false;

        draw.set_hidden(true);
        draw.set_fully_obscured_by_shroud(true);
        draw.set_shadows_enabled(false);
        draw.react_to_geometry_change();

        assert!(!draw.is_self_dirty());
    }

    #[test]
    fn parse_duration_frames_accepts_seconds_suffix() {
        assert_eq!(parse_duration_frames("1.5s").expect("duration"), 45);
    }

    #[test]
    fn parse_duration_frames_accepts_fractional_milliseconds() {
        assert_eq!(parse_duration_frames("250.0ms").expect("duration"), 8);
    }
}
