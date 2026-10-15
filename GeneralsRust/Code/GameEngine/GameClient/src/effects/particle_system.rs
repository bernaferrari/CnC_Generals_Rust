//! # Particle System Implementation
//!
//! Complete particle system implementation matching C++ behavior exactly.
//! Handles particle creation, physics simulation, and lifecycle management.

use game_engine::common::random_value::get_game_client_random_value_real;
use glam::{Mat3, Vec3};
use std::collections::VecDeque;
use std::sync::Arc;

use super::particle_manager::*;
use crate::core::DrawableId;
use game_engine::common::system::{Snapshotable, Xfer, XferMode, XferVersion};

/// Individual particle information (matches C++ ParticleInfo)
#[derive(Debug, Clone)]
pub struct ParticleInfo {
    pub velocity: Vec3,
    pub position: Vec3,
    pub emitter_position: Vec3,
    pub vel_damping: f32,

    pub angle_z: f32,
    pub angular_rate_z: f32,
    pub angular_damping: f32,

    pub lifetime: u32,

    pub size: f32,
    pub size_rate: f32,
    pub size_rate_damping: f32,

    pub alpha_keys: [Keyframe; MAX_KEYFRAMES],
    pub color_keys: [RGBColorKeyframe; MAX_KEYFRAMES],

    pub color_scale: f32,
    pub wind_randomness: f32,
    pub particle_up_towards_emitter: bool,
}

impl Default for ParticleInfo {
    fn default() -> Self {
        Self {
            velocity: Vec3::ZERO,
            position: Vec3::ZERO,
            emitter_position: Vec3::ZERO,
            vel_damping: 1.0,

            angle_z: 0.0,
            angular_rate_z: 0.0,
            angular_damping: 1.0,

            lifetime: 30,

            size: 1.0,
            size_rate: 0.0,
            size_rate_damping: 1.0,

            alpha_keys: [Keyframe::default(); MAX_KEYFRAMES],
            color_keys: [RGBColorKeyframe::default(); MAX_KEYFRAMES],

            color_scale: 1.0,
            wind_randomness: 1.0,
            particle_up_towards_emitter: false,
        }
    }
}

/// Individual particle (matches C++ Particle)
#[derive(Debug)]
pub struct Particle {
    // Receiver-local admission metadata; never part of the native payload.
    pub(super) admission_ordinal: Option<u64>,
    // Basic properties from ParticleInfo
    pub velocity: Vec3,
    pub position: Vec3,
    pub emitter_position: Vec3,
    pub vel_damping: f32,

    pub angle_z: f32,
    pub angular_rate_z: f32,
    pub angular_damping: f32,

    pub lifetime: u32,

    pub size: f32,
    pub size_rate: f32,
    pub size_rate_damping: f32,

    pub alpha_keys: [Keyframe; MAX_KEYFRAMES],
    pub color_keys: [RGBColorKeyframe; MAX_KEYFRAMES],

    pub color_scale: f32,
    pub wind_randomness: f32,
    pub particle_up_towards_emitter: bool,

    // Runtime state
    pub acceleration: Vec3,
    pub last_position: Vec3,
    pub lifetime_left: u32,
    pub create_timestamp: u32,

    pub alpha: f32,
    pub alpha_rate: f32,
    pub alpha_target_key: usize,

    pub color: [f32; 3],
    pub color_rate: [f32; 3],
    pub color_target_key: usize,

    pub is_culled: bool,
    pub personality: u32,

    // System linkage
    pub controlled_system: Option<ParticleSystemId>,

    // List linkage (for efficient memory management)
    pub system_next: Option<usize>,
    pub system_prev: Option<usize>,
    pub overall_next: Option<usize>,
    pub overall_prev: Option<usize>,

    pub in_system_list: bool,
    pub in_overall_list: bool,
}

impl Particle {
    /// Create a new particle from particle info
    pub fn new(info: &ParticleInfo, personality: u32, create_timestamp: u32) -> Self {
        let mut particle = Self {
            // Copy from info
            admission_ordinal: None,
            velocity: info.velocity,
            position: info.position,
            emitter_position: info.emitter_position,
            vel_damping: info.vel_damping,

            angle_z: info.angle_z,
            angular_rate_z: info.angular_rate_z,
            angular_damping: info.angular_damping,

            lifetime: info.lifetime,

            size: info.size,
            size_rate: info.size_rate,
            size_rate_damping: info.size_rate_damping,

            alpha_keys: info.alpha_keys,
            color_keys: info.color_keys,

            color_scale: info.color_scale,
            wind_randomness: info.wind_randomness,
            particle_up_towards_emitter: info.particle_up_towards_emitter,

            // Initialize runtime state
            acceleration: Vec3::ZERO,
            last_position: info.position,
            lifetime_left: info.lifetime,
            create_timestamp,

            alpha: info.alpha_keys[0].value,
            alpha_rate: 0.0,
            alpha_target_key: 1,

            color: info.color_keys[0].color,
            color_rate: [0.0, 0.0, 0.0],
            color_target_key: 1,

            is_culled: false,
            personality,

            controlled_system: None,

            system_next: None,
            system_prev: None,
            overall_next: None,
            overall_prev: None,

            in_system_list: false,
            in_overall_list: false,
        };

        // C++ Particle::Particle always seeds from key 0 (frame 0 is the start pose).
        particle.compute_alpha_rate();
        particle.compute_color_rate();

        particle
    }

    /// Update particle physics and animation (matches C++ Particle::update)
    ///
    /// # Arguments
    /// * `drift_velocity` - System-level drift velocity to apply to position (C++: m_system->getDriftVelocity())
    /// * `current_frame` - Current game frame for keyframe timing (C++: TheGameClient->getFrame())
    /// * `shader_type` - Shader type for alpha handling (C++: m_system->getShaderType())
    pub fn update(
        &mut self,
        drift_velocity: Vec3,
        current_frame: u32,
        shader_type: ParticleShaderType,
    ) -> bool {
        // C++ ParticleSys.cpp:453 — `if (m_lifetimeLeft && --m_lifetimeLeft == 0)`.
        // Lifetime 0 is infinite and is never decremented.

        // Interpolation history remains on the same particle owner.
        self.last_position = self.position;
        generals_particles::integrate_translation(
            &mut self.position,
            &mut self.velocity,
            self.acceleration,
            self.vel_damping,
            drift_velocity,
        );

        // Update rotation (C++ lines 336-337)
        self.angle_z += self.angular_rate_z;
        self.angular_rate_z *= self.angular_damping;

        if self.particle_up_towards_emitter {
            let emitter_dir_x = self.position.x - self.emitter_position.x;
            let emitter_dir_y = self.position.y - self.emitter_position.y;
            let emitter_len =
                (emitter_dir_x * emitter_dir_x + emitter_dir_y * emitter_dir_y).sqrt();

            // ParticleSys.cpp:3390-3420: preserve angleBetween's exact
            // orthogonality branch, signed acos, and zero-vector result.
            let angle_to_emitter = if emitter_len == 0.0 {
                0.0
            } else if emitter_dir_y == 0.0 {
                if emitter_dir_x > 0.0 {
                    std::f32::consts::PI
                } else {
                    0.0
                }
            } else {
                let theta = (emitter_dir_y / emitter_len).acos();
                if emitter_dir_x > 0.0 { theta } else { -theta }
            };
            self.angle_z = angle_to_emitter + std::f32::consts::PI;
        }

        // Update size (C++ lines 351-353)
        self.size += self.size_rate;
        self.size_rate *= self.size_rate_damping;

        // Update alpha - only for non-additive shaders (C++ lines 358-397)
        if shader_type != ParticleShaderType::Additive {
            self.alpha += self.alpha_rate;

            // Check keyframe timing using create_timestamp (C++ lines 385-391)
            let elapsed_frames = current_frame.saturating_sub(self.create_timestamp);

            if self.alpha_target_key < MAX_KEYFRAMES
                && self.alpha_keys[self.alpha_target_key].frame > 0
            {
                if elapsed_frames >= self.alpha_keys[self.alpha_target_key].frame {
                    self.alpha = self.alpha_keys[self.alpha_target_key].value;
                    self.alpha_target_key += 1;
                    self.compute_alpha_rate();
                }
            } else {
                self.alpha_rate = 0.0;
            }

            // Clamp alpha to [0,1] (C++ lines 395-397)
            self.alpha = self.alpha.clamp(0.0, 1.0);
        }

        // Update color (C++ lines 402-423)
        self.color[0] += self.color_rate[0];
        self.color[1] += self.color_rate[1];
        self.color[2] += self.color_rate[2];

        // Check color keyframe timing (C++ lines 410-418)
        let elapsed_frames = current_frame.saturating_sub(self.create_timestamp);

        if self.color_target_key < MAX_KEYFRAMES && self.color_keys[self.color_target_key].frame > 0
        {
            if elapsed_frames >= self.color_keys[self.color_target_key].frame {
                self.color_target_key += 1;
                self.compute_color_rate();
            }
        } else {
            self.color_rate = [0.0, 0.0, 0.0];
        }

        // Apply color scale (C++ lines 426-428)
        self.color[0] += self.color_scale;
        self.color[1] += self.color_scale;
        self.color[2] += self.color_scale;

        // ParticleSys.cpp:430-444: the green lower-bound test reads RED
        // after red's clamp; its upper-bound test reads GREEN. Negative
        // green therefore survives for ordinary finite colors.
        if self.color[0] < 0.0 {
            self.color[0] = 0.0;
        } else if self.color[0] > 1.0 {
            self.color[0] = 1.0;
        }
        if self.color[0] < 0.0 {
            self.color[1] = 0.0;
        } else if self.color[1] > 1.0 {
            self.color[1] = 1.0;
        }
        if self.color[2] < 0.0 {
            self.color[2] = 0.0;
        } else if self.color[2] > 1.0 {
            self.color[2] = 1.0;
        }

        // Clear acceleration for next frame (C++ line 447)
        self.acceleration = Vec3::ZERO;

        // Monitor lifetime (C++ ParticleSys.cpp:453-456)
        if self.lifetime_left > 0 {
            self.lifetime_left -= 1;
            if self.lifetime_left == 0 {
                return false;
            }
        }

        // Check if invisible (C++ lines 454-455)
        if self.is_invisible(shader_type) {
            return false;
        }

        true
    }

    /// Still in the live list: finite remaining life, or authored Lifetime=0 (forever).
    pub fn is_lifetime_active(&self) -> bool {
        self.lifetime == 0 || self.lifetime_left > 0
    }

    /// Visible for draw/cull: not box-culled and not a finished finite life.
    pub fn is_draw_alive(&self) -> bool {
        !self.is_culled && self.is_lifetime_active()
    }

    /// Apply force to particle (matches C++ Particle::applyForce)
    pub fn apply_force(&mut self, force: Vec3) {
        self.acceleration += force;
    }

    /// Do wind motion (matches C++ Particle::doWindMotion)
    ///
    /// Wind force is applied directly to position, not as acceleration.
    /// The force strength diminishes with distance from the emitter.
    ///
    /// # Arguments
    /// * `wind_angle` - Current wind angle from the particle system
    /// * `system_pos` - Position of the particle system (emitter)
    pub fn do_wind_motion(&mut self, wind_angle: f32, system_pos: Vec3) {
        generals_particles::apply_wind_motion(
            &mut self.position,
            wind_angle,
            system_pos,
            self.wind_randomness,
        );
    }

