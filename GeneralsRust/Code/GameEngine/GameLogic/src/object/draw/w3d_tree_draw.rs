//! W3DTreeDraw - Destructible tree rendering
//!
//! Port of C++ W3DTreeDraw.h
//! Reference: /GeneralsMD/Code/GameEngineDevice/Include/W3DDevice/GameClient/Module/W3DTreeDraw.h
//!
//! C++ parity note: `W3DTreeDraw` only registers terrain trees once. Tree
//! push/topple/sink behavior is owned by the terrain tree buffer path.

use super::draw_module::*;
use crate::common::*;
use crate::helpers::{TheGameClient, TERRAIN_TREE_STATE};
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::system::{Snapshotable, Xfer, XferVersion};
use game_engine::common::thing::module::{Module, ModuleData};
use std::any::Any;

#[derive(Debug, Clone)]
pub struct W3DTreeDrawModuleData {
    pub module_tag_name_key: NameKeyType,
    pub model_name: AsciiString,
    pub texture_name: AsciiString,

    // Push aside parameters
    pub frames_to_move_outward: u32,
    pub frames_to_move_inward: u32,
    pub max_outward_movement: Real,
    pub darkening: Real,

    // Topple parameters
    pub topple_fx: Option<AsciiString>,
    pub bounce_fx: Option<AsciiString>,
    pub stump_name: AsciiString,
    pub initial_velocity_percent: Real,
    pub initial_accel_percent: Real,
    pub bounce_velocity_percent: Real,
    pub minimum_topple_speed: Real,
    pub kill_when_toppled: bool,
    pub do_topple: bool,
    pub sink_frames: u32,
    pub sink_distance: Real,

    pub do_shadow: bool,
}

impl W3DTreeDrawModuleData {
    pub fn new() -> Self {
        Self {
            module_tag_name_key: 0,
            model_name: AsciiString::new(),
            texture_name: AsciiString::new(),
            // C++ defaults from W3DTreeDrawModuleData::W3DTreeDrawModuleData.
            frames_to_move_outward: 1,
            frames_to_move_inward: 1,
            max_outward_movement: 1.0,
            darkening: 0.0,
            topple_fx: None,
            bounce_fx: None,
            stump_name: AsciiString::new(),
            initial_velocity_percent: 0.2,
            initial_accel_percent: 0.01,
            bounce_velocity_percent: 0.3,
            minimum_topple_speed: 0.5,
            kill_when_toppled: true,
            do_topple: false,
            sink_frames: 10 * LOGICFRAMES_PER_SECOND,
            sink_distance: 20.0,
            do_shadow: false,
        }
    }

    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, W3D_TREE_DRAW_FIELDS)
    }
}

fn parse_field_value<'a>(tokens: &'a [&str]) -> Result<&'a str, INIError> {
    tokens
        .iter()
        .copied()
        .find(|t| *t != "=")
        .ok_or(INIError::InvalidData)
}

fn parse_model_name(
    _ini: &mut INI,
    data: &mut W3DTreeDrawModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let value = INI::parse_ascii_string(parse_field_value(tokens)?)?;
    data.model_name = AsciiString::from(value.as_str());
    Ok(())
}

fn parse_texture_name(
    _ini: &mut INI,
    data: &mut W3DTreeDrawModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let value = INI::parse_ascii_string(parse_field_value(tokens)?)?;
    data.texture_name = AsciiString::from(value.as_str());
    Ok(())
}

fn parse_move_outward_time(
    _ini: &mut INI,
    data: &mut W3DTreeDrawModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.frames_to_move_outward = INI::parse_duration_unsigned_int(parse_field_value(tokens)?)?;
    Ok(())
}

fn parse_move_inward_time(
    _ini: &mut INI,
    data: &mut W3DTreeDrawModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.frames_to_move_inward = INI::parse_duration_unsigned_int(parse_field_value(tokens)?)?;
    Ok(())
}

fn parse_move_outward_distance_factor(
    _ini: &mut INI,
    data: &mut W3DTreeDrawModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.max_outward_movement = INI::parse_real(parse_field_value(tokens)?)?;
    Ok(())
}

fn parse_darkening_factor(
    _ini: &mut INI,
    data: &mut W3DTreeDrawModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.darkening = INI::parse_real(parse_field_value(tokens)?)?;
    Ok(())
}

fn parse_topple_fx(
    _ini: &mut INI,
    data: &mut W3DTreeDrawModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let name = INI::parse_ascii_string(parse_field_value(tokens)?)?;
    data.topple_fx = if name.is_empty() {
        None
    } else {
        Some(AsciiString::from(name.as_str()))
    };
    Ok(())
}

fn parse_bounce_fx(
    _ini: &mut INI,
    data: &mut W3DTreeDrawModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let name = INI::parse_ascii_string(parse_field_value(tokens)?)?;
    data.bounce_fx = if name.is_empty() {
        None
    } else {
        Some(AsciiString::from(name.as_str()))
    };
    Ok(())
}

