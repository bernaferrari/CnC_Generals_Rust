//! W3DTreeDraw - Destructible tree rendering
//!
//! Port of C++ W3DTreeDraw.h
//! Reference: /GeneralsMD/Code/GameEngineDevice/Include/W3DDevice/GameClient/Module/W3DTreeDraw.h
//!
//! Features:
//! - Push aside when units move through
//! - Topple and fall when destroyed
//! - Bounce effects on impact
//! - Sink into ground after falling

use super::draw_module::*;
use crate::common::*;
use crate::helpers::{TheFXListStore, TheGameClient};
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TreeState {
    Standing,
    PushingAside,
    Returning,
    Toppling,
    Fallen,
    Sinking,
}

pub struct W3DTreeDraw {
    data: W3DTreeDrawModuleData,
    drawable_id: u32,
    tree_added: bool,
    tree_state: TreeState,
    current_offset: Coord3D,
    current_rotation: Real,
    push_direction: Coord3D,
    animation_frame: u32,
    topple_velocity: Real,
    sink_amount: Real,
    last_world_position: Coord3D,
}

impl W3DTreeDraw {
    pub fn new(data: W3DTreeDrawModuleData) -> Self {
        Self {
            data,
            drawable_id: 0,
            tree_added: false,
            tree_state: TreeState::Standing,
            current_offset: Coord3D::origin(),
            current_rotation: 0.0,
            push_direction: Coord3D::origin(),
            animation_frame: 0,
            topple_velocity: 0.0,
            sink_amount: 0.0,
            last_world_position: Coord3D::origin(),
        }
    }

    pub fn bind_drawable_id(&mut self, drawable_id: u32) {
        self.drawable_id = drawable_id;
    }

    fn reset_runtime_state(&mut self) {
        self.tree_added = false;
        self.tree_state = TreeState::Standing;
        self.current_offset = Coord3D::origin();
        self.current_rotation = 0.0;
        self.push_direction = Coord3D::origin();
        self.animation_frame = 0;
        self.topple_velocity = 0.0;
        self.sink_amount = 0.0;
        self.last_world_position = Coord3D::origin();
    }

    pub fn push_aside(&mut self, direction: &Coord3D) {
        if self.tree_state == TreeState::Standing {
            self.tree_state = TreeState::PushingAside;
            self.push_direction = *direction;
            self.animation_frame = 0;
        }
    }

    pub fn topple(&mut self, impact_velocity: Real) {
        if !self.data.do_topple {
            return;
        }

        if impact_velocity < self.data.minimum_topple_speed {
            return;
        }

        self.tree_state = TreeState::Toppling;
        self.topple_velocity = impact_velocity * self.data.initial_velocity_percent;
        self.animation_frame = 0;
    }

    fn update_push_aside(&mut self) {
        match self.tree_state {
            TreeState::PushingAside => {
                self.animation_frame += 1;
                let progress =
                    self.animation_frame as Real / self.data.frames_to_move_outward as Real;

                if progress >= 1.0 {
                    self.tree_state = TreeState::Returning;
                    self.animation_frame = 0;
                } else {
                    // Smooth interpolation
                    let t = (progress * std::f32::consts::PI).sin();
                    self.current_offset = self.push_direction * self.data.max_outward_movement * t;
                }
            }
            TreeState::Returning => {
                self.animation_frame += 1;
                let progress =
                    self.animation_frame as Real / self.data.frames_to_move_inward as Real;

                if progress >= 1.0 {
                    self.tree_state = TreeState::Standing;
                    self.current_offset = Coord3D::origin();
                } else {
                    let t = 1.0 - (progress * std::f32::consts::PI).sin();
                    self.current_offset = self.push_direction * self.data.max_outward_movement * t;
                }
            }
            _ => {}
        }
    }

    fn update_topple(&mut self) {
        if self.tree_state != TreeState::Toppling {
            return;
        }

        // Rotate tree down (per frame)
        self.current_rotation += self.topple_velocity / LOGICFRAMES_PER_SECOND as Real;
        self.topple_velocity += self.data.initial_accel_percent;

        // Check if hit ground
        if self.current_rotation >= 90.0 {
            self.current_rotation = 90.0;
            self.tree_state = TreeState::Fallen;

            // Bounce effect
            self.topple_velocity = -self.topple_velocity * self.data.bounce_velocity_percent;

            // Play bounce FX if configured
            // Matches C++ W3DTreeBuffer.cpp:1887-1897
            if let Some(bounce_fx) = &self.data.bounce_fx {
                if let Some(fx) = TheFXListStore::lookup_fx_list(bounce_fx) {
                    let _ = fx.do_fx_at_position(&self.last_world_position);
                } else {
                    log::warn!("W3DTreeDraw: unresolved bounce FXList '{}'", bounce_fx);
                }
            }

            if self.data.kill_when_toppled {
                self.tree_state = TreeState::Sinking;
                self.animation_frame = 0;
            }
        }
    }