    /// Check if particle is invisible (matches C++ Particle::isInvisible)
    ///
    /// Invisibility depends on shader type:
    /// - Additive: Black (sum of RGB <= 0.06) is invisible
    /// - Alpha: Near-zero alpha (< 0.02) is invisible
    /// - AlphaTest: Never invisible (assumes visible)
    /// - Multiply: Near-white (product of RGB > 0.95) is invisible
    pub fn is_invisible(&self, shader_type: ParticleShaderType) -> bool {
        match shader_type {
            ParticleShaderType::Invalid => true,
            ParticleShaderType::Additive => {
                // If color is black, particle is invisible for additive blending (C++ lines 468-476)
                // Check that we're not transitioning to another color
                // particle_key_bounds_safety: C++ reads beyond its key
                // array once exhausted; retain safe, non-invisible behavior.
                if self.color_target_key < MAX_KEYFRAMES
                    && self.color_keys[self.color_target_key].frame == 0
                {
                    (self.color[0] + self.color[1] + self.color[2]) <= 0.06
                } else {
                    false
                }
            }
            ParticleShaderType::Alpha => {
                // If alpha is near zero, particle is invisible (C++ lines 479-481)
                self.alpha < 0.02
            }
            ParticleShaderType::AlphaTest => {
                // These particles are never invisible (C++ lines 484-485)
                false
            }
            ParticleShaderType::Multiply => {
                // If color is white, particle is invisible for multiply (C++ lines 488-496)
                // Check that we're not transitioning to another color
                // particle_key_bounds_safety: C++ reads beyond its key
                // array once exhausted; retain safe, non-invisible behavior.
                if self.color_target_key < MAX_KEYFRAMES
                    && self.color_keys[self.color_target_key].frame == 0
                {
                    (self.color[0] * self.color[1] * self.color[2]) > 0.95
                } else {
                    false
                }
            }
        }
    }

    /// Compute alpha rate to reach next keyframe (matches C++ Particle::computeAlphaRate)
    fn compute_alpha_rate(&mut self) {
        if self.alpha_target_key >= MAX_KEYFRAMES
            || self.alpha_keys[self.alpha_target_key].frame == 0
        {
            self.alpha_rate = 0.0;
            return;
        }

        if self.alpha_target_key == 0 {
            self.alpha_rate = 0.0;
            return;
        }

        let delta = self.alpha_keys[self.alpha_target_key].value
            - self.alpha_keys[self.alpha_target_key - 1].value;
        let time = self.alpha_keys[self.alpha_target_key].frame
            - self.alpha_keys[self.alpha_target_key - 1].frame;

        self.alpha_rate = if time > 0 { delta / time as f32 } else { 0.0 };
    }

    /// Compute color rate to reach next keyframe (matches C++ Particle::computeColorRate)
    fn compute_color_rate(&mut self) {
        if self.color_target_key >= MAX_KEYFRAMES
            || self.color_keys[self.color_target_key].frame == 0
        {
            self.color_rate = [0.0, 0.0, 0.0];
            return;
        }

        if self.color_target_key == 0 {
            self.color_rate = [0.0, 0.0, 0.0];
            return;
        }

        let time = self.color_keys[self.color_target_key].frame
            - self.color_keys[self.color_target_key - 1].frame;

        if time > 0 {
            let time_f = time as f32;
            for i in 0..3 {
                let delta = self.color_keys[self.color_target_key].color[i]
                    - self.color_keys[self.color_target_key - 1].color[i];
                self.color_rate[i] = delta / time_f;
            }
        } else {
            self.color_rate = [0.0, 0.0, 0.0];
        }
    }
}

impl Snapshotable for ParticleInfo {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 0;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        const CURRENT_VERSION: XferVersion = 1;
        let mut version = CURRENT_VERSION;
        xfer.xfer_version(&mut version, CURRENT_VERSION)
            .map_err(|e| e.to_string())?;

        xfer_vec3(xfer, &mut self.velocity)?;
        xfer_point3(xfer, &mut self.position)?;
        xfer_point3(xfer, &mut self.emitter_position)?;
        xfer.xfer_real(&mut self.vel_damping)
            .map_err(|e| e.to_string())?;

        let mut temp_angle = 0.0f32;
        xfer.xfer_real(&mut temp_angle).map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut temp_angle).map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut self.angle_z)
            .map_err(|e| e.to_string())?;

        xfer.xfer_real(&mut temp_angle).map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut temp_angle).map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut self.angular_rate_z)
            .map_err(|e| e.to_string())?;

        xfer.xfer_unsigned_int(&mut self.lifetime)
            .map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut self.size).map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut self.size_rate)
            .map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut self.size_rate_damping)
            .map_err(|e| e.to_string())?;

        for key in &mut self.alpha_keys {
            xfer.xfer_real(&mut key.value).map_err(|e| e.to_string())?;
            xfer.xfer_unsigned_int(&mut key.frame)
                .map_err(|e| e.to_string())?;
        }

        for key in &mut self.color_keys {
            for component in &mut key.color {
                xfer.xfer_real(component).map_err(|e| e.to_string())?;
            }
            xfer.xfer_unsigned_int(&mut key.frame)
                .map_err(|e| e.to_string())?;
        }

        xfer.xfer_real(&mut self.color_scale)
            .map_err(|e| e.to_string())?;
        xfer.xfer_bool(&mut self.particle_up_towards_emitter)
            .map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut self.wind_randomness)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

impl Snapshotable for ParticleSystemInfo {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 0;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        const CURRENT_VERSION: XferVersion = 1;
        let mut version = CURRENT_VERSION;
        xfer.xfer_version(&mut version, CURRENT_VERSION)
            .map_err(|e| e.to_string())?;

        xfer.xfer_bool(&mut self.is_one_shot)
            .map_err(|e| e.to_string())?;
        xfer_particle_shader_type(xfer, &mut self.shader_type)?;
        xfer_particle_type(xfer, &mut self.particle_type)?;
        xfer.xfer_ascii_string(&mut self.particle_type_name)
            .map_err(|e| e.to_string())?;

        let mut temp_random = GameClientRandomVariable::default();
        xfer_random_variable(xfer, &mut temp_random)?;
        xfer_random_variable(xfer, &mut temp_random)?;
        xfer_random_variable(xfer, &mut self.angle_z)?;

        xfer_random_variable(xfer, &mut temp_random)?;
        xfer_random_variable(xfer, &mut temp_random)?;
        xfer_random_variable(xfer, &mut self.angular_rate_z)?;

        xfer_random_variable(xfer, &mut self.angular_damping)?;
        xfer_random_variable(xfer, &mut self.vel_damping)?;
        xfer_random_variable(xfer, &mut self.lifetime)?;
        xfer.xfer_unsigned_int(&mut self.system_lifetime)
            .map_err(|e| e.to_string())?;
        xfer_random_variable(xfer, &mut self.start_size)?;
        xfer_random_variable(xfer, &mut self.start_size_rate)?;
        xfer_random_variable(xfer, &mut self.size_rate)?;
        xfer_random_variable(xfer, &mut self.size_rate_damping)?;

        for key in &mut self.alpha_keys {
            let mut var = GameClientRandomVariable {
                min: key.min_value,
                max: key.max_value,
                distribution_type: key.distribution_type,
            };
            xfer_random_variable(xfer, &mut var)?;
            key.min_value = var.min;
            key.max_value = var.max;
            key.distribution_type = var.distribution_type;
            xfer.xfer_unsigned_int(&mut key.frame)
                .map_err(|e| e.to_string())?;
        }

        for key in &mut self.color_keys {
            for component in &mut key.color {
                xfer.xfer_real(component).map_err(|e| e.to_string())?;
            }
            xfer.xfer_unsigned_int(&mut key.frame)
                .map_err(|e| e.to_string())?;
        }

        xfer_random_variable(xfer, &mut self.color_scale)?;
        xfer_random_variable(xfer, &mut self.burst_delay)?;
        xfer_random_variable(xfer, &mut self.burst_count)?;
        xfer_random_variable(xfer, &mut self.initial_delay)?;
        xfer_vec3(xfer, &mut self.drift_velocity)?;
        xfer.xfer_real(&mut self.gravity)
            .map_err(|e| e.to_string())?;
        xfer.xfer_ascii_string(&mut self.slave_system_name)
            .map_err(|e| e.to_string())?;
        xfer_vec3(xfer, &mut self.slave_pos_offset)?;
        xfer.xfer_ascii_string(&mut self.attached_system_name)
            .map_err(|e| e.to_string())?;
        xfer_emission_velocity_type(xfer, &mut self.emission_velocity_type)?;
        xfer_particle_priority_type(xfer, &mut self.priority)?;
        xfer_emission_velocity(
            xfer,
            self.emission_velocity_type,
            &mut self.emission_velocity,
        )?;
        xfer_emission_volume_type(xfer, &mut self.emission_volume_type)?;
        xfer_emission_volume(xfer, self.emission_volume_type, &mut self.emission_volume)?;
        xfer.xfer_bool(&mut self.is_emission_volume_hollow)
            .map_err(|e| e.to_string())?;
        xfer.xfer_bool(&mut self.is_ground_aligned)
            .map_err(|e| e.to_string())?;
        xfer.xfer_bool(&mut self.is_emit_above_ground_only)
            .map_err(|e| e.to_string())?;
        xfer.xfer_bool(&mut self.is_particle_up_towards_emitter)
            .map_err(|e| e.to_string())?;
        xfer_wind_motion(xfer, &mut self.wind_motion)?;
        xfer.xfer_real(&mut self.wind_angle)
            .map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut self.wind_angle_change)
            .map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut self.wind_angle_change_min)
            .map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut self.wind_angle_change_max)
            .map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut self.wind_motion_start_angle)
            .map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut self.wind_motion_start_angle_min)
            .map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut self.wind_motion_start_angle_max)
            .map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut self.wind_motion_end_angle)
            .map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut self.wind_motion_end_angle_min)
            .map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut self.wind_motion_end_angle_max)
            .map_err(|e| e.to_string())?;
        let mut moving_to_end = i8::from(self.wind_motion_moving_to_end_angle);
        xfer.xfer_byte(&mut moving_to_end)
            .map_err(|e| e.to_string())?;
        self.wind_motion_moving_to_end_angle = moving_to_end != 0;

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

impl Snapshotable for Particle {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 0;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        const CURRENT_VERSION: XferVersion = 1;
        let mut version = CURRENT_VERSION;
        xfer.xfer_version(&mut version, CURRENT_VERSION)
            .map_err(|e| e.to_string())?;

        let mut particle_info = ParticleInfo {
            velocity: self.velocity,
            position: self.position,
            emitter_position: self.emitter_position,
            vel_damping: self.vel_damping,
            angle_z: self.angle_z,
            angular_rate_z: self.angular_rate_z,
            angular_damping: self.angular_damping,
            lifetime: self.lifetime,
            size: self.size,
            size_rate: self.size_rate,
            size_rate_damping: self.size_rate_damping,
            alpha_keys: self.alpha_keys,
            color_keys: self.color_keys,
            color_scale: self.color_scale,
            wind_randomness: self.wind_randomness,
            particle_up_towards_emitter: self.particle_up_towards_emitter,
        };
        particle_info.xfer(xfer)?;

        if xfer.get_xfer_mode() == XferMode::Load {
            self.velocity = particle_info.velocity;
            self.position = particle_info.position;
            self.emitter_position = particle_info.emitter_position;
            self.vel_damping = particle_info.vel_damping;
            self.angle_z = particle_info.angle_z;
            self.angular_rate_z = particle_info.angular_rate_z;
            self.lifetime = particle_info.lifetime;
            self.size = particle_info.size;
            self.size_rate = particle_info.size_rate;
            self.size_rate_damping = particle_info.size_rate_damping;
            self.alpha_keys = particle_info.alpha_keys;
            self.color_keys = particle_info.color_keys;
            self.color_scale = particle_info.color_scale;
            self.wind_randomness = particle_info.wind_randomness;
            self.particle_up_towards_emitter = particle_info.particle_up_towards_emitter;
        }

