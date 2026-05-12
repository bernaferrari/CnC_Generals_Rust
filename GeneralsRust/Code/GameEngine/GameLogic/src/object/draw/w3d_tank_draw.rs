//! W3DTankDraw - Tank drawing with animated treads and turret
//!
//! Port of C++ W3DTankDraw.h/cpp
//! Reference: /GeneralsMD/Code/GameEngineDevice/Include/W3DDevice/GameClient/Module/W3DTankDraw.h
//!
//! Extends W3DModelDraw with:
//! - Animated tank treads with UV scrolling
//! - Tread debris particle effects
//! - Pivot vs drive speed handling
//! - Independent left/right/middle tread support

use super::draw_module::*;
use super::w3d_model_draw::*;
use crate::common::*;
use crate::helpers::{MeshUvOverrideState, TheGameClient, TheGameLogic, TheParticleSystemManager};
use game_engine::common::ini::{INIError, INI};
use game_engine::common::system::{Snapshotable, Xfer, XferVersion};
use game_engine::common::thing::module::{Module, ModuleData, NameKeyType, TimeOfDay};
use std::any::Any;

/// Tread type classification
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TreadType {
    Left,   // Left tread
    Right,  // Right tread
    Middle, // Middle tread (for some vehicles)
}

/// Information about a single tread sub-object
#[derive(Debug, Clone)]
struct TreadObjectInfo {
    /// Type of this tread
    tread_type: TreadType,

    /// Current UV scroll offset
    uv_offset: Real,
}

impl TreadObjectInfo {
    fn new(tread_type: TreadType) -> Self {
        Self {
            tread_type,
            uv_offset: 0.0,
        }
    }
}

/// W3DTankDraw module data
///
/// Reference: W3DTankDrawModuleData in W3DTankDraw.h
#[derive(Debug, Clone)]
pub struct W3DTankDrawModuleData {
    /// Module tag name key
    module_tag_name_key: NameKeyType,

    /// Base model draw data
    pub base: W3DModelDrawModuleData,

    /// Particle system name for left tread debris
    pub tread_debris_name_left: AsciiString,

    /// Particle system name for right tread debris
    pub tread_debris_name_right: AsciiString,

    /// Tread animation rate (texture scroll per second, 1.0 = full width)
    pub tread_animation_rate: Real,

    /// Speed fraction below which pivoting is allowed
    pub tread_pivot_speed_fraction: Real,

    /// Speed fraction below which treads stop animating
    pub tread_drive_speed_fraction: Real,
}

impl W3DTankDrawModuleData {
    pub fn new() -> Self {
        Self {
            module_tag_name_key: 0,
            base: W3DModelDrawModuleData::new(),
            tread_debris_name_left: AsciiString::from("TrackDebrisDirtLeft"),
            tread_debris_name_right: AsciiString::from("TrackDebrisDirtRight"),
            tread_animation_rate: 0.0,
            tread_pivot_speed_fraction: 0.6,
            tread_drive_speed_fraction: 0.3,
        }
    }

    /// Parse module data from an INI block (base W3DModelDraw + tank-specific fields).
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

            if self.parse_ini_field(key.as_str(), &value_tokens)? {
                continue;
            }
            if self
                .base
                .parse_ini_field(ini, key.as_str(), &value_tokens)?
            {
                continue;
            }
            return Err(INIError::UnknownToken);
        }
        Ok(())
    }

    fn parse_ini_field(&mut self, key: &str, tokens: &[&str]) -> Result<bool, INIError> {
        match key.to_ascii_uppercase().as_str() {
            "TREADDEBRISLEFT" => {
                let value = INI::parse_ascii_string(parse_required_value(tokens)?)?;
                self.tread_debris_name_left = AsciiString::from(value.as_str());
                Ok(true)
            }
            "TREADDEBRISRIGHT" => {
                let value = INI::parse_ascii_string(parse_required_value(tokens)?)?;
                self.tread_debris_name_right = AsciiString::from(value.as_str());
                Ok(true)
            }
            "TREADANIMATIONRATE" => {
                self.tread_animation_rate =
                    INI::parse_velocity_real(parse_required_value(tokens)?)?;
                Ok(true)
            }
            "TREADPIVOTSPEEDFRACTION" => {
                self.tread_pivot_speed_fraction = INI::parse_real(parse_required_value(tokens)?)?;
                Ok(true)
            }
            "TREADDRIVESPEEDFRACTION" => {
                self.tread_drive_speed_fraction = INI::parse_real(parse_required_value(tokens)?)?;
                Ok(true)
            }
            _ => Ok(false),
        }
    }
}