fn parse_stump_name(
    _ini: &mut INI,
    data: &mut W3DTreeDrawModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let value = INI::parse_ascii_string(parse_field_value(tokens)?)?;
    data.stump_name = AsciiString::from(value.as_str());
    Ok(())
}

fn parse_kill_when_finished_toppling(
    _ini: &mut INI,
    data: &mut W3DTreeDrawModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.kill_when_toppled = INI::parse_bool(parse_field_value(tokens)?)?;
    Ok(())
}

fn parse_do_topple(
    _ini: &mut INI,
    data: &mut W3DTreeDrawModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.do_topple = INI::parse_bool(parse_field_value(tokens)?)?;
    Ok(())
}

fn parse_initial_velocity_percent(
    _ini: &mut INI,
    data: &mut W3DTreeDrawModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.initial_velocity_percent = INI::parse_percent_to_real(parse_field_value(tokens)?)?;
    Ok(())
}

fn parse_initial_accel_percent(
    _ini: &mut INI,
    data: &mut W3DTreeDrawModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.initial_accel_percent = INI::parse_percent_to_real(parse_field_value(tokens)?)?;
    Ok(())
}

fn parse_bounce_velocity_percent(
    _ini: &mut INI,
    data: &mut W3DTreeDrawModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.bounce_velocity_percent = INI::parse_percent_to_real(parse_field_value(tokens)?)?;
    Ok(())
}

fn parse_minimum_topple_speed(
    _ini: &mut INI,
    data: &mut W3DTreeDrawModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let value = INI::parse_real(parse_field_value(tokens)?)?;
    if value <= 0.0 {
        return Err(INIError::InvalidData);
    }
    data.minimum_topple_speed = value;
    Ok(())
}

fn parse_sink_distance(
    _ini: &mut INI,
    data: &mut W3DTreeDrawModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let value = INI::parse_real(parse_field_value(tokens)?)?;
    if value <= 0.0 {
        return Err(INIError::InvalidData);
    }
    data.sink_distance = value;
    Ok(())
}

fn parse_sink_time(
    _ini: &mut INI,
    data: &mut W3DTreeDrawModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.sink_frames = INI::parse_duration_unsigned_int(parse_field_value(tokens)?)?;
    Ok(())
}

fn parse_do_shadow(
    _ini: &mut INI,
    data: &mut W3DTreeDrawModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.do_shadow = INI::parse_bool(parse_field_value(tokens)?)?;
    Ok(())
}

const W3D_TREE_DRAW_FIELDS: &[FieldParse<W3DTreeDrawModuleData>] = &[
    FieldParse {
        token: "ModelName",
        parse: parse_model_name,
    },
    FieldParse {
        token: "TextureName",
        parse: parse_texture_name,
    },
    FieldParse {
        token: "MoveOutwardTime",
        parse: parse_move_outward_time,
    },
    FieldParse {
        token: "MoveInwardTime",
        parse: parse_move_inward_time,
    },
    FieldParse {
        token: "MoveOutwardDistanceFactor",
        parse: parse_move_outward_distance_factor,
    },
    FieldParse {
        token: "DarkeningFactor",
        parse: parse_darkening_factor,
    },
    FieldParse {
        token: "ToppleFX",
        parse: parse_topple_fx,
    },
    FieldParse {
        token: "BounceFX",
        parse: parse_bounce_fx,
    },
    FieldParse {
        token: "StumpName",
        parse: parse_stump_name,
    },
    FieldParse {
        token: "KillWhenFinishedToppling",
        parse: parse_kill_when_finished_toppling,
    },
    FieldParse {
        token: "DoTopple",
        parse: parse_do_topple,
    },
    FieldParse {
        token: "InitialVelocityPercent",
        parse: parse_initial_velocity_percent,
    },
    FieldParse {
        token: "InitialAccelPercent",
        parse: parse_initial_accel_percent,
    },
    FieldParse {
        token: "BounceVelocityPercent",
        parse: parse_bounce_velocity_percent,
    },
    FieldParse {
        token: "MinimumToppleSpeed",
        parse: parse_minimum_topple_speed,
    },
    FieldParse {
        token: "SinkDistance",
        parse: parse_sink_distance,
    },
    FieldParse {
        token: "SinkTime",
        parse: parse_sink_time,
    },
    FieldParse {
        token: "DoShadow",
        parse: parse_do_shadow,
    },
];

impl Default for W3DTreeDrawModuleData {
    fn default() -> Self {
        Self::new()
    }
}