        xfer.xfer_unsigned_int(&mut self.personality)
            .map_err(|e| e.to_string())?;
        xfer_vec3(xfer, &mut self.acceleration)?;
        xfer_point3(xfer, &mut self.last_position)?;
        xfer.xfer_unsigned_int(&mut self.lifetime_left)
            .map_err(|e| e.to_string())?;
        xfer.xfer_unsigned_int(&mut self.create_timestamp)
            .map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut self.alpha).map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut self.alpha_rate)
            .map_err(|e| e.to_string())?;

        let mut alpha_target_key = self.alpha_target_key as i32;
        xfer.xfer_int(&mut alpha_target_key)
            .map_err(|e| e.to_string())?;
        if alpha_target_key < 0 || alpha_target_key as usize > MAX_KEYFRAMES {
            return Err("invalid particle alpha target key".into());
        }
        self.alpha_target_key = alpha_target_key as usize;

        for component in &mut self.color {
            xfer.xfer_real(component).map_err(|e| e.to_string())?;
        }
        for component in &mut self.color_rate {
            xfer.xfer_real(component).map_err(|e| e.to_string())?;
        }

        let mut color_target_key = self.color_target_key as i32;
        xfer.xfer_int(&mut color_target_key)
            .map_err(|e| e.to_string())?;
        if color_target_key < 0 || color_target_key as usize > MAX_KEYFRAMES {
            return Err("invalid particle color target key".into());
        }
        self.color_target_key = color_target_key as usize;

        let mut drawable_id = 0u32;
        xfer.xfer_drawable_id(&mut drawable_id)
            .map_err(|e| e.to_string())?;

        let mut controlled_system = self.controlled_system.unwrap_or(INVALID_PARTICLE_SYSTEM_ID);
        xfer.xfer_unsigned_int(&mut controlled_system)
            .map_err(|e| e.to_string())?;
        self.controlled_system = if controlled_system == INVALID_PARTICLE_SYSTEM_ID {
            None
        } else {
            Some(controlled_system)
        };

        if xfer.get_xfer_mode() == XferMode::Load {
            self.system_next = None;
            self.system_prev = None;
            self.overall_next = None;
            self.overall_prev = None;
            self.in_system_list = false;
            self.in_overall_list = false;
        }

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        // MAX_KEYFRAMES is a legitimate exhausted cursor: update stops
        // interpolation there. Never rewind it to the last authored key.
        if self.alpha_target_key > MAX_KEYFRAMES || self.color_target_key > MAX_KEYFRAMES {
            return Err("invalid particle target key".into());
        }
        Ok(())
    }
}

impl Snapshotable for ParticleSystem {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 0;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.xfer_with_particle_random(xfer, &mut random::AmbientParticleRandom)
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        for particle in &mut self.particles {
            particle.load_post_process()?;
        }

        Ok(())
    }
}

/// Combine freshly generated owned observations for the ordinary slave path.
/// The manager generates master then slave under separate mutable borrows;
/// C++ mergeRelatedParticleSystems(..., false) supplies these exact fields.
pub(crate) fn merge_related_particle_infos(
    mut merge_info: ParticleInfo,
    slave_info: ParticleInfo,
    offset: Vec3,
) -> ParticleInfo {
    // Override unique attributes of slave particle (C++ lines 2292-2309)
    merge_info.lifetime = slave_info.lifetime;

    // Size becomes a scale factor of master's particles (C++ lines 2294-2297)
    merge_info.size *= slave_info.size;
    merge_info.size_rate *= slave_info.size_rate;
    merge_info.size_rate_damping *= slave_info.size_rate_damping;

    merge_info.angle_z = slave_info.angle_z;
    merge_info.angular_rate_z = slave_info.angular_rate_z;
    merge_info.angular_damping = slave_info.angular_damping;

    // Copy alpha and color keys from slave (C++ lines 2303-2308)
    merge_info.alpha_keys = slave_info.alpha_keys;
    merge_info.color_keys = slave_info.color_keys;

    merge_info.color_scale = slave_info.color_scale;

    // Offset slave's position relative to master's (C++ lines 2311-2315)
    merge_info.position.x += offset.x;
    merge_info.position.y += offset.y;
    merge_info.position.z += offset.z;

    merge_info
}

fn xfer_vec3(xfer: &mut dyn Xfer, value: &mut Vec3) -> Result<(), String> {
    xfer.xfer_real(&mut value.x).map_err(|e| e.to_string())?;
    xfer.xfer_real(&mut value.y).map_err(|e| e.to_string())?;
    xfer.xfer_real(&mut value.z).map_err(|e| e.to_string())?;
    Ok(())
}

fn xfer_point3(xfer: &mut dyn Xfer, value: &mut Vec3) -> Result<(), String> {
    xfer.xfer_real(&mut value.x).map_err(|e| e.to_string())?;
    xfer.xfer_real(&mut value.y).map_err(|e| e.to_string())?;
    xfer.xfer_real(&mut value.z).map_err(|e| e.to_string())?;
    Ok(())
}

