//! W3DProjectileStreamDraw - Projectile stream rendering
//!
//! Port of C++ W3DProjectileStreamDraw.h/cpp
//! Reference: /GeneralsMD/Code/GameEngineDevice/Include/W3DDevice/GameClient/Module/W3DProjectileStreamDraw.h

use super::draw_module::*;
use crate::common::*;
use crate::helpers::TheGameClient;
use crate::object::behavior::projectile_stream_update::{
    ProjectileStreamUpdateModule, MAX_PROJECTILE_STREAM,
};
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::system::{Snapshotable, Xfer, XferVersion};
use game_engine::common::thing::module::{Module, ModuleData};
use std::any::Any;

#[derive(Debug, Clone)]
pub struct W3DProjectileStreamDrawModuleData {
    pub module_tag_name_key: NameKeyType,
    pub texture_name: AsciiString,
    pub width: Real,
    pub tile_factor: Real,
    pub scroll_rate: Real,
    pub max_segments: Int,
}

impl W3DProjectileStreamDrawModuleData {
    pub fn new() -> Self {
        Self {
            module_tag_name_key: 0,
            texture_name: AsciiString::new(),
            width: 0.0,
            tile_factor: 0.0,
            scroll_rate: 0.0,
            max_segments: 0,
        }
    }

    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, PROJECTILE_STREAM_DRAW_FIELDS)
    }
}

impl Default for W3DProjectileStreamDrawModuleData {
    fn default() -> Self {
        Self::new()
    }
}

fn parse_texture_field(
    _ini: &mut INI,
    data: &mut W3DProjectileStreamDrawModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let value = tokens
        .iter()
        .copied()
        .find(|t| *t != "=")
        .ok_or(INIError::InvalidData)?;
    let parsed = INI::parse_ascii_string(value)?;
    data.texture_name = AsciiString::from(parsed.as_str());
    Ok(())
}

fn parse_width_field(
    _ini: &mut INI,
    data: &mut W3DProjectileStreamDrawModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let value = tokens
        .iter()
        .copied()
        .find(|t| *t != "=")
        .ok_or(INIError::InvalidData)?;
    data.width = INI::parse_real(value)?;
    Ok(())
}

fn parse_tile_factor_field(
    _ini: &mut INI,
    data: &mut W3DProjectileStreamDrawModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let value = tokens
        .iter()
        .copied()
        .find(|t| *t != "=")
        .ok_or(INIError::InvalidData)?;
    data.tile_factor = INI::parse_real(value)?;
    Ok(())
}

