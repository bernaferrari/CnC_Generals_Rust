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
use game_engine::common::ini::{INI, INIError};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::{Snapshotable, Xfer, XferVersion};
use game_engine::common::thing::module::{Module, ModuleData, NameKeyType, TimeOfDay};
use std::any::Any;
use std::cell::RefCell;
use std::collections::HashMap;

/// C++ `W3DTankDraw.cpp:286` ground-speed gate for TrackDebrisDirt.
const DEBRIS_THRESHOLD: Real = 0.00001;

thread_local! {
    static LIVE_TREAD_DEBRIS: RefCell<HashMap<ObjectID, W3DTankDraw>> =
        RefCell::new(HashMap::new());
}

/// C++ `W3DTankDraw::doDrawModule` debris start/stop, driven by live host pose.
pub fn tick_live_host_tread_debris(
    owner_id: ObjectID,
    position: [f32; 3],
    vel_mag_sq: Real,
    hidden: bool,
    shrouded: bool,
) {
    LIVE_TREAD_DEBRIS.with(|map| {
        let mut map = map.borrow_mut();
        let draw = map.entry(owner_id).or_insert_with(|| {
            let mut draw = W3DTankDraw::new(W3DTankDrawModuleData::new());
            draw.bind_owner_id(owner_id);
            draw
        });
        draw.tick_live_move_debris(
            &Coord3D::new(position[0], position[1], position[2]),
            vel_mag_sq,
            hidden,
            shrouded,
        );
    });
}

/// Toss leftover TrackDebrisDirt emitters when the live drawable is pruned.
pub fn prune_live_host_tread_debris(owner_id: ObjectID) {
    LIVE_TREAD_DEBRIS.with(|map| {
        if let Some(mut draw) = map.borrow_mut().remove(&owner_id) {
            draw.toss_emitters();
        }
    });
}

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

    /// Leaf mesh name after the W3D `Model.TREADS*` dot (C++ `meshName`).
    mesh_name: String,
}

impl TreadObjectInfo {
    fn new(tread_type: TreadType) -> Self {
        Self {
            tread_type,
            uv_offset: 0.0,
            mesh_name: String::new(),
        }
    }

    fn with_mesh(tread_type: TreadType, mesh_name: String) -> Self {
        Self {
            tread_type,
            uv_offset: 0.0,
            mesh_name,
        }
    }
}

fn tread_leaf_name(full_name: &str) -> Option<&str> {
    let leaf = full_name.rsplit_once('.').map(|(_, leaf)| leaf)?;
    if leaf.len() >= 6 && leaf[..6].eq_ignore_ascii_case("TREADS") {
        Some(leaf)
    } else {
        None
    }
}