    fn update_sink(&mut self) {
        if self.tree_state != TreeState::Sinking {
            return;
        }

        self.animation_frame += 1;
        let progress = self.animation_frame as Real / self.data.sink_frames as Real;

        if progress >= 1.0 {
            self.sink_amount = self.data.sink_distance;
            // Mark tree for deletion when sink is complete
            // Matches C++ W3DTreeBuffer.cpp:1601-1603
            // In C++, this sets tree.treeType = DELETED_TREE_TYPE
            // In Rust, the object system should handle deletion via kill_when_toppled flag
            // The actual deletion is managed by the parent object/drawable system
        } else {
            self.sink_amount = self.data.sink_distance * progress;
        }
    }

    fn topple_axis(&self) -> glam::Vec3 {
        let lateral = glam::Vec2::new(self.push_direction.x, self.push_direction.y);
        if lateral.length_squared() > 1.0e-6 {
            // Rotate around axis perpendicular to push direction in X/Y plane.
            glam::Vec3::new(-lateral.y, lateral.x, 0.0).normalize()
        } else {
            // Deterministic fallback when no push direction is available.
            glam::Vec3::X
        }
    }

    fn topple_rotation_matrix(&self) -> Option<Matrix3D> {
        if self.current_rotation <= 0.0 {
            return None;
        }
        Some(Matrix3D::from_axis_angle(
            self.topple_axis(),
            self.current_rotation.to_radians(),
        ))
    }
}

impl Module for W3DTreeDraw {
    fn on_drawable_bound_to_object(&mut self) {}
    fn on_delete(&mut self) {}
    fn get_module_name_key(&self) -> NameKeyType {
        self.data.module_tag_name_key
    }
    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.data.module_tag_name_key
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

impl DrawModule for W3DTreeDraw {
    fn do_draw_module(&mut self, transform_mtx: &Matrix3D) {
        // Update animations
        self.update_push_aside();
        self.update_topple();
        self.update_sink();

        // Build transform with offset, rotation, and sink
        let mut final_transform = *transform_mtx;

        // Apply offset (push aside)
        final_transform = final_transform * Matrix3D::from_translation(self.current_offset);

        // Apply rotation (topple)
        // Matches C++ W3DTreeBuffer.cpp:1867-1868
        if let Some(rotation) = self.topple_rotation_matrix() {
            final_transform = final_transform * rotation;
        }

        // Apply sink
        if self.sink_amount > 0.0 {
            let sink_offset = Coord3D::new(0.0, 0.0, -self.sink_amount);
            final_transform = final_transform * Matrix3D::from_translation(sink_offset);
        }
        let (scale, rotation, translation) = final_transform.to_scale_rotation_translation();
        self.last_world_position = translation;

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
        let _ = final_transform;
    }

    fn set_shadows_enabled(&mut self, _enable: bool) {}
    fn release_shadows(&mut self) {}
    fn allocate_shadows(&mut self) {}
    fn set_fully_obscured_by_shroud(&mut self, _fully_obscured: bool) {}
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
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
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
    fn topple_axis_uses_push_direction_perpendicular() {
        let mut draw = W3DTreeDraw::new(W3DTreeDrawModuleData::new());
        draw.push_direction = Coord3D::new(0.0, 1.0, 0.0);
        let axis = draw.topple_axis();
        assert!((axis.x + 1.0).abs() < 1.0e-6);
        assert!(axis.y.abs() < 1.0e-6);
        assert!(axis.z.abs() < 1.0e-6);
    }

    #[test]
    fn topple_axis_falls_back_to_x_axis() {
        let draw = W3DTreeDraw::new(W3DTreeDrawModuleData::new());
        let axis = draw.topple_axis();
        assert!((axis.x - 1.0).abs() < 1.0e-6);
        assert!(axis.y.abs() < 1.0e-6);
        assert!(axis.z.abs() < 1.0e-6);
    }

    #[test]
    fn topple_rotation_matrix_absent_when_not_toppling() {
        let draw = W3DTreeDraw::new(W3DTreeDrawModuleData::new());
        assert!(draw.topple_rotation_matrix().is_none());
    }
}