fn parse_scroll_rate_field(
    _ini: &mut INI,
    data: &mut W3DProjectileStreamDrawModuleData,
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

fn parse_max_segments_field(
    _ini: &mut INI,
    data: &mut W3DProjectileStreamDrawModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let value = tokens
        .iter()
        .copied()
        .find(|t| *t != "=")
        .ok_or(INIError::InvalidData)?;
    data.max_segments = INI::parse_int(value)?;
    Ok(())
}

const PROJECTILE_STREAM_DRAW_FIELDS: &[FieldParse<W3DProjectileStreamDrawModuleData>] = &[
    FieldParse {
        token: "Texture",
        parse: parse_texture_field,
    },
    FieldParse {
        token: "Width",
        parse: parse_width_field,
    },
    FieldParse {
        token: "TileFactor",
        parse: parse_tile_factor_field,
    },
    FieldParse {
        token: "ScrollRate",
        parse: parse_scroll_rate_field,
    },
    FieldParse {
        token: "MaxSegments",
        parse: parse_max_segments_field,
    },
];

impl ModuleData for W3DProjectileStreamDrawModuleData {
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

impl DrawModuleData for W3DProjectileStreamDrawModuleData {
    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl Snapshotable for W3DProjectileStreamDrawModuleData {
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

pub struct W3DProjectileStreamDraw {
    data: W3DProjectileStreamDrawModuleData,
    cached_lines: Vec<Vec<Coord3D>>,
    owner_id: Option<ObjectID>,
    hidden: bool,
    fully_obscured_by_shroud: bool,
    shadows_enabled: bool,
}

impl W3DProjectileStreamDraw {
    pub fn new(data: W3DProjectileStreamDrawModuleData) -> Self {
        Self {
            data,
            cached_lines: Vec::new(),
            owner_id: None,
            hidden: false,
            fully_obscured_by_shroud: false,
            shadows_enabled: false,
        }
    }

    pub fn bind_owner_id(&mut self, owner_id: ObjectID) {
        self.owner_id = Some(owner_id);
    }

    fn build_lines_from_points(&self, points: &[Coord3D]) -> Vec<Vec<Coord3D>> {
        let mut lines = Vec::new();
        let mut staging: Vec<Coord3D> = Vec::with_capacity(MAX_PROJECTILE_STREAM);
        let zero = Coord3D::origin();

        let mut current = 0usize;
        while current < points.len() {
            while current < points.len() && points[current] != zero {
                staging.push(points[current]);
                current += 1;
            }

            if staging.len() > 1 {
                lines.push(staging.clone());
            }

            current += 1;
            staging.clear();
        }

        lines
    }
}

impl Module for W3DProjectileStreamDraw {
    fn on_drawable_bound_to_object(&mut self) {}
    fn on_delete(&mut self) {}
    fn get_module_name_key(&self) -> NameKeyType {
        self.data.module_tag_name_key
    }
    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.data.module_tag_name_key
    }
    fn get_module_data(&self) -> &dyn ModuleData {
        &self.data
    }
}

impl DrawModule for W3DProjectileStreamDraw {
    fn do_draw_module(&mut self, _transform_mtx: &Matrix3D) {
        let Some(owner_id) = self.owner_id else {
            return;
        };

        if self.hidden || self.fully_obscured_by_shroud {
            self.cached_lines.clear();
            if let Some(client) = TheGameClient::get() {
                client.set_drawable_projectile_stream(
                    owner_id,
                    Vec::new(),
                    self.data.texture_name.clone(),
                    self.data.width,
                    self.data.tile_factor,
                    self.data.scroll_rate,
                );
            }
            return;
        }

        let Some(object) = crate::helpers::TheGameLogic::find_object_by_id(owner_id) else {
            return;
        };
        let Ok(obj_guard) = object.read() else {
            return;
        };

        let mut update = None;
        if let Some(module) = obj_guard.find_update_module("ProjectileStreamUpdate") {
            module.with_module_downcast::<ProjectileStreamUpdateModule, _, _>(|module| {
                update = Some(module.behavior_mut().get_all_points());
            });
        }

        let Some(points) = update else {
            return;
        };

        let mut points = points;
        if self.data.max_segments > 0 && points.len() > self.data.max_segments as usize {
            points = points[points.len() - self.data.max_segments as usize..].to_vec();
        }

        let lines = self.build_lines_from_points(&points);
        self.cached_lines = lines.clone();

        if let Some(client) = TheGameClient::get() {
            client.set_drawable_projectile_stream(
                owner_id,
                lines,
                self.data.texture_name.clone(),
                self.data.width,
                self.data.tile_factor,
                self.data.scroll_rate,
            );
        }
    }

    fn set_shadows_enabled(&mut self, enable: bool) {
        self.shadows_enabled = enable;
    }
    fn release_shadows(&mut self) {}
    fn allocate_shadows(&mut self) {}
    fn set_hidden(&mut self, hidden: bool) {
        self.hidden = hidden;
    }
    fn set_fully_obscured_by_shroud(&mut self, fully_obscured: bool) {
        self.fully_obscured_by_shroud = fully_obscured;
    }
    fn react_to_transform_change(
        &mut self,
        _old_mtx: &Matrix3D,
        _old_pos: &Coord3D,
        _old_angle: Real,
    ) {
    }
    fn react_to_geometry_change(&mut self) {
        self.cached_lines.clear();
    }
}

impl Snapshotable for W3DProjectileStreamDraw {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        // C++ parity: W3DProjectileStreamDraw::xfer version stamp with no payload.
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