fn xfer_matrix3(
    xfer: &mut dyn Xfer,
    value: &mut Mat3,
    translation: &mut Vec3,
) -> Result<(), String> {
    for row in 0..3 {
        for col in 0..3 {
            xfer.xfer_real(&mut value.col_mut(col)[row])
                .map_err(|e| e.to_string())?;
        }
        xfer.xfer_real(&mut translation[row])
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Extract rotation + translation from a glam/SAGE 4x4 (column-major).
fn affine_from_glam_cols(cols: [f32; 16]) -> (Mat3, Vec3) {
    let rot = Mat3::from_cols_array(&[
        cols[0], cols[1], cols[2], cols[4], cols[5], cols[6], cols[8], cols[9], cols[10],
    ]);
    let trans = Vec3::new(cols[12], cols[13], cols[14]);
    (rot, trans)
}

/// C++ drawable attach uses `getFullyObscuredByShroud()` (black shroud),
/// not the object-attach Fogged test.
fn drawable_state_is_shrouded(
    state: &gamelogic::helpers::DrawableState,
    local_player_index: i32,
) -> bool {
    if let Some(draw) = state.drawable.as_ref() {
        if let Ok(guard) = draw.read() {
            return guard.fully_obscured_by_shroud();
        }
    }
    if state.shroud_status_object_id == 0 {
        return false;
    }
    let Some(object) =
        gamelogic::helpers::TheGameLogic::find_object_by_id(state.shroud_status_object_id)
    else {
        return false;
    };
    let Ok(guard) = object.read() else {
        return false;
    };
    use gamelogic::common::types::ObjectShroudStatus;
    // Fallback when the drawable flag is unavailable: full black shroud only.
    (guard.get_shrouded_status(local_player_index) as u8) >= (ObjectShroudStatus::Shrouded as u8)
}

fn xfer_random_variable(
    xfer: &mut dyn Xfer,
    value: &mut GameClientRandomVariable,
) -> Result<(), String> {
    xfer.xfer_unsigned_int(&mut value.distribution_type)
        .map_err(|e| e.to_string())?;
    xfer.xfer_real(&mut value.min).map_err(|e| e.to_string())?;
    xfer.xfer_real(&mut value.max).map_err(|e| e.to_string())?;
    Ok(())
}

fn xfer_cpp_enum(
    xfer: &mut dyn Xfer,
    value: &mut u32,
    valid: impl Fn(u32) -> bool,
    fallback: u32,
) -> Result<(), String> {
    xfer.xfer_unsigned_int(value).map_err(|e| e.to_string())?;
    if xfer.get_xfer_mode() == XferMode::Load && !valid(*value) {
        *value = fallback;
    }
    Ok(())
}

fn xfer_particle_shader_type(
    xfer: &mut dyn Xfer,
    value: &mut ParticleShaderType,
) -> Result<(), String> {
    let mut raw = *value as u32;
    xfer_cpp_enum(xfer, &mut raw, |v| v <= 4, ParticleShaderType::Alpha as u32)?;
    *value = match raw {
        0 => ParticleShaderType::Invalid,
        1 => ParticleShaderType::Additive,
        2 => ParticleShaderType::Alpha,
        3 => ParticleShaderType::AlphaTest,
        4 => ParticleShaderType::Multiply,
        _ => ParticleShaderType::Alpha,
    };
    Ok(())
}

fn xfer_particle_type(xfer: &mut dyn Xfer, value: &mut ParticleType) -> Result<(), String> {
    let mut raw = *value as u32;
    xfer_cpp_enum(xfer, &mut raw, |v| v <= 5, ParticleType::Particle as u32)?;
    *value = match raw {
        0 => ParticleType::Invalid,
        1 => ParticleType::Particle,
        2 => ParticleType::Drawable,
        3 => ParticleType::Streak,
        4 => ParticleType::VolumeParticle,
        5 => ParticleType::Smudge,
        _ => ParticleType::Particle,
    };
    Ok(())
}

fn xfer_particle_priority_type(
    xfer: &mut dyn Xfer,
    value: &mut ParticlePriorityType,
) -> Result<(), String> {
    let mut raw = *value as u32;
    xfer_cpp_enum(
        xfer,
        &mut raw,
        |v| ParticlePriorityType::from_index(v as usize).is_some(),
        ParticlePriorityType::Critical as u32,
    )?;
    *value =
        ParticlePriorityType::from_index(raw as usize).unwrap_or(ParticlePriorityType::Critical);
    Ok(())
}

fn xfer_emission_velocity_type(
    xfer: &mut dyn Xfer,
    value: &mut EmissionVelocityType,
) -> Result<(), String> {
    let mut raw = *value as u32;
    xfer_cpp_enum(
        xfer,
        &mut raw,
        |v| v <= 5,
        EmissionVelocityType::Spherical as u32,
    )?;
    *value = match raw {
        0 => EmissionVelocityType::Invalid,
        1 => EmissionVelocityType::Ortho,
        2 => EmissionVelocityType::Spherical,
        3 => EmissionVelocityType::Hemispherical,
        4 => EmissionVelocityType::Cylindrical,
        5 => EmissionVelocityType::Outward,
        _ => EmissionVelocityType::Spherical,
    };
    Ok(())
}

fn xfer_emission_volume_type(
    xfer: &mut dyn Xfer,
    value: &mut EmissionVolumeType,
) -> Result<(), String> {
    let mut raw = *value as u32;
    xfer_cpp_enum(xfer, &mut raw, |v| v <= 5, EmissionVolumeType::Point as u32)?;
    *value = match raw {
        0 => EmissionVolumeType::Invalid,
        1 => EmissionVolumeType::Point,
        2 => EmissionVolumeType::Line,
        3 => EmissionVolumeType::Box,
        4 => EmissionVolumeType::Sphere,
        5 => EmissionVolumeType::Cylinder,
        _ => EmissionVolumeType::Point,
    };
    Ok(())
}

fn xfer_wind_motion(xfer: &mut dyn Xfer, value: &mut WindMotion) -> Result<(), String> {
    let mut raw = *value as u32;
    xfer_cpp_enum(xfer, &mut raw, |v| v <= 3, WindMotion::NotUsed as u32)?;
    *value = match raw {
        0 => WindMotion::Invalid,
        1 => WindMotion::NotUsed,
        2 => WindMotion::PingPong,
        3 => WindMotion::Circular,
        _ => WindMotion::NotUsed,
    };
    Ok(())
}

fn xfer_emission_velocity(
    xfer: &mut dyn Xfer,
    velocity_type: EmissionVelocityType,
    velocity: &mut EmissionVelocity,
) -> Result<(), String> {
    match velocity_type {
        EmissionVelocityType::Invalid => {}
        EmissionVelocityType::Ortho => {
            let (mut x, mut y, mut z) = match *velocity {
                EmissionVelocity::Ortho { x, y, z } => (x, y, z),
                _ => (
                    GameClientRandomVariable::default(),
                    GameClientRandomVariable::default(),
                    GameClientRandomVariable::default(),
                ),
            };
            xfer_random_variable(xfer, &mut x)?;
            xfer_random_variable(xfer, &mut y)?;
            xfer_random_variable(xfer, &mut z)?;
            *velocity = EmissionVelocity::Ortho { x, y, z };
        }
        EmissionVelocityType::Spherical => {
            let mut speed = match *velocity {
                EmissionVelocity::Spherical { speed } => speed,
                _ => GameClientRandomVariable::default(),
            };
            xfer_random_variable(xfer, &mut speed)?;
            *velocity = EmissionVelocity::Spherical { speed };
        }
        EmissionVelocityType::Hemispherical => {
            let mut speed = match *velocity {
                EmissionVelocity::Hemispherical { speed } => speed,
                _ => GameClientRandomVariable::default(),
            };
            xfer_random_variable(xfer, &mut speed)?;
            *velocity = EmissionVelocity::Hemispherical { speed };
        }
        EmissionVelocityType::Cylindrical => {
            let (mut radial, mut normal) = match *velocity {
                EmissionVelocity::Cylindrical { radial, normal } => (radial, normal),
                _ => (
                    GameClientRandomVariable::default(),
                    GameClientRandomVariable::default(),
                ),
            };
            xfer_random_variable(xfer, &mut radial)?;
            xfer_random_variable(xfer, &mut normal)?;
            *velocity = EmissionVelocity::Cylindrical { radial, normal };
        }
        EmissionVelocityType::Outward => {
            let (mut speed, mut other_speed) = match *velocity {
                EmissionVelocity::Outward { speed, other_speed } => (speed, other_speed),
                _ => (
                    GameClientRandomVariable::default(),
                    GameClientRandomVariable::default(),
                ),
            };
            xfer_random_variable(xfer, &mut speed)?;
            xfer_random_variable(xfer, &mut other_speed)?;
            *velocity = EmissionVelocity::Outward { speed, other_speed };
        }
    }
    Ok(())
}

fn xfer_emission_volume(
    xfer: &mut dyn Xfer,
    volume_type: EmissionVolumeType,
    volume: &mut EmissionVolume,
) -> Result<(), String> {
    match volume_type {
        // C++ stores no union payload for INVALID_VOLUME.  Keep the Rust
        // storage benign while the separate enum still preserves that state.
        EmissionVolumeType::Invalid => {
            *volume = EmissionVolume::Point;
        }
        EmissionVolumeType::Point => {
            *volume = EmissionVolume::Point;
        }
        EmissionVolumeType::Line => {
            let (mut start, mut end) = match *volume {
                EmissionVolume::Line { start, end } => (start, end),
                _ => (Vec3::ZERO, Vec3::ZERO),
            };
            xfer_point3(xfer, &mut start)?;
            xfer_point3(xfer, &mut end)?;
            *volume = EmissionVolume::Line { start, end };
        }
        EmissionVolumeType::Box => {
            let mut half_size = match *volume {
                EmissionVolume::Box { half_size } => half_size,
                _ => Vec3::ZERO,
            };
            xfer_vec3(xfer, &mut half_size)?;
            *volume = EmissionVolume::Box { half_size };
        }
        EmissionVolumeType::Sphere => {
            let mut radius = match *volume {
                EmissionVolume::Sphere { radius } => radius,
                _ => 0.0,
            };
            xfer.xfer_real(&mut radius).map_err(|e| e.to_string())?;
            *volume = EmissionVolume::Sphere { radius };
        }
        EmissionVolumeType::Cylinder => {
            let (mut radius, mut length) = match *volume {
                EmissionVolume::Cylinder { radius, length } => (radius, length),
                _ => (0.0, 0.0),
            };
            xfer.xfer_real(&mut radius).map_err(|e| e.to_string())?;
            xfer.xfer_real(&mut length).map_err(|e| e.to_string())?;
            *volume = EmissionVolume::Cylinder { radius, length };
        }
    }
    Ok(())
}

/// Result of the emit half of `ParticleSystem::update` (C++ delay early-out vs emit).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameEmitPhase {
    Dead,
    Delayed,
    Emitted,
}

/// One synchronous emission loop; no references into the registry survive a step.
pub(crate) struct EmissionCursor {
    count: u32,
    next: u32,
}

/// Particle system (matches C++ ParticleSystem)
pub struct ParticleSystem {
    // C++ m_template identity is separate from owned ParticleSystemInfo.
    template: Arc<ParticleSystemTemplate>,
    info: ParticleSystemInfo,
    system_id: ParticleSystemId,

    // Particle storage
    particles: VecDeque<Particle>,
    personality_counter: u32,

    // Attachment
    attached_drawable_id: DrawableId,
    attached_object_id: ObjectId,

    // Transform (C++ Matrix3D is 3x4: rotation + translation)
    local_transform: Mat3,
    local_translation: Vec3,
    transform: Mat3,
    transform_translation: Vec3,
    position: Vec3,
    last_position: Vec3,

    // Timing
    burst_delay_left: u32,
    delay_left: u32,
    start_timestamp: u32,
    system_lifetime_left: u32,

    // Size accumulation for StartSizeRate
    accumulated_size_bonus: f32,

    // Coefficients for scaling
    vel_coeff: Vec3,
    count_coeff: f32,
    delay_coeff: f32,
    size_coeff: f32,

    // Slave system
    slave_system: Option<ParticleSystemId>,
    master_system: Option<ParticleSystemId>,

    // Control particle
    control_particle_pos: Option<Vec3>,

    // State flags
    is_local_identity: bool,
    is_identity: bool,
    is_forever: bool,
    is_stopped: bool,
    is_destroyed: bool,
    is_first_pos: bool,
    is_saveable: bool,
    skip_parent_transform: bool,

    // Shroud/visibility state (C++ ParticleSystem::m_isShrouded)
    is_shrouded: bool,

    // Parent transform override for attached systems
    parent_transform: Option<Mat3>,

    // Particle scale multiplier
    particle_scale: f32,

    // Emission overrides
    emission_volume_override: Option<EmissionVolume>,
    emission_volume_type_override: Option<EmissionVolumeType>,

    /// Burst infos waiting for C++ `createParticle` LOD/budget gates (manager applies).
    pending_emission_infos: Vec<ParticleInfo>,
}

impl ParticleSystem {
    /// Create a new particle system (matches C++ ParticleSystem constructor)
    pub fn new(
        template: Arc<ParticleSystemTemplate>,
        system_id: ParticleSystemId,
        create_slaves: bool,
    ) -> Self {
        Self::new_with_random(
            template,
            system_id,
            create_slaves,
            &mut get_game_client_random_value_real,
        )
    }

    /// Constructor observations belong to the receiving presentation session.
    /// Initial delay and wind keep their original order before admission.
    pub fn new_with_random(
        template: Arc<ParticleSystemTemplate>,
        system_id: ParticleSystemId,
        create_slaves: bool,
        random: &mut dyn FnMut(f32, f32) -> f32,
    ) -> Self {
        let mut info = template.info().clone();
        // C++ converts only owned runtime ColorScale and resets its distribution
        // to UNIFORM; immutable authored definitions retain their source units.
        info.color_scale = GameClientRandomVariable::new(
            info.color_scale.min / 255.0,
            info.color_scale.max / 255.0,
        );

        let mut system = Self {
            template,
            system_id,

            particles: VecDeque::new(),
            personality_counter: 0,

            attached_drawable_id: DrawableId::INVALID,
            attached_object_id: 0,

            local_transform: Mat3::IDENTITY,
            local_translation: Vec3::ZERO,
            transform: Mat3::IDENTITY,
            transform_translation: Vec3::ZERO,
            position: Vec3::ZERO,
            last_position: Vec3::ZERO,

            burst_delay_left: 0,
            delay_left: info.initial_delay.sample_with(&mut *random) as u32,
            start_timestamp: 0, // Will be set when started
            system_lifetime_left: info.system_lifetime,

            accumulated_size_bonus: 0.0,

            vel_coeff: Vec3::new(1.0, 1.0, 1.0),
            count_coeff: 1.0,
            delay_coeff: 1.0,
            size_coeff: 1.0,

            slave_system: None,
            master_system: None,

            control_particle_pos: None,

            is_local_identity: true,
            is_identity: true,
            is_forever: info.system_lifetime == 0,
            is_stopped: false,
            is_destroyed: false,
            is_first_pos: true,
            is_saveable: true,
            skip_parent_transform: false,

            is_shrouded: false,
            parent_transform: None,
            particle_scale: 1.0,

            emission_volume_override: None,
            emission_volume_type_override: None,

            pending_emission_infos: Vec::new(),
            info,
        };

        // Initialize wind motion
        system.initialize_wind_motion_with_random(random);

        system
    }

    /// Get immutable access to particles
    pub fn particles(&self) -> &VecDeque<Particle> {
        &self.particles
    }

    /// Get mutable access to particles  
    pub(crate) fn particles_mut(&mut self) -> &mut [Particle] {
        self.particles.make_contiguous()
    }

    /// Update the particle system (matches C++ ParticleSystem::update)
    ///
    /// # Arguments
    /// * `local_player_index` - Player index for visibility checks
    /// * `current_frame` - Current game frame for timing
    /// C++ update order: delay (return) → wind/transform → emit → particle update → lifetime.
    pub(crate) fn update(&mut self, local_player_index: i32, current_frame: u32) -> bool {
        match self.begin_frame_emit(local_player_index, current_frame) {
            FrameEmitPhase::Dead => false,
            FrameEmitPhase::Delayed => true,
            FrameEmitPhase::Emitted => {
                self.finish_frame_integrate(current_frame);
                !self.should_remove()
            }
        }
    }

    /// Compatibility observer; production commits each cursor step immediately.
    pub fn begin_frame_emit(
        &mut self,
        local_player_index: i32,
        current_frame: u32,
    ) -> FrameEmitPhase {
        let phase = self.prepare_frame_emit(local_player_index, current_frame);
        if matches!(phase, FrameEmitPhase::Emitted) {
            if let Some(mut cursor) = self.begin_emission_burst() {
                while let Some(info) = self.next_emission(&mut cursor) {
                    self.pending_emission_infos.push(info);
                }
                self.finish_emission_burst(cursor);
            }
        }
        phase
    }

    /// Delay, wind and transform phase before the first per-particle admission.
    pub(crate) fn prepare_frame_emit(
        &mut self,
        local_player_index: i32,
        current_frame: u32,
    ) -> FrameEmitPhase {
        // C++ ParticleSys.cpp: initial delay also delays lifetime (early return).
        if self.delay_left > 0 {
            self.delay_left -= 1;
            if self.delay_left == 0 {
                self.start_timestamp = current_frame;
            }
            return FrameEmitPhase::Delayed;
        }

        // C++1846 advances wind before attachment resolution, even if an
        // empty destroyed system retires at the end of this preparation.
        self.update_wind_motion();
        self.resolve_attached_parent(local_player_index);
        self.update_transform_from_parent();
        self.apply_control_particle_origin();

        if self.is_destroyed && self.particles.is_empty() && self.pending_emission_infos.is_empty()
        {
            return FrameEmitPhase::Dead;
        }

        FrameEmitPhase::Emitted
    }

    /// C++ update return false: destroyed with no particles, or finite lifetime
    /// exhausted with nothing left in the air (including pending createParticle).
    #[must_use]
    pub fn should_remove(&self) -> bool {
        if self.is_destroyed && self.particles.is_empty() && self.pending_emission_infos.is_empty()
        {
            return true;
        }
        if !self.is_forever
            && self.system_lifetime_left == 0
            && self.particles.is_empty()
            && self.pending_emission_infos.is_empty()
        {
            return true;
        }
        false
    }

    /// Set position (matches C++ ParticleSystem::setPosition).
    /// Writes local Matrix3D translation and clears local-identity.
    pub fn set_position(&mut self, pos: Vec3) {
        self.local_translation = pos;
        self.is_local_identity = false;
        self.update_transform();
    }

    /// World emission origin for callers (LOD / manager). C++ getPosition
    /// returns local translation; after compose this is parent*local origin.
    pub fn position(&self) -> Vec3 {
        Vec3::from(self.transform_translation)
    }

    /// Get emission volume type (uses overrides if present).
    pub fn get_emission_volume_type(&self) -> EmissionVolumeType {
        if let Some(kind) = self.emission_volume_type_override {
            return kind;
        }
        self.info().emission_volume_type
    }

    /// Set emission volume to a sphere with the given radius (instance override).
    pub fn set_emission_volume_sphere_radius(&mut self, radius: f32) {
        self.emission_volume_override = Some(EmissionVolume::Sphere { radius });
        self.emission_volume_type_override = Some(EmissionVolumeType::Sphere);
    }

    /// Set emission volume to a cylinder with the given radius (instance override).
    pub fn set_emission_volume_cylinder_radius(&mut self, radius: f32) {
        let length = match self.effective_emission_volume() {
            EmissionVolume::Cylinder { length, .. } => length,
            _ => 0.0,
        };
        self.emission_volume_override = Some(EmissionVolume::Cylinder { radius, length });
        self.emission_volume_type_override = Some(EmissionVolumeType::Cylinder);
    }

    /// Set local transform (matches C++ ParticleSystem::setLocalTransform)
    pub fn set_local_transform(&mut self, matrix: Mat3) {
        self.local_transform = matrix;
        self.is_local_identity = matrix == Mat3::IDENTITY && self.local_translation == Vec3::ZERO;
        self.update_transform();
    }

    /// Scale particle launch velocity for this system instance.
    pub fn set_velocity_multiplier(&mut self, multiplier: Vec3) {
        self.vel_coeff = multiplier;
    }

    /// Scale burst count for this system instance.
    pub fn set_burst_count_multiplier(&mut self, multiplier: f32) {
        self.count_coeff = multiplier.max(0.0);
    }

    /// Attach to drawable (matches C++ ParticleSystem::attachToDrawable)
    pub fn attach_to_drawable(&mut self, drawable_id: DrawableId) {
        self.attached_drawable_id = drawable_id;
    }

    /// Attach to object (matches C++ ParticleSystem::attachToObject)
    pub fn attach_to_object(&mut self, object_id: ObjectId) {
        self.attached_object_id = object_id;
    }

    /// Get shader type
    pub fn shader_type(&self) -> ParticleShaderType {
        self.info().shader_type
    }

    /// Get drift velocity
    pub fn drift_velocity(&self) -> Vec3 {
        self.info().drift_velocity
    }

    /// Get attached drawable
    pub fn attached_drawable_id(&self) -> DrawableId {
        self.attached_drawable_id
    }

    /// Get attached object
    pub fn attached_object(&self) -> Option<ObjectId> {
        if self.attached_object_id != 0 {
            Some(self.attached_object_id)
        } else {
            None
        }
    }

    /// Get attached object ID (raw, for Xfer)
    pub fn attached_object_id(&self) -> ObjectId {
        self.attached_object_id
    }

    /// Check if system is stopped
    pub fn is_stopped(&self) -> bool {
        self.is_stopped
    }

    /// Get particle type name
    pub fn particle_type_name(&self) -> &str {
        &self.info().particle_type_name
    }

    /// Get slave position offset
    pub fn slave_position_offset(&self) -> Vec3 {
        self.info().slave_pos_offset
    }

    /// Set system lifetime
    pub fn set_system_lifetime(&mut self, frames: u32) {
        self.system_lifetime_left = frames;
    }

    /// Check if system is forever
    pub fn is_system_forever(&self) -> bool {
        self.is_forever
    }

    /// Check if saveable
    pub fn is_saveable(&self) -> bool {
        self.is_saveable
    }

    /// Set saveable (matches C++ ParticleSystem::setSaveable)
    /// Cascades to slave system if present (C++ line 1232-1233)
    pub fn set_saveable(&mut self, b: bool) {
        self.is_saveable = b;
        // Note: In C++, this cascades to slave via direct pointer.
        // In Rust, slave_system is an ID — the manager handles cascading.
    }

    /// Set master system (matches C++ ParticleSystem::setMaster)
    pub fn set_master(&mut self, master_id: Option<ParticleSystemId>) {
        self.master_system = master_id;
    }

    /// Set slave system (matches C++ ParticleSystem::setSlave)
    pub fn set_slave(&mut self, slave_id: Option<ParticleSystemId>) {
        self.slave_system = slave_id;
    }

    /// Get slave system ID
    pub fn slave_system_id(&self) -> Option<ParticleSystemId> {
        self.slave_system
    }

    /// Get master system ID
    pub fn master_system_id(&self) -> Option<ParticleSystemId> {
        self.master_system
    }

    /// Drain the compatibility observer emission records.
    pub fn take_pending_emissions(&mut self) -> Vec<ParticleInfo> {
        std::mem::take(&mut self.pending_emission_infos)
    }

    /// Set skip parent transform
    pub fn set_skip_parent_xfrm(&mut self, enable: bool) {
        self.skip_parent_transform = enable;
    }

    /// Set shroud visibility state. C++ parity: ParticleSys.cpp lines 1860-1884
    pub fn set_shrouded(&mut self, shrouded: bool) {
        self.is_shrouded = shrouded;
    }

    /// C++ `ParticleSystem::m_isShrouded` after object-attach resolve.
    pub fn is_shrouded(&self) -> bool {
        self.is_shrouded
    }

    /// Set parent transform matrix. C++ parity: ParticleSys.cpp lines 1863, 1886-1890
    pub fn set_parent_transform(&mut self, transform: Option<Mat3>) {
        self.parent_transform = transform;
    }

    /// C++ `ParticleSystem::setControlParticle` — follow this world position.
    pub fn set_control_particle_position(&mut self, pos: Vec3) {
        self.control_particle_pos = Some(pos);
    }

    /// C++ detachControlParticle: remove only the receiving live pose binding.
    pub(super) fn clear_control_particle_position(&mut self) {
        self.control_particle_pos = None;
    }

    pub fn attached_system_name(&self) -> &str {
        &self.info().attached_system_name
    }

    pub fn is_emit_above_ground_only(&self) -> bool {
        self.info().is_emit_above_ground_only
    }

    pub fn update_particle_scale(&mut self) {
        self.particle_scale = Self::read_particle_scale_from_global_data();
    }

    fn apply_control_particle_origin(&mut self) {
        let Some(pos) = self.control_particle_pos else {
            return;
        };
        self.last_position = self.position;
        self.position = pos;
        // C++ ParticleSys.cpp:1953-1956 — override world translation, not local.
        self.transform_translation = pos;
        self.is_identity = false;
    }

    /// C++ ParticleSys.cpp:1852-1905 — follow drawable/object or self-destroy.
    fn resolve_attached_parent(&mut self, local_player_index: i32) {
        if self.attached_drawable_id.is_valid() {
            if let Some(client) = gamelogic::helpers::TheGameClient::get() {
                if let Some(state) = client.find_drawable_by_id(self.attached_drawable_id.0) {
                    self.last_position = self.position;
                    if let Some(draw) = state.drawable.as_ref() {
                        if let Ok(guard) = draw.read() {
                            let (rot, trans) =
                                affine_from_glam_cols(guard.get_transform_matrix().to_cols_array());
                            self.parent_transform = Some(rot);
                            self.position = Vec3::from(trans);
                        } else {
                            self.apply_yaw_parent_pose(
                                state.position.x,
                                state.position.y,
                                state.position.z,
                                state.orientation,
                            );
                        }
                    } else {
                        self.apply_yaw_parent_pose(
                            state.position.x,
                            state.position.y,
                            state.position.z,
                            state.orientation,
                        );
                    }
                    self.is_shrouded = drawable_state_is_shrouded(&state, local_player_index);
                    return;
                }
            }
            self.attached_drawable_id = DrawableId::INVALID;
            self.destroy();
            return;
        }

        if self.attached_object_id != 0 {
            // Live host IDs are not leftover OBJECT_REGISTRY entries.
            // C++ ParticleSys.cpp:1880-1905 follows TheGameLogic object; on
            // the production host path that is the published host pose / live
            // drawable, not leftover find_object.
            if let Some(pose) = gamelogic::helpers::host_fx_object_pose(self.attached_object_id)
                .or_else(|| {
                    crate::core::game_client::query_live_drawable_fx_pose(self.attached_object_id)
                })
            {
                self.apply_host_fx_object_pose(&pose);
                return;
            }
            if let Some(object) =
                gamelogic::helpers::TheGameLogic::find_object_by_id(self.attached_object_id)
            {
                if let Ok(guard) = object.read() {
                    use gamelogic::common::types::ObjectShroudStatus;
                    let status = guard.get_shrouded_status(local_player_index);
                    self.is_shrouded = (status as u8) >= (ObjectShroudStatus::Fogged as u8);
                    self.last_position = self.position;
                    if let Some(draw) = guard.get_drawable() {
                        if let Ok(draw_guard) = draw.read() {
                            let (rot, trans) = affine_from_glam_cols(
                                draw_guard.get_transform_matrix().to_cols_array(),
                            );
                            self.parent_transform = Some(rot);
                            self.position = Vec3::from(trans);
                        } else {
                            let (rot, trans) =
                                affine_from_glam_cols(guard.get_transform_matrix().to_cols_array());
                            self.parent_transform = Some(rot);
                            self.position = Vec3::from(trans);
                        }
                    } else {
                        let (rot, trans) =
                            affine_from_glam_cols(guard.get_transform_matrix().to_cols_array());
                        self.parent_transform = Some(rot);
                        self.position = Vec3::from(trans);
                    }
                }
                return;
            }
            self.attached_object_id = 0;
            self.destroy();
        }
    }

    fn apply_yaw_parent_pose(&mut self, x: f32, y: f32, z: f32, yaw: f32) {
        self.position = Vec3::new(x, y, z);
        let (s, c) = yaw.sin_cos();
        self.parent_transform = Some(Mat3::from_cols_array(&[
            c, s, 0.0, -s, c, 0.0, 0.0, 0.0, 1.0,
        ]));
    }

    fn apply_host_fx_object_pose(&mut self, pose: &gamelogic::helpers::HostFxObjectPose) {
        self.last_position = self.position;
        let (rot, trans) = affine_from_glam_cols(pose.transform.to_cols_array());
        self.parent_transform = Some(rot);
        self.position = Vec3::from(trans);
        // C++ ParticleSys.cpp:1880-1905 object-attach Fogged+ emit gate.
        self.is_shrouded = pose.is_shrouded;
    }

    fn read_particle_scale_from_global_data() -> f32 {
        match game_engine::common::ini::ini_game_data::get_global_data() {
            Some(arc) => arc.read().particle_scale,
            None => 1.0,
        }
    }

    /// Get velocity multiplier
    pub fn velocity_multiplier(&self) -> Vec3 {
        self.vel_coeff
    }

    /// Get burst delay multiplier
    pub fn burst_delay_multiplier(&self) -> f32 {
        self.delay_coeff
    }

    /// Get size multiplier
    pub fn size_multiplier(&self) -> f32 {
        self.size_coeff
    }

    /// C++ `ParticleSystem::setSizeMultiplier`.
    pub fn set_size_multiplier(&mut self, multiplier: f32) {
        self.size_coeff = multiplier;
    }

    /// Get burst count multiplier
    pub fn burst_count_multiplier(&self) -> f32 {
        self.count_coeff
    }

    /// Should billboard
    pub fn should_billboard(&self) -> bool {
        !self.info().is_ground_aligned
    }

    /// C++ `ParticleSystemInfo::isUsingSmudge`.
    pub fn is_using_smudge(&self) -> bool {
        self.info().particle_type == ParticleType::Smudge
    }

    /// Get start frame
    pub fn start_frame(&self) -> u32 {
        self.start_timestamp
    }

    /// Set initial delay
    pub fn set_initial_delay(&mut self, delay: u32) {
        self.delay_left = delay;
    }

    /// Remaining initial delay frames (C++ ParticleSystem::m_delayLeft).
    pub fn initial_delay_left(&self) -> u32 {
        self.delay_left
    }

    /// Local transform used for orient-to-object / rotate residuals.
    pub fn local_transform(&self) -> Mat3 {
        self.local_transform
    }

    /// Set lifetime range (C++ ParticleSystem::setLifetimeRange → m_lifetime.setRange).
    pub fn set_lifetime_range(&mut self, min: f32, max: f32) {
        self.info.lifetime = GameClientRandomVariable::new(min, max);
    }

    /// Rotate local transform X (matches C++ ParticleSystem::rotateLocalTransformX)
    pub fn rotate_local_transform_x(&mut self, angle: f32) {
        let cos_a = angle.cos();
        let sin_a = angle.sin();
        let rot = Mat3::from_cols(
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(0.0, cos_a, sin_a),
            Vec3::new(0.0, -sin_a, cos_a),
        );
        self.local_transform = rot * self.local_transform;
        self.is_local_identity = false;
        self.update_transform();
    }

    /// Rotate local transform Y (matches C++ ParticleSystem::rotateLocalTransformY)
    pub fn rotate_local_transform_y(&mut self, angle: f32) {
        let cos_a = angle.cos();
        let sin_a = angle.sin();
        let rot = Mat3::from_cols(
            Vec3::new(cos_a, 0.0, -sin_a),
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::new(sin_a, 0.0, cos_a),
        );
        self.local_transform = rot * self.local_transform;
        self.is_local_identity = false;
        self.update_transform();
    }

    /// Rotate local transform Z (matches C++ ParticleSystem::rotateLocalTransformZ)
    pub fn rotate_local_transform_z(&mut self, angle: f32) {
        let cos_a = angle.cos();
        let sin_a = angle.sin();
        let rot = Mat3::from_cols(
            Vec3::new(cos_a, sin_a, 0.0),
            Vec3::new(-sin_a, cos_a, 0.0),
            Vec3::new(0.0, 0.0, 1.0),
        );
        self.local_transform = rot * self.local_transform;
        self.is_local_identity = false;
        self.update_transform();
    }

    /// Start the particle system (matches C++ ParticleSystem::start)
    pub fn start(&mut self) {
        self.is_stopped = false;
    }

    /// Stop the particle system (matches C++ ParticleSystem::stop)
    pub fn stop(&mut self) {
        self.is_stopped = true;
    }

    /// Destroy the particle system (matches C++ ParticleSystem::destroy)
    /// In C++, this cascades to slave via m_slaveSystem->destroy().
    /// In Rust, the manager handles cascading via slave_system ID.
    pub fn destroy(&mut self) {
        self.is_destroyed = true;
    }

    /// Check if destroyed
    pub fn is_destroyed(&self) -> bool {
        self.is_destroyed
    }

    /// Trigger immediate burst (matches C++ ParticleSystem::trigger)
    pub fn trigger(&mut self) {
        self.burst_delay_left = 0;
        self.delay_left = 0;
    }

    /// Get particle count
    pub fn particle_count(&self) -> usize {
        self.particles.len()
    }

    /// Get system priority for C++-style particle budget culling.
    pub fn priority(&self) -> ParticlePriorityType {
        self.info().priority
    }

    /// Get personality counter (for creating particles externally, e.g., slave emissions)
    pub fn personality_counter(&self) -> u32 {
        self.personality_counter
    }

    /// Push a pre-created particle into the system (used by manager for slave emission)
    #[cfg(test)]
    pub(crate) fn push_particle(&mut self, particle: Particle) {
        self.push_particle_inner(particle);
    }

    pub(super) fn push_admitted_particle(&mut self, mut particle: Particle, ordinal: u64) {
        particle.admission_ordinal = Some(ordinal);
        self.push_particle_inner(particle);
    }

    fn push_particle_inner(&mut self, particle: Particle) {
        self.personality_counter = self.personality_counter.wrapping_add(1);
        self.particles.push_back(particle);
    }

    /// Remove oldest particles from the front of this system's queue.
    #[cfg(test)]
    pub(crate) fn remove_oldest_particles(&mut self, count: usize) -> usize {
        let remove_count = count.min(self.particles.len());
        for _ in 0..remove_count {
            self.particles.pop_front();
        }
        remove_count
    }

    pub(super) fn remove_admitted_particle(&mut self, index: usize) -> Particle {
        self.particles
            .remove(index)
            .expect("admitted eviction particle exists")
    }

    /// Get wind angle
    pub fn wind_angle(&self) -> f32 {
        self.info.wind_angle
    }

    pub(crate) fn begin_emission_burst(&mut self) -> Option<EmissionCursor> {
        if self.is_destroyed
            || (!self.is_forever && self.system_lifetime_left == 0)
            || self.is_shrouded
            || self.is_stopped
            || self.master_system.is_some()
        {
            return None;
        }
        if self.burst_delay_left > 0 {
            self.burst_delay_left -= 1;
            return None;
        }
        Some(EmissionCursor {
            // C++ REAL_TO_INT first, then the compound count coefficient cast.
            count: (((self.info.burst_count.sample() as i32) as f32 * self.count_coeff) as i32)
                .max(0) as u32,
            next: 0,
        })
    }

    pub(crate) fn next_emission(&mut self, cursor: &mut EmissionCursor) -> Option<ParticleInfo> {
        if cursor.next == cursor.count {
            return None;
        }
        let index = cursor.next;
        cursor.next += 1;
        self.generate_particle_info(index, cursor.count)
    }

    pub(crate) fn finish_emission_burst(&mut self, _cursor: EmissionCursor) {
        let delay = self.info.burst_delay.sample() as u32;
        self.burst_delay_left = (delay as f32 * self.delay_coeff) as u32;
    }

    /// Update transform matrix
    fn update_transform(&mut self) {
        self.transform = self.local_transform;
        self.transform_translation = self.local_translation;
        self.is_identity = self.is_local_identity;
    }

    // C++ parity: ParticleSys.cpp lines 1910-1946
    fn update_transform_from_parent(&mut self) {
        if let Some(parent_xfrm) = &self.parent_transform {
            if self.skip_parent_transform {
                self.transform = self.local_transform;
                self.transform_translation = self.local_translation;
            } else if !self.is_local_identity {
                self.transform = parent_xfrm * self.local_transform;
                self.transform_translation = parent_xfrm * self.local_translation + self.position;
            } else {
                self.transform = *parent_xfrm;
                self.transform_translation = self.position;
            }
            self.is_identity = false;
        } else if !self.is_local_identity {
            self.transform = self.local_transform;
            self.transform_translation = self.local_translation;
            self.is_identity = false;
        } else {
            self.transform = Mat3::IDENTITY;
            self.transform_translation = Vec3::ZERO;
            self.is_identity = true;
        }
    }

    /// Initialize wind motion (C++ ParticleSystem ctor lines 1114-1125)
    fn initialize_wind_motion_with_random(&mut self, random: &mut dyn FnMut(f32, f32) -> f32) {
        let info = &mut self.info;

        // C++ ParticleSys.cpp:1123-1125 — three client-stream draws. The client
        // residual returns `hi` for empty ranges, so no min/max clamping needed.
        info.wind_motion_start_angle = random(
            info.wind_motion_start_angle_min,
            info.wind_motion_start_angle_max,
        );
        info.wind_motion_end_angle = random(
            info.wind_motion_end_angle_min,
            info.wind_motion_end_angle_max,
        );
        info.wind_angle = random(info.wind_motion_start_angle, info.wind_motion_end_angle);
    }

    /// Update wind motion (matches C++ ParticleSys.cpp lines 2085-2180)
    fn update_wind_motion(&mut self) {
        let info = &mut self.info;

        match info.wind_motion {
            WindMotion::Invalid | WindMotion::NotUsed => {
                // No wind motion.
            }
            WindMotion::PingPong => {
                let start_angle = info.wind_motion_start_angle;
                let end_angle = info.wind_motion_end_angle;

                let total_span = end_angle - start_angle;
                let half_span = total_span / 2.0;
                let diff_from_center = (half_span - info.wind_angle + start_angle).abs();

                const MINIMUM_CHANGE: f32 = 0.005;
                let mut change = (1.0 - (diff_from_center / half_span)) * info.wind_angle_change;
                if change < MINIMUM_CHANGE {
                    change = MINIMUM_CHANGE;
                }

                if info.wind_motion_moving_to_end_angle {
                    info.wind_angle += change;
                    if info.wind_angle >= end_angle {
                        info.wind_motion_moving_to_end_angle = false;
                        // C++ ParticleSys.cpp:2137-2146.
                        info.wind_angle_change = get_game_client_random_value_real(
                            info.wind_angle_change_min,
                            info.wind_angle_change_max,
                        );
                        info.wind_motion_start_angle = get_game_client_random_value_real(
                            info.wind_motion_start_angle_min,
                            info.wind_motion_start_angle_max,
                        );
                        info.wind_motion_end_angle = get_game_client_random_value_real(
                            info.wind_motion_end_angle_min,
                            info.wind_motion_end_angle_max,
                        );
                    }
                } else {
                    info.wind_angle -= change;
                    if info.wind_angle <= start_angle {
                        info.wind_motion_moving_to_end_angle = true;
                        // C++ ParticleSys.cpp:2165-2174.
                        info.wind_angle_change = get_game_client_random_value_real(
                            info.wind_angle_change_min,
                            info.wind_angle_change_max,
                        );
                        info.wind_motion_start_angle = get_game_client_random_value_real(
                            info.wind_motion_start_angle_min,
                            info.wind_motion_start_angle_max,
                        );
                        info.wind_motion_end_angle = get_game_client_random_value_real(
                            info.wind_motion_end_angle_min,
                            info.wind_motion_end_angle_max,
                        );
                    }
                }
            }
            WindMotion::Circular => {
                if info.wind_angle_change == 0.0 {
                    // C++ ParticleSys.cpp:2189-2190.
                    info.wind_angle_change = get_game_client_random_value_real(
                        info.wind_angle_change_min,
                        info.wind_angle_change_max,
                    );
                }
                info.wind_angle += info.wind_angle_change;
                if info.wind_angle > std::f32::consts::TAU {
                    info.wind_angle -= std::f32::consts::TAU;
                } else if info.wind_angle < 0.0 {
                    info.wind_angle += std::f32::consts::TAU;
                }
            }
        }
    }

    /// Get system ID
    pub fn system_id(&self) -> ParticleSystemId {
        self.system_id
    }

    /// Immutable admitted authored definition; runtime changes live in info().
    pub fn template(&self) -> &Arc<ParticleSystemTemplate> {
        &self.template
    }

    /// Canonical owned runtime parameters: tint, lifetime and loaded state.
    pub fn info(&self) -> &ParticleSystemInfo {
        &self.info
    }

    /// Tint this system's owned color keys, preserving its authored identity.
    pub fn tint_all_colors(&mut self, tint_color: [f32; 3]) {
        self.info.tint_all_colors(tint_color);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_particle_creation() {
        let info = ParticleInfo::default();
        let particle = Particle::new(&info, 1, 0);

        assert_eq!(particle.personality, 1);
        assert_eq!(particle.lifetime_left, 30);
        // Unset Color1/Alpha1 are {0} / 0 — C++ newborns start from key 0.
        assert_eq!(particle.alpha, 0.0);
        assert_eq!(particle.color, [0.0, 0.0, 0.0]);
    }

    #[test]
    fn particle_new_applies_frame_zero_keyframes() {
        let mut info = ParticleInfo::default();
        info.alpha_keys[0] = Keyframe {
            value: 0.25,
            frame: 0,
        };
        info.color_keys[0] = RGBColorKeyframe {
            color: [0.1, 0.2, 0.3],
            frame: 0,
        };
        let particle = Particle::new(&info, 1, 0);
        assert_eq!(particle.alpha, 0.25);
        assert_eq!(particle.color, [0.1, 0.2, 0.3]);
    }

    #[test]
    fn set_position_writes_local_translation() {
        let template = Arc::new(ParticleSystemTemplate::new("Offset".to_string()));
        let mut system = ParticleSystem::new(template, 1, false);
        system.set_position(Vec3::new(10.0, 20.0, 30.0));
        assert_eq!(system.position(), Vec3::new(10.0, 20.0, 30.0));
        assert!(!system.is_local_identity);

        let info = system.generate_particle_info(0, 1).unwrap();
        assert_eq!(info.position, Vec3::new(10.0, 20.0, 30.0));
    }

    #[test]
    fn attached_parent_applies_set_position_through_full_rotation() {
        // C++ ParticleSys.cpp:1281-1286 + :1919-1924 + :1746-1758
        // setPosition is local Matrix3D translation; update concatenates
        // parent 4x4 * local (including pitch/roll, not yaw-only).
        let template = Arc::new(ParticleSystemTemplate::new("BoneFx".to_string()));
        let mut system = ParticleSystem::new(template, 1, false);
        system.set_position(Vec3::new(0.0, 0.0, 10.0));
        system.position = Vec3::new(100.0, 200.0, 0.0);
        let (s, c) = std::f32::consts::FRAC_PI_2.sin_cos();
        // Rotate +90° about X: (0,0,10) → (0,-10,0)
        system.parent_transform = Some(Mat3::from_cols_array(&[
            1.0, 0.0, 0.0, 0.0, c, s, 0.0, -s, c,
        ]));
        system.update_transform_from_parent();

        let info = system.generate_particle_info(0, 1).unwrap();
        assert!((info.position.x - 100.0).abs() < 1e-5);
        assert!((info.position.y - 190.0).abs() < 1e-5);
        assert!(info.position.z.abs() < 1e-5);
        // Yaw-only parent would leave the +Z offset unrotated → z=10.
        assert!(info.position.z.abs() < 1.0);
    }

    #[test]
    fn skip_parent_xfrm_keeps_local_set_position() {
        // C++ ParticleSys.cpp:1912-1915 — skipParentXfrm uses m_localTransform only.
        let template = Arc::new(ParticleSystemTemplate::new("SkipParent".to_string()));
        let mut system = ParticleSystem::new(template, 1, false);
        system.set_position(Vec3::new(0.0, 0.0, 10.0));
        system.position = Vec3::new(100.0, 200.0, 0.0);
        system.parent_transform = Some(Mat3::from_cols_array(&[
            0.0, 1.0, 0.0, -1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
        ]));
        system.set_skip_parent_xfrm(true);
        system.update_transform_from_parent();

        let info = system.generate_particle_info(0, 1).unwrap();
        assert_eq!(info.position, Vec3::new(0.0, 0.0, 10.0));
    }

    #[test]
    fn generate_spreads_burst_along_parent_motion() {
        let template = Arc::new(ParticleSystemTemplate::new("Trail".to_string()));
        let mut system = ParticleSystem::new(template, 1, false);
        system.set_position(Vec3::ZERO);
        system.position = Vec3::new(10.0, 0.0, 0.0);
        system.last_position = Vec3::new(0.0, 0.0, 0.0);
        system.is_first_pos = false;
        system.is_identity = false;
        system.transform = Mat3::IDENTITY;
        system.transform_translation = Vec3::new(10.0, 0.0, 0.0);

        let first = system.generate_particle_info(0, 2).unwrap();
        let last = system.generate_particle_info(1, 2).unwrap();
        // i=0: subtract full (pos-last) → 0. i=1: subtract half → 5.
        assert_eq!(first.position.x, 0.0);
        assert_eq!(last.position.x, 5.0);
    }

    #[test]
    fn set_lifetime_range_samples_on_generate() {
        let template = Arc::new(ParticleSystemTemplate::new("Weld".to_string()));
        let mut system = ParticleSystem::new(template, 1, false);
        system.set_lifetime_range(99.0, 99.0);
        let info = system.generate_particle_info(0, 1).unwrap();
        assert_eq!(info.lifetime, 99);
    }

    #[test]
    fn line_volume_includes_vol_line_start() {
        let mut template = ParticleSystemTemplate::new("Beam".to_string());
        {
            let info = template.info_mut();
            info.emission_volume_type = EmissionVolumeType::Line;
            info.emission_volume = EmissionVolume::Line {
                start: Vec3::new(5.0, 0.0, 0.0),
                end: Vec3::new(5.0, 0.0, 0.0),
            };
        }
        let mut system = ParticleSystem::new(Arc::new(template), 1, false);
        let info = system.generate_particle_info(0, 1).unwrap();
        assert_eq!(info.position, Vec3::new(5.0, 0.0, 0.0));
    }

    #[test]
    fn test_particle_xfer_load_applies_particle_info_base() {
        use game_engine::common::system::xfer_load::XferLoad;
        use game_engine::common::system::xfer_save::XferSave;
        use std::io::Cursor;

        let mut info = ParticleInfo::default();
        info.velocity = Vec3::new(1.0, 2.0, 3.0);
        info.position = Vec3::new(4.0, 5.0, 6.0);
        info.emitter_position = Vec3::new(7.0, 8.0, 9.0);
        info.vel_damping = 0.75;
        info.angle_z = 10.0;
        info.angular_rate_z = 11.0;
        info.lifetime = 99;
        info.size = 12.0;
        info.size_rate = 13.0;
        info.size_rate_damping = 0.5;
        info.alpha_keys[0] = Keyframe {
            value: 0.25,
            frame: 14,
        };
        info.color_keys[0] = RGBColorKeyframe {
            color: [0.1, 0.2, 0.3],
            frame: 15,
        };
        info.color_scale = 0.8;
        info.wind_randomness = 0.6;
        info.particle_up_towards_emitter = true;

        let mut saved = Particle::new(&info, 123, 456);
        saved.acceleration = Vec3::new(16.0, 17.0, 18.0);
        saved.last_position = Vec3::new(19.0, 20.0, 21.0);
        saved.lifetime_left = 77;
        saved.alpha = 0.4;
        saved.alpha_rate = 0.05;
        saved.alpha_target_key = 2;
        saved.color = [0.4, 0.5, 0.6];
        saved.color_rate = [0.7, 0.8, 0.9];
        saved.color_target_key = 3;
        saved.controlled_system = Some(321);

        let mut bytes = Vec::new();
        {
            let cursor = Cursor::new(&mut bytes);
            let mut save = XferSave::new(cursor, 1);
            save.open("particle").unwrap();
            saved.xfer(&mut save).unwrap();
            save.close().unwrap();
        }

        let mut loaded = Particle::new(&ParticleInfo::default(), 0, 0);
        let mut load = XferLoad::new(Cursor::new(bytes), 1);
        load.open("particle").unwrap();
        loaded.xfer(&mut load).unwrap();
        load.close().unwrap();

        assert_eq!(loaded.velocity, Vec3::new(1.0, 2.0, 3.0));
        assert_eq!(loaded.position, Vec3::new(4.0, 5.0, 6.0));
        assert_eq!(loaded.emitter_position, Vec3::new(7.0, 8.0, 9.0));
        assert_eq!(loaded.vel_damping, 0.75);
        assert_eq!(loaded.angle_z, 10.0);
        assert_eq!(loaded.angular_rate_z, 11.0);
        assert_eq!(loaded.lifetime, 99);
        assert_eq!(loaded.size, 12.0);
        assert_eq!(loaded.size_rate, 13.0);
        assert_eq!(loaded.size_rate_damping, 0.5);
        assert_eq!(loaded.alpha_keys[0].value, 0.25);
        assert_eq!(loaded.alpha_keys[0].frame, 14);
        assert_eq!(loaded.color_keys[0].color, [0.1, 0.2, 0.3]);
        assert_eq!(loaded.color_keys[0].frame, 15);
        assert_eq!(loaded.color_scale, 0.8);
        assert_eq!(loaded.wind_randomness, 0.6);
        assert!(loaded.particle_up_towards_emitter);
        assert_eq!(loaded.personality, 123);
        assert_eq!(loaded.acceleration, Vec3::new(16.0, 17.0, 18.0));
        assert_eq!(loaded.last_position, Vec3::new(19.0, 20.0, 21.0));
        assert_eq!(loaded.lifetime_left, 77);
        assert_eq!(loaded.create_timestamp, 456);
        assert_eq!(loaded.alpha, 0.4);
        assert_eq!(loaded.alpha_rate, 0.05);
        assert_eq!(loaded.alpha_target_key, 2);
        assert_eq!(loaded.color, [0.4, 0.5, 0.6]);
        assert_eq!(loaded.color_rate, [0.7, 0.8, 0.9]);
        assert_eq!(loaded.color_target_key, 3);
        assert_eq!(loaded.controlled_system, Some(321));
    }

    #[test]
    fn test_particle_system_creation() {
        let template = Arc::new(ParticleSystemTemplate::new("TestSystem".to_string()));
        let mut system = ParticleSystem::new(template, 1, false);

        assert_eq!(system.system_id(), 1);
        assert_eq!(system.particle_count(), 0);
        assert!(!system.is_destroyed());
    }

    #[test]
    fn test_particle_system_xfer_preserves_base_info_and_matrix3d_layout() {
        use game_engine::common::system::xfer_load::XferLoad;
        use game_engine::common::system::xfer_save::XferSave;
        use std::io::Cursor;

        let mut template = ParticleSystemTemplate::new("SavedSystem".to_string());
        {
            let info = template.info_mut();
            info.is_one_shot = true;
            info.shader_type = ParticleShaderType::Multiply;
            info.particle_type = ParticleType::Drawable;
            info.particle_type_name = "SavedDrawable".to_string();
            info.priority = ParticlePriorityType::AlwaysRender;
            info.angle_z = GameClientRandomVariable::new(1.0, 2.0);
            info.angular_rate_z = GameClientRandomVariable::new(3.0, 4.0);
            info.angular_damping = GameClientRandomVariable::new(0.8, 0.9);
            info.vel_damping = GameClientRandomVariable::new(0.7, 0.75);
            info.lifetime = GameClientRandomVariable::new(30.0, 60.0);
            info.system_lifetime = 120;
            info.start_size = GameClientRandomVariable::new(5.0, 6.0);
            info.start_size_rate = GameClientRandomVariable::new(0.1, 0.2);
            info.size_rate = GameClientRandomVariable::new(0.3, 0.4);
            info.size_rate_damping = GameClientRandomVariable::new(0.95, 0.96);
            info.alpha_keys[0] = RandomKeyframe {
                min_value: 0.2,
                max_value: 0.4,
                distribution_type: 5,
                frame: 7,
            };
            info.color_keys[0] = RGBColorKeyframe {
                color: [0.1, 0.2, 0.3],
                frame: 8,
            };
            info.color_scale = GameClientRandomVariable::new(0.5, 0.6);
            info.burst_delay = GameClientRandomVariable::new(9.0, 10.0);
            info.burst_count = GameClientRandomVariable::new(11.0, 12.0);
            info.initial_delay = GameClientRandomVariable::new(13.0, 14.0);
            info.drift_velocity = Vec3::new(15.0, 16.0, 17.0);
            info.gravity = -0.25;
            info.slave_system_name = "SlaveSystem".to_string();
            info.slave_pos_offset = Vec3::new(18.0, 19.0, 20.0);
            info.attached_system_name = "AttachedSystem".to_string();
            info.emission_velocity_type = EmissionVelocityType::Cylindrical;
            info.emission_velocity = EmissionVelocity::Cylindrical {
                radial: GameClientRandomVariable::new(21.0, 22.0),
                normal: GameClientRandomVariable::new(23.0, 24.0),
            };
            info.emission_volume_type = EmissionVolumeType::Cylinder;
            info.emission_volume = EmissionVolume::Cylinder {
                radius: 25.0,
                length: 26.0,
            };
            info.is_emission_volume_hollow = true;
            info.is_ground_aligned = true;
            info.is_emit_above_ground_only = true;
            info.is_particle_up_towards_emitter = true;
            info.wind_motion = WindMotion::PingPong;
            info.wind_angle_change_min = 0.01;
            info.wind_angle_change_max = 0.02;
            info.wind_motion_start_angle = 0.03;
            info.wind_motion_start_angle_min = 0.04;
            info.wind_motion_start_angle_max = 0.05;
            info.wind_motion_end_angle = 0.06;
            info.wind_motion_end_angle_min = 0.07;
            info.wind_motion_end_angle_max = 0.08;
        }

        let mut saved = ParticleSystem::new(Arc::new(template), 42, false);
        saved.local_transform =
            Mat3::from_cols_array(&[1.0, 4.0, 7.0, 2.0, 5.0, 8.0, 3.0, 6.0, 9.0]);
        saved.transform = Mat3::from_cols_array(&[9.0, 6.0, 3.0, 8.0, 5.0, 2.0, 7.0, 4.0, 1.0]);
        saved.is_local_identity = false;
        saved.is_identity = false;
        // Snapshot fixture is a live C++ ParticleSystemInfo, not authored ranges.
        saved.info.wind_motion_start_angle = 0.03;
        saved.info.wind_motion_end_angle = 0.06;
        saved.info.wind_angle = 0.33;
        saved.info.wind_angle_change = 0.44;
        saved.info.wind_motion_moving_to_end_angle = true;

        let mut bytes = Vec::new();
        {
            let cursor = Cursor::new(&mut bytes);
            let mut save = XferSave::new(cursor, 1);
            save.open("particle_system").unwrap();
            saved.xfer(&mut save).unwrap();
            save.close().unwrap();
        }

        let mut loaded = ParticleSystem::new(
            Arc::new(ParticleSystemTemplate::new("LoadedSystem".to_string())),
            0,
            false,
        );
        let mut load = XferLoad::new(Cursor::new(bytes), 1);
        load.open("particle_system").unwrap();
        loaded.xfer(&mut load).unwrap();
        load.close().unwrap();

        let loaded_info = loaded.info();
        assert!(loaded_info.is_one_shot);
        assert_eq!(loaded_info.shader_type, ParticleShaderType::Multiply);
        assert_eq!(loaded_info.particle_type, ParticleType::Drawable);
        assert_eq!(loaded_info.particle_type_name, "SavedDrawable");
        assert_eq!(loaded_info.priority, ParticlePriorityType::AlwaysRender);
        assert_eq!(loaded_info.angle_z.min, 1.0);
        assert_eq!(loaded_info.angle_z.max, 2.0);
        assert_eq!(loaded_info.alpha_keys[0].min_value, 0.2);
        assert_eq!(loaded_info.alpha_keys[0].max_value, 0.4);
        assert_eq!(loaded_info.alpha_keys[0].distribution_type, 5);
        assert_eq!(loaded_info.alpha_keys[0].frame, 7);
        assert_eq!(loaded_info.color_keys[0].color, [0.1, 0.2, 0.3]);
        assert_eq!(loaded_info.color_keys[0].frame, 8);
        assert_eq!(loaded_info.drift_velocity, Vec3::new(15.0, 16.0, 17.0));
        assert_eq!(loaded_info.gravity, -0.25);
        assert_eq!(loaded_info.slave_system_name, "SlaveSystem");
        assert_eq!(loaded_info.slave_pos_offset, Vec3::new(18.0, 19.0, 20.0));
        assert_eq!(loaded_info.attached_system_name, "AttachedSystem");
        assert_eq!(
            loaded_info.emission_velocity_type,
            EmissionVelocityType::Cylindrical
        );
        assert!(matches!(
            loaded_info.emission_velocity,
            EmissionVelocity::Cylindrical { .. }
        ));
        assert_eq!(
            loaded_info.emission_volume_type,
            EmissionVolumeType::Cylinder
        );
        assert!(matches!(
            loaded_info.emission_volume,
            EmissionVolume::Cylinder {
                radius: 25.0,
                length: 26.0
            }
        ));
        assert!(loaded_info.is_emission_volume_hollow);
        assert!(loaded_info.is_ground_aligned);
        assert!(loaded_info.is_emit_above_ground_only);
        assert!(loaded_info.is_particle_up_towards_emitter);
        assert_eq!(loaded_info.wind_motion, WindMotion::PingPong);
        assert_eq!(loaded_info.wind_motion_start_angle, 0.03);
        assert_eq!(loaded_info.wind_motion_end_angle, 0.06);
        assert_eq!(loaded_info.wind_angle, 0.33);
        assert_eq!(loaded_info.wind_angle_change, 0.44);
        assert!(loaded_info.wind_motion_moving_to_end_angle);
        assert_eq!(loaded.system_id, 42);
        assert_eq!(loaded.local_transform, saved.local_transform);
        assert_eq!(loaded.transform, saved.transform);
    }

    #[test]
    fn test_emission_volumes() {
        // C++ draws all emission randomness from the seeded GameClient stream
        // (ParticleSys.cpp:1351-1648), so the same seed must reproduce
        // identical position/velocity samples. thread_rng broke that.
        let mut template = ParticleSystemTemplate::new("SeededEmission".to_string());
        {
            let info = template.info_mut();
            info.emission_volume = EmissionVolume::Sphere { radius: 5.0 };
            info.emission_velocity = EmissionVelocity::Spherical {
                speed: GameClientRandomVariable::new(1.0, 2.0),
            };
        }
        let mut system = ParticleSystem::new(Arc::new(template), 1, false);

        game_engine::common::random_value::init_random_with_seed(90210);
        let pos_a = system.compute_particle_position();
        let vel_a = system.compute_particle_velocity(&pos_a);

        game_engine::common::random_value::init_random_with_seed(90210);
        let pos_b = system.compute_particle_position();
        let vel_b = system.compute_particle_velocity(&pos_b);

        assert_eq!(pos_a, pos_b);
        assert_eq!(vel_a, vel_b);

        // Sphere emission stays inside the template radius, and spherical
        // speed magnitude lands in [1, 2].
        assert!(pos_a.length() <= 5.0 + 1e-3);
        let speed = vel_a.length();
        assert!((1.0..=2.0).contains(&speed), "speed {speed} outside [1, 2]");
    }

    #[test]
    fn test_slave_system_tracking() {
        let template = Arc::new(ParticleSystemTemplate::new("Master".to_string()));
        let mut system = ParticleSystem::new(template, 1, false);

        assert_eq!(system.slave_system_id(), None);
        assert_eq!(system.master_system_id(), None);

        system.set_slave(Some(42));
        assert_eq!(system.slave_system_id(), Some(42));

        system.set_master(Some(99));
        assert_eq!(system.master_system_id(), Some(99));

        system.set_slave(None);
        assert_eq!(system.slave_system_id(), None);
    }

    #[test]
    fn remove_oldest_particles_pops_front_without_touching_newer_particles() {
        let template = Arc::new(ParticleSystemTemplate::new("Budgeted".to_string()));
        let mut system = ParticleSystem::new(template, 1, false);

        for frame in 0..5 {
            system.push_particle(Particle::new(&ParticleInfo::default(), frame, frame));
        }

        assert_eq!(system.remove_oldest_particles(3), 3);
        assert_eq!(system.particle_count(), 2);

        let remaining_timestamps = system
            .particles
            .iter()
            .map(|particle| particle.create_timestamp)
            .collect::<Vec<_>>();
        assert_eq!(remaining_timestamps, vec![3, 4]);
        assert_eq!(system.remove_oldest_particles(99), 2);
        assert_eq!(system.particle_count(), 0);
    }

    #[test]
    fn test_destroy_flag() {
        let template = Arc::new(ParticleSystemTemplate::new("Test".to_string()));
        let mut system = ParticleSystem::new(template, 1, false);

        assert!(!system.is_destroyed());
        system.destroy();
        assert!(system.is_destroyed());
    }

    #[test]
    fn lifetime_zero_lives_forever_until_invisible() {
        let mut info = ParticleInfo::default();
        info.lifetime = 0;
        info.alpha_keys[0] = Keyframe {
            value: 1.0,
            frame: 0,
        };
        let mut particle = Particle::new(&info, 1, 0);
        assert_eq!(particle.lifetime_left, 0);
        assert!(particle.is_lifetime_active());
        for frame in 0..40 {
            assert!(
                particle.update(Vec3::ZERO, frame, ParticleShaderType::AlphaTest),
                "Lifetime=0 must not expire at frame {frame}"
            );
            assert_eq!(particle.lifetime_left, 0);
        }
    }

    #[test]
    fn test_saveable_cascade_flag() {
        let template = Arc::new(ParticleSystemTemplate::new("Test".to_string()));
        let mut system = ParticleSystem::new(template, 1, false);

        assert!(system.is_saveable());
        system.set_saveable(false);
        assert!(!system.is_saveable());
    }

    #[test]
    fn system_lifetime_zero_is_forever() {
        let mut template = ParticleSystemTemplate::new("Forever".to_string());
        template.info_mut().system_lifetime = 0;
        let mut system = ParticleSystem::new(Arc::new(template), 1, false);
        assert!(system.is_system_forever());
        assert!(system.system_lifetime_left == 0 || system.is_system_forever());
    }

    #[test]
    fn attached_host_object_id_follows_published_pose() {
        let template = Arc::new(ParticleSystemTemplate::new("HostAttach".to_string()));
        let mut system = ParticleSystem::new(template, 1, false);
        system.attach_to_object(4242);
        gamelogic::helpers::set_host_fx_object_pose(gamelogic::helpers::HostFxObjectPose {
            id: 4242,
            position: gamelogic::common::Coord3D::new(10.0, 20.0, 30.0),
            transform: gamelogic::common::Matrix3D::from_cols_array(&[
                1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 10.0, 20.0, 30.0, 1.0,
            ]),
            player_index: 0,
            bounding_circle_radius: 0.0,
            is_shrouded: false,
        });
        // Parent resolution and world composition are consecutive phases of
        // the ordinary update. Stop emission to isolate attachment following.
        system.stop();
        assert!(system.update(0, 1));
        assert!(!system.is_destroyed());
        assert_eq!(system.attached_object_id(), 4242);
        assert!((system.position().x - 10.0).abs() < 1e-5);
        assert!((system.position().y - 20.0).abs() < 1e-5);
        assert!((system.position().z - 30.0).abs() < 1e-5);
        gamelogic::helpers::remove_host_fx_object_pose(4242);
    }

    #[test]
    fn attached_host_object_fogged_sets_shrouded_emit_gate() {
        let template = Arc::new(ParticleSystemTemplate::new("HostFog".to_string()));
        let mut system = ParticleSystem::new(template, 1, false);
        system.attach_to_object(4243);
        gamelogic::helpers::set_host_fx_object_pose(gamelogic::helpers::HostFxObjectPose {
            id: 4243,
            position: gamelogic::common::Coord3D::new(1.0, 2.0, 3.0),
            transform: gamelogic::common::Matrix3D::from_cols_array(&[
                1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 1.0, 2.0, 3.0, 1.0,
            ]),
            player_index: 0,
            bounding_circle_radius: 0.0,
            is_shrouded: true,
        });
        system.resolve_attached_parent(0);
        assert!(!system.is_destroyed());
        assert!(system.is_shrouded());
        gamelogic::helpers::remove_host_fx_object_pose(4243);
    }

    #[test]
    fn attached_missing_host_object_still_destroys() {
        let template = Arc::new(ParticleSystemTemplate::new("HostMissing".to_string()));
        let mut system = ParticleSystem::new(template, 1, false);
        system.attach_to_object(4343);
        gamelogic::helpers::remove_host_fx_object_pose(4343);
        system.resolve_attached_parent(0);
        assert!(system.is_destroyed());
        assert_eq!(system.attached_object_id(), 0);
    }
}

#[cfg(test)]
#[path = "particle_system/owned_info_tests.rs"]
mod owned_info_tests;

#[cfg(test)]
#[path = "particle_system/emission_cursor_tests.rs"]
mod emission_cursor_tests;

#[cfg(test)]
#[path = "particle_system/color_scale_tests.rs"]
mod color_scale_tests;

#[path = "particle_system/integration.rs"]
mod integration;
pub(super) use integration::ParticleIntegrationStep;

#[path = "particle_system/generation.rs"]
mod generation;
#[path = "particle_system/random.rs"]
mod random;
#[path = "particle_system/state.rs"]
mod state;

#[cfg(test)]
#[path = "particle_system/native_load_tests.rs"]
mod native_load_tests;

#[cfg(test)]
#[path = "particle_system/retail_quirk_tests.rs"]
mod retail_quirk_tests;
