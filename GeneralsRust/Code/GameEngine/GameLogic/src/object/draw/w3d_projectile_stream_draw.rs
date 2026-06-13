//! W3DProjectileStreamDraw - Projectile stream rendering
//!
//! Port of C++ W3DProjectileStreamDraw.h/cpp
//! Reference: /GeneralsMD/Code/GameEngineDevice/Include/W3DDevice/GameClient/Module/W3DProjectileStreamDraw.h

use super::draw_module::*;
use crate::common::*;
use crate::helpers::TheGameClient;
use crate::object::behavior::projectile_stream_update::MAX_PROJECTILE_STREAM;
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::name_key_generator::NameKeyGenerator;
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

pub struct W3DProjectileStreamDraw {
    data: W3DProjectileStreamDrawModuleData,
    cached_lines: Vec<Vec<Coord3D>>,
    owner_id: Option<ObjectID>,
}

impl W3DProjectileStreamDraw {
    pub fn new(data: W3DProjectileStreamDrawModuleData) -> Self {
        Self {
            data,
            cached_lines: Vec::new(),
            owner_id: None,
        }
    }

    pub fn bind_owner_id(&mut self, owner_id: ObjectID) {
        self.owner_id = Some(owner_id);
    }

    pub fn owner_id(&self) -> Option<ObjectID> {
        self.owner_id
    }

    pub fn cached_lines(&self) -> &[Vec<Coord3D>] {
        &self.cached_lines
    }

    fn sync_client_projectile_stream(&self, lines: Vec<Vec<Coord3D>>) {
        let Some(owner_id) = self.owner_id else {
            return;
        };

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
        NameKeyGenerator::name_to_key("W3DProjectileStreamDraw")
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

        let Some(object) = crate::helpers::TheGameLogic::find_object_by_id(owner_id) else {
            return;
        };
        let Ok(obj_guard) = object.read() else {
            return;
        };

        let mut update: Option<Vec<crate::common::Coord3D>> = None;
        if let Some(module) = obj_guard.find_update_module("ProjectileStreamUpdate") {
            module.with_module(|module| {
                if let Some(stream) = module.get_projectile_stream_draw_interface() {
                    update = Some(
                        stream
                            .projectile_stream_points()
                            .into_iter()
                            .map(crate::common::Coord3D::from_array)
                            .collect(),
                    );
                }
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
        self.sync_client_projectile_stream(lines);
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
        if fully_obscured {
            self.sync_client_projectile_stream(Vec::new());
        } else {
            self.sync_client_projectile_stream(self.cached_lines.clone());
        }
    }
    fn react_to_transform_change(
        &mut self,
        _old_mtx: &Matrix3D,
        _old_pos: &Coord3D,
        _old_angle: Real,
    ) {
    }
    fn react_to_geometry_change(&mut self) {}
}

impl Snapshotable for W3DProjectileStreamDraw {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        const CURRENT_VERSION: XferVersion = 1;
        let mut version = CURRENT_VERSION;
        xfer.xfer_version(&mut version, CURRENT_VERSION)
            .map_err(|e| e.to_string())?;
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

#[cfg(test)]
mod tests {
    use super::*;

    fn point(x: Real, y: Real, z: Real) -> Coord3D {
        Coord3D::new(x, y, z)
    }

    #[test]
    fn module_name_key_is_projectile_stream_draw() {
        let draw = W3DProjectileStreamDraw::new(W3DProjectileStreamDrawModuleData::new());
        assert_eq!(
            draw.get_module_name_key(),
            NameKeyGenerator::name_to_key("W3DProjectileStreamDraw")
        );
    }

    #[test]
    fn bind_owner_id_sets_owner() {
        let mut draw = W3DProjectileStreamDraw::new(W3DProjectileStreamDrawModuleData::new());
        draw.bind_owner_id(42);
        assert_eq!(draw.owner_id(), Some(42));
    }

    #[test]
    fn build_lines_splits_on_zero_and_skips_single_points() {
        let draw = W3DProjectileStreamDraw::new(W3DProjectileStreamDrawModuleData::new());
        let zero = Coord3D::origin();
        let lines = draw.build_lines_from_points(&[
            point(1.0, 0.0, 0.0),
            point(2.0, 0.0, 0.0),
            zero,
            point(3.0, 0.0, 0.0),
            zero,
            point(4.0, 0.0, 0.0),
            point(5.0, 0.0, 0.0),
        ]);

        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0], vec![point(1.0, 0.0, 0.0), point(2.0, 0.0, 0.0)]);
        assert_eq!(lines[1], vec![point(4.0, 0.0, 0.0), point(5.0, 0.0, 0.0)]);
    }

    #[test]
    fn geometry_change_keeps_cached_lines_like_cpp() {
        let mut draw = W3DProjectileStreamDraw::new(W3DProjectileStreamDrawModuleData::new());
        draw.cached_lines = vec![vec![point(1.0, 2.0, 3.0), point(4.0, 5.0, 6.0)]];
        draw.react_to_geometry_change();
        assert_eq!(draw.cached_lines().len(), 1);
    }
}
