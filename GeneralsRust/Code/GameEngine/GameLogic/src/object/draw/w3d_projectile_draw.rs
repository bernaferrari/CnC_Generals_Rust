//! W3DProjectileDraw - Projectile rendering
//!
//! Port of projectile-specific drawing from W3DModelDraw
//! Handles flying projectiles with rotation and trail effects

use super::draw_module::*;
use super::w3d_model_draw::*;
use crate::common::*;
use crate::helpers::TheParticleSystemManager;
use game_engine::common::ini::{INIError, INI};
use game_engine::common::system::{Snapshotable, Xfer, XferVersion};
use game_engine::common::thing::module::{Module, ModuleData, NameKeyType, TimeOfDay};
use glam::Vec4;
use std::any::Any;

#[derive(Debug, Clone)]
pub struct W3DProjectileDrawModuleData {
    pub base: W3DModelDrawModuleData,

    /// Module tag name key
    pub module_tag_name_key: NameKeyType,

    /// Whether to orient model along velocity vector
    pub orient_to_flight_path: bool,

    /// Trail particle system
    pub trail_particle_system: AsciiString,

    /// Smoke trail interval in frames
    pub trail_interval_frames: u32,
}

impl W3DProjectileDrawModuleData {
    pub fn new() -> Self {
        Self {
            base: W3DModelDrawModuleData::new(),
            module_tag_name_key: 0,
            orient_to_flight_path: true,
            trail_particle_system: AsciiString::new(),
            trail_interval_frames: 2,
        }
    }

    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        loop {
            ini.read_line()?;
            if ini.is_eof() {
                return Err(INIError::EndOfFile);
            }

            let tokens = ini
                .get_line_tokens()
                .into_iter()
                .map(|token| token.to_string())
                .collect::<Vec<_>>();
            let Some(key) = tokens.first().cloned() else {
                continue;
            };
            if key.eq_ignore_ascii_case("End") {
                break;
            }

            let value_tokens = tokens
                .iter()
                .map(String::as_str)
                .skip(1)
                .filter(|token| *token != "=")
                .collect::<Vec<_>>();

            match key.to_ascii_uppercase().as_str() {
                "ORIENTTOFLIGHTPATH" => {
                    self.orient_to_flight_path =
                        INI::parse_bool(parse_required_value(&value_tokens)?)?;
                }
                "TRAILPARTICLESYSTEM" => {
                    let value = INI::parse_ascii_string(parse_required_value(&value_tokens)?)?;
                    self.trail_particle_system = AsciiString::from(value.as_str());
                }
                "TRAILINTERVAL" | "TRAILINTERVALFRAMES" => {
                    self.trail_interval_frames =
                        INI::parse_duration_unsigned_int(parse_required_value(&value_tokens)?)?;
                }
                _ => {
                    if !self
                        .base
                        .parse_ini_field(ini, key.as_str(), &value_tokens)?
                    {
                        return Err(INIError::UnknownToken);
                    }
                }
            }
        }

        Ok(())
    }
}

fn parse_required_value<'a>(tokens: &'a [&str]) -> Result<&'a str, INIError> {
    tokens
        .iter()
        .copied()
        .find(|token| !token.is_empty())
        .ok_or(INIError::InvalidData)
}

impl Default for W3DProjectileDrawModuleData {
    fn default() -> Self {
        Self::new()
    }
}

impl ModuleData for W3DProjectileDrawModuleData {
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

impl DrawModuleData for W3DProjectileDrawModuleData {
    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl Snapshotable for W3DProjectileDrawModuleData {
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

pub struct W3DProjectileDraw {
    data: W3DProjectileDrawModuleData,
    base: W3DModelDraw,
    current_velocity: Coord3D,
    trail_spawn_counter: u32,
}

impl W3DProjectileDraw {
    pub fn new(data: W3DProjectileDrawModuleData) -> Self {
        let base_data = data.base.clone();
        let base = W3DModelDraw::new(base_data);

        Self {
            data,
            base,
            current_velocity: Coord3D::origin(),
            trail_spawn_counter: 0,
        }
    }

    pub fn bind_owner_id(&mut self, owner_id: ObjectID) {
        self.base.bind_owner_id(owner_id);
    }

    fn update_orientation(&mut self, transform: &mut Matrix3D) {
        if !self.data.orient_to_flight_path {
            return;
        }

        if self.current_velocity.length() < 0.01 {
            return;
        }

        // Orient model to face velocity direction
        let forward = self.current_velocity.normalize();
        let mut up = Coord3D::new(0.0, 0.0, 1.0);
        let mut right = forward.cross(up);
        if right.length() < 0.001 {
            up = Coord3D::new(0.0, 1.0, 0.0);
            right = forward.cross(up);
        }
        let right = right.normalize();
        let corrected_up = right.cross(forward);

        // Build rotation matrix from basis vectors
        let translation = transform.w_axis;
        *transform = Matrix3D::from_cols(
            Vec4::new(right.x, right.y, right.z, 0.0),
            Vec4::new(corrected_up.x, corrected_up.y, corrected_up.z, 0.0),
            Vec4::new(forward.x, forward.y, forward.z, 0.0),
            Vec4::new(translation.x, translation.y, translation.z, 1.0),
        );
    }

    fn spawn_trail_particle(&mut self, transform: &Matrix3D) {
        if self.data.trail_particle_system.is_empty() {
            return;
        }

        let Some(ps_manager) = TheParticleSystemManager::get() else {
            return;
        };

        let Some(system_id) =
            ps_manager.create_particle_system(Some(self.data.trail_particle_system.as_str()))
        else {
            return;
        };

        let translation = transform.w_axis;
        let position = Coord3D::new(translation.x, translation.y, translation.z);
        ps_manager.set_particle_system_position(system_id, &position);
    }
}

impl Module for W3DProjectileDraw {
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

impl DrawModule for W3DProjectileDraw {
    fn do_draw_module(&mut self, transform_mtx: &Matrix3D) {
        let mut adjusted_transform = *transform_mtx;

        // Orient to flight path
        self.update_orientation(&mut adjusted_transform);

        // Spawn trail particles periodically
        self.trail_spawn_counter += 1;
        if self.trail_spawn_counter >= self.data.trail_interval_frames {
            self.trail_spawn_counter = 0;
            self.spawn_trail_particle(&adjusted_transform);
        }

        // Draw model
        self.base.do_draw_module(&adjusted_transform);
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
        Some(&self.base as &dyn ObjectDrawInterface)
    }

    fn get_object_draw_interface_mut(&mut self) -> Option<&mut dyn ObjectDrawInterface> {
        Some(&mut self.base as &mut dyn ObjectDrawInterface)
    }
}

impl Snapshotable for W3DProjectileDraw {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.base.crc(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        const CURRENT_VERSION: XferVersion = 1;
        let mut version = CURRENT_VERSION;
        xfer.xfer_version(&mut version, CURRENT_VERSION)
            .map_err(|e| e.to_string())?;
        self.base.xfer(xfer)?;
        xfer.xfer_real(&mut self.current_velocity.x)
            .map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut self.current_velocity.y)
            .map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut self.current_velocity.z)
            .map_err(|e| e.to_string())?;
        xfer.xfer_unsigned_int(&mut self.trail_spawn_counter)
            .map_err(|e| e.to_string())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        self.base.load_post_process()
    }
}