fn parse_required_value<'a>(tokens: &'a [&str]) -> Result<&'a str, INIError> {
    tokens
        .iter()
        .copied()
        .find(|token| !token.is_empty())
        .ok_or(INIError::InvalidData)
}

impl Default for W3DTankDrawModuleData {
    fn default() -> Self {
        Self::new()
    }
}

impl ModuleData for W3DTankDrawModuleData {
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

impl DrawModuleData for W3DTankDrawModuleData {
    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl Snapshotable for W3DTankDrawModuleData {
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

/// W3DTankDraw module instance
///
/// Reference: W3DTankDraw in W3DTankDraw.h
pub struct W3DTankDraw {
    /// Module data
    data: W3DTankDrawModuleData,

    /// Base W3DModelDraw functionality
    base: W3DModelDraw,

    /// Tread sub-objects (up to MAX_TREADS_PER_TANK)
    treads: Vec<TreadObjectInfo>,

    /// Symbolic tread offsets used by the render bridge once real TREADS* meshes are present.
    tread_uv_left: Real,
    tread_uv_right: Real,
    tread_uv_middle: Real,

    /// Last direction vector (for calculating rotation)
    last_direction: Coord3D,

    /// Particle system IDs for tread debris
    tread_debris_left: Option<u32>,
    tread_debris_right: Option<u32>,

    /// Whether debris emitters are active
    debris_active: bool,

    /// Current velocity (for tread animation)
    current_velocity: Real,

    /// Maximum velocity (for speed fraction calculations)
    max_velocity: Real,
}

impl W3DTankDraw {
    pub fn new(data: W3DTankDrawModuleData) -> Self {
        let base_data = data.base.clone();
        let base = W3DModelDraw::new(base_data);

        Self {
            data,
            base,
            treads: Vec::new(),
            tread_uv_left: 0.0,
            tread_uv_right: 0.0,
            tread_uv_middle: 0.0,
            last_direction: Coord3D::new(1.0, 0.0, 0.0),
            tread_debris_left: None,
            tread_debris_right: None,
            debris_active: false,
            current_velocity: 0.0,
            max_velocity: 1.0,
        }
    }

    pub fn bind_owner_id(&mut self, owner_id: ObjectID) {
        self.base.bind_owner_id(owner_id);
    }

    /// Create tread debris particle emitters
    fn create_emitters(&mut self) {
        let Some(ps_manager) = TheParticleSystemManager::get() else {
            return;
        };
        let owner_id = self.base.owner_id();

        if self.tread_debris_left.is_none() && !self.data.tread_debris_name_left.is_empty() {
            if let Some(id) =
                ps_manager.create_particle_system(Some(self.data.tread_debris_name_left.as_str()))
            {
                if let Some(owner_id) = owner_id {
                    ps_manager.attach_particle_system_to_drawable(id, owner_id);
                }
                // C++ creates these emitters in stopped state.
                ps_manager.stop_particle_system(id);
                self.tread_debris_left = Some(id);
            }
        }

        if self.tread_debris_right.is_none() && !self.data.tread_debris_name_right.is_empty() {
            if let Some(id) =
                ps_manager.create_particle_system(Some(self.data.tread_debris_name_right.as_str()))
            {
                if let Some(owner_id) = owner_id {
                    ps_manager.attach_particle_system_to_drawable(id, owner_id);
                }
                // C++ creates these emitters in stopped state.
                ps_manager.stop_particle_system(id);
                self.tread_debris_right = Some(id);
            }
        }
    }

    /// Destroy tread debris emitters
    fn toss_emitters(&mut self) {
        let ps_manager = TheParticleSystemManager::get();
        if let Some(id) = self.tread_debris_left {
            if let Some(ps_manager) = ps_manager {
                ps_manager.destroy_particle_system(id);
            }
        }
        if let Some(id) = self.tread_debris_right {
            if let Some(ps_manager) = ps_manager {
                ps_manager.destroy_particle_system(id);
            }
        }
        self.tread_debris_left = None;
        self.tread_debris_right = None;
    }

    /// Start creating move debris from tank treads
    fn start_move_debris(&mut self) {
        if !self.debris_active {
            if !self.base.is_visible() {
                return;
            }
            self.debris_active = true;
            if let Some(ps_manager) = TheParticleSystemManager::get() {
                if let Some(id) = self.tread_debris_left {
                    ps_manager.start_particle_system(id);
                }
                if let Some(id) = self.tread_debris_right {
                    ps_manager.start_particle_system(id);
                }
            }
        }
    }

    /// Stop creating move debris
    fn stop_move_debris(&mut self) {
        if self.debris_active {
            self.debris_active = false;
            if let Some(ps_manager) = TheParticleSystemManager::get() {
                if let Some(id) = self.tread_debris_left {
                    ps_manager.stop_particle_system(id);
                }
                if let Some(id) = self.tread_debris_right {
                    ps_manager.stop_particle_system(id);
                }
            }
        }
    }

    /// Update tread sub-object pointers
    ///
    /// Finds tread sub-objects in the model and caches them for animation.
    fn update_tread_objects(&mut self) {
        self.treads.clear();
        // C++ W3DTankDraw only populates this list from real mesh sub-objects named
        // TREADS* that use a LinearOffsetTextureMapper. Until the render object bridge
        // exposes those sub-objects, keep the list empty rather than animating fake treads.
    }

    /// Update tread UV coordinates for animation
    ///
    /// # Arguments
    /// * `uv_delta` - Amount to scroll UV coordinates (based on speed and time)
    fn update_tread_positions(&mut self, uv_delta: Real) {
        self.tread_uv_left = wrap_uv_offset(self.tread_uv_left + uv_delta);
        self.tread_uv_right = wrap_uv_offset(self.tread_uv_right - uv_delta);
        self.tread_uv_middle = wrap_uv_offset(self.tread_uv_middle + uv_delta);

        for tread in &mut self.treads {
            let offset = match tread.tread_type {
                TreadType::Left => tread.uv_offset + uv_delta,
                TreadType::Right => tread.uv_offset - uv_delta,
                // The C++ path only explicitly handles L/R for pivot mode.
                // Keep middle treads moving in the same direction as left for stability.
                TreadType::Middle => tread.uv_offset + uv_delta,
            };
            tread.uv_offset = wrap_uv_offset(offset);
        }
    }

    fn publish_tread_uv_overrides(&self) {
        let Some(owner_id) = self.base.owner_id() else {
            return;
        };
        let Some(client) = TheGameClient::get() else {
            return;
        };
        let Some(mut state) = client.get_drawable_model_draw(owner_id) else {
            return;
        };

        state.mesh_uv_overrides.push(MeshUvOverrideState {
            mesh_name_prefix: "TREADSL".to_string(),
            u_offset: self.tread_uv_left,
            v_offset: 0.0,
        });
        state.mesh_uv_overrides.push(MeshUvOverrideState {
            mesh_name_prefix: "TREADSR".to_string(),
            u_offset: self.tread_uv_right,
            v_offset: 0.0,
        });
        state.mesh_uv_overrides.push(MeshUvOverrideState {
            mesh_name_prefix: "TREADS".to_string(),
            u_offset: self.tread_uv_middle,
            v_offset: 0.0,
        });

        client.set_drawable_model_draw(owner_id, state);
    }

    /// Update tread animation based on movement
    fn update_tread_animation(
        &mut self,
        velocity: Real,
        max_velocity: Real,
        turning: Real,
        is_motive: bool,
        direction: &Coord3D,
    ) {
        if self.data.tread_animation_rate == 0.0 {
            self.last_direction = *direction;
            return;
        }

        let speed_fraction = if max_velocity > 0.0 {
            velocity / max_velocity
        } else {
            0.0
        };
        let tread_scroll_speed = self.data.tread_animation_rate;

        // C++ parity: when mostly stationary and turning, use left/right differential scrolling.
        if turning != 0.0 && speed_fraction < self.data.tread_pivot_speed_fraction {
            let angle_to_goal =
                direction.x * self.last_direction.x + direction.y * self.last_direction.y;
            if (1.0 - angle_to_goal).abs() > 0.00001 {
                if turning < 0.0 {
                    self.update_tread_positions(-tread_scroll_speed);
                } else {
                    self.update_tread_positions(tread_scroll_speed);
                }
            }
            self.last_direction = *direction;
            return;
        }

        // C++ parity: moving straight at speed uses uniform scroll on all treads.
        if is_motive && speed_fraction >= self.data.tread_drive_speed_fraction {
            self.tread_uv_left = wrap_uv_offset(self.tread_uv_left - tread_scroll_speed);
            self.tread_uv_right = wrap_uv_offset(self.tread_uv_right - tread_scroll_speed);
            self.tread_uv_middle = wrap_uv_offset(self.tread_uv_middle - tread_scroll_speed);
            for tread in &mut self.treads {
                let offset = tread.uv_offset - tread_scroll_speed;
                tread.uv_offset = wrap_uv_offset(offset);
            }
        }

        // Save direction for next frame
        self.last_direction = *direction;
    }
}

impl Module for W3DTankDraw {
    fn on_object_created(&mut self) {
        self.base.on_object_created();
    }

    fn on_drawable_bound_to_object(&mut self) {
        self.base.on_drawable_bound_to_object();
        self.create_emitters();
        self.update_tread_objects();
    }

    fn preload_assets(&mut self, time_of_day: TimeOfDay) {
        self.base.preload_assets(time_of_day);
    }

    fn on_delete(&mut self) {
        self.toss_emitters();
        self.base.on_delete();
    }

    fn get_module_name_key(&self) -> NameKeyType {
        self.base.get_module_name_key()
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.base.get_module_tag_name_key()
    }

    fn get_module_data(&self) -> &dyn ModuleData {
        &self.data
    }
}

impl DrawModule for W3DTankDraw {
    fn do_draw_module(&mut self, transform_mtx: &Matrix3D) {
        let mut direction = Coord3D::new(transform_mtx.x_axis.x, transform_mtx.x_axis.y, 0.0);
        let mut turning = 0.0;
        let mut is_motive = false;

        if let Some(owner_id) = self.base.owner_id() {
            if let Some(owner) = TheGameLogic::find_object_by_id(owner_id) {
                if let Ok(owner_guard) = owner.read() {
                    let (dir_x, dir_y) = owner_guard.get_unit_direction_vector_2d();
                    if dir_x != 0.0 || dir_y != 0.0 {
                        direction = Coord3D::new(dir_x, dir_y, 0.0);
                    }

                    if let Some(physics) = owner_guard.get_physics() {
                        if let Ok(physics_guard) = physics.lock() {
                            let velocity = physics_guard.get_velocity();
                            self.current_velocity =
                                (velocity.x * velocity.x + velocity.y * velocity.y).sqrt();
                            turning = physics_guard.get_turning();
                            is_motive = self.current_velocity > 0.0;
                        }
                    }

                    if let Some(ai) = owner_guard.get_ai_update_interface() {
                        if let Ok(ai_guard) = ai.lock() {
                            let locomotor_speed = ai_guard.get_cur_locomotor_speed();
                            if locomotor_speed > 0.0 {
                                self.max_velocity = locomotor_speed;
                            }
                        }
                    }
                }
            }
        }

        if self.max_velocity <= 0.0 {
            self.max_velocity = 1.0;
        }

        // Update tread animation
        self.update_tread_animation(
            self.current_velocity,
            self.max_velocity,
            turning,
            is_motive,
            &direction,
        );

        const DEBRIS_THRESHOLD: Real = 0.00001;
        let velocity_mag_sq = self.current_velocity * self.current_velocity;
        if velocity_mag_sq > DEBRIS_THRESHOLD && self.base.is_visible() {
            self.start_move_debris();
        } else {
            self.stop_move_debris();
        }

        if let Some(ps_manager) = TheParticleSystemManager::get() {
            let vel_mag = self.current_velocity;
            let x = (0.5 * vel_mag + 0.1).min(1.0);
            let z = (vel_mag + 0.1).min(1.0);
            let vel_mult = Coord3D::new(x, x, z);

            if let Some(id) = self.tread_debris_left {
                ps_manager.set_particle_system_velocity_multiplier(id, &vel_mult);
                ps_manager.set_particle_system_burst_count_multiplier(id, z);
            }
            if let Some(id) = self.tread_debris_right {
                ps_manager.set_particle_system_velocity_multiplier(id, &vel_mult);
                ps_manager.set_particle_system_burst_count_multiplier(id, z);
            }
        }

        // Draw base model (includes turret positioning and recoil)
        self.base.do_draw_module(transform_mtx);
        self.publish_tread_uv_overrides();

        // When render object system is implemented:
        // Reference: C++ W3DTankDraw.cpp - tread rendering
        // - Treads are rendered as part of the base model
        // - UV offsets have already been applied to tread materials
        // - No additional rendering needed here
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

        // Stop debris when hidden
        if fully_obscured {
            self.stop_move_debris();
        }
    }

    fn set_hidden(&mut self, hidden: bool) {
        DrawModule::set_hidden(&mut self.base, hidden);
        if hidden {
            self.stop_move_debris();
        }
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

        // Model changed, re-find tread sub-objects
        self.update_tread_objects();
    }

    fn get_object_draw_interface(&self) -> Option<&dyn ObjectDrawInterface> {
        Some(&self.base as &dyn ObjectDrawInterface)
    }

    fn get_object_draw_interface_mut(&mut self) -> Option<&mut dyn ObjectDrawInterface> {
        Some(&mut self.base as &mut dyn ObjectDrawInterface)
    }
}

impl Snapshotable for W3DTankDraw {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.base.crc(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let current_version: XferVersion = 1;
        let mut version = current_version;
        xfer.xfer_version(&mut version, current_version)
            .map_err(|e| e.to_string())?;

        self.base.xfer(xfer)
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        self.base.load_post_process()?;
        self.toss_emitters();
        self.create_emitters();
        Ok(())
    }
}

/// Maximum number of treads per tank
#[allow(dead_code)]
const MAX_TREADS_PER_TANK: usize = 4;

fn wrap_uv_offset(offset: Real) -> Real {
    offset - offset.floor()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn update_tread_objects_does_not_fabricate_treads_without_render_subobjects() {
        let mut draw = W3DTankDraw::new(W3DTankDrawModuleData {
            tread_animation_rate: 0.25,
            ..W3DTankDrawModuleData::default()
        });
        draw.treads.push(TreadObjectInfo::new(TreadType::Left));

        draw.update_tread_objects();

        assert!(
            draw.treads.is_empty(),
            "C++ only caches discovered W3D tread meshes and does not create fallback treads"
        );
    }

    #[test]
    fn tread_animation_keeps_symbolic_offsets_without_discovered_treads() {
        let mut draw = W3DTankDraw::new(W3DTankDrawModuleData {
            tread_animation_rate: 0.25,
            ..W3DTankDrawModuleData::default()
        });
        let direction = Coord3D::new(0.0, 1.0, 0.0);

        draw.update_tread_animation(3.0, 10.0, 1.0, true, &direction);

        assert!(draw.treads.is_empty());
        assert_ne!(draw.tread_uv_left, 0.0);
        assert_ne!(draw.tread_uv_right, 0.0);
        assert_ne!(draw.tread_uv_middle, 0.0);
        assert_eq!(draw.last_direction, direction);
    }
}