fn classify_tread_leaf(leaf: &str) -> TreadType {
    match leaf.as_bytes().get(6).map(|b| b.to_ascii_uppercase()) {
        Some(b'L') => TreadType::Left,
        Some(b'R') => TreadType::Right,
        _ => TreadType::Middle,
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
        self.base.set_module_tag_name_key(key);
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

    pub fn owner_id(&self) -> Option<ObjectID> {
        self.base.owner_id()
    }

    pub fn set_animation_loop_duration(&mut self, num_frames: u32) {
        self.base.set_animation_loop_duration(num_frames);
    }

    pub fn set_animation_completion_time(&mut self, num_frames: u32) {
        self.base.set_animation_completion_time(num_frames);
    }

    pub fn set_animation_frame(&mut self, frame: i32) {
        self.base.set_animation_frame(frame);
    }

    pub fn show_sub_object(&mut self, name: &str, show: bool) {
        self.base.show_sub_object(name, show);
    }

    pub fn update_sub_objects(&mut self) {
        self.base.update_sub_objects();
    }

    /// Create tread debris particle emitters
    fn create_emitters(&mut self) {
        let Some(ps_manager) = TheParticleSystemManager::get() else {
            return;
        };
        let owner_id = self.base.owner_id();
        let drawable_attached = owner_id.is_some_and(|id| {
            TheGameClient::get().is_some_and(|client| client.find_drawable_by_id(id).is_some())
        });

        if self.tread_debris_left.is_none() && !self.data.tread_debris_name_left.is_empty() {
            if let Some(id) =
                ps_manager.create_particle_system(Some(self.data.tread_debris_name_left.as_str()))
            {
                if drawable_attached {
                    if let Some(owner_id) = owner_id {
                        ps_manager.attach_particle_system_to_drawable(id, owner_id);
                    }
                }
                // C++ marks these do-not-save and creates them stopped.
                ps_manager.set_particle_system_saveable(id, false);
                ps_manager.stop_particle_system(id);
                self.tread_debris_left = Some(id);
            }
        }

        if self.tread_debris_right.is_none() && !self.data.tread_debris_name_right.is_empty() {
            if let Some(id) =
                ps_manager.create_particle_system(Some(self.data.tread_debris_name_right.as_str()))
            {
                if drawable_attached {
                    if let Some(owner_id) = owner_id {
                        ps_manager.attach_particle_system_to_drawable(id, owner_id);
                    }
                }
                // C++ marks these do-not-save and creates them stopped.
                ps_manager.set_particle_system_saveable(id, false);
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
        self.debris_active = false;
    }

    /// Start creating move debris from tank treads
    fn start_move_debris(&mut self) {
        if self.debris_active {
            return;
        }
        if !self.base.is_visible() {
            return;
        }
        if self.tread_debris_left.is_none() && self.tread_debris_right.is_none() {
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

    fn place_emitters_at(&self, position: &Coord3D) {
        let Some(ps_manager) = TheParticleSystemManager::get() else {
            return;
        };
        if let Some(id) = self.tread_debris_left {
            ps_manager.set_particle_system_position(id, position);
        }
        if let Some(id) = self.tread_debris_right {
            ps_manager.set_particle_system_position(id, position);
        }
    }

    /// C++ `W3DTankDraw.cpp:309-335` start/stop + velocity/burst multipliers.
    fn update_move_debris(&mut self, vel_mag_sq: Real) {
        if vel_mag_sq > DEBRIS_THRESHOLD && self.base.is_visible() {
            self.start_move_debris();
        } else {
            self.stop_move_debris();
        }

        let Some(ps_manager) = TheParticleSystemManager::get() else {
            return;
        };
        let vel_mag = vel_mag_sq.sqrt();
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

    /// Live host pose/velocity — leftover GameLogic physics is dual-world only.
    fn tick_live_move_debris(
        &mut self,
        position: &Coord3D,
        vel_mag_sq: Real,
        hidden: bool,
        shrouded: bool,
    ) {
        DrawModule::set_hidden(self, hidden);
        self.set_fully_obscured_by_shroud(shrouded);
        self.create_emitters();
        self.place_emitters_at(position);
        self.update_move_debris(vel_mag_sq);
    }

    /// Update tread sub-object pointers
    ///
    /// Finds tread sub-objects in the model and caches them for animation.
    fn update_tread_objects(&mut self) {
        if self.data.tread_animation_rate == 0.0 {
            self.treads.clear();
            return;
        }
        let Some(owner_id) = self.base.owner_id() else {
            self.treads.clear();
            return;
        };
        let Some(children) = peek_hlod_live_child_states(owner_id) else {
            return;
        };
        if children.is_empty() {
            return;
        }
        let any_linear_offset = children.iter().any(|child| child.uv_animations_disabled);
        let previous = std::mem::take(&mut self.treads);
        for child in children {
            if self.treads.len() >= MAX_TREADS_PER_TANK {
                break;
            }
            let Some(leaf) = tread_leaf_name(&child.name) else {
                continue;
            };
            if any_linear_offset && !child.uv_animations_disabled {
                continue;
            }
            let uv_offset = previous
                .iter()
                .find(|tread| tread.mesh_name.eq_ignore_ascii_case(leaf))
                .map(|tread| tread.uv_offset)
                .unwrap_or(0.0);
            self.treads.push(TreadObjectInfo {
                tread_type: classify_tread_leaf(leaf),
                uv_offset,
                mesh_name: leaf.to_string(),
            });
        }
    }

    /// Update tread UV coordinates for animation
    ///
    /// # Arguments
    /// * `uv_delta` - Amount to scroll UV coordinates (based on speed and time)
    fn update_tread_positions(&mut self, uv_delta: Real) {
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
        let overrides: Vec<MeshUvOverrideState> = self
            .treads
            .iter()
            .filter(|tread| !tread.mesh_name.is_empty())
            .map(|tread| MeshUvOverrideState {
                mesh_name_prefix: tread.mesh_name.clone(),
                u_offset: tread.uv_offset,
                v_offset: 0.0,
            })
            .collect();
        if overrides.is_empty() {
            return;
        }
        let _ = client.with_active_object_model_draw(owner_id, |state| {
            state.mesh_uv_overrides = overrides;
        });
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

        if self.treads.is_empty() {
            self.last_direction = *direction;
            return;
        }

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
        NameKeyGenerator::name_to_key("W3DTankDraw")
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.data.get_module_tag_name_key()
    }

    fn get_module_data(&self) -> &dyn ModuleData {
        &self.data
    }
}

impl DrawModule for W3DTankDraw {
    fn do_draw_module(&mut self, transform_mtx: &Matrix3D) {
        self.update_tread_objects();
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

        self.update_move_debris(self.current_velocity * self.current_velocity);

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
        if fully_obscured && self.base.fully_obscured_by_shroud() != fully_obscured {
            self.stop_move_debris();
        }
        self.base.set_fully_obscured_by_shroud(fully_obscured);
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
    fn tread_animation_does_not_fabricate_offsets_without_discovered_treads() {
        let mut draw = W3DTankDraw::new(W3DTankDrawModuleData {
            tread_animation_rate: 0.25,
            ..W3DTankDrawModuleData::default()
        });
        let direction = Coord3D::new(0.0, 1.0, 0.0);

        draw.update_tread_animation(3.0, 10.0, 1.0, true, &direction);

        assert!(draw.treads.is_empty());
        assert_eq!(draw.last_direction, direction);
    }

    #[test]
    fn tank_treads_match_cpp_pivot_and_drive_scroll_directions() {
        let mut draw = W3DTankDraw::new(W3DTankDrawModuleData {
            tread_animation_rate: 0.25,
            tread_pivot_speed_fraction: 0.6,
            tread_drive_speed_fraction: 0.3,
            ..W3DTankDrawModuleData::default()
        });
        draw.treads.push(TreadObjectInfo::new(TreadType::Left));
        draw.treads.push(TreadObjectInfo::new(TreadType::Right));
        draw.treads.push(TreadObjectInfo::new(TreadType::Middle));

        draw.update_tread_animation(1.0, 10.0, 1.0, true, &Coord3D::new(0.0, 1.0, 0.0));
        assert_eq!(draw.treads[0].uv_offset, 0.25);
        assert_eq!(draw.treads[1].uv_offset, 0.75);
        assert_eq!(draw.treads[2].uv_offset, 0.25);

        draw.update_tread_animation(8.0, 10.0, 0.0, true, &Coord3D::new(0.0, 1.0, 0.0));
        assert_eq!(draw.treads[0].uv_offset, 0.0);
        assert_eq!(draw.treads[1].uv_offset, 0.5);
        assert_eq!(draw.treads[2].uv_offset, 0.0);
    }

    #[test]
    fn update_tread_objects_discovers_linear_offset_treads() {
        let object_id = 8801;
        publish_hlod_live_child_states(
            object_id,
            vec![
                HlodLiveChildState {
                    name: "AVTank.TREADSL".to_string(),
                    hidden: false,
                    local_transform: Matrix3D::IDENTITY,
                    uv_animations_disabled: true,
                },
                HlodLiveChildState {
                    name: "AVTank.TREADSR".to_string(),
                    hidden: false,
                    local_transform: Matrix3D::IDENTITY,
                    uv_animations_disabled: true,
                },
                HlodLiveChildState {
                    name: "AVTank.Turret".to_string(),
                    hidden: false,
                    local_transform: Matrix3D::IDENTITY,
                    uv_animations_disabled: false,
                },
            ],
        );
        let mut draw = W3DTankDraw::new(W3DTankDrawModuleData {
            tread_animation_rate: 0.25,
            ..W3DTankDrawModuleData::default()
        });
        draw.bind_owner_id(object_id);
        draw.update_tread_objects();
        let _ = take_hlod_live_child_states(object_id);

        assert_eq!(draw.treads.len(), 2);
        assert_eq!(draw.treads[0].tread_type, TreadType::Left);
        assert_eq!(draw.treads[0].mesh_name, "TREADSL");
        assert_eq!(draw.treads[1].tread_type, TreadType::Right);
        assert_eq!(draw.treads[1].mesh_name, "TREADSR");
    }

    #[test]
    fn module_name_key_is_tank_draw() {
        let draw = W3DTankDraw::new(W3DTankDrawModuleData::new());
        assert_eq!(
            draw.get_module_name_key(),
            NameKeyGenerator::name_to_key("W3DTankDraw")
        );
    }

    #[test]
    fn bind_owner_id_forwards_to_base_model_draw() {
        let mut draw = W3DTankDraw::new(W3DTankDrawModuleData::new());
        draw.bind_owner_id(91);
        assert_eq!(draw.owner_id(), Some(91));
    }

    #[test]
    fn module_tag_key_is_shared_with_base_model_data() {
        let mut data = W3DTankDrawModuleData::new();
        data.set_module_tag_name_key(1234);
        assert_eq!(data.get_module_tag_name_key(), 1234);
        assert_eq!(data.base.get_module_tag_name_key(), 1234);
    }
}