impl ModuleData for W3DTreeDrawModuleData {
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

impl DrawModuleData for W3DTreeDrawModuleData {
    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl Snapshotable for W3DTreeDrawModuleData {
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

pub struct W3DTreeDraw {
    data: W3DTreeDrawModuleData,
    drawable_id: u32,
    tree_added: bool,
}

impl W3DTreeDraw {
    pub fn new(data: W3DTreeDrawModuleData) -> Self {
        Self {
            data,
            drawable_id: 0,
            tree_added: false,
        }
    }

    pub fn bind_drawable_id(&mut self, drawable_id: u32) {
        self.drawable_id = drawable_id;
    }

    fn reset_runtime_state(&mut self) {
        self.tree_added = false;
    }

    fn unregister_tree(&mut self) {
        if !self.tree_added || self.drawable_id == INVALID_ID {
            return;
        }
        if let Ok(mut tree_map) = TERRAIN_TREE_STATE.lock() {
            tree_map.remove(&(self.drawable_id as u32));
        }
        self.tree_added = false;
    }

    pub fn is_tree_added(&self) -> bool {
        self.tree_added
    }
}

impl Module for W3DTreeDraw {
    fn on_drawable_bound_to_object(&mut self) {}
    fn on_delete(&mut self) {
        self.unregister_tree();
        self.reset_runtime_state();
    }
    fn get_module_name_key(&self) -> NameKeyType {
        game_engine::common::name_key_generator::NameKeyGenerator::name_to_key("W3DTreeDraw")
    }
    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.data.module_tag_name_key
    }
    fn get_module_data(&self) -> &dyn ModuleData {
        &self.data
    }
}

impl DrawModule for W3DTreeDraw {
    fn do_draw_module(&mut self, transform_mtx: &Matrix3D) {
        let (scale, rotation, translation) = transform_mtx.to_scale_rotation_translation();

        // C++ parity bridge: add the tree to terrain rendering once using current
        // drawable transform (W3DTreeDraw::reactToTransformChange + addTree call path).
        if !self.tree_added && (translation.x != 0.0 || translation.y != 0.0) {
            if let Some(client) = TheGameClient::get() {
                let (_, _, angle) = rotation.to_euler(glam::EulerRot::XYZ);
                client.add_tree(
                    self.drawable_id,
                    &translation,
                    scale.x,
                    angle,
                    0.0,
                    &self.data,
                );
                self.tree_added = true;
            }
        }

        // In C++, W3DTreeDraw::doDrawModule just returns early (W3DTreeDraw.cpp:134-137).
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
        // Tree registration is handled in do_draw_module where current transform is available.
    }
    fn react_to_geometry_change(&mut self) {}
}

impl Snapshotable for W3DTreeDraw {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        const CURRENT_VERSION: XferVersion = 1;
        let mut version = CURRENT_VERSION;
        xfer.xfer_version(&mut version, CURRENT_VERSION)
            .map_err(|e| e.to_string())?;

        let mut draw_module_version: XferVersion = 1;
        xfer.xfer_version(&mut draw_module_version, 1)
            .map_err(|e| e.to_string())?;
        let mut drawable_module_version: XferVersion = 1;
        xfer.xfer_version(&mut drawable_module_version, 1)
            .map_err(|e| e.to_string())?;
        let mut module_version: XferVersion = 1;
        xfer.xfer_version(&mut module_version, 1)
            .map_err(|e| e.to_string())?;

        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        // C++ parity: W3DTreeDraw::xfer writes version only and has no module payload.
        const CURRENT_VERSION: XferVersion = 1;
        let mut version = CURRENT_VERSION;
        xfer.xfer_version(&mut version, CURRENT_VERSION)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        // C++ parity: xfer has no payload for this module; runtime state should be
        // reconstructed from current world transform after load.
        self.reset_runtime_state();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constructor_matches_cpp_module_defaults() {
        let data = W3DTreeDrawModuleData::new();

        assert_eq!(data.frames_to_move_inward, 1);
        assert_eq!(data.frames_to_move_outward, 1);
        assert_eq!(data.darkening, 0.0);
        assert_eq!(data.max_outward_movement, 1.0);
        assert_eq!(data.initial_velocity_percent, 0.2);
        assert_eq!(data.initial_accel_percent, 0.01);
        assert_eq!(data.bounce_velocity_percent, 0.3);
        assert_eq!(data.minimum_topple_speed, 0.5);
        assert_eq!(data.sink_frames, 10 * LOGICFRAMES_PER_SECOND);
        assert_eq!(data.sink_distance, 20.0);
        assert!(data.kill_when_toppled);
        assert!(!data.do_topple);
        assert!(!data.do_shadow);
    }

    #[test]
    fn hidden_shadow_shroud_and_geometry_hooks_are_noops() {
        let mut draw = W3DTreeDraw::new(W3DTreeDrawModuleData::new());
        draw.tree_added = true;

        draw.set_hidden(true);
        draw.set_fully_obscured_by_shroud(true);
        draw.set_shadows_enabled(false);
        draw.react_to_geometry_change();

        assert!(draw.is_tree_added());
    }

    #[test]
    fn delete_clears_registration_state() {
        let mut draw = W3DTreeDraw::new(W3DTreeDrawModuleData::new());
        draw.tree_added = true;

        draw.on_delete();

        assert!(!draw.is_tree_added());
    }
}
